# Road to 100

> **Companion to:** [Full Audit 2026-08-08](Full%20Audit%202026-08-08.md) (71/100).
> **Purpose:** turn every score in that audit into a number that can be *measured*, and lay out
> the order in which the work removes the most risk per unit of effort.

---

## 0. What a 100 has to mean, or the number is decoration

A score somebody can argue with is a score nobody can act on. So each category below gets a
**gate**: a command, a count, or a check that either passes or does not. The score is then not an
opinion — it is a reading, which is the same rule the Launcher's instruments follow.

Two categories are honest exceptions, and pretending otherwise would be the first violation:

- **Product Readiness** is not fully an engineering number. Packaging, updates, crash reporting and
  a first-run experience are engineering; *whether the product is good* is not gate-able. The gate
  covers the engineering half and stops there.
- **Scalability** can only be gated against declared limits. "Scales" is meaningless; "1,000
  Quests × 500 entries opens in under 200 ms, asserted by a test" is not. Every scalability claim
  below names its limit.

| Category | 100 means | Gate |
|---|---|---|
| Architecture | No accepted ADR is contradicted by code | `docs/ADR/_index.md` lists a *verification* per ADR; a test or a compile-time boundary for each |
| Kernel | Closed, total, zero I/O, no vendor leakage | `grep std::fs\|std::net\|std::process crates/epoch-kernel` = 0 · every enum exhaustively matched · 100% of public API documented |
| Engine | Every subsystem testable headless | No engine module needs a window · `cargo test -p epoch-engine` covers every public entry point |
| Runtime | One turn orchestrator, both brains | `turn::run` and `turn::agent::run` in the engine · a scripted `Agent` test drives a full Quest |
| UI | Presentation only, tested | 0 files > 400 LOC · no domain rules in components · Vitest suite green |
| Experience Layer | Derived, never stored | State holds user intent only · camera/reveal computed on read · asserted by test |
| Performance | Every hot path measured | No unbounded poll · every declared limit has a benchmark |
| Code Quality | Enforced, not remembered | `cargo fmt --check` clean · `clippy -D warnings` clean · MSRV true |
| Maintainability | A contributor knows where code goes | No file > 900 LOC · no type with > 30 public methods · module map in CONTRIBUTING |
| Testability | The risky code is the tested code | Every crate ≥ 10 tests / 1k LOC · UI ≥ 5 / 1k |
| Product Readiness | Installable, updatable, diagnosable | Signed installer · update channel · crash report the user can send · first run works with nothing configured |
| Scalability | Declared limits, asserted | Stress tests at the stated numbers, in CI |
| Security | No unreviewed trust | CSP on · no panic reachable from IPC · secrets never on disk · dependency audit in CI |
| Documentation | No contradiction, no drift | Every ADR states implementation status and date · doc lint in CI |
| Technical Debt | Named, owned, dated | Zero items in Critical/High older than one milestone |

---

## Phase 0 — Make change safe *(done 2026-08-08)*

Everything else edits live code. This phase makes that reversible.

- [x] **`git init`, private, no remote.** 268 files, first commit `b3c9e53`.
- [x] **`.gitignore`** excluding build outputs (19 GB), `node_modules`, reference clones, browser
      capture noise, and **`BUILD/vault/agent-token`** — a credential does not belong in a
      repository, private or not: repositories are copied to laptops, restored from backups, and
      shared the day they stop being private.
- [x] **`.gitattributes`** pinning line endings so the first cross-platform edit does not rewrite
      all 268 files.

**Remaining in Phase 0, before Phase 1 starts:**

- [ ] **Branch discipline.** `main` protected by habit: work on `feat/*`, merge with a message
      that says *why*. Cheap now, impossible to retrofit.
- [ ] **`docs/Build/Conventions.md`** — the commit message shape, the branch shape, and the rule
      that a commit which changes behaviour changes a test.

> **Why this is Phase 0 and not item 1 of Phase 1:** every recommendation below is a refactor of
> working code. Without history, a bad refactor is indistinguishable from a rewrite.

---

## Phase 1 — Make regression impossible *(Testability 55 → 85, Code Quality 70 → 92)*

The audit's sharpest finding is that **407 passing tests did not catch the three defects found by
using the product**. All three were in layers with no tests. Adding more engine tests would not
have helped; the gap is structural.

### 1.1 A frontend test harness *(blocks: every UI item below)*

**Why.** 13,811 LOC with zero tests, and `contracts.ts` (679 LOC) mirrors Rust types by hand with
nothing asserting they still match. A field rename on the Rust side is currently discovered by a
user seeing `undefined`.

**Do.** Vitest + Testing Library. First three tests, in this order:
1. `Missions` renders 34 conversations across 3 pages and the ✕ calls `onClose`.
2. `useTurn` clears `knew` when the character changes (the exact defect from this session).
3. Reasoning dial: a brain with an empty scale renders **no** control.

**Risk.** None — additive.
**Benefit.** The refactors in Phase 3 stop being dangerous.
**Complexity.** Low. **Blocks:** 3.1, 3.2, 3.3.

### 1.2 IPC contract tests *(the single highest-value test in the project)*

**Why.** The Rust ↔ TypeScript boundary carries 79 commands and a dozen view types, and nothing
verifies the two sides agree.

**Do.** For each view type, a Rust test writes `serde_json` output to `ui/src/ipc/__fixtures__/`,
and a Vitest test asserts the TypeScript type accepts the fixture. A rename then fails in CI
instead of in the app.

**Risk.** Low. **Benefit.** Kills a whole bug class permanently. **Complexity.** Medium.
**Blocks:** confident renaming anywhere in the engine.

### 1.3 CI, and the gates it holds

**Why.** `cargo fmt --check` reports **601 hunks** and clippy 27 warnings, because nothing has ever
enforced either. Formatting is not cosmetic at this size — it is what makes a diff reviewable.

**Do.** One workflow: `fmt --check` · `clippy -D warnings` · `cargo test --workspace` ·
`npm run typecheck` · `npm test` · `cargo deny check` (licences + advisories). Run `cargo fmt` and
`clippy --fix` **once, in their own commit**, so the noise never mixes with a behaviour change.

**Also fix here:** `rust-version = "1.80"` is false — the code uses `is_none_or` (1.82) in five
places. Set the floor to what is true.

**Risk.** None. **Benefit.** Every quality number stops regressing silently. **Complexity.** Low.

**Exit criteria.** CI green on a clean checkout. `fmt` and `clippy` clean. UI suite exists and runs.

---

## Phase 2 — Restore the architecture *(Architecture 82 → 96, Runtime 68 → 92)*

### 2.1 Move the agent turn into the engine *(the one real ADR violation)*

**Why.** `main.rs` says the shell owns no business logic (ADR-0003). It does:
`prepare_agent_turn` + `run_agent_turn` (≈210 LOC) inaugurate Quests, read Trust, drive the
Simulation and key sessions — and `grep -rn "agent" crates/epoch-engine/src/turn.rs` returns **0**.
The model turn is headless; the agent turn cannot exist without a window. That makes the CLI
surface — which `PRODUCT_ARCHITECTURE.md` calls architecturally equal — impossible to build.

**Do.**
1. `epoch-engine/src/turn/agent.rs` with `pub fn run(...)`, mirroring `turn::run`.
2. Replace `AppHandle` with a `&dyn Witness` sink (`asked`, `settled`, `step`, `token`, `window`).
   The shell implements it by emitting Tauri events; a test implements it by pushing to a `Vec`.
3. `state.rs` keeps only: hold the `Arc`, translate IPC types, forward events.

**Risk.** Medium — live code. Mitigated by writing the scripted-agent test *first* (1.1 and 1.2
must land before this).
**Benefit.** The largest single reduction in the God object; the CLI becomes possible; the turn
becomes testable headless.
**Complexity.** Medium-high. **Blocks:** CLI surface, second agent (Codex), any headless test of a
Quest.

### 2.2 Sessions belong to the Quest

**Why.** `threads: Mutex<BTreeMap<(String, CharacterId), String>>` in the shell is a domain fact
stored outside the domain, stringly-typed, unpersisted, and documented only in a field comment.

**Do.** `Quest.sessions: BTreeMap<CharacterId, SessionHandle>`, persisted with the Quest
(ADR-0014). "Each Quest is a session" then becomes a property of the type instead of an agreement
between two maps.

**Risk.** Low (migration is additive — an absent field means no session).
**Benefit.** Sessions survive restart; the invariant is enforced. **Complexity.** Low-medium.

### 2.3 One `QuestSummary`

`QuestView` and `Conversation` overlap and disagree: both have `said`, one a `Vec<Said>`, one a
`usize`. Collapse to one summary + an optional chronicle the caller asks for.

**Exit criteria.** `state.rs` under 900 LOC. A headless test drives a full agent Quest with a
scripted `Agent`. Every ADR has a named verification in `docs/ADR/_index.md`.

---

## Phase 3 — Make the UI a presentation layer again *(UI 61 → 92, Experience 74 → 95)*

### 3.1 The `<Overlay>` primitive

**Why.** The pointer-events rule is documented in `CLAUDE.md` and was violated again this session
by `.missions__over` — a window that rendered perfectly and could not be clicked. A rule that must
be remembered for every new overlay will be forgotten again.

**Do.** One component owning: `pointer-events: auto`, the scrim, `Escape`, focus trap, focus
restore. Missions and Dialogue adopt it. The rule becomes a type.

### 3.2 Break up the four megafiles

`Dialogue.tsx` 1,222 · `CharacterPanel.tsx` 1,212 · `WorldScreen.tsx` 1,022 ·
`LauncherScreen.tsx` 1,022. `Dialogue` alone holds ten responsibilities.

**Do.** Extract pure, testable pieces: `<ReasoningDial>`, `<ContextGauge>`, `<ApprovalPrompt>`,
`<ModePicker>`, `<ChronicleList>`, `<Composer>`. Target: no file over 400 LOC.

### 3.3 `useTurn` → reducer

661 LOC and 20+ `useState`. Two defects this session came directly from that shape: an effect keyed
on a stable callback (ran once, forever) and a name collision with existing state.

**Do.** `useReducer` over an explicit `TurnState`, or split into `useChronicle` / `useSpeaking` /
`useApproval`.

### 3.4 Domain rules out of components

`CharacterPanel` decides which rungs to show and which model names are valid. `deliberation.rs`
already answers the first — the frontend should ask, not know.

### 3.5 Accessibility to a stated bar

Focus trap and `Escape` in every overlay, focus restore on close, and a keyboard path to `Visit`.
Today the World is mouse-only.

**Exit criteria.** No UI file over 400 LOC. Vitest suite covering every extracted component. No
`fetch`-shaped domain decision inside a component.

---

## Phase 4 — Close the security gaps *(Security 76 → 96)*

Three items, all small, all real.

### 4.1 Content Security Policy

`tauri.conf.json` has `"csp": null` in a webview that renders `data:` URIs from imported artwork
and text produced by a remote model, in a process holding filesystem capabilities. Set a strict
policy now, while the surface is small enough to know what it needs.

### 4.2 Rotate the door token

`Token::remembered(vault)` persists a bearer token that outlives the door it describes. The
per-run `--mcp-config` path (2026-08-08) is the correct one. Keep the persisted token **only** for
the manual paste flow, mark it as such in the UI, and rotate per session otherwise.

### 4.3 One lock helper

87 `expect()` calls, 35 in `state.rs`, each a panic reachable from IPC. `World::lock()` already
does the right thing — `unwrap_or_else(|e| e.into_inner())` — and the other 35 do not. One private
`guard()` used everywhere.

**Also:** `cargo deny` in CI; a property test for `ProjectRoot` containment with `..`, symlinks
and UNC paths.

**Exit criteria.** CSP on. No `expect` on a lock. `cargo deny` green. Containment property test
passing.

---

## Phase 5 — Earn the scalability claims *(Performance 64 → 92, Scalability 55 → 90)*

Nothing here is broken today. Every item is a claim we would otherwise be making without evidence.

| Fix | Today | Declared limit to assert |
|---|---|---|
| **Per-Quest persistence** | `QuestLog` rewrites every Quest of a World on every turn | 1,000 Quests × 500 entries: open < 200 ms, save < 20 ms |
| **Filesystem watcher** | `read_dir` + `metadata` per character, **2 Hz, forever**, holding the World lock | 0 syscalls while nothing changes |
| **Probe cache** | `probe()` spawns **2 processes**, from **4 call sites**, uncached | ≤ 2 processes per explicit "probe again" |
| **`resolve()` once** | PATH scan inside `Default::default()` | 1 scan per process (`OnceLock`) |
| **Bounded `Reading`** | `pending` and `text` grow without limit | 10,000 tool calls in one turn stays under a stated ceiling |
| **UI virtualisation** | Every Chronicle entry rendered | 5,000-entry Chronicle scrolls at 60 fps |

**Exit criteria.** Each row has a test or benchmark asserting its limit, running in CI.

---

## Phase 6 — Become a product other people can hold *(Product 58 → 90, Readiness across the board)*

Ordered by what unblocks the next thing.

1. **Packaging.** A signed installer, one command to produce it, reproducible in CI.
2. **Update path.** Even manual: "a new version exists" is a feature.
3. **Crash surface.** A panic must produce something the user can send. Today it poisons a lock
   and the UI guesses.
4. **First run.** With nothing configured: the World renders, the Launcher explains what to
   connect, no dead ends. (Build From Life rule 1 already demands the first half.)
5. **Contribution docs.** `CONTRIBUTING.md` with the module map — "where does this go?" answered in
   a document instead of by gravity toward `state.rs`.
6. **LICENSE** — before any of this is shared, not after.

---

## The order, and why it is this order

```
Phase 0  ─ safety ──────────────► nothing is irreversible
   │
Phase 1  ─ tests + CI ──────────► nothing regresses silently
   │                              (blocks 2 and 3: both are refactors)
   ├─────────────┐
Phase 2          Phase 4        ─► architecture restored · gaps closed
 engine          security         (independent — can run in parallel)
   │             │
Phase 3  ─ UI ───┘              ─► presentation layer is one again
   │
Phase 5  ─ limits ──────────────► claims become evidence
   │
Phase 6  ─ product ─────────────► somebody else can hold it
```

Two dependencies are hard: **Phase 1 before Phases 2 and 3** (they are refactors of live code), and
**Phase 0 before everything** (they are refactors at all). Everything else can move.

---

## What this gets to, honestly

| Category | Now | After | Why not 100 |
|---|---:|---:|---|
| Architecture | 82 | 96 | ADR-0015 (Activity Stream) stays deliberately unbuilt; the last 4 points are earned by building it when a subsystem actually needs it, not before |
| Kernel | 90 | 98 | Asymptotic — a vendor's ladder will pull at `Reasoning` again |
| Engine | 79 | 94 | Knowledge Engine (ADR-0010) unimplemented by design |
| Runtime | 68 | 92 | Lifecycle branching (ADR-0025) is still a sequence, correctly |
| UI | 61 | 92 | Some CSS mass is irreducible until a design-token pass |
| Experience | 74 | 95 | — |
| Performance | 64 | 92 | Real profiling needs real Worlds |
| Code Quality | 70 | 96 | — |
| Maintainability | 62 | 90 | Judged by a contributor who is not us; unprovable alone |
| Testability | 55 | 90 | — |
| Product | 58 | 85 | The rest is not an engineering score |
| Scalability | 55 | 90 | Only claims we assert |
| Security | 76 | 96 | No external audit |
| Documentation | 86 | 97 | — |
| Debt | 60 | 92 | Debt is a rate, not a state |

**Weighted: 71 → 93.**

The remaining seven points are not a backlog. They are the difference between a codebase that is
correct and one that has been *used* by people who did not write it — and no plan can schedule
that. The way to collect them is to ship Phase 6, hand it to somebody, and write down what they
could not find.
