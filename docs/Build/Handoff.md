# Handoff — where the build stands, and what comes next

> Written 2026-08-25 for whoever picks this up next, and re-measured against the code on
> 2026-09-07. It says what is **true right now**, what is **open**, and the order the owner and I
> agreed. It is not a substitute for `ROADMAP.md`, which is the record of what happened; this is
> the state and the plan.
>
> **Every entry below was checked against the repository on 2026-08-30, not copied forward.** The
> previous copy described a shipped phase as the next thing to build, which is the one failure
> this document is capable of causing.

---

## The third audit, and what is left of it

The audit reports are not in this repository — the owner's decision, 2026-09-08 — so this is
where their state lives. Five of the six findings are closed in code, each with the commit that
closed it; the sixth is not code.

| # | what it found | where it went |
|---|---|---|
| 1 | the body budget was given back before the body | `9587243` |
| 2 | the deadline could not stop a read already in flight | `9587243` |
| 3 | a Vitest advisory, and no gate on npm advisories at all | `abec3b3` |
| 4 | agent approvals lost the diff and called every call a write | `628d95a` |
| 5 | the TLS identity was two writes with no way back from half of one | `780a53a` |
| 6 | configuration is not a validated artefact | below |

**Finding 6 is five-sixths measured and one-sixth impossible here.** Clean install, update
preserving the vault, silent uninstall, the delete-app-data checkbox on the real uninstaller and
a first run on a virgin machine were all measured on 2026-09-08 — see `installer-hooks.nsh`,
which carries the numbers. What is left needs a real release run on GitHub: the SBOM and the
provenance of the artefacts somebody actually downloads, checked with `gh attestation verify`
against the same commit.

**Two things the audit's last finding pointed at were decisions rather than bugs, and the owner
made them on 2026-09-08.** Both are done:

- **The MSI is no longer published.** It is `InstallScope="perMachine"`, into Program Files, while
  every other part of Epoch is per user — `%LOCALAPPDATA%\Epoch` for the program,
  `SetShellVarContext current` in the uninstall hook, a vault under `%APPDATA%\Epoch` — and its
  uninstall has neither the delete-app-data option nor the hook. It is still **built**, because
  `epoch-setup` embeds it; it is not offered as a download. One installation path, and it is the
  one every measurement here is about.
- **`epoch-setup` is built and published.** The crate exists so that "a person downloads one
  program rather than a program and a list of instructions", and the release had never produced
  it. The order matters and the workflow keeps it: `world:build` first, then
  `cargo build --release -p epoch-setup`, whose `build.rs` embeds the MSI that step just made.

So a release now carries **two** files and three parts lists: `Epoch_x.y.z_x64-setup.exe` (the
NSIS installer) and `epoch-setup.exe` (the same Epoch plus the first-run offers), with
`epoch.cargo.cdx.json`, `epoch-setup.cargo.cdx.json` and `epoch.npm.cdx.json` beside them.

**What is still open on finding 6**, and it is the only thing: nobody has run a real release and
checked the published artefacts and their attestations with `gh attestation verify`. Everything
above was measured on this machine, which is not the same errand.

## Every skipped test says what it is waiting for

93 tests are `#[ignore]`d, and that is the right number: they need a server, a signed-in agent, a
graphics card, the network, or minutes of model time. What was wrong is that **12 of them said so
nowhere `cargo test` prints it** — the condition lived in a doc comment above the attribute, which
the person running the suite never sees. They carry it in the attribute now, and CI refuses a bare
`#[ignore]`, so the rule stops depending on 93 people remembering it.

One was not waiting for anything. `a_changed_file_is_actually_noticed` builds five characters in a
temporary directory, touches one and asserts the sweep noticed — no network, no server, 0.01 s. It
was skipped for sitting in a file of measurements, so a correctness assertion had been behind
`--ignored` for as long as it has existed. It runs with the suite now.

## What ships beside the installer

Three files, and each answers a different question. `SHA256SUMS` — is this the file that job
produced. Build provenance — did it come from this commit of this repository, on GitHub's
runners. And the **SBOM**, `epoch.cargo.cdx.json` and `epoch.npm.cdx.json` — what is inside it:
509 Rust components and the frontend's, every one with a licence, in CycloneDX.

The SBOM is the one that answers a question asked *later*: an advisory lands on a crate, and
either you grep a published file or you reconstruct the dependency tree of a version already in
somebody's hands. It is attested along with the installer, because an unsigned parts list is a
claim.

Not signed, and that is decided rather than pending: a certificate is a recurring cost and buys
*who made this*, which is the half a stranger can least act on.

## The model picker shows names now, and keeps itself right

`claude -p /model --model <alias>` answers `Current model: Opus 5` for nothing — `num_turns: 0`,
no tokens — so Epoch's picker reads `Opus 5 · opus` rather than `opus`. Both halves are on the
row because several aliases share a model: `fable` and `best` are both Fable 5 today, and
`sonnet`, `sonnet[1m]` and `default` are all Sonnet 5.

It needs no maintenance, and that was watched rather than asserted: `Fable 5.1` was missing on
2026-09-08 because the machine had Claude Code 2.1.247; the owner ran `claude update`, 2.1.265
landed, and the name appeared in the picker **with no code change**.

**What it cannot do is know a model the installed CLI does not** — so a missing model is an
out-of-date CLI, and `claude update` is the CLI's own answer.

2.1.265 also changed the shape of the answer, an hour after 2.1.247 was measured: it writes the
name in backticks. They are trimmed, and the fixture keeps both shapes.

## If the installer will not bundle

`npm run world:build` failing at *"Patching epoch-tauri.exe with bundle type information: nsis"*
with `os error 32` means something holds a handle inside `BUILD/target/release`. Measured on
2026-09-08: it was a **shell whose working directory was `target/release/nsis/x64`** — opened to
read the generated `installer.nsi` and never left. `rm` and PowerShell both refuse to delete that
directory; the Restart Manager reports *nobody*, because it tracks files and not a directory
somebody is standing in.

The cure needs no reboot and touches nothing:

```
CARGO_TARGET_DIR=…/BUILD/target-pack npm run world:build
```

A full rebuild, ~13 minutes, into a directory nothing is holding. `BUILD/target-pack/` is
ignored.

**Four wrong diagnoses came first, and they are worth not repeating**: Windows Defender (the file
is free every time it is asked), the running app, orphaned WebView2 children (all twelve on this
machine belong to Windows), and the Tauri CLI — the Restart Manager named `node`, so the Rust CLI
was installed to replace it and failed identically. *Who holds this file* is not the same question
as *who holds it in a way that blocks a writer*, and the first one has an answer for every build
that works.

## Read these first, in this order

0. **`LastClaudeTransfer.md`** at the project root — the newest full session record
   (2026-09-07): the external audit, everything it found, everything that was closed, what the
   Mac found that Windows could not, and the five things still open. Read it before anything
   else if you are picking this up cold.
0b. **`Claudetransfer8.md`** at the project root — the previous one (2026-09-02) — the newest full session record: what shipped,
   what was measured, what broke and why, the security notes, and how the owner works. Read it
   before anything else if you are picking this up cold. `CLAUDEtransfer.md` beside it is the
   **previous** one (2026-08-25) and is kept for its picture-chain detail; when the two disagree,
   the numbered one is later. **A transfer file records a session, not the present** — check the
   code before acting on any list in one.
1. **`CLAUDE.md`** — the constitution. The bottom third is a running log of rules earned by
   building, and every one of them was paid for by a defect. Read the last ten sections.
2. **`ROADMAP.md`** — **Phase 15¾** is the audit and what closed it, and its last section is the
   list that is actually open. §11.19–11.28 describe the picture chain in detail.
3. **`docs/Build/Build From Life.md`** — how milestones are judged.
4. **`docs/Build/The Picture Chain.md`** — what Epoch can and cannot do with a model that arrives
   in parts, with the measurements behind it.
5. The ADRs the current work leans on: **0025** (Quests own the work), **0026** (Characters are
   assets), **0030–0033** (image generation, the Workshop, the Generative Library, the Studio
   Panel), and **0029 §16** — the encrypted bridge and the fingerprint that replaces an
   authority nobody has.
6. **`AUDIT-QA-2026-09-07.md`** at the project root — the external audit itself. Untracked on
   purpose; it is evidence, not documentation.

---

## How to work on this

**Drive the product, not the API.** Every defect of the last week was found by opening the window
and using it, and none by a test. Start Epoch with

```
WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333 ./target/release/epoch-tauri.exe
```

and drive the real DOM over CDP. `docs/Build/Retrospectives` and §11.19 explain why.

**Build with `npm run world:build` from `BUILD/ui`.** Never `cargo build --release` — it omits the
frontend and the window opens on a dead `localhost:1420`. Kill the running exe first or the link
fails.

**Both surfaces, same session.** `EpochServices/` is a separate workspace that shares Engine
crates. Changing a shared type and not building it there means the bill arrives on the day you
need the other machine. `cd EpochServices && cargo check`.

**Measure, then claim.** A success status code is a claim about the transport. Read the side
effect. Somebody else's flag is a measurement, not a memory.

---

## What is true right now

*Re-measured 2026-09-07. Nothing here is copied forward.*

- **1685 Rust tests on Windows, 1671 on macOS, 105 EpochServices, 586 TypeScript — all green.**
  82 Rust tests are `#[ignore]`d and have not been triaged.
- **The bridge between machines is encrypted** and pinned to a certificate exchanged at pairing
  (ADR-0029 §16). A bond made before this **cannot connect** and says so; re-pairing is one code.
- **EpochServices has two listeners**: the page on `127.0.0.1:11499`, the Host on `11500` under
  TLS. The page is not reachable from the network because it is not bound there.
- **`cargo fmt`, Clippy under `-D warnings` and `cargo deny` all pass**, on both programs. CI runs
  them on Windows *and* macOS, plus the production bundle build.
- **The whole workspace compiles and passes on macOS**, which was not true on the morning of
  2026-09-07 — `epoch-tauri` called a function that a stray `cfg` had removed from that platform.
- **Nothing is signed**, and that is settled rather than pending: checksums and Sigstore build
  provenance ship instead, and both READMEs say what each platform's warning will be.
- **Every medium works.** IMAGE, VIDEO, AUDIO and 3D in the Studio Panel, measured in the window.
  Phase 12 closed 2026-08-29.
- **Two characters work at once** (Phase 12½, 2026-08-26).
- **One thing draws, and it is the panel.** `draw_image` and `edit_image` were deleted 2026-08-28.
- ComfyUI **starts when GENERATE is pressed**, is let go when the picture arrives, and is asked
  for its memory back.

---

## Open, in the order agreed

**Everything engineering is closed.** What is open is QA of the real artefact, and it is the
reason the release is not cut. The order below is the owner's, agreed 2026-09-07, and the
reasoning is part of it.

1. ~~**Network loss during an inference.**~~ **Closed 2026-09-07.** The failure names the
   machine somebody has to go and look at, and the `Keep` lamp reads that machine's card rather
   than the local one.
2. ~~**`LICENSE`, `NOTICE`, `SECURITY.md`, `CONTRIBUTING.md` at the repository root.**~~
   **Closed 2026-09-07**, in both trees, and `publish.py` carries them.
3. ~~**Install the real `Epoch_Setup.exe`** and, without leaving that sitting: **audio, GPU,
   cancellation and navigation**.~~ **Done 2026-09-08, in the installed window.**
   - **Navigation** — each Place on the dock is visited and shows itself (*"The Harbour · Where
     every crossing begins · Nobody is here"*); `BRIDGE` returns to the Launcher.
   - **Cancellation** — STOP appears at 2.0 s and the crew card is idle again at 4.1 s. It also
     found a defect and it is fixed: the Chronicle read `gemini could not be started: it said
     nothing` for a turn the user had cancelled on purpose. Codex and Claude Code both answer
     `AgentError::Stopped`; Gemini now does too. **Verified in the installed window** — STOP at
     2.2 s, idle at 4.2 s, and the Chronicle reads `Gemini CLI stopped: you asked it to stop`.
   - **Audio** — 13 oscillators started by real World controls, so the chain runs. Sound is
     synthesised (`experience/sfx.ts`), fires only for `button.epbtn`, and the Launcher's own
     controls are deliberately silent. *Whether it is audible in the room is not something this
     method can measure; it wants a person with speakers.*
   - **GPU** — `REACTOR LOAD 2.7 GB / 12.9 GB` against `nvidia-smi` `2254 MiB used`. Not a
     disagreement: the gauge reads **total − free** (2609 MiB = 2.74 GB), and the 285 MiB
     between them is the driver's reserved block, which `memory.used` excludes. Total − free is
     also the quantity that matters before loading a 9 GB model — what you cannot have.
   - ~~**Approvals and denials in all three agent adapters**~~ — **closed 2026-09-08.** Claude
     Code and Codex were verified in the installed window on 2026-09-07 (denied: no file;
     allowed: the file). Gemini did not ask at all and now does: its transport is the Agent
     Client Protocol, `session/request_permission` reaches Epoch's own approver, and refusing
     leaves nothing behind. Two defects the protocol probe could not have found are recorded in
     `CLAUDE.md` (2026-09-08) — a `session/prompt` racing `session/set_mode`, and a `.cmd` shim's
     grandchild holding a pipe open after the turn had finished.
4. ~~**Update over it and uninstall.**~~ **Done 2026-09-08.** Roughly ten in-place updates, and
   then the whole loop: uninstall, measure, reinstall, open.
   - **Taken:** the install directory (`epoch-tauri.exe`, `uninstall.exe`,
     `packs\archipelago\pack.toml`), the Start Menu shortcut, the Desktop shortcut and the
     `HKCU\…\Uninstall\Epoch` key. All four, nothing left behind.
   - **Kept:** `%APPDATA%\Epoch`, 43,129 files, **100.07 GB** — and the vault compared file by
     file against a manifest taken beforehand, identical. Reinstalled from the same installer it
     opens on `1 WORLD DOCKED · 1 CREW`.
   - **Left over:** `%LOCALAPPDATA%\dev.epoch.world`, 0.16 GB of WebView2 profile. A silent
     uninstall does not tick the *delete application data* box, which is the right default.

   **One finding, and it is a decision rather than a bug.** That checkbox deletes
   `%APPDATA%\dev.epoch.world` and `%LOCALAPPDATA%\dev.epoch.world` — Tauri's bundle identifier.
   Epoch's own data is under `%APPDATA%\Epoch`. So somebody who ticks it to reclaim space
   reclaims 0.16 GB of browser cache and leaves 100 GB of models and their vault exactly where it
   was, with nothing on screen saying so. The wiring exists (`NSIS_HOOK_POSTUNINSTALL`) and is one
   line — but it would make a checkbox that silently deletes a 98 GB model library, which is not
   what most people mean by *application data*. **Owner's call**, and until it is made the
   honest reading is that Epoch's uninstall never destroys user data.
5. **Triage the 82 ignored tests.** It unblocks nothing, so it goes when the rest is green; its
   value is knowing how many wait for an environment and how many are dead claims.
6. **The second audit — re-measured 2026-09-08, and the list was wrong.**
   `AUDIT-QA-2026-09-07-SEGUNDA-REVISION.md` has **seven** findings, not three, and the three
   named here had quietly lost one on the way into this document. Checked against the code, one
   by one:

   | # | finding | state |
   |---|---|---|
   | 1 | pairing code redeemable twice | **closed** — atomic reservation, 8-thread barrier test |
   | 2 | TLS configured did not force HTTPS | **closed** — refused before anything is sent |
   | 3 | listener resource exhaustion | **closed 2026-09-08** |
   | 4 | MSRV 1.88 wrong | **closed** — 1.90, and CI proves it |
   | 5 | command-runner bounds | **closed 2026-09-08**, both halves |
   | 6 | trust-on-first-use at the first pairing | **accepted 2026-09-08** — written into
        `SECURITY.md`, in scope and out, so it is a documented limit rather than a silence |
   | 7 | release did not require CI to pass on the same commit | **closed 2026-09-08** |

   **3, closed.** Three separate bounds, because the audit named three separate leaks. A
   **global body budget** of 192 MiB shared by every connection — thirty-two callers times a
   64 MiB ceiling was two gigabytes reserved on request, before anything a caller said had been
   believed, since whether a bearer is *valid* is the handler's question and the handler runs
   after the body is read. It is a `Reserved` guard rather than a counter, so it comes back
   however the door returns, and a request that does not fit is answered `503`. An **absolute
   deadline** of 120 s for the whole request: `set_read_timeout` bounds one `read`, so a caller
   sending a byte every fifty seconds resets it forever. And the body is **read as it arrives**
   rather than `vec![0u8; length]`, so a caller promising 64 MiB and sending one costs one.

   **5, mostly.** A line is capped at 8 KiB (`capabilities/reader.rs`): `split(b'\n')` read until
   a newline however long that took, so the 32 KiB cap on what is *kept* was never a cap on what
   is *held*, and a line past the cap is split rather than dropped. And the readers are no longer
   joined unconditionally — dropping the receiver frees a reader stuck **sending**, not one stuck
   **reading** on a pipe a grandchild still holds, so they get `GRACE` and are then abandoned. A
   parked thread is a bounded cost; a turn that never returns is not.

   **And the Unix half, closed the same day and more cheaply than it was priced.** It was
   quoted as needing `pre_exec` and `libc` — `unsafe` and a dependency. It needs neither:
   `Command::process_group(0)` has been in the standard library since 1.64, and `kill -KILL
   -<pgid>` ends the group without one either. `Quiet::own_group` and `stop_tree` carry it, and
   `capabilities::machine` and the Gemini adapter ask for it.

   Measured on the Mac, three tests: without a group the grandchild survives (the defect,
   reproduced), with one it goes, and a child sharing Epoch's own group does **not** take Epoch
   with it — `stop_tree` asks `ps` what the child's group actually is rather than trusting the
   spawn site to have remembered.

   > **A price quoted from memory is not a measurement either.** `unsafe` and a new dependency
   > were what this needed the last time somebody looked, and the standard library had moved.

   So what remains is **6**, which is a scope decision rather than a task.

   *A list inherited is a list nobody checked*, and this document is where that gets paid.
7. ~~One measurement outstanding~~ — **closed 2026-09-08.** Both adapters send the plain
   Windows path form; Codex's `cwd` and `writableRoots` come from one helper, and an allowed
   write was watched landing in the installed window afterwards.

**A known flake, reported rather than filed away:** `ran::tests::really::code_that_passes_its_tests_passes`
failed 2 of 7 full-workspace runs on 2026-09-07 — a Python assertion inside a temporary
directory, unrelated to anything either session changed. Worth a look before it is trusted as a
gate.

Out of the critical path, and to be said in the announcement rather than hidden: **Linux is
unmeasured** (no machine), the **963 kB JavaScript bundle**, and the **two large files**
(`workflow.rs` 6,514 lines; `workshop.rs` 5,025 with application logic inside Tauri) — deferred
to post-release by agreement.

The lettered phases below are kept with their dates because each carries something worth not
relearning. **All of them are closed.**

### Phase A — Concurrency *(done 2026-08-26, `310eae4`)*

**It is `ROADMAP.md`'s Phase 12½ — Two at once**, and the way this entry read until 2026-08-30
is worth keeping in mind while you read the rest of the list. It said *the only one of these
still open* — four days after the work shipped. The section beneath it described, step by step,
things that were already in `state.rs`.

**How that happened is the ordinary way.** This list is inherited from handover to handover. On
2026-08-29 two of its entries were correctly re-measured and closed; this one was not, because it
was the one being recommended as next, and a recommendation reads as fresh. It was then copied
into `ROADMAP.md` as a new phase.

> **Re-measure the entry you are about to act on first, not last.** The cost of a stale entry is
> paid by whoever trusts it most, and that is always the person doing the next piece of work.

**What was built.** `waiting` and `running` are maps keyed by character; `Working` remembers whose
turn it is and lets go of its own, so a finished turn cannot take another's Quest away
(**misfiled evidence is worse than missing evidence** — ADR-0025); an answer names the character
it answers for; a second turn *for the same character* is refused rather than queued; and the
Chronicle's caret names whoever is speaking rather than whoever you are looking at.

**Measured in the window:** Mage on `gemma4-12b` through llama.cpp and Paladin on a second Claude
Code sign-in, two Quests, both cards reading WORKING, each answer landing in its own conversation.

**What is still out of scope on purpose:** two characters *drawing* at once. ComfyUI queues.

### Phase B — The two Phase-11 debts *(done 2026-08-25)*

- **11.8 — Styles derived.** A Style exists because a workflow serves it and stops existing when
  the last one is taken away; `GENERAL` is the exception, because it is the absence of a style.
  Naming one on a workflow row (`IT DRAWS…`) is what creates it — Epoch never reads the name off
  the file. The migration drops the names that shipped dark and keeps any somebody attached to.
- **11.10 — the licence at export.** An export marks the files its declared licence was not
  written about — artwork the user imported — and **says it rather than refusing**: the hard rule
  governs what Epoch *distributes*, and Epoch cannot read what a picture is or who made it.

### Phase C — Phase 11½ *(done — A 2026-08-26, B 2026-08-27/28)*

Both halves were found by using the product and both are the owner's stated next want.

**Remember a recipe that worked.** Epoch refuses nothing — the panel draws whatever is picked — so
compatibility is not the problem; *guidance* is. Detection rules cover six families and will never
cover an open set. When a picture is drawn successfully, Epoch already holds the whole answer:
this model file, these encoders, this family string, this VAE, and a graph that ran. Filed by
**hash** (ADR-0032 already owns identity by hash), the next person to choose that model is offered
what worked — evidence, not a guess.

**Settled 2026-08-25, before building** (ADR-0032's amendment): a recipe belongs to the **machine**
and never to the character — it fails ADR-0026's *survives changing the engine* test, so it is
`num_gpu` rather than `temperature`. It lives in **`epoch-models`** beside `generative.rs`, because
both surfaces need it and ADR-0029 §9 forbids EpochServices from linking `epoch-engine`. It does
not travel: the machine that drew keeps it. A refusal is the same record with its outcome, bounded
to refusals **about the configuration** and worded as *this combination was refused*.

**Upscalers have a consumer** (2026-08-26). `UpscaleModelLoader` + `ImageUpscaleWithModel`,
measured off the live server, between the decode and the save — it enlarges the picture that was
made rather than making another. Marked, never fenced off; absent entirely on a machine with none.
**Not yet drawn with here**, because nothing is installed to draw one with.

**ControlNet is in the panel** (2026-08-27): a reference row per steering, each asking what the
ControlNet should read, with `Canny` filtered against what the server actually has and *it is
already a control map* always offered. Nothing is preselected — which preparation a ControlNet
wants is a property of the ControlNet, and the only thing naming it is the filename.

The older note here said the panel had no path for a reference image and `draw_image` did.
`draw_image` was deleted on 2026-08-28 (ADR-0030's fourth amendment); the panel is the only path
there is, and it has one.

### Phase D — Phase 12, long work *(closed 2026-08-29)*

**Video works, end to end, measured in the window.** VIDEO beside IMAGE in the Studio Panel,
`WAITING` on the crew card within the second, a video playing in the Chronicle 40.3 s later.
Long work has its first producer; the Job machinery did not change to accept one.

Three things are worth carrying forward:

- **One family, `LTXV`,** because one has been measured. Wan and Hunyuan have nodes on this
  server and no model, so their recipes would be memory. `PREPARATIONS` grows the same way.
- **`Loading::Encoded`** — a checkpoint that carries its model and its VAE and not its encoder.
  Found by drawing: ComfyUI answers *"clip input is invalid: None"*. Most video models ship this
  way, because the encoder is a T5 the size of the model.
- **A picture is still drawn on the calling thread.** 12 to 35 s with somebody standing there.
  Making it a job for consistency would buy nothing.

**Nothing is left in the phase.** Audio landed 2026-08-28 — two families, and ACE-Step writes
90 s of music with lyrics in 8.0 s. 3D landed 2026-08-29, and its stated condition was **met
rather than waived**: `polyscope-rs` renders headless, so a mesh is turned through 24 frames into
a GIF the picture path already carries. The MAKE row is `IMAGE · VIDEO · AUDIO · 3D` and none of
it is cold.

Two more things worth carrying forward from those two:

- **A result key belongs to a recipe, not to the server.** `SaveAudio` reports under `audio` and
  `SaveGLB` under `3d`; reading `images` and only `images` made a perfect FLAC arrive as nothing.
- **A job's optional last step must not be able to take the job down.** A 2.8M-face mesh made
  wgpu panic inside the turntable and the crew card read `WAITING` for twenty minutes over a model
  that was already finished in the vault. See `CLAUDE.md`, last section.

---

### Phase D½ — The window left the character *(done 2026-08-31)*

`ADR-0026`'s own open question doubted `context_tokens` — *"the weakest member of the set"* — and
the Models deck settled it. The amendment is in the ADR; what shipped is the whole migration,
seven points, closed in two commits.

> **NPC chooses the Brain. MODELS decides how that Brain runs. Context Composer decides what that
> Brain sees.**

| | |
|---|---|
| **Kernel** | `ContextPolicy` (Compact · Adaptive · Long · Custom) and `effective_use`, which enforces `Required ≤ Effective ≤ Window`. `context_tokens` survives as a **legacy ceiling a character may lower and never raise**, and no surface writes it. |
| **Engine** | `inherits.rs` — `impact_of` lists every character on a Brain *before* a profile is applied, `badge` gates the profile list by what the context benchmark measured, `Inherited` is the read-only view. |
| **Surface** | The character panel's `Context tokens` box is gone. In its place: a reading, its provenance, and a door to MODELS. |

**The two things worth not relearning:**

**A window has to say whose it is.** `64K` means three different things — a profile Epoch
searched for, a loadout somebody chose, or a number the backend reports about its own model — so
`Source` travels with it. And **only a profile carries a speed**: a model's last timing was taken
at whatever loadout was current that afternoon, so printing it beside a window chosen afterwards
would be a real reading of a different configuration. A Brain nobody has configured reads
`Not configured in MODELS` and borrows nothing.

**`Inherited` deliberately has no `effective` and no `policy`.** Both were on it for a day with no
consumer. `None` would have meant *nobody measured the Required blocks* here and *this character
cannot run in that window* on `Consumer`, one file apart — two spellings of one fact. Measuring a
character's Required cost is the Composer's job and needs a turn to do it against, which is why
`impact_of` takes a closure rather than a number.

**And the defect the window found, which no test could.** A character on llama.cpp carries the
name the router serves it under — `gemma4-12b` — while the Models deck and the profile store both
key on the shelf's own name, `gemma4:12b`. Every model without a colon in its name matches either
way, so the panel read `Not configured in MODELS` for a model with a 64K loadout on the deck, and
would never have found a profile applied to its own Brain. Both sides are compared through
`shelf_name` now, which works only because shelving is idempotent — asserted, because that is the
property the fix rests on.

**And the one the same click found next.** The reading is a shelf read, so it takes a moment —
and until it arrived the panel drew `Not configured in MODELS` for a model with a 64K loadout,
correcting itself a second later. **The wrong answer was the one on screen at the moment somebody
opens the section**, and it wore the shape of a finished measurement. `null` means *not asked*
everywhere else in this codebase; it had never been given a shape on a surface. It has one now,
and the door waits with it — addressed by the name the reading returns, so it cannot travel with
a spelling the deck does not hold.

**And a cost that was inherited rather than chosen.** The panel's reading called the Models
deck's own read, which is **15.7 s in the running window** — `pair_up` reads every GGUF header
and `deck::about` reads them all again. Split, on this machine with six models
(`vault/measurements/shelfread.*`, which the vault's `.gitignore` keeps out of the tree, so the
numbers are here too): `ollama_weights` 0.7 ms · `gguf_in(vault)` **11.3 s** · `deck::about`
**6.2 s** · `Machine::measure` 0.28 s · `llama --cache-list` 0.18 s. That is a page somebody opens
on purpose; a character panel is not. `optimized.json` answers the same question
for any measured model in four milliseconds, so the profile half now comes from Epoch's own file
and the shelves are walked only when nothing has been measured. **The 15.7 s itself is still
there and is now written down** — the MODELS deck pays it every time it opens, and reading each
header once rather than twice is most of it.

**Still open from the seven points**, and neither needs a graphics card:

- `inherits::Impact` is built and tested and **is not yet wired into the apply-profile flow** —
  the confirmation that lists every character on the Brain before a window shrinks under them.
- `inherits::badge` is built and tested and **is not yet drawn in the MODELS configuration list**;
  `MAX LOADABLE` is meant to sit behind `Advanced`.

### Phase D¾ — A benchmark reference is an entity *(done 2026-08-31)*

The machine was rebooted, the paused benchmark resumed, and Epoch answered *"the control
reproduces: 40.1 tok/s. This machine is itself again."* **It had compared nothing.**

`may_begin(rate, None)` returned `Ok`, because a machine with nothing measured must be allowed its
first search — which is true, and which made *I have never measured this* indistinguishable from
*I checked and it is fine*. It then cleared the paused session and offered to run twenty minutes
of measurement.

> **A permission may default to yes; a verification may not.** *A gate with nothing behind it must
> stay open* is a rule about not withholding a capability. A safety check is the opposite case.

**The number it should have used was in the same record.** A pause stored `clean_reference: 46.0`
as a bare `f64` while the gate read a separate `references` list that only `optimize` ever writes
to — one fact in two places, and the gate read the vacant one.

**Pointing the gate at 46.0 would have been the second mistake.** Re-measured with the fix in
place: **46.7 tok/s over 3 runs**, at the historical figure. The 40.1 was one median taken minutes
after a reboot. Had the lookup simply been fixed, Epoch would have declared a healthy machine
degraded — which is why the answer is provenance rather than a better lookup.

| | |
|---|---|
| `reference.rs` | `PerformanceReference` + `Fingerprint`: artefact, build, **GPU backend**, card, driver, OS, context, cache, flash attention, speculation, offload, workload + version, sampler, runs, dispersion |
| `Comparison` | **Comparable · Differs · Incomplete** — silence about a field is never evidence it matched |
| `State::ReferenceRequired` | *I cannot tell*, as distinct from `RecoveryRequired`'s *I know and I am not there* |
| `CALIBRATE` | a separate press: a check that minted the reference it failed to find would pass the next check by construction |
| legacy figures | kept, labelled, demoted — `46.0` is real information and may not gate |
| `stop_holding` | a benchmark is not a turn: it hands the card back |

The two llama.cpp builds here are the whole argument for the fingerprint: same GGUF, same card,
**4.5× apart**, because one is CUDA and one is Vulkan. Anything comparing across them reports a
package as a degraded machine.

**Where it stands.** `Qwen3.6-35B-A3B-UD-IQ4_XS` is `REFERENCE_REQUIRED`. The only reference on
file is the legacy `46.0`, which carries artefact, card, build and runtime and nothing else. The
next step is one CALIBRATE press — not a search.

### Phase D⁷⁄₈ — STANDARD_CONTROL_V2 *(done 2026-09-01)*

Two identical calibrations, run back to back with nothing changed, produced **39.68** and
**41.90** tok/s — three runs each, **run sets that do not overlap** (A max 40.50, B min 41.44),
and every other reading the same: placement, shared GPU memory, VRAM, RAM, prompt throughput,
TTFT. A clean-state check of the same nominal thing had read **46.7** an hour earlier.

**No cause is claimed.** Three cells, n = 1 each, not a factorial experiment. What it proved is
that the instrument had two unnamed variables, and the evidence is in
`vault/measurements/calibrate_ab.out`.

| what was wrong | what it is now |
|---|---|
| three near-duplicate measurement paths | one `measure_standard_control(protocol)` |
| the instrument was not recorded | `measurement_protocol` on the fingerprint — **not** `instrumented: bool` |
| process lifecycle was whatever happened | `Lifecycle::Fresh`, obeyed and recorded on a `Sequence` |
| warmup was undefined | 1 discarded run, kept as evidence beside the measured ones |
| 3 measured runs | 5 |
| one-sided gate | `Reproduction`: Reproduced · BelowReference · AboveReference · Incomparable · Incomplete |
| "spread ±5.5%" (it was the gate band) | `Spread` — observed range, max deviation, MAD, each named |
| `version: 0.3.0-dev (…)` as identity | `identity::Build` fields; the sentence is built for display |
| `format!("{:?}", speculation)` | `identity::speculation_id` — a `Debug` impl was part of a record's meaning |
| store keyed on the fingerprint | `R-001`, `R-002`, … immutable; `history_of` keeps all, `latest_reference_for` indexes |
| a run became an authority by running | `Observation` → `validate` → `PerformanceReference`; a refusal keeps the evidence |

**Two rules worth carrying:**

> **A control is only comparable if its lifecycle and warmup protocol are comparable.** *A first
> load is not a latency* one layer out.

> **Reproduction is equivalence, not "at least as fast".** A control 18% above its reference has
> not reproduced it. `AboveReference` is not a pass — and it is not `RecoveryRequired` either,
> because faster is not evidence of damage.

**A and B demote themselves.** Their fingerprints have no `measurement_protocol`, which is now
required, so `is_safety_reference()` is false: kept as history, unable to gate. No purge needed —
the required field did it.

**Where it stands.** `Qwen3.6-35B-A3B-UD-IQ4_XS` is `REFERENCE_REQUIRED`. Nothing may gate. The
agreed next step is **C1 · C2 · C3 under V2**, each a fresh process, comparing the three before
minting anything — and only then a RETRY. No search until a control reproduces a valid reference.

### Phase D⁹⁄₁₀ — The instrument works, and it found something *(2026-09-01)*

`STANDARD_CONTROL_V2` was built, validated, and then used to measure one experiment six times in
three hours. Every run: same binary, same artefact, same 32K/f16/FA-auto/CUDA fingerprint, fresh
process verified by distinct router PIDs, protocol execution MATCH.

| | median | within-range | GPU/CPU | warmup |
|---|---|---|---|---|
| O-001 07:41 | 51.81 | 1.00% | 30.3 / 42.6 | 49.17 |
| O-002 07:57 | 51.60 | 1.45% | 32.8 / 42.3 | 48.90 |
| O-003 08:02 | 51.19 | 1.83% | 33.6 / 36.1 | 48.44 |
| *retry 08:29* | *48.04* | *2.21%* | — | — |
| O-004 08:39 | 42.75 | 4.39% | 37.7 / 47.1 | 41.59 |
| O-005 09:53 | **38.58** | **5.24%** | 40.7 / 55.1 | 39.14 |

**−25.5%, monotonic, and ten minutes of real idle did not interrupt it** — O-005 came after 29
minutes with nothing loaded and fell another 9.7%. GPU temperature was 38 °C throughout, so it is
not thermal. Shared GPU memory grew 0.16 → 0.30 GB, two orders of magnitude below the eviction
event of 2026-08-31, so it is not that either.

**What is claimed:** `LOWER PERFORMANCE STATE PERSISTS THROUGH IDLE`. Within-process dispersion
grew 5× and crossed the one-configuration threshold at O-005; GPU and CPU utilisation rose while
throughput fell. **What is not claimed:** any cause. Time, use, driver state, memory
fragmentation and twelve load/unload cycles of a 17.7 GB model all moved together.

**`R-001` is marked `TemporalCoverageInsufficient` and its numbers are untouched.** It is a true
summary of the three processes it was built from and a poor description of what this machine does
over hours. The band was **not** widened — widening it would have destroyed the only instrument
that detected this.

> **A reference can be correct about its sources and stop describing the machine.** Those are
> different failures and only one is fixed by re-measuring.

**And three properties had to be separated**, because this machine is excellent at two of them:
within-process stability (1–5%), between-process reproducibility (1.2% over twenty minutes) and
**temporal stability** (25% over two hours). A reference built only from the second is confidently
wrong about the machine.

**Phase 1 shipped with it:** every valid standard control now produces an Observation, whatever
errand asked for it — `calibration`, `retry`, `openingControl`, `sentinel`, `recovery`. The rule
came from a control that read 48.04 with five runs and a MATCH and existed only as a sentence on
a screen, because the path that took it was a comparison rather than a calibration.

**The operational consequence, and it matters more than the cause:** a search that opens at 51.6
and reaches its third sentinel at 42.7 must **stop and classify the state**, not measure twenty
candidates and attribute the loss to them.

### Phase E — Phase 12¾, the deck tells the whole truth *(open, asked 2026-08-30)*

Three things from driving the Models and Creations decks on both machines, and they are one
phase because they share a rule: **a deck must say what it can do, in the words of the thing
that can do it.**

- **MEASURE on a chosen runtime.** TIME IT offers one; MEASURE cannot and now says so. A curve
  is eight `llama-server` flags, and the loadout it produces is only consumed by llama.cpp's
  presets — so the order is *measure what Ollama and LM Studio accept*, then *teach Epoch to
  apply a loadout to them*, and only then offer the picker. Offering it first is the dead
  `Manual` mode of ADR-0027 in a second deck. The table of what each backend allows is in
  `ROADMAP.md`, with the half that is documentation rather than measurement marked as such.
- **A preview beside a search result.** Both catalogues publish the image. The CSP allows no
  remote host, and the answer is `epoch://` rather than widening `img-src` — the narrower door,
  which is what pictures already use.
- **Filter the catalogue by medium.** The shelves are filtered; the search is not. Derived from
  what the source said — Civitai's base, Hugging Face's pipeline tag — and a result nobody
  placed stays visible under every medium, as an unreadable file already does.
- **A bar while something downloads.** Today a 6.9 GB fetch says *this takes as long as it
  takes*, which is true and is not a reading. No new mechanism: `catalogue::fetch` already loops
  over one-megabyte chunks and the catalogue knows the size — the shape to copy is the loadout
  search's `&dyn Fn(usize, usize)` and the bar it already draws.

**All four are two surfaces.** The Host and EpochServices both search, both install and both
list a library, and this session was billed twice for building on one and not the other. The one
constraint over there is `script-src: 'none'`, which shapes them rather than blocking them: an
`<img>` needs no script, a filter is a form, and a progress bar is server-rendered and redrawn by
the `<meta http-equiv="refresh">` that page already uses for a download in flight.

### Phase F — Phase 13, AMD Build *(open; the build itself is done)*

**Asked for by the owner on 2026-08-30, and it belongs before the release rather than inside it.**
Everything Epoch knows about a graphics card was measured on one NVIDIA machine and one Apple M2.
`engines.rs` reads `rocm_v7_1` out of Ollama's own backend directory here and has never seen a
machine where ROCm is the one that matters; `Machine::measure` runs `nvidia-smi` and `sysctl` and
nothing else; whether a quantized attention cache works on ROCm or Vulkan is written down nowhere,
because nobody could measure it.

`AMDBuild/` holds the installer and the measurement plan. **Not** the Phase 16 installer — no
detection, no WebView2 check, no uninstaller — the bundle Tauri already produced, put where a USB
stick can reach it.

**Building it found the defect that would have wasted the trip.** `tauri.conf.json` declared no
`resources`, so an installed Epoch would have opened with no Worlds and no asset kit: `packs/` and
`assets/` sit beside the executable and neither was in the bundle. Fixed, and verified by listing
the installer's own contents rather than by trusting the build.

**No vault travels with it** — `paths.rs` sends an installed build to `%APPDATA%\Epoch`, so that
is a guarantee rather than a step somebody has to remember.

What comes back goes into Phase 13 of `ROADMAP.md`, beside the readings taken here, and **the
exact words of anything that refused**: a program's own sentence is the only true account of why
it said no.

### Phase G — Phase 14, The Universe *(open)*

Two doors, then artwork.

1. **World Edit sets the skin.** The `[[ui]]` concept pipeline exists and is used; what is missing
   is any way to reach it that is not hand-edited TOML.
2. **World Edit gives a World sound.** Music requested as a **mood, never a track** — and a pack
   supplying a sound **overrides** a synthesised voice rather than filling an empty slot, because
   `sfx.ts` deliberately ships no files. Phase 12's audio half is the machinery that makes this
   answerable rather than aspirational.
3. **The artwork** — drawing, composing, writing. Not engineering, not in this repository, and
   governed by the hard rule: original, commissioned or CC0. **Never generated** (`CLAUDE.md`:
   *assets are authored, never generated*).

### Phase H — Phase 15, The crew has a voice, and so do you *(open, planned 2026-09-03)*

Whisper for the ear, Piper for the mouth, a Voices Workshop shelf, and two switches rather than
one. Both repositories were cloned and read rather than summarised; the full plan, the comparison
that put Piper first, and the three measurements that come before any code are in `ROADMAP.md`.
**Computer use was proposed with it and cancelled by the owner in the same conversation.**

### Phase I — Phase 16, Somebody else can install it *(open, last by the owner's decision)*

Installer, licence, crash surface, `CONTRIBUTING.md`, and the GitHub push — which belongs here
because a push is *distribution*, not a backup. Git has been the backup all along, locally.
**whisper.cpp (MIT) is offered here**, alongside Ollama, Claude Code, Codex, ComfyUI and Git:
offered, never imposed.

---

## Known gaps, stated rather than hidden

- **Upscalers** are composed into the graph since 2026-08-26, though **no picture has been drawn
  through one on this machine** — nothing is installed to draw one with. ControlNets were on this
  list beside them and came off it on 2026-08-27: they have a row in the panel, a preparation
  step, and a test that follows every wire to the class on the other end.
- ~~**Video models** can be filed and cannot be drawn with.~~ **Closed 2026-08-28** — the panel's
  VIDEO tab makes one, `epoch://` serves MP4 and WebM, and the Chronicle plays it. What remains
  true and narrower: a video model of a family other than LTXV is filed and cannot be drawn with,
  because only LTXV has been measured.
- ~~**`gpt-oss-20b`** fails to load on llama.cpp and LM Studio. Never diagnosed.~~ **Moot 2026-09-02 — the model was deleted from this machine and the note outlived it.** Kept struck through rather than removed: the owner had to correct a report built on it, which is what a stale record costs. Nothing was diagnosed and nothing now depends on it.
- **Linux credential store** — parked by the owner, and now also **unmeasurable here**: there is
  no Linux machine, and the audit asks for it before support is promised.
- **Nothing is signed.** A certificate is a recurring cost the owner has decided against for now.
  What ships instead is `SHA256SUMS` plus Sigstore build provenance, and both READMEs say what
  each platform will warn. SmartScreen and Gatekeeper still warn; nothing free changes that.
- **82 Rust tests are `#[ignore]`d** and have never been triaged. The audit flagged the number;
  nobody has yet asked how many are environment-gated and how many are claims nobody can check.
- **The macOS bundle is `adhoc, linker-signed`** and `spctl` refuses it. Note that a copy built on
  the Mac carries **no `com.apple.quarantine`** — so testing "does it open" there has never taken
  the path a downloader takes.
- ~~A **Markdown image in a message** would render if Rich Chronicle projects it.~~ **Closed** —
  and it had been closed before this note was written. `MARKDOWN_LINK` carries a negative
  lookbehind on `!` (`RichChronicle.tsx`), so an image a model writes stays literal text and
  nothing in that file promotes it. A picture in a Chronicle still comes only from
  `Entry::Produced` (ADR-0030, third amendment). Left visible rather than deleted: this note was
  read months later as a live gap and put on a list of work, which is the cost of a record that
  describes the past as the present.

---

## Where things live in `epoch-tauri`

`state.rs` was 11,134 lines and is now seven files. Each module is a child of `state`, so it still
reaches the World's private fields; each is re-exported, so every `state::X` path a surface uses
resolves exactly where it did.

| | | |
| --- | ---: | --- |
| `state/turn.rs` | 2803 | what a character is given, who answers, what the answer may do |
| `state/studio.rs` | 2276 | the picture chain: the panel, the easel, the composed graph |
| `state/workshop.rs` | 5025 | the Generative Library and the two catalogues that fill it |
| `state/crew.rs` | 686 | the people, and what thinks for them |
| `state/map.rs` | 439 | the geography, and undoing a change to it |
| `state/machines.rs` | 211 | other computers a turn may go to |
| `state.rs` | 4512 | the World itself, its lock, the views, and what has no other home |

**Put new work in the module that owns its subject.** The reason the file reached eleven thousand
lines is that there was nowhere else obvious, and one more thing in `state.rs` was always the
cheapest move. It is not any more.

~~**A caution before running `cargo fmt`.** The repository has pre-existing rustfmt drift…~~
**Gone since 2026-09-07**: `cargo fmt --check` passes and CI fails on it, so a blanket `cargo fmt`
is now a no-op rather than a landmine. Run it freely.

---

## Machines

- **This PC** — Windows 11, RTX 4070 SUPER 12 GB. ComfyUI at
  `%LOCALAPPDATA%\Comfy-Desktop\ComfyUI-Installs\ComfyUI\ComfyUI`. llama.cpp's CUDA build lives in
  `~/.llama/bin` and is preferred over winget's Vulkan-only one — 4.5× measured.
- **The MacBook** — `10.0.1.20`, Apple M2, key at `~/.ssh/epoch_mac`. EpochServices runs from
  `/Applications/EpochServices.app`; rebuild the bundle or your work is invisible. Deploy with a
  git bundle over `scp`.
- **Never ask the owner for a password.** The key is there; if it stops working, ask them to run
  the command themselves.
