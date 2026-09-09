# Flash attention `off` does not work — and the scope is the finding

Confirmed 2026-08-31, in **every run of every search** that included it. Recorded with its exact
scope rather than as a fact about flash attention, because the only thing measured is one
combination.

## Scope

| | |
|---|---|
| hardware | NVIDIA GeForce RTX 4070 SUPER, 12 GB |
| OS | Windows 11 Pro 10.0.26200 |
| backend | CUDA (`~/.llama/bin` build, not the winget Vulkan one) |
| llama.cpp | `0.3.0-dev` build **10622**, commit `3737e4137` |
| artefact | `Qwen3.6-35B-A3B-UD-IQ4_XS`, 17,730,509,792 bytes, `unsloth/Qwen3.6-35B-A3B-GGUF` |
| context | 32,768 |

**Not generalised to anything.** Not to other models, not to other quantisations of this one, not
to other backends, not to other builds, and not to flash attention as a technique. Another
combination is another measurement.

## What happens

`--flash-attn off`:

- **no answer at all** — every request in every attempt, across four separate searches
- system memory climbs from about **28 GB to 33.7 GB**
- the card reports high peak utilisation with a very low mean (96% / 12%), which is the shape of
  work starting and not finishing

`--flash-attn on` and llama.cpp's own `auto` (the flag not written at all) both answer normally on
the same model in the same searches, which is what makes this a property of the value rather than
of the model or the moment.

## Why it is recorded as `Invalid` rather than as slow

`health::State::Invalid` means *it did not produce a usable reading at all* — distinct from
`Unstable`, which is a configuration that answered and disagreed with itself. A profile can never
be built on either, and only one of them has a number.

## What it does *not* say

It does not say that flash attention `off` caused the GPU residency degradation this machine has
been in. The control was already at 28.0 tok/s before that search began, and the sentinel named
`Flash attention · off` as the candidate the control stopped reproducing *after* — which is where
it happened, not proof of what did it. Re-measuring on a clean machine is what would tell the two
apart.
