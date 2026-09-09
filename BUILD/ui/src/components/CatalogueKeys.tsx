import { useEffect, useState } from "react";

import {
  catalogueKeys,
  forgetCatalogueKey,
  saveCatalogueKey,
  type CatalogueKeyRow,
} from "../ipc/launcher";

/**
 * The keys asset catalogues want, and what each one is for.
 *
 * ## Only the sources that need one
 *
 * Measured rather than assumed (2026-08-23): Civitai answers `401` to an anonymous download and
 * `200` to an anonymous *search*; Hugging Face answers both without an account. So exactly one
 * row appears, and a panel asking for a Hugging Face key would be asking for a credential
 * nothing would use — the same failure as a gauge nobody can explain.
 *
 * ## It says what the key can do, not only what Epoch does with it
 *
 * A Civitai key is an **account token**, not a download-scoped one. Epoch uses it to fetch a
 * file and nothing else, and somebody pasting one still deserves to know what they are handing
 * over. Saying so is cheap; discovering it later is not.
 *
 * ## Write-only, by construction
 *
 * There is no field on `CatalogueKeyRow` a value could arrive in, so this component cannot
 * display a stored key however it is written. That is ADR-0026's guarantee — a credential the
 * compiler keeps out — applied to a surface instead of to a file.
 */
export function CatalogueKeys() {
  const [rows, setRows] = useState<readonly CatalogueKeyRow[] | null>(null);
  const [typing, setTyping] = useState<Record<string, string>>({});
  const [said, setSaid] = useState<string | null>(null);

  const read = () => void catalogueKeys().then(setRows);
  useEffect(read, []);

  return (
    <div className="rm">
      <span className="rm__label">Catalogue keys</span>
      <p className="cc__hint">
        Some sites hand a file over only to an account. Searching never needs
        one, so a shelf works before you sign in anywhere.
      </p>

      {rows === null ? (
        <p className="cc__hint">Asking the vault…</p>
      ) : (
        <ul className="rdy__list">
          {rows.map((row) => (
            <li
              key={row.id}
              className={`rdy__row rdy__row--${row.held ? "ready" : "needsYou"}`}
            >
              <span className="rdy__dot" aria-hidden />
              <span className="rdy__name">{row.name}</span>
              <span className="rdy__note">
                {row.held ? "a key is stored" : row.neededFor}
              </span>

              {row.held ? (
                <button
                  type="button"
                  className="btn btn--mini"
                  title="Remove it from this machine"
                  onClick={() =>
                    void forgetCatalogueKey(row.id).then((failure) => {
                      setSaid(failure ?? `${row.name}'s key was removed.`);
                      read();
                    })
                  }
                >
                  FORGET
                </button>
              ) : (
                <>
                  <input
                    className="cedit__field"
                    type="password"
                    autoComplete="off"
                    spellCheck={false}
                    placeholder="paste your key"
                    value={typing[row.id] ?? ""}
                    onChange={(e) =>
                      setTyping({ ...typing, [row.id]: e.target.value })
                    }
                  />
                  <button
                    type="button"
                    className="btn btn--mini"
                    disabled={(typing[row.id] ?? "").trim() === ""}
                    onClick={() =>
                      void saveCatalogueKey(row.id, typing[row.id] ?? "").then(
                        (failure) => {
                          setSaid(
                            failure ??
                              `${row.name}'s key is stored, encrypted for this Windows account.`,
                          );
                          // Cleared whether or not it worked: a rejected key left in the box is a
                          // key sitting in the window's memory for no reason.
                          setTyping({ ...typing, [row.id]: "" });
                          read();
                        },
                      )
                    }
                  >
                    KEEP IT
                  </button>
                </>
              )}
            </li>
          ))}
        </ul>
      )}

      {/*
        Beside the thing it is about. A caution shown only after somebody has already pasted a
        token is a caution that arrived too late, so every row's is on screen from the start.
      */}
      {(rows ?? []).map((row) => (
        <p key={`${row.id}-why`} className="cc__hint">
          <b>{row.name}</b> — {row.caution} Get one at <code>{row.foundAt}</code>.
        </p>
      ))}

      {said && <p className="notice notice--warn">{said}</p>}
    </div>
  );
}
