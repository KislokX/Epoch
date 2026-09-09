/**
 * The HUD's small parts: icons, cursors, bars, portraits, toasts.
 *
 * No emoji and no icon font anywhere. Every glyph is a pixel doodle on an 8×8 grid, drawn
 * with `shapeRendering="crispEdges"` so it stays a pixel at any size — and every one is a
 * **reserved slot**: pass `src` and a real sprite draws instead, with the layout unchanged.
 *
 * That is deliberate for the same reason the frames are nine-slice shaped: a World Pack may
 * one day restyle all of this, and an icon set baked into the build would be the one thing it
 * could not replace.
 */

import type { CSSProperties, ReactNode } from "react";

/* -------------------------------------------------------------------------- */
/* Icons                                                                       */
/* -------------------------------------------------------------------------- */

export type Glyph =
  | "sword"
  | "shield"
  | "potion"
  | "gem"
  | "star"
  | "scroll"
  | "key"
  | "core";

const GLYPHS: Record<Glyph, string> = {
  sword: "M4 0h1v5H4zM3 5h3v1H3zM2 6h1v2H2zM5 6h1v1H5z",
  shield: "M3 0h2v1H3zM1 1h6v3H1zM2 4h4v2H2zM3 6h2v1H3z",
  potion: "M3 0h2v2H3zM2 2h4v5H2zM3 7h2v1H3z",
  gem: "M3 0h2v1H3zM1 1h6v1H1zM2 2h4v2H2zM3 4h2v3H3z",
  star: "M3 0h2v3H3zM0 3h8v2H0zM2 5h4v1H2zM1 6h2v2H1zM5 6h2v2H5z",
  scroll: "M1 0h6v1H1zM1 1h1v6H1zM6 1h1v6H6zM1 7h6v1H1z",
  key: "M2 0h3v3H2zM3 3h1v5H3zM4 5h2v1H4zM4 7h2v1H4z",
  core: "M3 0h2v2H3zM1 2h6v4H1zM3 6h2v2H3zM3 3h2v2H3z",
};

export type Tone = "gold" | "parchment" | "hp" | "energy" | "dim";

const TONES: Record<Tone, string> = {
  gold: "var(--ep-gold)",
  parchment: "var(--ep-parchment)",
  hp: "var(--ep-hp)",
  energy: "var(--ep-energy)",
  dim: "var(--ep-frame-lo)",
};

export function PixelIcon({
  glyph = "core",
  size = 20,
  src,
  alt,
  tone = "gold",
  className,
}: {
  readonly glyph?: Glyph;
  readonly size?: number;
  /** A real sprite, when one exists. Drawn pixelated, layout unchanged. */
  readonly src?: string;
  readonly alt?: string;
  readonly tone?: Tone;
  readonly className?: string;
}) {
  return (
    <span
      className={`epicon${className ? ` ${className}` : ""}`}
      style={{ width: size, height: size }}
      role={alt ? "img" : undefined}
      aria-label={alt}
      aria-hidden={alt ? undefined : true}
    >
      {src ? (
        <img src={src} alt={alt ?? ""} width={size - 6} height={size - 6} draggable={false} />
      ) : (
        <svg
          viewBox="0 0 8 8"
          width={size - 8}
          height={size - 8}
          fill={TONES[tone]}
          shapeRendering="crispEdges"
        >
          <path d={GLYPHS[glyph]} />
        </svg>
      )}
    </span>
  );
}

/* -------------------------------------------------------------------------- */
/* Selection cursor                                                            */
/* -------------------------------------------------------------------------- */

/** The bobbing pointer beside the current entry. */
export function Cursor({ shape = "arrow" }: { readonly shape?: "arrow" | "diamond" | "spark" }) {
  return (
    <span className="epcursor" aria-hidden>
      <svg viewBox="0 0 12 12" width="12" height="12" fill="currentColor" shapeRendering="crispEdges">
        {shape === "arrow" && (
          <>
            <path d="M2 1h2v10H2zM4 2h2v8H4zM6 3h2v6H6zM8 4h2v4H8z" />
            <path d="M1 0h1v11H1z" fill="var(--ep-outline)" />
          </>
        )}
        {shape === "diamond" && (
          <path d="M5 1h2v1H5zM4 2h4v1H4zM3 3h6v1H3zM2 4h8v3H2zM3 7h6v1H3zM4 8h4v1H4zM5 9h2v1H5z" />
        )}
        {shape === "spark" && (
          <path d="M5 0h2v4H5zM5 8h2v4H5zM0 5h4v2H0zM8 5h4v2H8zM3 3h2v2H3zM7 3h2v2H7zM3 7h2v2H3zM7 7h2v2H7z" />
        )}
      </svg>
    </span>
  );
}

/* -------------------------------------------------------------------------- */
/* Status bar                                                                  */
/* -------------------------------------------------------------------------- */

export type BarKind = "hp" | "energy" | "xp" | "loading" | "progress";

const BARS: Record<BarKind, { colour: string; label: string }> = {
  hp: { colour: "var(--ep-hp)", label: "HP" },
  energy: { colour: "var(--ep-energy)", label: "EN" },
  xp: { colour: "var(--ep-xp)", label: "XP" },
  loading: { colour: "var(--ep-gold)", label: "LOAD" },
  progress: { colour: "var(--ep-frame-hi)", label: "PROG" },
};

export function StatusBar({
  kind = "hp",
  value,
  max = 100,
  showLabel = true,
  striped = false,
  height = 14,
  label,
}: {
  readonly kind?: BarKind;
  readonly value: number;
  readonly max?: number;
  readonly showLabel?: boolean;
  readonly striped?: boolean;
  readonly height?: number;
  /** Overrides the default two-letter label. */
  readonly label?: string;
}) {
  const bar = BARS[kind];
  const pct = max > 0 ? Math.max(0, Math.min(100, (value / max) * 100)) : 0;

  return (
    <div className="epbar-row">
      {showLabel && <span className="epbar-row__label">{label ?? bar.label}</span>}
      <div
        className="epbar"
        style={{ "--h": `${height}px` } as CSSProperties}
        role="progressbar"
        aria-valuenow={Math.round(value)}
        aria-valuemin={0}
        aria-valuemax={max}
        aria-label={label ?? bar.label}
      >
        <div
          className={`epbar__fill${striped ? " epbar__fill--striped" : ""}`}
          style={{ width: `calc(${pct}% - var(--px) * 4)`, "--bar": bar.colour } as CSSProperties}
        />
      </div>
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Portrait                                                                    */
/* -------------------------------------------------------------------------- */

/**
 * A framed bust.
 *
 * With no artwork it shows what *kind* of face is missing rather than inventing one — the same
 * rule the Launcher's plate follows, and the World's figures (ADR-0024).
 */
export function Portrait({
  src,
  alt,
  kind = "npc",
  size = 52,
  children,
}: {
  readonly src?: string | null;
  readonly alt?: string;
  readonly kind?: "player" | "npc" | "ai";
  readonly size?: number;
  /** Drawn instead of an image — used for a character's own world sprite. */
  readonly children?: ReactNode;
}) {
  return (
    <div className="epportrait" style={{ width: size, height: size }}>
      {src ? (
        <img src={src} alt={alt ?? ""} draggable={false} />
      ) : children ? (
        children
      ) : (
        <span className="epportrait__none">{kind.toUpperCase()}</span>
      )}
    </div>
  );
}

/* -------------------------------------------------------------------------- */
/* Toast                                                                       */
/* -------------------------------------------------------------------------- */

export type ToastTone = "info" | "success" | "warning" | "reward";

const TOASTS: Record<ToastTone, { colour: string; glyph: Glyph; tone: Tone }> = {
  info: { colour: "var(--ep-energy)", glyph: "core", tone: "energy" },
  success: { colour: "var(--ep-hp)", glyph: "shield", tone: "hp" },
  warning: { colour: "var(--ep-gold)", glyph: "star", tone: "gold" },
  reward: { colour: "var(--ep-gold)", glyph: "gem", tone: "gold" },
};

export function Toast({
  title,
  message,
  tone = "info",
}: {
  readonly title: string;
  readonly message?: string;
  readonly tone?: ToastTone;
}) {
  const t = TOASTS[tone];
  return (
    <div className="frame ep-notify hud__toast" style={{ "--corner": "8px" } as CSSProperties}>
      <div className="frame__slices" aria-hidden>
        {["tl", "t", "tr", "l", "c", "r", "bl", "b", "br"].map((s) => (
          <i key={s} data-slice={s} />
        ))}
      </div>
      <span className="frame__stud" data-c="tl" aria-hidden />
      <span className="frame__stud" data-c="tr" aria-hidden />
      <span className="frame__stud" data-c="bl" aria-hidden />
      <span className="frame__stud" data-c="br" aria-hidden />
      <div className="frame__body hud__toast-body" role="status">
        <span className="hud__toast-icon" style={{ "--c": t.colour } as CSSProperties}>
          <PixelIcon glyph={t.glyph} tone={t.tone} size={22} />
        </span>
        <div className="hud__toast-text">
          <p className="hud__toast-title" style={{ color: t.colour }}>
            {title}
          </p>
          {message && <p className="hud__toast-msg">{message}</p>}
        </div>
      </div>
    </div>
  );
}
