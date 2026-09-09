# The Picture Chain

> What Epoch can and cannot do with a model that arrives in parts — measured, on 2026-08-25 and
> again on 2026-08-27, against a real ComfyUI on the owner's machine.
>
> Written because the honest answer to *"if I download a new model tomorrow, will it work?"* is
> **it depends, and here is exactly what on**. A yes would have been a gauge nobody can explain.

---

## The question this answers

The Studio Panel (ADR-0033) asks the user for a model, its parts, a prompt and a size, and
composes a ComfyUI graph from them. For a **checkpoint** — one file that can draw alone — there is
nothing to decide. For a model that **arrives in parts**, four questions have to be answered
correctly or the server refuses the graph:

1. Which diffusion model.
2. Which text encoder(s), and **how many**.
3. Which `type` string the CLIP loader is given.
4. Which VAE.

Epoch helps with (2) and (3) where it has a measurement, and says nothing where it does not.

---

## What Epoch measures, and how well

| Question | Where the answer comes from | Coverage |
| --- | --- | --- |
| What **kind** a file is | tensor names in the safetensors header (`asset.rs::kind_of`) | checkpoint · diffusion model · text encoder · LoRA · VAE · ControlNet · embedding · upscaler |
| Which **family** a checkpoint is | tensor names and shapes (`asset.rs::base_from_names`) | **only** SD 1.5 · SD 2 · SDXL · SD 3 · Flux · Z-Image |
| What an **encoder** is | the width of its embedding table (`asset.rs::encoder_of`) | CLIP-L · CLIP-G · CLIP-H · T5-XXL · T5 · a language model |
| What a **VAE** is | only what the file claims about itself in `__metadata__` | whatever the author wrote, or nothing |

**The asymmetry is the point.** A checkpoint's family is readable for six families and comes back
`Unknown` for Qwen-Image, Chroma, HiDream and everything newer. An encoder is readable
for *every* file, because the embedding table's width is a fact — `[49408, 768]` is a CLIP-L
whatever the file is called.

---

## Which is why the guidance is keyed on the encoder

`comfy/sd.py` picks the text-encoder implementation **from the detected model file**, and reads
`clip_type` only to tell a Flux/Klein setup apart. Verified by drawing rather than by reading:

| model | encoder | `type` | result |
| --- | --- | --- | --- |
| Z-Image Turbo | `qwen_3_4b_fp8_mixed` | `stable_diffusion` | drew in **12.2 s** |
| Z-Image Turbo | `qwen_3_4b_fp8_mixed` | `qwen_image` | drew in **10.1 s** |
| Z-Image Turbo | `qwen_3_4b_fp8_mixed` | `flux2` | **failed** |

So the panel's rules, in the order they are consulted (`state.rs::assembly_for`):

1. **The checkpoint's own measured family**, when there is one. Flux → two encoders, `flux`.
   SD 3 → three encoders, no `type` at all. Z-Image → one encoder, a language model,
   `stable_diffusion`.
2. **Otherwise, the encoders that were picked** (`state.rs::by_encoder`):
   - CLIP-L **+** a T5 → Flux's arrangement, `flux` on the double loader.
   - a single **language model** → Z-Image / Qwen-Image, `stable_diffusion`, and `flux2` named as
     the one that will not work.
   - a single **CLIP-L** → Stable Diffusion.
3. **Otherwise nothing.** No count, no `type`, and a sentence saying so.

Step 1 must stay ahead of step 2, and step 2 must claim nothing about the model. Both were learnt
the hard way. Written the other way round, a Flux model with one CLIP-L chosen so far was told
*"Epoch measured this model as Stable Diffusion."* And with Z-Image still unreadable, choosing a
CLIP-L for one made Epoch infer *Stable Diffusion* from the choice and then **mark that same
CLIP-L as the file the model needs** — a machine agreeing with the mistake it had been handed.

So the fallback's sentences all open with *"Epoch could not read which family this model is"*,
describe what **was picked**, and invite a change; and its `wants` list is always empty, because
marking a row is a statement about the model and only a family read from the model's own tensors
may make one.

**Z-Image is read from a shape, and its neighbour is not.** ComfyUI's own detection tests
`cap_embedder.1.weight` beside `noise_refiner.0.attention.k_norm.weight` for the Lumina 2
arrangement and separates the two by the first dimension of that weight — 2304 is Lumina 2, 3840
is Z-Image. Plain Lumina 2 stays `Unknown` because nobody has drawn with one here; filing it by
association would be the guess this module refuses.

---

## So: will a model I download tomorrow work?

**It falls into one of four cases.** Only the first two are automatic.

### 1. A checkpoint — yes

One file that carries its own encoder and VAE. Nothing to choose, nothing to get wrong. Anything
`CheckpointLoaderSimple` can load.

### 2. A model in parts whose encoder Epoch already recognises — yes

If it loads with a CLIP-L, a T5, or a Qwen-style language model, rule 2 answers it even though the
family reader has never heard of the model — with the family unmarked and the sentence saying so.
Qwen-Image is this case today.

### 3. A model in parts with an encoder Epoch cannot read — it opens, and asks

The panel still lists every file with what it is, still requires an encoder, a family and a VAE,
and still draws once they are chosen. What is missing is the *suggestion*: the family list is the
loader's own 28 or 12 names, unmarked, and the user picks from the model's own page. **Nothing is
blocked and nothing is guessed.**

### 4. Not a picture at all — no

- **Video models** (Minimax H3, Wan, LTX-V, Mochi, Hunyuan Video). ~~No.~~ **Closed 2026-08-28.**
  Both halves of the refusal were real and both were fixed: `MadeFormat` delivers MP4 and WebM
  beside the PNG, and ADR-0034's long work means a capability may answer *started*. LTX-V is
  measured — 49 frames at 512×320 in **40.3 s**, from the panel's `VIDEO` tab, which is not cold
  any more. Sound and meshes arrived the same week; the tabs are `IMAGE · VIDEO · AUDIO · 3D`.

  **Wan and Hunyuan Video are still no, for a different reason**: their nodes are on this server
  and no model of either is, so writing their recipes would be writing them from memory.
- **Embeddings — asked, and the answer was that there is nothing to wire** (2026-08-27). Nothing
  in `/object_info` takes an embedding; ComfyUI gives them their own `GET /embeddings`, which
  answers a flat list of names **without extensions**. That endpoint exists *because* the only
  way to use one is to write `embedding:<name>` into the prompt — the editor needs the names for
  autocomplete.

  So the panel names what is installed and writes the token for you. Measured with a
  hand-built embedding on this machine, same seed, same everything:

  | prompt | picture |
  | --- | --- |
  | `a lighthouse` | `3e374484…` |
  | `a lighthouse embedding:epoch-probe` | `a3c38883…` — the embedding was read |
  | `a lighthouse embedding:not-installed` | `89eb85ec…` — **also different** |

  The third row is the reason the list is worth building. A name the server does not have is
  **not refused**: `embedding`, `:` and the misspelling are encoded as ordinary words and quietly
  change the picture. Typing one is guessing.

**Closed on 2026-08-27, and this section said otherwise for two days.** Both were listed here as
inventory nothing consumes, and both are consumed now:

- **Upscalers.** The panel has an `UPSCALE` step and the composed graph gets
  `UpscaleModelLoader` → `ImageUpscaleWithModel` between the decode and the save. Measured: 4K UHD
  through a 4× upscaler is 15360×8640 and 182 MB, drawn in 152 s.
- **ControlNets.** `ControlNetLoader` → `LoadImage` → `ControlNetApplyAdvanced`, and they
  **chain** — each apply takes a pair of conditionings and answers a pair, so several stack rather
  than compete. Each row also says what the ControlNet should *read*: a canny one reads edges, and
  handed a photograph it draws noise (measured, one variable at a time).

> A document that describes a hole somebody has already filled is worse than no document: it is
> read before deciding what to build, so it does not merely go stale, it misdirects. This page is
> the answer to *"will my new model work?"*, and answering it with last week's gaps is the same
> failure as a gauge with nothing behind it.

---

## How to extend it, when the day comes

**Adding a family Epoch can read** (`asset.rs::base_of`): find a tensor name or shape unique to
that architecture, add a rule, add a case to `assembly_for` with its encoder count, the sentence,
the `type` string and what it `wants`. The `type` is only ever reported when the live server
publishes it, so a name ComfyUI has never heard of is left unsaid rather than recommended into a
refusal.

**Adding an encoder** (`asset.rs::encoder_of`): read the width, add a variant, add a `plainly()`
name. Then it can appear in a `wants` list and be marked in the dropdown by comparison.

**Never add a rule from a filename.** `pixel_art_final_v3` is a claim; its embedding table is a
fact (ADR-0024, ADR-0032).

**And measure it by drawing.** Every table on this page came from a real graph on a real server.
A rule that was reasoned about and not drawn with is a rule that will be wrong in the combination
nobody is watching.
