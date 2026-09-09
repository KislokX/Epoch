/** The current Quest can be put down before the next conversation begins. */

interface SetAsideButtonProps {
  readonly hasQuest: boolean;
  readonly onSetAside: () => void;
}

/**
 * Starting a new Quest is only meaningful while one exists. The parent keeps the Engine action;
 * this button expresses that single visible choice and never guesses whether it succeeded.
 */
export function SetAsideButton({ hasQuest, onSetAside }: SetAsideButtonProps) {
  if (!hasQuest) return null;

  return (
    <button
      type="button"
      className="epbtn dlg__aside"
      onClick={onSetAside}
      title="Put this work down; the next thing you say starts a new Quest"
    >
      NEW
    </button>
  );
}
