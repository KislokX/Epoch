# Execution Plan — item by item

> **Companion to:** [Road to 100](Road%20to%20100.md). That document says *what* and *why*.
> This one is the checklist: every item states what changes **in code**, what changes **for the
> user**, and how you know it worked.
>
> Rule that governs all of it: **nothing is refactored until a test exists that would notice.**

---

## Phase 1 — Regression becomes impossible

*Additive. Nothing existing changes behaviour. Can run alongside feature work.*

### 1.1 Vitest harness
- **Code:** `ui/package.json` gains `vitest` + `@testing-library/react`; `ui/vitest.config.ts`;
  first three tests — Missions pagination, `useTurn` clearing `knew` on character change, a brain
  with an empty scale rendering no dial.
- **User:** nothing. No runtime code is touched.
- **Done when:** `npm test` runs and the three pass.

### 1.2 IPC contract tests
- **Code:** a Rust test per view type writes `serde_json` to `ui/src/ipc/__fixtures__/*.json`; a
  Vitest test asserts the TS type accepts each fixture.
- **User:** nothing today. Later: a whole class of "the panel shows `undefined`" never ships.
- **Done when:** renaming a field in `WorldView` fails `npm test` rather than surfacing in the app.

### 1.3 One formatting commit
- **Code:** `cargo fmt` (601 hunks) and `clippy --fix`, **alone in their own commit**.
- **User:** nothing.
- **Done when:** `cargo fmt --check` and `clippy -D warnings` are both clean.

### 1.4 MSRV honesty
- **Code:** `rust-version = "1.82"` in `Cargo.toml` (the code already uses `is_none_or`).
- **User:** nothing.
- **Done when:** clippy reports no MSRV warnings.

### 1.5 CI
- **Code:** one workflow: `fmt --check` · `clippy -D warnings` · `cargo test` · `tsc --noEmit` ·
  `npm test` · `cargo deny check`.
- **User:** nothing directly; every later change is safer.
- **Done when:** green on a clean checkout.

---

## Phase 2 — The architecture is restored

*Live code. Requires 1.1 and 1.2 first.*

### 2.1 `Witness` trait
- **Code:** `epoch-engine/src/turn/witness.rs` — `asked`, `settled`, `step`, `token`, `window`.
  `epoch-tauri` implements it by emitting Tauri events; tests implement it with a `Vec`.
- **User:** nothing.
- **Done when:** the engine compiles with no `AppHandle` anywhere in a turn path.

### 2.2 Scripted-agent test *(written before 2.3, against today's code)*
- **Code:** a fake `Agent` that replays a captured stream; asserts Quest inauguration, evidence,
  approval, refusal, and session handle.
- **User:** nothing.
- **Done when:** it passes against the **current** implementation. That is what makes 2.3 a
  refactor instead of a rewrite.

### 2.3 Move the agent turn into the engine
- **Code:** `prepare_agent_turn` + `run_agent_turn` (≈210 LOC) leave `epoch-tauri/src/state.rs`
  and become `epoch-engine/src/turn/agent.rs`. `state.rs` keeps: hold the `Arc`, translate IPC
  types, forward events.
- **User:** nothing visible. Same turn, same approvals, same Terminal.
- **Done when:** `state.rs` < 900 LOC (from 2,917) and the 2.2 test still passes.

### 2.4 Sessions on the Quest
- **Code:** `Quest.sessions: BTreeMap<CharacterId, SessionHandle>`, persisted (ADR-0014). The
  shell's `threads` and `agent_windows` maps are deleted.
- **User:** **visible improvement** — an agent conversation resumes after closing and reopening
  Epoch. Today it silently starts fresh.
- **Done when:** restart, reopen a Quest, and the agent continues its session.

### 2.5 One `QuestSummary`
- **Code:** `QuestView` and `Conversation` collapse into one type + an optional chronicle.
- **User:** nothing.
- **Done when:** `said` means one thing everywhere.

---

## Phase 3 — The UI is a presentation layer again

*Requires Phase 1. Each item independently shippable.*

### 3.1 `<Overlay>`
- **Code:** one component owning `pointer-events: auto`, scrim, `Escape`, focus trap, focus
  restore. Missions and Dialogue adopt it.
- **User:** `Escape` closes windows; focus returns where it was; the next overlay cannot repeat the
  unclickable-Missions bug.
- **Done when:** no overlay declares `pointer-events` itself.

### 3.2 Domain rules out of components
- **Code:** `AGENT_MODELS` (CharacterPanel.tsx:280) moves to the engine beside
  `deliberation::scale`. The Manual sentence (Dialogue.tsx:979) comes from `Autonomy::describe`,
  which already takes the brain into account.
- **User:** nothing today. Adding Codex, or a new Anthropic model, stops requiring a `.tsx` edit.
- **Done when:** no component contains a list of model names or a mode's meaning.

### 3.3 Split the megafiles
- **Code:** extract `<ReasoningDial>`, `<ContextGauge>`, `<ApprovalPrompt>`, `<ModePicker>`,
  `<ChronicleList>`, `<Composer>` from `Dialogue.tsx` (1,222); the same for `CharacterPanel.tsx`
  (1,212), `WorldScreen.tsx` (1,022), `LauncherScreen.tsx` (1,022).
- **User:** nothing, if done right. This is the item with the most visual-detail risk.
- **Done when:** no UI file over 400 LOC and each extracted piece has a test.

### 3.4 `useTurn` → reducer
- **Code:** 661 LOC / 20+ `useState` become one `TurnState` + reducer.
- **User:** the class of bug that produced two defects this session stops being possible.
- **Done when:** no `useState` in `useTurn` outside the reducer.

### 3.5 Accessibility bar
- **Code:** focus trap and `Escape` (from 3.1), plus a keyboard path to `Visit`.
- **User:** the World becomes navigable without a mouse.
- **Done when:** every action is reachable by keyboard.

---

## Phase 4 — The security gaps close

*Independent of 2 and 3. Can run in parallel.*

### 4.1 CSP
- **Code:** a strict `csp` in `tauri.conf.json`, replacing `null`.
- **User:** invisible unless something was loading from outside — in which case it appears at once
  and the policy is adjusted.
- **Done when:** the World and the Launcher render with the policy on.

### 4.2 The door token — revised 2026-08-08

> **The first version of this item was wrong, and the objection that corrected it was right:**
> rotating per session would have made the user re-paste a configuration on every restart. That is
> real friction bought with very little security — the threat is a *leaked* token, and a token
> that leaks is dangerous the moment it leaks, not the next morning. Trading a daily chore against
> a risk that rotation barely touches is exactly the bargain a product whose first principle is
> *"never harder than a video game"* should refuse.
>
> Looking again also found the larger hole, which rotation would have hidden rather than closed.

**What is actually wrong today**

1. `Token::remembered` writes 64 hex characters as **plaintext** to `vault/agent-token`.
2. `adopt()` writes the same token as **plaintext into the user's own project**, at
   `<project>/.mcp.json`, under `headers.Authorization`. Verified in a real project. That is the
   file most likely to be committed, and `CLAUDE.md`'s own rule — *"never written into the
   project: a token in a repo outlives the door it describes"* — is broken by our own code.

The second is worse than the first and rotation addresses neither.

**Do**

- **4.2a — The token lives in the secret store.** `Secrets` (DPAPI, `secrets.rs`) already exists
  and already holds provider keys. The token moves there. Same usability, no plaintext, and a
  vault copied to another machine or restored under another account decrypts to nothing, because
  DPAPI is scoped to the user.
- **4.2b — Rotation on demand, never on a timer.** A **Regenerate** button beside the door. Stable
  by default, so nothing to re-paste; rotated the moment the user thinks it leaked, which is when
  rotation is actually worth something. Regenerating states plainly that existing `.mcp.json`
  files stop working.
- **4.2c — `adopt()` stops being the common path.** Since 2026-08-08 Epoch hands a per-run
  `--mcp-config` to every agent it spawns, so the file is needed **only** by an agent the user
  starts themselves. The panel should say so, and offer adoption as the exception rather than the
  setup step.
- **4.2d — When it is used, the file is protected.** `adopt()` adds `.mcp.json` to the project's
  `.gitignore` (creating it if absent) and reports that it did. The token has to be in the file —
  a third-party agent can read nothing else — so the honest mitigation is making the file harder
  to commit, and saying it out loud.

**User:** nothing to re-paste, ever. One new button that is *theirs* to press. And a token that
stops being copied into their repository behind their back.

**Done when:** `vault/agent-token` no longer exists as plaintext · a fresh `adopt()` leaves
`.mcp.json` ignored by git · Regenerate invalidates old configurations and says so.

### 4.3 One `guard()`
- **Code:** the 35 `.expect("… lock")` in `state.rs` become one helper that recovers a poisoned
  lock, matching what `World::lock()` already does.
- **User:** a panic in one command stops cascading into a dead window.
- **Done when:** no `expect` on a `Mutex` remains.

### 4.4 Supply chain + path property test
- **Code:** `cargo deny` in CI; a property test for `ProjectRoot` containment with `..`, symlinks
  and UNC paths.
- **User:** nothing.
- **Done when:** both run in CI.

---

## Phase 5 — The scalability claims are earned

*Each item names the limit it asserts.*

| # | Code | User | Limit asserted |
|---|---|---|---|
| 5.1 | `QuestLog` → one file per Quest + an index | Missions opens instantly with thousands of conversations | 1,000 × 500: open < 200 ms, save < 20 ms |
| 5.2 | Filesystem watcher replaces the 2 Hz `read_dir` + `metadata` sweep | Less background CPU; edits still appear live | 0 syscalls while nothing changes |
| 5.3 | Probe cache in `World` with a TTL; "probe again" bypasses it | Panels stop feeling slow; up to 8 process spawns per screen becomes 2 | ≤ 2 processes per explicit probe |
| 5.4 | `resolve()` behind `OnceLock` | Nothing | 1 PATH scan per process |
| 5.5 | Cap `Reading.pending` and `Done.text` | A runaway agent cannot exhaust memory | 10,000 tool calls stays under a stated ceiling |
| 5.6 | Virtualise the Chronicle list | Long conversations scroll smoothly | 5,000 entries at 60 fps |

---

## Phase 6 — Somebody else can hold it

### 6.1 Packaging
- **Code:** one command produces a signed installer, reproducibly, in CI.
- **User:** Epoch installs like an application instead of running from a dev server.

### 6.2 Update path
- **Code:** a version check and a download link. Manual is enough to start.
- **User:** learns a new version exists without being told.

### 6.3 Crash surface
- **Code:** a panic hook that writes a report the user can send; the UI says what happened.
- **User:** a crash produces something actionable instead of a frozen window.

### 6.4 First run
- **Code:** with nothing configured — no models, no project, no packs — the World still renders and
  the Launcher says what to connect first.
- **User:** the first five minutes have no dead ends.

### 6.5 CONTRIBUTING + module map
- **Code:** a document answering "where does this go?" per subsystem.
- **User:** none. A contributor stops adding to `state.rs` by gravity.

### 6.6 LICENSE
- **Code:** chosen and added **before** anything is shared.
- **User:** knows what they are allowed to do.

---

## Order

```
1 ──► 2 ──► 3        1 blocks 2 and 3 (both refactor live code)
 └──► 4              4 is independent — parallel is fine
      5              after 2 (persistence lives in the engine by then)
      6              last: it packages whatever the others produced
```

**Feature work continues through 1, 4 and 5.** Only 2 and 3 want a window with nothing
half-finished.
