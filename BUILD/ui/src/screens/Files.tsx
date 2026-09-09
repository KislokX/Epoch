/**
 * FILES — everything this World has made.
 *
 * ## The other half of MISSIONS
 *
 * That window answers *what has this World worked on*; this one answers *what came out of it*.
 * They are different questions and a Quest answers both, which is why the two are separate
 * windows over the same records rather than two tabs of one list.
 *
 * ## Enumerated from the Quests, never a folder
 *
 * The same rule deletion and export follow. A folder listing would show files this World did not
 * make, miss the ones it wrote into somebody's source tree, and could never say **which Quest
 * made a thing, or when** — which is most of what anybody is looking for when they come here.
 *
 * ## Nothing is softened
 *
 * A World that produced nothing shows nothing and says so. ADR-0025: a Quest that produced no
 * evidence produced nothing, and History has to be able to say that. A window that padded the
 * list with *"3 conversations"* would be describing effort rather than results.
 */

import { useEffect, useState } from "react";

import { Frame, Plate } from "../components/hud/Frame";
import { Overlay } from "../components/Overlay";
import { Pager, pageOf, slice } from "../components/Pager";
import { madeHere, openMade, pictureSrc, type MadeRow } from "../ipc/world";

/** How many fit before the list stops being readable. The number MISSIONS uses. */
const PER_PAGE = 15;

export function Files({ onClose }: { readonly onClose: () => void }) {
  const [all, setAll] = useState<readonly MadeRow[] | null>(null);
  const [page, setPage] = useState(0);
  const [said, setSaid] = useState<string | null>(null);

  useEffect(() => {
    void madeHere().then(setAll);
  }, []);

  const rows = all ?? [];
  const at = pageOf(page, rows.length, PER_PAGE);
  const shown = slice(rows, at, PER_PAGE);

  return (
    <Overlay label="Files" onClose={onClose}>
      <Frame className="missions" corner={10} studs fill="var(--ep-window)">
        <div className="missions__head">
          <Plate>FILES</Plate>
          <span className="missions__count">
            {all === null
              ? "reading…"
              : rows.length === 1
                ? "1 thing made"
                : `${rows.length} things made`}
          </span>
          <button
            type="button"
            className="dlg__close"
            onClick={onClose}
            aria-label="Close"
            title="Put this away"
          >
            ✕
          </button>
        </div>

        {all !== null && rows.length === 0 ? (
          <p className="hud__empty missions__none">
            Nothing yet. Everything the crew makes here — pictures, files,
            notes — is listed as it is made, with the conversation it came from.
          </p>
        ) : (
          <ol className="missions__list">
            {shown.map((made) => (
              <li key={`${made.reference}-${made.at ?? 0}`}>
                <button
                  type="button"
                  className="missions__row"
                  onClick={() =>
                    void openMade(made.reference).then(setSaid)
                  }
                  title={`Open ${made.reference}`}
                >
                  {made.shown && <Thumbnail file={made.reference} />}
                  <b>{made.summary || made.reference}</b>
                  <span className="missions__facts">
                    <i>{made.kind}</i>
                    <i>{made.questTitle}</i>
                    {/*
                      Compacted work has no time of its own — the record that carried it was
                      replaced by a continuity brief. Saying so beats printing the moment of
                      compaction, which is not when the thing was made.
                    */}
                    <i>{made.at === null ? "earlier" : when(made.at)}</i>
                  </span>
                </button>
              </li>
            ))}
          </ol>
        )}

        <Pager
          count={rows.length}
          perPage={PER_PAGE}
          at={at}
          onGo={setPage}
          label="Pages of things this World made"
        />
        {said && <p className="notice notice--warn">{said}</p>}
      </Frame>
    </Overlay>
  );
}

/**
 * The picture itself, in the list.
 *
 * Fetched by reference like every other picture in this World: the Engine resolves the vault's
 * own name to a `data:` URI, and a file that has gone resolves to nothing and is simply not
 * drawn — a broken image icon explains less than a row without one.
 */
function Thumbnail({ file }: { readonly file: string }) {
  const [gone, setGone] = useState(false);

  // **This list is where the cost showed.** Every row asked the Engine for the whole picture and
  // drew a hundred pixels of it: measured on one 182 MB render, 2135 ms to read and base64 it
  // across the IPC and 348 ms to undo that, before the browser had decoded anything — 3.4 s for
  // one thumbnail, on a page that draws several. Over Epoch's own scheme the browser fetches it,
  // caches it and decodes it on its own threads, and none of that happens in JavaScript.
  if (gone) return null;
  return (
    <img
      className="missions__thumb"
      src={pictureSrc(file)}
      alt=""
      // A row whose file has left the vault keeps its row and loses its picture, which is what
      // it did before — a broken-image icon explains less than a row without one.
      onError={() => setGone(true)}
    />
  );
}

/** When, in words somebody reads rather than a timestamp they decode. */
function when(at: number): string {
  const seconds = Math.max(0, (Date.now() - at) / 1000);
  if (seconds < 90) return "just now";
  const minutes = seconds / 60;
  if (minutes < 90) return `${Math.round(minutes)}m ago`;
  const hours = minutes / 60;
  if (hours < 36) return `${Math.round(hours)}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}
