/**
 * Offering an image to the Engine.
 *
 * ## Why there is no file dialog
 *
 * The webview already has one. `<input type="file">` gets the bytes, `FileReader` encodes
 * them, and the Engine writes the file — so the frontend never needs filesystem permission in
 * order to hand over a picture, and no capability in `world.json` grants it any.
 *
 * The Engine also chooses the destination name, from the image's real format rather than from
 * what the file happened to be called. Nothing here is trusted; this component's whole job is
 * to turn a click into bytes.
 *
 * ## Drag and drop, because this is artwork
 *
 * Picking key art from a folder full of candidates is a dragging job. The same handler serves
 * both paths, so there is one way in and one place for it to go wrong.
 */

import { useId, useRef, useState } from "react";

/** Mirrors `epoch_engine::import::MAX_BYTES`. Checked here only to fail fast and kindly. */
const MAX_BYTES = 8 * 1024 * 1024;

interface ImageDropProps {
  /** What this picture is for. Shown on the button and used as the accessible label. */
  readonly label: string;
  /** Receives the raw `data:` URI. Resolve to an error message, or null on success. */
  readonly onChoose: (dataUri: string) => Promise<string | null>;
  /** Offered only when something is already set. */
  readonly onClear?: () => Promise<string | null>;
  readonly busy?: boolean;
  /**
   * Why this cannot be used yet, when something is missing rather than merely in progress.
   *
   * Shown beside the button. A disabled control that cannot explain itself is worse than a
   * missing one: somebody clicks it repeatedly and concludes the feature does not work.
   */
  readonly why?: string | null;
  /** Extra classes for the button, so callers can size it into their own layout. */
  readonly className?: string;
  /**
   * What the picker offers, when this door is not for pictures.
   *
   * **A courtesy, never a control.** The Engine sniffs what actually arrived and refuses anything
   * else (ADR-0024); this only stops the file dialog showing somebody a file it will then reject.
   * The same shape as a capability's own permission check: a surface's check is a courtesy.
   *
   * The name of this component is now one medium too narrow — it is the import door and a World's
   * sounds come through it too. Renaming it touches twelve call sites and is worth doing, but not
   * in the same change that gives Worlds a voice.
   */
  readonly accept?: string;
}

export function ImageDrop({
  label,
  onChoose,
  onClear,
  busy,
  why,
  className,
  accept = "image/png,image/jpeg,image/webp,image/svg+xml",
}: ImageDropProps) {
  const inputId = useId();
  const input = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);
  const [over, setOver] = useState(false);
  const [working, setWorking] = useState(false);

  const take = async (file: File | undefined) => {
    if (!file) return;
    setError(null);

    // Refused here as well as in the Engine — not as a substitute for it, but so an 80 MB
    // file is not read into memory and base64-encoded before being told no.
    if (file.size > MAX_BYTES) {
      setError(`That image is ${(file.size / 1024 / 1024).toFixed(1)} MB; the limit is 8 MB.`);
      return;
    }

    setWorking(true);
    const dataUri = await new Promise<string | null>((resolve) => {
      const reader = new FileReader();
      reader.onload = () => resolve(typeof reader.result === "string" ? reader.result : null);
      reader.onerror = () => resolve(null);
      reader.readAsDataURL(file);
    });

    if (dataUri === null) {
      setWorking(false);
      setError("That file could not be read.");
      return;
    }

    const failure = await onChoose(dataUri);
    setWorking(false);
    setError(failure);
    if (input.current) input.current.value = "";
  };

  const disabled = busy || working;

  return (
    <div className="drop">
      <div className="drop__row">
        <label
          htmlFor={inputId}
          /*
            **A label over a disabled input looks exactly like a label over an enabled one.**
            The browser silently refuses to open the picker and nothing on screen changes, so
            the control reads as broken rather than as unavailable. `aria-disabled` says it to
            assistive technology and `btn--off` says it to everyone else.
          */
          aria-disabled={disabled}
          title={disabled ? (why ?? undefined) : undefined}
          className={`btn btn--mini${disabled ? " btn--off" : ""}${over ? " btn--over" : ""}${className ? ` ${className}` : ""}`}
          onDragOver={(e) => {
            e.preventDefault();
            setOver(true);
          }}
          onDragLeave={() => setOver(false)}
          onDrop={(e) => {
            e.preventDefault();
            setOver(false);
            void take(e.dataTransfer.files[0]);
          }}
        >
          {working ? "READING…" : label}
        </label>
        <input
          id={inputId}
          ref={input}
          type="file"
          accept={accept}
          className="drop__input"
          disabled={disabled}
          onChange={(e) => void take(e.target.files?.[0])}
        />

        {onClear && (
          <button
            type="button"
            className="btn btn--mini"
            disabled={disabled}
            onClick={async () => {
              setWorking(true);
              setError(await onClear());
              setWorking(false);
            }}
          >
            CLEAR
          </button>
        )}
      </div>

      {error && <p className="notice notice--warn">{error}</p>}
      {/*
        Quieter than an error, because nothing has gone wrong: something has not been said yet.
        Hidden once the control works, so it never becomes a permanent caption.
      */}
      {!error && disabled && why && <p className="drop__why">{why}</p>}
    </div>
  );
}
