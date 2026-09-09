interface EditorFailureNoticeProps {
  readonly failure: string | null;
  readonly onDismiss: () => void;
}

/** A refusal stays readable until the person deliberately dismisses it. */
export function EditorFailureNotice({ failure, onDismiss }: EditorFailureNoticeProps) {
  if (!failure) return null;

  return (
    <button
      type="button"
      className="pxfail"
      aria-label={`Dismiss failure: ${failure}`}
      title="Dismiss this message"
      onClick={onDismiss}
    >
      {failure}
    </button>
  );
}
