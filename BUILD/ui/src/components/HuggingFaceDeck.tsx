import { useEffect, useState } from "react";

import { huggingFaceEverywhere, type HuggingFaceRow } from "../ipc/launcher";

/**
 * Whether each machine can put a model file on disk.
 *
 * ## Why it is a fleet reading rather than one
 *
 * `hf` on this computer says nothing about the machine lending a graphics card, and it is
 * *that* machine a model would be downloaded onto. A single line saying "Hugging Face CLI:
 * installed" would be true of the Host and read as true of everything — which is the shape of
 * every gauge that has had to be taken back out of this product.
 *
 * ## Three facts, kept apart
 *
 * **reached** · **installed** · **signed in**. They have three different fixes — wake the
 * machine, install the tool, sign in — and a surface that collapsed any two of them would send
 * somebody to do the wrong one. The same discipline the agents' readiness follows (ADR-0027),
 * where "Not logged in" was being reported as the character being unavailable.
 *
 * ## Measured, never remembered
 *
 * Each remote is asked now. The roster keeps what a machine answered when it was paired, and a
 * machine changes: somebody installs `hf` while Epoch is open, and a reading taken at pairing
 * time would keep saying otherwise for as long as the pairing lasts.
 */
export function HuggingFaceDeck() {
  const [rows, setRows] = useState<readonly HuggingFaceRow[] | null>(null);
  const [asking, setAsking] = useState(false);

  const read = () => {
    setAsking(true);
    void huggingFaceEverywhere().then((next) => {
      setRows(next);
      setAsking(false);
    });
  };

  useEffect(read, []);

  return (
    <div className="rm hfd">
      <div className="hfd__head">
        <span className="rm__label">Hugging Face CLI</span>
        <button
          type="button"
          className="btn btn--mini"
          disabled={asking}
          onClick={read}
        >
          {asking ? "ASKING…" : "ASK AGAIN"}
        </button>
      </div>

      {rows === null ? (
        <p className="cc__hint">Asking each machine…</p>
      ) : (
        <ul className="hfd__list">
          {rows.map((row) => (
            <li key={row.machine} className="hfd__row">
              <span className="hfd__where">
                {row.machine}
                {row.local && <i className="hfd__here">here</i>}
              </span>
              <span className={`hfd__state hfd__state--${state(row)}`}>
                {said(row)}
              </span>
              {/*
                Where it was found, so "not installed" is a fact somebody can go and check —
                and the reason PATH alone was not enough: its installer writes to
                `~/.local/bin`, which is not on the PATH of an application started from a
                desktop.
              */}
              {row.foundAt && <em className="hfd__at">{row.foundAt}</em>}
            </li>
          ))}
        </ul>
      )}

      <p className="cc__hint">
        What puts a GGUF on disk, for llama.cpp or LM Studio. Ollama needs none
        of this — it fetches its own. A machine without it downloads through
        Ollama instead, which is fewer things it can do rather than a fault.
      </p>
    </div>
  );
}

/** Which of the three facts is the one to report, worst first. */
function state(row: HuggingFaceRow): "cold" | "absent" | "out" | "ready" {
  if (!row.reached) return "cold";
  if (!row.installed) return "absent";
  if (!row.user) return "out";
  return "ready";
}

function said(row: HuggingFaceRow): string {
  switch (state(row)) {
    // Not "no hf": a machine nobody could reach was never asked, and saying it has none would
    // be inventing an answer on its behalf.
    case "cold":
      return "OFFLINE · could not be asked";
    case "absent":
      return "NOT INSTALLED";
    // Installed and signed out is a real, working state — it downloads anything ungated. Said
    // plainly rather than as a fault, because the fix is only needed for gated repositories.
    case "out":
      return `${row.version ?? "installed"} · signed out`;
    default:
      return `${row.version ?? "installed"} · ${row.user}`;
  }
}
