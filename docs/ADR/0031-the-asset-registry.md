# ADR-0031: The Asset Registry — one Workshop, many catalogues

- Status: Accepted
- Date: 2026-08-22
- Depends on: [[0003-engine-presentation-separation]], [[0007-provider-abstraction]], [[0009-trust-engine]], [[0016-asset-resolution]], [[0024-imported-artwork]], [[0030-images-are-made-by-a-capability]]
- Amends: nothing. It generalises what `models::hf` and `models::ollama_library` already are.
- Related architecture: [[../../CONTENT_PHILOSOPHY]], [[../../PRODUCT_ARCHITECTURE]]
- Horizon: the canonical type, the `Catalogue` trait, Hugging Face and Civitai **IMPLEMENT NOW**; dependency resolution and install-to-the-right-folder **IMPLEMENT NOW**; further providers **DESIGN NOW**; anything that installs without being asked **refused**

## Context — the Workshop is not new

It would be easy to read *"we need a Workshop for LoRAs, presets and models"* as a new screen. It
is not. The Models Workshop exists, is used, and already solves most of the hard parts:

- it searches **Hugging Face** with facets, and pages results;
- it **weighs every model against this machine's memory** before anything is fetched;
- it refuses to hide that something does not fit — it exists because 17.7 GB once landed on a
  17.2 GB machine and nothing said a word;
- it has **two ways to fetch** (`PULL` into Ollama, `SAVE THE FILE` into the vault) because those
  land in different places for different runtimes;
- and it exists in **both surfaces**, Epoch and EpochServices.

What it holds is one kind of thing: models that think. Images need checkpoints, LoRAs, VAEs,
upscalers and workflows. Building a second Workshop for those would be two answers to *where do
things come from* — and the second one would immediately be worse, because it would not know how
to weigh anything.

There is also already more than one catalogue: `models::hf` asks Hugging Face and
`models::ollama_library` reads Ollama's shelf. So *"a source of things you can install"* is a
shape this codebase has twice and has never named.

## Decision

### 1. One canonical Asset; the Engine never knows which site it came from

```text
Asset
  id · name · author · summary · preview
  kind         checkpoint | lora | vae | upscaler | controlnet | workflow
  base_model   SDXL | FLUX | SD1.5 | …            (what it is built for)
  needs        [Asset requirements]                (what it cannot run without)
  bytes · license · adult · version
  source       which catalogue answered
```

`source` exists so a person can go and look, and so a licence can be traced. **Nothing in the
Engine may branch on it.** Internal First: the day a third catalogue arrives, every screen already
renders it.

### 2. A `Catalogue` is a Provider, in the sense ADR-0007 already means

Search, fetch one, resolve a download. Interchangeable, and adding one is a new implementation
rather than an edit to every screen — the same reason `Kind::OpenAi` made adding a backend a form
rather than a pull request.

**Two ship** (owner, 2026-08-22): **Hugging Face** and **Civitai**. Both have documented APIs.
Together they are a larger library than anyone will exhaust, and the pair was chosen over five on
the grounds that three more integrations buy breadth nobody has asked for yet.

ComfyWorkflows, Tensor.Art, the official ComfyUI Templates browser and GitHub were considered and
**deferred, not refused**. Each is one more implementation of one trait. What survives from the
Templates work is not a provider but a **format**: a workflow may declare the models it needs,
with a URL and a destination folder, and Epoch reads that wherever it appears.

### 3. Measured before it is built, every time

Each catalogue is interrogated before a line of it is written, and one that cannot be asked
honestly does not ship. This is not caution for its own sake — it has already deleted a feature:
an Ollama *cloud model listing* was written, measured, found to be answering a different question
than it appeared to, and removed. The alternative was a screen telling somebody that a model on
their own disk was a cloud model.

### 4. Installing is a decision, and the size is never hidden

Everything the Workshop already does applies unchanged: what it costs, whether it fits, and what
this machine will be left with — **before** the button. A dependency list is part of that: a
workflow needing a checkpoint, two LoRAs and two custom nodes says so, says which are already
here, and totals what is missing.

### 5. A character offers; it never installs

> *"I have nothing for pixel art. I found 14 styles that fit your machine."* — and then it stops.

Fetching fourteen gigabytes is not a decision a tool call makes. This is the same line ADR-0030
draws around generated images and the one the Launcher draws around invented gauges: Epoch may
*say* what exists; the person decides what lands on their disk.

### 6. Licence travels, and adult material is filtered by default

Downloading somebody else's model into your own vault is yours (ADR-0024's rule for imported
artwork, unchanged). **Redistributing it is what `CONTENT_PHILOSOPHY`'s hard rule governs**, and
export is where that is checked — an asset with no redistributable licence is named at that
moment, not silently packed.

Civitai carries a great deal of adult material. It is filtered by default and the filter is
**stated rather than hidden**: a catalogue that quietly removes results is a catalogue nobody can
trust about anything else either.

### 7. Both surfaces

A lent machine has its own Workshop today, and it is the machine a model would be downloaded
*onto*. The new shelves belong there too — the standing question since 2026-08-21: *does
EpochServices need this as well?*

## Consequences

- The Workshop grows shelves rather than gaining a sibling.
- A Style (ADR-0030) becomes installable: *find something that draws watercolour* is a search,
  and attaching it to the Style is one press.
- `models::hf` becomes the first `Catalogue` rather than a special case.
- Nothing in this ADR knows what ComfyUI is. A registry of assets and an engine that consumes
  them are separate, which is what keeps the second engine cheap.

## Refused

- **Scraping where an API exists.** Both chosen sources answer properly.
- **Installing anything without being asked**, by a character or by Epoch.
- **A default workflow shipped with Epoch.** It would become the house style invisibly (ADR-0030).
- **Branching on `source` anywhere in the Engine.** The moment one screen says *"if Civitai…"*,
  the abstraction is decoration.

---

## Amendment — three shelves, one registry (2026-08-23)

> **A Brain is not a checkpoint.** One registry, rendered in three places.

### What this corrects

The Consequences say *"the Workshop grows shelves rather than gaining a sibling"*. That is a rule
about the **Engine**, and as an Engine rule it stands entirely: one canonical `Asset`, one
`Catalogue` trait, nothing branching on `source`.

Read as a rule about the **surface** it produces the wrong screen, and the owner named the reason
before it was built:

> A Brain is not a `.safetensors`. A Brain is not a checkpoint.

They are different kinds by the test this repository already uses (ADR-0026). A **Brain is
identity** — changing the model changes who somebody is, which is why the Runtime never chooses
one. A **checkpoint is inventory** — it belongs to nobody, it is consumed by a capability, and
swapping it changes a picture rather than a person. A single list holding `gemma4:12b` next to
`sd_xl_base_1.0.safetensors` groups them by the only thing they share, which is being a large
file.

Measured while checking: `WORKSHOP`'s own subtitle already reads *"Tools, MCPs & automations"*
while `MCP` is a separate main-menu entry directly above it
([`LauncherScreen.tsx`](../../BUILD/ui/src/screens/LauncherScreen.tsx)). The overlap predates this
amendment; the split resolves it rather than creating it.

### The decision

```text
WORKSHOP
  MODELS      brains — Ollama · LM Studio · llama.cpp · Hugging Face
  CREATION    checkpoints · LoRAs · VAEs · upscalers · ControlNet · workflows
  MCP         tools from outside Epoch
```

**MCP moves in**, so the main menu loses an entry instead of gaining two.

**The MCP shelf is not a catalogue of assets, and the document says so rather than glossing it.**
Nothing there is downloaded, hashed or weighed against this machine's memory; a server is
connected, not installed. It shares the *place* — *where things Epoch can use come from* — and
none of the mechanism. Pretending otherwise would produce an `Asset` with no bytes and no licence
in order to make a menu look tidy.

### The invariant, and it is the whole of this ADR unchanged

> **One canonical `Asset`. One `Catalogue` trait. One install path. One measurement of whether it
> fits this machine. Three shelves are three renderings of it.**

This is the canonical-versus-presentation split `PRODUCT_ARCHITECTURE` already draws everywhere
else: the domain language never changes, surfaces project it. The Engine keeps `Asset.kind` and
gains **no** `Workshop` enum — the grouping is computed where it is drawn.

What this must never become is three searchers, three install flows and three ways of asking
whether 17.7 GB fits on a 17.2 GB machine. That outcome is what §1 exists to prevent, and it is
the one way this amendment could go wrong.

**Held by a test:** adding a catalogue changes no screen, and the shelves resolve from `kind`
alone.

### Consequences

- **Everything installed under CREATION appears in `CREATIONS` as something this World can make**,
  through the library (ADR-0032). The Workshop is where things come from; `CREATIONS` is what this
  World can do with them. Neither searches for what the other holds.
- The Models Workshop stops growing kinds it has no way to weigh, and keeps being about brains.
- EpochServices gets the same three shelves, for the same reason it got the same one shelf.
