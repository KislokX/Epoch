# Repollo — what is waiting on the owner

*The codeword means the owner is back at the machine and can run Epoch. Everything here needs a
person in front of the app, or a decision only they can make.*

Nothing in this file is a task Claude can finish alone. Anything that could be finished alone is
in ROADMAP.md instead.

---

## Decisions

### Which Services to add *(asked 2026-08-19)*

Epoch hosts three today: Claude Code, Codex, Gemini CLI. Candidates split into two shapes with
very different costs.

**Shape A — speaks HTTP. Already works, zero new code.** The OpenAI-compatible backend landed in
Phase 8 step 1, so each of these is an endpoint entry and nothing else:

| Service | endpoint |
|---|---|
| LM Studio | `http://localhost:1234/v1` |
| llama.cpp (`llama-server`) | `http://localhost:8080/v1` |
| vLLM | `http://localhost:8000/v1` |
| Jan · LocalAI · KoboldCpp · text-generation-webui | their own |
| Ollama | already native |
| DeepSeek · GLM/Z.ai · Groq · Together · OpenRouter · Mistral · xAI | their URL plus a key |

**Shape B — owns its own loop. One Rust module each**, like `agents/gemini.rs`.

| candidate | what its docs claim | estimated cost |
|---|---|---|
| **Antigravity CLI (`agy`)** | `-p` plus `--output-format stream-json` | **cheap** — nearly Gemini's shape |
| opencode | headless by design, agents per model in one config | medium |
| Cline CLI | runs the Cline agent from a terminal | medium |
| Sourcegraph Amp | CLI and IDE | medium |
| Cursor (`cursor-agent`) | headless | medium |
| OpenHands | headless for CI, but Docker-shaped | expensive |

**Everything in Shape B is what the documentation says, not what Epoch measured.** With Gemini,
two of the three facts that mattered would have been wrong from memory. The list is candidates.

Recommendation: **`agy` first** — the owner already uses Antigravity, the shape is nearly
identical to Gemini CLI's, and the install is one command. The IDE ships **no** CLI (measured:
`Antigravity.exe` and a 144 MB language server, no shim, no PATH entry), so `agy` is a separate
install.

Sources: [Antigravity CLI headless](https://antigravity.google/docs/cli/headless) ·
[install & auth](https://antigravity.google/docs/cli/install) ·
[headless agents compared](https://www.developersdigest.tech/blog/headless-ai-coding-agents-ci-comparison-2026)

---

## Visual checks nobody but the owner can run

The Launcher and the World only run inside Tauri; the dev server in a browser stops at the boot
screen because `invoke` is not there. None of this can be verified from here.

### Phase 6 — the crew is alive

1. Character editor → an **Animations** door under Sprite and Icon. The layout does not overflow.
2. With no grid stated, IMPORT is disabled. Cols and Rows alone are still not enough; adding ms
   enables it.
3. Import a 4×4 walk sheet → the preview **plays**, cycling all four directions.
4. Set Rows wrong on purpose → **RE-CUT** appears, and fixes it without re-uploading the file.
5. Type an invented direction → IMPORT disables before anything is uploaded.
6. Speak to a character → *thinking*; when a tool runs it becomes *work* and the sentence changes
   to the tool's name.
7. Hand work to somebody in another building → they walk, and with a `walk` sheet they play it in
   their heading's row, with no CSS bob on top.
8. A character with no sheets still walks exactly as before — bob and shadow.
9. A handover → about a second's pause **beside the person who handed it over**.
10. A handover where the giver leaves first → the same pause, but alone.
11. A routine walk → the same brief pause on arrival; nobody snaps from walking to reading.

### This session

12. Close a Quest with X → the vault note reads **"finished"**, not *"you ended the
    conversation"*.
13. A long conversation while a model writes → **no stutter**. It was 138 ms per token.

### Phase 7 — both built; one needs eyes

14. **CSP is on.** The app runs 150 s under it with no panic, which is all that can be proved
    from here — violations go to the webview console. The check is five seconds:
    **does the HUD still have its textures, and do the sprites draw?** Those are the two rules
    the audit said were load-bearing (`img-src data:`, `style-src` inline). If the World looks
    right, the policy is right.
15. **Regenerate**, in Settings. Pressing it closes every open door — so afterwards, connect an
    agent again and check it still reaches Epoch's tools. And that the project's `.gitignore`
    gained `.mcp.json` with the line saying why.

### Removing a character

16. Character editor → **REMOVE FROM THE CREW** at the bottom, set apart from SAVE. Three lists
    in gold — deleted, changes, survives — and REMOVE stays disabled until the name is typed
    exactly. Check the counts are real: *"N conversations in this World name them"*.

### Removing a World, erasing data, and the Service cascade

> **17 is now also a test.** `removing_a_world_keeps_your_documents.rs` builds a real vault on
> disk — an Obsidian folder with `.obsidian/`, the user's own notes, and the `Epoch/` folder —
> points a World at it, removes the World, and asserts that every file Epoch made is gone and
> every file the user made is there byte for byte. It also asserts that the removal **plan never
> names** one of the user's files, because checking only the aftermath would pass for a plan that
> meant to delete a note and merely failed.
>
> The manual pass below is still worth doing once: the test covers the half that touches disk,
> not the four other things `remove_world` does. **A disposable vault is waiting at
> `Downloads\Test2`** — `.obsidian/`, `pepe.txt` and `Notes/mine.md`. Point a throwaway World's
> library at that instead of at your real vault.

17. Launcher → Worlds → pick one → **REMOVE**. The plan should say how many conversations go,
    who stops living there, how many permissions are forgotten — and that **your library and your
    project folder are untouched**. Type the name, remove, then **check your Obsidian vault is
    still there, `Epoch/` folder and all.** That is the one that matters.
18. Try it on **Default World**: it should say the World itself stays because everything is built
    from it, and only its conversations and map go.
19. Settings → **Erase data**. Every row with a real size. Tick the models catalogue, erase,
    reopen the Workshop and check it refetches. Leave **Agent conversation handles** alone unless
    you want to test it — it ends the threads your agents are holding.
20. Character editor → the field is **Service** now, and lists Ollama *and* Claude Code, Codex,
    Gemini together. Pick a different Service: the model should clear rather than carry over.
    The model list should hold only that Service's models.


## 21 · Pair a machine, end to end

Needs two machines on the same network. On the remote: `cd EpochServices && cargo run`.

1. Epoch → **Services** → **SHOW A CODE**. It shows six characters and an address ending `:11501`.
2. On the remote, in the EpochServices window: type the code and the Host address, **CONNECT**.
3. Within two seconds the machine should appear in Epoch's list, named after itself, with
   *may think for me* ticked. **Nothing was typed into Epoch.**
4. The remote's window should now read **Connected** and say how many models it lends.
5. In Epoch, a character's Provider list should now offer that machine. Pick it, pick one of its
   models, and say something. The answer arrives whole (no typing animation) — that is correct.
6. **CANCEL** on a shown code, then try redeeming it: EpochServices should say the code is wrong
   or expired rather than hanging.
7. **DISCONNECT** on the remote, then ask again from Epoch: the Provider should read OFFLINE with
   a note, not crash the turn.


## 22 · Gemini CLI says which model actually answered

One machine. `gemini` is installed and signed in.

1. Characters → give somebody the **Gemini CLI** brain. Leave the model blank.
2. Ask them anything that needs a file read ("what is in this folder?").
3. While it works, the crew card shows the brain: **Gemini CLI**.
4. When the turn ends, a second, quieter line appears under it:
   `thought with gemini-3-flash-preview` (or whichever it routed to).
5. Ask a character with a **Claude Code** brain the same thing. **No second line** — that CLI
   does not report this, and inventing one is the thing being avoided.
6. Hover the line: it should say the agent reported it after the turn.


## 23 · Gemini CLI stops reading as dead

1. Connections → the **Gemini CLI** row. It should read **READY** with the light on, and the
   middle column should say **Gemini API Key** — the same words its own `/about` shows.
2. The button should say **CHANGE SIGN-IN**, not SIGN IN. Somebody already configured is not
   signed out.
3. The line under it should say Epoch could not *verify* it. That is the honest half: a chosen
   method is not a working credential.
4. Claude Code's row must be unchanged — **ONLINE**, with the account. It reports a real session
   and must not borrow the weaker word.
5. Rename `~/.gemini/settings.json` and press **CHECK AGAIN**: the row must fall back to `—`
   and "could not ask", never to SIGNED OUT. Put it back afterwards.


---

## 24 · Connect the MacBook Pro (M2, 16 GB) as a thinking machine

The end state: the Mac runs Ollama and EpochServices; the Windows PC runs Epoch and can pick the
Mac's models for any character. The Mac never installs Epoch.

**Two ports, and they are not the same one.** Windows listens on **11501**, and only while a
pairing code is on screen. The Mac listens on **11500**, always, once it is running. Each side
needs its own firewall opening, in opposite directions.

### 24.0 · Built and measured on the machine itself (2026-08-19)

**This whole section stopped being a plan.** The MacBook Pro (M2, macOS 26.6.1, arm64) was
reached over SSH and everything below was run on it:

| what | result |
|---|---|
| `cargo tauri build` | **`EpochServices.app` + `EpochServices_0.1.0_aarch64.dmg`, 3.75 MB** |
| `cargo test` | **17 passed**, including two that had never executed anywhere |
| the program, running | `GET /have` without a bearer → **403**; the page reads **17 GB memory** |
| Ollama | not installed, and the page says so: *"None. Pull one with `ollama pull ...`"* |

The two tests that had never run are the ones that only exist off Windows:
`the_bond_is_written_readable_by_this_user_and_nobody_else` (`#[cfg(unix)]`, asserts `0600`) and
the no-NVIDIA case, which had only ever been checked on a machine that *has* an NVIDIA card. Both
pass. **17 GB** is `sysctl hw.memsize` on a 16 GB machine, which is the same number said in
decimal — the unified-memory path is real rather than a code path nobody had exercised.

Two things measured that correct what was written here earlier:

- **The `.dmg` built on that Mac carries no `com.apple.quarantine`.** Opening it from the Desktop
  needs no right-click → Open. A copy *downloaded* from anywhere does, and that is a property of
  where the file came from rather than of the bundle.
- **The app is ad-hoc signed**, not unsigned: `flags=0x20002(adhoc,linker-signed)`. Apple Silicon
  requires at least that, and Tauri applies it without being asked. What is still missing is a
  Developer ID and notarisation, which is a different thing from having no signature at all.

It is at `~/Desktop/EpochServices_0.1.0_aarch64.dmg` on the Mac, and the repository is cloned at
`~/Epoch`.

### 24.0.0 · Two routes, and the shorter one needs GitHub

**A `.dmg` can only be built on macOS.** Tauri does not cross-compile from Windows, and the
bundle needs Apple's own tooling — so somebody's Mac has to build it, and the only question is
*whose*.

| route | the Mac installs | when it is available |
|---|---|---|
| **A — download a bundle** | nothing but the `.dmg` | once this repository is on GitHub |
| **B — build it there** | Xcode CLT, Rust, the repository | now |

Route A is a macOS runner in CI doing the build once
([.github/workflows/epochservices.yml](../../.github/workflows/epochservices.yml)) and publishing
the `.dmg`, `.msi` and `.AppImage` as artifacts. It is written and ready; it needs a push, and
the push is deliberately deferred until the release. **It is not signed or notarised**, so the
first launch on macOS needs **right-click → Open** once — that instruction ships with the
download rather than being hidden.

Route B is below, and it is what to do today.

### 24.0.1 · What the Mac needs, before anything

EpochServices is not shipped as a binary yet, so the Mac compiles it — and compiling it needs
`epoch-kernel`, which lives in this repository and reads `serde` from the workspace root. So the
**repository** travels, not the folder. (Measured: `EpochServices/Cargo.toml` points at
`../BUILD/crates/epoch-kernel`, and that crate's `Cargo.toml` says `serde = { workspace = true }`.)

The tracked repository is **12.5 MB**; the working tree without `target/` and `node_modules/` is
99 MB. So the bundle is the cheap way.

**On Windows**, from the project root:

```bash
git bundle create ~/Desktop/epoch.bundle --all
```

Copy `epoch.bundle` to the Mac — USB stick, a shared folder, iCloud Drive, anything. It is one
file.

**On the Mac**, in Terminal:

```bash
git clone ~/Desktop/epoch.bundle ~/Epoch
```

### 24.1 · Install the three things the Mac is missing

Terminal, one at a time, and each is a one-off:

```bash
xcode-select --install
```

The linker. It opens a dialog; accept it and wait. If it says the tools are already installed,
that is the answer, move on.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Rust. Accept the default (option 1). Then close and reopen Terminal, or run
`source "$HOME/.cargo/env"`. Check it took: `cargo --version` must print 1.82 or higher.

Ollama: download from [ollama.com/download](https://ollama.com/download), drag it to
Applications, open it once. It puts a llama in the menu bar and listens on `127.0.0.1:11434` —
which is exactly where it should stay. **Do not set `OLLAMA_HOST=0.0.0.0`.** Avoiding that is the
entire reason EpochServices exists.

### 24.2 · Pull a model that fits 16 GB of unified memory

```bash
ollama pull qwen3:8b
```

About 5 GB, and comfortable on an M2 with 16 GB. `qwen3:14b` is roughly 9 GB and will fit but
leaves little room for anything else — start with 8b, and pull 14b later if 8b feels thin.

Check it answers locally before involving anything else:

```bash
ollama run qwen3:8b "say OK"
```

### 24.3 · Build and start EpochServices

Two ways, and the second one gives you something you can double-click afterwards.

**To just run it:**

```bash
cd ~/Epoch/EpochServices && cargo run --release
```

**To get an app you can keep** — this is route B's version of the `.dmg`:

```bash
cd ~/Epoch/EpochServices && cargo install tauri-cli --version "^2" --locked && cargo tauri build
```

It leaves `target/release/bundle/macos/EpochServices.app` and a `.dmg` beside it. Drag the app to
Applications and it starts like any other program from then on — no Terminal.

Either way the first build takes several minutes; it is compiling a webview toolkit. Afterwards
it is seconds.

The dock icon is Epoch's own for now — the two programs wear the same face until EpochServices
gets its own mark ([docs/Build/Missing Art.md](Missing%20Art.md)). Measured on Windows: the same
configuration produces an `.msi` and a `-setup.exe`, so the macOS side has everything it needs to
produce the `.app` and the `.dmg`.

A window opens titled **EpochServices**. It should say **Connect to a Host**, show the Mac's own
name and address, and list `qwen3:8b` under *Models here*. If the model list is empty, Ollama is
not running — open it from Applications.

macOS will ask whether `epoch-services` may accept incoming network connections. **Allow it.**
That is the Mac's firewall, and without it Epoch cannot reach the Mac afterwards.

### 24.4 · Open the one port on Windows

Windows blocks incoming connections by default, so the Mac cannot deliver its `Hello` without
this. **Run PowerShell as Administrator** (right-click → Run as administrator):

```bash
netsh advfirewall firewall add rule name="Epoch pairing" dir=in action=allow protocol=TCP localport=11501
```

Inbound TCP, one port. Epoch only listens on it while a code is on screen, so the rule describes
a door that is shut almost all the time.

### 24.5 · Pair them

1. On Windows, start Epoch: `cd BUILD/ui && npm run world`.
2. **Services** → **SHOW A CODE**. It shows six characters and an address like
   `192.168.1.10:11501`. Both come from this machine; nothing is typed in from elsewhere.
3. On the Mac, in the EpochServices window: type the six characters into **CODE**, and the
   Windows address into the second field. `192.168.1.10` is enough — scheme and port are filled
   in for you.
4. **CONNECT**.
5. Within about two seconds the Mac appears in Epoch's Services list, named after itself, with
   *may think for me* ticked. **Nothing about the Mac was typed into Epoch** — it reported its
   own address.
6. The Mac's window now reads **Connected** and says how many models it lends.

### 24.6 · Use it

Characters → pick anybody → **Service**: the Mac is now in the list, by its own name. Pick it,
pick `qwen3:8b`, save. Say something.

The answer arrives **whole**, with no typing animation. That is correct and not a bug: the turn
crosses the network complete, and faking a stream would be the UI inventing information.

### If it does not work

| what you see | what it means |
|---|---|
| Mac says *could not reach Epoch* | The Windows firewall rule (24.4), or the wrong address. Check the address Epoch showed. |
| Mac says *that code is wrong or has expired* | Codes last five minutes and are spent on first use. Show a new one. |
| Machine paired, but Epoch reads it OFFLINE | EpochServices is closed, or macOS never got the incoming-connections allow. Reopen it. |
| Paired and online, but no models | Ollama is not running on the Mac. |
| Nothing at all, both look fine | The two machines are on different networks, or the router isolates wireless clients from each other. Put both on the same Wi-Fi. |


---

## 25 · The Workshop weighs what it offers

1. Workshop → **Search Hugging Face** for `qwen`. Results appear, and if there are more than 24
   there is now a **page bar** under them. The panel keeps its height instead of running past the
   bottom of the window.
2. Click any Hugging Face result. It should now **weigh** — before, every one of them answered
   `no manifest ... registry.ollama.ai/v2/library/hf.co/...: 404`, which was a URL that could
   never have worked.
3. The verdict begins with a word: **RUNS HERE** in green, **TOO BIG** in gold, or **CANNOT SAY**
   when there is no video-memory reading.
4. It should also say *"the largest quantisation that runs here, of the ones published"* — a
   repository has 25 different files and only one of them was weighed. Check the size is
   plausible for your 12.9 GB card rather than the whole repository's.
5. Type a quantisation yourself: `hf.co/unsloth/Qwen3-8B-GGUF:BF16`. It should weigh **that** one
   and say TOO BIG, rather than quietly picking a smaller one.
6. Click `glm-5.1` or `deepseek-v4-flash:0731` in **Featured by Ollama**. Instead of a bare 404 it
   should say that list also names models Ollama runs in its cloud, which have nothing to
   download. Measured — `gemma4:31b` and `gpt-oss:20b` do have manifests; those four do not.

## 26 · Machines is its own deck

1. Main menu: **MACHINES** sits directly under CONNECTIONS.
2. It opens with **This machine** — your 4070 SUPER, free of total, and system memory. Measured,
   not configured.
3. Below it: **Machines**, **Pair a machine**, and **Where a turn may go** — all three moved out
   of Connections.
4. Connections keeps **Readiness** and the per-provider rows. It answers *what can think*; the
   new deck answers *on which computer*.
5. The Ship's Log on the bridge is unaffected — two components shared the name `BridgeConsole`
   and one of them was renamed.

## 27 · Import sheet explains itself

1. Character editor → **Animations** → open a slot. **Import sheet** should look *dimmed*, and a
   line under it should say `Say how many columns and rows the sheet has.`
2. Fill columns and rows only → the line becomes `Say how long a frame lasts, in milliseconds.`
3. Fill ms → the button lights up and opens a file picker. **This is the part that did nothing
   before**: it was a label over a disabled input, which looks identical to an enabled one.
4. Type `up` in Row directions → `"up" is not a direction — use north, east, south or west.`
5. Put four directions with two rows → `4 directions for 2 rows — there is one direction per row.`

## 28 · Settings talks where you are

1. Settings → **REGENERATE**. The confirmation now appears **in Settings**, right under the
   button. It used to be written into the Worlds panel, four screens away, which is why the
   button looked dead.
2. Reconnect an agent afterwards and check it still reaches Epoch's tools.


## 29 · The Workshop stops implying there are only forty models

1. Search `qwen`. Under the page bar there is now **MORE RESULTS** and a count that says
   *N fetched so far*. The count is about what arrived, never about how many exist — Hugging Face
   never tells anybody that number, so Epoch does not state it.
2. Press MORE a few times. Results **accumulate**; the page bar grows. Nothing is replaced,
   because Hugging Face pages by cursor and there is no way back to an earlier page.
3. Keep pressing until it says **That is the end of the results.** — that is a different fact
   from "we asked for sixty and stopped", which is what it used to imply.

## 30 · Downloading a model

1. Weigh something from Hugging Face, e.g. `hf.co/unsloth/Qwen3-8B-GGUF`.
2. **DOWNLOAD TO OLLAMA** appears under the verdict. Press it.
3. The line underneath is the **runtime's own words** as they arrive: `pulling manifest`, then a
   percentage of a real size, then `verifying sha256 digest`, then `writing manifest`. Not a
   spinner and not a sentence Epoch wrote in advance.
4. When it finishes it says the model is on this machine, and the featured shelf re-reads itself
   so **already here** appears where it should.
5. Try a name that does not exist. The failure is Ollama's own sentence — measured, a missing
   model answers `pull model manifest: file does not exist` and nothing else.
6. The note beside the button says llama.cpp and LM Studio take a GGUF from disk and have no way
   to be told to fetch one. **That is why there is one button and not three** — a download button
   for them would be a control that does nothing.


## 31 · Epoch asks for the firewall port

1. Launcher → **MACHINES**. Under *This machine* there is a lamp and a sentence about whether
   other machines can reach this one.
2. Press **ALLOW**. **Windows** shows its own consent dialog — not Epoch. Approve it.
3. The lamp lights and the sentence changes to say the rule stays until it is removed here.
4. Press **STOP ALLOWING** and approve again: the lamp goes out. That is the point of it being a
   switch — a firewall rule outlives Epoch being closed *and* uninstalled, so the product that
   created it has to be able to remove it.
5. Press ALLOW and then **dismiss** the dialog. The switch must stay off: it re-reads the rule
   rather than trusting that it asked.

## 32 · EpochServices weighs before it downloads

On the remote machine, in the EpochServices window.

1. Two tabs at the top: **CONNECT** and **MODELS WORKSHOP**.
2. Workshop → *This machine* shows that machine's hardware, and says every verdict is measured
   **there**.
3. Weigh `qwen3.8:27b` on the MacBook. Measured: **TOO BIG** — *"Needs 17.7 GB, and this machine
   has 17.2 GB in total."* The button reads **PULL ANYWAY**.
4. Weigh `qwen3:4b`: **RUNS HERE**, *"Fits in this machine's 17.2 GB of memory"*, and an ordinary
   **PULL**.
5. Press PULL. The page refreshes itself and shows Ollama's own words with a percentage. No
   script is involved — the CSP forbids it, and a meta refresh is older than the problem.
6. When it finishes, the model appears under *Models here* on both tabs.
7. **Search Hugging Face** for `qwen3`. Real repositories appear, each with a **WEIGH** button
   and nothing else — a name on a list has no size attached, and a PULL beside one would be the
   silent download this tab exists to prevent. **MORE RESULTS** appends the next page.
8. **Featured by Ollama** lists what Ollama promotes. Weigh `glm-5.1` from it: it should say that
   list also names models Ollama runs in its cloud, which have nothing to download.
9. The footer says **EpochServices v0.1.0**. That line exists because of the failure below.

## 33 · Two copies cannot hide from each other

1. Start EpochServices. Leave it running.
2. Start it again — from the `.app`, or from `cargo run`.
3. The second copy must show **a dialog** saying the port is taken and another copy is answering,
   and then stop. It must **not** open a window.

   This is what went wrong: it printed to a stderr nobody reads — it is a windowed program — and
   exited. The first copy kept answering, so the new window opened onto the *old* program: an
   older build, without the routes that had just been added, replying `no` to a button that had
   just appeared. Nothing on screen suggested two copies existed.
4. The version in the footer is the other half of the fix: if a window ever looks out of date,
   that line says which build is answering.


## 34 · Pair the other way round (a network that isolates its clients)

This is the path for a router that will not let the Mac start a connection to the PC — which is
this network. Nothing here needs the router changed.

**On the PC, once:** Launcher → **MACHINES** → **ALLOW**, and approve the Windows dialog. (Not
needed for this direction, but it is where the switch lives and it costs nothing.)

1. On the Mac, open **EpochServices** → **CONNECT** tab.
2. Under *Or let the Host reach out*, press **SHOW A CODE**. Six characters and a countdown.
3. Note the Mac's address, shown under *This machine* — e.g. `10.0.1.20`.
4. On the PC: **MACHINES** → the form under *Or reach out to it instead*. Type the six characters
   and `10.0.1.20`. Tick **may think for me**. Press the button.
5. The Mac appears in the list, named after itself. On the Mac the page flips to **Connected**.
6. Characters → pick anybody → **Service**: the Mac is there. Pick a model that fits it —
   `gemma4:12b` does, `qwen3.8:27b` does not — and say something.

Measured over this exact network: a wrong code is refused, the right one enrols, the bearer works,
the code is spent on first use, and `gemma4:12b` on the MacBook answered a turn asked from the PC.
