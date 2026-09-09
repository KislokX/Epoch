# CODEX.md — continuing Epoch

> **You are taking over an in-progress build, not starting one.** The architecture is frozen, the
> discipline is unusual and deliberate, and most mistakes available to you are mistakes somebody
> already made and wrote down. This file exists so you do not have to rediscover any of them.
>
> Read this once, then read the documents in §1 before you touch code.

---

## 1. Read these, in this order

The project treats documentation as part of the product. Every one of these is short and every
one of them will change what you build.

| # | Document | What it answers |
|---|---|---|
| 1 | `CLAUDE.md` | **The constitution.** Mission, principles, and every standing rule adopted since. Longest, most important. |
| 2 | `PRODUCT_ARCHITECTURE.md` | The map: Kernel vs Engine vs Experience Surfaces, and the vocabulary. |
| 3 | `ROADMAP.md` | What is done and what is next. **Implementation status lives here and nowhere else.** |
| 4 | `docs/ADR/` (0001–0028) | How each subsystem works, and why alternatives were rejected. Read `_index.md`, then the ones your task touches. |
| 5 | `EXPERIENCE_CONSTITUTION.md` · `LIVING_WORLD_DESIGN_GUIDE.md` · `CHARACTER_BIBLE.md` · `CONTENT_PHILOSOPHY.md` | How it feels, how the World behaves, who lives in it, what may ship. Permanent — **not** ADRs, never amended by implementation. |
| 6 | `docs/Build/Build From Life.md` | The implementation philosophy governing every milestone. |
| 7 | `docs/Architecture/Full Audit 2026-08-08.md` | The known weaknesses, scored. Phases 4–7 are its remediation. |
| 8 | `docs/Build/Missing Art.md` | What the build is waiting on from a human, and the fallback shipping meanwhile. |

**The ADRs most likely to matter to you next:** 0009 (Trust), 0010 (Knowledge), 0012 (Composer),
0015 (Activity Stream — designed, unbuilt), 0016 (assets + the `ui.*` namespace), 0018 (World
Simulation), 0025 (Quests own the work), 0026 (portable characters), 0027 (brains).

---

## 2. The mentality. Do not change it.

These are not style preferences. Each one was paid for by a defect.

### The World never lies

Nothing on screen may state something the Engine does not know. No invented readings, no
placeholder numbers, no progress bars for work that is not happening.

- A panel with nothing behind it keeps its frame, reads `0` / `OFFLINE` / `—`, and **names the
  subsystem that will light it up**. A gauge nobody can explain is worse than no gauge.
- An answer you cannot parse is **unasked** (`None`), never a default. Claude Code's `auth status`
  taught this: unparseable had to mean *unknown*, not *signed out*.
- A character never reports a colleague's work. This actually happened; §7 of ADR-0025 exists
  because of it.

### Measured, never remembered

**Ask the program.** Do not write behaviour from memory of a CLI, an API or a file format.

Real examples from this build, all of which would have been wrong from memory:
- `--permission-prompt-tool` is undocumented and absent from `--help`. Found by asking the binary.
- `codex exec resume` takes **every option before the subcommand**; after it, exit code 2.
- A refused Codex patch says `blocked by read-only sandbox` on *stderr*; a refused shell command
  says nothing about a sandbox at all — Windows says `Access … is denied`.
- `cargo deny`'s ignore list: all five RUSTSEC ids recalled from memory were wrong. The tool
  prints them.

If you cannot measure it, say so in the comment. Do not guess quietly.

### Earn Complexity

No abstraction for imagined future use. Every abstraction states why it exists, its horizon, and
what it enables. A registry of widgets with nothing in it is the canonical violation.

### Assets are authored, never generated

When implementation would benefit from artwork, **implementation pauses and names what is
missing** in `docs/Build/Missing Art.md`. You do not generate art, and you do not build a system
to hold art that does not exist.

### Comments explain *why*, and name the defect

This codebase's comments are unusually long and that is deliberate. A comment says what went
wrong, what was measured, and what was rejected. Match it. A comment that credits the code with a
capability it does not have is treated as a defect — one was fixed for exactly that.

### Immersion leaks are a third category

Alongside bugs and features. Something that works correctly and still reminds the user they are
in software: a text selection across a building, a browser context menu, a spinner where the
World could have shown the work. **Fix one before adding the next feature.**

### The Engine owns reality; the UI owns animation

The UI interpolates between states the Engine committed to. It never invents a position, a
progress or an arrival.

### Refuse the impossible rather than handling it

A window smaller than its own two corners does not get smaller — no overlap logic. A walk that
cannot fit does not begin. This is the preferred shape of a fix here.

---

## 3. How work is done

### Verification — run all of it before every commit

```bash
export PATH="$USERPROFILE/.cargo/bin:$PATH"     # Rust is not on PATH by default here
cargo fmt && cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd ui && npx tsc --noEmit && npx vitest run && npm run build
```

Green baseline as of this handoff: **611 Rust tests, 52 UI tests, clippy 0 warnings.**

### Tests are evidence, not coverage

- A test is named after the defect it prevents: `a_walk_in_progress_is_never_cancelled`, not
  `test_walk`.
- **Prove a test fails without its fix.** One test here passed with and without the fix until it
  asserted the right thing. Mutate the fix out, run it, confirm red, restore.
- Comments in tests carry the captured evidence — real CLI output, real event JSON.

### Editing files on Windows

The tree is **CRLF**. Patches are applied by writing a Python script that reads with
`newline=''`, normalises to `\n`, edits, then writes back as `\r\n`. Assert the match count
before replacing:

```python
def sub(a, b, n=1):
    assert s.count(a) == n, (a[:70], s.count(a))
```

A silent zero-match replace is how a patch reports success and changes nothing.

### Commits

Present tense, lower-case type prefix, and a body that explains **why** and what was rejected —
matching the surrounding history. End with:

```
Co-Authored-By: <your name> <your email>
```

Commit only when asked, or when a coherent slice is complete and verified. `git` is local and
private; the GitHub remote (`https://github.com/KislokX/Epoch`, private) is **not pushed yet** —
the vault must leave the tree first.

### Documentation is part of the change

- Behaviour change that contradicts or extends an ADR → **amend the ADR** with what the build
  revealed. Amendments cite evidence, never speculation.
- Anything shipped or dropped → update `ROADMAP.md`. Implementation status belongs there and
  nowhere else.
- The five permanent constitutions are never amended by implementation.

---

## 4. Where the build is

**Done:** Phases 1, 2 and 3, plus two items of Phase 4.

- **Phase 1** — version control, Vitest, IPC contract tests, CI, `paths.rs`, Create Character.
- **Phase 2** — the agent turn moved into the Engine (`turn/agent.rs`), sessions on the Quest,
  one `QuestSummary`, Codex as the second agent, turn-level approval.
- **Phase 3** — the Simulation has a clock. Travel with speed/ETA, the causality rule enforced in
  code, routines authored per World, coarse publication on its own channel (`world:moved`),
  and the World renders it. **Verified working across a local model and two hosted agents.**
- **Unplanned, shipped:** the crew works together. A character may *ask* to hand work on
  (`mcp__epoch__hand_over`) and only the user hands it on; what travels is the Quest; the road
  carries it, so somebody walks across the map to pick it up.
- **Phase 4 so far:** `<Overlay>`, and the interface became authorable — `ui.*` in ADR-0016, with
  the first real window frame shipped in the default pack.

**Verify this list against `ROADMAP.md`** — it is the source of truth and this paragraph is a
summary.

---

## 5. What to do next

### Phase 4 — the UI is a presentation layer again *(in progress)*

Remaining, in the order I would take them:

1. **Split the megafiles.** Target: no file over 400 LOC.
   - `ui/src/components/hud/Dialogue.tsx` — 1,248
   - `ui/src/screens/WorldScreen.tsx` — 1,096
   - `crates/epoch-tauri/src/state.rs` — 3,423 (the Engine's, not the UI's, but the same problem)
   - Extract in the shape `HandoverOffer` already demonstrates: everything arrives as a prop,
     the component owns no turn state, and it ships **with tests** — the extraction is what makes
     testing possible, so a split with no tests has done half the job.
   - Candidates named in the audit: `<ContextGauge>`, `<ApprovalPrompt>`, `<ModePicker>`,
     `<ChronicleList>`, `<Composer>`, `<UndoBar>`, `<StandingList>`.
2. **`useTurn` → reducer.** 652 LOC, 20+ `useState`. One explicit state, one transition function.
   Write the reducer's tests first; it is the piece most able to break the product silently.
3. **Domain rules out of JSX.** The model list and the meaning of an autonomy mode come from the
   Engine, which already knows both. Any `if (mode === "auto")` in a component is a rule in the
   wrong layer.
4. **Life:** chat history with `↑` per conversation · `/compact` (ask the Composer to summarise a
   long Chronicle **and show what it dropped**) · attachments as context rather than paste ·
   full keyboard reach including `Visit`.

### Phase 5 — what the crew knows

1. **Capabilities from the live registry.** `CapabilityRequest::OFFERED` is a hardcoded Kernel
   constant, so **an MCP tool cannot be granted to a character today.** This is a correctness gap,
   not a feature. Start here.
2. **Knowledge Engine, minimally** (ADR-0010) — a source a character can read, with provenance.
   Not embeddings, not a graph.
3. **Composer measurement** — three probes (obedience · restraint · greeting) run *before*
   changing anything. The tools block has broken twice and is protected by a test; see the long
   comment in `crates/epoch-engine/src/context.rs`, which records the method and the numbers.
4. **Life:** Skills · the Obsidian vault as a readable source (Project > Model > Internet, per
   ADR-0025) · MCP servers as a first-class connection granted per character · a cookbook in the
   product.

### Phase 6 — safe and fast

1. **CSP** — `csp: null` becomes a strict policy.
2. **The door token** — into DPAPI, stable by default, a **Regenerate** button, and `.mcp.json`
   added to the project's `.gitignore` with that reported. Today `adopt()` writes a bearer token
   in plaintext into the user's project.
3. **One `guard()`** — 35 `expect()` on locks are panics reachable from IPC.
4. **Per-Quest persistence** — 1,000 Quests × 500 entries, open < 200 ms.
5. Filesystem watcher (replaces a `read_dir` sweep twice a second) · probe cache (8 process spawns
   per screen → 2, on an explicit press) · virtualised Chronicle (5,000 entries at 60 fps).

Every claim in this phase gets a number and a test that holds it.

### Phase 7 — somebody else can install it

`installer/` producing `Epoch_Setup.exe`; optional components **offered, never imposed**; the
installer never signs in, never pulls a model, never creates a crew, a World or a Project Root.
Plus update path, crash surface, `CONTRIBUTING.md`, `LICENSE`. See
`docs/Milestones/First Run.md`.

---

## 6. Blocked, and on what

| What | Blocked on | Fallback shipping today |
|---|---|---|
| **3.3 actions, not animations** (`walk.east`, `idle`, `work` → frames) | **Authored walk sprite sheets.** Do not build the mapping first. | A walking character reuses their standing sprite with a gait and a lifting shadow. Honest, and one drawing. |
| **3.4 Character Pack import** | The same artwork | — |
| **`ui.button.*`, `ui.tab.*`, `ui.checkbox.*`** | Real assets. The namespace exists; the widgets are deliberately not built. | Epoch's own CSS controls |
| **Tool-level approval for Codex** | Its `app-server` protocol is experimental. Deferred by decision. | Turn-level approval, behind a transport-independent `Approver` trait. Only the filling changes. |
| **Push to GitHub** | The vault must leave the tree | Local git only |
| **Default pack licence** | A human confirming `packs/default/assets/ui/window.png` is CC0/original | Flagged in `docs/Build/Missing Art.md` |

---

## 7. Traps this build already fell into

- **Windows batch files reject arguments containing newlines.** Resolve `claude.cmd` to the real
  `claude.exe` before spawning. There is a test asserting a newline argument can be spawned.
- **`ProjectRoot` canonicalises**, so paths are `\\?\C:\…` (verbatim). No other program writes
  one; strip the prefix before comparing against any external output.
- **PowerShell wraps long paths mid-word** in error text. Compare with whitespace removed.
- **`pointer-events` is inherited.** `.figure` is `none`, so a declared hit target inside one
  needs `pointer-events: auto` or it is drawn, aimed at, and silent. Use `<Overlay>` for windows;
  never `.hud > *`.
- **A system message loses to the conversation.** Measured against `qwen3:14b`: an instruction
  only took effect when moved **last** and carried on the **user** channel. Placement beats
  wording. The numbers are in `context.rs`.
- **Publication is coarse.** `world:changed` carries every mark and face — never send it per tick.
  Movement rides `world:moved`; large assets get their own one-shot command.
- **Persistence is not the Activity Stream.** ADR-0015 is designed and unbuilt; repositories stay
  the source of truth.

---

## 8. Before you commit

- [ ] Every command in §3 run and green
- [ ] Every new test named after the defect it prevents, and **proven to fail without the fix**
- [ ] No invented reading, no placeholder number, no capability claimed that does not exist
- [ ] ADR amended if the build revealed something, citing what it revealed
- [ ] `ROADMAP.md` updated if anything shipped or was dropped
- [ ] Missing artwork named in `docs/Build/Missing Art.md` rather than worked around
- [ ] Commit body explains *why*, and what was rejected

---

## 9. The one-sentence version

> Build the thing that is true, prove it with a test named after what it prevents, write down why,
> and when you need something a person has to make — stop and name it.
