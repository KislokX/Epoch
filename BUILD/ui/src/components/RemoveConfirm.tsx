import { useEffect, useState } from "react";

import type { RemovalView } from "../ipc/contracts";

/**
 * Removing something, and reading what that removes before it happens.
 *
 * ## Three lists, because "are you sure?" is a question about nothing
 *
 * The Engine builds the plan — **what goes, what changes, what survives** — and this shows all
 * three. The third is the one people forget to write and the one that stops a correct deletion
 * from reading as a broken one: somebody who removes a character and then finds their name in a
 * conversation would reasonably think it failed.
 *
 * ## Gold, not a traffic light
 *
 * The first draft used red, amber and green. That is semantics imported from somewhere else: the
 * Launcher speaks in gold and opacity, and three new colours would have made this one window
 * feel like a different application. Weight carries the difference instead — what is deleted is
 * brightest, what survives is quietest — which is the same grammar every other panel here uses.
 *
 * ## Typing the name
 *
 * A red button gets pressed without reading, and the three lists above exist to be read. Typing
 * the name is the cheapest thing that makes somebody look at *which* one they are removing —
 * and it costs nothing to the person who meant it.
 */
export function RemoveConfirm({
  plan,
  busy,
  onCancel,
  onConfirm,
}: {
  /** What the Engine says this will do. `null` while it is being asked. */
  readonly plan: RemovalView | null;
  readonly busy: boolean;
  readonly onCancel: () => void;
  readonly onConfirm: () => void;
}) {
  const [typed, setTyped] = useState("");

  // A different plan is a different thing being removed, so the name typed for the last one
  // must not carry over and unlock this one.
  useEffect(() => setTyped(""), [plan?.what]);

  if (!plan) {
    return (
      <p className="cc__hint rm__asking">Reading what that would remove…</p>
    );
  }

  const ready = typed.trim() === plan.what.trim();

  return (
    <div className="rm">
      <span className="rm__label">Remove {plan.what}</span>

      <div className="rm__list rm__list--goes">
        <span className="rm__heading">Deleted</span>
        {plan.files.map((file) => (
          <span key={file} className="rm__file">
            {file}
          </span>
        ))}
      </div>

      {plan.consequences.length > 0 && (
        <div className="rm__list rm__list--changes">
          <span className="rm__heading">Changes</span>
          {plan.consequences.map((line) => (
            <span key={line}>{line}</span>
          ))}
        </div>
      )}

      {plan.survives.length > 0 && (
        <div className="rm__list rm__list--survives">
          <span className="rm__heading">Survives</span>
          {plan.survives.map((line) => (
            <span key={line}>{line}</span>
          ))}
        </div>
      )}

      <label className="rm__gate">
        <span>
          Type <b>{plan.what}</b> to confirm
        </span>
        <input
          type="text"
          value={typed}
          onChange={(e) => setTyped(e.target.value)}
          autoComplete="off"
          spellCheck={false}
        />
      </label>

      <div className="art__actions">
        <button
          type="button"
          className="btn btn--mini"
          disabled={busy}
          onClick={onCancel}
        >
          CANCEL
        </button>
        <button
          type="button"
          className="btn btn--mini rm__go"
          // Disabled until the name matches, which is the whole point of asking for it.
          disabled={busy || !ready}
          onClick={onConfirm}
        >
          {busy ? "REMOVING…" : "REMOVE"}
        </button>
      </div>
    </div>
  );
}
