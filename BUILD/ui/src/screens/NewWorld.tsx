/**
 * Making a World.
 *
 * ## What it asks, and why the list is short
 *
 * A name, a folder to work in, and who is coming. Nothing else.
 *
 * Not a provider URL, `num_ctx`, temperature, a context size, capabilities, MCP servers, API
 * keys or sprite packs. Every one of those is **configuration** — it belongs to a machine, a
 * character or an installation, and none of them belong to the act of making a place. Asking
 * for them here would turn "make a World" into commissioning one, and the answer to most of
 * them is *the default*, which is exactly what the user should have to touch nothing to get
 * (ADR-0026).
 *
 * The test each field had to pass: **would somebody be unable to start without it?**
 *
 * - **Name** — a World has to be called something.
 * - **Project root** — the Quest cycle is Intent → Quest → Execution → Evidence → History, and
 *   all of it runs against real files. ADR-0025 is explicit that this should arrive *early*,
 *   because planning against the user's real codebase is worth immeasurably more than planning
 *   against a hypothesis. Optional, because a World with no project is still a World.
 * - **Crew** — a World nobody lives in opens, and says it is empty. Also optional, therefore.
 *   But choosing here saves the trip back out to the roster, and it is the only moment when
 *   "who is coming with me" is the question already on the user's mind.
 *
 * ## What the scan is for
 *
 * Facts, never a verdict. Epoch does not decide whether a folder is a good project — it says
 * what is there and lets the person who chose it recognise it. A repository *found* is a real
 * measurement; a repository *reported* without one is the kind of confident wrongness that
 * teaches people to stop reading the instruments.
 *
 * ## The disclosure
 *
 * ADR-0025 names this as its own decision: reading a project root with a **hosted** provider
 * sends the user's source code to a third party. That is theirs to make explicitly, and it must
 * not be buried in "the World has a project root". So it is said here, where the folder is
 * chosen, in a sentence rather than a warning triangle.
 */

import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import type { CharacterSummary } from "../ipc/contracts";
import { chooseFolder } from "../ipc/launcher";
import { Overlay } from "../components/Overlay";

/** What a folder turned out to contain. Every field measured. */
interface ProjectScan {
  readonly git: boolean;
  readonly entries: number;
  readonly folders: number;
}

interface NewWorldProps {
  /** Everybody the user has, so a starting crew can be chosen from real people. */
  readonly crew: readonly CharacterSummary[];
  readonly onClose: () => void;
  /** Made. The Launcher takes it from here, into the World Editor. */
  readonly onMade: (world: { id: string; name: string }) => void;
}

export function NewWorld({ crew, onClose, onMade }: NewWorldProps) {
  const [name, setName] = useState("");
  const [root, setRoot] = useState("");
  const [coming, setComing] = useState<readonly string[]>([]);
  const [scan, setScan] = useState<ProjectScan | null>(null);
  const [scanning, setScanning] = useState(false);
  const [scanFailed, setScanFailed] = useState<string | null>(null);
  const [browsing, setBrowsing] = useState(false);
  const [making, setMaking] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);

  /*
    Scanning follows the typing, a beat behind. Without the delay every keystroke of a pasted
    path would hit the filesystem, and the answer for `C:\\Us` is not interesting.
  */
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (timer.current) clearTimeout(timer.current);
    const typed = root.trim();
    if (!typed) {
      setScan(null);
      setScanFailed(null);
      setScanning(false);
      return;
    }
    setScanning(true);
    timer.current = setTimeout(() => {
      void invoke<ProjectScan>("scan_project", { path: typed })
        .then((found) => {
          setScan(found);
          setScanFailed(null);
        })
        .catch((error) => {
          setScan(null);
          setScanFailed(String(error).replace(/^Error: /, ""));
        })
        .finally(() => setScanning(false));
    }, 400);
    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [root]);

  const browse = async () => {
    setBrowsing(true);
    // Opened by the Engine, so the webview never gains a filesystem API. What comes back is a
    // path, validated exactly like a typed one — there is no trusted route in.
    const chosen = await chooseFolder(root.trim() || null);
    setBrowsing(false);
    if (chosen) setRoot(chosen);
  };

  const make = async () => {
    const world = name.trim();
    if (!world || making) return;
    setMaking(true);
    setFailed(null);
    try {
      const id = await invoke<string>("create_world", {
        name: world,
        // Only when it actually resolved. Handing over a path the Engine already refused would
        // make the World fail to create for a reason the user was told about a second ago.
        projectRoot: scan ? root.trim() : null,
        crew: coming,
      });
      onMade({ id, name: world });
    } catch (error) {
      setFailed(String(error).replace(/^Error: /, ""));
      setMaking(false);
    }
  };

  return (
    // `dismissOnScrim={false}`: this is a form somebody is part-way through, and a stray click
    // outside it should not throw away a half-typed World. Escape still closes — a window you
    // cannot leave at all is a trap.
    <Overlay
      label="Make a new World"
      onClose={onClose}
      className="nw"
      dismissOnScrim={false}
    >
      <div className="nw__panel" onKeyDown={(event) => event.stopPropagation()}>
        <header className="nw__head">
          <h2>NEW WORLD</h2>
          <button type="button" className="nw__x" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </header>

        <div className="nw__body">
          <label className="nw__field">
            <span>World name</span>
            <input
              autoFocus
              value={name}
              placeholder="The Archipelago"
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void make();
                if (event.key === "Escape") onClose();
              }}
            />
          </label>

          <div className="nw__field">
            <span>Project root — optional</span>
            {/*
              A real folder picker: the same native one the Launcher's Project Root already
              uses. It is opened by the **Engine** (`rfd`) rather than by a webview plugin, so
              the frontend gains no filesystem surface at all — it receives a path string and
              hands it straight back to be validated exactly like a typed one.

              The field stays typable beside it, because pasting a path somebody sent you is a
              real way to answer this and a dialog cannot be pasted into.
            */}
            <div className="nw__row">
              <input
                value={root}
                placeholder="Choose or paste a folder"
                spellCheck={false}
                onChange={(event) => setRoot(event.target.value)}
              />
              <button
                type="button"
                className="btn"
                disabled={browsing}
                onClick={() => void browse()}
              >
                {browsing ? "…" : "BROWSE"}
              </button>
              {root.trim() && (
                <button type="button" className="btn" onClick={() => setRoot("")}>
                  CLEAR
                </button>
              )}
            </div>
          </div>

          {/*
            The scan. Measured or visibly nothing — never a plausible-looking reading, and never
            a judgement about whether this is a good folder to work in.
          */}
          <div className="nw__scan">
            {!root.trim() ? (
              <p className="nw__quiet">
                A World with no project is still a World. You can point it at one later.
              </p>
            ) : scanning ? (
              <p className="nw__quiet">Scanning project…</p>
            ) : scanFailed ? (
              <p className="nw__bad">✕ {scanFailed}</p>
            ) : scan ? (
              <>
                <p className={scan.git ? "nw__good" : "nw__quiet"}>
                  {scan.git ? "✓ Git repository found" : "— No Git repository here"}
                </p>
                <p className="nw__quiet">
                  {scan.entries === 0
                    ? "The folder is empty. That is a fine place to start."
                    : `${scan.entries} ${scan.entries === 1 ? "item" : "items"} at the top level, ${scan.folders} ${scan.folders === 1 ? "folder" : "folders"}.`}
                </p>
                {/*
                  ADR-0025 names this as the user's decision to make explicitly. Said where the
                  folder is chosen, in a sentence — not buried, and not dressed as a warning
                  about something they have not done yet.
                */}
                <p className="nw__note">
                  Work done here can read these files. With a hosted provider, that means sending
                  them to a third party — a local one keeps them on this machine.
                </p>
              </>
            ) : null}
          </div>

          <div className="nw__field">
            <span>Starting crew</span>
            {crew.length === 0 ? (
              <p className="nw__quiet">
                You have not made anybody yet. A World nobody lives in opens fine, and says it is
                empty — you can move people in whenever.
              </p>
            ) : (
              <div className="nw__crew">
                {crew.map((person) => {
                  const on = coming.includes(person.id);
                  return (
                    <button
                      key={person.id}
                      type="button"
                      className={on ? "nw__soul nw__soul--on" : "nw__soul"}
                      aria-pressed={on}
                      onClick={() =>
                        setComing((who) =>
                          on ? who.filter((id) => id !== person.id) : [...who, person.id],
                        )
                      }
                    >
                      {/* Their real face, or their initial. Never a generated likeness. */}
                      <span className="nw__face" aria-hidden>
                        {person.portrait?.asset ? (
                          <img src={person.portrait.asset} alt="" draggable={false} />
                        ) : (
                          person.name.slice(0, 1).toUpperCase()
                        )}
                      </span>
                      {person.name}
                    </button>
                  );
                })}
              </div>
            )}
          </div>

          <div className="nw__field">
            <span>World template — optional</span>
            <button
              type="button"
              className="btn"
              disabled
              title="Importing a World pack is not built yet. New Worlds start empty."
            >
              Import a template
            </button>
            <p className="nw__quiet">
              Not built yet. A new World starts empty, and the World Editor is where it becomes
              somewhere.
            </p>
          </div>

          {failed && <p className="nw__bad">{failed}</p>}
        </div>

        <footer className="nw__foot">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn--go"
            disabled={!name.trim() || making}
            onClick={() => void make()}
          >
            {making ? "MAKING…" : "MAKE IT"}
          </button>
        </footer>
      </div>
    </Overlay>
  );
}
