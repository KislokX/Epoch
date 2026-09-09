# AMD Build — Epoch on a machine nobody built it on

**What this is.** `Epoch_Setup.exe`, carried to a second computer with an AMD graphics card, to
find out what Epoch does on hardware it has never run on. It is **not** the release: that is Phase
15, and it adds an uninstaller, an update path and a crash surface. This one installs Epoch and
what Epoch needs, and stops there.

**Why now.** Everything Epoch knows about a graphics card was measured on one NVIDIA machine and
one Apple M2. `engines.rs` reads `rocm_v7_1` out of Ollama's own backend directory *here*, and
has never seen a machine where ROCm is the one that matters. A control derived from what was
measured cannot offer a combination that does not exist — and it can still be wrong about a
machine nobody has measured.

---

## What is in the file

**`Epoch_Setup.exe`, 31 MB, and it is the whole thing.** One program: it opens a window, says what
this machine already has, offers what it does not, installs Epoch, and installs whatever was
ticked. Epoch's own MSI rides inside it, so there is nothing else to download and nothing else to
run.

It carries the application, both shipped World Packs (`default`, `archipelago`), the asset kit and
the WebView2 bootstrapper — verified by listing the installer's own contents rather than by
trusting the build.

**It carries no vault.** An installed Epoch keeps its crew, Worlds, Quests and settings in
`%APPDATA%\Epoch`, so the AMD machine starts empty: no characters, no conversations, no
portrait, and no `secrets.dat`. That is the build's own path logic (`paths.rs`) and not a step
somebody has to remember — which is the only kind of privacy guarantee worth having.

**And nothing signs in.** Epoch opens the agent's own login in the agent's own window and holds
no key, so a fresh machine is a fresh machine.

---

## What to measure there, in order

Each of these is a question this repository currently answers from one machine.

### 1. Setup itself — the half that has never been watched

The survey has run on two machines and **no row of it has ever installed anything**, because both
already had everything. This machine has nothing, so it is the first real run of the installing
half.

Watch for: does each row tick with a time beside it; does the count read *N of M* correctly; does
`Show details` carry the package manager's own output; and **does a step that fails leave the
others running**. A failure is a fine outcome — bring back its exact words.

Then: does Epoch open. If the window is blank, that is WebView2, which now rides inside setup —
write down what it said. And the Launcher should draw a World with terrain, roads and buildings.
**If the World is empty, the packs did not arrive**, and that is worth more than anything below.

### 2. What the machine says it is

`CONNECTIONS` → `THIS MACHINE`. On NVIDIA it reads the card, its free memory and the system
memory. On AMD nobody has ever run it: `nvidia-smi` does not exist there, and `Machine::measure`
must report *nothing measured* rather than a zero. **A card reading `0.0 GB` is a defect; a card
reading nothing is the honest answer.**

### 3. Which graphics each runtime offers — the reason this phase exists

`CONNECTIONS`, per runtime row:

- **Ollama** — install it, and the `GRAPHICS` menu should list what its own backend directory
  holds. On this machine that is `CUDA 12 · CUDA 13 · ROCm 7.1 · Vulkan`; on an AMD machine
  `ROCm` is the one that should be picked, and the interesting question is whether Ollama's
  own detection picks it without being told.
- **llama.cpp** — one backend per install. Its row is a sentence: *this build is …*. The winget
  package is Vulkan-only, so it should say Vulkan, and the deck should suggest **ROCm** as that
  vendor's native path — Epoch reads the vendor out of the device string the program printed, and
  has never seen an AMD one.
- **LM Studio** — `lms runtime ls` should list a ROCm or Vulkan engine rather than the CUDA ones
  installed here.

**Write down what each one actually says**, including where a name comes back unrecognised. A
name Epoch cannot place is shown verbatim on purpose, and an unrecognised AMD backend is exactly
the case that rule was written for.

### 4. The compressed attention cache

`COMPRESS THE ATTENTION CACHE`, then `START`. A quantized cache needs flash attention, flash
attention belongs to the backend, and without it **nothing loads at all** — measured here with
it forced off:

```text
llama_init_from_model: quantized V cache requires flash_attn to be enabled
```

Epoch proves it with one real load and switches itself back off if that load fails. **This is the
single most valuable reading of the trip**: whether ROCm and Vulkan support it is written down
nowhere in this repository, because nobody could measure it.

Expect one of two outcomes, and both are results:

- it loads → the switch stays on, and AMD gets the same memory back NVIDIA does;
- it refuses → Epoch says so in the server's own words and turns it off, which is the behaviour
  being tested as much as the answer.

### 5. A turn, and then a picture

Assign a character to whatever runtime answers, and say something. Then `CREATIONS` → the Studio
Panel. ComfyUI on AMD is its own question — the deck's START command was written for a machine
with CUDA, and PyTorch on ROCm is a different install.

**A refusal here is a fine outcome.** What must not happen is a silent failure: a panel that
draws nothing and says nothing, or a character that claims to have made a picture that does not
exist.

### 6. Anything that reads as software

The immersion leaks are found by using it, never by reading it, and a machine nobody tuned for is
where they show up: a scrollbar, a text selection painted across a building, a native widget in a
pixel-art window, a spinner where the World could have shown the work.

---

## What to bring back

Notes and screenshots, and the exact words of anything that refused. A program's own sentence is
the only true account of why it said no, and rewriting it from memory is how a measurement
becomes a recollection.

The entries that matter go into `ROADMAP.md` under **Phase 13**, beside the ones taken here — a
measurement is only honest about the box it was taken in, and this is the second box.

---

## Rebuilding this

**Two commands, and the order is not optional.** Setup carries Epoch's MSI inside itself, so the
app has to exist before setup can be built:

```bash
npm --prefix BUILD/ui run world:build
```

```bash
cargo build --release -p epoch-setup --manifest-path BUILD/Cargo.toml
```

`target/release/epoch-setup.exe` is the file; copy it here as `Epoch_Setup.exe`. Building setup
without the app first is not an error — it warns, and the program says *this setup was built
without Epoch inside it* and refuses, rather than reporting a successful install of nothing.

The binary is not tracked by git: the source is what history is for, and 31 MB per rebuild is not.
