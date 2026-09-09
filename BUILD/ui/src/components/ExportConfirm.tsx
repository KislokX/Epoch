import type { ExportView } from "../ipc/contracts";

/**
 * What sending a World out will carry, said before it is carried.
 *
 * ## The same shape as a removal, and deliberately so
 *
 * `RemoveConfirm` exists because a confirmation that only says "are you sure?" is a
 * confirmation about nothing. Export needs the same thing for the opposite risk: a removal may
 * take more than you meant, and an export may **send** more than you meant — somebody's
 * conversations, somebody's face — to a person you cannot un-send it to.
 *
 * So it borrows that panel's grammar rather than inventing one. Same `rm__*` frame, same
 * headings, same action row: two panels that do opposite things to the same object should not
 * look like they came from different products, and a second set of styles is a second place to
 * fix a spacing bug.
 *
 * ## What is different, and why
 *
 * **Stays-behind comes first.** A deletion reads worst-first because what goes is the risk.
 * Here what *goes* is the risk, so the question a person actually has — *what am I not handing
 * over* — is answered before the file list.
 *
 * **No name to type.** The gate on a removal exists because it cannot be undone. An export
 * writes a new folder and touches nothing else; making somebody type a name to copy files would
 * be ceremony borrowed without its reason.
 *
 * **A refusal has no button.** When a World declares no licence this states why and offers
 * nothing to press — an export is a distribution, CONTENT_PHILOSOPHY's hard rule applies at
 * exactly this moment, and a warning beside a live button is not a refusal.
 */
export function ExportConfirm({
  plan,
  busy,
  onCancel,
  onConfirm,
}: {
  readonly plan: ExportView | null;
  readonly busy: boolean;
  readonly onCancel: () => void;
  readonly onConfirm: () => void;
}) {
  // The panel opens on the user's intent and fills in when the folder has been read. A blank
  // frame in the meantime would read as *nothing would travel*.
  if (!plan) {
    return (
      <div className="rm">
        <p className="cc__hint">Working out what would travel…</p>
      </div>
    );
  }

  const blocked = plan.problems.length > 0;

  return (
    <div className="rm">
      <span className="rm__label">
        {blocked ? `Cannot send ${plan.what}` : `Send ${plan.what}`}
      </span>

      {blocked ? (
        <div className="rm__list rm__list--goes">
          <span className="rm__heading">Refused</span>
          {plan.problems.map((why) => (
            <span key={why}>{why}</span>
          ))}
        </div>
      ) : (
        <>
          {/* What is *not* handed over. First, because that is the question. */}
          <div className="rm__list rm__list--survives">
            <span className="rm__heading">Stays here</span>
            {plan.leaves.map((line) => (
              <span key={line}>{line}</span>
            ))}
          </div>

          <div className="rm__list rm__list--changes">
            <span className="rm__heading">
              Travels — {plan.carries.length}{" "}
              {plan.carries.length === 1 ? "file" : "files"}, {size(plan.bytes)}
            </span>
            {plan.carries.map((file) => (
              <span key={file} className="rm__file">
                {file}
                {plan.unvouched.includes(file) && (
                  <i className="cc__hint"> — licence not stated</i>
                )}
              </span>
            ))}
          </div>

          {/*
            **Under what travels, because that is what it is about.**
            
            The pack's `[license]` is a statement its author made about the pack. A picture
            dropped in afterwards leaves under that sentence without anybody having said so, and
            Epoch cannot read what a picture is or who made it — an imported PNG carries no
            manifest and no catalogue knows it.
            
            Said and not refused: the hard rule governs what Epoch distributes, and what somebody
            hands a friend from their own vault is theirs. Refusing would be Epoch deciding a
            question it cannot even measure.
          */}
          {plan.unvouched.length > 0 && (
            <p className="notice notice--warn">
              {plan.unvouched.length}{" "}
              {plan.unvouched.length === 1 ? "file" : "files"} above{" "}
              {plan.unvouched.length === 1 ? "is" : "are"} artwork you imported. The
              licence you declared was not written about{" "}
              {plan.unvouched.length === 1 ? "it" : "them"}, and Epoch cannot read
              what a picture is or who made it. Send{" "}
              {plan.unvouched.length === 1 ? "it" : "them"} only if{" "}
              {plan.unvouched.length === 1 ? "it is" : "they are"} yours to send.
            </p>
          )}

          {/*
            **What it will be called, not where it goes.** Where is the user's answer, given to
            a dialog after they accept this — announcing a path here would be Epoch deciding the
            one thing about an export that is not its business.
          */}
          <p className="cc__hint">
            One archive, {plan.into}. You choose where it lands; unzip it into
            another Epoch&rsquo;s Worlds folder to install it.
          </p>
        </>
      )}

      <div className="art__actions">
        <button
          type="button"
          className="btn btn--mini"
          disabled={busy}
          onClick={onCancel}
        >
          {blocked ? "CLOSE" : "CANCEL"}
        </button>
        {!blocked && (
          <button
            type="button"
            className="btn btn--mini"
            disabled={busy}
            onClick={onConfirm}
          >
            {busy ? "SENDING…" : "CHOOSE WHERE…"}
          </button>
        )}
      </div>
    </div>
  );
}

/**
 * How big it is, in words.
 *
 * A World is kilobytes when it is shapes and gigabytes when it has artwork, and one unit for
 * both makes one of them unreadable.
 */
export function size(bytes: number): string {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  if (bytes >= 1e3) return `${Math.round(bytes / 1e3)} KB`;
  return `${bytes} bytes`;
}
