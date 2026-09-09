# Install, and First Launch

> **Why this document exists.** The repository is meant to be *what a user installs*. Today it is
> what one developer's machine happens to contain, because every path resolves from
> `CARGO_MANIFEST_DIR` — a constant baked in at compile time. Nothing about installation can be
> designed until the three kinds of location are separated.
>
> **Two moments, and they are not the same one.** The **Wizard** runs once, at install time,
> before Epoch exists on the machine: it prepares the ground. The **Launcher** runs every time
> after that: it prepares the work. Conflating them was the first draft's mistake — it described
> a wizard *inside* the app, which is both a second front door and a screen standing in front of
> the World.

---

## 1. Three locations, not one

Today all three are the same folder. That is the whole bug.

| | What it holds | Who writes it | Where it belongs |
|---|---|---|---|
| **Shipped** | the default World Pack, the asset kit, the Wizard's own payload | the installer, never the app | beside the binary, read-only |
| **Data** | the vault: crew, Worlds, Quests, settings, secrets | the app | `%APPDATA%\Epoch` (Windows) · `~/.local/share/epoch` (Linux) · `~/Library/Application Support/Epoch` (macOS) |
| **Project roots** | the user's own codebases | the user, and agents they allow | anywhere. Chosen, never guessed |

**The repository contains the first column and nothing else.** That is what makes it "what a user
installs" rather than "what Kislok's machine looks like".

### Code change

`epoch-engine/src/paths.rs`:

```rust
pub struct Paths {
    /// Read-only, installed with the binary.
    pub shipped: PathBuf,
    /// Read-write, this user's own.
    pub data: PathBuf,
}
```

Resolved **once** at startup and passed down, rather than each module calling a free function that
computes a path from a compile-time constant. Development keeps working: when the binary sits in a
`target/` directory, `shipped` falls back to the source tree, exactly as today. Nothing breaks
while the installer does not exist yet.

---

## 2. What first run creates

Created lazily and only when needed — an empty folder tree made eagerly is a promise that
something is configured.

```
%APPDATA%\Epoch\
  vault\
    definitions\
      characters\        ← the user's crew (ADR-0023: characters are theirs)
    worlds\
      <world-id>\
        places.toml      ← their map (ADR-0028)
        quests.json      ← their Chronicles
    packs\               ← World Packs they import (the shipped one is not copied here)
    settings.toml
    providers.toml
    orchestrator.toml
    trust.toml
    secrets.dat          ← DPAPI blob. Never plaintext, never in a repo
  logs\
    epoch.log            ← so a crash produces something sendable (Phase 6.3)
```

Two things deliberately absent:

- **No `agent-token` file after the MCP door is opened.** A legacy plaintext bearer is
  validated, moved unchanged into `secrets.dat`, then removed. A failed migration leaves the
  legacy file in place rather than rotating a credential that an already-configured agent uses.
- **The shipped pack is not copied in.** It is read where it was installed. Copying would fork it
  the first time an update ships a better one.

---

## 3. The Wizard — one program, one time, before Epoch exists

`Epoch_Setup.exe`. Downloaded from the site, run once, and then never seen again.

Its job is the machine, not the product: put Epoch somewhere, make sure the things Epoch needs are
present, create the data directory, and get out of the way. It ends by launching Epoch, which
opens on the Launcher.

### What it does without asking

Only what is unambiguously ours to do:

- **Detect** the machine: OS and architecture, free disk, and whether **WebView2** is present —
  the runtime the window itself needs on Windows. Missing, it installs Microsoft's evergreen
  bootstrapper, because without it Epoch cannot draw a single frame.
- **Install Epoch** — the binary and its shipped content (the default World Pack, the asset kit).
  **Per user by default**, into `%LOCALAPPDATA%\Programs\Epoch`, which needs no elevation at all.
  A per-machine install is offered and asks for it honestly.
- **Create the data directory** (section 2) — empty, and nothing pretending to be configured.
- Register the uninstaller and a shortcut.

### What it detects and *offers*, never installs on its own

- **Ollama** - found: says so, and lists the models already pulled. Missing: offers to install it,
  with the size stated before the download starts.
- **Claude Code** - measured as installed / not, exactly as the Launcher measures it. Missing:
  offers to install it and says it needs a Claude subscription.
- **Codex** - the same shape, when Epoch can host it.
- **Git** - optional, and only mentioned because a Project Root is nicer with it.

Every one is a checkbox, unchecked by default, with its download size shown. **Declining any of
them still produces a working Epoch** that says what is missing and how to add it later. A setup
program that installs a third party's software without being asked is doing something the user did
not agree to, whatever the intent.

### What it must never do

- **Sign in to anything.** Not Claude, not a provider, not an account. Epoch opens the door and
  never holds the key (`CLAUDE.md`, 2026-08-08); a wizard asking for a credential would be the
  first violation of that, on the first screen anybody ever sees.
- **Ask for an API key.** That is configuration and belongs to the Launcher.
- **Pull a model.** Gigabytes, on a connection it knows nothing about, for a choice the user has
  not made yet.
- **Create a crew, a World, or a Project Root.** Those are the user's, and they are what the
  product is *for* - doing them here would mean the first thing Epoch shows is somebody else's
  work.

### Why this and not an in-app wizard

A wizard inside the app is a second front door: two places that can configure the same thing, two
places to keep honest, and a screen the user must get past before seeing what they installed. The
Wizard prepares the *machine* and stops existing; the Launcher prepares the *work* and is a place
you come back to. Neither can do the other's job.

---

## 4. The first frame is the World

**Build From Life, rule 1:** never a loading screen, never a wizard standing between the user and
the product. So there is no modal setup wizard. The World renders — empty, honest, and already
itself — and the *Launcher* is where preparation happens, because the Launcher is a place (the
bridge of a docked ship) and preparing is what you do there.

What "empty and honest" means concretely: the default pack renders its land and its Places, nobody
lives there yet, and every instrument reads what is actually true — `0 CREW`, `NO MODELS`,
`NO PROJECT`. Cold instruments, not hidden ones.

---

## 5. The three things that must happen, in the order they unblock each other

The Launcher shows them as three cold instruments that light up. Not a wizard: a checklist you can
do in any order, out of which nothing is hidden and nothing is skipped silently.

### 5.1 A brain — *"nobody can think yet"*

The only genuinely blocking one, and the only one where Epoch can help by **measuring** rather than
asking:

- **Ollama** — probed on the usual endpoint. Found: it lights up with the models actually pulled.
  Not found: one sentence saying where it looked and what to install.
- **Claude Code** — probed as installed / signed in (already built). Signed out offers **Sign in**,
  which opens the agent's own flow. Epoch never sees the credential.
- **Anthropic API** — offered, never pushed. Needs a key, is billed separately, and the panel says
  both before asking for anything.

Nothing is auto-selected. A machine with Ollama running has models a click away; a machine with
neither is told what to install, with the two routes named.

### 5.2 A crew — *"nobody lives here"*, and nobody is invented

> **Corrected.** The first draft offered *"start with a crew of four"* — four archetype templates
> copied in on request. That contradicts ADR-0023: characters belong to the user. Handing somebody
> four pre-made people and calling them theirs is the same act as generating a face for somebody
> who has not chosen one, one step earlier.

**The crew starts at zero.** The panel says so plainly and offers exactly one thing: **Add a
character.**

What the shipped content may contain is *nothing that becomes a character on its own*. When the
user adds one, the form starts with an archetype to classify them (Researcher, Coordinator,
Guardian, Historian — classification, not identity, ADR-0017) and empty fields for everything that
makes them somebody: name, portrait, prompt, routine.

An archetype may carry a **suggested prompt** the user can accept, edit or delete — a starting
point they chose to see, not a person that already existed. The difference is who performed the
act: offering words to somebody writing a character is help; putting four strangers in their World
is a decision made for them.

Zero is also the honest reading of the instrument. `0 CREW` on an empty install is true, and the
World showing nobody home is not a failure state — it is where authoring starts.

### 5.3 A World — *"there is nowhere to work"*

The existing NEW WORLD dialog, unchanged, and deliberately small:

- **World name**
- **Project root** — a folder picker. Ungated and early, because planning against the user's real
  codebase is worth immeasurably more than planning against a hypothesis (ADR-0025).
- **Git repo** *(optional)*
- **Starting crew** — from whoever exists
- **Import a World Pack** *(optional)*

And the list it must never grow, because all of it is configuration rather than creation:
Provider URL, `num_ctx`, temperature, context, tokens, capabilities, MCP, API keys, sprite packs.

Creating a World goes straight to the World Editor — a World you have just made is empty, and
empty is where authoring starts.

---

## 6. What the user sees, in order

1. **The World.** Land, Places, nobody home. It is already Epoch, not a setup screen.
2. **A single line on the bridge:** *"Nobody can think yet."* — with the two routes, measured.
3. Sign in or start Ollama → the crew list stops being cold.
4. **Add a character** → one person exists, and they made them.
5. **NEW WORLD** → name, folder, crew.
6. They are working. Nothing was configured that did not have to be.

**Total mandatory decisions: two.** A brain, and a folder. Everything else has a default that is
true.

---

## 7. What ships in the repository

```
BUILD/
  crates/            the engine, the kernel, the shell
  ui/                the World and the Launcher
  installer/         the Wizard - detection, install, data directory
  packs/default/     the placeholder World Pack - archetype names, no invented universe
docs/                the reasoning, including what was refuted
```

**One repository, two artefacts.** `Epoch_Setup.exe` and Epoch itself are built from the same
tree, tagged together, and versioned together - a separate installer repository would let the two
drift, and the failure mode of that is a wizard installing a layout the app no longer reads.

There is no `seed/characters/`. Nothing ships that becomes a person.

**Not in the repository:** anybody's vault. Once §1 lands, the vault is not in the tree at all, so
this stops being a rule anybody has to remember — it becomes where the code looks.

Until then, `BUILD/vault/` is the developer's own data sitting inside the distributable. It is
tracked today because the repository was initialised before this document existed; the decision
about whether personal Chronicles belong in a cloud backup is the user's, and is separate from
what the installed product contains.

---

## 8. Order of work

1. `paths.rs` — three locations, resolved once, dev fallback intact.
2. First-run creation of the vault, lazily.
3. `seed/characters/` + the "start with a crew" offer.
4. The bridge's three cold instruments, with the routes each one names.
5. Then, and only then, the installer (Phase 6.1) has something coherent to install.

Steps 1 and 2 are the ones that make the repository mean what it is supposed to mean.
