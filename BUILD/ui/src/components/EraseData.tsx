import { useEffect, useState } from "react";

import type { ErasableView } from "../ipc/contracts";
import { erase, fetchErasable } from "../ipc/launcher";

/**
 * What Epoch is holding, and choosing what to let go of.
 *
 * ## Why this is a list and not a "clear cache" button
 *
 * *Clear cache* is a button that cannot say what it does, and the button that cannot say what it
 * does is the button nobody presses. What Epoch keeps is not one kind of thing: most of it is a
 * measurement it can take again in a second, and one line of it is **somebody's open
 * conversation** that looks exactly like a cache from here.
 *
 * So every row carries what it is, what losing it costs, and how much is held **right now**.
 * Nothing is ticked when this opens: erasing is something you choose, never something you
 * confirm.
 *
 * ## Zero is a reading
 *
 * A row holding nothing says `0 B` rather than disappearing. Vanishing would leave somebody
 * wondering whether Epoch is keeping something it will not name.
 */
export function EraseData() {
  const [rows, setRows] = useState<readonly ErasableView[]>([]);
  const [chosen, setChosen] = useState<readonly string[]>([]);
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);

  const measure = () => {
    void fetchErasable().then(setRows);
  };

  useEffect(measure, []);

  const held = rows.reduce((total, row) => total + row.bytes, 0);

  const run = async () => {
    setBusy(true);
    const failure = await erase(chosen);
    setBusy(false);
    setChosen([]);
    setSaid(failure ?? "Erased.");
    // Measured again rather than assumed: what a removal left behind is a fact, and this panel
    // has just been told what to show for it.
    measure();
  };

  return (
    <div className="erase">
      <div className="erase__head">
        <span className="rm__label">Erase data</span>
        <span className="cc__hint">{size(held)} held</span>
      </div>

      {rows.map((row) => (
        <label key={row.id} className="erase__row">
          <input
            type="checkbox"
            checked={chosen.includes(row.id)}
            onChange={(e) =>
              setChosen(
                e.target.checked
                  ? [...chosen, row.id]
                  : chosen.filter((id) => id !== row.id),
              )
            }
          />
          <span>
            <b>
              {row.what} · {size(row.bytes)}
            </b>
            <i>{row.cost}</i>
          </span>
        </label>
      ))}

      <div className="art__actions">
        <button
          type="button"
          className="btn btn--mini"
          disabled={busy || chosen.length === 0}
          onClick={() => void run()}
        >
          {busy ? "ERASING…" : "ERASE SELECTED"}
        </button>
        {said && <span className="cc__hint">{said}</span>}
      </div>
    </div>
  );
}

/**
 * Bytes as a person reads them.
 *
 * `0 B` rather than "empty": it is a measurement, and writing it as a word would make it look
 * like a state somebody chose.
 */
function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
