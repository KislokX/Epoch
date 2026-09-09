/**
 * Making somebody who did not exist.
 *
 * ## Four fields, and only one of them is required
 *
 * A name. Everything else has a default that is true, or is honestly absent — because the crew
 * starts at **zero** and the first person somebody makes has to be *theirs* (ADR-0023). A form
 * that arrived with a personality, a face and a model already chosen would be handing them a
 * stranger and calling it their character.
 *
 * ## What this form deliberately does not ask
 *
 * No model, no provider, no capabilities, no home, no portrait. Every one of those is either
 * configuration (which belongs to the panels that own it) or a decision that would surprise
 * somebody with a cost they did not choose — on this machine two local models differed by
 * minutes on a cold start.
 *
 * They can be given all of it one click later, in the editor that already exists. The difference
 * is that by then the character exists and the choices are being made *about somebody*.
 *
 * ## The suggested prompt
 *
 * Offered, never applied. Choosing an archetype fills the field with words the Engine suggests,
 * and the user can keep, edit or delete them. The difference between help and imposition is who
 * performed the act — so it appears in an editable field rather than being written behind them.
 */

import { useState } from "react";

import { hireCharacter, suggestedPrompt } from "../ipc/launcher";

interface HireProps {
  /** Canonical archetype ids, from the Engine. The surface never holds this list. */
  readonly archetypes: readonly string[];
  /** Someone now exists. Their id, so the caller can open the editor on them. */
  readonly onHired: (id: string) => void;
  readonly onCancel: () => void;
}

/** `researcher` reads as *Researcher*. Presentation only; the id is what travels. */
function titleise(id: string): string {
  return id.replace(/[-_]/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());
}

export function Hire({ archetypes, onHired, onCancel }: HireProps) {
  const [name, setName] = useState("");
  const [archetype, setArchetype] = useState(archetypes[0] ?? "researcher");
  const [role, setRole] = useState("");
  const [prompt, setPrompt] = useState("");
  /** Whether the user has written their own words, so a suggestion never overwrites them. */
  const [written, setWritten] = useState(false);
  const [failed, setFailed] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const offer = (of: string) => {
    void suggestedPrompt(of).then((words) => {
      // Only into an empty field the user has not touched. Replacing what somebody wrote because
      // they changed a dropdown would be the form deciding it knew better.
      if (!written) setPrompt(words);
    });
  };

  const submit = async () => {
    setBusy(true);
    setFailed(null);
    const made = await hireCharacter(name, archetype, role, prompt);
    setBusy(false);
    if (typeof made === "string") onHired(made);
    else setFailed(made.error);
  };

  return (
    <form
      className="hire"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div className="cedit">
        <label className="cedit__field">
          <span>Name</span>
          <input
            value={name}
            autoFocus
            placeholder="Who are they?"
            onChange={(e) => setName(e.target.value)}
          />
        </label>

        <label className="cedit__field">
          <span>Archetype</span>
          {/*
            Classification, not identity (ADR-0017). Several characters may share one and remain
            several people — it says what kind of work they do, never who they are.
          */}
          <select
            value={archetype}
            onChange={(e) => {
              setArchetype(e.target.value);
              offer(e.target.value);
            }}
          >
            {archetypes.map((id) => (
              <option key={id} value={id}>
                {titleise(id)}
              </option>
            ))}
          </select>
        </label>

        <label className="cedit__field cedit__field--wide">
          <span>Description</span>
          <input
            value={role}
            placeholder="One line on what they are for — optional"
            onChange={(e) => setRole(e.target.value)}
          />
        </label>

        <label className="cedit__field cedit__field--wide">
          <span>Character prompt</span>
          <textarea
            rows={4}
            value={prompt}
            placeholder="How do they work? Leave it empty and fill it in later."
            onChange={(e) => {
              setPrompt(e.target.value);
              setWritten(true);
            }}
          />
          <i className="cedit__hint">
            {archetypes.length > 0 && (
              <button
                type="button"
                className="hire__offer"
                onClick={() => {
                  setWritten(false);
                  void suggestedPrompt(archetype).then(setPrompt);
                }}
              >
                Use the {titleise(archetype)}&rsquo;s suggested prompt
              </button>
            )}{" "}
            Words to start from. Keep them, change them, or delete them — this is where the
            personality actually lives.
          </i>
        </label>
      </div>

      {failed && <p className="notice notice--warn">{failed}</p>}

      <p className="cedit__hint">
        No model, no face and no home yet — all three are chosen afterwards, about somebody who
        exists.
      </p>

      <div className="conn__acts">
        <button type="submit" className="btn" disabled={busy || name.trim().length === 0}>
          {busy ? "MAKING…" : "MAKE THEM"}
        </button>
        <button type="button" className="btn btn--ghost" onClick={onCancel} disabled={busy}>
          CANCEL
        </button>
      </div>
    </form>
  );
}
