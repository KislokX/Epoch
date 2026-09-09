/**
 * Somebody was named, and they have not answered.
 *
 * ## Offered, never done
 *
 * A character does not decide where a Quest goes — the user does (ADR-0025 §7b), and that is
 * what separates exposing collaboration from simulating it. So this is an offer with two
 * refusals in it: you may decline the whole thing, and you may read what would travel and still
 * say no.
 *
 * Bringing a colleague in also starts a second model, which is the user's memory and money to
 * spend rather than ours.
 *
 * ## Reading it is the point
 *
 * The agent's own `hand_over` tool has to show its words rather than its intention. This offers
 * the same guarantee to every brain, because a local model cannot call a tool at all and the
 * user should not get less for using one.
 *
 * The text is the Engine's, unedited: the goal, who said what, what already exists. Epoch does
 * not summarise a Quest before showing it — a summary of the record is narration, and the user
 * would be approving that instead.
 *
 * ## Why it is its own file
 *
 * It lived in the middle of a 1,300-line conversation box, which is why the flow it describes
 * had no tests. Everything it needs arrives as a prop, so it has none of the box's state and can
 * be driven directly.
 */

import { useState } from "react";

import type { CharacterView } from "../../ipc/contracts";

interface HandoverOfferProps {
  /** Ids the user named in what they just said. Empty means there is nothing to offer. */
  readonly invited: readonly string[];
  /** Everyone in this World, so an id can be shown as a person. */
  readonly crew: readonly CharacterView[];
  /**
   * What this character would be given, asked of the Engine.
   *
   * `null` means they have already seen everything here — which is said out loud rather than
   * hidden, because "there is nothing new" and "here is the work" are different answers.
   */
  readonly preview: (characterId: string) => Promise<string | null>;
  /** Hand it over. The recipient answers next, in this same conversation. */
  readonly onHandOver: (characterId: string) => void;
  /** Put the offer away without handing anything over. */
  readonly onDecline: () => void;
}

export function HandoverOffer({
  invited,
  crew,
  preview,
  onHandOver,
  onDecline,
}: HandoverOfferProps) {
  /** Who the user is being shown a handover for, and what it would carry. User intent only. */
  const [reading, setReading] = useState<{
    readonly id: string;
    readonly name: string;
    readonly what: string | null;
  } | null>(null);

  const nameOf = (id: string) => crew.find((person) => person.id === id)?.name ?? id;

  if (reading) {
    return (
      <div className="dlg__await">
        <p>
          <b>HAND THIS TO {reading.name.toUpperCase()}?</b>
          {reading.what
            ? `${reading.name} would be given:`
            : `${reading.name} has already seen everything here. They would be brought in with nothing new.`}
        </p>
        {reading.what && <pre className="dlg__diff">{reading.what}</pre>}
        <p className="dlg__await-note">They answer next, in this same conversation.</p>
        <div className="dlg__await-acts">
          <button
            type="button"
            className="epbtn epbtn--primary"
            onClick={() => {
              const to = reading.id;
              setReading(null);
              onHandOver(to);
            }}
          >
            HAND IT OVER
          </button>
          <button type="button" className="epbtn" onClick={() => setReading(null)}>
            NO
          </button>
        </div>
      </div>
    );
  }

  if (invited.length === 0) return null;

  return (
    <div className="dlg__invite">
      <span>
        {invited.map(nameOf).join(", ")} {invited.length === 1 ? "was" : "were"} mentioned.
      </span>
      {invited.map((id) => (
        <button
          key={id}
          type="button"
          className="epbtn"
          onClick={() => {
            // Read first, hand over second.
            void preview(id).then((what) => setReading({ id, name: nameOf(id), what }));
          }}
        >
          ASK {nameOf(id).toUpperCase()}
        </button>
      ))}
      <button type="button" className="epbtn" onClick={onDecline}>
        NO
      </button>
    </div>
  );
}
