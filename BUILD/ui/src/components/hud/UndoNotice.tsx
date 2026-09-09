/**
 * The latest reversible change in the World, not in this conversation.
 *
 * The Journal is global to a World: another character may have made the last change. The notice
 * must say that plainly rather than attach evidence to whichever dialogue happens to be open.
 * Dismissing only hides this exact summary; it neither undoes nor forgets the Journal entry.
 */

import type { Undoable } from "../../ipc/world";

interface UndoNoticeProps {
  /** The Engine's latest undoable change, or no reversible World change. */
  readonly change: Undoable | null;
  /**
   * The id of the change the user has already put away.
   *
   * The **id**, not the sentence. Dismissing "created notes.md" and then creating `notes.md`
   * again produced the same sentence, so the second change arrived already dismissed — a real
   * undo nobody could see or reach. And the id names its World, so putting a notice away in one
   * World cannot hide an identically worded change in another.
   */
  readonly hidden: string | null;
  /** Ask the parent to invoke the Engine's undo and read the resulting truth back. */
  readonly onUndo: () => void | Promise<void>;
  /** Remember that this exact change was read; it is not an undo. */
  readonly onDismiss: (id: string) => void;
}

export function UndoNotice({ change, hidden, onUndo, onDismiss }: UndoNoticeProps) {
  if (!change || change.id === hidden) return null;

  return (
    <div className="dlg__undo">
      <span>
        <i>Last change in this World</i> {change.summary}
      </span>
      <button type="button" className="epbtn" onClick={() => void onUndo()}>
        UNDO
      </button>
      <button
        type="button"
        className="dlg__dismiss"
        onClick={() => onDismiss(change.id)}
        title="Put this away. The change stays in the Journal."
        aria-label="Dismiss"
      >
        ✕
      </button>
    </div>
  );
}
