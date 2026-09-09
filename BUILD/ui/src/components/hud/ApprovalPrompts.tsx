/**
 * The two decisions a turn may put in front of its user.
 *
 * A capability grant is not an approval: granting changes a Character's Definition, then the
 * call is judged in its own right. An approval is permission for one exact call that has not
 * happened yet. Rendering them together used to make that distinction live in `Dialogue`, where
 * it could not be exercised without opening a whole World.
 *
 * This component owns neither decision nor state. The Engine owns what is waiting; the turn owns
 * resuming it. It only makes the difference visible and sends the user's exact answer back.
 */

import type { CharacterView } from "../../ipc/contracts";
import type { Awaiting, Wanted } from "../../experience/useTurn";

interface ApprovalPromptsProps {
  /** Who is asking, so the question names a person rather than a generic agent. */
  readonly who: CharacterView;
  /** A capability they do not have yet. Granting it is a separate decision from the call. */
  readonly wanted: Wanted | null;
  /** One exact call stopped before anything was done. */
  readonly awaiting: Awaiting | null;
  readonly onAnswerWanted: (grant: boolean) => void | Promise<void>;
  readonly onAnswer: (approve: boolean, always: boolean) => void | Promise<void>;
}

export function ApprovalPrompts({
  who,
  wanted,
  awaiting,
  onAnswerWanted,
  onAnswer,
}: ApprovalPromptsProps) {
  return (
    <>
      {wanted && (
        <div className="dlg__await">
          <p>
            <b>{who.name.toUpperCase()} NEEDS A SKILL</b>
            To carry on, {who.name} needs <em>{wanted.capability}</em> — {wanted.summary}
          </p>
          <p className="dlg__await-note">
            It {wanted.effects.join(", and ")}. {wanted.reversal} · risk {wanted.risk}. They do not
            have it yet.
          </p>
          <div className="dlg__await-acts">
            <button
              type="button"
              className="epbtn epbtn--primary"
              onClick={() => void onAnswerWanted(true)}
            >
              GIVE IT TO THEM
            </button>
            <button type="button" className="epbtn" onClick={() => void onAnswerWanted(false)}>
              NO
            </button>
          </div>
        </div>
      )}

      {awaiting && (
        <div className="dlg__await">
          <p>
            <b>{who.name.toUpperCase()} NEEDS YOUR WORD</b>
            {awaiting.what}
          </p>
          {awaiting.preview && <pre className="dlg__diff">{awaiting.preview}</pre>}
          <p className="dlg__await-note">
            {awaiting.reversal} · risk {awaiting.risk}. Nothing has been done.
          </p>
          <div className="dlg__await-acts">
            <button
              type="button"
              className="epbtn epbtn--primary"
              onClick={() => void onAnswer(true, false)}
            >
              ALLOW ONCE
            </button>
            <button
              type="button"
              className="epbtn"
              title="And stop asking about this in this World"
              onClick={() => void onAnswer(true, true)}
            >
              ALWAYS HERE
            </button>
            <button type="button" className="epbtn" onClick={() => void onAnswer(false, false)}>
              NO
            </button>
          </div>
        </div>
      )}
    </>
  );
}
