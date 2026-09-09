import type { ReactNode } from "react";

import { ImageDrop } from "./ImageDrop";

interface CharacterArtSlotProps {
  readonly label: string;
  readonly hint: string;
  readonly portrait: ReactNode;
  /** Whether this character owns artwork in this exact slot. */
  readonly hasArt: boolean;
  /** What the World is using when this slot has no authored image. */
  readonly fallback?: string;
  readonly busy: boolean;
  readonly onChoose: (dataUri: string) => Promise<string | null>;
  readonly onClear: () => Promise<string | null>;
}

/**
 * One independently editable image slot. Sprite and icon remain separate because clearing one
 * must not change the other or hide the World's fallback.
 */
export function CharacterArtSlot({
  label,
  hint,
  portrait,
  hasArt,
  fallback,
  busy,
  onChoose,
  onClear,
}: CharacterArtSlotProps) {
  return (
    <div className="art">
      <span className="art__label">{label}</span>
      <div className="art__preview">{portrait}</div>
      <p className="cc__hint">{hasArt ? hint : (fallback ?? hint)}</p>
      <div className="art__actions">
        <ImageDrop
          label={`Change ${label.toLowerCase()}`}
          busy={busy}
          className="btn btn--mini"
          onChoose={onChoose}
        />
        {hasArt && (
          <button type="button" className="btn btn--mini" disabled={busy} onClick={() => void onClear()}>
            Clear
          </button>
        )}
      </div>
    </div>
  );
}
