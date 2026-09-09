import { useEffect, useState } from "react";

import type { MachineView } from "../ipc/contracts";
import { fetchModelVariants } from "../ipc/launcher";

/**
 * Every quantisation a Hugging Face repository publishes, and which of them run here.
 *
 * ## Why a repository opens rather than resolving
 *
 * `unsloth/Qwen3.8-27B-GGUF` publishes 27 of these, from 6.19 GB to 55.6 GB. Epoch used to pick
 * one — the largest that fits — and that was a reasonable answer to a question nobody had asked
 * precisely. Somebody choosing between `UD-Q4_K_M` and `UD-Q4_K_XL` is making a real decision,
 * and 400 MB is not the whole of it.
 *
 * So the repository opens, exactly as it does on Hugging Face's own page: rows by width,
 * quantisations across.
 *
 * ## And the one thing their page cannot say
 *
 * Which of them this machine can run. A list of names and sizes is available anywhere; the
 * reason to have one inside Epoch is the verdict beside each — read from the same measurement
 * the Workshop's own field uses, against the same graphics card. A chip that says
 * `UD-Q4_K_XL · 17.6 GB` without saying it will not fit in 12.9 GB is Hugging Face's list with
 * Epoch's colours on it.
 *
 * ## The tag is not decoration
 *
 * `MTP` marks a module published beside the model — 1.37 GB next to a 27B. Without it, the
 * cheapest-looking entry in the 4-bit row is one that is not the model at all.
 */
export function ModelVariants({
  repo,
  machine,
  onChoose,
}: {
  readonly repo: string;
  readonly machine: MachineView | null;
  /** Picking one is picking exactly that file — the caller weighs and pulls it. */
  readonly onChoose: (pull: string) => void;
}) {
  const [groups, setGroups] = useState<Groups | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    setGroups(null);
    setFailure(null);
    void fetchModelVariants(repo).then((answer) => {
      if (!alive) return;
      if (typeof answer === "string") setFailure(answer);
      else setGroups(answer);
    });
    return () => {
      alive = false;
    };
  }, [repo]);

  if (failure) return <p className="notice notice--warn">{failure}</p>;
  if (!groups) return <p className="cc__hint">Reading what it publishes…</p>;
  if (groups.length === 0)
    return <p className="cc__hint">It publishes no GGUF files.</p>;

  return (
    <div className="mvar">
      {groups.map((group) => (
        <div key={group.bits} className="mvar__row">
          {/* Zero means the name said nothing about width — honest, and rare. */}
          <span className="mvar__bits">
            {group.bits > 0 ? `${group.bits}-bit` : "other"}
          </span>
          <div className="mvar__quants">
            {group.variants.map((variant) => {
              const verdict = fits(variant.bytes, machine);
              return (
                <button
                  key={(variant.tag ?? "") + String(variant.ownHead) + variant.quant}
                  type="button"
                  className={`mvar__quant mvar__quant--${verdict ?? "unknown"}`}
                  title={reason(variant, verdict, machine)}
                  onClick={() => onChoose(variant.pull)}
                >
                  {variant.tag && <i className="mvar__tag">{variant.tag}</i>}
                  {/*
                    **Not the same word as the tag above it**, which marks a module rather than a
                    model. This one is a whole quantisation that carries the prediction head, and
                    calling both of them `MTP` would say of an 18 GB model exactly what the tag
                    says of a 1.4 GB one — that it is not a version of the model.
                  */}
                  {variant.ownHead && <i className="mvar__head">+ HEAD</i>}
                  <b>{variant.quant}</b>
                  <span>{gb(variant.bytes)}</span>
                  {/*
                    A download that arrives in parts is worth knowing about before it starts,
                    and it is also the number that used to be wrong: a shard on its own was
                    being reported as the whole variant.
                  */}
                  {variant.files > 1 && (
                    <em className="mvar__parts">
                      {variant.files} files
                      {/*
                        And Ollama will not take them. Measured against its own answer rather
                        than inferred: a sharded tag returns *"Ollama does not yet support
                        pulling sharded GGUF via the registry"*. Said on the chip because that
                        is where somebody is choosing, and the file is still fetchable with the
                        Hugging Face CLI — one destination closes, the variant stays.
                      */}
                      {!variant.pullable && " · Ollama cannot pull these"}
                    </em>
                  )}
                </button>
              );
            })}
          </div>
        </div>
      ))}
    </div>
  );
}

type Groups = readonly {
  readonly bits: number;
  readonly variants: readonly {
    readonly quant: string;
    readonly bytes: number;
    readonly files: number;
    readonly pull: string;
    readonly tag: string | null;
    /** The same quantisation with the model own prediction head in it. Not the tag above. */
    readonly ownHead: boolean;
    readonly pullable: boolean;
  }[];
}[];

/**
 * Whether this one runs here.
 *
 * `null` when the machine has no reading to compare against — **not** a no. A machine with no
 * NVIDIA card has no VRAM number, and unknown is a different answer from *nothing fits*.
 *
 * The same tenth is left over that `Machine::fits` leaves: a model that exactly fills the memory
 * it runs in does not run.
 */
function fits(bytes: number, machine: MachineView | null): boolean | null {
  const free = machine?.vramFree ?? null;
  if (free === null) return null;
  return bytes + bytes / 10 <= free;
}

function reason(
  variant: { quant: string; bytes: number; tag: string | null; ownHead: boolean },
  verdict: boolean | null,
  machine: MachineView | null,
): string {
  const head =
    variant.tag === "MTP"
      ? `${variant.quant} — the MTP module, published beside the model rather than a version of it. `
      : variant.ownHead
        ? `${variant.quant}, with the model's own prediction head — the same quantisation plus one block, so llama.cpp can draft its own tokens (measured here: 47.6 tok/s against 45.8 without, and 3.7% slower if the head is never used). The head is weights: choosing the plain one puts this out of reach for ever. `
        : `${variant.quant} — `;
  if (verdict === null)
    return `${head}no video-memory reading on this machine, so nobody can say whether it runs here.`;
  // On a unified machine the words have to change with the architecture. The GPU reads system
  // memory directly, so there is no second pool to spill into: a model that does not fit does
  // not run slowly, it does not load. "It will spill into system memory" would be a correct
  // number wrapped in a description of a machine this is not.
  if (machine?.unified) {
    if (verdict) return `${head}fits in this machine's free unified memory.`;
    return `${head}larger than the ${gb(machine.vramFree ?? 0)} of unified memory free — on a machine that shares one pool there is nowhere for the rest to go.`;
  }
  if (verdict) return `${head}fits in this machine's free video memory.`;
  return `${head}larger than the ${gb(machine?.vramFree ?? 0)} free — it will spill into system memory and run slowly.`;
}

function gb(bytes: number): string {
  const value = bytes / 1_000_000_000;
  return `${value >= 10 ? value.toFixed(1) : value.toFixed(2)} GB`;
}
