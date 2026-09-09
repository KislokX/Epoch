# ADR-0030: Images are made by a capability, not by a brain

- Status: Accepted
- Date: 2026-08-21
- Depends on: [[0007-provider-abstraction]], [[0009-trust-engine]], [[0016-asset-resolution]], [[0025-quests-own-the-work]], [[0026-characters-are-portable-assets]]
- Amends: nothing. It **grants one narrow exception** to `CLAUDE.md`'s ban on node graphs — see *The exception, and its fence*.
- Related architecture: [[../../CONTENT_PHILOSOPHY]], [[../../CLAUDE]]
- Horizon: the capability, the backend contract and workflow import **IMPLEMENT NOW**; a second backend family and image *editing* **DESIGN NOW**; anything generating World Pack artwork **refused, permanently** — see *Refused*

## Context — what changed since this was written down as one line

The roadmap said: *"Image generation (ComfyUI or similar) — a Provider of a different kind. It
needs the Provider abstraction to stop assuming text, which is a real change rather than a
plugin."*

Two ADRs have landed since, and both change the answer.

**ADR-0026** settled how a backend exposes what it can be tuned with: the Provider declares its
controls at runtime and the surface renders them generically. That was most of the work this item
was afraid of, and it is done.

**ADR-0025** settled who owns work: the Quest does, and characters contribute to it. So *"which
character can make images"* is the wrong question — the right one is *what a Quest produces, and
what enters its History as evidence.*

## Decision

### 1. Making an image is a capability, not a Provider

> **A Provider is what a character thinks with. A character does not think in pictures — it asks
> for one.**

`draw_image` joins `see_image` as a capability. That follows from what each half is:

> **Amended 2026-08-28 — see the fourth amendment.** `draw_image` and `edit_image` are deleted.
> Drawing is still a capability and still not a Provider; the capability is now `open_studio`,
> and it takes no arguments. Everything in this section about *why a picture is a call and not a
> turn* stands unchanged — what changed is who fills the call in.

- A Provider answers a **turn**: a Conversation in, tokens out, streamed, entering a Chronicle.
  Making that return bytes would mean widening `take_turn`, `Answer` and `Chunk` for one caller —
  the abstraction stops assuming text, and every text backend pays for it forever.
- A capability answers a **call**: named arguments in, a result out, through `decide()`
  (ADR-0009), recorded as an artifact on the Quest.

The capability route also arrives already wired: **all three local runtimes make native tool
calls** — measured 2026-08-21, `finish_reason: tool_calls` from Ollama, llama.cpp and LM Studio
against the same declaration — so any character with any brain can reach it on the day it ships.

`see_image` is the precedent and the shape is deliberately its mirror: sight turns a picture into
words, this turns words into a picture, and both are translations rather than turns.

### 2. Requested, never declared

A character asks for `draw_image` in its Definition (ADR-0026) and the capability exists only
when an image backend is configured and answering. A Guardian that never asks for it does not
have it — **identity, not permission** — and asking for it on a machine with no backend produces
a sentence that says so, never a tool that quietly is not there.

Trust gates it like anything else. Writing the result to the Project Root is a *file* capability
and stays one; nothing here invents a second way to write to disk.

### 3. ComfyUI, and workflows are imported rather than authored here

The backend contract is an HTTP endpoint that takes a prompt and returns bytes. ComfyUI is the
first implementation because it is what the owner runs.

**Its API is a graph**, and that collides with a standing rule.

### 4. The exception, and its fence

`CLAUDE.md` forbids node graphs: *"Users should create powerful workflows using intuitive steps
instead of complicated node graphs."* The owner grants an exception on 2026-08-21, and it is
written down here rather than remembered:

> **The exception applies to image generation only.** Nothing else in Epoch may grow a node
> graph on its authority, and this ADR is where anybody proposing one is sent.

It is defensible for exactly one reason: **the graph is not authored in Epoch.** A ComfyUI
workflow is made in ComfyUI, exported as JSON, and *imported* — the same shape as every other
asset the user brings (ADR-0024: the frontend never touches the filesystem; the Engine names the
file from its bytes). Epoch renders no canvas, no nodes and no wires.

What Epoch shows is what a workflow **asks for**: the fields the imported JSON leaves open —
prompt, size, steps, seed — as ordinary controls, declared by the backend and rendered
generically, exactly as ADR-0026 already does for `num_ctx`. A person who has never opened
ComfyUI sees a prompt box; a person who has sees their own workflow behind it.

The rule the exception does **not** touch: Quests are sequences, never graphs (ADR-0025).

### 5. A generated image is evidence, never World artwork

`CONTENT_PHILOSOPHY` and the *assets are authored, never generated* rule stand unchanged. An
image a Quest produced is an **artifact on that Quest** — it belongs to the work, enters History
as evidence, and the user may do anything they like with it in their own vault.

It may not become a World Pack asset through any path Epoch provides. Not a place, not a
character portrait, not a backdrop, not a tile. The moment Epoch can fill its own world with
generated art, the World stops being authored and starts being average.

## Consequences

- One new capability, one new backend family, no change to `Provider`'s text assumption.
- An image backend is a *machine's* configuration, like a runtime — not a Character parameter
  (ADR-0026's test: it does not survive changing the engine).
- A Bridge can host it later without a new concept: ADR-0029 already says a Bridge contributes
  *computation*, and named diffusion when it said so.
- Hosted image backends inherit the disclosure question ADR-0025 raised for Project Roots: a
  prompt naming somebody's work, sent to a third party, is the user's decision to make explicitly.

## Refused

- **Generating World Pack artwork.** Permanently, per above.
- **A node editor inside Epoch.** The exception covers importing somebody else's graph, not
  growing one here.
- **Making image generation a Provider.** It would widen the turn for every text backend to serve
  one caller, and the capability path already reaches every brain.
- **Picking a workflow on the user's behalf.** A default that ships is a default that quietly
  becomes the house style; the first import is the user's.

---

## Amendment — Styles (2026-08-22)

> A character asks for **pixel art**. It never asks for `pixel_v3.json`.

### What this corrects

The decision above says the surface shows *"the fields the imported JSON leaves open"*, which is
true and one layer short. It left an open question — *which workflow does a character use?* — and
the first answer proposed was that a Character may name one. That answer was wrong, and the reason
it was wrong is already written down elsewhere in this repository:

> The engine references abstract **Asset Concepts** and NEVER filenames. Music is requested as a
> **mood**, never a track. — `CONTENT_PHILOSOPHY`, ADR-0016

*Pixel art* is a mood. `pixel_v3.json` is a filename. A character naming a workflow breaks a rule
Epoch has held since the Asset Resolver, one subsystem over.

### The three layers

**Workflows belong to the machine.** A graph depends on the checkpoints, LoRAs, custom nodes and
GPU of one installation. Provider-native tuning, namespaced, opaque to the Kernel — the same
place ADR-0026 puts `num_ctx`.

**Styles belong to the product.** A Style is a name a person and a character both understand,
pointing at whichever installed workflow can serve it. Six ship, chosen by the owner:
**General · Realistic · Pixel Art · Anime · Fantasy Illustration · Character Portrait**.

**Preferences belong to the character.** A canonical parameter, because it passes ADR-0026's one
test: *does this survive changing the engine?* A Historian who draws in watercolour still draws in
watercolour after ComfyUI is replaced with something else. `steps: 20` does not survive; *fine
detail* does.

### Resolution, in three levels and not four

1. **What was asked for now.** The user's or the character's words win.
2. **The character's preference.** A preference, never a cage: *"if nobody says otherwise, this is
   how I draw"*, not *"this is all I can do"*.
3. **The machine's default Style.**

A fourth level was proposed — falling back within a *family* when the exact workflow is missing —
and is **refused**. It requires Epoch to author and maintain a taxonomy of which workflows are
equivalent, and the day that taxonomy is wrong it substitutes silently, which is the thing its own
rule was trying to prevent. What replaces it is a sentence:

> *"I have no pixel art workflow installed. I can use General, or you can install one."*

That needs no taxonomy, and it is a cold instrument saying what it lacks.

### What a Style may not do

**A Style never crosses from local to hosted on its own.** It may list several workflows, and one
of them living on somebody else's server is not a technical detail — it decides whether the user's
prompt leaves the machine. `sight::who_can_see` already refuses to pick a hosted backend on the
user's behalf for exactly this reason (ADR-0025's disclosure rule). Crossing that line is asked.

**A Style's abilities are derived, never declared.** Whether a workflow can do img2img is
answered by whether its graph contains a `LoadImage`; inpaint by `VAEEncodeForInpaint`. A
capability somebody typed into a manifest is a capability that will eventually lie — *requested,
never declared* (ADR-0005), applied one layer down.

**Epoch ships the six names and nothing behind them.** A shipped workflow becomes the house style
by default, invisibly. A Style with nothing installed keeps its frame, loses its light, and names
what would light it up — the Launcher's rule, and the reason it must not quietly fall back to
General.

### What the capability looks like

```text
draw_image(
  describe:   "a lighthouse on a cliff at dawn"
  style:      "pixel art"                 optional -> character, then machine
  shape:      square | portrait | wide    optional
  detail:     quick | normal | fine       optional
  from_image: something shared in this Quest   optional
)
```

No `steps`, no `cfg`, no `sampler_name`. Those belong to the workflow, which belongs to the
machine. `detail` is semantic on purpose: *fine* survives changing the engine and `steps: 40` does
not.

### Consequences

- One capability per **intent** — `draw_image`, `edit_image`, later `make_video`. Never a
  `run_workflow(name)`: that is the node graph returning disguised as a tool, and it fails the
  survives-changing-the-engine test that everything else here passes.
- The Style layer is what lets a second engine arrive without the World noticing. A character
  asking for *anime* does not care that ComfyUI became something else.

---

## Second amendment — a Style is derived, not shipped (2026-08-23)

> A Style is not created. **A Style exists when something that can draw it arrives.**

### What this corrects

The amendment above says *"six ship, chosen by the owner"* and *"Epoch ships the six names and
nothing behind them"*. Six names in an array ([`images.rs`](../../BUILD/crates/epoch-engine/src/images.rs))
were a hypothesis, and using the product refuted it.

The failure is one sentence. A person asks for **watercolour**, and a closed set of six answers
*"that style does not exist"* — which is a sentence about Epoch's array, delivered as a sentence
about the world. Watercolour exists. What is missing is something to paint it with, and the
closed list cannot say that because it does not know the difference.

Five of six rows also sat permanently dark on a fresh install. That was defended as the cold
instrument rule, and the defence does not hold: a dark **REACTOR LOAD** is a real quantity not yet
wired, while a dark **Anime** was never a quantity — it was a category somebody chose on a
Tuesday. The rule protects readings, not catalogues.

### The decision

**Styles are read from the library** (ADR-0032) and from the workflows compiled against this
server. Install something that paints watercolour and the Style `watercolour` exists, because
something draws it. Remove it and the Style goes, because nothing does.

This is *derived, never declared* — the identical rule this ADR already applies one section up,
where a workflow's ability to do img2img is answered by whether its graph holds a `LoadImage`.
It was applied to a Style's **abilities** and not to its **existence**, and that was the
inconsistency.

**`General` survives, and it is the only one.** It is not a style; it is the absence of one. It
exists the moment any model does, which is what `BUILD ME ONE` attaches to.

### What a character may not do

A character **may not create a Style**, and this is the load-bearing half of the amendment.

Creating a Style would be creating **the appearance of a capability**. *"Done, I made you a
watercolour style"* followed by a plain SDXL render labelled watercolour is precisely the
substitution this whole design exists to refuse — and it is worse than the refusal it replaces,
because it arrives confidently and with a picture attached.

What a character does instead is what ADR-0031 §5 already licenses: it says what it cannot do,
lists what it can, and **offers to look**.

> *"I cannot draw watercolour yet. This World draws general, pixel art, portrait. I found four
> things that paint watercolour and fit this machine — shall I show you?"*

The Style then comes into existence from a real cause: a file arrived. Causality survives, which
is the same test ADR-0018 puts on a character crossing a map.

### The cost, stated

An open vocabulary is **not exact**, and six closed names were. `watercolour` · `watercolor` ·
`aquarelle` are three strings.

So: the name is whatever the asset declares (a catalogue answers with `name` and `trainedWords`),
the user may rename it, and matching is **exact, or a refusal that lists what exists**. Nothing is
matched by resemblance. A near-miss is exactly how a realistic picture gets drawn for somebody who
asked for pixel art — the failure this ADR opened by naming — and a fuzzy matcher is that failure
with a probability attached.

### What does not change

- **A preference is requested, never declared.** A character carrying `watercolour` into a World
  with nothing that paints it is told so (ADR-0026). The preference travels; the ability does not.
- **No silent fallback, ever.** It gets stronger here, not weaker: with Styles derived, falling
  back means drawing with something the user never asked for *and* never installed.
- **Resolution stays three levels** — what was asked now, the character's preference, the
  machine's default.
- **Nothing ships behind a name.** The reason is unchanged: a shipped workflow becomes the house
  style invisibly.

### Consequence for every surface

A Style with nothing behind it **cannot exist by construction**, so a deck of Styles is a list of
things that work. The cold instrument does not disappear — it **moves from the list to the
sentence**, where it is more useful: the honest report of an absence is now spoken by the
character at the moment somebody wants the thing, instead of sitting greyed out on a panel nobody
opened.


---

## Third amendment — what a refusal owes the character (2026-08-23)

> **A tool that refuses must say what did not happen, before it says anything else.**

### The run

Measured with `gemma4:12b`, one turn, in a World holding exactly one workflow attached to
`General`:

| | |
|---|---|
| asked | *"Haz una imagen de Crono de Chrono Trigger"* — no style named |
| the call | `draw_image(describe: "Crono — in a beautiful anime style", style: "anime")` |
| the tool | *"No workflow here draws anime. What this World can draw: General."* |
| the character | *"Aqui tienes la imagen de Crono."* + `![Crono](image_0.png)` |
| the Quest | **NO EVIDENCE YET** |

The Engine was right at every step and the conversation still ended in a lie. Nothing was
substituted, nothing was fabricated in the record, and a person reading the Chronicle was told a
picture existed.

### 1. The style was invented, and inventing it is what caused the failure

`General` was installed. It would have drawn this. What turned a working request into a refusal
was the model deciding, unprompted, that a Chrono Trigger character implies an *anime* style —
and `anime` was the one name with nothing behind it.

The descriptor had offered `pixel art`, `realistic`, `anime` as examples of what to ask for.
**That is a menu, and a model handed a menu orders from it.** So:

> **`style` is passed only when the user named one.** It is never inferred from the subject:
> somebody asking for a picture of a cartoon character has not asked for a cartoon style.

The fix is upstream of any prompt — the tool stops offering a list — and a test holds the
descriptor to naming no style at all. This is the second amendment's argument arriving from a
direction it did not anticipate: a closed set of six names does not only fail the person who
wants watercolour, it *invites* a model to pick one of the five that are dark.

### 2. A refusal must not read as a description of a situation

*"No workflow here draws anime. What this World can draw: General."* is a status report, and a
weak model completes a status report by narrating past it. It now opens with the outcome and
then says what to do with it:

> `REFUSED. Nothing was drawn and no picture exists. This World draws: General. Tell the user
> you cannot draw anime here, say what this World does draw, and offer to find something that
> draws anime. Never describe a picture as though one was made.`

This is `CLAUDE.md`'s existing rule turned the other way round. There, a result must not read as
an instruction to *call again* — `see_image` invited a sharper question and burned seven
rounds. Here, a result must not read as something a character can *talk over*. The two are the
same discipline: **what a tool says back is prompt, and it is read as such.**

The difference from the `see_image` mistake is that this instruction is **bounded and negative**.
It names what not to claim; it does not invite another attempt.

### 3. What is not fixed here, and is written down rather than left

`![Crono](image_0.png)` reached the Chronicle. Today it renders as literal text — verified,
not assumed — so nothing false was *shown*. But the Chronicle already draws real generated
pictures, and Rich Chronicle projects Markdown. **The one place a picture legitimately appears
is therefore forgeable by a model writing three characters**, and the day image Markdown renders,
a character can put a picture into a Quest's evidence that the Engine never made.

The rule that resolves it is the one this project already holds everywhere else: *a picture in a
Chronicle is an artifact the Engine produced, never text a model wrote.* The projection must
render images from `Entry::Produced` and never from Markdown in a message. Not built here
because nothing renders it yet; recorded here because the next person to make Markdown images
work would otherwise open the hole without noticing.

### What this does not change

The refusal itself was **correct** and stays. Nothing is substituted, the alternatives are named,
and no evidence is recorded for a picture nobody made. The defect was never that Epoch said no.
It was that Epoch said no in a sentence somebody could talk over, about a style nobody had asked
for.


### Verified, and two things the first diagnosis had wrong (2026-08-23)

Same sentence, same model, same World, rebuilt binary:

```text
said      "Haz una imagen de Crono de Chrono Trigger"
answered  "He aqui una representacion de Crono de Chrono Trigger."
produced  8914b23c27e2fbf4.png — "Crono from the video game Chrono Trigger — a classic JRPG
          art style."                                             drew in General, 8s
```

No `style` argument, so nothing was refused; a picture exists and the Quest holds it as evidence.
The markdown reference is gone too.

**A Style is not a gate on what can be drawn.** Measured on the turn immediately before the fix:
asked for *pixel art* explicitly, the model put `16-bit pixel art style` into **`describe`** rather
than into `style`, `General` drew it, and the result genuinely was pixel art. Epoch never knew a
style had been requested.

That is not a hole to close. Words in the description reach the prompt, which is what a
description is for, and a `describe` Epoch policed would be Epoch deciding what a picture may be
about. What it means is that the Style layer selects **how** something is drawn — which
workflow, which weights — and never **what may be asked for**. Both amendments above are about
selection, and neither claims otherwise; it is written down because the phrase *"nothing is
substituted"* could be misread as a promise that a Style is a fence.

**The fake markdown was never a reaction to the refusal.** It appeared on the *successful* turn
too, beside a real picture. It is a habit of the model, not a symptom, so the rule in point 3
stands on its own: a picture in a Chronicle is rendered from `Entry::Produced`, never from
Markdown in a message.

---

## Fourth amendment — one thing draws, and it is the panel (2026-08-28)

> **`draw_image` and `edit_image` are deleted.** A character asked for a picture opens the Studio
> Panel; the user makes the picture. `open_studio` is the only drawing capability.

### What this corrects

ADR-0033 built the panel and kept `draw_image` beside it, on the reasoning that *"hazme una imagen
de Chrono"* must keep working or Epoch is an application with a render button rather than a World.
That reasoning was right about what must keep working and wrong about what has to exist for it to.

Two capabilities answered one intent. Which one a character reached for was the model's decision,
and a model that picks the one taking a `describe` will take it — the panel was offered *after*
the picture, or not at all. ADR-0033's own sentence is that the panel comes first.

Measured in the window (Default World, 2026-08-28): *"hazme una imagen de Chrono"* opens the panel
in **11.4 s**, and GENERATE draws in **13.0 s**. The sentence still works.

### Why the guarantees get stronger rather than weaker

Every rule the three previous amendments added was a **sentence in a descriptor**, and a sentence
is a request a model may read past:

| The rule | How it was held | How it is held now |
|---|---|---|
| A character may not name a workflow | `style` is semantic, never a filename | there is no argument |
| A Style is never invented | the descriptor names no style, and a test holds it to that | there is no argument |
| A refusal must not read as something to talk over | `REFUSED. Nothing was drawn…` | there is nothing to refuse; the panel shows what is installed |
| A result must not hand over a filename | the file goes to evidence, not to the sentence | nothing is drawn inside the turn |

> **Two tools for one intent is a decision the model makes and the user lives with.** The wording
> of a descriptor is a request; the shape of a tool surface is a rule.

### What it also closed

A turn that called `draw_image` never ended — 901.4 s, three times, which was the driver's own
patience rather than a latency — while the picture always drew and always landed correctly as
evidence. Four diagnoses, each disproved by the next measurement. With nothing left to call, the
turn ends.

### What is untouched

**Codex.** It draws with its own `image_gen`, and Epoch witnesses the result rather than
authorising the tool — Epoch does not govern an agent's own tools.

**The Style layer, the workflow contract, the node-graph exception and everything under
*Refused*.** A Style still exists because a workflow serves it; the panel is still a form and
never a graph; Epoch still generates no World Pack artwork.

**`Easel`, `Asked`, `Shape` and `Detail`** survive with one consumer: the Connections deck's chain
test, which walks Style → workflow → compile → server → kept file and needs to draw one picture
through the real machinery. The halves that parsed a model's prose into a size and an effort went
with the capability.

### Cost, named

The World's defaults can no longer draw without the panel being opened once. That is the point —
twelve decisions assumed from one sentence is what ADR-0033 exists to stop — but it is a real
change in how a first picture feels, and it is here rather than glossed.

---

## Fifth amendment — the panel makes four things, and it is still one panel (2026-08-29)

The fourth amendment settled that **one thing draws and it is the panel**. Phase 12 then asked the
question that argument had not been tested against: what happens when the thing being made is not
a picture?

**Nothing new was needed, and that is the finding.** A video, a sound and a mesh are all
*something a graph produces and the vault keeps*. The panel grew a `MAKE` row —
`IMAGE · VIDEO · AUDIO · 3D` — and every layer underneath grew a variant rather than a sibling:
one `Beyond` on the `Ask`, one `Keeping` per family naming which nodes end the graph, one
`MadeFormat` sniffed from the bytes, one `epoch://` scheme, one Chronicle row.

Had a `make_video` capability been written, there would now be four of everything.

### A tab is a medium, and a family belongs to a medium

Measured by opening it: with LTXV, Stable Audio, ACE-Step and Hunyuan3D installed together, every
tab listed every model. `Base::makes()` answers which medium a family produces and `None` means
*offered under all of them* — a model Epoch cannot classify is not hidden, because **silence is
not no** and hiding a file the user installed is the worst version of that mistake.

### The keys a server answers under

`SaveAudio` reports under `audio`, `SaveGLB` under `3d`, `SaveVideo` under `video`. The picture
chain read `images` and only `images`, so the first sound generated correctly and arrived as
nothing at all. **A result key is part of a family's recipe**, not a constant of the server.

### And the thing that is not a picture still has to be shown

A mesh is the one medium the Chronicle genuinely could not display, and the fix is not a viewer:
`polyscope-rs` renders headless, so the model turns through 24 frames into a GIF and the existing
picture path carries it. **The preview is named after the mesh's own hash** — `<stem>.gif` beside
`<stem>.glb` — which is a statement rather than a convenience: it is a preview *of that mesh*, and
it is what lets the Chronicle find one without a second field on the artifact.

A mesh whose preview is missing still shows its row and still says a model was made. That matters,
because a preview can legitimately fail: a 2.8M-face mesh passes a GPU buffer limit, and the whole
point of naming the preview optional is that the thing that was asked for was already finished.

### What the media did not change

`draw_image` is still deleted, a character still asks for a **mood and never a filename**, a Style
still exists because a workflow serves it, and a tool result is still prompt. The panel is still a
form and never a node graph — in four media now, and it did not need to become one to get there.
