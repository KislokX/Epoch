# Full Engineering Audit — Epoch

> **Date:** 2026-08-08
> **Scope:** the whole tree — `BUILD/crates` (32,270 LOC Rust), `BUILD/ui/src` (13,811 LOC TS/TSX,
> 6,692 LOC CSS), 77 documents, 28 ADRs.
> **Question asked:** does the implementation faithfully match the architecture we accepted?
> **Method:** measured. Every claim below cites a file, a count, or a command's output.

---

## 0. Method, and what "measured" means here

Everything numeric in this document came from running something, not from reading the code and
forming an impression:

| Measurement | Command | Result |
|---|---|---|
| Rust size | `find crates -name "*.rs" \| xargs wc -l` | 32,270 |
| Frontend size | same over `ui/src` | 13,811 TS + 6,692 CSS |
| Tests | `grep -rn "#\[test\]"` per crate | kernel 89 · engine 409 · **tauri 2** · **UI 0** |
| Suite | `cargo test --workspace` | 407 pass, 0 fail, 2 ignored |
| Lint | `cargo clippy --workspace --all-targets` | 27 warnings, 0 errors |
| Format | `cargo fmt --check` | **601 diff hunks** |
| Panic surface | `grep unwrap/expect/panic!` | 571 sites (87 `expect`) |
| `unsafe` | `grep unsafe` | 7 (5 real, all in `secrets.rs` + 1 Tauri init + 1 test shim) |
| Locking | `grep "\.lock()" state.rs` | **84** |
| God object | `grep "    pub fn " state.rs` | **94 public methods, 2,917 LOC** |
| IPC surface | `grep "#\[tauri::command\]"` | **79 commands** |
| Version control | `git status` | **fatal: not a git repository** |
| CI | `ls .github` | **absent** |

Two of those lines are the audit's headline, and neither is about architecture.

---

## 1. Score

**Overall: 71 / 100.**

A foundation with an unusually strong core and an unusually weak envelope. The domain model is
better than most production systems I would expect at this size; the engineering process around
it is not yet a process.

| Category | Score | Why — with evidence |
|---|---:|---|
| **Architecture** | 82 | The principles are real and mostly enforced *by the compiler*, not by discipline. `epoch-kernel` depends on `serde` and nothing else; `grep "std::fs\|std::net\|std::process"` over the kernel returns **0**. That is the strongest thing in this codebase. Points lost for one large drift (§2.1). |
| **Kernel** | 90 | 626 + 810 + 780 + 583 + 503 LOC of closed, total types with 89 tests. `Reasoning`, `Autonomy`, `Verdict`, `QuestState` are closed enums whose exhaustiveness caught two real bugs during this session (adding `XHigh` produced a compile error in `anthropic.rs`, which is exactly what a kernel is for). |
| **Engine** | 79 | 35 modules, clean seams (`Provider`, `Agent`, `Capability`), 409 tests. Loses points for `provider.rs` (1,082) and `turn.rs` (1,052) doing several jobs each, and for the agent turn having *no* engine-side orchestrator (§2.1). |
| **Runtime** | 68 | The Definition/Runtime/Instance split (ADR-0011) is honoured for characters. But the *turn* runtime is split across `engine/turn.rs` (models) and `tauri/state.rs` (agents), which is a seam in the wrong crate. |
| **UI** | 61 | Real strengths (derived camera, honest instruments) undermined by four files over 1,000 LOC — `Dialogue.tsx` 1,222, `CharacterPanel.tsx` 1,212, `WorldScreen.tsx` 1,022, `LauncherScreen.tsx` 1,022 — and **zero tests**. |
| **Experience Layer** | 74 | ADR-0022's "derive, never store" is genuinely followed in `useCamera` and `useVisit`. Lost points: `useTurn.ts` (661 LOC) now holds 20+ `useState`s and is drifting into a store. |
| **Performance** | 64 | No measured hot path is broken, but there is unmeasured waste: a 2 Hz directory scan (§7.1), an agent probe that spawns **two processes per call from four call sites** (§7.2), and a per-`ClaudeCode::default()` PATH scan (§7.3). |
| **Code Quality** | 70 | Comment quality is exceptional and rare — comments explain *why*, cite evidence, and record refuted hypotheses. Against that: 601 unformatted hunks, 27 clippy warnings, and a declared MSRV that is false (§6.2). |
| **Maintainability** | 62 | `state.rs` at 94 public methods is the single biggest obstacle. A new contributor cannot answer "where does this go?" — the honest answer today is "state.rs", which is the definition of a God object. |
| **Testability** | 55 | 498 tests is a strong number hiding a bad distribution: **2 tests for 4,212 LOC of `epoch-tauri`**, which is where quests are inaugurated, trust modes read, agents spawned and threads keyed. The riskiest code is the least tested. |
| **Product Readiness** | 58 | The World runs, agents work, approvals work end to end. It is a compelling demo and an unshippable product: no crash reporting, no telemetry-free error surface, no update path, no packaging story. |
| **Scalability** | 55 | Nothing is O(n²) in an obvious place, but several structures assume small n without saying so: `QuestLog` holds **every** Quest of every World in one `BTreeMap` in memory and rewrites the whole file on every save (§7.4). |
| **Security** | 76 | Genuinely good instincts: loopback-only bind, constant-time token compare, `Origin` → 403, 4 MB body cap, DPAPI secrets, `csp` is the one gap. Deducted for `csp: null` and 571 panic sites reachable from IPC. |
| **Documentation** | 86 | 77 documents, 28 ADRs, a real archival practice (`docs/History/`). The best-documented codebase of this size I have audited. Deducted for stale statements now contradicted by code (§10). |
| **Technical Debt** | 60 | Debt is *known and named* — which is worth a lot — but the biggest items (no VCS, no CI, God object) have been carried for a long time. |

---

## 2. Architectural Audit

### 2.1 CRITICAL — ADR-0003 is violated by `epoch-tauri/src/state.rs`

**The claim.** `main.rs` opens with:

> "This binary owns **no business logic** (ADR-0003). It starts the engine, exposes IPC commands,
> forwards the engine's projections, and nothing else. Everything it can do, the engine can also
> do headless."

**The evidence.** That sentence is false today, and measurably so:

- `state.rs` is **2,917 LOC with 94 public methods** and **84 `.lock()` calls**.
- It inaugurates Quests: `inner.quests.inaugurate(&world, &id, said, Lifecycle::default())`.
- It reads and applies Trust: `TrustStore::load(&vault_dir()).mode(&world)`.
- It drives the Simulation: `instance.start_working(...)` / `stop_working()`.
- It owns the entire agent turn: `prepare_agent_turn` (≈120 LOC) + `run_agent_turn` (≈90 LOC).
- `grep -rn "agent" crates/epoch-engine/src/turn.rs` returns **0** — the engine has a turn
  orchestrator for models and **none** for agents.

So the model turn is headless and the agent turn is not. Half the runtime cannot exist without a
window.

**Why it happened.** Understandable and not careless: the agent turn needs `AppHandle` to emit
`ASKING`, and the door needs a window. But that argues for the *event sink* being injected, not
for the whole orchestration living in the shell.

**Risk.** High and compounding.
1. The CLI surface (`PRODUCT_ARCHITECTURE.md` says surfaces are architecturally equal) cannot run
   an agent Quest at all. The architecture claims parity the code cannot deliver.
2. It is the least-tested code in the repo (2 tests / 4,212 LOC) *and* the most stateful.
3. Every new subsystem lands here because it is where the locks are — the God object grows by
   gravity.

**Correction.** Move the agent turn into the engine as `epoch-engine/src/turn/agent.rs`, mirroring
`turn::run`, taking a `&dyn Asking` sink instead of `AppHandle`. `state.rs` keeps only: hold the
`Arc`, translate IPC types, forward events. Target: `state.rs` under 900 LOC.

**Priority: P0.** Do this before another subsystem is added.

### 2.2 HIGH — the session/thread map is engine state living in the shell

`state.rs` holds:

```rust
threads: Mutex<BTreeMap<(String /*quest*/, CharacterId), String>>,
agent_windows: Mutex<BTreeMap<(String, CharacterId), (u64, u64)>>,
```

These are **domain facts** — which conversation an agent is continuing, how full its window is —
stored outside the domain, unpersisted, and keyed by stringly-typed tuples. `QuestId` exists and is
not used as the key type; `String` is.

**Risk.** A restart silently loses every agent thread with no user-visible statement that it did.
Worse, "session-scoped" is a decision nobody can find: it lives in a field comment in the shell.

**Correction.** `Quest` owns its agent sessions: `sessions: BTreeMap<CharacterId, SessionHandle>`
on the Quest, persisted with it (ADR-0014). Then "each Quest is a session" is enforced by the type
rather than by two parallel maps agreeing.

**Priority: P1.**

### 2.3 MEDIUM — `Conversation` and `QuestView` are two projections of one thing

`state.rs` defines `QuestView` (for the Chronicle) and `Conversation` (for the list), overlapping in
`id`, `title`, `state`, `producedEvidence`, and disagreeing in shape: `QuestView.said` is a `Vec<Said>`
while `Conversation.said` is a `usize`. The frontend has both and they are the same field name with
different meanings.

**Risk.** Low today, guaranteed confusion later. This is exactly the "two documents answering one
question" rule applied to types.

**Correction.** One `QuestSummary` + an optional `chronicle` the caller asks for.

**Priority: P2.**

### 2.4 What is *not* drifting — and deserves saying

- **Internal First holds.** Every projection is built in `world.rs` / `launcher.rs` and never
  reaches back. `PlaceView`, `CharacterView`, `MapView` are one-directional.
- **The kernel boundary is compiler-enforced.** Zero I/O imports. This is the single best decision
  in the project and it has paid for itself repeatedly.
- **Runtime over Configuration holds.** `Brain` is `Model | Agent`, dispatched once at the top
  (`brain_of`), and nothing downstream branches on which it got.
- **Cold instruments are real.** `AgentStatus.signed_in: Option<bool>` distinguishes *unasked* from
  *signed out*. That distinction is enforced in `read_whoami` and tested.

---

## 3. Kernel Audit

**Score 90.** The strongest crate. Specific findings:

### 3.1 `Reasoning` is now a leaky ladder (MEDIUM)

`Reasoning::XHigh` was added this session because Claude Code has it. The doc comment justifies it
honestly ("a canonical ladder is allowed to be finer than a backend"), and `deliberation.rs` keeps
per-brain scales so no surface offers an unsupported rung. But note what happened: **a vendor's
capability changed a Kernel enum.** One more vendor doing that (OpenAI's `minimal`) and the ladder
becomes a union of everyone's, which is the opposite of canonical.

**Correction to consider:** make the rung an ordered scalar (`Deliberation(u8)` 0–100) with named
constants, and let each brain quantise. Not urgent; flag it before the third backend.

### 3.2 `Parameters.context_tokens` is admitted to be wrong and still there (LOW)

Its own doc says: *"The weakest member of the canonical set, and knowingly so… it reads closer to a
resource preference than to identity."* That is a correct self-diagnosis. It stays because the
Composer needs it. Fine — but it should be an issue, not a comment.

### 3.3 `QuestId::from_raw` accepts anything (MEDIUM)

```rust
pub fn from_raw(raw: impl Into<String>) -> Self { Self(raw.into()) }
```

`CharacterId::new` validates and returns `Result`; `QuestId` does not. `state.rs::select_quest` now
passes user-supplied strings straight into `from_raw`. It is safe today only because `select`/`close`
check World membership. That is defence at the wrong layer — the same pattern that made
`CharacterId` validating in the first place.

**Correction:** `QuestId::parse(&str) -> Option<Self>` matching the minted format.

### 3.4 API inconsistency: three naming conventions for the same act (LOW)

`CharacterId::new` (validating), `QuestId::from_raw` (not), `Reasoning::from_id` (validating),
`CharacterArchetype::from_id` (validating). Pick one: `parse` for fallible, `new` for infallible.

---

## 4. Engine Audit

### 4.1 HIGH — `Invocation::default()` performs filesystem work

`resolve()` scans `PATH`, stats candidates, and may read `claude.cmd` — and it runs inside
`Default::default()`, which `agents::installed()` calls, which four UI paths call. A `Default` impl
that touches the disk is a trap: it is called in places nobody expects I/O.

**Correction:** resolve once behind `OnceLock`. Same answer, one scan per process.

### 4.2 HIGH — `probe()` spawns two processes, uncached, from four call sites

`ClaudeCode::probe` runs `--version` **and** `auth status --json`, each bounded by a 6 s timeout.
`fetchAgents()` is called from `CharacterPanel`, `LauncherScreen`, `ConnectionsPanel`, and
`Dialogue` (on failure). Opening the Launcher with the panel visible can spawn 4–8 processes.

**Correction:** cache the survey in `World` with a short TTL and an explicit "probe again" that
bypasses it. The button already exists; the caching does not.

### 4.3 MEDIUM — no Activity Stream (ADR-0015) despite eight subsystems wanting one

`ls crates/epoch-engine/src` shows no `activity.rs`. Every subsystem that would publish instead
either emits a Tauri event directly (shell coupling) or returns values up the stack. This is
"DESIGN NOW" per the ADR, so it is not drift — but the cost is now visible: the Terminal, the
Ship's Log, the Chronicle and the Simulation each learn about work through a different path.

### 4.4 MEDIUM — `Reading` accumulates unboundedly

`Reading.pending` holds every `tool_use` awaiting a result. An agent that calls 10,000 tools in one
turn grows it without limit, and `Done.text` accumulates every token. Fine at today's scale;
worth a cap before "long-running Quests" is claimed.

### 4.5 LOW — 14 `result_large_err` warnings

Clippy reports 14 functions returning a very large `Err` variant. `DefinitionError` carries
`PathBuf` + `toml::de::Error`. Box the error or shrink it; this is free.

---

## 5. UI Audit

### 5.1 CRITICAL — zero frontend tests

`find ui -name "*.test.ts*"` → **0**. 13,811 LOC of TypeScript with no test of any kind: no unit
test, no render test, no contract test against the IPC types. The `contracts.ts` file (679 LOC) is a
hand-maintained mirror of Rust types with **nothing asserting they still match** — a rename on the
Rust side is caught only when a user sees `undefined`.

**Minimum correction:** Vitest + one contract test per IPC boundary that asserts the serialised
shape from a Rust fixture. This is the single highest-value test investment available.

### 5.2 HIGH — four components over 1,000 LOC

`Dialogue.tsx` (1,222) contains: the Chronicle, the composer, resize/drag, the context popover, the
reasoning dial, the approval panel, mode selection, standing decisions, sign-in recovery, and
invitation. Ten responsibilities in one file.

**Correction:** extract `<ReasoningDial>`, `<ContextPopover>`, `<ApprovalPrompt>`,
`<ModePicker>` — all pure, all testable, none needing `useTurn`.

### 5.3 HIGH — `useTurn` is becoming a store

661 LOC, 20+ `useState`, and now cross-cutting concerns (remembered context, planned name). Two
symptoms already appeared during this session: an effect keyed on a stable callback that therefore
ran once (the context bug), and a name collision with an existing `wanted` state.

**Correction:** `useReducer` with an explicit `TurnState`, or split into `useChronicle`,
`useSpeaking`, `useApproval`.

### 5.4 MEDIUM — business logic in components

`CharacterPanel.tsx` decides which rungs to show, which model names are valid, and how a brain maps
to a form. `deliberation.rs` already answers the first; the other two are domain rules living in
JSX.

### 5.5 MEDIUM — accessibility is inconsistent

Good: `aria-modal`, `aria-expanded`, `aria-current` on pagination, `aria-label` on icon buttons.
Missing: no focus trap in the Missions modal, no `Escape` to close it, focus is not restored on
close, and the World stage has no keyboard path at all (`Visit` is mouse-only).

### 5.6 MEDIUM — the pointer-events rule is a repeated footgun

`CLAUDE.md` documents it; it was violated again this session by `.missions__over`. A rule that must
be remembered for every new overlay will be forgotten again.

**Correction:** one `<Overlay>` component that owns `pointer-events: auto`, the scrim, Escape, and
the focus trap. Then the rule is a type, not a memory.

### 5.7 LOW — 6,692 LOC of CSS in a handful of files

`hud.css` and `launcher.css` are long enough that dead rules are undetectable — one 154-line dead
block already caused a layout bug earlier in this project's history. No tooling detects the next
one.

---

## 6. Code Quality Audit

### 6.1 CRITICAL — no version control

`git status` → `fatal: not a git repository`. 46,000 LOC, months of decisions, no history, no
branches, no blame, no bisect, no PRs, no rollback. Every claim in every doc comment about "an
earlier version did X" is unverifiable, and every refactor below is irreversible.

**This is the single most important finding in the audit.** Nothing else on this list can be worked
on safely until it is fixed, and it takes ten minutes.

### 6.2 HIGH — the declared MSRV is false

`Cargo.toml` says `rust-version = "1.80"`. The code uses `is_none_or` (stable 1.82) in five places;
clippy flags each one. Either the floor is 1.82 or the code must not use it. A build constraint that
is documented and wrong is worse than an undocumented one.

### 6.3 HIGH — `cargo fmt --check` reports 601 hunks

Formatting is not cosmetic at this size: it is what makes a diff reviewable. With no VCS and no CI,
nothing has ever enforced it.

### 6.4 MEDIUM — 87 `expect()` calls, 35 in `state.rs`

`.expect("threads lock")`, `.expect("endpoint lock")`, `.expect("agent windows lock")`. Each is a
panic reachable from IPC. `World::lock()` already does the right thing —
`unwrap_or_else(|e| e.into_inner())`, with a comment explaining why — but the *other* 35 locks in
the same file do not follow it.

**Correction:** a private `fn guard<T>(m: &Mutex<T>) -> MutexGuard<T>` used everywhere.

### 6.5 MEDIUM — magic numbers with no home

`PROBE_PATIENCE = 6s`, `PATIENCE = 300s`, `WAKE = 250ms`, `HEARTBEAT = 250ms`, `WORKERS = 4`,
`TICK = 500ms`, `PER_PAGE = 15`, `DEFAULT_FOOTPRINT = 90.0`. Each is named and documented where it
lives — good — but they are scattered across six files with no way to see them together.

### 6.6 What is genuinely excellent

The comments. Not their volume — their *epistemics*. They record what was measured, what was
refuted, and what was assumed:

> "Two of these were guesses, and both guesses would have failed on the first real run."

> "The flag was found by asking the program, the call shape by pointing it at a throwaway MCP
> server and reading what arrived."

I have not audited a codebase that distinguishes measured from assumed this consistently. It should
be preserved through every refactor below.

---

## 7. Performance Audit

### 7.1 The 2 Hz disk scan

`reload_if_changed()` runs every 500 ms and calls `changed_on_disk()`, which does a `read_dir` plus
a `metadata()` per character. With 20 characters that is 42 syscalls/second, forever, to detect an
edit that happens a few times an hour. It also takes the **World lock** to do it.

**Correction:** filesystem watcher, or back off to 2 s when nothing has changed for a minute.

### 7.2 Agent probes (see §4.2) — up to 8 process spawns per screen.

### 7.3 `resolve()` per `Default::default()` (see §4.1).

### 7.4 `QuestLog` is whole-file, in-memory, for all Worlds

`QuestLog::save` serialises **every Quest of the World** on every turn. `history()` filters the full
map and sorts. With 34 conversations this is free; with 5,000 turns of Chronicle it is a
multi-megabyte rewrite per message.

**Correction before "Long-running Quests":** one file per Quest, an index for the list.

### 7.5 React: no memoisation where it matters

`WorldScreen` has 4 `useMemo/useCallback` for 1,022 LOC and re-renders on every `world:changed`
(every presence change, ~seconds). `chats` is re-fetched on `turn.quest?.said.length`. Nothing is
virtualised. Today's scale hides all of it.

### 7.6 What is right

- `world:changed` emits **only on actual change** (`if reloaded || now != last`).
- Large assets deliberately left out of the per-tick projection — documented in `CLAUDE.md` and
  followed.
- The camera is derived, not stored, which eliminates a class of ordering bugs by construction.

---

## 8. Security Audit

### 8.1 Strong, and worth naming

`endpoint.rs` is the best-reasoned file in the project: loopback-only bind ("a bind address is not a
setting"), bearer token required ("loopback is not authentication"), `Origin` present → **403 not
401** (a 401 teaches a page that a token exists), constant-time compare, 4 MB body cap, token
redacted in `Debug`. `secrets.rs` uses DPAPI and confines all `unsafe` to one file.

### 8.2 HIGH — `csp: null`

`tauri.conf.json` disables Content Security Policy. The webview renders `data:` URIs from imported
artwork and, through MCP, text produced by a remote model. No CSP means a single injection point
anywhere becomes script execution in a process holding the user's filesystem capabilities.

**Correction:** a strict CSP now, while the surface is small.

### 8.3 HIGH — the token outlives the door it describes

`Token::remembered(vault)` persists to `vault/agent-token` and is reused across restarts. The
`--mcp-config` path is per-run and correct; the persisted token is the older design and still there.
A stolen vault file is a permanent key to a localhost port that can run capabilities.

**Correction:** rotate per session; keep the persisted one only for the manual paste flow, and mark
it as such.

### 8.4 MEDIUM — panics are a denial-of-service surface

571 panic sites, 87 explicit `expect`. A panic in an IPC command poisons the `Mutex`; `World::lock`
recovers, but the *other* 35 sites re-panic on a poisoned lock, cascading.

### 8.5 MEDIUM — `open_sign_in` builds a raw command line

`raw_arg` with a `format!`ed string containing an interpolated program path. The path comes from
`resolve()` (our own PATH scan, not user input), so it is not exploitable today — but it is
string-built shell, one refactor away from taking a user-supplied path.

### 8.6 LOW — no path canonicalisation audit for `--add-dir` equivalents

`ProjectRoot` keeps authored and canonical forms, which is right. Capabilities check containment.
Worth a property test with `..`, symlinks, and UNC paths before external contributors arrive.

---

## 9. Testing Audit

### 9.1 The distribution is the finding

| Crate / area | LOC | Tests | Tests per 1k LOC |
|---|---:|---:|---:|
| `epoch-kernel` | ~3,300 | 89 | 27 |
| `epoch-engine` | ~24,700 | 409 | 17 |
| **`epoch-tauri`** | **4,212** | **2** | **0.5** |
| **`ui/src`** | **13,811** | **0** | **0** |

The two least-tested areas are where quests are inaugurated, trust is read, agents are spawned,
sessions are keyed, and every user interaction happens.

### 9.2 Weak tests / false confidence

- **407 passing tests did not catch** the three defects found by *using* the product this session:
  the refused-tool-shown-as-work bug, the once-only effect, and the pointer-events omission. All
  three are integration-shaped; all three are in untested layers.
- `#[ignore]`d tests (2) are the only ones touching the real CLI. They are excellent and they never
  run automatically.

### 9.3 Missing, in priority order

1. **IPC contract tests** — serialise each Rust view, assert against a TS fixture. Kills a whole
   bug class.
2. **A headless turn test** — a scripted `Agent` impl driven end to end. Impossible today because
   the agent turn lives in the shell (§2.1); this is the test that would justify that refactor.
3. **Property tests** on `ProjectRoot` containment and `QuestId` round-trip.
4. **A stress test** for `QuestLog` at 1,000 Quests × 500 entries, to find §7.4 before a user does.
5. **Regression tests** for every defect fixed this session — three exist (refused tools, window
   accounting, scale/effort agreement); the two UI ones have none, because there is no harness.

---

## 10. Documentation Audit

**Score 86 — the best-documented project of this size I have reviewed.** 28 ADRs, six constitutions,
an archival practice, and `CLAUDE.md` as a living index of decisions with dates and evidence.

### Contradictions found (documentation issues, not implementation issues)

1. **`main.rs` header vs reality.** "This binary owns no business logic" — false (§2.1). *The
   implementation is wrong here, not the doc.* Fix the code; keep the sentence.
2. **`CLAUDE.md` 2026-08-07 entry** says an agent's autonomy "is the agent's" and Epoch must not
   display a mode it does not enforce. The 2026-08-08 amendment corrects it. Both are present and
   the reader must get to the second to know the first is superseded. *Mark superseded paragraphs
   inline.*
3. **ROADMAP Phase 1** ("Foundation validation… camera, pan, zoom, minimap") is described as *in
   progress* while Phases well beyond it have shipped (agents, MCP, approvals). The roadmap has not
   kept pace with the build. *This is the doc drifting, not the code.*
4. **ADR-0015 (Activity Stream)** is accepted and unimplemented, and four subsystems now route
   around it. Not drift — but the ADR should say "no implementation as of 2026-08-08" so a new
   contributor does not go looking for `activity.rs`.
5. **`Reasoning` doc in the Kernel** still says the ladder is five rungs in prose while `ALL` has
   six.

### Intentional architectural evolution (correctly recorded)

- ADR-0025's renaming of Quest/Chronicle from presentation to domain terms.
- ADR-0023 taking the cast out of World Packs.
- The 2026-08-08 permission-tool amendment, which explicitly records that the previous claim was
  refuted by measurement. **This is the model for how the rest should be maintained.**

---

## 11. Product Audit

**Would another engineer understand this architecture?** Yes — unusually well. The ADRs, the crate
boundaries and the comments make intent legible.

**Would a contributor know where to implement a feature?** *No.* The honest answer for anything
user-facing today is "add a method to `state.rs` and a command to `main.rs`", and 94 methods later
that is not guidance, it is gravity. This is the gap between a well-architected project and a
contributable one.

**Does the design scale?** The domain does. The process does not: no VCS, no CI, no PR flow, no
issue tracker, no packaging.

**What still feels prototype:** `state.rs`; the UI's four megafiles; `csp: null`; no tests in the
two layers users touch; the Terminal/Ship's Log/Chronicle each learning about work differently.

**What already feels production:** `epoch-kernel` entirely. `endpoint.rs`. `serve.rs`. `secrets.rs`.
`deliberation.rs`. The World projection pipeline. The approval bridge.

---

## 12. Technical Debt

### Critical — before anything else
1. **No version control.** `git init`, `.gitignore`, first commit. (§6.1)
2. **Zero frontend tests + no IPC contract test.** (§5.1, §9.3)
3. **ADR-0003 violation: the agent turn lives in the shell.** (§2.1)

### High — this milestone
4. `csp: null`. (§8.2)
5. `cargo fmt` (601 hunks) + clippy (27) + MSRV honesty, then CI to hold them. (§6.2, §6.3)
6. `state.rs` decomposition: 94 methods → four focused services. (§2.1)
7. Agent probe caching + `OnceLock` for `resolve()`. (§4.1, §4.2)
8. Persist agent sessions on the Quest instead of shell-side maps. (§2.2)

### Medium
9. `Dialogue.tsx` / `CharacterPanel.tsx` extraction. (§5.2)
10. `useTurn` → reducer. (§5.3)
11. One `<Overlay>` primitive owning pointer-events, Escape, focus trap. (§5.6)
12. `QuestId::parse`. (§3.3)
13. Watcher instead of the 2 Hz scan. (§7.1)
14. `QuestLog` per-file persistence before long Quests. (§7.4)
15. ROADMAP re-sync; mark superseded `CLAUDE.md` paragraphs. (§10)

### Low
16. `result_large_err` boxing. (§4.5)
17. Naming convention pass (`new` vs `from_raw` vs `from_id`). (§3.4)
18. CSS dead-rule tooling. (§5.7)
19. Consider `Deliberation(u8)` before the third backend. (§3.1)

---

## 13. Readiness

| For | Ready | Why |
|---|---:|---|
| **Production** | 35% | Works; unpackaged, unversioned, no crash path, `csp: null`. |
| **Open Source** | 25% | No repo, no LICENSE, no CONTRIBUTING, no CI. Docs are ready; nothing else is. |
| **External Contributors** | 30% | Excellent docs, no VCS, no tests where they would work, one God object. |
| **Marketplace** | 20% | Pack contract exists (ADR-0016); no signing, no validation service, no distribution. |
| **Plugin Ecosystem** | 25% | Capability + MCP contracts are strong; no sandbox, no manifest, no versioning. |
| **Multiple Providers** | 70% | `Provider` + declared `Control`s are genuinely provider-agnostic. Anthropic path is unverified for lack of credit. |
| **Cloud Providers** | 40% | Nothing blocks it; nothing proves it. |
| **Large Worlds** | 45% | Projection is whole-world per change; no virtualisation. |
| **Large Teams** | 20% | No VCS. Everything else is secondary to that. |
| **Long-running Quests** | 30% | `QuestLog` whole-file rewrite + unbounded `Reading` + session-only threads. |

---

## 14. Refactoring Plan — by impact

**1. `git init` + `.gitignore` + first commit.**
*Why:* every other item is irreversible without it. *Risk:* none. *Benefit:* history, bisect,
rollback, review. *Complexity:* trivial. *Blocks:* everything.

**2. IPC contract tests + Vitest harness.**
*Why:* the boundary with the most silent-failure surface has no assertions. *Risk:* low.
*Benefit:* kills a bug class permanently; makes the UI refactors safe. *Complexity:* medium.
*Blocks:* items 4 and 6.

**3. Move the agent turn into `epoch-engine`.**
*Why:* restores ADR-0003, makes the CLI surface possible, makes the turn testable headless.
*Risk:* medium — it is live code. Mitigated by item 1 and by a scripted-agent test written first.
*Benefit:* the largest single reduction in `state.rs`. *Complexity:* medium-high. *Blocks:* CLI,
headless testing, any second agent.

**4. Decompose `state.rs` into services** (`QuestService`, `AgentService`, `WorldEditService`,
`ConfigService`), each owning its own lock.
*Why:* 94 methods / 84 locks is where contributors will get stuck and where deadlocks will
eventually live. *Risk:* medium. *Benefit:* answers "where does this go?". *Complexity:* high.
*Blocks:* external contributors.

**5. CSP + token rotation + `expect` → `guard()`.**
*Why:* three cheap fixes closing the three real security gaps. *Risk:* low. *Benefit:* removes the
only findings that could become incidents. *Complexity:* low. *Blocks:* any public release.

**6. `<Overlay>` + component extraction + `useTurn` reducer.**
*Why:* the UI is where every future feature lands and it is the least structured layer. *Risk:*
low with item 2 in place. *Benefit:* new surfaces stop re-deriving modal behaviour. *Complexity:*
medium. *Blocks:* nothing, enables everything.

**7. Persistence: per-Quest files + sessions on the Quest + watcher.**
*Why:* the three things that break at scale, none of which break today. *Risk:* medium (migration).
*Benefit:* "long-running Quests" becomes true. *Complexity:* medium. *Blocks:* that claim only.

---

## 15. Final Verdict

**Would I continue building on this codebase?** **Yes** — with one condition: `git init` before the
next line of code. The domain model is right, the boundaries are real, and the reasoning is
recorded better than in most funded products. That is the expensive part, and it is done.

**The single biggest architectural risk:** `epoch-tauri/src/state.rs`. Not because it is bad code —
it is careful, well-commented code — but because it is where the architecture stops being enforced
and starts being *observed*. The kernel boundary is held by the compiler; the shell boundary is held
by a sentence in a doc comment that is already false. Everything drifts toward the place with no
enforcement.

**The single biggest engineering strength:** the epistemic discipline. `epoch-kernel` with zero I/O
imports is the structural expression of it, and the comments are the cultural one — a codebase that
routinely records *"this was a guess and the run refuted it"* will stay correct longer than one with
twice the tests. Do not let a refactor strip those comments.

**What I would rewrite today:** the agent turn — out of the shell, into the engine, with a scripted
agent test. It is the only piece where the implementation contradicts an accepted ADR rather than
merely lagging it.

**What should absolutely not be touched:** `epoch-kernel`, `endpoint.rs`, `secrets.rs`, and
`serve.rs`. They are load-bearing, correct, tested, and their reasoning is documented. Every
temptation to "simplify" them should be refused.

---

## Executive Summary

### Top 10 strengths

1. **The kernel boundary is compiler-enforced** — zero I/O imports, one dependency. It has already
   caught real bugs through exhaustiveness.
2. **Epistemic comments** — measured vs assumed is stated, refuted hypotheses are recorded.
3. **`endpoint.rs`** — loopback-only, bearer token, 403-not-401, constant-time compare, bounded
   body. Textbook.
4. **Honest instruments** — `signed_in: Option<bool>` distinguishes *unasked* from *no*; the
   cold-instrument rule is real code, not a slogan.
5. **Provider/Agent separation (ADR-0027)** — dispatched once at the top; nothing downstream asks
   which brain it got.
6. **Documentation practice** — 28 ADRs, archival to `docs/History/`, dated amendments citing
   evidence.
7. **The projection pipeline** — one-directional, always succeeds, never shows a loading screen.
8. **The approval bridge** — one waiting slot, four distinct endings, a timeout that refuses.
   Now serving both models and agents.
9. **Derived Experience Layer** — camera and reveal computed from (world + intent + time),
   eliminating ordering bugs by construction.
10. **407 engine/kernel tests that assert behaviour**, not implementation, and read as sentences.

### Top 10 issues before continuing

1. **No version control.** Fix in the next ten minutes. (§6.1)
2. **The agent turn violates ADR-0003** — business logic in the shell, untestable headless. (§2.1)
3. **Zero frontend tests and no IPC contract test** across 13,811 LOC. (§5.1)
4. **`state.rs`: 94 public methods, 84 locks, 2 tests.** (§2.1, §9.1)
5. **`csp: null`** in a webview holding filesystem capabilities. (§8.2)
6. **601 unformatted hunks, 27 clippy warnings, a false MSRV, no CI to hold any of it.** (§6.2–6.3)
7. **Agent probes spawn two processes per call from four call sites, uncached.** (§4.2)
8. **Agent sessions live in shell-side string-keyed maps**, unpersisted, undocumented outside a
   field comment. (§2.2)
9. **`QuestLog` rewrites every Quest on every turn** — fine now, fatal for long Quests. (§7.4)
10. **Four UI files over 1,000 LOC**, with `useTurn` (661) drifting into an unstructured store —
    already the source of two defects this session. (§5.2, §5.3)
