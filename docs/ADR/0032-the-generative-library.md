# ADR-0032: The Generative Library — Epoch owns it, the engine reads it

- Status: Accepted
- Date: 2026-08-23
- Depends on: [[0016-asset-resolution]], [[0024-imported-artwork]], [[0029-bridges-and-the-worlds-machines]], [[0030-images-are-made-by-a-capability]], [[0031-the-asset-registry]]
- Amends: nothing. It answers a question ADR-0031 left open — *where does an installed asset land?*
- Related architecture: [[../../CONTENT_PHILOSOPHY]], [[../../CLAUDE]]
- Horizon: the library, the manifest and the search-path handshake **IMPLEMENT NOW**; a second image engine **DESIGN NOW**; copying files into somebody else's install **refused**

## Context — the files exist and nobody owns them

ADR-0031 settled where assets *come from*. It did not say where they *go*, and the gap showed up
the first time somebody tried to use one.

Measured on this machine, 2026-08-22:

| asked | answered |
|---|---|
| `object_info/CheckpointLoaderSimple` | `[]` |
| that folder on disk | SDXL 6.9 GB · SD 1.5 4.3 GB |
| `/models/loras` | the LoRA it downloaded, **from the same shared root** |

Two facts follow, and only one of them is about ComfyUI.

**Epoch already asks the authoritative question.** `object_info` is the enum the server will
accept inside a graph, so an empty answer is the server saying it would load nothing — not Epoch
looking in the wrong place. That half needs no fix.

**Epoch owns none of what the answer is made of.** The files live wherever an installer put them,
in a tree belonging to another program, discovered by a listing that can go stale and did. A
Workshop that installs into a folder it does not own is a Workshop whose results can vanish
because a different application was reinstalled.

The owner also arrived at the same shape from the other end: assets should be dropped on Epoch and
be *understood*, rather than filed by hand into directories whose names are the vocabulary this
product exists to hide.

## Decision

### 1. Epoch owns the library; the engine is a reader

```text
%APPDATA%/Epoch/library/generative/
  models/        checkpoints and diffusion models
  loras/
  vaes/
  upscalers/
  controlnet/
  embeddings/
  workflows/
```

One library per installation, not per World. A 6 GB checkpoint is a fact about a machine, in the
same category as `num_gpu` (ADR-0026) and for the same reason: it does not travel with anybody.
Worlds reference it; none of them contains it.

This is the vault rule pointed outward. Epoch already refuses to touch what the user pointed it
at and freely owns what it created (ADR-0024, Phase 9.5). The library is created by Epoch, so
Epoch may enumerate, verify and remove it.

### 2. Epoch adds a search path; it never moves the user's files

ComfyUI reads external roots through `extra_model_paths.yaml`. Epoch writes **one block it owns**,
leaves every other entry untouched, and can remove exactly what it added.

> **Epoch adds a search path. It never moves, copies or renames a file the user already had.**

Copying is refused for three separate reasons and any one of them is sufficient: it duplicates
tens of gigabytes; it makes Epoch's copy and the user's original disagree the first time one is
updated; and it is writing into a tree belonging to a program Epoch does not own. Adding a line to
a config file is a change with an inverse. Copying 24 GB is not.

**What the user already had stays where it is** and is *listed*, never adopted. A library that
silently absorbs an existing ComfyUI installation is the directory walk Phase 9.5 forbade, running
forwards instead of backwards.

**Not yet measured, and it gates the work:** whether this ComfyUI Desktop honours a search path
Epoch adds while it is running, and whether it needs a restart to see it. Measured before built,
like every source before it — an install that lands correctly and is invisible until a restart
nobody mentioned is worse than one that refuses.

### 3. An asset is understood from its bytes, never from its name

A file arrives — dropped, or installed from a catalogue. Epoch hashes it, asks the registry
(ADR-0031) what that hash is, and writes a manifest beside it:

```text
kind · base_model · version · trigger words · recommended strength
license · adult · source + id · sha256 · bytes
```

This is ADR-0024's rule, one subsystem over: **the Engine names the file from the bytes, never
from the uploaded name or the claimed extension.** A `.safetensors` called `pixel_art_final_v3`
is a claim; its hash is a fact. An unknown hash is **unknown**, stated plainly and still usable —
never guessed at from the filename, which is how a FLUX LoRA gets filed as SDXL.

### 4. Compatibility is measured, and a refusal names the reason

`base_model` decides whether a LoRA and a model can work together. Where they cannot, Epoch says
so **before anything runs**, in the words a person uses:

> *Made for FLUX. Everything here is SDXL — they will not work together.*

Never `mat1 and mat2 shapes cannot be multiplied`. And never a silent block: the file is the
user's, in the user's vault, so **keep it anyway** is offered. Epoch measured and said; the answer
is theirs. The same arrangement the Workshop already has for a model too large for this card.

### 5. The library is what makes a World able to make something

`what can this World draw` is answered by reading the library and the workflows compiled against
this server — never by a list Epoch shipped. This is what ADR-0030's second amendment rests on,
and the two must land together or the Styles have nothing to be derived from.

### 6. Both surfaces

A Bridge contributes computation (ADR-0029) and is the machine an asset would be installed *onto*.
It has its own library, on its own disk, with its own manifests. The standing question since
2026-08-21 — *does EpochServices need this too?* — is answered yes, and the policy lives where
both can reach it.

## Consequences

- The Workshop's results survive reinstalling ComfyUI, because they were never inside it.
- A second image engine reads the same library. Nothing about the library knows what ComfyUI is,
  which is the whole reason the second engine is cheap.
- Erasing is an enumerated list of files Epoch created (Phase 9.5), and the search-path block is
  one of the entries.
- The user never types a directory name, and never learns one.

## Refused

- **Copying assets into ComfyUI's tree.** Duplication, divergence, and writing into somebody
  else's install.
- **Writing anywhere in another program's installation except the one block Epoch owns.**
- **Trusting a filename** for kind, base model or anything else.
- **Absorbing an existing installation.** Listed, never adopted.
- **A library shipped with Epoch.** Whatever ships becomes the house style invisibly — the same
  refusal ADR-0030 makes about workflows.

---

## Amendment (2026-08-25, from the owner) — the library remembers what drew

**Status: DESIGN NOW for both surfaces; IMPLEMENT NOW for Epoch only.** The decision below is
Phase 11½ A ([[../../ROADMAP]]), settled before any of it was built so that it is not settled
twice by two people who each guessed.

### The question

The Studio Panel can only advise on a model whose family Epoch can read — six today, against an
open set that grows faster than anybody reads release notes. Detection rules will never close
that gap. But when somebody draws **successfully**, Epoch is holding the entire answer it could
not give: this model file, these encoders, this `type` string, this VAE, and a graph that ran.

That is a measurement of exactly the thing the panel could not measure, and it is free, because
it already happened. So it is kept, and offered the next time that model is chosen — as **what
drew last time**, never as a claim about the architecture.

### Where a recipe lives, and why it is not the character's

**Beside the library, keyed by hash, one store per installation.** Not in a Character, not in a
World, not in a Character Pack.

The owner's question is the one that settles it, and ADR-0026 had already written the test:
*does this survive changing the engine?* A recipe does not. It names **this machine's files**,
by **this machine's hashes**, against **this machine's server**. It belongs to a machine in the
same category as `num_gpu` — putting it on a Character would be the identical mistake ADR-0026
refused one floor up, and it would have a practical cost the same day: two characters using one
model would each discover it separately and could advise differently about the same file.

A recipe is not authored, either. `epoch-assets` states the boundary as *did a person author
this, or did a machine happen to have it?* — a workflow is authored, and a recipe is what a
machine ran. So the type and its store live in **`epoch-models`**, beside `generative.rs`.

That placement is not a preference. **ADR-0029 §9 forbids EpochServices from linking
`epoch-engine`**, and this is policy both surfaces need, so `epoch-engine` is not available to
hold it. Deciding this now is what stops it being written in the wrong crate and moved later.

### It is not portable, and that is the point

**The recipe stays on the machine that drew, and is read by the machine about to draw.** A lent
easel's recipes describe files on the lent machine; carrying them home to advise about files that
are not here would be advice about somebody else's disk.

**Honest about the second surface:** nothing on the Host reads `Have.easels` yet, so drawing on a
lent machine is not wired. EpochServices therefore gets the *contract* now and the store the day
that machine actually draws. A store nothing fills is a cold instrument with no reading behind
it, and this ADR does not ship one.

### What did not run is the same record, with its outcome

Not a second store. One entry per attempt, carrying whether it **drew** or was **refused** — and
for a refusal, **the server's own words**, which are the only true account (ADR-0030, third
amendment).

Two limits, both to stop the record lying:

- **Only a refusal about the configuration.** The server said that combination is not one it will
  load. An out-of-memory is not that: it is the card that day, and filing it as *does not work*
  would be false the next time the card is free.
- **Said as what it is.** *"This combination was refused"*, never *"this model cannot"*. A recipe
  proves **a** combination ran, not that it is the best one; a refusal proves **one** failed, not
  that the model is limited. A second recipe for the same model is two things that worked, not a
  contradiction.

### Consequences

- A model Epoch has never heard of is advised the second time it is used, from evidence.
- A model nobody has drawn with still says so, exactly as it does today. It degrades honestly.
- The advice is per-machine and cannot become a claim about a machine it was not measured on.
- `epoch-models` gains the type; neither surface gains a second way of asking the question.
