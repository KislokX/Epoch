/**
 * The Workshop deck — where the crew's tools come from.
 *
 * Two doors. The MCP Workshop is open; the Models Workshop is not built and says so, keeping its
 * frame and losing its light, which is what a deck that exists but is not finished should look
 * like.
 *
 * ## The button shows the command
 *
 * Installing an MCP server means running a third party's program on this machine, and every tool
 * it offers is registered with every effect and no reversal because MCP declares neither
 * (ADR-0008). So the card carries who published it, which version, and the literal command with
 * its arguments — before the button, never behind it. There is no "Install" here that hides what
 * it does.
 *
 * ## Nothing on this screen is composed
 *
 * Whether a server can run, and why not, are computed by the Engine and rendered. A second copy
 * of that rule living here would eventually disagree with the one the install path uses, and the
 * disagreement would look like a button that lies.
 */

import { useCallback, useEffect, useState } from "react";

import { forgetMcp, installFromWorkshop, searchWorkshop } from "../ipc/launcher";
import { FindAssets } from "../components/FindAssets";
import { FindVoices } from "../components/FindVoices";
import { ModelsWorkshop } from "../components/ModelsWorkshop";
import { openLink } from "../ipc/world";
import type { WorkshopInput, WorkshopOffer, WorkshopShelf, WorkshopShown } from "../ipc/contracts";

type Door = "none" | "mcp" | "models" | "creation" | "voices";

interface WorkshopPanelProps {
  /** Take a freshly downloaded model to MODELS and map its curve there. */
  readonly onMeasure?: (model: string) => void;
  /** Re-read the MCP configuration after an install, so the deck next door is never stale. */
  readonly onInstalled: () => void | Promise<void>;
}

/** The command, exactly as it would be run. */
function Command({ offer }: { readonly offer: WorkshopOffer }) {
  if (offer.kind !== "start") return null;
  return (
    <code className="wk__cmd">
      {offer.command} {offer.args.join(" ")}
    </code>
  );
}

function inputsOf(offer: WorkshopOffer): readonly WorkshopInput[] {
  return offer.kind === "start" ? offer.inputs : [];
}

/**
 * The form a server needs filled, generated from what the catalogue declared.
 *
 * Never written per server. A Workshop that knew Notion wants `NOTION_TOKEN` would need editing
 * to add the next one, which is the mistake ADR-0026 named for Provider controls.
 */
function Inputs({
  inputs,
  answers,
  onChange,
}: {
  readonly inputs: readonly WorkshopInput[];
  readonly answers: Record<string, string>;
  readonly onChange: (name: string, value: string) => void;
}) {
  return (
    <div className="wk__inputs">
      {inputs.map((input) => (
        <label key={input.name} className="wk__input">
          <span>
            {input.name}
            {input.required && <i className="wk__req">required</i>}
            {input.secret && <i className="wk__secret">kept encrypted</i>}
          </span>
          {input.choices.length > 0 ? (
            <select
              value={answers[input.name] ?? input.default ?? ""}
              onChange={(e) => onChange(input.name, e.target.value)}
            >
              <option value="">—</option>
              {input.choices.map((choice) => (
                <option key={choice} value={choice}>
                  {choice}
                </option>
              ))}
            </select>
          ) : (
            <input
              // A credential is never drawn in the clear, and it is never read back afterwards
              // either: the value goes to the encrypted store and no command returns it.
              type={input.secret ? "password" : "text"}
              value={answers[input.name] ?? input.default ?? ""}
              placeholder={input.placeholder ?? ""}
              onChange={(e) => onChange(input.name, e.target.value)}
            />
          )}
          <em>{input.description}</em>
        </label>
      ))}
    </div>
  );
}

function Card({
  entry,
  onInstalled,
}: {
  readonly entry: WorkshopShown;
  readonly onInstalled: (id: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);

  const { listing, ready, blocked, installedAs } = entry;
  const inputs = ready ? inputsOf(ready) : [];
  const have = installedAs.length > 0;

  async function install() {
    if (!ready) return;
    setBusy(true);
    setFailed(null);
    const outcome = await installFromWorkshop(listing, ready, answers);
    setBusy(false);
    if ("error" in outcome) {
      setFailed(outcome.error);
      return;
    }
    setOpen(false);
    setAnswers({});
    onInstalled(outcome.id);
  }

  async function remove(id: string) {
    setBusy(true);
    setFailed(null);
    const failure = await forgetMcp(id);
    setBusy(false);
    if (failure) {
      setFailed(failure);
      return;
    }
    onInstalled(id);
  }

  return (
    <article className={`wk__card${blocked ? " wk__card--cold" : ""}`}>
      <header className="wk__head">
        <h4>{listing.title}</h4>
        <span className="wk__ver">{listing.version}</span>
      </header>
      <p className="wk__by">
        {listing.publisher}
        {!listing.active && <b className="wk__gone"> · withdrawn from the registry</b>}
        {have && <b className="wk__have"> · installed as {installedAs.join(", ")}</b>}
      </p>
      <p className="wk__desc">{listing.description}</p>

      {ready ? (
        <>
          <Command offer={ready} />
          {open && inputs.length > 0 && (
            <Inputs
              inputs={inputs}
              answers={answers}
              onChange={(name, value) => setAnswers({ ...answers, [name]: value })}
            />
          )}
          <div className="wk__actions">
            {/*
              Once it is here, removing it is the offer. Installing a second copy is still
              possible — the button says AGAIN rather than disappearing — but it is no longer the
              obvious thing to click, which is how the third copy happened.
            */}
            {have &&
              installedAs.map((id) => (
                <button
                  key={id}
                  type="button"
                  className="btn btn--mini"
                  disabled={busy}
                  onClick={() => void remove(id)}
                >
                  {`UNINSTALL ${id}`}
                </button>
              ))}
            {inputs.length > 0 && !open ? (
              <button type="button" className="btn btn--mini" onClick={() => setOpen(true)}>
                {have ? "INSTALL AGAIN" : `SET UP · ${inputs.length}`}
              </button>
            ) : (
              <button
                type="button"
                className="btn btn--mini"
                disabled={busy}
                onClick={() => void install()}
              >
                {busy ? "WORKING…" : have ? "INSTALL AGAIN" : "INSTALL"}
              </button>
            )}
            {open && (
              <button type="button" className="btn btn--mini" onClick={() => setOpen(false)}>
                CANCEL
              </button>
            )}
            {/*
              A button, not an anchor. A Tauri webview ignores `target="_blank"` — the link
              looked right and did nothing — and the Engine checks the scheme before anything
              opens, because these addresses came from a catalogue written by strangers.
            */}
            {listing.repository && (
              <button
                type="button"
                className="wk__src"
                onClick={() => void openLink(listing.repository!)}
              >
                SOURCE
              </button>
            )}
          </div>
        </>
      ) : (
        /* The Engine's sentence, not one composed here. */
        <p className="wk__blocked">{blocked}</p>
      )}
      {failed && <p className="wk__failed">{failed}</p>}
    </article>
  );
}

/**
 * Shortcuts into the search, and **not** categories.
 *
 * The registry has no notion of a category, and its `search` was measured to match the server's
 * **name** only — `pdfassistant` says "redact" in its description and does not appear in a search
 * for `redact`. So these are preset searches, they are labelled as such next to the row, and one
 * of them finds servers *named* for a subject rather than every server that does it.
 *
 * A chip pretending to be a category would be the worse thing: it would look complete, quietly
 * miss whatever is named differently, and train somebody to trust a filter that lies.
 */
const SHORTCUTS = [
  "pdf",
  "excel",
  "image",
  "browser",
  "database",
  "filesystem",
  "git",
  "search",
  "email",
  "calendar",
  "slack",
  "notion",
] as const;

export function WorkshopPanel({ onInstalled, onMeasure }: WorkshopPanelProps) {
  const [door, setDoor] = useState<Door>("none");
  const [query, setQuery] = useState("");
  const [shelf, setShelf] = useState<WorkshopShelf | null>(null);
  const [busy, setBusy] = useState(false);
  const [added, setAdded] = useState<string | null>(null);
  /**
   * The cursors already walked through, so BACK works.
   *
   * The registry hands out forward cursors only — there is no "previous" to ask for — so going
   * back means remembering where each page started. Kept as a trail rather than a page number
   * because a cursor is the registry's own bookmark and a number would be Epoch guessing at one.
   */
  const [trail, setTrail] = useState<string[]>([]);

  const look = useCallback(async (q: string, cursor?: string) => {
    setBusy(true);
    setShelf(await searchWorkshop(q, cursor));
    setBusy(false);
  }, []);

  /** Any new search starts at the beginning, and the trail with it. */
  const start = useCallback(
    (q: string) => {
      setQuery(q);
      setTrail([]);
      void look(q);
    },
    [look],
  );

  useEffect(() => {
    if (door === "mcp" && shelf === null) void look("");
  }, [door, shelf, look]);

  if (door === "none") {
    return (
      <section className="pnl">
        <h2 className="pnl__title">WORKSHOP</h2>
        <p className="cc__hint">
          Where the ship is fitted out. Tools the crew can use, and the models that think for them.
        </p>
        <div className="wk__doors">
          <button type="button" className="wk__door" onClick={() => setDoor("mcp")}>
            <b>MCP WORKSHOP</b>
            <span>Find and install servers the crew can use.</span>
            <i>OPEN</i>
          </button>
          {/*
            Lit, 2026-08-19. It was cold and named what had to exist first — the Ollama surface,
            which now does: what this machine is, what it has, and what a model really weighs.
          */}
          <button
            type="button"
            className="wk__door"
            onClick={() => setDoor("models")}
          >
            <b>MODELS WORKSHOP</b>
            <span>What this machine can run, measured.</span>
            <i>OPEN</i>
          </button>
          {/*
            The third shelf (ADR-0031's amendment). **A Brain is not a checkpoint**: one is
            identity and the other is inventory, and a single list holding `gemma4:12b` beside
            `sd_xl_base_1.0.safetensors` would group them by the only thing they share — being a
            large file.

            Downloading lives here rather than on the Creations deck because fetching gigabytes
            is fitting out the ship, not using it. Creations shows what arrived.
          */}
          <button
            type="button"
            className="wk__door"
            onClick={() => setDoor("creation")}
          >
            <b>CREATIONS WORKSHOP</b>
            <span>Find and install what draws — models, LoRAs, VAEs.</span>
            <i>OPEN</i>
          </button>
          {/*
            The fourth shelf (Phase 15). One `Asset`, one `Catalogue`, one `fetch`, one install
            path — and its own screen, because a voice has no answer to a base-model filter, an
            adult filter, a medium filter, a download count or a preview picture. Five controls a
            row cannot answer is a screen that teaches you to stop reading the controls.
          */}
          <button
            type="button"
            className="wk__door"
            onClick={() => setDoor("voices")}
          >
            <b>VOICES WORKSHOP</b>
            <span>Find and install voices the crew can speak with.</span>
            <i>OPEN</i>
          </button>
        </div>
      </section>
    );
  }

  if (door === "voices") {
    return (
      <section className="pnl">
        <div className="wk__bar">
          <button
            type="button"
            className="btn btn--mini"
            onClick={() => setDoor("none")}
          >
            BACK
          </button>
          <h2 className="pnl__title">VOICES WORKSHOP</h2>
        </div>
        <FindVoices />
      </section>
    );
  }

  if (door === "creation") {
    return (
      <section className="pnl">
        <div className="wk__bar">
          <button
            type="button"
            className="btn btn--mini"
            onClick={() => setDoor("none")}
          >
            BACK
          </button>
          <h2 className="pnl__title">CREATIONS WORKSHOP</h2>
        </div>
        <FindAssets />
      </section>
    );
  }

  if (door === "models") {
    return (
      <section className="pnl">
        <div className="wk__bar">
          <button
            type="button"
            className="btn btn--mini"
            onClick={() => setDoor("none")}
          >
            BACK
          </button>
          <h2 className="pnl__title">MODELS WORKSHOP</h2>
        </div>
        <ModelsWorkshop onMeasure={onMeasure} />
      </section>
    );
  }

  return (
    <section className="pnl">
      <div className="wk__bar">
        <button type="button" className="btn btn--mini" onClick={() => setDoor("none")}>
          BACK
        </button>
        <h2 className="pnl__title">MCP WORKSHOP</h2>
      </div>

      <form
        className="wk__search"
        onSubmit={(e) => {
          e.preventDefault();
          start(query);
        }}
      >
        <input
          type="search"
          value={query}
          placeholder="Search the registry — notion, filesystem, browser…"
          onChange={(e) => setQuery(e.target.value)}
        />
        <button type="submit" className="btn btn--mini" disabled={busy}>
          {busy ? "…" : "SEARCH"}
        </button>
      </form>

      <div className="wk__chips">
        {SHORTCUTS.map((word) => (
          <button
            key={word}
            type="button"
            className={`chip${query === word ? " chip--on" : ""}`}
            onClick={() => start(query === word ? "" : word)}
          >
            {word}
          </button>
        ))}
      </div>
      {/*
        Said plainly rather than left to be discovered. These are searches, and the registry
        matches a server's name — so this finds what is *named* for a subject, not everything
        that does it. A chip that looked like a category would quietly miss the rest.
      */}
      <p className="cc__hint wk__note">
        Shortcuts into the search. The registry matches server names, so these find servers named
        for a subject rather than every server that can do it.
      </p>

      {added && (
        <p className="wk__added">
          Installed as <b>{added}</b>. Give it to a character from the Characters deck.
        </p>
      )}

      {/*
        Measured, and only what was measured. `docker` absent is a fact about this machine, and
        it is what makes a blocked card's sentence make sense.
      */}
      {shelf && (
        <p className="cc__hint">
          This machine runs:{" "}
          {shelf.runtimes.present.length > 0 ? shelf.runtimes.present.join(" · ") : "no package runner"}
          {shelf.fetchedMs !== null && (
            <> · catalogue from {new Date(shelf.fetchedMs).toLocaleDateString()}</>
          )}
        </p>
      )}
      {shelf?.problem && <p className="wk__failed">{shelf.problem}</p>}

      <div className="wk__grid">
        {shelf?.shown.map((entry) => (
          <Card
            key={entry.listing.name}
            entry={entry}
            onInstalled={(id) => {
              setAdded(id);
              void onInstalled();
              void look(query, trail[trail.length - 1]);
            }}
          />
        ))}
      </div>

      {/*
        The registry hands out forward cursors only, so BACK is the trail rather than a request.
        Both buttons are absent — not disabled-looking — when there is nowhere to go, because a
        dead control is one more thing to read.
      */}
      {shelf && (trail.length > 0 || shelf.next) && (
        <div className="wk__pages">
          <button
            type="button"
            className="btn btn--mini"
            disabled={busy || trail.length === 0}
            onClick={() => {
              const back = trail.slice(0, -1);
              setTrail(back);
              void look(query, back[back.length - 1]);
            }}
          >
            ← PREVIOUS
          </button>
          <span>PAGE {trail.length + 1}</span>
          <button
            type="button"
            className="btn btn--mini"
            disabled={busy || !shelf.next}
            onClick={() => {
              const forward = [...trail, shelf.next!];
              setTrail(forward);
              void look(query, shelf.next!);
            }}
          >
            NEXT →
          </button>
        </div>
      )}

      {shelf !== null && shelf.shown.length === 0 && !busy && !shelf.problem && (
        <p className="cc__hint">Nothing in the catalogue matches that.</p>
      )}
    </section>
  );
}
