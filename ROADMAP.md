# Roadmap

> **What exists now, and what comes next.** The architecture describes what exists *conceptually*
> and stays stable; this file changes constantly. Implementation status never belongs in
> `PRODUCT_ARCHITECTURE.md`.
>
> Rewritten 2026-08-08, merging three plans that were running in parallel and contradicting each
> other about order: the feature roadmap, the [Full Audit](docs/Architecture/Full%20Audit%202026-08-08.md)
> remediation, and the [Install and First Launch](docs/Milestones/First%20Run.md) design. The
> previous roadmap is archived at [docs/History/ROADMAP 2026-08-08.md](docs/History/ROADMAP%202026-08-08.md)
> — it holds measurements worth keeping.

---

## Where this stands (2026-09-07)

**Every engineering phase through 15 is closed**, and so is **15¾**, which is the external
audit of 2026-09-07 and everything it found. What is open is not engineering: it is QA of the
real artefact — five items, listed at the end of Phase 15¾ in the owner's order, beginning
with *network loss during an inference* because it is the only one that expires.

Phase 16 is the release and is last by the owner's decision. Nothing has been published.

---

## The rule this roadmap is built on

**Every phase validates an architectural contract *and* makes the World visibly more alive.**
A phase that only does the first is not finished ([Build From Life](docs/Build/Build%20From%20Life.md)).

That is why the audit's remediation is not a separate "cleanup milestone" nobody would ever
schedule. Each phase below has two halves, and they ship together:

| | **Ground** — the contract | **Life** — what the user sees |
|---|---|---|

Rule 6 — *prioritise visible life over invisible infrastructure whenever architecture permits* —
was outranked once, deliberately, in the previous roadmap, so that behaviour would not be built
twice on an engine about to be reshaped. **That debt is now owed and this roadmap pays it in
Phase 3.**

---

## Phase 1 — Ground truth, and the first person *(done 2026-08-08)*

> Closed. [Retrospective](docs/Build/Retrospectives/Phase%201%20-%20Ground%20truth.md) — what the
> tests found, what the audit's remediation actually cost, and the one implementation that would
> have destroyed a developer's crew.

**Ground.** Nothing can be refactored safely and nothing can be installed until two things are
true: a change can be undone, and the app knows where it lives.

1. **Version control** — done (`b3c9e53`). Private, local, no remote.
2. **Vitest harness** — three tests first: Missions pagination, `useTurn` clearing the gauge on a
   character change, an empty scale rendering no dial. All three are defects that actually
   happened.
3. **IPC contract tests** — a Rust test writes each view as JSON; a TS test asserts the type
   accepts it. Renaming a field then fails in CI instead of showing `undefined` to a user.
4. **CI** — `fmt --check` · `clippy -D warnings` · `cargo test` · `tsc` · `npm test` ·
   `cargo deny`. Preceded by one commit that runs `fmt` and `clippy --fix` alone, so the 601
   formatting hunks never mix with a behaviour change. MSRV corrected to 1.82, which is what the
   code already requires.
5. **`paths.rs`** — three locations instead of one: *shipped* (read-only, beside the binary),
   *data* (`%APPDATA%\Epoch`), *project roots* (anywhere, chosen). Resolved once at startup, with
   a development fallback so nothing breaks before the installer exists.
6. **Lazy vault creation** — the data directory appears when it is needed, never as an empty tree
   pretending something is configured.

**Life.** *Create Character.*

7. **The crew starts at zero** and the panel says so. One action: **Add a character**.
8. **The skeleton of a person** — archetype (classification, ADR-0017), name, portrait, sprite,
   prompt, routine, brain. Sensible defaults for everything that has one; **nothing invented for
   the things that make somebody who they are.** An archetype may offer a *suggested prompt* to
   accept, edit or delete — words offered to an author, not a stranger placed in their World.
9. **First launch from empty** — no crew, no models, no project: the World still renders and every
   instrument reads what is true.

> **What the user sees:** they install nothing yet, but from an empty vault they can make their
> first character — and it is *theirs*, not one of ours.
>
> **Summary:** the repository becomes what a user installs, and change becomes reversible.

---

## Phase 2 — The turn belongs to the Engine

**Ground.** The one place where code contradicts an accepted ADR
([§2.1](docs/Architecture/Full%20Audit%202026-08-08.md)): the agent turn lives in
`epoch-tauri/src/state.rs`, so half the runtime cannot exist without a window.

1. **`Witness` trait** — `asked`, `settled`, `step`, `token`, `window`. The shell implements it
   with Tauri events; a test implements it with a `Vec`.
2. **Scripted-agent test, written first, against today's code.** It is what makes the next step a
   refactor rather than a rewrite.
3. **Move the agent turn** into `epoch-engine/src/turn/agent.rs`. `state.rs` goes from 2,917 LOC
   toward 900: hold the `Arc`, translate IPC types, forward events.
4. **Sessions belong to the Quest** — `Quest.sessions`, persisted (ADR-0014). The shell's two
   string-keyed maps disappear.
5. **One `QuestSummary`** — `QuestView` and `Conversation` stop disagreeing about what `said`
   means.

**Life.** Continuity, and a second agent as proof.

6. **Conversations resume after a restart.** Today an agent thread is lost silently when Epoch
   closes; after this, reopening a Quest continues the same session.
7. **Codex** — the second agent. Its cost is the evidence: if the refactor worked, hosting it is
   declaring an `Invocation` and a scale, not touching the turn.

> **What the user sees:** close Epoch mid-conversation, open it tomorrow, and the character
> remembers. And a second agent appears without anything else changing.
>
> **Summary:** the Engine owns the runtime again, and the CLI surface stops being impossible.

---

## Phase 3 — The World moves *(the debt Rule 6 is owed)*

> **1, 2, 5, 6 and 7 are done** (2026-08-09); **3 and 4 moved to Phase 6** (2026-08-15).
> The clock, travel, the causality rule, the authoring
> that declares a routine, and the visible difference between walking to work and walking on a
> routine. What the build revealed is recorded as an amendment to
> [ADR-0018](docs/ADR/0018-world-simulation.md) — including two teleports of our own making, and
> a per-World correction that repeats ADR-0028 one field over.

**Ground.** Presence has had `Idle` and `Working` since ADR-0018 and no clock. The contract exists
and has never been exercised.

1. ~~**Time in the Simulation**~~ — done. Routines, destinations, travel with a speed and an ETA,
   published coarsely and on **its own channel**: `world:changed` carries every mark and face, so
   a walk rides a `world:moved` that carries only who moved.
2. ~~**The causality rule enforced in code**~~ — done, and *enforced* is the word: there is no
   method that takes only a destination. Presence changes in exactly the three ways ADR-0018
   permits, and no fourth can be added by accident because a caller has nothing to call.
3. **Actions, not animations** — **moved to [Phase 6](#phase-6--the-crew-is-alive), 2026-08-15.**
   Left open here because it was thought to be waiting on artwork that only the Official Epoch
   Universe could supply. That was wrong in its premise: the artwork is the **user's**, imported
   the same way a sprite and an icon already are (ADR-0023, ADR-0024). The Official Universe is
   about the cast Epoch *ships*, never about whether somebody can animate the crew they invented.
   Phase 6 carries it with the surface that makes it usable.
4. **Character Pack import** — **moved to [Phase 6](#phase-6--the-crew-is-alive)** with the step
   above, for the same reason: a sheet nobody can import is a contract nobody can reach.

**Life.** The first wow, finally.

5. ~~**Characters walk.**~~ — done. Two causes, both real: a **handoff** walks the Quest from one
   building to the other (the sentence `places.rs` has carried since ADR-0028, finally executed),
   and an **authored routine** walks somebody to the Place a step happens at and home again. They
   never teleport — including when the reason for a journey ends mid-walk.
6. ~~**Work looks different from routine**~~ — done. The Engine decides which a walk is from what
   waits at the other end; the World draws a quicker gait and the same cold blue that marks every
   other kind of work.
7. ~~**The Terminal, the Chronicle and the World agree**~~ — done. The minimap stopped leaving
   travellers at the door they left, `Visit` matches identity rather than a Place's *kind*, and
   an agent's own patch tool reports what it changed — half of all edits used to leave no
   Terminal line and no evidence at all.

**Unplanned, and it belongs here:** the crew actually works together now. A character can ask to
hand work on and only the user hands it on; what travels is the Quest; the road carries it, so
you watch somebody cross the map to pick it up. Verified across `qwen3:14b` (local) and two
hosted agents. Recorded as an amendment to
[ADR-0025](docs/ADR/0025-quests-own-the-work.md).

> **What the user sees:** the World stops being a diorama. Hand a Quest to somebody in another
> building and they walk there to pick it up; give somebody a routine and they keep it. You can
> tell work from routine across the map without reading a word.
>
> **Summary:** the Living World stops being a promise in a document.

---

## Phase 4 — The UI is a presentation layer again

**Ground.** Four files over 1,000 LOC, zero tests before Phase 1, and domain rules living in JSX.

1. **`<Overlay>`** — one component owning `pointer-events`, scrim, `Escape`, focus trap, focus
   restore. The pointer-events rule stops being something to remember.
2. **Split the megafiles** — `<ReasoningDial>`, `<ContextGauge>`, `<ApprovalPrompt>`,
   `<ModePicker>`, `<ChronicleList>`, `<Composer>`. Target: no file over 400 LOC.
3. **`useTurn` → reducer** — 661 LOC and 20+ `useState` become one explicit state.
4. **Domain rules out** — the model list and the meaning of a mode come from the Engine, which
   already knows both.

1b. ~~**`<Overlay>`**~~ — done. One component owns pointer-events, the scrim, Escape and the
   focus, so a window cannot forget any of them. `HandoverOffer` left the 1,300-line conversation
   box and took five tests with it.
1c. ~~**The interface can be authored**~~ — done, as one vertical slice.
   [ADR-0016](docs/ADR/0016-asset-resolution.md) gains a `ui.*` namespace — same resolver, same
   manifest, same fallback chain, no theme layer — and `Frame` consumes `ui.frame.window`. A pack
   supplies one nine-slice PNG and every window in Epoch changes. Buttons, tabs and the rest are
   named and **not built**: the first authored window says what they need.
   Waiting on art: [docs/Build/Missing Art.md](docs/Build/Missing%20Art.md).
1d. ~~**The Chronicle can be tested on its own**~~ — done. `ChronicleList` left `Dialogue` with
   every value arriving as a prop; its tests hold the two distinctions a conversation view cannot
   lose: a colleague's words keep their author, and evidence/work never become speech or a result
   before the Engine reports one.
1e. ~~**A grant is not an approval**~~ — done. `ApprovalPrompts` receives the two questions as
   explicit values: giving someone a missing skill cannot silently allow the call that follows,
   and one-time, standing and refused permissions each keep their own Engine answer.
1f. ~~**The draft is not a Chronicle entry**~~ — done. `DialogueComposer` owns only the words
   being typed, their measured height and the Enter/Shift+Enter distinction; it hands the draft
   to `useTurn` unchanged and never becomes a second record of what was said.
1g. ~~**The mode is read back, never assumed**~~ — done. `ModePicker` renders the selected mode
   and every option from the Engine, returns only the selected Engine id, and labels the active
   brain's gate only when the Engine has supplied that exact reading.
1h. ~~**A cold context gauge is not an empty one**~~ — done. `ContextGauge` keeps no Engine
   reading separate from a known count with an unknown budget, exposes dropped context and reply
   reserve, and keeps its report anchored to the composer through its control slot.
1i. ~~**Manual says how Codex actually asks**~~ — done. Codex begins a turn read-only; when that
   sandbox blocks a change inside the Project Root, Epoch asks before retrying the same turn.
   This remains distinct from Claude Code's per-tool prompt and is now asserted in the Engine.
1j. ~~**Standing decisions can be taken back exactly**~~ — done. `StandingDecisions` accepts only
   Engine records, keeps a standing denial visibly distinct, and returns the entire record when
   the user takes it back so scope and character cannot be guessed by the UI.
1k. ~~**Undo belongs to the World, not a dialogue**~~ — done. `UndoNotice` names the global
   Journal change, keeps dismissing it distinct from undoing it, and returns only the exact
   summary the user put away; Engine persistence and refresh remain in `Dialogue`.
1l. ~~**Failure says what was measured**~~ — done. `TurnFailureNotice` keeps the raw failed-turn
   evidence visible, offers sign-in only for an agent the Engine measured as signed out, and shows
   recovery guidance only for its observed failure trigger. Probing and credential launch remain
   in `Dialogue`; the notice only renders the resulting facts and delegated actions.
1m. ~~**A new Quest only replaces the current one**~~ — done. `SetAsideButton` appears only when
   the Engine reports an active Quest and delegates exactly its existing set-aside action; creating
   the next Quest and recording what changed remain Engine work.
1n. ~~**A turn changes through named transitions**~~ — done. `useTurn` now uses one pure reducer
   for the temporary surface state of a turn. Its tests hold concurrent tool completion, measured
   context precedence, turn endings, and clearing the previous Quest's citations and instruments.
   The Engine remains the record; the reducer only describes what this surface is currently seeing.
1o. ~~**A sidebar reading is a control only when the World supplied one**~~ — done. Shared
   `SidebarCard` and `SidebarRow` keep dormant instruments visibly present, and render navigation
   as a button only when the parent supplies its real World action.
1p. ~~**Connected Models keeps distinct measurements distinct**~~ — done. `ConnectedModels`
   receives Provider readings from the Engine and keeps unreachable, reachable-without-models,
   and ready model states visibly separate rather than inventing one reassuring status.
1q. ~~**Models and autonomy are read from the Engine**~~ — verified. `ModePicker` renders only
   the Engine's `AutonomyView`, including its per-agent gate reading; character model choices and
   the World model plate render Provider probes. The only unmatched model kept by the editor is
   the existing Engine-backed assignment, so a temporary offline probe cannot erase it.
1r. ~~**An unset provider value stays unset everywhere**~~ — done. `BoundedControl` is the one
   tested implementation for both canonical model parameters and provider-declared numeric
   controls; a visible provider default is never turned into an invented zero.
1s. ~~**A character's two images are independently actionable**~~ — done. `CharacterArtSlot`
   carries the exact ownership boundary: a sprite or icon may be changed separately, and `Clear`
   appears only for artwork that belongs to that character rather than for a World fallback.

1t. ~~**The World chrome keeps layout and quota truth separate**~~ â€” done as a first honest
   slice. `TRAVEL` and `LAYOUT` now occupy the dock's separate edges; Layout only filters local
   drawing layers. Empty Workflows retain their full frame. Claude's two plan windows and Codex's
   plan window have reserved HUD instruments, but remain `—` until a documented machine-readable
   remaining-allowance source exists: per-turn context/token readings are not subscription usage.

**Amendment 2026-08-10.** Codex CLI 0.147.0 exposes a read-only rate-limit snapshot through its
local experimental app-server. The Codex plan instrument now reports its measured remaining
percentage from that snapshot and goes cold if the protocol cannot be read. Claude's interactive
`/status` remains a person-facing surface, not a machine contract. The CLI's later zero-turn
`/usage` result is the narrower source now used for its 5-hour and weekly bars. See
[Plan Usage](docs/Build/Plan%20Usage.md).

**Amendment 2026-08-10 (Claude Code 2.1.226).** Claude's zero-cost print-mode `/usage` result
now reports the current 5-hour and seven-day percentages. Epoch parses only those two bounded
readings and renders their remaining allowance; an unfamiliar result remains cold. See
[Plan Usage](docs/Build/Plan%20Usage.md).

**Amendment 2026-08-10 (road authoring).** A World owner's roads are now a named editable list
in World Editor: each record can be re-traced, saved with authored corners, or removed without
guessing which crossing line was meant. Pack routes stay read-only. See
[Road Authoring](docs/Build/Road%20Authoring.md).

**Amendment 2026-08-10 (a World arrives unshaped).** The default pack ships no roads. A pack
route is now `from`, `to` and `prominence` only: its `waypoints` were authored against positions
ADR-0028 gave to the user, so a user who painted their own land got roads running into open
ocean. Where the roads run is the first thing a World's owner decides. Two defects went with it —
a road the user traced was discarded in favour of the pack's between the same pair, and the
editor's unreachable warning counted only vault roads while the map drew both. See
[Road Authoring](docs/Build/Road%20Authoring.md).

**Life.** The small things that make it feel finished.

5. **Chat history** — ↑ recalls what you said, per conversation.
   **Status: done.** The Composer reads the current Quest's Chronicle, recalls newest first with
   Up/Down, preserves multiline draft arrows, and resets recall when the Quest changes.
6. **`/compact`** — ask the Composer to summarise a long Chronicle, showing what it dropped.
   **Status: physical delivery done.** `/` exposes only Engine-declared
   commands, capabilities and settings; an agent summary is read-only, persists a Chronicle
   digest, then rotates that agent's external session. The next agent turn starts fresh with the
   digest. The durable Quest memory carries structured coverage: it says how many earlier records
   now travel as that brief, preserves historical evidence and participants as data, and keeps a
   literal recent tail. Agents retire their external session after the brief; providers do not have an opaque
   session, so Epoch runs the same read-only maintenance turn and the next provider request is
   freshly composed from the resulting Chronicle marker. A provider only starts it when its
   current window still contains the whole covered prefix; otherwise Epoch refuses rather than
   call a partial brief complete. In both cases the brief never appears as character dialogue
   and tools are unavailable while it is made. Chunked provider reduction is a later capability.
6a. **Per-Quest persistence** — **done as the first scale boundary.** Each Quest now owns one
   durable JSON document; opening a World reads the most recently changed Quest rather than a
   monolithic `quests.json`, and a turn carries its exact Quest id until it is recorded. The alpha
   file migrates once with an auditable backup and interrupted replacement recovery. Missions
   still enumerates Quest files on demand: paging and header caching remain measured scale work,
   not a claim we have made early.
7. **Rich Chronicle** — Markdown seguro y común para listas, tablas, bloques de código y flujos;
   el registro conserva texto, la UI solo lo proyecta. **Status: entrega 1 done.** Direcciones
   `http(s)` se abren por el shell, tablas se revisan dentro del chat y el subconjunto Mermaid no
   ejecuta un intérprete de terceros. Excel real sigue siendo una Skill, no una tabla decorada.
8. **Attachments** — drop a text file into a conversation; it becomes attributed context, not a
   paste or filesystem permission. Status: delivery 1 done (small text references with durable
   provenance); rich documents and images wait for their own context path.
9. **Keyboard** — every action reachable without a mouse, including `Visit`.
   **Status: Visit, Talk and editor refusal dismissal done.** The World dock uses native buttons
   and returns the exact authored Place; a focused person in the map accepts Enter and Space to
   begin a conversation; an editor refusal is a focusable button rather than clickable text.

> **What the user sees:** Escape closes things, ↑ recalls, files can be dropped in, and nothing
> feels like a prototype.
>
> **Summary:** one authority for what is true, and a UI that can be tested.

---

**Closed 2026-08-12, by the owner, in the running desktop application.** Verified: a response
started in one Quest never paints in another and is there when you come back; a shared Quest
settles when its author finishes even while you are visiting a colleague; Auto writes inside the
Project Root without asking and Manual stops for **MAGE NEEDS YOUR WORD** first, with **NO**
leaving no file and **ALLOW ONCE** leaving one plus attributed evidence; `/compact` keeps a Quest
to its own subject; and the cold start, the crew card's context, and Codex's credit balance all
read correctly.

Phase 4.5 (cleanliness and auditability) ran between the last slice and this closure and is
recorded in [CLAUDE_SUPERNEWPLAN.MD](CLAUDE_SUPERNEWPLAN.MD).

---

## Phase 5 — What the crew knows

**Ground.** Two subsystems the architecture declares and the code has never had.

1. ~~**Capabilities from the live registry**~~ — **done 2026-08-12.**
   `CapabilityRequest::OFFERED` was a hardcoded array of nine names in the Kernel, and it could
   not have worked: the Kernel has no I/O, so it cannot know what this build registers and
   certainly cannot know an MCP server is offering `gmail_send`. The tool joined the live
   registry, was judged by the same `decide()` as everything else, and could never be requested
   by anybody. What a surface offers is now `capabilities::catalogue()` plus what the bridge is
   currently offering; `wants()` returns `Option` and the Engine resolves undecided against the
   registry; the Kernel keeps only `INTENTIONS`, the requests nothing can do yet.
1. ~~**A server is one thing, not two dozen**~~ — **done 2026-08-12.**
   The step above made an outside tool requestable and immediately showed why one tool is the
   wrong unit: Playwright put twenty-four tick-boxes in the editor, and ticking them wrote that
   day's twenty-four ids into the file. Frozen — a tool the server added later was missing and
   nothing said so. A request may now name a *source*: `mcp:playwright`, resolved per turn
   against the live bridge. The Kernel validates the shape and never interprets the scope, the
   same arrangement a provider's tuning namespace already has. Membership is stated by the
   source rather than inferred from an id prefix, because a server free to name its tools
   anything would defeat a prefix rule. Requesting is still not permitting: every outside tool
   keeps every effect and no reversal (ADR-0008), and the editor shows what a box grants before
   it is ticked.
**Life.** Characters that know things — and a way to get them more.

4. ~~**MCP Workshop**~~ — **done 2026-08-12.** Browse and install a server without a terminal.
   Taken ahead of Skills deliberately: the two do not block each other, and when two orders are
   architecturally equivalent the one that makes the World alive sooner wins (Build From Life,
   rule 4). It is also what makes the step above worth having.

   Three things had to be measured rather than assumed, and two of them changed the code.
   `version=latest` is required or the registry answers with every published version, so a
   search for "filesystem" came back as one server five times. `runtimeHint` is routinely absent
   — the specification only asks for it alongside runtime arguments — so the runner comes from
   `registryType`, and an unrecognised registry is refused rather than guessed. And `uvx` is
   `uvx.exe`, not `uvx.cmd`: the symmetric "both run packages, both get `.cmd`" rule was written
   first and would have reported uvx missing on a machine that has it.

   A configured server could not carry environment at all, which is why most of the catalogue
   was uninstallable. It now holds plain values in `mcp.toml` and only the **names** of
   credentials, whose contents live in the encrypted store — so that file cannot contain a key,
   and a test reads it back to prove it. A server whose credential is missing is not started at
   all: started incomplete it fails in a third party's words, far from the screen where the
   credential is entered, and reads as Epoch being broken.

   What shipped, against what was planned below: the seed is not there yet and the catalogue
   opens from cache when offline. Installing renames on collision, never overwrites.

   **Measured before designing.** Of the four candidate catalogues, exactly one is machine
   readable: `registry.modelcontextprotocol.io` (`packages[]` carries `registryType`,
   `identifier`, `version`, `runtimeHint`, `transport` and `environmentVariables[]` with names
   and descriptions; `_meta` carries `status` and `isLatest`). PulseMCP's `v0beta` API answers
   **410 Gone**, `mcpservers.org` exposes only logo and preview endpoints, and
   `appcypher/awesome-mcp-servers` is a CC0 markdown README with no consistent install commands.
   So: one `CatalogueSource`, one implementation, and no scraping.

   **One click is not always true, and the surface must not pretend.** Remote-with-no-auth or a
   package whose runtime is present and which requires no environment variables installs
   directly. Required environment variables produce a **generated** form — the registry declares
   their names and descriptions, so a frontend that knew Notion needs `NOTION_TOKEN` would have
   to be edited to add the next server (ADR-0026's rule, applied to MCP). A missing runtime is
   said **before** the button, never as a failure afterwards; Epoch detects and explains, and
   never installs a runtime silently.

   **Security is the weight of this step.** The button shows the exact command and arguments
   before running anything; provenance and `status`/`isLatest` are surfaced; the installed
   version is pinned and never auto-updated; and environment secrets go to the DPAPI store, never
   to a file in the vault. Offline-first: the Workshop opens with what is installed, the
   catalogue is cached, and a cold catalogue names the date it was fetched rather than showing an
   empty grid. It ships with **~12 servers we verified ourselves** rather than a 200-entry seed
   parsed out of somebody's README — the seed's one gain is a screen, and its cost is a second
   data path plus staleness baked into the binary.
5. ~~**Skills**~~ **— done 2026-08-15.** A character can be given a way of working, not just a prompt. After 4 so a Skill
   declares itself against a World with many sources rather than two, and after 1b so it can say
   *requires Playwright* instead of listing two dozen ids that will be wrong next month.
6. ~~**The `/` menu**~~ **— done 2026-08-15.** A projection of Engine facts, read from the same
   `on_the_table` the turn composes from, so what it offers and what the turn allows cannot
   disagree. `/playwright` fell out of the group work as predicted. It also cost the camera the
   arrow keys: a `window` listener wins wherever the focus is, so no list could ever have used
   them. Panning is W/A/S/D now — **arrows belong to whatever is in front of you.**
7. ~~**The Obsidian vault as a source**~~ **— done 2026-08-15.** The second brain is readable by
   the crew, with the Project > Model > Internet priority ADR-0025 declares — *said* in the
   turn, because a model that is not told there is a place that knows answers from memory.

   **The Library is not a second Project Root.** Same confinement mechanism, different question:
   a project is worked in, a library is read. Kept separate so that pointing a World at your
   vault does not also hand `write_file` and `run_command` the same folder — a permission
   decision a convenience would otherwise make silently. Nothing writes to a library, and not
   by policy: no writing capability is ever constructed over one.

   `search_notes` and `read_note`, and the second is why these are note tools rather than the
   file tools aimed elsewhere: a vault addresses a note by *title*, so `[[Wikilink]]` resolves
   from anywhere — and two notes with one title are named rather than guessed, because a
   citation nobody can check is worse than a refusal.
8. ~~**An image reaches a Chronicle**~~ **— done 2026-08-15.** ADR-0024 reused whole: bytes
   across IPC, only the Engine writes, and the Engine names the file from the bytes — taken to
   its end, because a conversation has no slot to replace, so the *content* is the name. The
   same picture pasted twice is one file.

   The honest part is what it does **not** do. Sight is a capability and it does not exist yet,
   so the turn is told the image is here and that nobody can look at it — a model told only that
   a file was shared describes it from its name, confidently, with nothing admitting a guess.
   The composer says the same thing before a turn is spent.
9. **Models Workshop** — the same shell, another door, and where the vision model is obtained.
   Carries the cookbook: worked examples in the product, not in a README. "Will this fit on this
   machine" is measured or cold, never estimated.
10. **`see_image`** — *Engine half done 2026-08-16 (`fc90aa9`), not wired.* **Sight is a
    capability, not a property of the model.** A translator model
    turns pixels into text and the reasoner never touches pixels, so a blind local model gets
    sight through the same door it gets `read_file`, no provider coupling and far less VRAM.

    The design point that decides whether it works: **the reasoner writes the extraction prompt,
    because it is a tool call, and can ask again.** The published version of this technique
    hardcodes one generic extraction prompt — and then lists generic extraction prompts as its
    own biggest failure. As a capability rather than a preprocessing step, that failure mode is
    not something we avoid; it is not available.

    The description is a model's claim, not a fact, and enters the Chronicle attributed as one.
    Local and hosted backends are separate in Trust, because a hosted one sends the user's
    screenshot to a third party — the disclosure decision ADR-0025 already named for Project
    Roots. `keep_alive` is Provider configuration, never a character parameter. Out of scope:
    PDF (a native dependency), and any promise about charts. When it lands, `INTENTIONS` empties
    and the constant goes with it — which is the honest end of that list.

9a. **What a brain declares** — *added 2026-08-16, from measurement.* **The prerequisite that was
    missing**, and it was found by asking three programs a question we had been answering from
    memory: *can this brain see?*

    | asked | answered |
    |---|---|
    | Ollama `/api/show` | `capabilities`: `gemma4:26b` = completion·**vision**·tools·thinking · `gemma4:12b` = the same plus **audio** · `qwen3:14b` and `gpt-oss:20b` = no vision |
    | Codex app-server | `Model.inputModalities` — `text`/`image`/`audio`, default `["text","image"]` |
    | Claude Code | its first `system` event carries `tools` and `mcp_servers`, and Epoch **discards it today** |

    Every backend publishes what it can do, and Epoch reads none of it. ADR-0026 already says the
    Provider declares its surface and the Character holds the value — applied to `num_ctx` and to
    the sliders, never to what a model *can*.

    So: read the declarations, show them where a brain is chosen, and let them decide behaviour.
    Two things fall out immediately. **Whether a picture needs translating** stops being a guess.
    And a model that does not declare `tools` stops being handed tools — a defect nobody has hit
    yet because all four of these declare them, and one that would surface as "the character
    ignores its tools", which is among the hardest things to diagnose.

    Cold where nothing answers: a backend that will not say is *unasked*, never assumed.

9b. **A picture reaches a brain that can see** — *added 2026-08-16, measured end to end.* Step 8
    put an image in the Chronicle and told every character it could not see it. That was true
    when it was written and is now false for two of the three brains, so the sentence and the
    capability move together — removing it earlier would lie in the other direction.

    All three answered "Blue" to one flat `(16,96,220)` PNG:

    - **Claude Code** — no `--image` flag; `-p --input-format stream-json` and an API-shaped
      `image` block on stdin.
    - **Codex** — `turn/start` → `input: [{"type":"image","url":"data:image/png;base64,…"}]`.
      **The `data:` URI is the measured part.** A plain path is *accepted* by the app-server and
      fails minutes later, upstream, with somebody else's error message — the exact shape of a
      feature that looks wired and is not.
    - **Ollama** — `images: [base64]` on the message, for models that declare vision.

    Two roads, not one: whoever declares sight gets the picture whole, whoever does not gets it
    translated by step 10.

9c. **Files reach a brain** — *added 2026-08-16, decided by the owner.* An attachment lands in the
    vault the way an image does — Engine writes, Engine names it from the bytes — and the brain is
    told **where it is**. Its own tools read it, and they are better than ours: PDF, csv, zip,
    without Epoch understanding a single format.

    Not into the Project Root: Epoch writing into somebody's repository because they attached
    something is a side effect nobody asked for. To be measured before it is built: whether each
    agent reads a path *outside* its working folder, and under which permission.

11. **Knowledge Engine, minimally** (ADR-0010) — a source a character can read, with provenance.
    Not embeddings, not a graph: the smallest honest version.
12. ~~**Composer measurement**~~ — **done 2026-08-18.** Three probes — obedience · restraint ·
    greeting — driving the real composer, the real turn loop and a real model, with stand-in
    capabilities that record being reached for. Baseline on `gemma4:12b`: looked 3 of 3, went
    further 0 of 3, acted on a greeting 0 of 3. The table lives in ADR-0012 beside the hand-made
    one it makes repeatable. The first restraint probe passed while measuring nothing — its
    Chronicle ended with the character's own reply, so reaching for nothing was guaranteed;
    restraint happens *inside* a turn and is measured there now.

    **Both moved to the end of this phase by the owner, 2026-08-15**, and the reason is worth
    keeping: they are foundations with no visible face, and everything above them is a slice
    somebody can see. The Composer measurement in particular has to happen *before* the tools
    block is touched again — it has broken twice — so it gates the next phase rather than this
    one's visible work.

> **What the user sees:** characters stop being models with names. They can use the tools you gave
> them, read the notes you keep, and you can hand them new tools without opening a terminal.
>
> **Summary:** knowledge and capability stop being architecture and start being product.

---

## Phase 6 — The crew is alive *(done 2026-08-19)*

**Ground.** ADR-0016 named `character.architect` + `walk.east` as canonical concepts on the day it
was written. The Engine has never had the first half of that pair.

Presence already carries the load-bearing distinction — `ActivityClass { Idle, Work }`, enforced so
routine can never look like execution. What it does not carry is **which action**, in a closed
vocabulary a renderer can resolve. Today it publishes `activity: String`: *"walking to Library"*,
*"thinking with gpt-5.4"*. No renderer can map that to frames, and it is assembled by `format!()`
in a dozen places.

1. **`Action` in the Kernel** *(done 2026-08-19)* — a closed enum beside `ActivityClass`, **set at the three call
   sites ADR-0018 permits** and never parsed back out of the sentence. Deriving it from the string
   would be two authors of one truth, which is the defect this project spent a day removing in
   four other places. The human sentence stays beside it for the HUD; they answer different
   questions.

   Four to begin with, and the cut is the causality rule rather than taste:

   - `Idle` — routine, available. A real fact today.
   - `Walk` — travelling; the direction falls out of the leg being walked.
   - `Work` — something is genuinely running.
   - `Think` — **the turn has started and no tool has run yet.** Included from the start at the
     owner's insistence, and the argument is right: this is already a real state, it has an
     explicit cause exactly like the other three, and it is the difference between *nothing is
     happening*, *the character is reasoning*, and *execution has begun*. The moment the first
     tool call, file change or command begins, it becomes `Work` on its own.

   - `Talk` — **a handover completed: the receiver has arrived at the giver's Place and the
     giver is still there.** Added 2026-08-15; the owner is right that it is real, and the
     Engine already knows it. A handover walks the receiver to `carried_from` — the last
     contributor's Place — so two characters standing together *because the work passed between
     them* is a measured fact, not an inference.

     What the Engine does **not** know is that anything is said: no words pass between
     characters, and ADR-0023 forbids one writing another's lines. So this is the arrival, not a
     conversation, and two constraints keep it honest.

     **A beat, never a state.** A `Talk` that persisted would imply a conversation in progress
     that is not happening — the one thing the Living World Design Guide names outright. It
     lands, then becomes `Think`.

     **And only if the other is still there.** Checked on arrival rather than on departure: the
     giver may have walked somewhere else while the receiver crossed the map. With nobody to
     arrive to, it is `Settle` — which is the same beat without the second person, and the
     difference between them is the whole point.

   `Sleep` is deliberately **not** here. It has no cause the Engine can point at, and a sleeping
   character with no night to cause it is precisely the autonomous NPC behaviour ADR-0018
   forbids. It arrives when its cause does.
2. **A Mark that can hold frames** *(done 2026-08-19)* (extends ADR-0020/0021) — sheet, grid, frame count, duration,
   and direction for `walk`. The Engine still never names a file: it asks for `walk.east` and the
   pack resolves it.
3. **Per-action art slots on a character** *(done 2026-08-19)* — `set_character_art` becomes slot-addressed. Same
   pipeline as every other import (ADR-0024): bytes from the webview, and **the Engine names the
   file from its content**, never from the name that was uploaded.

**Life.** Your own crew, animated, with your own artwork.

4. **The animation editor** *(done 2026-08-19)* — under `sprite` and `icon`, an **ANIMATIONS** door: Idle, Walking,
   Working, Thinking, each importing a sheet, each with a **preview that plays**. The preview is
   not decoration: a mis-cut sheet is only ever discovered by watching it.
5. **The renderer plays the action** *(done 2026-08-19)* — the World stops laying a gait over a standing sprite and
   draws the frames the action names. With no sheet it falls back to today's behaviour, then to
   the visible stand-in: ADR-0016's chain, never blank, never a crash.
6. **Transitions, so nobody teleports between states** *(done 2026-08-19)* — two arrivals that today look identical:

   ```text
   routine:   Idle -> Walk -> Arrival -> Settle -> Think -> Work
   handover:  Idle -> Walk -> Arrival -> Talk   -> Think -> Work
   ```

   `Settle`, `Talk` and the `Think` beat are the point: a character arrives, takes in the place,
   thinks, *then* begins. Without them a figure snaps from walking to working, which reads as an
   animation switching rather than somebody deciding. And the branch between the two is not
   decoration — it is whether somebody else is standing there because the work came from them.
7. **The Quest moves the World** *(done 2026-08-19 — and it was nearly done already)*. A stage
   is **a session with one NPC** (owner, 2026-08-19): it begins when somebody takes the work and
   ends when they hand it on, so a handover is the boundary between two stages rather than
   something inside one. Nothing is authored in advance; a Quest's shape is the record of who
   worked on it.

   Which means *"the Quest changed stage"* **is** *"the work passed to somebody else"* — and the
   World has walked that since step 6. The Engine simply had no word for it. It now records
   `StageStarted` / `StageFinished`, and `Quest::stage` has exactly one writer so the cursor and
   the record cannot disagree. The X in Workflows ends a Quest as `Completed`, which means
   **ended** and never *succeeded* — that distinction was backwards in the Kernel and is fixed.

   A first amendment written earlier the same day called this blocked. Its three measurements
   were right and its premise was wrong: it assumed ADR-0025's authored Lifecycle. Both
   amendments are in the ADR, the second superseding the first, because the mistake is the
   useful part.

> **What the user sees:** the crew they drew, moving the way they drew them — and a Quest whose
> progress is legible across the map without reading a word.
>
> **Summary:** the animation stops being a property of the renderer and becomes a projection of
> what is true.

---

## Phase 7 — Safe and fast at the size we claim *(done 2026-08-19)*


**Ground.** Three security gaps and six unmeasured costs, none of which are broken today.

1. ~~**CSP**~~ **— done 2026-08-19.** `csp: null` became a policy **derived from what the code
   needs**, not from a template. Audited first: no `eval`, no `new Function`, no `innerHTML`, no
   `fetch`, no external resource the webview loads, and the built page carries one external
   script and zero inline ones — so `script-src 'self'` costs nothing in production.

   The two that would have broken it, found by looking rather than by running: **`img-src` must
   allow `data:`** (every mark, sprite, backdrop and skin is one) and **`style-src` must allow
   inline** (the Frame's skin and the sprite sheets build `url(...)` in style attributes). A
   textbook policy would have left the HUD with no textures and no sprites.

   `connect-src` carries `ipc:` and `http://ipc.localhost` — Tauri v2's IPC on Windows. `devCsp`
   is separate and looser, because Vite's HMR needs inline and a websocket that production does
   not.

   **Measured, as far as it can be from here:** the app runs for 150 s under the production
   policy with no panic and no World problem. What it cannot prove is that the window *looks*
   right — CSP violations go to the webview console. That is one line in `Repollo.md`.
2. ~~**The door token**~~ **— done 2026-08-19.** DPAPI was already there — `endpoint.rs` has
   migrated the pre-DPAPI `agent-token` file into it for a while, which the audit found rather
   than assumed. What was missing was the rest:

   **Regenerate**, in Settings, the user's to press. It closes every open door, which is the
   point: a token is regenerated because the old one may have escaped into a commit, a
   screenshot or a paste, and a regeneration that let the old one keep working would be a button
   that reassures without protecting. The old plain-text file goes with it.

   **`.mcp.json` into the project's `.gitignore`, and said out loud.** That file holds this
   session's door and its bearer; committed, it outlives the door and travels to everyone who
   clones. Only where there is already a `.gitignore` or a `.git` — creating one in a folder
   that is not a repository would be Epoch leaving litter to solve a problem the user does not
   have — and never a duplicate line. A line appearing in somebody's repository is Epoch editing
   it, so `Adopted` carries what it did and the surface says so.
3. **One `guard()`** — 35 `expect()` on locks stop being panics reachable from IPC.
4. **Quest-history measurement and paging** — **half proved** *(2026-08-19)*. Missions read
   every Quest file whole and threw the transcripts away. It now reads a **digest** of the same
   files: every fact a row shows, and none of the words
   (`crates/epoch-engine/src/digest.rs`, measured by `tests/the_cost_of_history.rs`).

   | quests × entries | on disk | whole | digest |
   |---|---|---|---|
   | 20 × 40 | 0.2 MB | 6.9 ms | **1.1 ms** |
   | 200 × 200 | 9.2 MB | 31.7 ms | **28.1 ms** |
   | 1000 × 500 | 114.7 MB | 281.6 ms | **221.9 ms** |

   *"Missions never needs every Chronicle body at once"* is now true. *"Under 200 ms at 1,000 ×
   500"* is not — it misses by a tenth, and the remaining cost is reading and lexing 114 MB,
   which no amount of not-allocating removes. Closing it means not opening the files, which
   means an index: a **second author of a truth the Quest already holds**, and the list is the
   one people would believe when the two disagree. Not spent on a size nobody has; at a World
   somebody actually has, this is 28 ms.

   The two readers cannot drift: a test derives one row both ways and compares it. It earned
   itself immediately — `said_count` turned out to mean *every record*, not the conversational
   ones, and the digest had been written to the name rather than to the meaning.
5. **Filesystem watcher** — **measured first, and it has not earned itself** *(2026-08-19)*.
   The plan was to replace a `read_dir` sweep running twice a second forever. A watcher is a
   dependency, a background thread and a class of platform-specific failure, so the number came
   first — `crates/epoch-engine/tests/the_cost_of_watching.rs`, and it stays as the instrument:

   | crew | per check | held per second |
   |---|---|---|
   | 1 | 38 µs | 0.08 ms |
   | 5 | 87 µs | 0.17 ms |
   | 50 | 687 µs | 1.4 ms |
   | 500 | 6.6 ms | 13.3 ms |

   Release and debug agree to within 10%, which says what it is: syscall-bound, and linear at
   about **13 µs per character per check**. What matters is not the syscalls but that
   `reload_if_changed` holds the **World lock** across them — so this is time every IPC command
   and every projection waits. At any crew a person actually has, that is under a fifth of a
   millisecond per second of wall clock.

   So the sweep stays. Revisit if a real vault ever gets large; the instrument is there and the
   slope is known.
6. **Probe cache** — **done 2026-08-19, and this one was real.** Measured first, like items 4
   and 5: asking which agents this machine has costs **537 ms** with Claude Code and Codex both
   installed and signed in (`tests/the_cost_of_asking.rs`) — two processes each, because
   *installed* and *signed in* are two questions with two different fixes.

   Four surfaces asked independently — startup, the character editor, the connections panel, a
   dialogue — so a pass through the Launcher spent over **two seconds** spawning processes to
   learn the same answer four times. The Engine keeps the reading now: 543 ms to measure, 1 µs
   to answer again.

   **It stays a measurement.** `fresh` re-runs the programs, and every REMEASURE and CHECK AGAIN
   press sends it — a kept reading with no way to repeat it would be a claim with a label on it.
   Signing somebody in throws it away, because that is the one action that makes it wrong.
   Nothing expires on a timer: a timer would be a guess about how often somebody installs a CLI.
7. **Virtualised Chronicle** — **not virtualised, and the measurement is why** *(2026-08-19)*.
   The plan was windowing. Windowing takes ownership of scroll position, breaks find-in-page,
   and makes *"follow the newest line"* — which this dialogue does on every token — something to
   implement rather than something the browser already does. So the number came first
   (`ui/src/components/hud/ChronicleList.cost.test.tsx`).

   | lines | nodes | build |
   |---|---|---|
   | 50 | 200 | 20 ms |
   | 500 | 2,000 | 90 ms |
   | 5,000 | 20,000 | 350 ms |

   Four nodes a line, and a browser scrolls 20,000 nodes. **The list was never expensive to
   have. It was expensive to keep**: with 5,000 lines standing, one streamed token cost
   **138 ms** — the settled Chronicle re-reconciling every node because `writing` changed. A
   local model produces about ten tokens a second, so the window fell a second behind for every
   second of talking and never caught up.

   Memoising the settled lines takes that to **0.7 ms**, roughly two hundredfold, and the
   dialogue keeps native scrolling and find-in-page. The one thing it needed was a caller that
   does not hand it a fresh callback every render — `onOpenLink` was an inline arrow, which is a
   changed prop per token all by itself.

   A test holds the number. If a Chronicle ever gets long enough that *having* the lines is the
   problem rather than keeping them, windowing is still available and the measurement is there
   to say so.

**Life.** Speed is a feature.

8. Missions opens instantly. Panels stop stuttering. Long conversations scroll.

> **What the user sees:** one new button, and everything faster.
>
> **Summary:** every scalability claim gets a number and a test that holds it.

**Why this now moved ahead of the World spanning machines.** Items 1 to 3 are the security half,
and the next phase puts Epoch on a local network. Shipping a network service on top of `csp: null`,
a door token that is not in DPAPI, and thirty-five lock `expect()`s reachable from IPC would be
building the reachable surface before closing the known holes. The order is not preference; it is
the only order in which the next phase is honest.

---

## Phase 8 — Any brain, from anywhere *(restructured 2026-08-16)*

**Ground.** Adding a brain should be configuration, not a pull request — and the phase that used
to sit here assumed the opposite.

**Why this moved in front of the distributed work.** A Mac mini running llama.cpp *is* a backend
speaking the OpenAI API at a remote address. Build that first and the next phase stops being
"construct distributed providers" and becomes "make a remote one fail gracefully" — from building
to hardening. Doing it the other way round would have meant writing the distributed machinery
against the one backend kind we already had, and discovering the abstraction afterwards.

1. ~~**A backend that speaks the OpenAI API**~~ **— done 2026-08-16 (`5f2aa48`).** — one `Kind`, and llama.cpp, LM Studio, vLLM,
   Deepseek, GLM and most of what ships next all work the day they ship. Endpoint plus optional
   key, no Rust. This is the highest return per line left anywhere in the plan.

   `Backend` already carries `id`, `kind`, `endpoint` and `enabled`, so the remote case needs no
   new shape — the field was always free.

2. ~~**Models Workshop**~~ **— done 2026-08-19.** Moved here from Phase 5, where it sat because
   it was "where the vision model is obtained". Sight turned out to be measurable (`Declared`),
   so it was never the blocker it was placed as.

   **Three things were measured before a line was written**, and two of them changed the design:

   Two sources, because one is nineteen entries: **Hugging Face** is the other, GGUF only —
   what `ollama pull hf.co/…` can actually take. Filters are Vision · Image · Audio · Tools ·
   Thinking, and **two of them are cold**, which is the honest part: Hugging Face has no field
   for tools or reasoning, Ollama declares those only for models already installed, and Ollama's
   featured list annotates nothing at all. A chip that returned an empty shelf would teach
   somebody that no model does it.

   And **`cloud` turned out not to be a facet of a model.** It is a property of the *Service* it
   runs on, which Epoch already measures (`ProviderStatus::local`). A model is not cloudy; the
   machine it runs on is somewhere.

   - `ollama.com/api/tags` **is not the library.** Nineteen entries, of mixed shape — some
     families (`glm-5.1`), some tags (`gpt-oss:20b`) — and it does not contain `gemma4:12b`,
     which this machine has installed. So it is presented as **featured**, and the useful half is
     a field that weighs *any* name, because the manifest endpoint takes any.
   - **The catalogue's size is not the model's size.** It reports `nemotron-3-super` at 230 GB
     and `deepseek-v4-pro` at 892 GB — a family's whole publication history. The manifest for one
     tag reports what a pull downloads: `gemma4:12b` is 7.56 GB, which matches what the local
     install says for the same thing.
   - **This machine answers.** RTX 4070 SUPER, 12.9 GB of video memory with 10.2 free, 33.9 GB of
     system memory. A machine with no NVIDIA card has **no VRAM reading** and says so — *unknown*
     is a different answer from *nothing fits*, and only one of them is measured.

   Fitting leaves a tenth as headroom, and that is not superstition: a model's weights are not
   its whole cost, because its context window is KV cache on the same card.

   The door in the Workshop had been **cold**, naming the subsystem it was waiting on. A test
   asserted that it was cold, and failed the moment it was lit — which is what that test was for.

3. ~~**Provider · Service · Model**~~ **— the cascade, done 2026-08-19.** — the editor collapses "where it runs" and
   "what it is" into one dropdown (`ollama/gemma4:12b`), so a character cannot be moved to
   another machine without re-choosing their model. Three fields, **in cascade**: the model list
   comes from the backend that was picked, so an impossible pair is not expressible rather than
   merely refused.

   | field | is | example |
   |---|---|---|
   | Provider | the machine | This PC |
   | **Service** | what runs it | Claude Code |
   | Model | the model | Opus 5 |

   **The middle field is a `Service`** *(settled by the owner, 2026-08-19)*. It had been written
   as `Brain`, and `Brain` already means something else in the Kernel: `Brain = Model | Agent`,
   which answers *who owns the loop* — Epoch, or the agent. One word for two things is the drift
   this project catches everywhere else.

   And the split falls out cleanly: **which Service you pick is what decides the kind.** Ollama
   is a Service that thinks; Claude Code is a Service that thinks *and works*. ADR-0027 already
   wanted both visible rather than inferred — *"Claude Code — as model"* and *"Claude Code — as
   agent"* as two entries — and under this they are simply two Services.

   **The middle word collides, and it has to be settled before this is built** *(noticed
   2026-08-19)*. `Brain` already means something in the code: ADR-0027 defines
   `Brain = Model | Agent`, which answers *who owns the loop* — Epoch, or the agent. The middle
   field here is a different question: *what program is listening*. llama.cpp, LM Studio and
   vLLM are servers, which in the code are what a **Provider** points at; Deepseek and GLM are
   model families, reachable through their own hosted endpoints or through a local server.
   Neither is a `Brain` in ADR-0027's sense.

   Whatever the field ends up called — Backend, Runtime, Engine — it must not be `Brain`, or the
   editor and the Kernel will be using one word for two things, which is the drift this project
   catches everywhere else.

   **Collapsed while there is only one backend.** Three controls where there was one is worse for
   somebody with a single Ollama, and the default is that nobody touches anything.

4. ~~**The setup reading**~~ **— done 2026-08-19.** `readiness.rs` had existed since 2026-08-16 with no screen — the Engine measured and nothing showed it. It reads **above Connections** rather than as a deck of its own: that panel already lists providers and agents with their notes, and two screens answering *what does this machine have* would drift the moment one sentence was fixed in only one of them. Rows that were
   *measured*: what runs, what is installed and signed out, what answered and with how many
   models. Then rows nothing can measure, saying what they would need.

   **Look first; teach only what looking did not answer** — the owner's correction, and the rule
   this file now states correctly. A key already in the environment is found rather than asked
   for, by name and never by value. The one search still refused is sweeping a network: not
   because it would guess, but because it probes machines that are not ours to probe.

5. ~~**Gemini**, measured~~ **— done 2026-08-19.** Driven and asked, against 0.54.4:
   `-p/--prompt` for headless, `--output-format stream-json` for NDJSON, `-m/--model`,
   `--approval-mode default|auto_edit|yolo|plan`, `--session-id` and `--resume` for threads.

   **Two absences that change what the surface may claim**, both probed rather than assumed:
   there is **no permission callback** (`--permission-prompt-tool` is rejected), so Epoch cannot
   be invited into its decisions the way Claude Code invites it — `Manual` maps to its own
   read-only `plan`, a promise Epoch can keep. And there is **no way to read sign-in status**
   (`--auth-status`, `--print-auth`, both rejected), so `signed_in` is `None` — *unasked* —
   never `false`.

   **No door.** MCP servers are configured globally by `gemini mcp add`, with no per-run
   `--mcp-config`; writing Epoch's session token into somebody's global config would outlive the
   door it describes. A Gemini character works with its own tools, and Epoch says so.

   No `Progress::Window` either: `result.stats` reports tokens used and no window size, and a
   budget guessed from a model name is the invented reading this project removes everywhere.

6. **Anything else with a headless mode** — measured 2026-08-19, and the answers differ.

   **Cursor and opencode are not on this machine**, so nothing is claimed about either.

   **Antigravity is**, and it is a Service Epoch cannot drive today — which is a different
   sentence from "not installed" and worth the distinction. Its install ships `Antigravity.exe`
   and `resources/bin/language_server.exe`, and **no CLI shim at all**: no `bin/` script, nothing
   on PATH. The language server's own `--help` is 60 flags of daemon configuration —
   `-api_server_url=http://0.0.0.0:50001`, `-csrf_token`, `-headless`, `-persistent_mode`,
   `-lsp_port` — and **not one of them takes a prompt.**

   So its entry point is a **local HTTP API with a CSRF token**, not a program you hand a task
   and read. Every agent Epoch hosts today is spawn-prompt-read-exit; this is speak-a-private-
   protocol-to-a-daemon. That is a different kind of commitment — reverse-engineering an
   undocumented API rather than reading a `--help` — and it needs a decision before it needs
   code.

   Two details worth keeping for whoever picks it up: it stores its state under `.gemini`, the
   same directory Gemini CLI uses, and `-subclient_type` names `sdk`, `cli` and `hub` as things
   that exist — so a supported client may ship later and be a far better door than this one.

   **And one did: `agy`.** Antigravity ships a real CLI, installed separately from the IDE, with
   `-p` and `--output-format stream-json` — nearly Gemini CLI's shape, so nearly Gemini CLI's
   cost. **Parked in `docs/Build/Repollo.md`** rather than guessed at: it is not on this machine,
   and nothing is claimed about a program that is not here. The day it is installed it gets
   measured the way Gemini was.

> **What the user sees:** their own hardware and their own accounts, listed honestly, and a way to
> add the rest that does not involve a terminal.
>
> **Summary:** Epoch stops having a list of backends and starts having a shape for one.

---

## Phase 9 — The World spans machines

**Ground.** One Runtime per World, and it is not negotiable:

> **Only one Runtime exists per World. Every Quest, Chronicle, Knowledge Base, Trust decision, MCP
> interaction and Project Root belongs to that Runtime. Remote Bridges contribute inference only.
> They never own World state.**

That principle is what keeps this cheap. A remote machine adds capacity and learns nothing.

**And a second one, added 2026-08-16, which governs everything below it: the Runtime never
chooses a Brain.**

> A Character owns a Brain. Changing the model changes the character — the same prompt on Gemma,
> Qwen and Claude reasons differently, and that is not an implementation detail, it is who
> somebody is. So Epoch **never** selects a backend on the user's behalf: no automatic failover,
> no load balancing, no "fastest available". The user assembles their crew deliberately, and the
> Runtime executes that decision rather than reinterpreting it.

ADR-0026 already says this one level down — a Guardian at 0.2 and a Researcher at 0.9 differ in
*who they are*, not in how they are configured. The model is the same argument, one field up.

When a backend is unreachable, Mage does not quietly become another Mage. Epoch says the assigned
Brain cannot be reached and offers to retry, to pick a different one, or to stop — and if the user
picks another, the Chronicle records that they did. Causality and identity both survive.

**Recommending is not choosing.** The Workshop measures this machine and says which models suit
it; the user may ignore that and assign something their VRAM cannot hold. Epoch told them, and
respects the answer. Helping is measuring and saying so — never deciding afterwards.


### The Bridge contract *(settled 2026-08-16)*

> **The Host owns reality. The Bridge owns computation.**

Stronger than "owns inference" on purpose: a Bridge may end up doing embeddings, speech or
diffusion, and none of those are inference. What it never owns is any part of the World.

**The protocol already exists and is not new work.** `Request` → `Answer` on the `Provider` trait
is exactly the boundary — model, conversation, parameters, tools going out; text and tool calls
coming back, with `Chunk` for streaming. A Bridge is a Provider whose transport happens to be
another machine, and nothing else in the Engine notices. The OpenAI-compatible backend already
wrote the HTTP-with-streaming half of it.

Four rules, and each one closes a hole:

- **The Bridge never executes a tool.** It *asks*, the Host judges through Trust and runs it. A
  Bridge running `write_file` on another machine is exactly the hole ADR-0027 closed.
- **The Bridge is stateless.** The Composer builds the whole conversation every turn, so a tool
  result arrives as another message in the next `Request` rather than as a continuation of a
  session the Bridge is holding. That is what makes "unplug it and nothing is lost" true rather
  than aspirational.
- **Pairing, not an API key.** A short code shown on the remote machine, exchanged **once** for a
  long secret kept in DPAPI on both sides. The code expires in minutes; the bond lasts. A code is
  guessable and lives on a screen — it is the introduction, never the credential.
- **The remote owns its own models.** The Host never configures them; it asks what is there, the
  way it asks every other backend.

Deliberately **not** in it: a heartbeat, and a latency reading. Pinging N machines forever is work
nobody watches, and the number would measure how fast a backend says "I am here" while reading as
how fast it thinks. The Launcher probes when it is open; a turn discovers a dead backend by
failing honestly.

**And more of it already exists than when this was written**, which is the first thing to say out
loud. A Bridge *is* a Provider (ADR-0003), and Ollama already speaks HTTP across a network —
pointing Epoch at `http://192.168.1.45:11434` is a backend entry today. Phase 8 now adds the other
half of the hardware anybody actually owns: llama.cpp and everything else speaking the OpenAI API,
at whatever address, which is what a machine under the desk usually is. So this phase must not build a protocol for
something that is already a URL. What a Bridge earns its existence for is the part a bare
network port does not have: identity, authentication, what it can do, what it is doing right now,
and what happens when it is gone.

1. **A Bridge is a Provider, not a new domain concept.** It accepts what a Provider accepts — a
   composed `Conversation` and the declared tools — and streams back. A `take_turn()` that took
   "a turn" would need to understand tools, capabilities and Quests, and a machine that
   understands those has started owning World state.
2. ~~**Pairing**~~ **— done 2026-08-19** (LAN discovery deliberately deferred, see the ADR). A bare Ollama on a
   network has **no authentication at all**, so the Bridge is where identity and a shared secret
   live. Same rule the agent door already follows: loopback is not authentication, and neither is
   a LAN.
3. ~~**A Bridge reports itself**~~ **— done 2026-08-20.** — models, memory, online or not.
   Measured by the Bridge, never estimated by the Runtime, and read the way every other
   instrument in Epoch is read: what is true, or visibly nothing. *Current load* and *latency*
   were struck by the ADR that same week and stay struck: one is work nobody watches, the other
   is a number that measures how fast a backend says "I am here" and reads as how fast it thinks.

   **Three things the build added that this line did not ask for**, each because using it made
   the absence obvious:

   **A machine is several brains.** The line assumed a machine reports *its* models. The MacBook
   serves Ollama, LM Studio and llama.cpp at once — measured — holding different files in each,
   so a flat list could not say which shelf a model was on. `Have` reports per runtime now, an
   `Ask` names which one should answer it (by id from a closed set, never an address), and a
   machine is one Provider per program on it. `Provider · Brain · Model`.

   **A model reports itself too.** A Bridge never implemented `surface`, so a lent machine's
   models were drawn with no capabilities at all — not measured and refused, never asked, and
   empty reads as *cannot*. `POST /show` closes it: `gemma4:12b` on the MacBook declares vision,
   audio, tools and thinking; through LM Studio the same question answers **nothing**, and the
   editor says so rather than drawing a blank twice.

   **Ask, write, rebuild, survey — in that order.** Providers are built from what a machine was
   last seen serving, and the probe is what finds that out. They ran the wrong way round, so a
   freshly paired machine was one Provider however many times PROBE AGAIN was pressed: not a
   stale reading, a reading that could not become visible. Pairing, unpairing and re-granting
   now re-probe on their own.

3.5. **A runtime that answers is a Service, wherever it is** *(2026-08-20)*. Not on the original
   list, and it should have been: the local case was built before the remote one and never
   revisited. A paired machine's LM Studio became a Provider the moment that machine reported it
   serving; this machine's needed `ADD AS A SERVICE` pressed, which wrote a backend entry saying
   what had already been measured. One fact, two rules, and which one applied depended on which
   computer the program happened to be installed on. The registry builds both the same way now,
   writes nothing, and a backend somebody configured by hand still outranks a measurement.
4. ~~**Preferred and fallback, per character**~~ **— answered differently, 2026-08-19, by the
   owner.** The phase text assumed an authored fallback chain. What shipped is the question asked
   **at the moment it matters**: when an assigned Brain cannot be reached, Epoch says so and
   offers three answers — try again, use somebody else's machine, or stop — and the Chronicle
   records the substitution when one happens.

   Same principle, less machinery. A stored preference is a decision made in advance about a
   situation nobody could see yet; asked live, the user has the actual failure in front of them.
   And it keeps the Runtime from ever choosing a Brain, which is the ground rule of this phase —
   a fallback list *is* the Runtime choosing, just earlier and with the user's fingerprints on
   it. Build the stored version the day somebody wants a crew that keeps working while they are
   asleep, which is a real want and not this one.
5. ~~**The disclosure question, asked once and honestly**~~ **— done 2026-08-19.** — a composed turn carries the user's
   source, their Chronicle and their knowledge. Sending it to another machine they own is
   different from sending it to a third party, and both are different from keeping it here.
   ADR-0025 already named this decision for hosted providers; this is the same decision with a
   different destination.
6. ~~**An ADR**~~ **— written first, 2026-08-19: [ADR-0029](docs/ADR/0029-bridges-and-the-worlds-machines.md).**
   It introduces a component, a protocol and a trust boundary, so it got written before anything
   was built — and writing it settled three things the phase text had left loose.

   **The protocol is `Request → Answer`, and that is all of it.** A Bridge is a `Provider` whose
   transport is another machine, so there is nothing new to design; the Composer already builds
   the whole conversation every turn, which is what makes *stateless* free rather than
   aspirational.

   **What is refused is now written down** — automatic failover, load balancing, "fastest
   available", a heartbeat, a latency reading, and any World state on a Bridge. Those are the
   obvious features and each one either makes the Runtime choose a Brain or produces a number
   that reads as something it is not.

   And a question the phase text did not have, **asked and answered the same day**: a Bridge
   **may** run capabilities that touch only *its own* machine — a terminal on the remote box.
   That is not an exception to rule 3, which refuses a Bridge acting on the *Host's* World; a
   remote terminal touches nothing of the World, Trust is still the Host's, and the pairing is
   what makes it safe. What it changes is the surface: "ran `cargo test`" and "ran `cargo test`
   on the Mac mini" are different sentences, and History records what happened.

**Life.** More machines, one crew.

7. ~~**The Bridge Console in the Launcher**~~ **— done 2026-08-19, above Connections.** — the devices as members of the World's infrastructure
   rather than a server list. Green, amber, offline; what each is holding; what it is doing now.
   Cold when there is nothing behind it, like every other instrument on that bridge.

> **What the user sees:** they buy a second machine and their crew gets stronger, without
> reinstalling anything or moving a single Quest.
>
> **Summary:** Epoch scales from one laptop to a room full of machines with exactly one source of
> truth.

**Phase 9 is complete (2026-08-20).** Every numbered item is struck, two of them by being
answered differently than written.

What the phase did not predict, and what the build taught instead: **the interesting unit turned
out to be the program, not the machine.** The phase text says "machines" throughout and every
rule in it survived — but a machine is a place where several brains live, and almost every defect
found in the last week was some surface answering a question about a program with a fact about a
computer, or the reverse. A Bridge named after a hostname offered `kisloks-MacBook-Pro.local` as
something to think with; a flat model list could not say which shelf a file was on; a local
runtime and a remote one obeyed different rules for becoming a Service.

**Measured, not remembered, again.** Eleven defects this week, and the ones that mattered were
all found by using the product and then measuring the thing itself — including one that was not a
defect at all: `cargo build --release` ships whatever `ui/dist` was last built, so a full session
of UI work was invisible in the running window with 305 tests green. Green tests measure the
source; the window renders the bundle. Use `cargo tauri build`.

---

## Phase 9.5 — The things it can lose, and the things it never measured

**Ground.** These were the *"Later, deliberately"* list: named so they would not be forgotten,
unscheduled so they would not distort the phases above. They are scheduled now, and the owner
added the three that make the difference between a product and a demo — **a demo only ever
accumulates.**

Everything in Epoch today can be created. Almost nothing can be removed. A World, a character
and a cache are three different kinds of deletion and none of them exists yet, which is why
they sit together rather than each waiting for the phase that happens to touch them.

**One rule above all of them** — measured, not chosen. A World's id appears in **eight** places
and two of them are paths into the user's own documents (`libraries.toml` at an Obsidian vault,
`projects.toml` at a source tree):

> **Deleting is an enumerated list of files Epoch created. It is never walking a directory.**

**Six questions, answered by the owner 2026-08-19:**

1. **The library is never touched — not even `Epoch/` inside it.** Epoch wrote those notes and
   they sit in a vault the user may have linked them from. Deleting a World removes Epoch's copy
   of the work, never the user's notes about it. Same for the project root: the pointer goes, the
   folder never does.
2. **The shipped `default` pack stays** — it is the base other Worlds are built from, so it can
   be *hidden from the list* rather than deleted. A World the user made is deleted entirely.
3. **A character is never deleted with a World.** The confirmation states what it implies —
   *"3 characters stop living there"* — and the user accepts it.
4. **Trust decisions for that World go with it.** Security, not tidiness: a later World reusing
   the id would otherwise inherit permissions granted to a different one.
5. **A Quest whose character was deleted stays open**, and the user hands it to somebody else.
6. **"Clear cache" is an "Erase data" screen** where the user picks what goes. Agent session
   handles are their own line with their own cost stated — they look like cache and are somebody's
   open conversation.

1. ~~**Delete a World**~~ **— done 2026-08-19.** The largest thing a person can throw away here, and the one that must be
   hardest to do by accident. A World is a pack, a map, Quests, Knowledge and a library path
   that may point at a folder full of somebody's own notes. **What Epoch created it may remove;
   what the user pointed it at, it never touches** — the same confinement rule the import
   pipeline already follows, applied to going the other way.

2. ~~**Delete a character**~~ **— Engine done 2026-08-19; the surface waits on the app.** They
   live in the vault and travel into every World (ADR-0023), so removing one is an edit to *the
   crew*. Their artwork goes with them; their name does not vanish from the Quests they worked
   on, because History records what happened and is not rewritten by somebody leaving.

   **Say it, then do it.** `character_removal` returns a plan — what goes, what changes, what
   survives — the user accepts *that*, and only then does `remove_character` run. A confirmation
   that only says "are you sure?" is a confirmation about nothing. The plan is rebuilt from the
   Engine at delete time rather than carried back from the window: a list of paths that came out
   of a surface is a list somebody could change, and this removes files.

   **Enumerated by slot, never by prefix.** `mage` and `mage-of-the-north` are two people, and
   anything matching a name prefix would take the second one's sprite when the first one left. A
   test holds it.

   How many conversations name them is read from digests rather than whole Quests — a
   confirmation must not spend a second reading transcripts nobody will look at.

3. ~~**Clear cache**~~ **— done 2026-08-19, as "Erase data".** It has to say what a cache *is*
   before it offers to clear one. Measured
   probes, resolved marks, context windows, an agent's own session handles: some are free to
   rebuild and some are somebody's conversation. The button that cannot say which is which is
   the button nobody presses.

**And the ones that were already waiting.**

4. ~~**XP, levels and energy**~~ **— done 2026-08-21, and the premise had aged.** `LV 0` and
   `ENERGY 0/100` are nowhere in the source: they were the reference design's readings and Epoch
   never copied them, so this item's own examples had been answered as the panels were built.
   Three rows were left, and each needed a different answer:

   **REACTOR TEMP → REACTOR LOAD, measured.** It read `—` with a note promising GPU telemetry
   "once local models run on this machine". They had run here since Phase 1 and `Machine::measure`
   had been reading VRAM through `nvidia-smi` the whole time — the Workshop weighs models against
   it. The note was not describing a missing subsystem; it was describing a wire nobody had run.
   **A dark gauge beside a live reading of the same number is the cold-instrument rule failing in
   the other direction**: silence about something Epoch knows. Load, not temperature — nothing
   here reads a thermometer.

   **CONTEXT DRIFT → removed.** It waited on the Context Composer's report, which exists and is
   measured — but per *turn*, arriving as an event while a conversation runs, and the World's HUD
   already shows it beside the character it describes. The Launcher is outside every World, so
   the row could never fill.

   **LEVEL → removed.** Drawn cold and honestly, resting on the rule that a dormant panel earns
   its outline by saying what it is waiting for. What it waited for was never scheduled: no XP
   design, no ADR, nothing scoring anything. **A frame reading NOT YET COUNTED for long enough
   stops being a promise and becomes furniture.**

5. ~~**Export a World**~~ **— done 2026-08-21.** It does **not** carry the crew, and ADR-0023 is
   why: a character is a portable asset that belongs to the user and travels into every World they
   open. Before that ADR a pack supplied the cast and carrying them would have been obvious; it
   stopped being true, and an export that sent people would be sending people. Their face is also
   whatever they imported, which is exactly what the hard rule governs at the moment of
   distribution. Quests do not travel either — they hold what was said and what was read out of
   the user's own source tree.

   **A World with no licence is refused**, and *declared* means answered: an empty `type` is a
   manifest filled in without being answered.

   **An archive, and the user says where** *(corrected 2026-08-21, using it)*. It first wrote a
   folder, reasoning that a World Pack *is* one — `WorldPack::discover` looks for `pack.toml` in a
   directory — so what came out was directly what another Epoch installs and the import half
   needed no code. True, and it missed the point of exporting: a folder does not travel, and
   somebody handing a World to a friend sends **one file**. Epoch also chose the destination and
   announced a path, which turned *sending a World* into *finding the World Epoch put somewhere*.

   So `CHOOSE WHERE…` opens a native Save dialog **from Rust** — `rfd` has no JavaScript surface,
   so ADR-0024 survives: the window still cannot open anything or learn a path it was not handed.
   Cancelling is an answer, and writes nothing. The folder writer went with the button that
   called it: two ways to write one plan, one of them unreachable, is the second-worst shape.
   Enumerated, never a directory copied, for the reason `erase` gives: a `copy_dir_all` carries
   whatever happens to be in the folder, which is how a Quest reaches a stranger.

6. ~~**Activity Stream**~~ **— done 2026-08-21**, at the horizon the ADR set: in-memory pub/sub,
   emission, subscription and metadata built; the Recorder a trait with no implementation; replay
   nowhere.

   The hard rule shaped the types more than anything else. Nothing may depend on an Activity
   existing after emission, so `emit` returns nothing and cannot fail — a caller that could handle
   a failure would be a caller depending on delivery. An Activity carries ids and a sentence,
   never state, and a test asserts the wire has no `messages`, no `transcript`, no `content`.

   **Synchronous dispatch**, which the ADR left open: a queue is a thread, a buffer and a
   backpressure policy for a delivery nothing may rely on. The cost is named rather than hidden —
   an emitter is as slow as its slowest subscriber — and it is acceptable because a subscriber
   that would block is doing something it should not be doing on that path.

   **Emitters and a consumer in the same change**, because a bus with nobody on it is the
   cathedral Earn Complexity forbids. It listens to the probe (the only thing that learns whether
   a machine answers) and to turns beginning and ending, correlated so a turn can be grouped from
   the stream alone. `WHAT JUST HAPPENED` sits above the Ship's Log and says a different thing on
   purpose: one is a forgetful window on memory, the other is persisted evidence.

7. **Image generation** — decided 2026-08-21: [[docs/ADR/0030-images-are-made-by-a-capability]].
   A **capability**, not a Provider — a character does not think in pictures, it asks for one, and
   widening the turn for every text backend to serve one caller is the cost that decision avoids.
   ComfyUI first, its workflows **imported** rather than authored here, which is what makes the
   owner's narrow exception to the node-graph ban defensible. A generated image is evidence on a
   Quest and never World Pack artwork.

   ComfyUI installs from Settings and is measured like a runtime, without being one.

8. **The Official Epoch Universe** — **scoped by the owner 2026-08-21, and most of it already
   exists** ([its own milestone](docs/Milestones/Official%20Epoch%20Universe.md)). The world name
   is the pack's name, lore is Missions, cities are buildings, the logo is the mark EpochServices
   ships; factions and symbols are discarded; the cast left with ADR-0023.

   What is left is two doors and then artwork: **World Edit can set the World's skin** (the `[[ui]]`
   concept pipeline exists and has no interface) and **World Edit can give a World sound** (which
   does not exist at all — SFX are synthesised with no files, music is absent, and `asset.rs`
   delivers only images). After that, creating the universe replaces a World Pack and nothing else,
   which was the point of the concept indirection.

> **What the user sees:** they can undo the big decisions, not only make them — and every gauge
> on screen means something.
>
> **Summary:** Epoch stops being something that only ever grows.

9. **A machine that lends gets the same shelf the Host has** *(done 2026-08-21)*. Measured on
   the paired MacBook: Ollama answering with 8 models, llama.cpp with none, on one disk — the
   other two runtimes read files and Ollama stores blobs named by digest with no extension. The
   Host had grown one press for that and the machine whose entire purpose is lending models had
   not, which made *what a machine can lend* depend on which program you looked at it through.

   The policy lives in `epoch-models` (`everything_here`, `serve_everything`, `lend_everything`)
   because both surfaces depend on it. **This is now a question asked before building anything:
   does EpochServices need it too?**

### The loose ends, answered 2026-08-21

**Ollama's own shelf is searchable.** `ollama pull` takes names from two registries and only one
was searchable. Measured first: `ollama.com` has no search API (`/api/search` → 404, and
`?format=json` returns HTML whatever you ask for) and its registry manifest endpoint genuinely
answers. That pair is what makes reading names off the page defensible rather than a scrape —
they come from `href="/library/<name>"`, the site's own URL structure rather than a CSS class, and
every one is weighed against the registry that answers. A page that changes shape yields *fewer*
results, never wrong ones, and none is reported as *unreadable* rather than *no matches*.

**A cloud model is not local, however local the program running it is.** Ollama runs on the
user's own machine, and `ollama pull gpt-oss:120b-cloud` gives it a model it executes on Ollama's
servers — measured, that tag resolves in the registry like any other. `disclosure::destination`
takes the model now.

**A cloud *listing* was written and removed**, because measuring caught it lying.
`ollama.com/search?c=cloud` answers *which families have a cloud tag somewhere*, not which models
are cloud models: `gemma4` is on that page and its tag list is twenty ordinary local tags. It
would have told somebody "gemma4 is a cloud model" about a model on their own disk.

**Non-GGUF models: deliberately not built, and the constraint made visible instead.** Measured:
`Qwen/Qwen3-8B` is five safetensors shards, and searching `Qwen3-8B` *with* the GGUF filter
returns `Qwen/Qwen3-8B-GGUF` — the official conversion of the same model. For nearly anything
somebody wants, the converted version already exists and the Workshop already finds it. Where it
does not, converting means fetching full-precision weights (~16 GB for an 8B in bf16 against ~5 GB
as Q4), running `ollama create --experimental`, and holding both: three times the disk and an hour
of CPU to produce something usually already published. So nothing converts, and an empty search
now says *why* it is empty rather than looking like a model that does not exist.

---

## Phase 10 — The World makes things

**Ground.** Image generation, end to end, on the contracts already accepted: a **capability**
rather than a Provider (ADR-0030), a **Style** rather than a workflow filename (ADR-0030
amendment), and Trust deciding every call.

Nothing here starts from zero. Measured 2026-08-21: ComfyUI installs from Settings and serves,
SDXL draws 1024×1024 in **12.1 s** on the machine this was built for, Epoch's own detection
reports it holding both checkpoints, and all three local runtimes answer with native
`tool_calls`. What is missing is the capability, the Style layer and the door to import a
workflow.

1. **Styles** — the semantic layer. A character asks for *pixel art*, never for `pixel_v3.json`.
   ~~Six ship, chosen by the owner 2026-08-21: **General · Realistic · Pixel Art · Anime · Fantasy
   Illustration · Character Portrait**.~~ **Superseded 2026-08-23** by Phase 11.7 — the six were a
   hypothesis and using the product refuted it: a closed set answers *"that style does not exist"*
   to somebody asking for watercolour, which is a sentence about an array delivered as a sentence
   about the world. Styles are now **read from the library**. What survives unchanged is that
   nothing ships *behind* a name.
2. **Workflow import** — `Export (API)` from ComfyUI, and the Engine derives the rest from the
   graph: which `CLIPTextEncode` is positive (the `KSampler` says), the size, and whether it can
   do img2img (is there a `LoadImage`?). **Derived, never declared** — a hand-written capability
   that lies is worse than none (ADR-0005).
3. **`draw_image`** — descriptor, `decide()`, and `Entry::Produced { artifact }` on the Quest.
   The picture is drawn in the conversation by the path that already draws shared ones.
4. **Reference images** — `from_image`, confined to what was shared *in this Quest* by
   `shared_images_in`. Structural: there is no argument a character can send that reaches a
   picture nobody offered it.
5. **`edit_image`** — inpaint and retouch. Its own descriptor, because *modify this* and *create
   one* are different sentences. **Never overwrites**: a new file, so undo is delete.
6. **A character's preferred Style** — a canonical parameter (it survives changing the engine,
   ADR-0026). A preference, never a cage: the request wins, then the character, then the machine.
7. **A lent machine can draw** — ADR-0029 already said a Bridge contributes *computation* and
   named diffusion when it said it. Same panel, same three facts, in EpochServices.

> **What the user sees:** they ask a character for a picture — in any local model or any agent —
> and it appears in the conversation, after an approval that showed the whole prompt first.
>
> **Summary:** the World stops only writing things.

---

## Phase 11 — The Library *(done 2026-08-24, except Styles)*

> **What it turned out to be.** The phase was written expecting a shelf and a Style layer. What
> the build produced is larger and simpler than that: **one library, one reader of bytes, one
> catalogue trait with two sources, and one assembled recipe** that draws every family which
> ships in parts. Everything below is finished and measured except where it says otherwise.
>
> Six things were found by using it rather than by reading it, and each is written where it was
> fixed: a `.epoch.json` manifest listed as if it were a LoRA; a panel that said *nobody
> measured* about a checkpoint sitting in ComfyUI's own folder; cards whose titles pushed past
> their column; a version picker that fetched a 170 MB Z-Image LoRA for a Flux graph; a panel
> that took half a minute to open; and a character that wrote its own tool call into the
> conversation.

> **Rewritten 2026-08-23**, after the owner tried to import a workflow and could not, and then
> read the whole picture chain end to end. Three decisions came out of it and each has a
> document: [ADR-0032](docs/ADR/0032-the-generative-library.md) (Epoch owns the library),
> [ADR-0031's amendment](docs/ADR/0031-the-asset-registry.md) (three shelves, one registry) and
> [ADR-0030's second amendment](docs/ADR/0030-images-are-made-by-a-capability.md) (a Style is
> derived, not shipped). The previous version of this phase — *"the Workshop grows shelves"* —
> was right about the Engine and wrong about the screen.

**Ground.** The Workshop is not new and must not be built again. It already searches Hugging
Face, pages results, weighs every model against this machine's memory, and exists in **both**
surfaces. What is missing is not a screen: it is **somewhere for the files to live**, a way to
know what a file *is*, and a Style layer that reads both instead of being an array somebody
chose.

**Cut to two sources by the owner, 2026-08-22: Civitai and Hugging Face.** Both have documented
APIs; together they are a larger library than anyone will exhaust. ComfyWorkflows, Tensor.Art and
GitHub were considered and deferred — each is a provider that can be added later without touching
anything else, which is the whole point of the registry.

1. **The Generative Library** — *done.* (ADR-0032) — one library per installation, owned by Epoch,
   `models · loras · vaes · upscalers · controlnet · embeddings · workflows`. Epoch **adds a
   search path; it never moves the user's files.** Nothing lands in another program's tree.

   **Measured first, because it gated the rest** — asked of ComfyUI v0.33.3 on 2026-08-23:
   `extra_model_paths.yaml` beside `main.py` **is** read (`Adding extra search path checkpoints
   ...`); a **new file** in an existing search path is seen **without a restart**, within seconds;
   a **new search path** is **not**. `main.py` agrees: that file is loaded unconditionally at
   boot, before any `--extra-model-paths-config` argument.

   So the handshake happens **once**, asks for one restart at that moment, and after it every
   asset that arrives is visible immediately -- the arrangement worth having. It compares before
   it writes, so opening the panel twice writes once. Comfy Desktop generates its own
   `instance-model-paths/<id>.yaml` and stamps it *do not edit manually*; Epoch writes a
   different file, so neither program overwrites the other.

2. **A file is understood from its bytes** — *done.* — hash, then the registry, then a manifest beside it:
   kind, base model, trigger words, licence, size. ADR-0024's rule one subsystem over. An unknown
   hash is **unknown**, stated and still usable; never guessed from a filename, which is how a
   FLUX LoRA gets filed as SDXL.

3. **The Asset Registry** — *done.* (ADR-0031) — one canonical `Asset`, one `Catalogue` trait, a provider
   per source, and **two real implementations from the first day** so the shape is exercised
   rather than asserted. A test holds the property that matters: the caller never learns which
   source answered, and one busy site does not empty the screen.

   **A refusal is not an empty answer**, and it is four sentences rather than one — *busy*,
   *unreachable*, *unreadable*, *needs a key* — because they have four different fixes. This came
   straight out of the measurement below.

   `hf.rs` stays what it is (the CLI, fetching a GGUF for a brain); `hugging_face.rs` is the
   catalogue. Same company, different question, and ADR-0031's amendment is exactly about not
   filing `gemma4:12b` beside `sd_xl_base_1.0.safetensors`. **Folding the brain shelf onto the
   same `Asset` is deliberately deferred** until a screen needs it — the trait is proven by two
   generative sources today, and rewriting a working Workshop to prove it twice is churn.

4. **Civitai** — *done.* **Measured on 2026-08-23 before a line was written**, like every source
   before it:

   | asked | answered |
   |---|---|
   | `/models/2851202` (the owner's own link) | 200, no key |
   | `/models?types=LORA&limit=3` | 200, no key |
   | `/models?query=watercolor&…` | **503 six times over minutes, then 200** |
   | `/model-versions/by-hash/<sha256>` | 200 with the whole hash; **404 with a short one** |
   | `/api/download/models/…` | **401 Unauthorized** |

   Four of those shaped the design. Free-text search is the flaky endpoint, so `Busy` exists and
   is never rendered as *nothing matched*. The hash lookup needs the **whole** SHA-256 — a
   truncated one 404s, which would have read as *not ours* — and it is what makes hashing a
   6.9 GB file worth the time. **Downloading needs the user's own key**; searching does not, so
   the shelf is fully usable before anybody signs in anywhere, and Epoch says which key is
   missing rather than asking for a password.

   And the base vocabulary is **open** while Epoch's is closed: the owner's LoRA is `Krea 2`,
   which nothing here has a graph for. So both travel — the source's words verbatim, because a
   person recognises them, and the family Epoch can actually build with, `Unknown` when it maps
   to nothing. Measured live: `Pony` → SDXL (an SDXL-architecture fine-tune really does load),
   `ZImageTurbo` and `Anima` → unknown and shown as such.

5. **Compatibility, measured** — *done 2026-08-24.* `base_model` decides. A refusal names the reason in the words a
   person uses, never `mat1 and mat2 shapes cannot be multiplied`, and **KEEP IT ANYWAY** is
   always offered: the file is theirs, in their vault. Epoch measured and said; the answer is
   theirs. Same arrangement as a model too large for this card.

6. **The Studio Panel** ([ADR-0033](docs/ADR/0033-the-studio-panel.md)) — *done 2026-08-24,
   and measured end to end: searched, installed `nerijs/pixel-art-xl` (170,543,052 bytes, its
   sha256 matching Hugging Face's `lfs.oid`), read from its own bytes as a LoRA of family SDXL,
   listed by ComfyUI through the search path Epoch added, and drawn in 8.0 s.*

   One defect found by using it: the panel said **nobody measured** about a 6.9 GB checkpoint
   sitting right there, because a checkpoint the user already had lives in ComfyUI's tree and not
   Epoch's library. Epoch now reads the search paths ComfyUI itself declares and opens the file —
   **reading is not adopting**, which is what ADR-0032 forbids. — the character **offers**
   it and the user fills it in: model, LoRAs and strengths, prompt, aspect ratio, output, and a
   folded `ADVANCED ▾`. A **form, never a node graph**. Only what is installed appears, read by
   hash. Incompatible LoRAs are greyed with the reason and **USE IT ANYWAY** is always there.
   `IMAGE · VIDEO · 3D` from the first version, with the last two **cold** — each keeps its frame
   and names Phase 12 as what will light it. *(Phase 12 lit all of them on 2026-08-29, and added
   `AUDIO` between them.)*

   *Why now:* measured 2026-08-23, `LoraLoader` exists nowhere in the codebase, so a downloaded
   Civitai LoRA is inert no matter how correctly it lands. The Style vocabulary constrains the
   *character* and had been quietly extended to the *user* — who owns the machine and the file.

7. **The Workflow Compiler** — *done 2026-08-24, as `workflow::compose`.* Generalise `starter`. It already asks the server for its schema,
   writes a graph and compiles it against *that* server. It grows from *one plain graph* to
   *a request plus resolved assets*: model, LoRAs and strengths, size, reference, upscale.
   **Nothing ships as a `.json`** — the graph is derived from what this machine reports, which is
   what made `BUILD ME ONE` defensible and is unchanged here.

   Two things ADR-0033 makes load-bearing: a **recipe per architecture** (SD 1.5, SDXL, Flux and
   Z-Image do not share a graph), chosen from the registry's `base_model` and **never** from a
   filename; and `LoraLoader` chained per selection. An **unknown** architecture gets the plainest
   recipe and is told so — a guess fails inside a render instead of at the door. It compiles
   against whichever bench will actually draw, local or lent.

8. **Styles are read, not shipped** — **the one thing left.** Today they are still the fixed six under *Advanced*. (ADR-0030's second amendment) — install something that paints
   watercolour and the Style exists, because something draws it. The six-name array goes.
   `General` survives because it is the absence of a style, not one of them. **A character may
   never create a Style** — that is inventing a capability — it says what it cannot do, lists what
   it can, and offers to look.

9. **Three shelves** — *done.* (ADR-0031's amendment) — `MODELS` (brains) · `CREATION` (checkpoints, LoRAs,
   VAEs, upscalers, workflows) · `MCP` (tools from outside, which moves in, so the main menu loses
   an entry). One registry, one install path, one measurement, rendered three times. Held by a
   test: adding a catalogue changes no screen.

10. **Licence and NSFW, explicit** — *done.* Adult is filtered and said; every manifest carries the licence; and an export names the imported artwork its declared licence was never written about. — downloading into your own vault is yours; redistributing is
   what `CONTENT_PHILOSOPHY`'s hard rule governs, and export checks it. Civitai carries a great
   deal of adult material: filtered by default, and **said rather than hidden**.

11. **A character offers; it never installs** — *done by construction:* no capability can install, and the Workshop is a screen. — *"I found 4 things that paint watercolour and fit
    this machine"* and then it stops. Fetching gigabytes is not a decision a tool call makes.

12. **Both surfaces** — *done.* Library, manifests, catalogues, the Creations Workshop, import, the credential store and the VRAM verdict all exist in EpochServices. — a lent machine is the machine an asset would be installed *onto*. Library,
    manifests and the three shelves belong there too.

13. **One assembled recipe** — *done 2026-08-24, and it replaced fifteen.* Flux, Flux.2, SD 3.5,
    Z-Image, Qwen-Image, Hunyuan, HiDream, Chroma, PixArt, Wan and LTXV differ in **one string**:
    the `type` a `CLIPLoader` is given. ComfyUI publishes that list — 28 of them, measured — so
    Epoch offers what the server reports and a family added tomorrow works with no code.

    Measured end to end on the same code path: **Flux** (85 s), **Flux + the owner's VHS LoRA**
    (65 s), and **Z-Image Turbo** (8 steps). SDXL and SD 1.5 keep the checkpoint recipe, which
    covers about 92% of a 400-model sample of Civitai on its own.

14. **A version is a different file** — *done.* The owner's LoRA is published twice, 170 MB for
    Z-Image and 18 MB for Flux, and taking the largest fetched the wrong one. An id can now name
    one (`civitai:2851202#2142473`), and a named version that no longer exists yields nothing
    rather than quietly falling back.

15. **One credential store, three operating systems** — *done, and half of it unmeasured.*
    `epoch-secrets` is linked by both programs: DPAPI on Windows, the Keychain on macOS, the
    Secret Service on Linux.

    **Windows and macOS are both exercised now.** macOS was settled 2026-08-24 on a real machine
    (26.6.2, arm64, M2): a Rust toolchain was installed there and the crate's own tests were run
    against a real, unlocked keychain — all seven, `what_is_here` among them. Store, read back, a
    missing item as exit **44**, delete, gone.

    Getting there found a defect and killed an invention:

    - **The value was in the process list.** `security` says of itself *"Use of the -p or -w
      options is insecure"*, and Epoch was passing `-w <value>` — the very thing the Linux path
      had been written to avoid on purpose. It is fed twice on stdin now (`security` prompts
      twice; measured, because the manual only says *prompted* and does not say whether a pipe
      counts).
    - **A guard against newlines was written and then deleted.** It could never fire: what reaches
      `save` is one compact `serde_json::to_vec` document, so a newline in a stored value is two
      characters inside a single line. The test that replaced it asserts the true thing — such a
      value round-trips — and it passes on macOS.

    **Linux remains written from its manual and never run.**

16. **What fits, measured** — *done.* Every asset is weighed against this card's **free** memory,
    with the same tenth of headroom the Models Workshop leaves. Nothing is said when nothing was
    measured, and what does not fit is still offered.

**Life.** *The wall stops being a wall.*

Today, asking for a style Epoch does not have ends the conversation. After this it starts one:
the character says what it cannot do, lists what it can, and offers to find the rest. Two presses
later the Style exists — and it exists because a file arrived, not because anybody said so.

> **Both of the two it left standing are now done (2026-08-25).** 11.8: a Style is derived — it
> exists because a workflow serves it and goes when the last one is taken away, `General`
> excepted, and naming one on a workflow row is what creates it. 11.10: an export names the files
> the declared licence was not written about — artwork the user imported — and says it rather than
> refusing, because the hard rule governs what Epoch *distributes* and Epoch cannot read what a
> picture is or who made it.
>
> **What the user sees:** they ask a character for a picture; it opens a panel; they choose a
> model, a LoRA and a size from what they actually have; and the picture comes back as the
> character's reply. If they do not have what they want, they search two sites at once, see what
> fits their card, choose a version and install it — without ever opening ComfyUI.
>
> And when they *do* want to know what a checkpoint is, the panel is there: the character asks
> whether to set it up or just draw, and either answer is one press.
>
> **Summary:** things have somewhere to live, Epoch knows what they are, and what a World can make
> is read off what it has.

### 11.17 — Every brain, measured against the brush *(done 2026-08-24)*

The picture chain was finished and only ever exercised with one brain on one runtime. So it was
put to all of them — the three local runtimes started **from Epoch's own Connections deck**, and
both agents through Epoch's door. Four defects came out of it, and not one was visible to the
1005 tests that were passing.

**LM Studio and llama.cpp both start from the deck.** llama.cpp in under 3 seconds, LM Studio in
under 30, each in its own terminal, each serving the five models Ollama already holds — linked,
not copied.

**A written call is now a call** (`provider.rs`, `recover_written_call`). The same question put to
`gemma4:12b` through all three runtimes: Ollama emitted **no** `tool_calls` and wrote
`{"action":"draw_image","action_input":"…"}` into its answer, LM Studio did the same, and only
llama.cpp turned it into a real call. Whether a written call becomes a real one is the *chat
template's* job and the templates disagree — so a character could draw on one of this machine's
three servers and not the other two, and on those two it said a line of JSON instead. Epoch now
reads it, bounded exactly like `hush_echoed_calls`: the whole message must be one JSON object
naming a capability **offered this turn**.

**Codex draws with its own tool, and Epoch now shows it** (`agents/codex.rs`). Measured against
the app-server: the item is `imageGeneration`, the PNG arrives **base64 inside the notification**,
and the working directory was left **empty** — so the picture existed only in a message Epoch was
dropping on the floor. It is now written into the World's pictures, named from its bytes and never
from the agent's `revisedPrompt`, and it counts as evidence. 36 s to draw, 54 s for the turn.

**Claude Code in Auto could not reach *any* Epoch capability** (`agents/claude.rs`). Epoch named
its permission tool only when the mode was stricter than `Auto`, reasoning that Auto already means
yes. Claude Code 2.1.241 answered *"I need your permission to call the epoch_watchword tool"*,
twice, and gave up — the crew, the Quest, the World's knowledge and the brush, all unreachable.
Asking the program shows why: `--permission-mode` grew a `dontAsk` rung, so `auto` is no longer
the one that never asks. The tool is now named unconditionally and Epoch answers with the mode it
already holds. Codex was unaffected.

**Latencies, warm, on this card** (RTX 4070 SUPER, 12.9 GB). First load is 30–42 s on every
runtime and is not a latency — quoting it sends somebody optimising the wrong thing. After that,
one short answer: Ollama 8.5 s · llama.cpp 10.5 s · LM Studio 19.8 s. Declaring the two picture
tools does not itself cost much; the length of what the model writes does. `qwen3-14b` reaches for
the brush first try on both OpenAI-door runtimes and needs no recovery.

> **What the user sees:** every brain in the deck can now make a picture, and the two that could
> not say so are fixed rather than documented.

### 11.18 — The picture nobody could actually make *(done 2026-08-24)*

11.17 fixed four real things and **not one of them was why the product did not work.** The owner
sat down, asked for a picture, and could not get one on any runtime. Three defects, found by
using it; two of them were places where Epoch had turned *not knowing* into *no*.

**No capability at all on llama.cpp or LM Studio.** `Declared::uses_tools` was a `bool`. Ollama
publishes it per model so `false` meant no; the two OpenAI-door backends publish nothing about
tools and filled it with `false` anyway, the surface read that as a refusal, and every character
on those backends was handed **no tools whatsoever** — then said so honestly: *"no tengo acceso a
herramientas en este mundo."* It is `Option<bool>` now, `None` is unasked, and only a definite
`Some(false)` makes a turn toolless. `Shown` had carried this rule correctly since the Bridge was
built; `Declared` had it in its comments and not in its type.

The reason 11.17's measurement missed it entirely is worth keeping: **that harness declared the
tools itself.** It proved the models call them and proved nothing about whether Epoch would offer
them.

**A vocabulary attributed to the wrong node.** The assembled recipe was built on `CLIPLoader`'s 28
encoder families, measured correctly and then treated as ComfyUI's rather than as that node's.
Asked directly: `DualCLIPLoader` publishes **twelve**, and `stable_diffusion` is not among them;
`TripleCLIPLoader` has **no `type` input at all**. So the panel offered `stable_diffusion` beside
two encoders and the server refused the graph after the user had filled everything in correctly.
The panel now asks both loaders and shows the list belonging to the one the choice selects, and
the compiler sends no `type` where the loader has none.

**And ComfyUI's refusal became an instruction.** Its raw JSON reached the model, which answered
*"hubo un error técnico… Intentaré de nuevo con una descripción ligeramente ajustada"* and called
again — twice, changing the one thing that was not wrong. The reason is kept, bounded now:
nothing exists, this is not the description's fault, another attempt fails the same way.

> **What the user sees:** asking a character for a picture produces one.

---

### 11.19 — Giving the card back, and saying what is on it *(done 2026-08-25)*

The measured-not-yet note above, closed — and the two things it was hiding.

**Both servers can be told to let go, and each has its own word for it.** Asked rather than
remembered, which mattered because the two answers are not the same mechanism:

| runtime | how | measured |
| --- | --- | --- |
| Ollama | `keep_alive` in the request | already honoured |
| llama.cpp | `POST /models/unload` → `{"success":true}` | `loaded` → `unloaded` |
| LM Studio | `ttl` in the request body | 7.56 GB → gone in 8 s |

Two traps, and both are the kind that pass a status check:

**`ttl: 0` does nothing.** LM Studio reads zero as *unset* and falls back to its own hour. The
obvious spelling of *now* would have shipped a fix that changed nothing.

**And LM Studio answers `200` to a route it does not have**, with the refusal in the body. Every
unload spelling tried against it (`/api/v0/unload`, `/models/unload`, `/v1/internal/model/unload`)
came back *"Unexpected endpoint or method"* — behind a success code. A status-code check would
have reported a working release for a call that did nothing; the state was read back instead.

One honest limit is written into the code rather than glossed: **LM Studio binds `ttl` when the
model loads**, so a model somebody opened in LM Studio's own window keeps their hour whatever
Epoch sends. This frees what Epoch loaded and leaves alone what it did not.

`Run several crew members at once` therefore means something on every door now, and the trade is
measurable: llama.cpp answers a cold turn in **17.7 s** and the next one in **2.0 s**.

**The deck was reading a shelf and calling it a card.** `models` is what a server *offers*, and
the row printed it as *"holding …"* — so a router listing five files on disk reported five models
in memory, to somebody looking at a 7.5 GB process in Task Manager. There is a second, real
reading now: `offers …` and `in memory: …`, the latter measured from each server's own answer
(`/api/ps`, `status.value`, `state`), and it **reads `nothing` out loud** rather than vanishing.
`sleeping` is not counted — across one idle release the process went 8812 MB → 126 MB and the
card 10736 MiB → 1937 MiB.

**And `llama-server.exe` under LM Studio was not a bug in Epoch.** `lms server start` starts
`LM Studio.exe --run-as-service`; the moment it loads a model it spawns `llama-server.exe`,
because LM Studio's inference runtime *is* llama.cpp. Two rows, one process name, and the
reasonable reading was that Epoch pressed the wrong button. The row says so now, in one sentence.

**Two more, found on the way.** LM Studio publishes `capabilities: ["tool_use"]` per model on
`/api/v0/models` — a real yes where Epoch had been answering *unasked*, with a missing array
still meaning unasked because `gpt-oss-20b` has none and calls tools fine. And llama.cpp's
`/v1/models` grew a `meta` block: `n_ctx` is **what the model was actually loaded with** (82944
where `n_ctx_train` says 262144), which is the number a turn must be composed against.

**And a prompt that had been wrong in silence.** `PANEL_IS_OPEN` existed twice in two spellings
and one of them carried literal `\n` and five spaces of source indentation into every turn that
opened the panel. Nothing asserts on whitespace, so nothing saw it. One constant now.

**Every brain, all the way to a file.** A new test in the shell — the only crate that has the
easel — asks each Provider what it declares, applies **the rule the turn loop applies**, calls
the same `take_turn` the window calls, and runs the real `Painter` against the real ComfyUI:

```text
Ollama    / gemma4:12b  draw_image in  8.0s  ->  drew in 11.8s
llama.cpp / gemma4-12b  draw_image in 70.2s  ->  drew in 11.7s
LM Studio / gemma4-12b  draw_image in 23.6s  ->  drew in  8.7s
```

llama.cpp's 70 s is the model load, not the model — and it is the cost of `Never`, paid once per
turn, which is exactly what the toggle is for. This test is the answer to 11.18's own lesson:
the harness that missed the toolless bug built its own request.

> **What the user sees:** the picture arrives on every runtime, the deck says what is actually on
> the card, and an idle crew gives it back.

### 11.20 — Flux through the window, on all three brains *(done 2026-08-25)*

The first work driven **through Epoch's own UI** rather than through its API. WebView2 opens a
CDP endpoint when the process is started with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`, so every
click and keystroke below landed in the real window: the real Launcher, the real airlock, the real
Studio Panel, the real Chronicle. Three runs — Ollama, LM Studio, llama.cpp — one prompt,
*asuka evangelion in the city, night, 80s style, neon lights city*, drawn with **Flux 1-dev** and
the **japanese_retro_aesthetic** LoRA chosen in the panel.

**Everything the panel was built for worked on the first pass.** Picking `flux1-dev` grew the
`THIS MODEL ARRIVES IN PARTS` block; picking a second encoder swapped FAMILY from `CLIPLoader`'s
28 names to `DualCLIPLoader`'s twelve, which is 11.18's fix seen from the user's side; the Flux
LoRA moved to the top marked `FLUX · USE` while `pixel-art-xl` dropped to
*"This is for SDXL … It will not load. USE IT ANYWAY"*, and the two unmeasured LoRAs read
*"Nobody measured which family one of these belongs to"* — unknown, not refused.

**And then four defects that only using it could find.**

**A turn released the model after every round.** `KeepLoaded::Never` was honoured inside the
Provider, and a Provider sees rounds: the model was unloaded while ComfyUI drew and fully
reloaded to say one sentence about the picture. The release moved to `turn::drive`, which is the
only place that knows the answer is finished — including the pause for approval, because a turn
waiting on a person is exactly when the card should be somebody else's. **LM Studio: 203.5 s →
86.9 s.**

**llama.cpp could not take a tool-using turn at all.**

```text
request (8235 tokens) exceeds the available context size (4096 tokens)
```

`llama-server` had loaded the model with 4096 and Epoch had never said what it needed. Ollama has
been told since the `num_ctx` fix; llama.cpp cannot be told per request, so it is told once, at
`-c 16384`, and a test asserts the number clears the turn that was measured failing rather than
merely asserting the flag. `--sleep-idle-seconds` joins it as a lifeboat for a turn that never
got to release. **llama.cpp: total failure → 173.4 s.**

**`draw_image` handed the model the filename.** `gemma4-12b` answered a finished render with
nothing but `![c0386631e57b0607.png](c0386631e57b0607.png)` — a Markdown image in place of a
reply, beside the real picture. Harmless only because the Chronicle renders Markdown as text; it
is the hole `CLAUDE.md` recorded as written-down-and-not-built, arriving from the other side. The
name is evidence and stays evidence; the sentence the model reads no longer contains it.

**Eleven prompts had been quietly mangled.** The same damage as `PANEL_IS_OPEN`: Rust string
continuations lost in scripted edits, leaving six to twenty spaces jammed inside sentences a model
reads every turn. Nothing asserts on whitespace, so 1113 tests never saw one of them.

**Why llama.cpp is still last, measured and not guessable.** Same GGUF, same card, same engine —
200 tokens took **36.7 s through Epoch's llama.cpp (5.4 tok/s)** and **11.3 s through LM Studio
(17.7 tok/s)**. Both programs say why when asked: `llama-server --list-devices` reports
`Vulkan0: NVIDIA GeForce RTX 4070 SUPER`, and `lms runtime ls` reports
`llama.cpp-win-x86_64-nvidia-cuda12-avx2` selected. **The winget package the deck installs ships
CPU and Vulkan backends and no CUDA one**, and winget has exactly one `llama.cpp` package, so
there is no better command to offer. Epoch cannot fix it — which is precisely why the deck now
says it, in the program's own words, with no number attached because 3× was measured on one card.

**And the deck gained the reading it was missing.** `models` is a shelf and was being printed as
*"holding …"*; `resident` is the card. Both are shown now, plus what llama.cpp will actually
offload to.

**Timings, all through the window, SAY to picture:**

| runtime | before | after |
| --- | --- | --- |
| Ollama | 84.3 s | 96.8 s |
| LM Studio | 203.5 s | **86.9 s** |
| llama.cpp | 213.6 s → *impossible* | **173.4 s** |

Ollama's difference is variance — its reload was always cheap, which is why it was fastest before
the fix and why the fix does nothing for it.

> **What the user sees:** they name a picture, the character opens the panel, they choose Flux and
> a LoRA, and it draws — on whichever brain they picked, and twice as fast on two of the three.

**Observed, recorded, not fixed:** a raw chat-template marker (`<channel|>`) reached a visible
message once, on `gemma4:12b`. One sighting is not a vocabulary, and a guessed list of tokens to
strip would be the invented gauge; it needs measuring across templates first.

---

### 11.21 — The backend belongs to the card, not to Epoch *(done 2026-08-25)*

11.20 ended by naming a difference Epoch could measure and not fix: the deck installs
`ggml.llamacpp` from winget, that package ships no CUDA backend, and an NVIDIA card was running
through Vulkan at a third of its speed. The owner's answer was to fetch the CUDA build, and to
ask the better question — *what about somebody with an AMD card?*

**The CUDA build, measured.** `llama-b10622-bin-win-cuda-13.3-x64` plus its `cudart`, unpacked
into `~/.llama/bin` — which `found_in` already searches **before** the winget package, so Epoch
preferred it with no code change and without writing a byte into somebody else's install
(ADR-0032's rule, arriving unprompted). Same 200 tokens, same card, same model:

| build | warm | tokens/s |
| --- | --- | --- |
| llama.cpp · Vulkan (winget) | 36.7 s | 5.4 |
| LM Studio · CUDA 12 | 11.3 s | 17.7 |
| **llama.cpp · CUDA 13.3** | **8.1 s** | **~24** |

4.5× the Vulkan build, and now the fastest of the three. On a realistic 7,626-token turn the gap
holds: llama.cpp 8.8 s against LM Studio 23.4 s.

**The proposal, and why it cannot be a dropdown.** A per-card `CUDA · Vulkan · Auto` chooser was
the obvious shape, and measuring killed it: **one build carries one GPU backend.** The CUDA
archive contains `ggml-cuda.dll` and no Vulkan; the winget archive contains `ggml-vulkan.dll` and
no CUDA. There is nothing to switch at runtime — the choice is which archive is installed, and
`--device` can only pick between devices the installed build already supports.

The proposal's own example row shows the failure a menu would produce: **`Intel Arc B60: CUDA`**
is not a slow configuration, it is an impossible one. A control offering backends a card cannot
run is the invented gauge arriving as advice, which is worse than the blank it replaced.

**So the rule was widened rather than the control invented.** `slower_than_it_could_be` knew only
"Vulkan on an NVIDIA card"; it would have told a Radeon owner to install CUDA. It now reads the
vendor out of the device string the program printed and names *that vendor's* native path —
NVIDIA → CUDA, AMD → ROCm, Intel → SYCL, and **nothing at all** for a vendor it cannot name,
because Epoch has no archive to point those people at. The seven backend names come from the
release itself (`cpu · cuda · opencl · openvino · rocm · sycl · vulkan`), not from memory.

**And the deck already had the detection half**, shipped in 11.19: `--list-devices` is the
program answering for itself, which is why this needs no new subsystem. It now reads
`CUDA0: NVIDIA GeForce RTX 4070 SUPER (12281 MiB, 11069 MiB free)` and says nothing further —
a card on its native backend has nothing to be told.

**Why Ollama looked worse, answered.** 84.3 s against 96.8 s was never a regression: ComfyUI's
own history shows the *identical* Flux workflow taking anywhere from **34.7 s to 116 s**, which
swamps every difference being compared. Runs now report both halves, and the model half is the
stable one — Ollama 19.7 s and 22.7 s across two runs, totals 54.7 s and 56.7 s.

**A trade-off that turned out to run the wrong way.** With `Run several crew members at once`
switched **on**, everything got slower, renders most of all:

| | crew off | crew on |
| --- | --- | --- |
| Ollama | 56.7 s (render 34 · model 22.7) | 187.7 s (render 117 · model 70.7) |
| LM Studio | 56.7 s (render 34 · model 22.7) | 86.9 s (render 36 · model 50.9) |
| llama.cpp CUDA | 147.1 s (render 42 · model 105.1) | 64.7 s (render 38 · model 26.7) |

Three runtimes each holding a copy of a 7.5 GB model, on a card that also has to fit an 11.9 GB
Flux checkpoint, is 12 GB asked to hold about 22. **Stated honestly and not over-claimed:** this
was measured while moving between three runtimes, and somebody using one may well see the plain
win the setting promises. What the measurement does settle is that the default being Off is the
right default on a card that also draws.

**And the setting itself was lying.** Switched on, entering a World and coming back showed it
**Off** while `settings.toml` held `concurrentCrew = true`. The Launcher seeded its state from a
boot-time snapshot, and the line that re-read the file sat *below* an early return meant for the
expensive provider probes — so any mount that already had a snapshot never asked. A small TOML is
not a probe; it was on the wrong side of a line the file itself draws, and it is now beside the
other cheap local reads.

That is the worst shape a wrong instrument can take. A blank gauge tells you nothing; this one
showed Off, and a user clicking to switch it on switched it **off** believing the opposite — the
correction did the damage. Held by a test that fails with the line back in its old place.

**And the template survey came back clean.** `<channel|>` was seen once and did not reproduce:
three runtimes × two models × two prompts, through `Provider::take_turn` and the same two repairs
`turn.rs` applies, **no marker of any kind** in any answer. The survey is kept
(`what_the_template_leaves_behind.rs`) because that is where a strip list would have to come from,
and it found something else worth knowing on the way: `gpt-oss-20b` fails to load on both
llama.cpp and LM Studio while working on Ollama.

> **What the user sees:** llama.cpp is the fastest brain on the machine instead of the slowest,
> the deck says which processor it is actually using, and the settings toggle says what is true.

---

### 11.22 — Four things found by using it, and one of them on the other machine *(done 2026-08-25)*

Nothing here was found by reading code. The owner opened the World with a dozen Workflows and
five model rows in it, pressed START on the MacBook, and asked why picking Flux does not help you
pick Flux's parts. Every one of the four was reachable only from the product.

**A rail scrolls; the cards in it do not shrink.** Measured through Epoch's own window: the left
rail was 699px tall and its three cards held 227px, 148px and 1150px of content — while being
114px, 83px and 486px. A flex item shrinks by default and a `.frame` does not clip, so every card
was painting its contents out through its own bevel and over the card below: the second crew
member ran under the AGENTS plate, the agent note ran under WORKFLOWS. The column's
`overflow-y: auto` never engaged **because nothing ever overflowed it** — the children gave way
first, which is why this looked like a layout that worked until a second crew member arrived.

`flex: none` is what makes the scroller above it real. And once the cards stopped shrinking,
thirteen models simply pushed the crew off the top of the rail — correct layout, useless
instrument — so a list with no natural length keeps its own bounded scroll and every card's
title stays reachable however much work is open. Six squeezed cards and four overlaps became
zero, measured the same way they were found.

The invariant is asserted against the stylesheet's own text, because there is nowhere else it
exists: jsdom applies no stylesheet, and Vitest replaces every `.css` import with an empty
string — `?raw` included, measured.

**Choosing Flux said only that it "arrives in parts".** Then it offered every text encoder on the
machine, every VAE, and twelve family names, with nothing to tell them apart — after Epoch had
already measured the model as Flux and said nothing about what that means.

**Measuring first killed the obvious fix.** Greying the parts that belong to another family, the
way a LoRA row is greyed, would have greyed nothing: run against the files really on this machine,
`understand` reads the *kind* of every part correctly and the *family* of almost none —
`clip_l.safetensors`, `t5xxl_fp8.safetensors` and `flux-vae-bf16.safetensors` all came back
`unknown`. What Epoch does know is the model's family, so that is what is said: how many encoders
the family is loaded with, what they are, and — when the server publishes it, in the vocabulary of
the loader that many encoders selects — which name in the FAMILY list this model actually is.

Marked, never selected. The user still picks every file (ADR-0033); what stops is being made to
guess. This is the same statement `shapes_for` has always made — SD 1.5 was trained at 512 — from
one measurement, and `None` for a family Epoch has no account of.

**ComfyUI would not start on the MacBook, and the reason was six characters.** The command began
`cd /d "..."` on every platform. `/d` is `cmd.exe`'s, and it is *needed* there: without it,
`cd "D:\…"` from a C: prompt changes the directory on D: and leaves the shell on C:. Terminal.app
hands the identical line to `zsh`, which answered `zsh:cd:1: string not in pwd: /d` and never
reached the `&&`. So pressing START opened a terminal, printed one error and stopped. The same
command with `/d` removed had ComfyUI answering on 8188 in twenty-five seconds — measured over
SSH, on the machine that could not do it.

> **A command is written in a shell, and the shell is not the same everywhere.** Both spellings
> are correct; they are simply not correct for the same reader.

**And a Mac had never been asked what graphics it has.** `Machine::measure` ran `nvidia-smi` and
nothing else, so every Apple machine reported no card, no memory and no verdict — on the platform
where *will this run here?* is least obvious. That was not a machine that cannot be measured. It
was a machine asked the wrong question, which is the inversion `Declared::uses_tools` made one
crate over: **`None` was a property of the question and was read as a property of the world.**

Apple Silicon is asked with `sysctl` and `vm_stat`; an Intel Mac's card is asked of
`system_profiler`, and its free figure stays `None` because there is no cheap way to read it —
`fits` then answers *nobody could tell you* rather than a number nobody measured. On the owner's
M2: **`Apple M2` · 7.1 of 17.2 GB unified memory free**, cross-checked against the `ram_free`
ComfyUI's own torch build reported at the same moment — an independent reading of the same
quantity, agreeing to three digits.

**And the number needed a different sentence.** `Machine` carries `unified`, because a correct
number described wrongly is still a gauge nobody can explain: "larger than free video memory, it
will spill into system memory and run slowly" describes a second pool, and there is not one. On
one pool a model that does not fit does not get slower — it does not load. Every surface that
weighs a model now words it as what it is, and EpochServices' verdict — which used to fall back
to *total* memory on a Mac — compares against what is actually free.

**Two surfaces, one crate, and the second one had stopped compiling.** `epoch-services` still
wrote `uses_tools: bool` and still built `Available` without `resident`, `devices` or `handicap`:
changes landed on the Host in 11.19 and 11.21 and were never carried across. That is the standing
rule failing quietly — *ask whether EpochServices needs it too* — and the cost of finding out on
the day you need to deploy to the other machine rather than the day it broke.

> **What the user sees:** the World's panels stay inside their frames however much is open,
> choosing Flux tells you what Flux needs, ComfyUI starts on the MacBook, and a Mac finally
> appears in Epoch as the machine it is.

---

### 11.23 — A part is not a family, and a scrollbar is not the World *(done 2026-08-25)*

11.22 taught a Flux panel to say what Flux is loaded with. The owner's answer was the right one:
**every model that arrives in parts needs that, not only the one Epoch happens to have a family
for.** And the widget the previous fix left behind turned out to be the ugliest thing on the
screen.

**Reading the parts, and the two files Epoch could not name at all.** Measured against the shelves
on this machine, `understand` was silent about three things it should not have been:

| file | before | after |
| --- | --- | --- |
| `clip_l.safetensors` | a text encoder, family unknown | a text encoder · **CLIP-L** |
| `t5xxl_fp8_e4m3fn.safetensors` | a text encoder, family unknown | a text encoder · **T5-XXL** |
| `qwen_3_4b_fp8_mixed.safetensors` | **something Epoch could not identify** | a text encoder · a language model |
| `z_image_turbo_int8_convrot.safetensors` | **something Epoch could not identify** | **a diffusion model** |

The last two were not cosmetic. `Shelf::for_kind` sends `Unknown` to the **checkpoints** shelf, so
a bare Z-Image installed through Epoch would land where a `CheckpointLoaderSimple` gets put in
front of it — the refusal this module's own header exists to prevent, arriving at install time.
Qwen-Image and HiDream load their encoder the same way `qwen_3_4b` does; it is not a curiosity.

**And the description comes from the width, never the name.** `clip_l.safetensors` is a claim;
`text_model.embeddings.token_embedding.weight` at `[49408, 768]` is a fact, and 768 is an L
whatever the file is called. `shared.weight` at `[32128, 4096]` is a T5-XXL. A decoder-only model
is `model.embed_tokens.weight` at `[151936, 2560]` — a vocabulary in the hundred thousands, which
is what tells it from CLIP's 49408. ADR-0024's rule, one shelf further in.

**Why a part gets a description rather than a verdict.** A part has no family to be greyed by:
`clip_l.safetensors` is byte-for-byte the same file beside Flux and beside SDXL, which is exactly
why 11.22 found that greying by family would have greyed nothing. So the guidance names what the
family wants — *a CLIP-L and a T5-XXL* — and every row says which file is which. The two halves
meet, and Epoch never claims a file belongs to a family it cannot see.

**A model whose family is unreadable still gets the half that is knowable.** `assembly` used to be
`None` for anything that was not Flux or SD 3, so Z-Image got a panel headed *THIS MODEL ARRIVES IN
PARTS* over four empty dropdowns. The count stays `None` — how many encoders an unmeasured family
takes is precisely what is not known — but that a bare diffusion model carries no encoder and no
VAE is not a guess, it is what `Kind::DiffusionModel` *means*. So it says that, names each row, and
points at the page the model came from.

> **Something is still said, because something is still known.** Silence is the honest answer to a
> question nobody measured — not to a question that has two halves and one of them measured fine.

**And the scrollbars went.** 11.22's fix gave the rails something to scroll, so a gold thumb in a
bevelled track with arrow buttons appeared down the right of the World, over the frames. It was a
careful pixel imitation of a scrollbar, which is still a scrollbar — an **immersion leak**, and the
owner named it before anything else.

What says there is more is now a row clipped at the edge of a bounded list: the content itself,
not a widget about it. Nothing is hidden except the widget.

**Which exposed the real defect underneath.** With the scrollbar gone, the Terminal's bottom bevel
was simply cut off — the right rail wanted 191 + 347 + 187 out of 699, and the scrollbar had been
the only thing explaining the missing eight pixels. A fixed `30vh` slice cannot know what the rail
can hold. The cards that *can* give way now do: a list with no natural length shrinks to fit and
grows into what is spare, with a floor where it stops and the rail scrolls instead. Measured in the
rebuilt window: **no rail scrolls, no gutter anywhere, every frame drawn whole, nothing squeezed.**

> **What the user sees:** choosing any model that arrives in parts tells them what it needs and
> what each file on the machine is, and the World's panels have no scrollbars running through them.

---

### 11.24 — Two accounts of one program, and the gauge that follows the sign-in *(done 2026-08-25)*

The owner asked for something small: two Claude Code sign-ins, two Codex sign-ins, each with its
own row in the usage bar. Building it found that Epoch had been treating *a program* and *an
account* as the same thing everywhere, and one of the places that was wrong was a gauge.

**Isolation was measured, not assumed.** `CLAUDE_CONFIG_DIR` and `CODEX_HOME` each separate a
sign-in completely — neither is in `--help`, both were found by asking the program. So an account
is a folder in the vault (`agents/<id>`) handed to the program as its own environment, and two
accounts of one agent are two genuinely separate sessions rather than one session drawn twice.

**Id identifies; kind classifies.** The same line ADR-0023 draws for characters, one subsystem
over: `claude-code-2` is *who*, `claude-code` is *what kind of program*. `AgentStatus` carries
both, and everything that reasons about the program — the gate, the model list, the display name
— keys on `kind_of(id)`. Nothing may take an id apart to work out which program it is; the
surface that did would be the second place deciding, and two places deciding is how they come to
disagree.

**And the allowance follows the sign-in.** The World had two fixed states — `claudePlanUsage` and
`codexPlanUsage` — measured once per *program*. With two Claude accounts that draws the first
account's window twice, on two rows, with no way to tell which one is running out. It is the
worst of the four ways a gauge can lie, because something is genuinely being measured: it is a
real reading of the wrong quantity. Readings are now keyed by account id, a missing key means
*unasked* and a `null` value means *asked and it could not say* — neither is zero.

**The label is the user's, because the measurement does not exist.** Claude Code will say which
email is signed in and that is shown under the row; Codex has no way to be asked at all. So the
account is named by the person adding it — *work*, *personal* — and Epoch invents nothing. A
title guessed from an empty answer is the gauge nobody can explain, in a place where the honest
alternative costs one text field.

**Gemini is refused, in the Engine's own words.** It signs in with an API key that cannot be
asked about, so two rows of it would be two things nobody could tell apart.

**Epoch still never holds the key.** Adding an account makes a folder and starts the program's
own login in the program's own window. There is no password field on this surface — a test
asserts there is not — nothing is read back, and `AgentAccount` has no field a credential could
travel through, which makes it a guarantee the compiler holds rather than a review checklist.

**One test caught a real defect before the product did.** `next_account_id` counted over the
current list, so forgetting an account freed its id and the next one would silently inherit a
character's brain. It now also asks whether the folder is there, because removal deliberately
leaves it — the record of a sign-in outlives Epoch's note about it.

**Measured through the window, not the API:** ADD ACCOUNT on Claude Code and Codex and not on
Gemini; the usage bar reading `CLAUDE CODE · owner@example.com · 5 HOURS 28% · WEEKLY 86%`
beside `CODEX · 5 HOURS 100% · CREDITS 374.74`; and Paladin's crew card reading her *own*
account's allowance.

**One thing was asked and answered honestly rather than built.** Two characters cannot work at
the same time — not because of accounts, but because the World runs one turn at a time by
design, and two single-slot fields depend on it (`waiting`, and the Quest a running agent turn
files evidence against). Concurrency is a real feature with a real cost, and it is written down
as the next question rather than claimed.

> **What the user sees:** ADD ACCOUNT on Claude Code and Codex, a name they choose, and one row
> in the usage bar per sign-in — each reading its own window.

---

### 11.25 — Five things found by using it, and a snapshot that hid a sixth *(done 2026-08-25)*

None of these came from reading the code. The owner opened Epoch, maximised the window, added an
account, drew a picture and watched Task Manager.

**A card sized by the window instead of by the rail it is in.** 11.23 bounded a scrolling list at
`max-height: 32vh`. Maximised at 2560×1369 that is 438px — inside a MODELS card the rail had given
761px, so the list drew its own edge across the panel and left a dead band underneath. CREW got
338px for two crew members by the same arithmetic. The cap was right about *what* (no one card may
eat the rail) and wrong about *which box*: it measured the viewport, and the rail is the box. It is
`max-height: 55%` of the rail now, and a card no longer grows past its own contents — spare height
belongs at the bottom of the column, where it is World. Measured after: CREW and MODELS drawn whole
with no scroll, WORKFLOWS scrolling because twelve of them genuinely is more than half a rail, and
nothing anywhere cut off.

> **A percentage is only honest about the box it resolves against.** `vh` in a column that is not
> the viewport is a measurement of something else, and it will disagree with the layout on exactly
> the machines you do not own.

**ComfyUI never gave anything back, and it was holding both pools.** Measured on this machine with
`ponyDiffusionV6XL`: before a render 18841 MB of system memory and 11069 MB of video memory free;
after it, 10728 and 4369. One `POST /free {unload_models, free_memory}` returned them to 17832 and
10994 — **7.1 GB of RAM and 6.6 GB of VRAM**. So the suspicion that it *was not even using the
card* was wrong in an interesting way: it uses both, and releases neither.

The ask now happens where a picture is finished rather than in either caller, under the same single
setting the text runtimes answer to — somebody who wants models resident wants the one that draws
resident too. And the same discipline 11.21 earned: `/free` answers an empty 200, so the claim
above comes from reading `/system_stats` back. The test draws first, because the first version of
it asserted on a precondition it had not established and failed against a perfectly healthy server.

**A form is not a window.** The Studio Panel was in the right place — inside the Chronicle, because
what it makes is evidence — and was the wrong object: a translucent slab with one hairline, pinned
between the conversation and the composer. It is a `Frame` now, which is what every other window
here is, so a World Pack re-skins it with the rest; and it sits *in* the log rather than under it,
because a thing a character handed you belongs inside the record of them handing it to you.

**A snapshot that hid an account, and then hid it again.** `settings` was moved above the Launcher's
startup guard in 11.21 for exactly this reason and `agents` was left below it. So adding a second
Claude Code account showed nothing in CREW LINKS until a full re-probe, and — worse — an account
that *had* appeared **disappeared** on the way back from a World, because the remount re-seeded from
the boot photograph. A vanishing row is not a cold instrument, it is a wrong one. Reading the
Engine's kept survey is the cheap half of that pair and belongs on the same side of the line the
settings do.

> **The rule was already written down, and it was applied to one field.** When a guard exists to
> skip *expensive* work, everything cheap that sits below it is there by accident.

**And a login that finished without telling anybody.** A sign-in runs in the agent's own window and
its own browser tab; nothing calls back. So both surfaces re-read the agents **when the window
regains focus** — a real cause, not a poll: the user was elsewhere and has come back.

The rest of that report was not a defect and is now said out loud instead: the browser completes the
login through its own callback, the pasteable code is the fallback for a browser that could not
open, and the terminal ending afterwards is what success looks like. Epoch opens the door and never
holds the key — so what it can do about a confusing doorway is describe it.

> **What the user sees:** panels drawn whole at any window size, a graphics card that is free when
> the picture is finished, the picture panel as a window inside the conversation, and an account
> that appears when it is added and is still there when they come back.

---

### 11.26 — GENERATE generates, and a list that could not answer *(done 2026-08-25)*

The Studio Panel worked in every part except the one it exists for. Everything here came from the
owner filling it in and being unable to explain what he was looking at.

**A button called GENERATE that asks somebody else to generate.** It recorded the settings and
handed the prompt to the character, so that the picture would arrive as their reply and land on
the Quest as evidence. The reasoning was right about evidence and wrong about what happens:
measured, `gemma4:12b` handed *"a lighthouse at night"* with the panel already open called
`open_studio` **again** and answered *"El panel está abierto; elige lo que quieras y pulsa
Generar."* ComfyUI never received a graph.

GENERATE draws now, and the evidence half is kept rather than dropped — the Engine files the
picture on the active Quest in the same `Produced` entry a capability's run makes, which the
Chronicle already renders as the picture itself. Measured end to end through the window: one
`got prompt`, one `Prompt executed`, the picture in the conversation, the Quest reading **HAS
EVIDENCE**, and the character idle throughout.

**And then it drew twice.** With the Engine drawing directly, still speaking the prompt made the
model read it as a fresh request and draw the same picture again — one render from the button and
one from the conversation. Nothing is said afterwards now. Nothing is asked of a model, which
costs nothing and cannot misread anything; whoever wants to talk about the picture can, because it
is right there. The Chronicle is re-read once, caused by the press, since there is no turn event
to carry it.

**A dropdown offering a list that could not contain the answer.** For a Flux model the FAMILY
field showed twenty-eight names with `flux2` among them and no `flux` at all — measured on this
machine, `CLIPLoader` publishes no `flux` and `DualCLIPLoader` does. The list was keyed to how
many encoders the user had picked **so far**, so it only became answerable after the step it was
meant to help with, and until then it offered a near-miss. It follows the count Epoch measured the
family as needing.

**And the family is chosen, not merely marked** — a deliberate narrowing of 11.22. That version
marked the measured family and refused to select it, on ADR-0033's rule that the panel is the
user's. The rule is about *files*; an encoder family is a property of the model Epoch already read
out of its tensors, with one right answer. Leaving it blank moved the guessing onto the user
rather than out of the product. Every other family stays exactly as selectable, which is what
keeps it a default rather than a decision.

**The VAE was nearly given the same treatment, and measuring stopped it.** The only VAE whose own
metadata named the family looked like the obvious default — and run against the files here it
chose `z_image_ae.safetensors` for a Flux model, because that file says *Flux.1-AE* while
`flux-vae-bf16.safetensors` — obviously right to anybody reading the name — says nothing at all.
Epoch does not read filenames (ADR-0024), and a property only one of two candidates happens to
carry is not a comparison. Nothing is chosen there.

> **A default is only safe where the measurement is a comparison.** One file answering a question
> the others were never asked is not evidence that it is the answer.

**Sizes people name out loud.** Three ratio buttons were the whole offer, which quietly decided
that nobody wants a wallpaper: HD, Full HD, QHD, 4K, each of them the other way up, the square
sizes, and Custom. The three the family was trained at are **marked** — asking SD 1.5 for 1024
gives two heads and asking Flux for 4K gives an out-of-memory, both true, and neither Epoch's call
to make for somebody who owns the card. A custom size shows the number the sampler will actually
use, because latent space works in eights and rounds silently.

**A slider nobody could read.** The LoRA strength was an unstyled range input — the platform's own
blue control inside a pixel-art window, an immersion leak — and the owner read `1.50` as *none of
it*. Both ends are written under it now: `0 = off · 0.80 = usual · 1.50 = strongest`.

**And one report that was not a defect.** ComfyUI still holding 2 GB after `/free` is the process
itself: measured at **954 MB** freshly started having never loaded a model, 8957 MB after a
render, 2201 MB after the ask. 6.76 GB comes back; what stays is the server plus torch's CUDA
context, and the only way to zero is to stop the server.

> **What the user sees:** GENERATE makes a picture, once, in the conversation — and the panel
> tells them which family their model is instead of asking them to pick it out of twenty-eight.

---

### 11.27 — The encoder is the measurement Epoch can always take *(done 2026-08-25)*

11.26 taught the panel to name the family for Flux and SD 3. The owner's next model was a
Z-Image, and the panel was back to twenty-eight names and no help. Fixing that properly meant
noticing which measurement had been used, and it was the wrong one.

**Epoch reads a checkpoint's family for five families and an encoder for every file.** `Base`
knows SD 1.5, SD 2, SDXL, SD 3 and Flux; Z-Image, Qwen-Image and everything newer come back
`Unknown`. But since 11.23 Epoch reads what an *encoder* is from the width of its embedding table
— CLIP-L, T5-XXL, a language model — and that is available for every part on the shelf.

**And it is the measurement ComfyUI itself keys on.** `comfy/sd.py` picks the text-encoder
implementation from the detected model and consults `clip_type` only to tell a Flux/Klein setup
apart. Verified by drawing rather than by reading the source: the same Z-Image graph with a
Qwen3-4B encoder drew in **12.2s as `stable_diffusion`**, **10.1s as `qwen_image`**, and **failed
as `flux2`**. So the panel now says exactly that, names the one that does not work, and arrives
with `stable_diffusion` chosen.

**After the checkpoint, never before it — and the window refuted the first version in a minute.**
Written with the encoders answering first, a Flux model with one CLIP-L picked so far was told
*"a CLIP-L on its own is how Stable Diffusion is loaded"* and *"Epoch measured this model as
Stable Diffusion."* A false sentence about the stronger measurement, derived from a selection that
was merely unfinished.

> **A partial choice is not evidence about the thing being chosen for.** It is evidence about how
> far somebody has got, and reading it as the first is how a measurement gets overwritten by a
> guess about it.

**A regression from 11.26, and the shape is worth keeping.** That release keyed the family list on
how many encoders the family *takes* rather than how many were picked — which fixed a list that
could not contain the answer and opened a worse hole: the panel offered `flux` from the double
loader's vocabulary while one encoder still built a single loader, and ComfyUI answered `'flux'
not in (list of length 28)` for every configuration the owner tried. The loader is chosen by
`clip.len()`; only the list had moved.

The two agree by construction now: GENERATE waits until the measured count is picked and says so
— *"Flux is loaded with two text encoders, and one is still to pick."* Epoch measured the number;
saying it is information, and a dead button that explains itself beats a refusal after the panel
was filled in perfectly.

> **When two things must agree, derive both from the same measurement.** Moving one of them to a
> better source and leaving the other where it was is a defect that only appears in the
> combination neither side is watching.

**And the bar with both ends on it went onto its own line.** `.rdy__row` is a four-column grid, so
the strength control wrapped into column one — the 8px status dot — and its legend broke into four
lines a few pixels wide. It spans from the file's own column to the end now.

> **What the user sees:** choosing the parts for any model that arrives in parts — not only Flux —
> comes with the family already answered, and a GENERATE that says what it is waiting for instead
> of failing after the fact.

---

### 11.28 — The sentence that would have saved him was the least legible thing on screen *(done 2026-08-25)*

The Studio Panel now draws, and the owner still could not fill it in for Flux. Everything here is
about the difference between *Epoch knows* and *the user knows*.

**A label that read as the opposite of what it meant.** `SECOND ENCODER · Flux uses one` was about
that field and was read as *Flux uses one encoder*. So he chose one, found GENERATE grey with no
explanation he could see, added a second at random, and got a server refusal. It says
`Flux needs 2 in total` now — unambiguous, and it agrees with the sentence above it.

**And the sentence above it was grey.** *"Flux is loaded with two text encoders (a CLIP-L and a
T5-XXL) and its own VAE"* is the one thing that gets somebody through this panel, and it was
`--lx-ink-faint` — the tone for prose you read afterwards. There is a real distinction between a
hint that *describes* a control and one you must read *to use* it, and it had not been drawn:
`cc__hint--read` draws it, in parchment rather than gold, because gold is reserved for a hint that
changes what a control means and cheapening it would leave nothing to say that with.

**The two halves still had to be joined by eye.** The guidance named *a CLIP-L and a T5-XXL*, every
row named which file was which, and somebody had to hold one and match it against the other, once
per field. A family now says what it `wants` in **the same words `Encoder::plainly` produces**, so
the row can be marked by comparison — nothing typed, nothing guessed. Every remaining want is
marked rather than only the first, because either of Flux's two may go in either field, and a want
another field has already taken drops out.

> **Epoch measured it and Epoch said it, and the user still could not act on it.** Two facts in
> two places, joined only in the reader's head, is a measurement that has not arrived. The
> cold-instrument rule has a fifth face: a reading that is correct, present and unusable.

**And an honest answer to a question about the future.** *"If I download Minimax H3 tomorrow, will
it work?"* — no, and the reason is worth writing down rather than discovering later:
[The Picture Chain](docs/Build/The%20Picture%20Chain.md) sets out the four cases. A checkpoint
always works. A model in parts whose encoder Epoch recognises works even when Epoch has never
heard of the model — which is why Z-Image draws today. One with an unreadable encoder still opens
and still draws, with the family unmarked and taken from the model's own page. And a video model
did not, because `asset.rs` delivered a PNG and nothing downstream could display a video — which
is what the panel's cold `VIDEO` tab had been saying since ADR-0033. **Phase 12 closed that on
2026-08-28**: video, sound and meshes all arrive now, and the tab is not cold any more.

Upscalers and ControlNets are named in the same document as what they are: filed correctly,
installed correctly, and consumed by nothing.

> **What the user sees:** the panel tells them which two files to pick, in the dropdown where they
> are picking them, in text they can read.

---

### 11.29 — Epoch confirmed the mistake it had just been handed *(done 2026-08-25)*

The owner opened the panel on a Z-Image, chose `clip_l` because it was the first encoder in the
list, and Epoch told him he was right:

> *A CLIP-L on its own is how Stable Diffusion is loaded. Epoch measured this model as **Stable
> Diffusion** — the files are yours to choose.*

and then marked that same `clip_l` as **one this model needs**. Both sentences are produced by the
fallback 11.27 added, and together they are a circle: he picked a file, Epoch inferred a family
from the pick, and then used the inference to endorse the pick. Every render failed.

**The mark was the worse half.** A guess presented as a guess is survivable; a guess that then
*points at the thing you already did* is a machine agreeing with you. Nothing was invented —
`by_encoder` is a correct rule about a correct measurement — and the answer was still wrong,
because the question it answers is *what does this combination mean* and it was rendered as *what
is this model*.

> **A derivation from the user's own input must never be presented as evidence about the thing
> they were choosing for.** It can only ever tell them what they have already told it.

Two changes, and the first is the real one.

**Epoch reads Z-Image now.** `Base` knew five families and this was the gap. ComfyUI's own
detection (`comfy/model_detection.py`) tests `cap_embedder.1.weight` beside
`noise_refiner.0.attention.k_norm.weight` for the Lumina 2 arrangement, and then separates the two
by the first dimension of that weight — **2304 is Lumina 2, 3840 is Z-Image**. Measured on the
owner's file: `[3840, 2560]`. A rule read out of the program rather than remembered about it, and
a shape rather than a name (ADR-0024).

So the checkpoint answers first, as it should: *"Z-Image is loaded with one text encoder — a
language model, not a CLIP — and its own VAE"*, `qwen_3_4b` marked as the one it needs,
`stable_diffusion` already chosen, and no second-encoder field at all. Measured through the
window: **it drew first try.**

**And plain Lumina 2 stays unmeasured**, sharing every one of those tensors. Epoch has never drawn
with one, and filing it by association with its neighbour is precisely the guess this module
exists to refuse.

**The fallback still exists and now describes only the choice.** Every sentence it produces opens
with *"Epoch could not read which family this model is"*, says what **was picked**, and invites a
change. And its `wants` is always empty: marking a row is a statement about the *model*, and the
only thing entitled to make one is a family read from the model's own tensors.

> **What the user sees:** choosing Z-Image fills in its own parts and draws — and a model Epoch
> cannot read says so, instead of agreeing with whatever was chosen for it.

---

---

## Phase 11½ — Every model, and the shelves nothing consumes *(done 2026-08-28)*

**A is done and B is half done (2026-08-26).** Recipes are built and measured end to end in
the window; upscalers have a consumer; ControlNet is still open, and the reason is that it is a
different size — a reference image has to travel and be confined, `draw_image` has that path and
the panel has none.

**Closed 2026-08-27/28.** ControlNet landed in the panel — a reference row per steering, each
asking what the ControlNet should read. And the half of the sentence about `draw_image` is gone
with the capability: it was deleted on 2026-08-28 (ADR-0030's fourth amendment), so the panel is
not the surface that lacks a path, it is the only surface there is.

Originally: Named here because both halves were discovered by using the product and both are
the owner's stated next want; neither is a defect and neither should be found again from scratch.

**Ground.** Two gaps, and they are different in kind.

### A. A model that arrives in parts, whatever it is

The compatibility question is **already answered and it is not the one that hurts**: Epoch
refuses nothing. The panel composes whatever the user picks and ComfyUI draws it, so every base
image model that ComfyUI can load, Epoch can already draw with. What varies is **how much help
the panel gives**, and today that is: full for six families, partial for anything whose encoder
Epoch recognises, and none for the rest.

Three routes were considered. Only the third scales.

1. **More detection rules.** One per architecture, each needing a real measurement — six families
   in, and every one of them earned. It covers what people actually use and it will never cover
   *all*, because the set is open and grows faster than anybody reads release notes.
2. **A manifest.** There is no standard that says *this diffusion model needs a Qwen3-4B*, and a
   field Epoch invented would be a field nobody fills in. Dead end.
3. **Remember what worked.** ← *built 2026-08-26* (`epoch-models/src/recipes.rs`).

**A recipe is evidence, not a guess.** When somebody draws successfully, Epoch knows the whole
answer: this model file, these encoders, this `type`, this VAE, and *a graph that ran*. That is a
measurement of exactly the thing the panel could not advise on, and it is free — it already
happened. Filed by **hash** against the Generative Library (ADR-0032 already owns identity by
hash, never by filename), the next person to choose that model is offered what worked, marked as
*what drew last time* rather than as a claim about the architecture.

It fits every rule the picture chain already follows: derived, never declared · measured, not
remembered · unknown stays unknown until something is drawn. And it degrades honestly — a model
nobody has drawn with says so, exactly as it does today.

Worth stating so it is not overclaimed: a recipe is evidence that *a* combination ran, not that it
was the best one. The wording has to say that, and a second recipe for the same model is two
things that worked, not a contradiction.

**Both open questions were settled by the owner on 2026-08-25**, before anything was built, and
the reasoning is an amendment to [ADR-0032](docs/ADR/0032-the-generative-library.md). In short:

**A recipe belongs to the machine, never to the character.** ADR-0026's one test decides it —
*does this survive changing the engine?* A recipe does not: it names this machine's files, by
this machine's hashes, against this machine's server. It is `num_gpu`, not `temperature`. It is
also not authored content — a workflow was written by a person, a recipe is what a machine ran —
so the type and its store live in **`epoch-models`** beside `generative.rs`, one store per
installation, keyed by hash.

That crate is not a preference. **ADR-0029 §9 forbids EpochServices from linking `epoch-engine`**,
and both surfaces need this policy, so `epoch-engine` was never available to hold it.

**It does not travel.** The machine that drew keeps the recipe; the machine about to draw reads
it. Carrying one home would be advice about a disk that is not here. EpochServices gets the
contract now and the store the day a lent easel actually draws — nothing on the Host reads
`Have.easels` yet, and a store nothing fills is a cold instrument with no reading behind it.

**A refusal is the same record with its outcome**, not a second store: *drew* or *refused*, and
for a refusal the server's own words. Bounded twice so the record cannot lie — only a refusal
**about the configuration** is kept (an out-of-memory is the card that day, not a fact about the
combination), and it is said as *"this combination was refused"*, never *"this model cannot"*.

### B. Upscalers and ControlNets are inventory nothing consumes *(upscalers done 2026-08-26)*

`Shelf::Upscalers`, `Shelf::ControlNet`, `Kind::Upscaler`, `Kind::ControlNet` all exist. A file of
either kind is identified from its bytes, filed on the right shelf, installed correctly and listed
in the Workshop — and **no composed graph has ever contained one**. There is no upscale step and
no ControlNet input in the Studio Panel, and `draw_image` has no way to ask for either.

ADR-0033 named `Add ControlNet` as a deliberate gap in the first version. This is the note that it
is still open, and that the shelves fill up in the meantime.

For an upscaler the shape is small: a second sampler pass or an `ImageUpscaleWithModel` after the
decode, one control in `ADVANCED`, and the same *marked, never fenced off* rule the sizes already
follow — an upscale is where a 12 GB card runs out, and saying so beats refusing.

For a ControlNet it is larger, because a reference image has to travel and be confined (the panel
has no `from_image` path; `draw_image` does, and refuses on a lent easel).

**Both closed.** The ControlNet rows shipped 2026-08-27 with the reference confined the same way
`see_image` confines one — and the comparison to `draw_image` no longer names anything: it was
deleted 2026-08-28.

> **What the user sees:** any model that arrives in parts remembers how it was drawn with — and
> the two shelves that fill up have somewhere to go.

---

## Phase 12 — Long work, and every medium *(done 2026-08-29)*

**Every medium works.** Measured in the window, Default World: the Studio Panel's MAKE row is
`IMAGE · VIDEO · AUDIO · 3D` and none of the tabs is cold any more.

| tab | measured, in the window | what it leaves in the vault |
|---|---|---|
| IMAGE | 12–35 s, drawn on the calling thread | a `.png` |
| VIDEO | **40.3 s**, LTXV, 49 frames at 512×320 | an `.mp4` |
| AUDIO | **8.0 s** for 90 s of music (ACE-Step); 6.1 s for 10 s (Stable Audio); **50 s** for a minute with lyrics (ACE-Step 1.5, cold) | a `.flac` |
| 3D | **61.8 s** smooth · **359.2 s** fine, Hunyuan3D | a `.glb` **and a turntable beside it** |

The two conditions Phase 12 named have both been met rather than waived. *Something that can
display a mesh* is `polyscope-rs`, headless, turning the model through 24 frames into an animated
GIF that the picture path already carries unchanged — no viewer, no second window, no immersion
leak. And *audio delivery* is `MadeFormat` gaining FLAC, MP3, OGG and WAV, with `media-src`
widened to exactly the origins `img-src` already allowed.

**Ground.** The one piece Epoch has in no form at all. A capability runs *inside* the call today:
the model waits, the turn waits, the window waits. Twelve seconds fits; four minutes does not.

1. ~~**Long work**~~ — **done.** A capability may answer *started*, and so may the panel. The
   machinery landed with ADR-0034 and sat unexercised until video gave it a producer: a picture
   takes 12 to 35 s and is still drawn on the calling thread, which is measured rather than
   assumed.
2. ~~**Absence that is true**~~ — **done**, and corrected on the way: the character is *waiting*,
   not working. ComfyUI is working. `Effort::Waiting` is its own state and the card reads
   `WAITING` — a World that showed them working while a machine worked for them would be claiming
   something it cannot see.
3. ~~**Audio and video delivery**~~ — **done.** `MadeFormat` is what a capability produced and
   `ImageFormat` stays what may be imported: adding MP4 to the second would let a video be a
   World's key art and be handed to a model that can see. `epoch://` serves both, `media-src`
   allows exactly the origins `img-src` already did, and the Chronicle plays one where it would
   show the other. Sound arrives the same way and is played by a transport drawn in the World's
   own palette — the platform's `<audio>` element is a browser widget in a pixel-art window, and
   it drew at `width: 0` besides, being a percentage of a content-sized button. Same gap the
   World's music needs (Phase 14).
4. ~~**`make_video`**~~ — **done, and it is not a capability.** It is the panel's VIDEO tab: one
   thing draws and it is the panel (ADR-0030's fourth amendment), so a capability that made a
   video would be that mistake in a second medium. The panel says how many frames before
   anything starts, and GENERATE waits until the encoder and family a video checkpoint needs are
   chosen — found by drawing one, because LTXV loads like a checkpoint and answers `CLIP = None`.
5. ~~**Audio**~~ — **done.** Three families now, and each of them refuted something the one
   before it settled. Stable Audio caps at 47 s; ACE-Step writes 90 s of music with lyrics in
   8.0 s; **ACE-Step 1.5** is its own family rather than a newer file — its nodes are separate
   classes (`TextEncodeAceStepAudio1.5`, `EmptyAceStep1.5LatentAudio`) and it is told apart by
   ComfyUI's own key, a Qwen3 under `text_encoders.` where v1 carries a lyric embedding.

   The AUDIO tab asks for seconds, and `SaveAudio` reports under ComfyUI's `audio` key rather
   than `images`, which is why `first_image` now tries every key a capability can produce under.

   **And a song is told two things.** The lyrics reached the encoder as an empty string from the
   day sound worked, with a comment saying the panel had nowhere to type them — right about the
   fix belonging to the panel rather than the composer. `LYRICS` is that box, offered by a
   family whose encoder takes one and by no other: Stable Audio is told a description only, and
   a control that reaches nothing is worse than one that is absent. Empty is an instrumental.
6. ~~**3D**~~ — **done, and the condition was met rather than waived.** `polyscope-rs` renders
   headless, so the mesh is turned into 24 frames and kept as a GIF named after the mesh's own
   hash — the Chronicle finds a preview without a second field on the artifact. Both directions
   are offered because both are real: **words → picture → mesh** in one graph (Hunyuan3D has no
   text encoder anywhere in it), and **pictures the user hands over**, up to four named views.
   `SURFACE` is three named looks rather than two dials — smooth, fine, blocky — with the minute
   on the control. It still can never become World artwork (ADR-0030, CONTENT_PHILOSOPHY).

> **What the user sees:** a character says *"this will take a few minutes"* and **leaves** — and
> the World shows them waiting, because that is what they are doing. Then a video plays in the
> conversation, or a sound, or a cow turns on the spot.
>
> **Summary:** the World works in every medium, and time becomes visible.

---

## Phase 12½ — Two at once *(done 2026-08-26, `310eae4`)*

**Named here on 2026-08-29, four days after it was built.** That is the whole lesson of this
phase and it was learnt twice over.

The first half: concurrency had been the agreed next piece of engineering since 2026-08-25 and
existed only in `docs/Build/Handoff.md`, so this file — the one that lists phases — went from
Phase 12 straight to the Universe. Anybody reading the roadmap alone would not have known it was
coming.

**The second half is worse, and it was found by the next session measuring the code instead of
reading the note.** By the time the phase was written down it had already shipped: `310eae4`,
2026-08-26, measured in the window with Mage on `gemma4-12b` through llama.cpp and Paladin on a
second Claude Code sign-in, two Quests, both cards reading WORKING. The session that added this
section had, in the same pass, correctly re-measured two *other* entries that were stale — and
then wrote forty-six lines describing delivered work as upcoming.

> **A list inherited is a list nobody checked, and re-measuring part of it is not re-measuring
> it.** The entry that escaped was the one being *recommended*, which is the entry a reader is
> most likely to act on. Left standing rather than deleted: the phase is real, the reasoning
> below is what was built, and the mistake is worth more here than a clean page would be.

**Ground.** The World ran **one turn at a time by design**, and that was not an oversight — it is
what made evidence safe to file. Two single-slot fields depended on it:

| field | what it holds | what breaks with two |
| --- | --- | --- |
| `Inner::waiting` | the open decision waiting on the user | one approval, two askers |
| `Inner::running` | the Quest a running agent turn files evidence against | **evidence on the wrong Quest** |

Two agent accounts (Phase 11) did *not* give two characters working at once, and that was said
honestly rather than built at the time. This is the phase that built it.

**Why then rather than later.** Four media produce evidence as of Phase 12, and every one of them
inherited the single slot. The cost of this grows with the number of things that can produce a
`Produced` entry, and that number had just gone from one to four.

1. ~~**`waiting` becomes a map keyed by character.**~~ **Done** — `state.rs`. Every reader that
   assumed one became explicit about which. The cheap half, as expected.
2. ~~**`running` becomes a map, and `Working` becomes a per-character guard.**~~ **Done, and it
   was the part that had to be right.** `Working` cleared the whole slot on drop — correct while
   there could only be one, and with two turns in flight the first to finish would have taken the
   other's Quest away, so evidence filed afterwards lands wherever the surface happens to be
   pointing. **Misfiled evidence is worse than missing evidence** (ADR-0025), and a guard that
   exists to prevent a defect must not create it. It remembers whose turn it is and lets go of
   its own. `a_running_turn_names_its_own_quest_and_lets_go_however_it_ends` asserts two at once.
3. ~~**The turn loop refuses a second turn *for the same character*.**~~ **Done**, refused rather
   than queued: two sets of evidence against one Quest would each be half the story, and a queue
   would answer minutes later against a conversation that had moved on.
4. ~~**The World shows two characters working.**~~ **Done, and it was not free.** Presence has
   been per-character since ADR-0018, but the Chronicle's caret named whoever you were *looking
   at* rather than whoever was speaking — with Mage working, clicking Paladin's crew card
   streamed Mage's live tokens under **Paladin's** name. Clicking a crew card changes who you are
   addressing, not who is answering. Tokens are matched by Quest now. A character appearing to
   say something they never said is the one thing a Chronicle may not do.
5. ~~**Memory, said where the setting is.**~~ **Done.** `Run several crew members at once` says
   what it costs on a card that also draws — and says explicitly that two crew members work at
   the same time either way, because what the setting buys is not having to load the model
   again. It never decided turns, and one instrument still said it did until 2026-08-30
   (`ONE TURN AT A TIME` at boot). **An answer that outlives the question it was true about is a
   real reading of the wrong quantity.**

**What was deliberately out of scope, and still is.** Two characters drawing at once: ComfyUI
queues, so a second render waits on the first whatever Epoch does. Saying they both work while
one is queued would be the World claiming something it cannot see — the same correction
`Effort::Waiting` already made once.

> **What the user sees:** Mage and Paladin working at the same time, and the World showing both.
>
> **Summary:** the World stopped being one worker with several faces.

---

## Phase 12¾ — The deck tells the whole truth about a runtime *(done 2026-08-30)*

**Three things the owner asked for on 2026-08-30, after driving the Models and Creations decks
on both machines.** They are one phase because they share a rule: *a deck must say what it can
do, in the words of the thing that can do it.*

---

### A. MEASURE on a runtime the user picks

**Ground.** TIME IT names and offers a runtime (Phase 11, 2026-08-30). MEASURE cannot, and the
button now says so — `MEASURE ON LLAMA.CPP`. That is honest and it is not what was asked for.

**Why it is a phase and not a control.** A curve is eight loads at eight settings, and the
settings are `llama-server`'s own flags. What each backend allows is not the same shape:

| | context per run | KV cache per run | applies a loadout |
|---|---|---|---|
| **llama.cpp** | `-c N` on the child | `--cache-type-k/v q8_0` | **yes** — `presets.ini`, read when the router spawns a child |
| **Ollama** | `options.num_ctx` per request — Epoch already sends it | **no** — `OLLAMA_KV_CACHE_TYPE`, read once when the server starts | not yet — but Epoch **starts** Ollama, so it can set that variable |
| **LM Studio** | `lms load -c N`, read back as `loaded_context_length` | **no, and by nothing** — no flag, and its REST API has no load route at all | no |

#### Step 1 is done *(measured 2026-08-30, `gemma4:12b`, RTX 4070 SUPER 12 GB)*

Every cell above is now the program's own answer rather than somebody's documentation.

**Ollama ignores a per-request cache type, silently.** `options.cache_type_k` / `cache_type_v`
sent with a turn produce **8.39 GB** — byte for byte what the default produces. It is not
refused; it does nothing, which is the worse of the two. The variable does work: the same load
under `OLLAMA_KV_CACHE_TYPE=q8_0` reports **7.84 GB**.

**And at a real context the difference is not memory, it is whether the model is on the card.**

| gemma4:12b on Ollama | f16 (default) | q8_0 |
|---|---|---|
| 16,384 tokens | 8.39 GB, all on the card | 7.84 GB, all on the card |
| 65,536 tokens | 9.11 GB total — **only 3.53 GB on the card**, ~5.6 GB spilled to system memory | **7.95 GB, all of it on the card** |

That is the KV cache cliff, measured on Ollama in Ollama's own numbers. A 12B model at 64k is
either resident or it is running across the PCIe bus, and one environment variable decides which.

**LM Studio cannot be told a cache type by anything Epoch can drive.** `lms load` offers
`--context-length`, `--gpu`, `--parallel`, `--ttl` and speculative-decoding flags, and no cache
option; the REST API has no load route. Verified rather than assumed, because **LM Studio answers
`200` to routes it does not have** with the refusal in the body — `POST /api/v0/load` and a route
invented on the spot (`/api/v0/nonsense-route-xyz`) return the *same* answer, which is the control
that makes the reading mean something.

**What it can be told works and reads back**: `lms load gemma4-12b -c 16384` →
`loaded_context_length: 16384`. And `--estimate-only` answers *before* loading — 7.04 GiB, which
is exactly what it reported after loading. That is a measurement Epoch currently makes by
arithmetic and could ask for instead.

#### And it is not a setting that travels — measured the same day

The owner asked whether any of this changes on AMD, Intel or Apple Silicon. One of the three
settings does, and it fails in the loudest possible way.

**A quantized cache requires flash attention, and without it the model does not load at all.**
Measured by starting Ollama with `OLLAMA_KV_CACHE_TYPE=q8_0 OLLAMA_FLASH_ATTENTION=0`:

```text
llama_init_from_model: quantized V cache requires flash_attn to be enabled
sched.go: Load failed
```

Not slower, not silently f16 — nothing loads. And flash attention is a **backend** feature, not a
model one: solid on CUDA and Metal, uneven on Vulkan, ROCm and SYCL. Ollama's own startup config
shows it ships all of those paths — `OLLAMA_VULKAN`, `HIP_VISIBLE_DEVICES`,
`HSA_OVERRIDE_GFX_VERSION`, `ROCR_VISIBLE_DEVICES` — so a machine where this cannot work is an
ordinary user, not a hypothetical one.

**This kills the environment variable as anything set once and remembered.** On an unsupported
machine, a switch flipped in CONNECTIONS would make every model on Ollama fail to load, with a
message about flash attention that names llama.cpp and not Epoch. It has to be **tried, and the
result read back** — set it, attempt one load, keep it only if the load succeeded, and say what
happened. *Derived, never declared*, applied to a machine's capability rather than a file's.

**Epoch's own llama.cpp curve is already right for this reason**, and is the pattern to copy: a
q8_0 row that cannot load is a row that did not load, and `explore` keeps the ones that did. The
curve measures this per machine without ever being told which vendor it is on — which is the same
answer the CUDA·Vulkan·Auto chooser got (11.20): *a control assembled from what is conceivable
will offer combinations that do not exist.*

**The other two settings do travel.** A context length is an allocation rather than a feature —
`num_ctx`, `-c`, `lms load -c` mean the same thing on every backend.

**Apple Silicon is unmeasured and half of it is already known to be worded wrong.** On unified
memory there is nothing to spill into, so the cliff is not *slow*, it is *does not load* (11.22).
The MacBook did not answer SSH when this was written, so nothing about Metal is claimed here.

**One thing named as unmeasured rather than inferred**: the refusal names the **V** cache
specifically, so a `q8_0` K cache with an `f16` V may well need no flash attention. Epoch's
`Cache` enum offers `F16` and both-quantized and nothing between. Nobody has measured the third
row here, so it is not offered.

#### A backend chooser — and this time the measurement says yes *(2026-08-30)*

The owner's next question was whether the user could pick CUDA · AMD · Intel · Apple when a
runtime starts. That is the control 11.20 killed, so it was measured again rather than remembered
— and **the earlier answer was right about llama.cpp and wrong as a general claim.**

| | GPU backends in one install | how one is chosen |
|---|---|---|
| **Ollama** | `cuda_v12`, `cuda_v13`, `rocm_v7_1`, `vulkan` — **all four**, plus fourteen CPU variants | `OLLAMA_LLM_LIBRARY`, set when the server starts |
| **LM Studio** | six engines installed here, including `vulkan-avx2` beside three CUDA 12 versions | `lms runtime select`, and `lms runtime get` downloads more |
| **llama.cpp** | **one** — `~/.llama/bin` holds `ggml-cuda.dll` and nothing else; the winget package holds `ggml-vulkan.dll` and nothing else | which archive is installed; there is nothing to switch |

**`OLLAMA_LLM_LIBRARY` was verified by using it**, not by reading the help text: started with
`vulkan`, the log says `library=Vulkan` and `using device Vulkan`, and the model answered — on an
NVIDIA card, through Vulkan, on purpose.

**Why this does not break the rule that killed the first proposal.** 11.20 refused a menu
*assembled from what is conceivable* — one that would offer `Intel Arc: CUDA`. This list is
**read from what is installed**: Ollama's own backend directory, `lms runtime ls`. A machine with
one engine gets one row; a machine with six gets six; nobody is ever offered a combination that
does not exist. The rule survives intact and it is what makes the control buildable.

**And llama.cpp keeps the old answer, correctly.** Its row is not a chooser but a sentence: this
build is CUDA, or this build is Vulkan, and changing it means installing the other archive —
which `~/.llama/bin` already makes possible without touching anybody's install (11.20).

**It bears directly on the cache switch below**, which is why it belongs in this phase: flash
attention is a property of the *backend*, so a user who switches Ollama to Vulkan may lose the
quantized cache in the same movement. Two settings that interact must be measured together, and
the proving load below is what does it.

**Worth noting for later**: `lms runtime survey` reports the hardware available to each selected
engine, and `lms load --estimate-only` answers *will this fit* before loading. Both are questions
Epoch currently answers with its own arithmetic.

#### Built, and driven *(2026-08-30)*

`engines.rs` reads the backends; `Preference` is what a runtime is started with; the CONNECTIONS
deck shows all three shapes. Driven in the real window rather than reasoned about:

| runtime | what the deck shows |
|---|---|
| Ollama | `GRAPHICS: Let it choose · CUDA 12 · CUDA 13 · ROCm 7.1 · Vulkan` + the cache switch |
| LM Studio | six engines, each distinguishable, `CUDA 12 · 2.31.2 — in use` |
| llama.cpp | *this build is CUDA — a different one means installing another build* |

**Three defects, and none was reachable from a test.**

**A backend list that named a transport as a card.** The first real reading answered
`CUDA · rpc · rpc-server`: `ggml-rpc` forwards work to another host and ships beside the GPU
backend in every build. A file counts now only where the same function that *names* things
recognises a family in it — what Epoch cannot place is not claimed to be the card.

**Three rows that read alike.** `nvidia-cuda12-avx2@2.31.2`, `@2.29.1` and `@2.28.2` are three
installed engines that all resolve to `CUDA 12`, and a menu of three identical rows is a gauge
that identifies nobody. The engine version is a qualifier now — and a qualifier rather than part
of the family, because `2.31.2` is LM Studio's packaging of llama.cpp and not CUDA's version.

**And two controls on one row lost one of each other.** Setting the backend and the cache in one
gesture wrote only the second: each handler read the whole `Settings`, changed its own field and
wrote it back, so the later one carried a copy taken *before* the earlier one saved and silently
put the old value back. The deck showed the value it had just been told and the file held the
other. `World::change_settings` holds the lock across the change and the write together. **A
surface may read a stale copy to draw with; it may never edit from one.**

**And the expectation that motivated the proving load was wrong here, which is why it was
measured.** Vulkan on this NVIDIA card **does** support flash attention: driven through the deck,
Ollama started with `OLLAMA_LLM_LIBRARY=vulkan` *and* the compressed cache loaded a 17.7 GB model
and answered — `Ollama loaded qwen3.8:latest with the compressed cache`, 35 s after START. So
*flash attention is uneven across backends* stands as a reason to prove rather than declare, and
**which** backends lack it is not something measured here. The failure is real and reproducible
(`OLLAMA_FLASH_ATTENTION=0`); attributing it to a vendor would be the invented gauge again.

The command that reached the terminal, read from the running process:

```text
cmd /k "title Epoch - Ollama&&set "OLLAMA_LLM_LIBRARY=vulkan"&&set "OLLAMA_KV_CACHE_TYPE=q8_0"&&"…\ollama.exe" serve"
```

**Not carried to EpochServices yet**, and it should be: the shared crate compiles there and gains
`engines` for free, but the Mac's deck offers neither control. Metal is exactly the case worth
having it for, and the MacBook did not answer SSH while this was built.

#### What the measurement changed about steps 2 and 3

**Ollama's product is not a loadout.** Epoch already sizes `num_ctx` to *this turn* (11.16), which
is strictly better than a fixed measured value — so a curve on Ollama must not produce a context
to apply. What it can honestly produce is **where the cliff is**: the largest context that still
sits entirely on the card, and whether the cache type moves it. Epoch can act on that by capping
the per-turn window, which is a change to a number it already sends.

**And the one lever Epoch holds is the start.** Epoch starts Ollama from CONNECTIONS, so it can
start it with `OLLAMA_KV_CACHE_TYPE`. That is the same shape as llama.cpp's `presets.ini` — read
when the server starts, said as such, never pretended to be live. **It is a decision and not a
default**: q8_0 costs some quality, llama.cpp's curve *measures* which side of that trade wins,
and inheriting the answer from a different runtime would be measuring the wrong thing.

**LM Studio maps one axis, and applying it needs something Epoch does not do today.** LM Studio
JIT-loads on the first request with its own default context; to hold a chosen context Epoch would
have to `lms load -c N` before use. Four rows on LM Studio and eight on llama.cpp is the honest
curve this phase was written to allow.

**The part that matters more than the measuring.** A curve produces a **loadout**, and a loadout
is only consumed by llama.cpp. Measuring on Ollama today would produce a chosen row that nothing
applies — a control that changes nothing, which is the dead `Manual` mode of ADR-0027 arriving in
a second deck. So the order is fixed:

1. ~~**Measure what Ollama and LM Studio actually accept**, per request and at load.~~
   *(done 2026-08-30 — the table above)*
2. ~~**Teach Epoch to apply what each one can be told.**~~ *(Ollama done 2026-08-30; LM Studio
   deliberately not)* The per-turn window is capped at the largest context measured to load, and
   CONNECTIONS offers starting the server with `OLLAMA_KV_CACHE_TYPE` — read when it starts,
   chosen rather than defaulted, and proved by one real load that switches it back off if the
   load fails. **LM Studio's half is not built and is not a hole**: `lms load -c N` before use is
   a load Epoch does not perform, and until it does, a context chosen for LM Studio is a setting
   nothing reads.
3. ~~**Only then** offer the runtime picker on MEASURE, and only for the settings that backend
   can be told.~~ *(done 2026-08-30)* MEASURE offers `Ollama · context only` and
   `llama.cpp · context and cache`, and each row says how much of the curve it maps. Measured
   through the window: an Ollama curve is six rungs in 90 s and answers
   *gemma4:12b on Ollama: 49152 tokens of context on the full-precision cache, 23.3 tok/s.*

**LM Studio is still absent, for the reason that was always the real one.** A curve there would
end in a setting nothing reads — it takes a context only at load, through its own CLI, and Epoch
does not load through it. That is now a sentence the Engine returns when asked, rather than a rule
written into a button.

**What each curve is applied to, and it is no longer the same thing.** `tell_llama_cpp` ran at the
end of *every* search — correct while llama.cpp was the only thing measurable, and a real defect
the moment it was not: the first Ollama curve was written into llama.cpp's `presets.ini`. A
measurement of one program applied to another is the shape of every wrong gauge in this file.
An Ollama curve now produces a **ceiling** instead: the largest window measured to load here caps
what a turn may size itself to, which is a number Epoch already sends every turn.

**Two things named rather than claimed.** The cap only bites where a model genuinely fails to
load — on this card `gemma4:12b` *loaded* at 65,536 and merely ran at half speed, so nothing is
capped for it. And an Ollama curve is **not browsable afterwards**: the deck's curve panel is
keyed by llama.cpp's build string, so it keeps showing llama.cpp's. The result is stored, applied
and reported when measured; a second panel is work nobody has done yet.

**Four defects found by driving it, none reachable from a test**: the presets write above; a
runtime picker that only appeared on an *unmeasured* row, so a model measured on llama.cpp could
never be measured on Ollama (a control keyed to how far somebody has got is wrong until they have
finished); an answer that said *llama.cpp has been told* about an Ollama curve; and five strings
carrying runs of jammed spaces into what the user reads — the eleventh time that has happened
here, so the whole workspace was swept for it and the four remaining hits are column-aligned test
fixtures.

**What is deliberately out of scope.** Making the three comparable by measuring only what all
three share. The interesting difference between them *is* what they each allow, and flattening
it to the intersection would throw away the reason somebody has three.

> **What the user sees:** MEASURE offers the runtimes that can answer, and each one maps the part
> of the curve it is able to be told.

---

### A½. Identity adapters — asked for, noted, and probably not needed

**Written down on 2026-08-30 because it was promised on 2026-08-29 and was not.** The owner asked
how to draw himself and his girlfriend in low poly and keep their faces; the answer offered two
routes, he said *do the first and note the second*, and only the first was built. Claiming a note
had been taken when no file carried it is the failure this repository already has a rule about:
**being written down somewhere is not being documented**, and a sentence in a conversation is not
somewhere.

**What it is.** IP-Adapter (FaceID) and InstantID condition a render on a face rather than on a
prompt, through a `IPAdapterApply`-style node and a face encoder. The Studio Panel's chain has no
node of that shape, and the Workshop has no shelf for the adapter or the encoder — so it is a
real piece of work: a shelf, an identification rule from bytes (ADR-0032), rows on the panel, and
a place in `compose`.

**And the measurement says it is not the thing to build.** img2img with the photo kept at 0.25
and the low-poly LoRA at 0.80 produced a render both people were recognisable in — measured
through the window at 1280×960, which is the photo's own size and the proof the photo was used.
The route that already exists reached the goal that motivated the route that does not.

So: **not scheduled.** It goes on the list if somebody wants a face in a scene the photo cannot
supply — a different pose, a different composition — which is the case img2img genuinely cannot
answer and an adapter can. That is the trigger to build it, and nothing smaller is.

---

### B. A preview beside a search result *(done 2026-08-30, both surfaces)*

**Ground.** The Creations Workshop lists what Civitai and Hugging Face answered — name, kind,
base, size, downloads — and no picture. Choosing a style LoRA by its name is choosing blind, and
both sources publish exactly the image that would settle it.

**The constraint is the CSP, and it is the interesting part.** `tauri.conf.json` allows
`img-src 'self' data: epoch: http://epoch.localhost` — no remote host. There are two ways and
they are not equal:

- **Widen `img-src` to the two image CDNs.** One line, and it lets the window fetch from Civitai
  directly: every scroll of the results is a request from the user's browser context to a third
  party.
- **Serve it through `epoch://`, as pictures already are** (ADR-0024 §2b). The Engine fetches,
  the window asks for a name, `img-src` is untouched, and the file is cached under a name that is
  its own hash — the same mechanism that took a 182 MB render from 3401 ms to 0 ms on the second
  view.

**The second one.** It is the narrower door, it keeps a guarantee rather than spending it, and it
is the one this codebase already chose when the wider door was on the table.

`Asset` gains the preview URL its source already sends (Civitai: `modelVersions[].images[]`;
Hugging Face: the card's image where there is one). **Never fetched at search time** — a page of
twenty results must not be twenty downloads before anything is drawn; the row asks for its own
preview when it is on screen.

**Two things that must be decided in the open, not implied.** Civitai marks adult content and
Epoch already filters it and says so rather than hiding it — a preview makes that setting visible
in a way a name never did. And a preview is somebody's artwork fetched onto this machine:
`CONTENT_PHILOSOPHY`'s hard rule governs what Epoch *distributes*, and a cached thumbnail is not
distribution — but it is worth writing down that it is cached, where, and that clearing it is
possible.

> **What the user sees:** the search looks like a shelf of things rather than a list of names.

---

### C. Filter the catalogue by medium *(done 2026-08-30, both surfaces)*

**Ground.** The library is filtered by medium since 2026-08-30 — `EVERYTHING · IMAGE · VIDEO ·
AUDIO · 3D`, from `Understood::makes`, on all three surfaces. **The search is not.** So a person
who knows what they want has to read forty names to find the three that make a video.

**And the measurement is weaker here, which has to be said on the control.** A file on a shelf is
read from its own bytes; a search result has not been downloaded and cannot be. What the sources
say instead:

- **Civitai** publishes a `type` (Checkpoint, LORA, VAE…) and a base model name. A base that maps
  to a family Epoch knows answers the medium exactly — `Base::makes` already does it, and LTXV,
  ACE-Step and Hunyuan3D are in that table. Everything else is unplaced.
- **Hugging Face** publishes pipeline tags — `text-to-video`, `text-to-audio`, `text-to-3d` —
  which name the medium directly and are the better source where they exist.

So the filter is **derived from what the source said**, and a result the source did not place
stays visible under every medium, exactly as an unreadable file does on the shelves. That rule
has been paid for once already and must not be relearned here.

**One thing this must not become.** A medium filter that quietly narrows the query to one site
— the `base` filter already does that and says so on the page, because only Civitai publishes a
base model. If a medium can only be asked of one source, the page says which, rather than
returning half an answer that looks like a whole one.

> **What the user sees:** press VIDEO in the Workshop and the results are things that make video.

---

### D. A bar while something is downloading *(done 2026-08-30, both surfaces)*

**Ground.** A download says *"Fetching Illustrious-XL. 6.9 GB — this takes as long as it takes"*
and the button reads `FETCHING…`. Every word of that is true and none of it is a reading: at no
point does the user learn whether six gigabytes are half done or barely started, and *as long as
it takes* is what somebody stares at before deciding the program has hung.

**It needs no new mechanism, which is why it belongs here rather than in a phase of its own.**
`catalogue::fetch` already reads the body in one-megabyte chunks in a loop, and the catalogue
already knows the file's size — bytes-so-far and the total are both in hand and neither is
reported. The shape to copy is the one the loadout search already uses: a
`&dyn Fn(usize, usize)` called as it goes, and a surface that draws a bar from it. `.mdls__step`
is that bar, and it says *"setting 5 of at most 7"* because a four-minute job that only speaks
on completion is one somebody kills at three.

**Two things it must not become.** A percentage where there is no total: a source that sends no
`Content-Length` gives bytes and no fraction, and the honest reading there is *how much has
arrived*, not a bar that guesses at a denominator. And a speed nobody asked for — *2.3 MB/s* is
measurable and it is also what turns a quiet wait into a number somebody watches; the useful one
is how much is left.

> **What the user sees:** six gigabytes arriving, with the amount that has arrived on screen.

---

### Every one of these is two surfaces

**The Host and EpochServices both search catalogues, both install, and both list a library.** A
preview, a medium filter and a download bar that exist on one of them is the gap the owner has
already been billed for twice this week — the runtime picker built here and not there, and
`held_on` living in the Host while EpochServices had no library listing at all.

EpochServices has one constraint the Host does not, and it shapes all three rather than blocking
them: **`script-src: 'none'`**. A preview is an `<img>` and needs no script. A medium filter is a
form, exactly as its search already is. And a progress bar is a server-rendered `<progress>` or a
bevelled div, redrawn by the `<meta http-equiv="refresh">` that page already uses for a download
in flight — HTML older than the problem, which is that file's own phrase for it.

> **Summary:** the deck answers the question somebody actually has, says how well it knows, and
> says it on both machines.

---

## Phase 12⅞ — A benchmark that measures the model *(done 2026-08-31)*

**The owner's brief:** stop trusting benchmarks from the internet. Measure the models on this
machine, with answers that can be checked — reasoning that has a right answer, code whose unit
tests are actually **run**, instruction-following whose constraints are checked automatically,
tool calling scored in five parts — and performance as a warmup plus three runs, reported as the
**median**.

Built in `bench` · `trials` · `ran` · `suite` · `card` · `limit` · `kept`, with BENCHMARK on every
row of MODELS. The Limit phase (how much context one model holds *here*) is built in the Engine
and has no button yet.

### What it measured, and it discriminates

| model | on the card | tok/s | reasoning | coding | following | tools |
|---|---|---|---|---|---|---|
| `gemma-3-270m` Q8 | 0.3 GB | **509.3** | 0% | 0% | 8% | 0% |
| `gemma4:12b` | 7.4 GB | 48.5 | 100% (4/5) | 100% (2/3) | 100% (3/4) | 100% |
| `Qwen3.6-35B-A3B` IQ4_XS | 17.7 GB | 35.4 | 100% | 100% (2/3) | 100% | 100% |

RTX 4070 SUPER 12 GB, llama.cpp, 32K, one sitting each. **The 35B is a MoE with 3B active and it
spills six gigabytes**, which is why 17.7 GB on a 12 GB card costs only a quarter of the speed of a
model that fits.

### The four defects the first real run found, none of them reachable from a test

**`coding 33%` was not a coding score.** `gemma4:12b` thinks before it answers and its thinking is
in `reasoning_content`; asked what day a 45-day task starting on a Tuesday ends on, it spent every
one of 700 tokens reasoning and returned an empty `content` with `finish_reason: "length"`. Four of
fourteen trials came back that way and every one was scored **wrong**. That would have ranked every
thinking model below every model that answers immediately — the cold-instrument rule in a table
cell.

**The budget was not a measurement.** Raised to 4,096 from three runs of each failing trial at a cap
of 8,192: 950 tokens · 1,296–1,316 · 1,600–1,790 · 2,496 twice with one runaway. The first run of
each differs from the two after it, because a cold cache changes the numerics — so a fixed seed and
a temperature of zero do **not** make the length repeatable.

**And no budget fixes it.** The same model thought for 12,953 characters about twenty words on
lighthouses and stopped with nothing. Running out is `Refused` now: *unanswered is not wrong*.

**Then the fix produced a worse gauge than the defect.** With refusals out of the denominator the
card read `coding 100%` having answered one coding trial of three — a real reading of the wrong
quantity, the most convincing kind, because something genuinely is being measured. The share now
carries `(2 of 3)` whenever the denominator moved.

**Nothing satisfies nothing.** `AtMostWords(20)` is true of an empty string and so is
`Forbids("light")`, so a model that said nothing scored `following 100%`.

**A card was filed under the wire name.** Ollama's `gemma4:12b` is llama.cpp's `gemma4-12b`, so
MODELS could not find the result it had just taken.

**And the frontend's copy of the scoring is deleted rather than corrected twice.** It disagreed with
the Engine in public — the row read `follow 0%` beside the Engine's `following 8%` — because a
passing constraint serialises as `{"Ok": null}` and only one of the two implementations knew it.
The Engine scores once; the window renders.

### What the suite cannot yet do, said plainly

**It saturates above about twelve billion parameters.** It separates a 270M from a 12B decisively
and it cannot rank a 12B against a 35B on quality — both answer everything they answer correctly,
and the only things that still separate them are speed and how many trials they managed to finish.
Ranking good models against each other needs harder questions, and that is a decision about what to
ask rather than a defect in the machinery.

### One thing found on the way

**A machine that is switched off cost 88.2 seconds**, every time MODELS opened, with every button
dead and nothing saying why. Half was a sequential survey; half was `bridge.rs` bounding a call and
not its socket — the rule `provider.rs` had written down and nobody had carried one file across.
13.4 s now.

---

## Phase 13⁻ — Speculative decoding, measured *(done 2026-08-31)*

**The owner's question:** `Qwen3.6-35B-A3B` runs at 35–41 t/s here and `gemma4:12b` at 48.5.
The target is 40–50. Can speculative decoding carry the Qwen over it without costing quality —
and stop attributing the gap to *"it is a MoE"* before measuring GPU offload, VRAM, RAM, CPU and
GPU utilisation.

Both halves were measured. Both answers are no, and the second one says why.

### Every configuration this build offers, on the Qwen

RTX 4070 SUPER 12 GB · llama.cpp build 10622 · `Qwen3.6-35B-A3B-UD-IQ4_XS` (17.7 GB) · 32K ·
warm-up plus three runs, median. `fresh` is three different questions; `repeat` is one question
three times.

| | fresh | ×off | repeat ×off | drafted / kept |
|---|---|---|---|---|
| **off** | **41.0 t/s** | 1.00 | 1.00 | — |
| ngram-simple n=1 | 28.3 | 0.69 | 0.73 | *nothing* |
| ngram-simple n=2 | 33.9 | 0.83 | 0.85 | *nothing* |
| ngram-simple n=3 | 27.7 | 0.68 | 0.62 | *nothing* |
| ngram-simple n=5 | 29.3 | 0.72 | 0.68 | *nothing* |
| ngram-simple n=8 | 27.4 | 0.67 | 0.66 | *nothing* |
| ngram-map-k | 22.0 | 0.54 | 0.51 | *nothing* |
| **ngram-map-k4v** | **5.5** | **0.13** | 0.13 | *nothing* |
| **ngram-mod** | 25.8 | 0.63 | **2.35** | 128 / 128 |
| ngram-cache | 27.2 | 0.66 | 0.61 | *nothing* |

**Not one configuration was faster on unseen text.** Every text output was identical to the
baseline's, so nothing here is a quality trade — it is simply slower.

**`ngram-map-k4v` costs 87% of the machine's speed**, which is worth knowing before somebody
picks it off a dropdown because it sounds thorough.

**`ngram-mod` is 2.35× on repeated text and 0.63× on fresh.** That is the whole finding: an ngram
speculator drafts only from text it has already seen. It is not a trick case — an agent looping
over a diff, a Quest re-reading a file, a model quoting its own plan back all live there — and an
average of 2.35 and 0.63 would describe neither.

**And four of the five ngram types drafted nothing at all**, on any prompt, on either model. The
flag reached the child (`--spec-type` is in its argv, read from the process) and was accepted.
Only `ngram-mod` ever fired.

### `draft-mtp` is not available for *this file*, and the sentence after it was too wide

Read from the GGUF's own tensor table: `Qwen3.6-35B-A3B-UD-IQ4_XS` carries **733 tensors and no
MTP head** — no `mtp`, `nextn`, `eh_proj` or `shared_head` anywhere. So `draft-mtp` there is a
configuration that cannot run rather than one that is slow, and the sweep does not offer it.

> **Corrected the same day, by the owner.** This section originally concluded that *"speculative
> decoding cannot rescue Qwen3.6"*. The measurement supports **"ngram speculative decoding does
> not improve `Qwen3.6-35B-A3B-UD-IQ4_XS` on unseen text"** and nothing wider. A property of one
> file was reported as a property of a model.
>
> `unsloth/Qwen3.6-35B-A3B-MTP-GGUF` and `ggml-org/Qwen3.6-35B-A3B-MTP-GGUF` both exist, and the
> second documents `--spec-type draft-mtp --spec-draft-n-max 2` as the recommended way to run it.
> Real MTP is unmeasured here and is the next thing measured, not a thing concluded about.
>
> The shape is worth keeping: **a measurement of an artefact is not a measurement of the thing the
> artefact is a version of.** ADR-0024's rule — read the bytes, never the name — was followed
> correctly and then the *conclusion* was written about the name.

### The build does contain MTP support, verified three ways

Not inferred from a version number. Asked, on 2026-08-31:

1. **`--help` lists it.** `--spec-type` offers `draft-mtp` among eleven values, and
   `--spec-draft-n-max` beside it.
2. **The source is an ancestor.** PR #22673 *"llama + spec: MTP Support"* merged 2026-05-16 as
   `2555826`; this build is commit `3737e4137` of 2026-08-25, and GitHub's compare API answers
   `ahead_by: 1442, behind_by: 0` — the merge is behind us.
3. **The binaries carry it.** `llama-common.dll` holds `draft-mtp` and `spec-draft-n-max`;
   `llama.dll` holds **`qwen35moe` and 116 occurrences of `nextn`** — the loader knows this exact
   architecture and its MTP tensors.

The third check nearly reported the opposite. Searching `llama-server.exe` alone found **zero** of
every string, including `spec-draft-n-max`, which `--help` demonstrably prints. The strings live
in the DLLs. *Check the governor the thing actually answers to* — the same mistake as testing
`epoch://` with `fetch`, and it would have produced a confident wrong answer.

### The three MTP artefacts are three different sets of bytes

Asked of the Hugging Face API rather than assumed from the names:

| file | `unsloth/…-GGUF` | `unsloth/…-MTP-GGUF` | delta |
|---|---|---|---|
| `UD-IQ2_XXS` | 10,756,586,464 | 11,819,399,456 | +1.06 GB |
| `UD-IQ2_M` | 11,522,702,304 | 11,882,969,376 | +0.36 GB |
| `UD-IQ4_XS` | **17,730,509,792** | 18,209,036,576 | +0.48 GB |

Every file differs and every MTP file is larger. `17,730,509,792` is the file on this machine, so
the 41.0 t/s baseline is confirmed to come from the **plain** repo by its size rather than by the
folder it sits in.

`ggml-org/Qwen3.6-35B-A3B-MTP-GGUF` — the repo whose README documents the flags — ships only
`Q8_0` (37.8 GB) and `BF16` (71.1 GB), neither runnable here. So the technique is documented by
one repo and the small files come from the other, and `PROVENANCE.md` beside the download records
which commit each came from.

### Why the Qwen is slower, now that the machine was asked

| while generating | `gemma4:12b` (7.4 GB) | `Qwen3.6-35B-A3B` (17.7 GB) |
|---|---|---|
| GPU utilisation, peak / mean | 97% / **59%** | 60% / **39%** |
| CPU, peak / mean | 26% / **17%** | 73% / **52%** |
| video memory, peak | 11.76 GB | **12.34 GB** (the card is 12.28) |
| system memory, before → peak | 9.1 → 17.2 GB (**+8.1**) | 9.5 → 26.4 GB (**+17.0**) |

**The card is full and seventeen gigabytes are in system memory.** The GPU is idle three fifths of
the time while the CPU does half a core's worth of work per sample — the Qwen is running
substantially on the CPU, across the bus, and 41 t/s is *good* for that only because a 35B MoE
activates 3B parameters per token.

So it is not "a MoE penalty". It is a model that does not fit, and the honest fix is a quant that
does: `UD-IQ2_M` at 11.9 GB or `UD-IQ2_XXS` at 11.8 GB would sit on the card the way `gemma4:12b`
does. Untested here, and named as the next thing to measure rather than as a recommendation.

**What is deliberately not reported: PCIe traffic.** `nvidia-smi` answers `[N/A]` to the
throughput counters on this card, and a column that is empty on the hardware most people have is
an instrument nobody can read. The consequence is measured instead, and it is the pair above.

### The control, and why it has no AUTO

`MTP` is renamed `SPECULATIVE DECODING`: MTP is one of eleven `--spec-type` values and four of
the others need no second file, so a control named after one of them was naming a technique and
offering a feature. The list is read from the Engine, which reads it from `llama-server`.

There is no `AUTO`, and the table above is why: the right answer depends on the workload, so an
`AUTO` would have to pick one of 2.35× and 0.63× to be wrong about. `SWEEP SPECULATION` measures
both and puts back whatever the model had — the measurement is Epoch's and the choice is the
user's (ADR-0033).

---

## Phase 13⁻⁻ — MTP measured, one variable at a time *(done 2026-08-31)*

The owner's sequence, designed so nothing is confounded: **A** plain artefact with speculation
off · **B** MTP artefact with speculation off · **C** MTP with `draft-mtp` at several draft
lengths. Run against `llama-server` directly, one model per server, `-c 32768`, warm-up plus three
runs, median. The plain artefact is measured **first and last**, so drift is visible rather than
invisible.

### What the two artefacts actually are

Compared tensor by tensor, from the headers: **the MTP file is the plain file plus one block.**
733 tensors → 753, all twenty additions `blk.40.*` including `blk.40.nextn.eh_proj`, nothing
removed, and 12 of 14 sampled common tensors byte-identical. Its header carries
`nextn_predict_layers = 1`; the plain one does not carry the key at all.

Not *identical* weights, and the difference is worth naming: `blk.39.ffn_gate_exps` and
`blk.39.ffn_up_exps` are at different quantisation types in the two files, and
`quantize.imatrix.chunks_count` reads 76 against 77. They are two separate quantisation runs of
one model, not one run with a head bolted on — which is exactly why step **B** had to exist.

### The measurements

| | fresh | repeat | drafted / kept | accept | GPU % peak/mean | CPU % | VRAM MiB | RAM MiB |
|---|---|---|---|---|---|---|---|---|
| **A** plain, off | **45.8** | 46.3 | — | — | 42 / 28 | 18 / 13 | 11031 | 24308 |
| **B** MTP, off | **44.1** | 44.1 | — | — | 41 / 28 | 19 / 14 | 11013 | 24402 |
| **C** MTP `n=1` | **47.6** | 47.9 | 257 / 219 | **85%** | 36 / 25 | 12 / 8 | 10887 | 24666 |
| **C** MTP `n=2` | 47.0 | 45.7 | 381 / 284 | 75% | 33 / 24 | 13 / 9 | 10669 | 24629 |
| **C** MTP `n=3` | 46.2 | 46.3 | 474 / 317 | 67% | 34 / 23 | 15 / 11 | 10454 | 24581 |
| **C** MTP `n=4` | 37.5 | 37.5 | 608 / 323 | 53% | 30 / 19 | 17 / 11 | 10218 | 24490 |
| **A′** plain, off | **45.9** | 46.1 | — | — | 44 / 30 | 13 / 10 | 11030 | 24244 |

**A and A′ agree to 0.1 t/s**, so nothing in the sequence drifted.

**MTP works, and it is a real win rather than a large one.** `n=1` is 47.6 against the plain
artefact's 45.8 — **+3.9%** — and against the MTP artefact's *own* baseline of 44.1 it is +7.9%.
Both comparisons are true and they answer different questions; the first is the one somebody
choosing what to run cares about.

**The MTP artefact costs 3.7% to load with speculation off** (44.1 against 45.8). Step B is the
reason that is known rather than folded into the speed-up.

**The sweet spot is `n=1`, and ggml-org's README recommends 2 and 3.** Acceptance falls the
further ahead it guesses — 85% → 75% → 67% → 53% — and past `n=1` the extra work costs more than
the extra tokens save. At `n=4` it is an **18% loss**. *More drafting is not more speed*, measured
rather than assumed.

**And speculation makes the machine work less, not more.** GPU utilisation falls from 42/28 to
36/25 and CPU from 18/13 to 12/8: fewer forward passes for the same tokens, which is what the
technique is.

### ~~The finding that is worth more than the MTP win~~ — withdrawn the same day

**The baseline was 41.0 through Epoch and 45.8 asking `llama-server` directly**, and this section
originally blamed Epoch's own preset:

```ini
ctx-size = 16384
cache-type-k = q8_0
cache-type-v = q8_0
```

The owner asked for the two halves to be measured separately rather than blamed together. Doing
it **withdrew the finding**:

| | tok/s | vs default |
|---|---|---|
| default — 32K, full cache | 46.3 | — |
| context only — 16K, full cache | 47.2 | +2.0% |
| cache only — 32K, `q8_0` | 46.7 | +0.9% |
| **both — the actual stale preset** | **46.9** | **+1.5%** |
| default again — 32K, full cache | 45.6 | −1.4% |

**The bracket spans 1.5% on its own**, and every difference in the table is inside it. Neither
setting costs anything measurable, and the preset Epoch was using is *not slower* than the
default. There was no 10%.

So what was the 41.0? Almost certainly the same contamination as the 18.0: that number came out of
a sweep running in the background while this session was building the frontend. **The 10% was an
artefact of measuring on a busy machine, reported as a fact about Epoch.**

The two settings stay in the search's plan — they are worth *knowing* about, and now they are
known — but they are no longer the headline. What replaces it is the preflight, below.

### And the run before it had to be thrown away too

The first attempt at that table read **31.6 t/s** for the default and **45.9** for the identical
last row. Prompt processing gives it away: 24.1 tok/s against 80.

> **The first model load of a session is measuring the disk.** Eighteen gigabytes come off a cold
> drive once, and everything after hits the operating system's cache. A throwaway load now runs
> first, and the same configuration is measured at both ends — which is the only reason this was a
> visible defect rather than a mysterious result.

### A rule the first attempt paid for

The first run of this sequence read **18.0 t/s** for the plain model. It was taken on a machine
that was also compiling Rust — CPU 88% peak against 18% on the quiet re-run — and the number was
discarded rather than reported.

> **A benchmark on a machine that is also building something is measuring the build.** The
> sequence is bracketed with the same configuration at both ends now, which is what makes that
> visible instead of plausible.

---

## Phase 13⁻⁻⁻ — The cliff is an eviction, not a threshold *(measured 2026-08-31)*

Three runs of one search on one machine gave **48.3, 44.2 and 25.0 tok/s** for the identical
baseline. Two experiments, designed to be causal rather than exhaustive, found what it is — and
refuted both standing hypotheses, including the one this section was written to confirm.

### Experiment 1 — the window is not the cause; *starting* something is

One model, loaded once and left resident for all nine cells. The only variable is Epoch's window:
open, minimised, or not running. Ordered `A B C · C B A · A B C` so drift is bracketed.

| # | condition | tok/s | GPU mean | shared GPU MiB |
|---|---|---|---|---|
| 1 | A open | 45.0 | 32% | 229 |
| 2 | B minimised | 45.0 | 28% | 217 |
| 3 | C no Epoch | 46.1 | 32% | 222 |
| 4 | C no Epoch | 45.8 | 26% | 222 |
| **5** | **B minimised** | **26.2** | **57%** | **295** |
| 6 | A open | 26.9 | 56% | 280 |
| 7 | A open | 27.1 | 56% | 292 |
| 8 | B minimised | 26.3 | 56% | 293 |
| 9 | C no Epoch | 28.3 | 53% | 284 |

**The split is by time, not by condition** — and it never recovers, not even in cell 9 with Epoch
killed. Both fast and slow rows appear under every condition.

What separates cell 4 from cell 5 is that cell 5 **launched** Epoch: cells 3 and 4 had killed it,
and cells 1 and 2 found it already running. So the event is a GPU-using application *arriving*
while the card is full — the driver evicts about 70 MiB of the resident model into shared memory,
and does not migrate it back for the life of that model.

**Throughput halving while GPU utilisation doubles** is the signature: the card is spending its
time moving memory rather than computing.

### Experiment 2 — `-ngl` does not buy headroom

Five levels, eight rounds each, so a collapse had time to appear.

| level | resident MiB | headroom | median | spread | shared GPU MiB |
|---|---|---|---|---|---|
| **all layers (no `-ngl`)** | 11643 | 639 | **45.9** | **0.8** | 223 |
| `-ngl 40` | 11701 | 581 | 12.0 | 0.2 | **7511** |
| `-ngl 38` | 11652 | 630 | 12.2 | 0.3 | 6665 |
| `-ngl 36` | 11687 | 595 | 13.8 | 0.6 | 5888 |
| `-ngl 32` | 11722 | 560 | 16.3 | 0.1 | 4191 |

**Residency does not move.** llama.cpp fills the card at every level; `-ngl` only changes *what*
goes there, and it puts the rest in the GPU's **shared** pool rather than plain host memory —
7.5 GB of it at `-ngl 40`, and four times slower.

The fullest configuration is the fastest **and** the stable one: eight rounds, 45.3 to 46.1.

### What this means for `BENCHMARK & OPTIMIZE`

**There is no headroom threshold to find**, so `VRAM_HEADROOM` as a measured per-machine margin is
not buildable the way it was specified: the runtime refills whatever is freed, and the
least-headroom row is the good one.

What the evidence does support:

1. **Collapse detection.** The signature is cheap and unambiguous — throughput falling while GPU
   utilisation rises, with shared memory growing. A run showing it is discarded and retried after
   a reload, rather than recorded as a profile.
2. **Selection on median and spread, not peak.** `48 t/s once` and `45.9 t/s eight times` are
   different configurations even when the first number is bigger.
3. **A preflight is not enough.** The machine was quiet when it collapsed. What is missing is a
   *postflight*: did this run degrade while it happened.

### And a result that has to be re-taken

The `-ot` row that read 43.0 (−11%) in the first search, and the MTP rows that read −3.7% and
+73% in two others, were all measured without knowing whether their baseline had collapsed. **None
of the search results in this file are trustworthy yet**, and they are kept as what they are: the
measurements that found the instrument was broken.

---

## Phase 13⁻⁻⁻⁻ — The detector works, and it found the diagnosis was wrong *(2026-08-31)*

First search with collapse detection live. Ten configurations, three attempts each, and the
detector refused to record seven of them.

| configuration | median | state | runs | |
|---|---|---|---|---|
| **baseline 32K f16** | **46.7** | stable | 46.0–47.0 ×3 | |
| baseline 32K q8_0 | 27.9 | unstable | 3 collapses, 2 discarded | |
| flash attention on | 28.2 | unstable | 3 collapses | |
| flash attention off | — | **invalid** | never answered | |
| **`-ot` ngl 41** | **44.5** | unstable | 42.0–45.2 ×3, **0 collapses** | 11.4 GB |
| every ngram kind | 24.2–26.8 | unstable | 3 collapses each | |

**It caught what it was built to catch**: seven configurations that would have been recorded as
profiles are now recorded as collapses, and the two numbers that survive are the two worth having.

### And the recovery does not recover

Every step after the first collapsed, was retried twice with a full unload and reload, and
collapsed again. **Reloading the model does not undo the eviction.** So "degraded model instance"
was the wrong name: the state is below the instance, in the driver, and a fresh model loaded into
it inherits it.

### Which reconciles the `-ngl` result

`-ot` is the only row that never collapsed, and it is the only row that is genuinely smaller —
11.4 GB against 12.1. The two knobs are not the same knob:

- **`-ngl N`** leaves the remainder in the GPU's **shared** pool. Residency does not fall,
  7.5 GB lands in host memory *through the driver*, and throughput drops four-fold.
- **`-ot …=CPU`** puts those tensors in **plain host memory**. Residency really falls, and the
  configuration stays out of the spill.

So headroom is real after all, and `-ngl` was simply the wrong instrument for making it. That is
not a reason to reinstate `VRAM_HEADROOM` as a universal criterion — it is a reason to measure
resilience separately, which is the experiment already deferred.

### What is still missing, and it is the same rule as everywhere else

**The baseline was measured first, on a freshly settled machine, and nothing re-measured it at the
end.** Every one of my own scripts brackets its sequence for exactly this reason, and the search
does not — so `46.7 stable` and `44.5 unstable` are not comparable: the first was taken before the
system degraded and the second after.

> A search that establishes its reference once, at the start, is comparing everything after it
> against a machine that no longer exists.

The next thing to build is the bracket: re-run the baseline configuration last, and if it no
longer reproduces, the whole search is invalid rather than the individual rows being wrong.

---

## Phase 13⁻⁻⁻⁻⁻ — The sentinel works, and it says the machine needs a clean GPU *(2026-08-31)*

First search with a control on both sides of every candidate. It behaved exactly as designed and
the answer is not a configuration — it is that this machine has been in the degraded state for
hours and nothing Epoch is willing to do restores it.

```text
SESSION recoveryRequired   usable = false
TRIGGER Flash attention · off

control 28.0 t/s   (opening)
control 27.9 t/s   after Baseline · nothing set
control 27.7 t/s   after KV cache · compressed
control 27.3 t/s   after Flash attention · on
control 26.5 t/s   after Flash attention · off
control 25.5 t/s   after Flash attention · off   (recovery attempt)
```

### What it found

**The opening control was 28.0.** A clean machine gives ~46. So this search never had a healthy
baseline to begin with — the environment was already degraded before it started, and it has stayed
that way across an Epoch restart, many `llama-server` restarts and several hours.

**Within the degraded state everything is consistent**, which is worth knowing: 27.3 · 26.5 · 27.3
for the first three candidates, each `stable` in its own right. The degradation is not noise; it is
a different operating point, and configurations compared inside it agree with each other.

**The drift is slow and downward** — 28.0 to 25.5 across six controls — and the sentinel stopped
when it fell past the tolerance derived from its own opening readings. That is the mechanism
working: it did not need to know that 28 is wrong for this card, only that the control stopped
reproducing itself.

**And `flash attention off` is genuinely broken here**, in every run of every search: no answer at
all, and system memory jumping from 28 GB to 33.7. That one is a real property of this model and
build rather than an artefact of the degraded state.

### Why it stopped rather than continuing

The preflight passed — the machine was quiet, nothing else held the card, 1468 MiB in use and 30
MiB shared with nothing running. **A preflight cannot see this**, which is the whole reason the
postflight and the control exist.

Recovery did what it is allowed to do: ended the instance, waited for the driver to reclaim, and
took the control again. It came back *lower*. So:

> Benchmark paused because the GPU residency state changed. Ending and reloading the model did not
> restore it. A clean GPU state is required to continue.

**No driver reset and no reboot were attempted**, per the standing instruction. `nvidia-smi
--gpu-reset` is not something Epoch performs on somebody's machine to make a benchmark more
convenient, and on a consumer card under WDDM it usually cannot anyway.

Uptime at the time of the run: **2 days**. The most likely clean-state action is a reboot, and that
is the owner's to take.

### What this leaves standing

Firm:

- a clean machine produces **~46 tok/s** for `Qwen3.6-35B-A3B-UD-IQ4_XS` at 32K
- the degradation is **persistent** and survives model and application restarts
- **unload and reload does not revert it**
- **`-ngl` and `-ot` do different things to memory** — one fills the GPU's shared pool, the other
  genuinely reduces dedicated residency
- `flash attention off` does not work for this model on this build

Not conclusions, and to be re-measured on a clean machine: every comparison between q8_0, flash
attention, `-ot` and the ngram kinds. Each was measured at a different position in a sequence whose
state was changing.

---

## Phase 13⁰ — A benchmark that can say no *(done 2026-09-01)*

Everything below was measured by running the product and watching what it did, and every entry
exists because something in it was wrong.

### The golden test

A full `BENCHMARK & OPTIMIZE` on `Qwen3.6-35B-A3B-UD-IQ4_XS` with nobody intervening. One failure
condition, agreed in advance: **did Epoch use evidence it had no right to use?**

Run one **failed**. `Series::mint` refused correctly, `may_begin` answered
`Ok(Verdict::NoReference)`, and the caller checked `if let Err` — so the search carried on and
measured a candidate with no governing reference. Three more defects came out of the same run: the
mint was handed every observation ever taken under that fingerprint rather than the three this run
took; the opening control ran *before* the calibrations that were about to define "now"; and the
llama.cpp consoles a search opened were left standing.

Run two **passed all four and failed on a fifth**. It refused `R-001` as
`TEMPORAL COVERAGE INSUFFICIENT`, calibrated three times, minted `R-002` at 47.52 from exactly
`O-012 O-013 O-014`, ran 10/10 configurations with every sentinel reproducing, 46.8 → 53.4 tok/s.
And then minted `R-003` at 48.61 **from the single opening control** — no sources, no policy, five
runs in one process, newer, so it governed everything after it. Meanwhile the band the eleven
sentinels were judged against came from that same control rather than from the reference the
calibrations had just built: `48.61 ± 1.75` where `R-002` said `47.52 ± 1.43`. The calibrations
decided nothing.

Run three **passed**. Three calibrations read 51.54, 53.59 and 52.46 — and `O-025` spanned
**38.7 %** inside one process, so minting refused. With nothing minted and nothing comparable, the
opening control read 49.8 tok/s and the search **stopped**: no candidates, no profiles, session
recorded as `REFERENCE_REQUIRED`, and the note said why.

> The golden test passed because Epoch correctly refused to continue. No end-to-end run that ends
> happily was required, and forcing one on a machine measuring 38.7 % spread would have been
> exactly the thing the instrument exists to prevent.

### What it fixed

**`Ok` was never a synonym for *may proceed*.** `Verdict::may_compare` is, and only `Clean` is
that. The refusal also records *which* refusal: `RecoveryRequired` is a claim about the machine,
and pointing somebody at a hardware fault because the **record** is empty is the invention this
codebase keeps a separate state for.

**A series is the calibrations one run took.** `Kept::these(&ids)`, not
`observations_of(fingerprint)`. The fingerprint says what kind of experiment; it never says when.

**A control does not become an authority by having run.** `Observation::validate` still refuses
and no longer confers; `Series::mint` is the only thing that mints, over as many fresh processes
as `REPRODUCTION_POLICY_V1` requires.

**The band belongs to the reference.** `session.open` takes the governing tolerance and falls back
to this machine's own control only where nothing governs — and that fallback is a *session* band
that nothing persists.

**One `Current` per experiment**, superseded where a replacement arrives, numbers untouched.

**The authority is a lifecycle, not a vector's last row.** `governing_reference_for` answers *which
reference may decide*; the newest one is a separate question and the right subject for a sentence.
And `Superseded` records **by what**: the same three rows have two right answers depending on
whether `R-003` merely stood aside (then `R-002` still governs) or arrived *as* its replacement
(then retiring `R-003` leaves nothing rather than reinstating what it replaced). Four tests, one
per case.

**A finished search clears a stale pause.** Eleven sentinels reproducing while the deck said the
machine needed a reboot is a gauge contradicted by the product's own measurements from minutes
earlier.

**A finished search must be re-runnable.** The row's primary becomes `QUICK TEST` once profiles
are usable, and `BENCHMARK & OPTIMIZE` was nowhere else — so the only measured model on the
machine was the only one that could not be re-measured. The degraded-record defect, from the
other side.

### The Runtime Candidate Planner

`Accepts` reads the installed build's `--help` and keeps what the program said about each option:
its spellings, its published `allowed values:`, its `(default:)`. The two cache types and
`--flash-attn` had been literals in the source — true of this machine the day they were written,
and not measurements. Two llama.cpp builds sit on this machine and differ.

Three rules it holds to: **a published value, never an inferred one** (`-fa` writes `[on|off|auto]`
in its prose and that is not the `allowed values:` line); **unasked is not empty** (a build
`--help` could not be run against is offered everything Epoch measures); and **being able to run
something is not a reason to recommend it** — `--cache-type-k` publishes `q4_0` here, and a speed
search recommending a 4-bit KV cache would trade quality it cannot measure. `unavailable` names
what this build cannot do, so a shorter search says why it is shorter.

### Still open

- **Temporal stability has no policy.** Three properties, and this machine is excellent at two:
  within-process (1.0–5.2 %), between-process (1.2 % across three in twenty minutes), temporal
  (25 % across six over three hours, and 38.7 % inside one process the next day). What to do about
  the third waits for evidence Epoch has not accumulated.
- The plan is still ten fixed candidates in a fixed order — *which* to run, and when to stop, is
  Phase 13¹.

---

## Phase 13½ — Benchmark & Optimize v1 *(done 2026-09-01)*

The core froze and the product shipped. One press, Epoch works alone, and what comes back is
either a configuration somebody can apply or a refusal that says why.

**The freeze rule, and it is the only one that let anything through:** *can this defect make Epoch
recommend a configuration it has no right to recommend?* Yes → fix. No → backlog. Three said yes.

### What it does

`BENCHMARK & OPTIMIZE` → calibrates if the record needs it → measures 8 candidates with a control
on both sides of each → discards the contaminated → picks one → writes a profile → `APPLY`.

**The strategy is small and ordered by value.** `Flash attention · off` is gone: the baseline
already measures `auto`, which is what somebody who never opens this screen is running, so the
question worth two minutes is whether forcing it *on* beats what the runtime chose. Flash
attention runs before the cache, because the order is what a drift-stopped search keeps. `-ot` is
planned only where llama.cpp's own fit moves something off the card. Ten candidates became eight
and nothing was lost.

**A practical tie goes to the simpler configuration.** 52.93, 53.37 and 53.40 across three ngram
kinds is 0.9% — inside the run-to-run variation of the machine that produced them. Picking the top
of that is picking noise, and what it buys is a flag somebody has to keep working. The band is 1%,
roughly this machine's measured between-process reproducibility; above it speed wins outright.

**LONG CONTEXT is offered only where a context was varied**, so it starts answering the day
Context Scaling measures a ladder and not before.

### The three that could have recommended wrongly

**APPLY put half a configuration into effect.** `Tunings` carries flash attention, speculation and
placement; the context and the KV cache live in `Loadouts`. So applying a profile won by
`KV cache · compressed` told llama.cpp nothing about the cache — and said *"set to BALANCED —
51.0 tok/s at 32K, measured here"* about a machine that was not the one running. Verified by
reading `presets.ini` rather than the code: `ctx-size = 32768` and `spec-type = ngram-map-k4v`
both arrive now.

**The frontend's copy of the resolution had drifted in the direction that recommends.** It
filtered on stability and collapses and knew nothing about brackets or about a degraded session —
`Configuration.bracket` was not even in the IPC type — so the panel could mark ★ a contaminated
row that `use_profile` would refuse to write.

**And `optimize_model` had no concurrency guard.** Two searches on one card interleave their
router restarts, so each one's model load lands inside the other's control and both produce rows
that look measured. A second call also cleared the cancellation flag, so STOP for the first
stopped nothing. `World::searching_for` is claimed before that flag is touched and released by a
`Drop`, so a refusal does not leave the button locked.

### The golden test

Three runs, no intervention, and the criterion was only ever *did Epoch use evidence it had no
right to use*.

Run one **failed, in the panel built to celebrate a result.** The search refused at the gate —
three calibrations, nothing mintable, control 49.8, `REFERENCE_REQUIRED`, paused, nothing written —
and beside that pause the deck drew `OPTIMIZATION COMPLETE · ★ BALANCED · 53.4 tok/s · +14.0% ·
APPLY`. Every number real, none from this run: `finished` held the model's *name* and the panel
read its stored optimization back. **The outcome of a run belongs to that run**; it is carried
now, and a refusal wins over anything stored.

Run two exposed the concurrency defect. Run three **passed**:

```text
NOTHING TO RECOMMEND
The control does not reproduce. 46.8 tok/s against a known clean 49.8 — 6% short,
and more than the 3% this comparison allows (derived from the reference's own runs).
The benchmark stays paused and no search will run.
```

No APPLY, nothing measured, nothing written. The 3% is `R-004`'s own measured band, minted an hour
earlier from three fresh calibrations agreeing to 1.27%.

### What is not claimed

**No single run on the final build went search → recommendation → APPLY**, because this machine
spent the evening drifting: 53.6 at 19:48, 48.0 at 19:51, 46.8 at 20:13, 39.3 on a recovery
attempt. Every stage was verified — 8 candidates planned and bracketed, a degraded session
producing no profiles, `APPLY` reaching `presets.ini` — and not in one sitting. Forcing one would
have meant measuring through the state the instrument exists to refuse.

Backlog, and none of it can produce a wrong recommendation: the calibration decision still reads
the *newest* reference rather than the governing one (it can only err towards measuring more), and
temporal stability still has no policy.

---

## Phase 13 — AMD Build

**Ground.** Everything Epoch knows about a graphics card was measured on **one** NVIDIA machine
and one Apple M2. `engines.rs` reads `rocm_v7_1` out of Ollama's own backend directory here and
has never seen a machine where ROCm is the one that matters; `Machine::measure` runs `nvidia-smi`
and `sysctl` and nothing else; and whether a quantized attention cache works on ROCm or Vulkan is
written down nowhere, because nobody could measure it.

**Asked for by the owner on 2026-08-30**, and it belongs before the release rather than inside it:
a phase that finds out what the product does on hardware nobody built it on is worth more than a
phase that packages it beautifully for the same hardware it grew up on.

**A control derived from what was measured cannot offer a combination that does not exist** — that
is the rule that made the graphics chooser buildable at all (11.20, and its narrowing on
2026-08-30). It is still only ever as honest as the machines it has been run on, and there have
been two.

### 1. The build that travels *(done 2026-08-30)*

`AMDBuild/` holds the installer and the measurement plan. It is **not** the Phase 16 installer —
no detection, no WebView2 check, no uninstaller, no update path — it is the bundle Tauri already
produced, put where a USB stick can reach it.

**Building it found the defect that would have wasted the trip.** `tauri.conf.json` declared no
`resources`, so the bundle carried the application and neither `packs/` (432 KB) nor `assets/`
(4.8 MB): an installed Epoch looks beside its own executable for those, and would have opened on
the AMD machine with **no Worlds and no asset kit**. Somebody would have flown a laptop across a
room to measure an empty World.

Fixed and verified by listing the installer's own contents rather than by trusting the build:
`packs\default`, `packsrchipelago` and `assets\kit` are all in it, beside `epoch-tauri.exe`
where `installed_beside()` looks. The bundles grew 13.1 → 18.5 MB (MSI) and 9.4 → 14.8 MB (NSIS),
which is the 5.2 MB those two directories weigh.

**No vault travels with it.** An installed build keeps the crew, Worlds, Quests and settings in
`%APPDATA%\Epoch` — that is `paths.rs`, not a step somebody has to remember, which is the only
kind of privacy guarantee worth having. The AMD machine starts with no characters, no
conversations, no portrait and no `secrets.dat`.

### 2. What is measured there

The plan is `AMDBuild/README.md`, and it is written as questions this repository currently answers
from one machine:

- **Does it open**, and does the World draw. An empty World means the packs did not arrive.
- **What the machine says it is.** `nvidia-smi` does not exist there. A card reading `0.0 GB` is a
  defect; a card reading *nothing* is the honest answer.
- **Which graphics each runtime offers** — Ollama's own backend directory, llama.cpp's single
  build, `lms runtime ls`. Including where a name comes back **unrecognised**, which is exactly
  the case *shown verbatim rather than guessed at* was written for.
- **The compressed attention cache.** The most valuable reading of the trip: a quantized cache
  needs flash attention, flash attention belongs to the backend, and without it nothing loads at
  all. Epoch proves it with one real load and switches itself back off if that fails — so both
  outcomes are results, and the behaviour is under test as much as the answer.
- **A turn, and then a picture.** ComfyUI on ROCm is its own install. A refusal is a fine outcome;
  a silent failure is not.
- **Anything that reads as software.** Immersion leaks are found by using it, and a machine nobody
  tuned for is where they show up.

### 3. Epoch Setup — the program that prepares a machine *(built 2026-08-30)*

**The owner asked whether the installer installs Epoch's dependencies. Measured: it does not.**
The NSIS carries eighteen files — `epoch-tauri.exe`, the two packs, the asset kit, and NSIS's own
wizard bitmap. Nothing else. Ollama, llama.cpp, LM Studio, ComfyUI and the agents are installed
**from inside Epoch**, through doors that already exist: `winget install …` / `brew install …`
opened in the user's own terminal, and for an agent, its own login in its own window.

That is the right division and it is not what a new machine experiences. A person who has just
run the installer opens a World with nothing behind it and has to *find* five decks to make it
think.

**So: `Epoch_Setup.exe`, a program of its own.** It opens a window, says what the machine already
has, offers what it does not, installs Epoch, and installs whatever was ticked. Epoch's MSI rides
inside the binary, so a person downloads **one file** and runs it.

**It is not a deck in the Launcher, and the first attempt was.** That version put `PREPARE` on the
Launcher's menu and was wrong twice over: it is a second copy of what `CONNECTIONS` already
offers, and it only exists on a machine that already has Epoch — which is the one machine that
does not need it. **The Launcher plays with what it has**; setup runs before there is a Launcher.
The deck was deleted.

What survived the correction is the whole Engine half — `firstrun.rs`, the survey, the plan and
the runner — because that was never about which window drew it.

**Governed by what is already written.** *Offer, never impose*: unchecked boxes with sizes shown,
and declining all of them still produces a working Epoch that says what is missing. *Epoch opens
the door and never holds the key*: it starts an agent's own login and reads nothing back. *A gauge
with nothing behind it must read empty*: what is already installed says so rather than being
offered again.

**One thing only the installer can do, and it is done.** WebView2: without it the window is blank,
so it cannot be fixed from inside a window that will not open. `webviewInstallMode` was Tauri's
default — download the bootstrapper at install time — and is now `embedBootstrapper`, so
`MicrosoftEdgeWebview2Setup.exe` (1.8 MB) rides inside the installer. Verified by listing the
installer's contents. 14.8 → 16.5 MB (NSIS), 18.5 → 20.3 MB (MSI).

**And it belongs in this phase rather than in 15**, because the AMD machine has none of these
programs on it: the trip measures what Epoch says about hardware nobody built it on, and the first
thing it will say is *nothing is installed here*.

**Built**, and driven in its own window: nine rows on the machine it was built on, every one a
real reading with the path it was found at — winget, the three runtimes, ComfyUI, Node.js and the
three agents. Nothing checked, one button reading `Install Epoch`.

**A setup with no Epoch inside it says so and refuses.** `build.rs` writes a placeholder when the
MSI is absent, because `cargo check` on a fresh clone must not fail on a file that only exists
after a release build — and the program then reports an empty payload rather than a successful
install of nothing. An empty gauge beats a compile error nobody can act on.

**Every package id was measured rather than remembered.** `winget show` answered for each of the
five, and `npm ls -g` on this machine reports `@anthropic-ai/claude-code` and `@google/gemini-cli`
— which is where those two names come from. `OpenAI.Codex` is in winget at 0.146.1, so Codex is
offered as its own program, which is how it is actually installed here.

**What something needs is a step of its own.** Asking for Claude Code on a machine with no Node.js
is asking for two installs, and a list that hid one would be a progress bar lying about its own
length. Node.js appears above it, saying whose it is.

**And writing it produced the defect it warns about.** The Gemini row was keyed `"gemini-cli"`,
from the npm package name, while the program calls itself `gemini` — so the screen read *not
installed* about an agent the Launcher was showing as **READY** two panels away. The ids come from
the agents' own constants now, and a test asserts every agent this screen offers is one the Engine
knows. Written in a file whose header says every command was measured, which is where that kind of
mistake goes.

**And it looks like Epoch**, which is not decoration: it is the first thing somebody sees of a
product whose whole argument is that it is a place rather than an application. Same palette, same
pixel face, same bevelled frame as every panel in the Launcher — with the fonts bundled beside the
page (OFL, licences carried), because a desktop program may not reach the network to draw its own
first frame.

Two things it does **not** borrow. The package manager's output stays in a plain monospace rather
than the World's pixel face: Epoch did not say it, and dressing somebody else's words in the
World's voice would be claiming that it did. And the palette is **copied** rather than imported —
this page has no bundler and runs where the application is not installed, so it cannot reach
`launcher.css`; the values are named identically so the two can be compared by eye.

**And the window found the one defect only a window could.** `plugin:event|listen` needs a
`target` key, and without it it throws — at the top level of a module, which takes everything after
it down: the survey ran, returned nine rows, and the screen sat on *Looking at what this machine
already has…* forever. Fixed twice over: the key is passed, and the subscription can no longer stop
the first paint, because a screen that cannot follow the steps should still say what it knows.

**What is Engine-tested and not yet seen:** the installing half. `run` is tested through a real
shell both ways — every line reported as it arrives, and a failure carrying the program's own last
words — and no row of it has been watched on a machine that actually needed something, because
both machines here need nothing. **That is the AMD machine's first job.**

### 4. What comes back

Notes, screenshots, and **the exact words of anything that refused** — a program's own sentence is
the only true account of why it said no, and rewriting it from memory turns a measurement into a
recollection. The findings land in this phase, beside the ones taken here.

> **What the user sees:** Epoch opens on a computer it was not built on, and says true things
> about it.
>
> **Summary:** the second box. Every measurement in this file is honest about the box it was taken
> in, and until now there has been one.

---

## Phase 14 — The Universe

**Ground.** Rescoped by the owner 2026-08-21 and mostly already built:
[Official Epoch Universe](docs/Milestones/Official%20Epoch%20Universe.md). The world name is the
pack's name, lore is Missions, cities are buildings, the logo is the mark EpochServices already
ships; factions and symbols are discarded; the cast left with ADR-0023.

Two doors, then artwork.

1. **World Edit sets the skin** — the `[[ui]]` concept pipeline exists and is used; what is
   missing is any way to reach it that is not hand-edited TOML.
2. **World Edit gives a World sound** — music requested as a **mood**, never a track. A pack
   supplying a sound **overrides a synthesised voice** rather than filling an empty slot, because
   `sfx.ts` deliberately ships no files.
3. **The artwork** — drawing, composing, writing. Not engineering, not in this repository, and
   governed by the hard rule: original, commissioned or CC0. Never generated.

> **What the user sees:** a World that looks and sounds like itself.
>
> **Summary:** creating the official universe replaces a World Pack and nothing else.

---

## Phase 15 — The crew has a voice, and so do you

**Asked for by the owner 2026-09-03**, sitting between the Universe and Distribution. Computer use
was proposed with it and **cancelled by the owner in the same conversation** — it is recorded here
because a phase that quietly loses a piece looks, later, like a piece nobody thought of.

**Ground.** Every character in Epoch is read. A World that has learned to draw, film, compose and
model still communicates in exactly one way, and the one thing a living world is supposed to do —
say something out loud — is the thing it cannot do.

### What the repositories actually are *(cloned and read 2026-09-03)*

Second-hand reading has cost this project once. Both were cloned.

**`ggml-org/whisper.cpp` — MIT, and a better fit than the README suggests.** It ships
`whisper-server`: HTTP on loopback, `POST /inference`, audio in and text out. **That is the shape
of llama.cpp's router**, which Epoch already starts, probes, reports in CONNECTIONS and says is
cold. It also ships **its own VAD** (`--vad`, Silero: threshold, minimum speech, minimum silence
to split a segment) — which is the turn-detection half of real-time conversation, included rather
than bolted on. GGML backends, so the CUDA/Vulkan story measured for llama.cpp applies unchanged.
Models are 75 MiB (tiny) to 2.9 GiB (large).

**`pipecat-ai/pipecat` — not a TTS engine, and not adopted.** It is a Python framework that
*orchestrates* real-time voice pipelines across some sixty services; its two local entries
(`piper`, `kokoro`) are thirty-line wrappers around other packages. Three reasons it does not fit,
in order of weight: it is **a second orchestrator, and the loop it wants to own is Epoch's
product** (the Runtime, the Composer, Quests, Trust, the Chronicle); it is **Python, in an
offline-first desktop application that has to be installable by a stranger** (Phase 16); and its
transports are WebRTC and WebSocket for **remote** clients, while Epoch's microphone is in the
same window as the World. What is worth taking from it is its interruption and turn-taking design,
as a reference.

### The engine that speaks

Two candidates, both measured from their own repositories rather than from a summary of them.

| | Kokoro-82M | Piper |
|---|---|---|
| licence | Apache-2.0 | GPL-3 |
| download | 327 MB, once | ~20-60 MB per voice |
| voices | 54, across 9 languages | hundreds, 40+ languages |
| Spanish | 1F · 2M | `es_ES` · `es_MX` · `es_AR` |
| custom voice | no | yes, trainable |
| without Python | via ONNX (unmeasured) | **yes, measured** — a standalone `piper.exe`, 22.5 MB, 377 ms for 5 s of Spanish |
| **file format** | **`.pt` — a Python pickle** | **`.onnx` + an `.onnx.json` sidecar** |

**Piper first, and the last row is why.** `asset.rs` refuses `.ckpt`, `.pt` and `.bin` by design —
reading a pickle means running it — so a Kokoro shelf would be a shelf of files Epoch is forbidden
to read. Piper's sidecar is plain JSON stating language, speaker count and sample rate, which
Epoch reads with **no new format support at all**. Its voices install one at a time from one
Hugging Face repository, which is exactly what the Workshop already does; Kokoro is 327 MB or
nothing. And it is the one that answers *can I use my own voice?*

**The licence question dissolves on inspection.** GPL-3 only bites if Epoch **bundles** Piper's
binary. It does not have to: ComfyUI and llama.cpp are installed by the user from the deck, and
ADR-0032 already says Epoch *adds a search path and never moves a file somebody already had*. The
voices come from Hugging Face, which is the user's business. **whisper.cpp (MIT) is the one that
goes in the installer** — Phase 16, step 2, offered and never imposed.

Kokoro follows as the second engine — *one download and you have 54 voices* is a real argument —
now that it is known its shelf would be opaque.

### The shape, decided with the owner 2026-09-03

1. **Voices Workshop** — **built 2026-09-04, and it cost what this said it would.** A shelf, not
   a sibling: one canonical `Asset`, one `Catalogue`, one `fetch`, one install path, rendered
   again. `Kind::Voice`, `Shelf::Voices`, and a reader that asks Hugging Face a different
   question.

   Downloads to `%APPDATA%\Epoch\library\generative\voices\`. `Shelf::comfy_key()` already
   returned `Option`, so this shelf answers `None` and ComfyUI is never told a folder it cannot
   read exists. **Not `vault/models`**, which is where the text GGUFs live: a voice filed there
   would appear in the dropdown that picks a character's brain.

   **Measured in the window**, not through the API: the shelf paints 24 voices in **1.5 s**, a
   search narrows to one, and INSTALL finishes in **5.1 s** — landing `es_ES-davefx-medium.onnx`
   (63,201,294 bytes) with its sidecar (4,817) and reporting *Español (es_ES) · medium ·
   22050 Hz*. That sentence is read from the sidecar that arrived, never from the name. The voice
   then spoke, from the library, in **413 ms**.

   Five things the build decided that this section had not:

   **A repository qualifies from its files, and the tree is paginated.** There is no Hugging Face
   tag for a Piper voice — measured, `rhasspy/piper-voices` declares only `onnx` and
   `license:mit` — so the rule is the pairing: an `.onnx` with an `.onnx.json` beside it. Asked
   of the real repository it finds **175 voices, in 2.05 s over four pages**. A first-page reader
   finds **28** and looks like it worked, so the network test asserts a number only a paged
   reader can reach.

   **A voice is two files and the second is not optional.** `principal()` takes the largest, which
   for a voice is the model alone — and Piper reads the sidecar for its phoneme table, so an
   `.onnx` by itself is a download that cannot speak. `Asset::parts()` answers *what has to land*,
   on the `Asset` rather than in the installer: the installer branching on `Kind` would be the
   second place deciding, and two places deciding is how they disagree when a third kind arrives.

   **`source` and `catalogue` are two fields now.** CREATIONS and VOICES both read Hugging Face
   and ask it different things — *what repositories exist* against *what is inside one* — and both
   truthfully answer `Hugging Face` to *who published this*. Routing an install on that would send
   a voice to the reader that cannot find one. **One identifies, the other classifies**, exactly as
   an agent account's id and kind do, and nothing takes the first apart to recover the second.

   **A shelf must not call a voice unreadable.** `understand` reads a picture model's tensors to
   place it; an `.onnx` carries none of what it looks for, so every voice would have shown as
   *unreadable · 0 bytes* — a wrong instrument about a file that is perfectly fine and that Epoch
   can describe exactly. The sidecar is the measurement, and it is already on disk.

   **And the same words existed twice.** `weigh` was written in `FindAssets` and again in
   `CreationsPanel`, with the same doc comment and **different behaviour at zero**: one said *size
   not stated*, the other *1 KB*. Neither was wrong alone. They are one module now, and the zero
   is in the name of the function rather than in a flag: a **measured** zero is an empty file, a
   **claimed** zero is a source that published no size.

   The screen is its own, and that is the one place this is not merely another shelf. A
   base-model filter, an adult filter, a medium filter, a download count and a preview picture are
   five controls a voice has no answer to, and a row that answers nothing on five controls teaches
   you to stop reading them. **Downloads is not reported at all**: Hugging Face counts
   repositories, and printing one repository's number on each of its 175 rows would be a real
   reading of the wrong quantity.

   Still open here: the **sample**, which is step 2 — the shelf describes and does not yet play.

1½. **The mouth** — **built 2026-09-04.** A third list beside the runtimes and the studios, on
   the Creations deck, and a third list rather than a row on either of them for the reason those
   two are separate from each other: everything on the runtimes deck becomes a brain a character
   can be assigned to, and a text-to-speech binary is not one.

   **Two facts, not three.** The other decks draw *installed · serving · neither*. Piper has no
   server and no port — it is handed a sentence and writes a `.wav` — so there is no `SERVING`
   lamp, because a lamp that can never move is worse than no lamp. The row **says that**, rather
   than leaving somebody who reads the neighbouring decks hunting for the state it is missing.

   **And it is the one program Epoch downloads itself.** Measured, 2026-09-04:

   | asked | answered |
   |---|---|
   | `winget search piper` | `npiperelay`, `PhotoPiper`. Nothing. |
   | `formulae.brew.sh` — `piper`, `piper-tts`, `rhasspy-piper` | **404** as formula and as cask |

   Every other program on these decks has a package manager command Epoch prints and runs in the
   user's own terminal. This one has none on either platform, so the row either fetches the
   release archive or it is a link and an instruction — a row that does nothing. It fetches, into
   **Epoch's own folder**, which is what ADR-0032 permits: nothing is written into somebody
   else's tree, and here there is no other tree to write into.

   Its size and its licence are on screen **before** the press, because a 22.5 MB fetch under
   GPL-3 is not a thing to discover afterwards. Epoch's own copy is preferred over one on the
   PATH, and the row says which answered — not decoration: **Epoch may delete what it fetched and
   may not delete what somebody put there themselves.**

   Measured in the window rather than through the Engine: the row reads `NOT HERE`, INSTALL
   finishes in **4.0 s**, and afterwards `%APPDATA%\Epoch	ools\piper\` holds `piper.exe` and
   its DLLs, 38 MB, with the archive deleted. The Engine's own live test goes further and ends at
   a `.wav` with sound in it — **not at an exit code**, because a success code is a claim about a
   process and this project has been caught by that shape before.

2. **Listen before installing** — **built 2026-09-04.** `LISTEN` on every row of the Voices
   Workshop. A voice is the one asset whose description is worthless: *es_ES-davefx-medium,
   medium, 22050 Hz* says nothing about how somebody sounds, and 63 MB is a lot to download to
   find out.

   The recording is **derived from the tree**, never assumed — `rhasspy/piper-voices` publishes a
   `samples/speaker_0.mp3` beside each voice, 191 KB measured, and a community repository may
   not. The button is **absent** where there is nothing behind it rather than present and dead.

   Its own field on the asset and not `preview`, which is a *picture*: an mp3 in a field every
   surface draws as an `<img>` would be a real reading of the wrong quantity. It plays at the
   crew's own volume, because this **is** the crew.

4. **Two switches** — **built 2026-09-04.** `NPC VOICES ON/OFF` in the HUD, beside `SOUND`.

   In the World rather than in Settings because it answers *do I want to hear anyone **right
   now***: somebody walks into the room, a call starts. Sending them to a settings screen to shut
   a character up is how people mute everything and never turn it back on. Checked at the moment
   before somebody would speak — a caller that read it earlier would act on a decision the user
   has since changed — and switching back on lets the *next* answer be heard rather than
   replaying what arrived while nobody was listening.

   **Absent is on.** A crew that arrives silent on a fresh machine looks broken, and somebody who
   has never seen the switch has not asked for silence.

2. ~~**Listen before installing.**~~ A voice is the one asset whose description is worthless —
   *"af_bella, grade B, 10-100 hours of training data"* says nothing about how it sounds. Both
   catalogues publish samples. So this shelf does not describe; it plays. The cold-instrument rule
   turned towards something better than a reading nobody can act on.

3. **The character owns the voice** (ADR-0026's one test: does it survive changing the engine?).
   It is a canonical Character field, travels in the Character Pack into every World, and the
   control lists **voices, never files** — one Kokoro model holds 54 of them, and a file list
   would show one entry called `kokoro-v1_0.pth`. The Style lesson, one medium over: *a character
   asks for a mood, never a filename.*

   ```
   Voice          ● ON
   Voice model    es_ES-carlfm  ▾
   ```

   With nothing installed the control keeps its frame, loses its light and names the Voices
   Workshop — never an empty dropdown, which reads as *you have none*.

4. **Two switches, because there are two questions** — the owner's correction, and it is the one
   that keeps this usable:

   | question | where it lives | what it decides |
   |---|---|---|
   | how does this person sound? | Character | identity; travels with them |
   | do I want to hear anyone right now? | HUD · **NPC VOICES** | the moment; everyone at once |

   **A separate toggle from `SOUND`, which is the interface's own voice** — hover, click, open,
   close, synthesised in `sfx.ts`. One control deciding both would mean muting your own clicks to
   silence Mage, and the first time somebody mutes a character at 3am they mute them forever
   without remembering why. *A setting whose name names one thing must not decide two.*

4½. **The audio block in Settings** — **built 2026-09-04**, at the owner's request:
   `Interface volume` (already there) · **`NPC voices`** · **`Input`** · **`Output`**.

   Two volumes because there are two questions, which is step 4's rule arriving one control
   earlier: muting your own clicks must not silence Mage.

   **The dropdowns were nearly built blind.** Measured in Epoch's own window before writing them:
   an unasked browser answers `enumerateDevices()` with **three devices whose labels and ids are
   empty strings**. A select built on that renders blank rows — *you have no speakers* rather than
   *nobody has been asked*. So `asked` is its own state, and until the browser has been allowed
   once the panel says which of the two it is and offers the button that asks. **Somebody presses
   it**: the prompt is about their microphone.

   After allowing, the same call answers **nine devices, eight labelled**. Windows publishes each
   one up to three times — `Default - Speakers (Razer…)`, `Communications - Speakers (Razer…)` and
   `Speakers (Razer…)` are one pair of headphones — so the aliases are collapsed **by their
   reserved ids**, never by matching words in a label. `default` survives, renamed *Follow the
   system*, because that is a real choice and it keeps working when the hardware changes.

   And the test caught the defect the file was written to prevent: the unasked entry has an empty
   id, which sailed past the alias filter and became a blank row anyway. **An entry with no name
   and no id is not a choice.**

   `WEBVIEW2`'s permission prompt is Edge's own — *"http://tauri.localhost wants to use your
   microphones"* — and it appears over the World, naming a URL. **An immersion leak, recorded
   rather than fixed**: WebView2 lets the host handle `PermissionRequested`, so Epoch could ask in
   its own words and then grant. Engine work, and it belongs with step 5.

5. **The microphone** — **built 2026-09-04.** `SPEAK` in the composer, beside `ATTACH`.

   **It writes into the draft and never sends.** What somebody said out loud is a first version —
   a name misheard, a word the room swallowed — and a press that fires a turn is a press nobody
   can take back. Appended rather than replacing, because somebody may have typed half a sentence
   and then said the rest.

   **No server, and that is measured rather than assumed.** The plan said start `whisper-server`
   headless; `whisper-cli` **including the model load** answers a breath in about a second, and
   the cost is per 30-second window rather than per second of speech — 6 s and 20 s cost the
   same. A server would buy back the load and cost a process, a port, a lifecycle and a console
   window. Nothing running is simpler and the measurement says it is fast enough.

   **Everything on the CPU.** 8.4 MB of binary and 148 MB of model; the card never enters the
   question, so pressing the microphone never competes with a character thinking or a picture
   drawing. Installed from the deck in **2.0 s** and **28.1 s**.

   **The page makes the WAV**, and that is not the frontend doing the Engine's work. whisper wants
   16 kHz mono; the Engine would need `ffmpeg` — a program Epoch does not ship and has no package
   to ask for on two platforms — and the page already has `decodeAudioData` and
   `OfflineAudioContext`. Bytes cross as base64 exactly as an imported picture does, and the
   recording is written once, read once and **thrown away**: a recording is a means, not a record.

   **And nobody was ever asked what language they speak** *(found 2026-09-06, by the owner
   using it)*. `whisper-cli --help` states its own default — `-l LANG [en]` — and Epoch passed
   the flag only when a caller named a language, which the composer never does. So *say nothing*
   was not *detect it*, it was *English*. Measured on his Spanish, through the crew prompt Epoch
   actually sends: `Hello, I'm Mage, welcome to the World of Poets.` against `Hola, soy Mage,
   bienvenido al mundo de Potso.` **A translation, confidently, into a language he was not
   speaking.** It is invisible without the prompt — the same clip with no `--prompt` stayed in
   Spanish either way — and the live test asked for `Some("es")`, which is why 1121 tests never
   saw it: *a harness that supplies the missing argument cannot find a missing argument.*

   **And a stage direction is not a message.** Pressed in a silent room, whisper answered
   `[Pause]` and it landed in the composer as though somebody had typed it. Non-speech markers —
   `[Pause]`, `[BLANK_AUDIO]`, `(música)` — are whisper describing the recording, the same shape
   as the tool call that once reached a model as the sentence `[I called see_image(...)]`. They
   are dropped, and a recording with nothing in it says **Nothing was heard**.

5. ~~**The microphone**~~ — a button in the Chronicle. Pressing it starts `whisper-server`
   **headless**: `quiet::Quiet` is the machinery that already exists for this, and a console
   window nobody asked for is the immersion leak ComfyUI is still remembered for.

5¾. **The permission prompt Epoch does not ask** — **built 2026-09-06.** Recorded at 4½ as
   *"an immersion leak, recorded rather than fixed"*, and it cost a real session first. The owner pressed `SPEAK`,
   nothing happened, and he reported it as a defect: *"Era que salia el pop up de ALLOW en el
   microfono y no lo vi."* Edge's own prompt — **"http://tauri.localhost wants to use your
   microphones"** — appeared over the World, naming a URL, and was missed.

   Nothing was broken. That is what makes it worth doing: the control was correct, the Engine
   was correct, and the product still looked dead. WebView2 lets the host handle
   `PermissionRequested`, so Epoch can ask in its own words, in the World's own frame, and then
   grant. Engine work, small, and *fix an immersion leak before adding the next feature*.

   The button should also say what it is waiting for while the prompt is up — a control whose
   only feedback is a label must not have a state in which the label does not move.

5⅞. **The 86-minute answer** — **closed 2026-09-06**, and the owner was right to say it could
   be ignored: it was not one defect, and two of the three that could produce it are now fixed.
   `stopSpeaking` wiped the memory of what had been said, so the Chronicle's every render offered
   the whole transcript back as new; and `alreadyHeard` returns at once when there is no voice,
   which is the state while the crew is still arriving — so the backlog was marked as heard by a
   pass that marked nothing. Kept here rather than deleted, with what was measured at the time:

   **The reading, for the record.** One `try_voice` call produced
   **227,666,492 bytes · 5162.5 s** of continuous speech at full level. That voice does 18.2
   characters a second by Piper's own log, so about **94,000 characters**; the Chronicle on disk
   holds nothing longer than **78**; Piper truncates its output file, measured three times; and
   that turn produced exactly **one** `.wav`. Those cannot all be about the same string, and
   which string reached Piper has not been read yet.

6. **Real-time conversation** — **built 2026-09-06**, and the cost named here turned out to be
   already paid.

   This section said the real cost was **cancelling a turn in flight**, since the World runs one
   turn at a time by design. Re-measured before a line was written: `stop_turn` exists, the
   Chronicle already draws a halted turn, and `useTurn` already exposes `halt`. So the expensive
   half was built, and what was actually missing is the cheap one — **knowing when somebody has
   finished a sentence.** *Re-measure the entry you are about to act on before the ones you are
   only mentioning*, and this is the second time in a week that rule has paid.

   **The noise floor is measured, never declared.** A fixed threshold is a number that works in
   the room it was written in. `converse.ts` listens to the room for half a second — the loudest
   moment, not the average, or a floor taken from the quiet gaps calls the fan speech — and takes
   speech to be 3.5× that. `THE ROOM…` is its own state on the button, because a control whose
   only feedback is a label must not have a moment where the label is lying.

   `AnalyserNode` rather than Silero: another download, another format and another thing to keep
   in step, to answer *is this louder than the room*. whisper.cpp ships `--vad` and that is where
   this goes next — the day somebody's own recording is the measurement that says energy is not
   enough.

   **And then it looped, and barge-in was the price** *(found by the owner using it, same day)*.
   The first version listened while a character was speaking, on exactly the reasoning above.
   Measured in his room: the microphone heard the speakers, Mage's own answer came back in as a
   new question, was answered, and came back again — one paragraph, forever.

   A room with speakers in it is a room where the microphone hears the speakers, and the whole
   plan had been written as though that were not true. Echo cancellation is requested now
   (`echoCancellation`, `noiseSuppression`, `autoGainControl`, asked for out loud rather than
   left to a default) and it is the right long-term answer — but **how well it works is a
   property of somebody's hardware and this cannot measure it**, so the loop is closed by
   construction: detection holds off while a character talks, and says so (`THEIR TURN`).

   *A loop that ships is worse than a feature that waits.* What is given up is written down
   rather than hidden: with echo cancellation measured against real speakers in a real room, the
   hold becomes a raised threshold and interrupting mid-sentence works again. Halting a turn in
   flight still fires — the model is answering something the user has changed their mind about,
   and that is true before a word is spoken — and the button stops a character at once.

   **And a send inside a state updater.** `onEnd` read the current draft through `setDraft`'s
   updater to avoid a dependency, which put a side effect somewhere React is free to run twice —
   StrictMode calls every updater twice on purpose. A turn sent twice, and it would only ever
   have appeared on somebody else's machine.

   **Its own button, not a mode SPEAK switches into.** A control whose meaning depends on hidden
   state is one this codebase has already deleted (`Manual`, ADR-0027), and *press to talk* and
   *talk to me* are two things a person wants at two different moments. This is also the one
   place the microphone **sends** — step 5's *writes into the draft and never sends* is not
   quietly broken, it is a mode somebody turns on whose entire purpose is that there is nothing
   to press.

   **Measured in the window:** `HANDS FREE` → `THE ROOM…` → `LISTENING` in 0.4 s, gold while
   live, released cleanly. What a driver cannot measure is a voice in the room, so the thresholds
   above are the first honest guess at a shape and the owner's own speech is what settles them.

### Where the voices come from *(measured against Hugging Face's API, 2026-09-03)*

**`rhasspy/piper-voices` — MIT, and that is a second licence decision, not the same one.** The
engine is GPL-3; the voices are MIT. What bites in Phase 16 is the binary, and the binary does not
have to be bundled.

**175 voices across 56 locales.** `medium` is 60 MB, `high` is 115 MB. The ones that matter here:

| Spanish | | English (6 of 38) |
|---|---|---|
| `es_MX/claude/high` · `es_AR/daniela/high` | | `en_US/ryan/high` · `en_GB/cori/high` |
| `es_ES/davefx/medium` · `es_ES/sharvard/medium` · `es_MX/ald/medium` | | `en_US/lessac/medium` · `en_US/amy/medium` |
| `es_ES/carlfm/x_low` | | `en_GB/alan/medium` · `en_US/hfc_female/medium` |

**The sidecar is a complete manifest, which is better than this phase first claimed.** Opened
rather than assumed — `es_MX-claude-high.onnx.json` carries `language` (code, family, region,
`name_native`, `name_english`, `country_english`), `audio` (`sample_rate`, `quality`),
`num_speakers`, `dataset` and `piper_version`.

So **the shelf fills itself from files Epoch is allowed to read**: name, language, country,
quality, speakers, with no filename parsed anywhere (ADR-0024). With Kokoro none of that exists
outside a README.

**A community ecosystem, and it is not small.** Thirty-odd repositories. Two worth naming:
`AIHeaven/piper_unofficial_voices` (the same two-file shape, character voices, **no licence
declared**) and `HirCoir/*` (several `es_MX` voices, some Apache-2.0, one repository each).

### How a source qualifies — derived, never declared

**Hugging Face has no tag for *a Piper voice*.** `rhasspy/piper-voices` declares no library; its
only tags are `onnx` and `license:mit`. A name search finds those thirty and would equally find
anything called something similar.

So the Voices Workshop **may not filter by tag, and must not try**:

> A repository qualifies when its files prove it: an `.onnx` with an `.onnx.json` beside it whose
> JSON carries `piper_version`.

That is the rule that already decides whether a workflow can do img2img — by whether its graph
holds a `LoadImage`, never by what a manifest claims. **A capability typed into a manifest is one
that will eventually lie**, and here there is no manifest to lie in the first place.

**And the licence travels with each asset, never with the search**: MIT for the official
repository, Apache-2.0 for some of HirCoir's, **none declared** for AIHeaven's. `Asset.licence`
has been mandatory since ADR-0016, so this costs nothing and is said on the shelf rather than
discovered at export.

### RVC — **measured 2026-09-04, and it works**

The question this section deferred was never *can it be wired* — it was **how it sounds**, and
that is now answered by the owner's own ears: converted through a Pato Donald model at **+12
semitones**, Mage is unmistakably Donald Duck.

**The pitch was the variable the first attempt did not have.** The same chain through a Naruto
model at `pitch 0` came back *"different, but I could not say it was working"* — Piper's `davefx`
is a low male voice and Naruto's Latin American dub is a woman, so no shift leaves RVC in a
middle ground that is neither. That was a measurement of *RVC in the wrong register*, not of RVC.

> **A parameter left at its default is a variable you did not control.** The first run was
> inconclusive because of a number nobody had thought about, and the fix was to vary it rather
> than to doubt the chain.

Consequence for the design: **pitch is not a property of the voice model.** It is the
relationship between the Piper voice that speaks and the RVC voice that colours it, so it belongs
beside the pair — a character who speaks with `es_ES-davefx-medium` and sounds like Donald needs
+12; the same Donald under a different Piper voice needs something else.

#### The path, measured end to end

**No `fairseq`, which was the real risk.** The RVC packages on PyPI pin `fairseq==0.12.2` and
`numpy<=1.23.5`, and that is a pit on Windows. The ONNX route avoids it entirely: `transformers`
loads the encoder once, and after conversion **only `onnxruntime` runs**.

| step | cost |
|---|---|
| RVC `.pth` → ONNX | **110.5 MB**, once per voice |
| ContentVec encoder → ONNX | 360 MB, once, shared by every voice |
| FCPE pitch → ONNX | once, shared |
| **converting 10.6 s of speech** | **3.8 s on CPU** — 0.36× real time |

Converted with `SUC-DriverOld/rvc.onnx` (MIT, 46 KB), **read before it was run** — no network, no
`subprocess`, no shell. One patch was needed: torch 2.14 defaults to the dynamo exporter, which
refuses the dynamic dimensions the script declares, and `dynamo=False` takes the TorchScript path
it was written against.

**And a third filename that was a claim.** The listing says *300 Epochs*; the file says
`info = 160epoch`. Naruto's said 500 and the file said 545. Every `.pth` so far has disagreed
with the page offering it.

**Both pickles were disassembled before torch opened them** — three globals each,
`collections.OrderedDict`, `torch._utils._rebuild_tensor_v2`, `torch.HalfStorage`, all on torch's
own allowlist. That check is what makes accepting a stranger's `.pth` acceptable at all, and it
is ~200 lines of Rust rather than a dependency.

#### What was left — **all of it built 2026-09-06**

This list read as three open items for two days after the last of them was written. It is
closed here rather than deleted, because *a record of a hole must be closed in the same commit
that closes it* and the only reader a stale list misleads is the one doing the next piece of
work.

- ~~read the pickle from Rust~~ — `pickle.rs`, an eight-name allowlist, both real `.pth` files
  read in 0.06 s without executing a byte (`2eef4bd`).
- ~~a deck row for the converter~~ — the RVC row on the Speaking deck, and Epoch builds **its own**
  Python under `tools/rvc` rather than using one on the PATH: measured, `python` here was another
  application's virtual environment (`1a9c690`).
- ~~the inference in `ort`~~ — `load-dynamic`, so the crate compiles against nothing and the
  15.8 MB runtime is fetched the day somebody asks. 13.41 s of Piper became 13.40 s of Pato
  Donald in **6.5 s**, confirmed by ear against the Python trial (`988e228`).

**Two things the build found that this section could not have.** The exported graph declares its
frame axis dynamic and it is not — RVC's relative attention freezes `int(length) - 1` into a
constant, so a 13 s sentence answered a Reshape error after every piece had passed its own test.
A conversion now *chooses* a window and writes it into the sidecar. And there is no ready-made
ONNX ContentVec to fetch: five candidates on 2026-09-06, two `404` and three `401`, so Epoch
converts the encoder itself with the toolchain it already owns.

**Wired into the product** (`d97f2e5`): `sounds_like` is one canonical Character field carrying
the voice *and* the pitch, because the two are meaningless apart. `Shelf::Timbres`, its own,
since both are `.onnx` and one folder would put an RVC model in the list of voices a character
can *speak* with. Verified in the window: Mage's own file, the World's projection, the Chronicle,
audio — 1.70 s plain against 2.86 s coloured for a 3.2 s line.

### RVC — the original note *(the owner found it 2026-09-03)*

`QuickWick/Music-AI-Voices` and its kind are **not TTS and cannot be used by Piper**. RVC is
*voice conversion*: audio in, audio out. It cannot read text; it has nothing to say. Measured:
881 files, every one marked `(RVC)`, every one a recognisable recording artist, licence `other`.

`Epoch` in those filenames is **training epochs, not a version and not a quality scale** — past a
point it is overfitting. A filename is a claim (ADR-0024); it says how long somebody trained, not
how it sounds.

Where it would fit is as a **third stage**: `text → Piper → RVC → audio`. Deferred, with the cost
written down so the decision is made with it in view rather than from memory: it brings **Python
and PyTorch back** through the door pipecat was turned away from, it is **a second model pass per
sentence** against a real-time budget nothing has measured yet, and its `.pth` is **a pickle**
`asset.rs` refuses by design.

**The reason it will come back is real and is recorded too:** training a Piper voice needs a
recorded, transcribed dataset of your own plus PyTorch Lightning and a Cython build — a weekend
per voice. RVC needs far less audio, which is why people use it. When the owner wants *their own*
voice, RVC is the short answer and Piper is the long one.

**On identity, as a fact rather than a position.** These are clones of identifiable real people.
The rule is already written for imported artwork (ADR-0024): CONTENT_PHILOSOPHY's hard rule
governs what Epoch **distributes**, not what a user keeps in their own vault, and it applies again
at export. Epoch cannot read whom a file imitates, and refusing on a question it cannot measure
would be the invented gauge wearing a veto. What it will not do is **seed the Voices Workshop with
a catalogue whose headline is famous people** — not a refusal, a product choice, and the
difference between *you may point Epoch at a file you have* and *Epoch ships a shelf of
impersonations*.

### Four rules that fall out of the shape

- **It speaks the answer, not the turn.** `Entry::Answered` and nothing else. A tool result is not
  speech, and a `run_command` call is certainly not.
- **Speech failing must never cost an answer.** With no engine running the turn arrives as text,
  exactly as today. Same discipline as the 3D job's preview: *a job's optional last step must not
  be able to take the job down* — the model sat finished in the vault while the card read WAITING
  for twenty minutes.
- **One voice at a time.** Phase 12½ made two characters answer at once. Two voices over each
  other is not a conversation. A queue, and the single slot is a shape the Engine already has.
- **Speak per sentence, as tokens arrive.** A model at 40 tok/s otherwise leaves you waiting for
  half an answer before the first word.

### Measured first — and all three are answered (2026-09-04)

Three questions, asked before a line of Phase 15 was written. **Two of the three came back the
opposite of what this section predicted**, and the plan below is the corrected one.

#### 1. WebView2's own voices — present, offline, and in the wrong language

`speechSynthesis` exists and speaks with nothing downloaded. Not taken from `onend` firing, which
a no-op would also do, but from a property a stub cannot have — **duration proportional to how
much there is to say**, plus a control in the other direction:

| said | took |
|---|---|
| 5 characters | 1 244 ms |
| 36 characters | 4 631 ms |
| 148 characters | 12 580 ms |
| the same 148 at `rate: 2` | 7 000 ms |

But `getVoices()` returns **an empty list**, before and after `onvoiceschanged`, so nothing can be
*chosen* through the web API. And what the machine actually holds, read from both Windows stacks:

```
SAPI     Speech\Voices\Tokens   David Desktop · Zira Desktop        en-US
OneCore  Speech_OneCore\...     David · Mark · Zira                 en-US
```

**Five voices, every one of them American English, none Spanish.** So the day-one story this
section was built on — *the crew speaks immediately and Piper is an improvement* — is false in the
half that matters. A character would read the owner's Spanish in an English accent, and Epoch
cannot even see the list to warn about it.

> **A capability that is present, offline and in the wrong language is not a working default.**
> It is the cold-instrument rule in its fifth form: measured, correct, and unusable.

**Piper therefore stops being the improvement and becomes the engine.** The system voice survives
only as the *last* fallback, and it must say what it is.

#### 2. whisper.cpp on Windows — a published binary, and faster than the wait it was budgeted

Release `b4938` (2026-08-20) publishes Windows builds. **No build step, and CUDA is optional:**

| asset | size |
|---|---|
| `whisper-bin-x64.zip` (CPU) | **8.4 MB** |
| `whisper-blas-bin-x64.zip` | 21.1 MB |
| `whisper-cublas-11.8.0-bin-x64.zip` | 269.9 MB |
| `whisper-cublas-12.4.0-bin-x64.zip` | 671.0 MB |

The CPU archive carries `whisper-cli.exe`, **`whisper-server.exe`**, `whisper-stream.exe` and
`whisper-vad-speech-segments.exe` — the server shape this section wanted, and a VAD, in 8.4 MB.

Timed on this machine, CPU only, `ggml-base.bin` (142 MiB), three runs each:

| clip | took |
|---|---|
| 6.1 s | 921 · 920 · 1 063 ms |
| 19.7 s | 1 340 · 1 341 · 1 347 ms |
| 36.7 s | 2 476 · 2 519 ms |

**The cost is per 30-second window, not per second of speech.** Six seconds and twenty seconds
cost the same because both fit in one window; thirty-seven takes two. That is the number the
microphone button should be designed around: **anything a person says in one breath transcribes in
about a second**, and there is no wait worth animating.

**And `small` is not simply better.** Same 5-second Spanish clip, `base` against `small`:

| model | took | heard |
|---|---|---|
| `base` | 1 035 ms | *Hola, soy **Image**. Trabajo en la Torre y convierto, **jechivos** en disenos limpios.* |
| `small` | 3 124 ms | ***Cola** soy **imagen**. Trabajo en la torre y convierto objetivos en **dícenos** limpios.* |

Three times the cost, and it traded one error for another. What actually fixed it was **telling
whisper the crew's names** — `--prompt` takes an initial prompt, and Epoch already knows who lives
in the World:

| model | with `--prompt "La tripulacion de este mundo: Mage, Paladin, Robo, Frog."` |
|---|---|
| `base` | still *Image* |
| `small` | ***Hola, soy Mage. Trabajo en la torre y convierto objetivos*** *en dícenos limpios.* |

> **Epoch knows the names and was not saying them.** The same shape as every defect in
> `CLAUDE.md`'s *What a turn owes the model*: not a missing capability, a fact withheld from
> something that needed it.

So `small` **plus the roster** is the recommended default and `base` is the small-machine option —
and the roster is not a nicety, it is what makes the larger model worth its three seconds.

**Stated as measured and not further:** the accuracy above is on **synthesised** speech at `x_low`
quality, not a human into a microphone. The *timings* are the product's; the *word errors* are a
robot voice's, and a real microphone must be measured separately before any of them is quoted.

#### 3. The mouth needs no CMake, and the modern Piper is the one that needs Python

This row asked the wrong question. `libpiper` with CMake was never the only way in:

| | `rhasspy/piper` `2023.11.14-2` | `OHF-Voice/piper1-gpl` `v1.7.0` |
|---|---|---|
| shape | **standalone `piper.exe` + DLLs**, 22.5 MB | Python wheel, 34.1 MB |
| Python | none | **required** — the wheel is `.py` files plus one `espeakbridge.pyd`; inference runs in Python on onnxruntime |
| CMake | none | not the obstacle either |
| maintained | frozen since Nov 2023 | active |

Measured, standalone binary, CPU only, Spanish, cold then warm:

| voice | on disk | 5 seconds of speech took |
|---|---|---|
| `es_ES-carlfm-x_low` | 28.1 MB | 1 204 ms cold · **377 · 384 ms** warm |
| `es_ES-davefx-medium` | 63.2 MB | **418 · 404 · 406 ms** |

**A voice is tens of megabytes and speaks in under half a second.** That is a shelf like ComfyUI
and llama.cpp, exactly as this section hoped — and the medium voice, published long after the
binary was frozen, **runs on the 2023 executable unchanged**, which is what makes frozen
acceptable: the engine is stable and the *voices* are what evolve.

**And the loop closes on itself.** Piper's Spanish, fed straight into whisper's ear, came back as
Spanish — mouth and ear verified against each other rather than against a description of
themselves.

**The one thing genuinely given up** is that the standalone line receives no fixes. If it ever
matters, `piper1-gpl` is the fallback and it brings Python — which is the cost this row existed to
find, arriving one repository to the left of where it was expected.

**CMake was never needed, and the proof is that it never installed.** The winget package failed with MSI `1603` after half an hour, and every number in this section had already been measured without it.

**And the number the owner was to decide with is decided.** `large` is 2.9 GiB on a 12.28 GB card
that already holds a text model and, when it draws, Flux — and none of it is needed. **The whole
ear runs on the CPU**: 8.4 MB of binary, 142 MiB or 466 MiB of model, about a second per
thirty-second window, and the card never enters the question. So pressing the microphone can
afford to start the server, and nothing competes with the brain or the easel.

> **What the user sees:** they press a microphone, say what they want, and a character answers
> out loud in a voice they chose.
>
> **Summary:** the World stops being something you read.

---

## Phase 15¾ — Safe to hand to somebody else *(2026-09-07)*

**An external audit read the code and scored it 48/100 for public release.** Not the product —
*the release*. Everything it found was real, and the shape it kept finding is worth naming
before the list: **a property that was true because forty places remembered to keep it true.**

This phase is what closes it. It is placed before Phase 16 because Phase 16 is the push, and
nothing here is optional in front of one.

### Ground — what was closed

1. **The wire between machines is encrypted** (ADR-0029 §16). Everything a Host and a lent
   machine said crossed the network in clear text: the composed turn — the whole Chronicle, the
   system prompt, the project's context — and, at pairing, the long secret itself.

   There is nobody to ask for a certificate on a home network, so what replaces one is the
   **fingerprint**, exchanged under the short code: the machine that shows the code says which
   certificate is its own, the side that dials records what it was shown, and every connection
   afterwards accepts that certificate and no other. A bond with no fingerprint **cannot connect
   at all** — accepting anything when nothing was recorded would mean the encryption bought
   nothing precisely on the bonds nobody has looked at since.

   `epoch-wire` is a new crate because two programs are the two ends of one conversation, and a
   handshake written twice is two handshakes. It does **not** use `tiny_http`'s TLS: that feature
   is built on `rustls` 0.20, from 2021.

2. **Two doors, because there were always two audiences.** EpochServices served the person's
   pages and the Host's routes on one listener, told apart by `if mine` on forty match arms. The
   page is now bound to `127.0.0.1:11499` and the Host's routes are on `11500` under TLS, in a
   different file. The *network* port did not move — 11500 is what the other machine knows.

3. **Loopback stopped being read as identity.** A page in the user's browser reaches 127.0.0.1
   exactly as a program's own window does. Three conditions now, none leaning on another: where
   it came from, what it called us (`Host` — the DNS-rebinding gate), and whose page it is
   (`Origin`). **No CSRF token**, and that is a decision: it would defend against a browser that
   omits `Origin` on a cross-origin `POST`, which is a browser that does not exist.

4. **Bodies, threads and attempts are bounded.** `read_to_string` had no ceiling, so how much a
   machine held was the caller's decision; accepting and working were one thread, so a caller
   trickling bytes held the only loop the program had; and a pairing code could be offered for
   ever. 64 MB / 1 MB ceilings, 32 workers, ten wrong answers and the code is burnt.

5. **The web tool connects to what it checked.** `vet` resolved a name, refused every internal
   address and returned the host — and `ureq` then looked the name up a second time. Pinned to
   the approved addresses now, on the port that was checked.

6. **`capabilities/machine.rs` says what it does.** Its header opened with *There is no shell*
   and reasoned that injection was inexpressible; `parse` had been handing the line to a shell
   since `git commit -m "two words"` had to work. The deadline and the output cap were weaker
   than they read, too: `child.kill()` ends the shell and not what the shell started, and the
   32 KiB cap bounded what was *kept* behind an unbounded channel.

7. **Four files that cannot be found half-written** — `secrets.dat`, `bridges.toml`,
   `settings.toml`, `bond.json`. Temporary in the same folder, flushed, renamed over;
   `MoveFileExW` on Windows, where `std::fs::rename` refuses an existing destination.

8. **The gates that nothing ran, and had all already drifted**: 601 formatting hunks, 27 Clippy
   warnings, an MSRV that said 1.82 while four dependencies needed 1.88, and a dependency policy
   that did not pass. CI now runs all of them **on both programs**, on the declared minimum as
   well as stable, plus the production bundle build and — since the Mac found what it found — a
   **macOS job**.

9. **Provenance instead of a signature.** A signature proves two things: *who made this*, which
   costs money every year, and *this file is the one that was built from this source*, which is
   free. `actions/attest-build-provenance` signs the second through Sigstore into a public
   transparency log. It does not stop SmartScreen — nothing free does — so both READMEs say what
   each platform will show and how to get past it.

### Life — what the other machine found

The security work was finished, measured and committed on Windows. Then the Mac was switched on
and asked the same questions.

- **The application did not compile on macOS at all.** A `#[cfg(not(target_os = "macos"))]` had
  drifted one item down onto `machine::driver()`, which `epoch-tauri` calls — sitting between
  that function's own two doc comments, one of which says *`None` on every machine that will not
  say, **including every Mac***.
- **A door that reported its failure to its own thread**: `wait_for_one` returned `Ok` and then
  bound the port on the thread it had just spawned. Windows won that race and macOS lost it.
- **Nine red tests, and only four were tests.** One was the product (the installer offered
  whisper.cpp with a live button and nothing to fetch — macOS gets an xcframework); one was a
  rule with two spellings (`confine` refused the same path for two different stated reasons,
  because Windows normalises `a/../..` lexically and Unix does not); four were fixtures written
  with Windows paths that measured nothing on a Mac; and three were the login keychain being
  asked over SSH, which is *nobody at the machine* rather than *broken* — `Store::reachable`
  gives three answers for that reason.

**Measured against the running programs, not against a harness**: pairing over TLS from the
panel with the code off the Mac's screen; the pinned fingerprint equal to the certificate's own
SHA-256, checked with `openssl` on the Mac; a probe with **one hex character changed** finding
nothing where the true one found `["ollama"]`; a real turn answered over the bridge; the app
updated in place with the bond surviving; and a page route on the Host's door answering 403
because it is not there.

**1685 Rust on Windows · 1671 on macOS · 105 EpochServices · 586 TypeScript, all green.**

### What is still open, in the order agreed

Everything here is QA of the real artefact rather than engineering, and it is the reason the
release is not cut yet.

1. **Network loss during an inference.** First, because it is the only perishable one: it needs
   the Mac switched on and paired.
2. **`LICENSE`, `NOTICE`, `SECURITY.md`, `CONTRIBUTING.md` at the repository root.** They exist
   only in the publication trees. Minutes, and publishing without them is publishing wrongly.
3. **Install the real `.exe`**, and in that same sitting: **approvals and denials in all three
   agent adapters**, then **audio, GPU, cancellation and navigation**. They share the
   installation, and they are exactly what jsdom and a development binary do not attest.
4. **Update over it and uninstall**, checking the vault survives the first and that the second
   takes nothing that is not its own.
5. **Triage the 82 ignored tests** — how many wait for an environment and how many are dead
   claims. It unblocks nothing, so it goes last.

### ~~Found while doing 3, and the only one that blocked a release~~ — done 2026-09-08

**Gemini CLI never asked Epoch, and its `Manual` stopped nothing.** Measured through the
installed application on 2026-09-08: asked in Manual to write a file, it wrote it. The adapter's
`_approver` was unused — Codex's is used nine times — and `Manual` mapped to Gemini's
`--approval-mode plan`, which is its own *read-only* mode rather than a permission one, and did
not hold.

**It goes through Epoch's approver now.** The one-way `--prompt "" --output-format stream-json`
transport is deleted and replaced by `gemini --acp`, the Agent Client Protocol: the agent sends
`session/request_permission` before running its own tool, carrying the tool call, a diff and
three options, and waits for the answer Epoch's one-slot approval bridge gives it. Verified end
to end through `work()` itself — **refused: the file does not exist; allowed: it does** — and in
the installed window. `clientCapabilities.fs` stays **false**, so Epoch does not take the agent's
own file tools (2026-08-07).

**Two defects the protocol measurement could not have found, and both needed the wire.** They
are why `EPOCH_TRACE_ACP` survives in the file:

- **`session/prompt` was sent without waiting for `session/set_mode` to reply**, and the agent
  sometimes dropped it — one run answered in 13 s, the next never answered at all. A Python probe
  had "proved the protocol" while quietly waiting between the two, which is exactly the kind of
  difference a harness hides. The safety half matters more than the hang: a prompt racing the
  mode it depends on could run under whatever the session was set to before.
- **The turn ended and never returned.** `Child::kill` ends the `gemini.cmd` shim; the real
  program is `node gemini.js`, which re-executes itself, so the grandchild survived holding the
  reader's pipe and `join()` waited forever — with the character's card reading `WORKING` and the
  answer already written. `capabilities::machine` had found the same fact about a *shell* months
  earlier and kept it private, so `stop_tree` is shared now: **a rule kept by repetition is a
  rule that has already been broken.**
- **The `cwd` was unreadable, and only in a real World.** Containment canonicalises a Project
  Root, so on Windows Epoch was sending `\\?\C:\Users\…`; Node splits that into a root of
  `\\?\` and a first segment of `C:` and answers `EISDIR: illegal operation on a directory,
  lstat 'C:'`. Every turn failed before the model was reached. `library::plainly` had been
  written for exactly this — for Obsidian, months earlier — and no adapter used it. The live
  test could not see it because `std::env::temp_dir()` carries no prefix: **a fixture that
  happens to supply clean input measures the wiring and nothing else.** The fixture
  canonicalises now.

**Codex sends the plain form too, and only after it was measured.** It had tolerated the
extended-length one since the adapter was written, so this was a change to a path that
demonstrably worked — the kind that is not made on reasoning. The first attempt could not be
measured at all (the account's allowance ran out that hour) and the request path was put back to
what was verified until it returned. With the plain form, in the installed window, an allowed
write lands: `codex-si2.txt`, with `HOLA` in it. `cwd` and `writableRoots` are built from the
same helper, so the sandbox and the working directory cannot disagree inside one request.

### What the router serves is listed too — done 2026-09-08

Reported by looking at the window: **CREW LINKS read `10 models` and MODELS read `2`**, in the
same frame, about the same machine, with nothing on screen to resolve them. Both readings were
real. `everything_here` reads three *file stores* — Ollama's, `<vault>/models`, and llama.cpp's
own download cache — and never asked the running router what its configuration points at, which
on that machine was a fourth directory none of them walk.

Read from `/v1/models`'s `status.args`, by flag: `--model` for the weights and `--mmproj` for the
eyes. The projector especially — `projector_beside` takes the first `mmproj` in a folder, which
is right for a Hugging Face snapshot and wrong for a flat shelf of fourteen files, where it would
hand the same eyes to every model. The router was told which one; it is asked.

**Listed, never adopted** (ADR-0032). Measured against the running router: 8 rows added, and the
two that were dropped were dropped for reasons worth having — one is a symlink whose target is
gone, one is the `-hf` form with no local file at all. A row for bytes that are not there is
worse than a missing row.

*(Separately, and not a defect: the installed application and `target/release` read different
vaults — `%APPDATA%\Epoch\vault` against `BUILD\vault`. A deck that looks empty in one of them
is usually that.)*

### Models are asked for, not remembered — done 2026-09-08

Epoch shipped a hardcoded six for Codex, four aliases for Claude Code and nothing for Gemini.
Measured on this machine the same day: Codex offers **`gpt-6-astra`**, which was not on the list,
and no longer offers `gpt-5.4`, which was. A character could be pointed at a model that does not
exist, and fail inside a Quest rather than at the door.

Both programs answer, and neither says so in `--help`: **Codex** `model/list` over its
app-server, **Gemini** in `session/new`'s own `models.availableModels`. Per account, because what
a sign-in may reach belongs to the sign-in; kept ten minutes, which is long enough that opening a
panel twice is instant and short enough that a CLI installed mid-session is noticed.

**Claude Code answers too, and the first pass said it did not.** `-p /model` prints its
vocabulary for nothing — `num_turns: 0`, no tokens, 94 ms — and names ten values where the
compiled list had four (`best`, `opusplan`, `default`, `sonnet[1m]` and the aliases). The earlier
conclusion came from measuring `--help`, which answers *which subcommands exist* and not *what a
slash command answers in `-p`*; `claude.rs` had been reading `/usage` that way for months. Left
recorded rather than tidied — it was written down as measured and would have been trusted.

The **aliases** still do what they claimed: `--model sonnet` reports `claude-sonnet-5`, so a
stale fallback keeps working. A brand-new *family* is answered by the picker's "name it myself"
field rather than by a probe that guesses at names.

**A program nobody could ask keeps its suggestions.** Not installed, not signed in, a release
that no longer speaks the protocol: the old list is shown and the field stays typeable. Replying
*there are no models* to a question nobody could put is the inversion this codebase keeps paying
for.

Out of the critical path and named in the announcement rather than hidden: **Linux is
unmeasured** (no machine), the **963 kB JavaScript bundle** (the audit itself says measure
startup before optimising), and the **two large files** — `workflow.rs` at 6,514 lines and
`workshop.rs` at 5,025 with application logic inside Tauri — deferred to post-release by
agreement, and both about 300 lines larger than when the audit measured them.

> **What the user sees:** nothing. That is the point of this phase — every change in it is
> something that was already supposed to be true.
>
> **Summary:** the release stops being a thing that would have to be explained.

---

## Phase 16 — Somebody else can install it

**The GitHub push belongs here, and the owner's reasoning is what moved it.** It had been treated
as a backup waiting on the repository being safe to copy — but git is already the backup, locally,
and it has been all along. A push is *distribution*, which is what this phase is, and nothing
before it needs one.

That also settles the history question by moving it. `git rm --cached` removed `BUILD/vault/` from
the future and not the past, so old commits still hold the author's face, name, machine paths and
one real conversation. **No credential was ever committed** — verified, not assumed. Deciding what
to do about the rest is part of preparing a release, alongside the licence and the name, rather
than a thing blocking work that never needed a remote.

**Last, by the owner's decision (2026-08-22).** It was proposed for straight after Phase 10, on
the reasoning that the first thing anybody installs should be able to *make* something. The owner
kept it at the end: the release ships what the product is, not the first thing that works.

### How the release is actually handled *(decided across 2026-09-05…07)*

Everything in this subsection was decided with the owner and had, until now, lived only in a
conversation. It is written here because Phase 16 is the phase that performs it.

#### Two repositories, both public, and what is in each

| | `KislokX/epoch` | `KislokX/epochservices` |
|---|---|---|
| code | `BUILD/crates`, `BUILD/ui`, `BUILD/packs`, the manifests | `EpochServices/` |
| documents | the nine root constitutions, `docs/` | its own README, LICENCE, NOTICE, SECURITY, CONTRIBUTING |
| CI | `ci.yml`, `release.yml` | `epochservices.yml` |
| Releases | `Epoch_Setup.exe` + `SHA256SUMS` + provenance | the three platform bundles, same |

**Apache-2.0**, and the copyright holder is the owner's name and email. **That is the only place
his personal information may appear** — everything else about him was deliberately swept out of
the source, including a hostname and a LAN address that lived in twenty-nine test fixtures.

#### What never travels

The publication pipeline decides this by an **allow-list, not a deny-list**, so a new file is
excluded until somebody adds it rather than included until somebody notices:

- `BUILD/vault/` — the working World, the crew, the conversations;
- every transfer file, this roadmap's own working notes, `AUDIT-QA-*.md`;
- `docs/Build/Handoff.md`, `Minecraft.md`, `Repollo.md` — they name machines and addresses;
- `packs/default` and the artwork the owner supplied; the two shipped Worlds carry **no artwork
  at all**, which is a state the engine is built for;
- anything under `.vs/`, `target/`, `node_modules/`.

**Never run `npm install` or `cargo test` inside the publication tree.** It happened once and put
ten thousand hits of the owner's username into it by way of `node_modules`.

#### Nothing is signed, and that is settled

A certificate is a recurring cost the owner has decided against. What ships instead is the half
of a signature that is free: `SHA256SUMS` beside every artefact, and **Sigstore build provenance**
(`actions/attest-build-provenance`) proving the file came from that repository at that commit, on
GitHub's runners rather than on somebody's desktop. `gh attestation verify` checks it.

It does **not** stop SmartScreen or Gatekeeper — nothing free does — so both READMEs say what each
platform will show and how to get past it. An unsigned build that explains itself is honest; one
that pretends is not.

#### The blocking problem, found 2026-09-07 while writing this down

**The published `epochservices` repository would not compile.** Its `Cargo.toml` reaches the
shared crates by path — `epoch-kernel`, `epoch-models`, `epoch-assets`, `epoch-secrets` and now
`epoch-wire`, all `../BUILD/crates/…` — and the publication tree contains only `EpochServices/`.
Its README already describes the intended arrangement and describes it wrongly: *"pinned by
revision in Cargo.toml"* is not what the file says, and *"four crates"* is now five.

Three ways out, and they are not equal:

| | what it costs |
|---|---|
| **git dependencies pinned by revision** *(recommended)* | the `epoch` repository must exist and be public first, and a shared-crate change needs a deliberate `rev` bump — which the README already frames as the feature it is |
| vendor the five crates into the tree at publish time | it builds standalone and there are now **two copies of the code**, which is the thing the crate split exists to prevent |
| one repository for both programs | simplest and truest, and it is not what the owner asked for |

The recommendation is the first, and the mechanical half belongs in the publication pipeline:
rewrite five `path = "../BUILD/crates/…"` lines into `git = "…", rev = "…"`. **It cannot be
finished before the `epoch` repository exists**, because the revision has to be a real commit —
so the order is: publish `epoch`, take its commit, then publish `epochservices` against it.

#### The order, when the five QA items above are closed

1. Create `KislokX/epoch`, public, and push the branch.
2. Take that commit's sha; rewrite EpochServices' five path dependencies against it; check that
   the tree builds **from the publication copy**, not from the working one.
3. Create `KislokX/epochservices`, public, and push.
4. Run `release.yml` and `epochservices.yml` on demand; attach the artefacts, the checksums and
   the attestations to a Release on each.
5. Only then the announcement — with Linux unmeasured, the bundle size and the two large files
   named in it rather than hidden.

**Ground.** The Wizard and the app are built from one tree, tagged together
([Install and First Launch](docs/Milestones/First%20Run.md)).

1. **`installer/`** — `Epoch_Setup.exe`: detect the machine, install WebView2 if missing, install
   Epoch per user, create the data directory, register the uninstaller. The bundle itself is no
   longer hypothetical: `Epoch_0.1.0_x64_en-US.msi` builds since 2026-08-21, when the icon nobody
   had pointed `tauri.conf.json` at was found.
2. **Offer, never impose** — Ollama, Claude Code, Codex, ComfyUI, Git as unchecked boxes with
   sizes shown. Declining all of them still produces a working Epoch that says what is missing.

   **The whole voice chain is offered here, not only the ear** (owner, 2026-09-03). Four boxes,
   because they are four separate *no*s and a person may want one without the others:

   | box | what it is | size |
   |---|---|---|
   | **whisper.cpp** | the ear — the microphone works | **8.4 MB** binary + 142 MiB (`base`) or 466 MiB (`small`), all CPU |
   | **Piper** | the mouth — characters can speak | **22.5 MB**, standalone, no Python; voices are separate |
   | **one voice** | who they sound like | 28–115 MB (`x_low` to `high`) |
   | **PyTorch CPU** | turns an RVC voice into one Epoch can use | **867 MB**, whole toolchain |

   **Why PyTorch is on a list of things this project otherwise refuses** (owner, 2026-09-04).
   It is needed for exactly one step and never at runtime: rebuilding an RVC model's graph to
   emit ONNX. After that conversion the whole chain is onnxruntime, and the box can be
   uninstalled without breaking a single voice. Declining it costs only the ability to *add* a
   character voice — every Piper voice still speaks.

   It is still the largest box by an order of magnitude, so it is the one that most needs its
   size next to it — 867 MB against 8.4 MB is not a difference somebody should discover
   afterwards.

   **And that number was wrong here for one commit, in the direction that matters.** It was
   written as *~2.5 GB* from memory, which is what PyTorch weighs *with CUDA*; installed and
   measured, `torch==2.14.0+cpu` is **544 MB**, and the whole toolchain it needs to convert a
   voice — torch, numpy, onnx, onnxsim, onnxruntime and their dependencies — is **867 MB**.

   Nearly three times over-stated, on the one row whose size is its whole argument. **A number
   carried in from memory is an anecdote**, and this project has a section about somebody
   else's flag being a measurement rather than a memory; the rule did not stop applying because
   the memory was its own.

   A starter voice is offered **because a mouth with no voice is a box that installs nothing you
   can hear** — the same reason the picture chain is useless with a running ComfyUI and an empty
   shelf. Which voice is the user's choice, defaulting to their own locale where Piper has one:
   `es_MX/claude/high` and `es_ES/davefx/medium` both exist, and so do 173 others.

   Declining all three still produces a working Epoch. It simply does not talk, and the Character
   editor says so on the control rather than showing an empty dropdown.
3. **Never** sign in, ask for a key, pull a model, or create a crew, a World or a Project Root.
4. **Update path** — even manual: knowing a new version exists is a feature.
5. **Crash surface** — a panic produces something sendable instead of a frozen window.
6. **`CONTRIBUTING.md` + module map** — "where does this go?" answered by a document rather than by
   gravity toward `state.rs`.
7. **LICENSE** — before anything is shared, not after.
8. **The release** — the tag, the push, and the first version that exists on somebody else's
   machine. **The last step of the last phase**, deliberately.

> **What the user sees:** they download one file, run it, and Epoch opens.
>
> **Summary:** Epoch becomes something that exists on other people's machines.

---

## How to read a phase

Each has a **Ground** half and a **Life** half, and neither ships alone. If a phase's Life half
slips, the phase is not done — that is the whole point of the rule, and the reason the previous
roadmap's one deliberate exception had to be written down as a debt rather than a decision.
