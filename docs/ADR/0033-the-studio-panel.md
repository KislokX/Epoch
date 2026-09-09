# ADR-0033: The Studio Panel — the character opens it, the user fills it in

- Status: Accepted
- Date: 2026-08-23
- Depends on: [[0005-capability-first-architecture]], [[0009-trust-engine]], [[0025-quests-own-the-work]], [[0026-characters-are-portable-assets]], [[0030-images-are-made-by-a-capability]], [[0031-the-asset-registry]], [[0032-the-generative-library]]
- Amends: [[0030-images-are-made-by-a-capability]] — its Style vocabulary stops being the only way in.
- Related architecture: [[../../CONTENT_PHILOSOPHY]], [[../../EXPERIENCE_CONSTITUTION]], [[../../CLAUDE]]
- Horizon: the panel, the compiler and images **IMPLEMENT NOW**; video, audio and 3D were **DESIGN NOW** and gated on Phase 12 — *all three shipped 2026-08-29*; a node editor **refused**

## Context — a downloaded LoRA is unusable, and the reason is not the LoRA

The owner asked what would happen after downloading a Civitai LoRA. Measured, 2026-08-23:
`LoraLoader` **appears nowhere in the codebase.** The only graph Epoch writes is
`CheckpointLoaderSimple → CLIPTextEncode ×2 → EmptyLatentImage → KSampler → VAEDecode → SaveImage`.
So the answer is *nothing happens* — the file lands, Epoch can even file it correctly after
ADR-0032, and there is no path from it to a picture short of hand-editing a workflow in ComfyUI
and attaching it as a Style. Which is the program this product exists to avoid opening.

The Style vocabulary was built for a real reason and it still holds: **a character must ask for a
mood, never a filename** (ADR-0030). But that rule constrains *the character*. It was quietly
extended to the user, and there it does damage — it makes the person who owns the machine, paid
for the card and downloaded the file unable to name what they downloaded. ADR-0025 already says
the user decides; a Style-only door contradicts it.

There is also a second failure of the same shape. A Style is one word, and a picture is a dozen
decisions: which checkpoint, which LoRAs and how strongly, aspect ratio, steps, seed, format. A
character guessing all twelve from a sentence will guess some of them wrong, and the user has no
way to correct it that is not another sentence.

## Decision

### 1. The Studio Panel: the character opens it; the user fills it in

A character never silently chooses a checkpoint on the user's behalf when the user wants control.
It **offers the panel**, and the panel is a form:

```text
MODEL          a checkpoint from the library, named
ADDITIONAL     LoRAs, each with a strength; trigger words shown
PROMPT         positive, negative
ASPECT RATIO   from the sizes this model reports it can do
OUTPUT         format, quality
ADVANCED ▾     steps, CFG, seed, sampler                     (folded)
               [ GENERATE ]
```

This is a **form, not a node graph**. `CLAUDE.md` forbids node graphs and the prohibition is
untouched: nothing here exposes an edge, a socket or a wire. The node-graph exception granted for
*importing somebody else's workflow* is likewise unchanged.

**Only what is installed appears.** The panel is a view over the Generative Library (ADR-0032),
read by hash through the registry (ADR-0031). Nothing is typed in; nothing is remembered from a
list somebody wrote.

### 2. The character asks first

`draw_image` survives (ADR-0030). *"Hazme una imagen de Chrono"* still works and still draws with
the World's defaults — deleting it would remove the one thing that makes this a World rather than
an application with a render button.

> **Superseded 2026-08-28 (ADR-0030's fourth amendment).** `draw_image` is deleted. The sentence
> still works and it opens the panel — measured in the window at 11.4 s, with GENERATE drawing in
> 13.0 s. Keeping both was what let a model answer the intent with the tool that skipped the
> person, which is the opposite of what this section is called.

What changes is that the character **asks before it assumes**: *do you want to set this up, or
shall I draw it?* Both answers are cheap, and neither is the character deciding for the user.
This is the same manners ADR-0025 requires of a handoff — the cause is visible, and the user is
the cause.

### 3. Styles stop being the vocabulary; they do not become a lie

ADR-0030's second amendment made a Style *derived* — it exists when something that can draw it
arrives. That stands, and the panel does not remove it: a Style remains the short way to say
*draw like this*, and the panel is the long way to say exactly what to do.

What the panel removes is Styles being the **only** way in, which is what made a downloaded LoRA
inert. A user who names a checkpoint has not violated any rule about moods and filenames, because
the user is not a character.

### 4. The Workflow Compiler is now load-bearing

Phase 11.6 described generalising `starter` from *one plain graph* to *a request plus resolved
assets*. The panel makes that the centre of the phase rather than an item in it. The compiler:

- asks the target server for its schema (`/object_info`) and compiles against **that** server —
  the local one or a lent one (ADR-0029), whichever will actually draw (measured, 2026-08-23: the
  deck once measured one machine and drew on another);
- picks a **recipe per architecture** — SD 1.5, SDXL, Flux and Z-Image do not share a graph — and
  the architecture comes from the registry's `base_model`, never from a filename;
- inserts `LoraLoader` per selected LoRA, chained, each with its strength;
- **ships no `.json`.** A graph derived from what this machine reports is what made `BUILD ME ONE`
  defensible, and it is unchanged here.

An **unknown** architecture stays unknown: the panel offers the plainest recipe and says so. It
does not guess, because a wrong recipe fails inside a render rather than at the door.

### 5. Compatibility is shown, never hidden

A LoRA whose base does not match the selected checkpoint is **greyed with the reason in a
person's words**, never removed from the list, and **USE IT ANYWAY** is always offered. Epoch
measured and said; the file is theirs and the answer is theirs — the same arrangement as a model
too large for this card (ADR-0032 §5).

An `unknown` base is not an incompatibility. It is shown plainly as unmeasured and left enabled.

### 6. VIDEO and 3D are cold tabs — *and both were lit, 2026-08-28 and 2026-08-29*

The panel carries `IMAGE · VIDEO · 3D` from the first version, and only `IMAGE` has light.

This is the Launcher's cold-instrument rule applied to a whole surface, and it is honest here for
a reason that is written down rather than hoped: both gate on **Phase 12**. Video needs *long
work* (a capability today answers inside the call — twelve seconds fit, four minutes do not) and
needs `asset.rs` to deliver something that is not a PNG. 3D needs something that can display a
mesh, and the World is 2D.

So each dark tab keeps its frame and names the subsystem that will light it. A tab that says
*what it waits for* is information; a tab that is missing teaches the user this product does not
do video, which is a different and false statement.

> **Closed, and the record is closed with it.** Phase 12 delivered long work (ADR-0034), MP4 and
> WebM out of `asset.rs`'s successor, and — for 3D — a headless renderer that turns a mesh into
> the one thing the Chronicle already knows how to show. The row is now
> `IMAGE · VIDEO · AUDIO · 3D`, all four measured in the window, and `AUDIO` was added between
> them rather than being another cold frame.
>
> **What is worth keeping from this section is that the tabs said what they waited for, and both
> conditions were then met rather than waived.** A cold tab is a promise about the future; this
> is what it looks like when one is kept.

### 7. Trust is unchanged

Generating is a capability call and goes through `decide()` exactly as it does today (ADR-0009).
The panel changes **what** is proposed, never **who** approves it. `GENERATE` is a request, not a
grant — and the approval shows the compiled request, because that is now the thing that could
surprise somebody.

## Why this is best

**It removes a limit without removing a guarantee.** The character still cannot name a file; only
the user can. The rule that stopped a model inventing `pixel_v3.json` is exactly as strong.

**It makes downloaded assets mean something.** After ADR-0032 a file lands correctly; after this
it is *reachable*. The two together are what turn the Workshop from a download manager into a
library.

**It is the same product philosophy one level in.** Epoch has never chosen the model for the user
(ADR-0027), the map (ADR-0028), the crew (ADR-0023) or the artwork (ADR-0024). Choosing the
checkpoint was the last place it still did, and it was the least defensible of them: the user is
the one who downloaded it.

## Alternatives considered

**Keep Styles as the only door and let a Style carry a LoRA.** Considered seriously — it is less
work and stays inside ADR-0030. Refused because it answers *watercolour* and never answers *this
LoRA at 0.8 with these trigger words*, and because it puts the user back in ComfyUI to author the
Style. It also drifts: every knob the user wants becomes another Style, and Styles become
filenames with nicer spelling.

**Expose ComfyUI's own UI in a webview.** Refused: it is the program this product exists to hide,
and it would make the panel's contents depend on which engine drew, breaking the one thing
ADR-0030 established — that a picture is a capability, not a provider.

**A node editor.** Refused by `CLAUDE.md`, and it would be refused anyway: the twelve decisions a
picture needs are a form, not a graph, and every motivating example the owner gave was a form.

**Ship the panel with video and 3D working.** Refused *at the time*: video needed Phase 12's long
work, and 3D had nothing to display a result with. Building either then meant faking a result or
blocking a window for four minutes. Both conditions were met in Phase 12 and both tabs work now —
which is the refusal being *retired by its own stated condition* rather than overruled.

## Consequences

- Phase 11's Workflow Compiler moves from *an item* to *the centre*, and grows a recipe per
  architecture.
- Creations stops rendering a Style list and starts rendering the library.
- The registry's `base_model` becomes user-visible, so *unknown* becomes a visible state and must
  read as unmeasured rather than broken.
- `draw_image` grows a preceding question, which is a change to a descriptor and therefore to what
  a model reads. Per ADR-0030's third amendment, the wording is bounded and negative: it must not
  invite a second call.
- EpochServices needs none of this. The panel is a Host surface; a lent machine draws what it is
  handed (ADR-0029). The compiler must nonetheless compile against the **lent** schema when the
  bench is lent.

## Risks

- **Recipe drift.** Four architectures is four graphs to keep working, and ComfyUI's node set
  moves. Mitigated by compiling against the live schema and refusing at the door when a class is
  missing, which is what `starter` already does.
- **The panel becomes ComfyUI.** Every field is a field somebody could ask for next. `ADVANCED ▾`
  stays folded and the default is that the user touches nothing (ADR-0026's rule) — and a field
  earns its place by a real request, not by parity.
- **Two doors, one result.** A picture from a sentence and a picture from the panel must be the
  same kind of evidence on the Quest (`Entry::Produced`), or History will grow two shapes.

## Open questions

- Does a per-character default checkpoint belong on the Character (ADR-0026's test: does it
  survive changing the engine)? Leaning **no** — a checkpoint is inventory, like a LoRA, and
  ADR-0031's amendment already refused to file a checkpoint next to a brain.
- Whether the panel remembers the last request per World. Cheap and probably right, but it is
  state, and state that is not derivable needs somewhere to live.
