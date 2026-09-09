/**
 * The lamp over a conversation: is this character's brain on the graphics card?
 *
 * ## Why a lamp and a switch, and not one thing
 *
 * The obvious build is a toggle that lights up when it is on. That is a gauge reading itself —
 * it says green because you pressed it, which you already knew, and it goes on saying green
 * through every reason the model is *not* actually there: llama.cpp's router holds one model at
 * a time and evicts this one for somebody else's turn, LM Studio runs its own idle countdown,
 * and a hold has a deadline. It also says red for a model that *is* resident because somebody
 * opened it in LM Studio's own window.
 *
 * So the light is a **reading**, measured from the server that holds it, and the switch is an
 * **intent**. They usually agree. When they do not, the card is right.
 *
 * ## What it is worth
 *
 * Measured on this machine, a 27B at IQ2_M over llama.cpp: with the model released after each
 * answer, every message paid ~6 s reading 10.6 GB off disk and ~13 s re-evaluating the prompt
 * from cold. Held, the second turn's prompt evaluation was 906 ms — the server reused its cache
 * (`f_sim_best 0.985`). ~19 s per message, and none of it visible to anybody.
 *
 * **Which is why holding is the default now** (2026-08-28). It used to follow `concurrentCrew`,
 * so the ordinary machine paid that 19 s on every single message to protect against a cost that
 * only *several* models have. This switch is now the exception — turning it off for one
 * character — rather than the only way to avoid a reload.
 *
 * It does **not** make generation faster. That model answers at ~7 tok/s held or cold.
 *
 * ## What it will not do
 *
 * Turning it on loads nothing. Reading a model onto the card costs seconds and gigabytes and
 * there is nothing to say with it yet; the next turn loads it and it stays. A lamp that went
 * green on a click would be green before the model was there, which is the one thing it is not
 * allowed to mean.
 */

import { useCallback, useEffect, useState } from "react";

import { fetchWarmth, holdWarm } from "../../ipc/world";
import type { Warmth } from "../../ipc/world";

/**
 * How often the card is re-read.
 *
 * The reading changes for reasons Epoch never hears about — another program loading a model, a
 * server's own idle timer, a router evicting one for another — so there is nothing to be
 * notified by and it has to be asked. Three seconds is faster than a person loses interest in a
 * lamp and slow enough that a localhost GET is not a cost.
 */
const RE_READ_MS = 3_000;

interface KeepWarmProps {
  /** Whose brain this is about. */
  readonly characterId: string;
  /**
   * Bumped by the parent whenever a turn ends, so the lamp catches up at the one moment it is
   * guaranteed to have changed instead of waiting out the poll.
   */
  readonly settled: unknown;
}

export function KeepWarm({ characterId, settled }: KeepWarmProps) {
  const [warmth, setWarmth] = useState<Warmth | null>(null);
  const [working, setWorking] = useState(false);

  useEffect(() => {
    let alive = true;
    const read = () => {
      void fetchWarmth(characterId).then((w) => {
        if (alive) setWarmth(w);
      });
    };
    read();
    const timer = window.setInterval(read, RE_READ_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [characterId, settled]);

  const flip = useCallback(() => {
    if (!warmth || working) return;
    setWorking(true);
    void holdWarm(characterId, !warmth.holding)
      .then((w) => {
        if (w) setWarmth(w);
      })
      .finally(() => setWorking(false));
  }, [characterId, warmth, working]);

  // Nothing to hold and nothing to read: an agent brain, a hosted model, or a local server
  // that is not answering. No control, rather than a control that governs nothing.
  if (!warmth || warmth.resident === null) return null;

  const lit = warmth.resident;
  return (
    <button
      type="button"
      className={
        `epbtn keepwarm${lit ? " keepwarm--lit" : ""}` +
        // **The emphasis marks a choice, never the default** (2026-08-28).
        //
        // `--on` used to follow `holding`, which was worth drawing while holding was the
        // exception: the setting had to be turned on, so a gold button meant somebody had done
        // something. Holding is the default now, so the same rule painted every button in the
        // World gold — a mark on everything marks nothing, and the owner noticed the button had
        // changed shape before anything else.
        //
        // So: gold when this character was pinned on purpose, dim when it was let go on purpose,
        // and the plain button — exactly what it was — for everybody nobody has touched. The
        // lamp keeps saying what is actually on the card, which is the fact that moves.
        (warmth.chosen
          ? warmth.holding
            ? " keepwarm--on"
            : " keepwarm--off"
          : "")
      }
      onClick={flip}
      disabled={working}
      aria-pressed={warmth.holding}
      title={
        // Both facts, in the order somebody reads them: what is true now, then what will
        // happen next. Never a promise the lamp cannot back up.
        `${warmth.model} — ${lit ? "on the card now" : "not loaded"}. ` +
        (warmth.holding
          ? "Stays loaded after each answer; click to unload it now."
          : "Unloaded after each answer; click to keep it loaded.") +
        (warmth.chosen ? "" : " (nobody has chosen for this character)")
      }
    >
      {/*
        The lamp says what is on the card; the word says what the switch is set to. Labelling
        the button with the reading too would be one fact drawn twice — and the two are allowed
        to disagree, which is the entire reason the lamp exists.
      */}
      <span className="keepwarm__lamp" aria-hidden="true" />
      KEEP
    </button>
  );
}
