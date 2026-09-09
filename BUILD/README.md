# BUILD — the product

This folder **is** Epoch. The vault (`../docs`, `../*.md`) documents the architecture and the reasoning; `BUILD/` contains the system.

Implementation philosophy: [Build From Life](../docs/Build/Build%20From%20Life.md).
Current milestone: [Milestone 1 — Walking Skeleton](../docs/Milestones/Milestone%201%20-%20Roadmap.md).

## What exists today

**Milestone 1, Step 2 — "Someone lives here".** The app opens directly into the World, and the Researcher is already there: in her Laboratory, doing her own authored idle behaviour, which changes over time. Places and names come from the active World Pack, never from code.

Edit `vault/definitions/characters/researcher.toml` while the World is open and she changes — her file is the source of truth, and the Registry watches it.

```
BUILD/
├─ Cargo.toml                       workspace
├─ crates/
│  ├─ epoch-kernel/                 Domain Kernel — pure types, zero I/O
│  │  └─ src/concept.rs             PlaceConcept · CharacterArchetype
│  │  ├─ src/definition.rs          CharacterDefinition · PresenceProfile · IdleBehavior
│  │  └─ src/presence.rs            PresenceState · ActivityClass
│  ├─ epoch-engine/                 all business logic; runs headless
│  │  ├─ src/pack.rs                manifest, fallback chain, coverage
│  │  ├─ src/definition.rs          Definition Registry over the vault files
│  │  ├─ src/simulation.rs          inhabitants + derived presence
│  │  ├─ src/world.rs               WorldView projection
│  │  └─ tests/                     shipped pack · wire contract · first inhabitant
│  └─ epoch-tauri/                  shell: IPC commands only, no logic
│     ├─ src/main.rs                get_world command
│     ├─ src/state.rs               engine state held for the window
│     ├─ tauri.conf.json
│     └─ icons/icon.ico             placeholder — see note below
├─ ui/                              React, presentation only
│  └─ src/
│     ├─ app/                       root + world.css
│     ├─ ipc/                       contracts.ts (the contract) · world.ts (transport)
│     ├─ hooks/useWorld.ts          contract → React state
│     ├─ screens/WorldScreen.tsx    the World
│     └─ components/Place.tsx       one place
├─ vault/definitions/characters/    Definitions — the source of truth, watched live
└─ packs/default/pack.toml          default World Pack
```

Directories are created when a step uses them. `state/`, `services/`, `assets/` do not exist yet because nothing lives in them (Build From Life, rule 11).

## How presence works

The Simulation owns time. A plain thread advances it every 500 ms, but presence is
**published only when it actually changes** — so the engine emits an event every few
seconds as her activity turns over, never per frame (ADR-0018).

- Engine → UI: the `world:changed` event carries a whole authoritative `WorldView`.
- The UI renders and interpolates. It never invents presence.
- Hot reload: definition files are checked by modification time on the same tick. No
  filesystem-watching dependency was added, because for a handful of authored files
  polling is sufficient and there is no evidence yet that it is not.

## Prerequisites

- Rust (stable, MSVC host) — verified against 1.97.1
- Node 20+ — verified against 22.23.1
- WebView2 runtime (ships with Edge on Windows 11)

## Run

Install frontend dependencies once:

```bash
npm --prefix ui install
```

Open the World (starts Vite, builds the shell, opens the window):

```bash
npm --prefix ui run world
```

Hot reload is live on all three crates and on the frontend.

> **Why the script cds.** The Tauri CLI discovers a project by finding `tauri.conf.json` in the current folder or a subfolder, and it runs `beforeDevCommand` from the **parent of the config directory** (the `src-tauri/` convention). Since our config lives in `crates/epoch-tauri/`, that parent is `crates/`, which is why `beforeDevCommand` uses `--prefix ../ui`. Both facts were found by running it, not by reading docs.

## Verify

Engine and contract checks:

```bash
cargo test --workspace
```

Frontend typecheck and production build:

```bash
npm --prefix ui run build
```

## Notes

**The icon is a placeholder.** `crates/epoch-tauri/icons/icon.ico` is an abstract original mark (a horizon under an arc) that exists only because `tauri-build` requires a Windows resource icon. It is **not** Epoch's visual identity — that belongs to the [Official Epoch Universe](../docs/Milestones/Official%20Epoch%20Universe.md) milestone.

**Pack paths are development-time.** `epoch-tauri` resolves `packs/default/pack.toml` relative to its own manifest directory. Installed builds will resolve packs from an application data directory; that belongs to the milestone that ships an installer.

**Three architecture rules are enforced by tests, not by discipline:**

- A World Pack without license metadata **fails to load** (`CONTENT_PHILOSOPHY.md` hard rule).
- The World renders with **no pack at all**, as visible placeholders — there is never a reason to show a loading screen instead of the World.
- A place label that equals its concept id fails the suite, catching anything that bypasses the pack (Internal First).
