import { useEffect, useState } from "react";

import type { ReadinessRow } from "../ipc/contracts";
import { fetchReadiness, signInAgent } from "../ipc/launcher";

/**
 * What this machine has, what it does not, and what to do about each.
 *
 * ## A reading, not a wizard
 *
 * A wizard is a sequence somebody has to finish. This is true right now, in any order, and a
 * person who has three of six things set up is not halfway through anything — they have three
 * things. So there is no progress bar, no step number and no "next".
 *
 * ## Above Connections, and not a deck of its own
 *
 * Connections already lists providers and agents with their notes. A second screen answering
 * *what does this machine have* would be a second list of the same facts, and two lists drift:
 * fix a sentence in one and the other keeps saying the old thing.
 *
 * So this is the summary at the top, and the per-provider detail stays below it where it
 * already was.
 *
 * ## What it does not list, and why that is the same rule
 *
 ## `omit`, and why nobody passes it any more
 *
 * This used to sit *below* the local runtimes panel and leave the three programs on this machine
 * out of its list, because that panel reported them a few centimetres above and could also start
 * one, install one and hand it every model on the disk. Two lists saying *llama.cpp is not
 * answering*, one of which can act, was the drift worth avoiding.
 *
 * The deck now answers its own question first (2026-09-06), so this reads **above** the
 * configuration — and the reasoning reverses with the order. **A summary that leaves out the
 * three programs you most want to know about is not a summary**, and what follows it is
 * configuration rather than a second opinion. The heading lost its *else* in the same breath: it
 * was true of a list that omitted them and is not true of this one.
 *
 * The prop stays because the argument for it was sound and could come back with a third caller.
 * Nothing passes it today, which is why the test that used to prove the omission now proves the
 * default includes everything.
 *
 * ## Nothing here is invented
 *
 * Every note is what the thing itself said, or what Epoch would need from the user. `unset` is
 * not a failure — it is *nothing to detect*, which is the honest state of a machine on somebody
 * else's network that nobody has typed an address for.
 */
export function Readiness({
  onConfigure,
  omit = [],
}: {
  readonly onConfigure?: () => void;
  /** Ids the panel above is already reporting and can act on. */
  readonly omit?: readonly string[];
}) {
  const [all, setAll] = useState<readonly ReadinessRow[]>([]);
  const [asked, setAsked] = useState(false);
  const [busy, setBusy] = useState(false);

  const read = () => {
    void fetchReadiness().then((rows) => {
      setAll(rows);
      setAsked(true);
    });
  };

  useEffect(read, []);

  /*
    **Nothing yet is not nothing.** This measures — agents' own programs, then every backend
    local and paired — and rendering `null` until it answered made a four-second probe look like
    a panel that had decided there was nothing to say. Same distinction `useThisMachine` draws
    between *unasked* and *absent*.
  */
  if (!asked) {
    return (
      <div className="rdy">
        <div className="rdy__head">
          <span className="rm__label">Everything that can answer</span>
          <span className="cc__hint">Measuring…</span>
        </div>
      </div>
    );
  }

  const rows = all.filter((row) => !omit.includes(row.id));
  if (rows.length === 0) return null;

  const ready = rows.filter((row) => row.state === "ready").length;

  return (
    <div className="rdy">
      <div className="rdy__head">
        {/*
          **Not "This machine" any more, and it had stopped being true.** The programs on this
          computer moved to the block above, which can start and stock them; what is left is the
          agents, the paired machines and whatever somebody typed in — and calling *another
          computer* this machine was the second wrong thing about that heading. The first was
          that the panel it sits in already carries the name.
        */}
        <span className="rm__label">Everything that can answer</span>
        {/*
          A count, never a percentage. "3 of 7" is a fact; "43% ready" implies a finish line
          that does not exist — most people will never want all seven.
        */}
        <span className="cc__hint">
          {ready} of {rows.length} measured and working
        </span>
        <button
          type="button"
          className="btn btn--mini"
          disabled={busy}
          onClick={read}
        >
          READ AGAIN
        </button>
      </div>

      <ul className="rdy__list">
        {rows.map((row) => (
          <li key={row.id} className={`rdy__row rdy__row--${row.state}`}>
            <span className="rdy__dot" aria-hidden />
            <span className="rdy__name">{row.name}</span>
            <span className="rdy__note">{row.note}</span>
            {row.act === "signIn" && (
              <button
                type="button"
                className="btn btn--mini"
                disabled={busy}
                onClick={() =>
                  void (async () => {
                    setBusy(true);
                    // Epoch opens the door and never holds the key: this starts the program's
                    // own sign-in and stops. Whether it worked is what READ AGAIN says.
                    await signInAgent(row.id);
                    setBusy(false);
                  })()
                }
              >
                SIGN IN
              </button>
            )}
            {row.act === "configure" && onConfigure && (
              <button type="button" className="btn btn--mini" onClick={onConfigure}>
                SET UP
              </button>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
