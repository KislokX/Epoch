/**
 * The HUD's frame primitives.
 *
 * A `Frame` is a nine-slice window: the DOM is a real 3×3 grid of slice elements, each
 * carrying a `data-slice` name. Today a hand-made pixel bevel is drawn behind them, and the
 * grid is empty. When authored frame art arrives, assigning `--tex-tl` … `--tex-br` swaps the
 * bevel for sprites and **not one line of markup changes**.
 *
 * That is the same bargain the World's `shape` and `sprite` marks strike: an honest drawing
 * now, an authored one later, one pipeline (ADR-0019, ADR-0020).
 */

import type { CSSProperties, PointerEvent, ReactNode } from "react";

import { useSkin } from "../../experience/useSkin";
import type { Skin } from "../../experience/useSkin";

/** Where authored art goes when it exists. Nine names, one per slice. */
export type SliceTextures = Partial<
  Record<"tl" | "t" | "tr" | "l" | "c" | "r" | "bl" | "b" | "br", string>
>;

interface FrameProps {
  readonly children?: ReactNode;
  readonly className?: string;
  /** Centre fill colour. Defaults to the window tone. */
  readonly fill?: string;
  /** Corner size in px — matches the eventual sprite's corner. */
  readonly corner?: number;
  readonly textures?: SliceTextures;
  /**
   * The Asset Concept this window is, e.g. `ui.frame.window`.
   *
   * The Engine's vocabulary, never a file (ADR-0016). A World that supplies this concept draws
   * every window that asks for it; a World that supplies nothing gets Epoch's own, which is what
   * every World has today.
   *
   * Defaults to `ui.frame.window` because that is what a `Frame` *is*. One authored image
   * therefore re-skins the whole interface, which is the promise being tested — and a window
   * that wants to be something else says so.
   */
  readonly concept?: string;
  /**
   * Draw *this* skin rather than the World's, for a preview of one that is not saved yet.
   *
   * Only the editor passes it. `undefined` means the ordinary thing: whatever the pack supplies
   * for this concept, or Epoch's own drawn window when it supplies nothing.
   */
  readonly skin?: Skin;
  /** Gold corner studs. */
  readonly studs?: boolean;
  readonly style?: CSSProperties;
  readonly role?: string;
  readonly "aria-label"?: string;
  /**
   * For a window that can be brought to the front by clicking anywhere in it.
   *
   * Added when terminals became Frames rather than boxes of their own. A window that draws its
   * own chrome is the one window a World Pack cannot re-skin, which is exactly the leak this
   * component exists to close.
   */
  readonly onPointerDown?: (event: PointerEvent<HTMLDivElement>) => void;
  /** A window that is minimised is still mounted, and should not be read out. */
  readonly "aria-hidden"?: boolean;
}

export function Frame({
  children,
  className,
  fill,
  corner = 12,
  textures,
  concept = "ui.frame.window",
  studs = false,
  style,
  skin: given,
  ...rest
}: FrameProps) {
  /*
    **A candidate skin wins over the installed one, and that is what the editor previews with.**

    The alternative was a second renderer drawing the nine-slice for the panel. It would have
    been fifteen lines and it would eventually have disagreed with this one -- and the entire
    value of that preview is that it can be trusted while somebody picks four numbers they have
    no other way to choose. *When two things must agree, derive both from the same measurement.*

    `undefined` is the ordinary case: every window in the World reads what the pack supplied.
  */
  const skin = given ?? useSkin()[concept];

  const vars: Record<string, string> = { "--corner": `${corner}px` };
  if (fill) vars["--frame-fill"] = fill;
  if (textures) {
    for (const [slice, url] of Object.entries(textures)) {
      if (url) vars[`--tex-${slice}`] = `url(${url})`;
    }
  }
  if (skin) {
    // **Nine-slice, natively.** One image and its corner insets is how somebody draws a window;
    // the browser cuts it into corners that never stretch, edges that fill and a centre that
    // fills, from exactly those facts. Nine separate files would be the renderer's way of
    // thinking imposed on the person holding the pencil.
    const [top, right, bottom, left] = skin.corner;
    const [wide, tall] = skin.minSize;
    vars["--skin"] = `url(${skin.image})`;
    // Unitless, because a border-image slice is measured in the *image's* pixels.
    vars["--skin-slice"] = `${top} ${right} ${bottom} ${left}`;
    // And these are screen pixels: the same insets, at the whole-number scale the author chose.
    vars["--skin-border"] = [top, right, bottom, left]
      .map((side) => `${side * skin.scale}px`)
      .join(" ");
    vars["--skin-repeat"] = skin.repeat;
    // A window smaller than this cannot show its own frame, so it does not get smaller. Refusing
    // the size beats drawing corners that overlap.
    vars["--skin-min-w"] = `${wide}px`;
    vars["--skin-min-h"] = `${tall}px`;
  }

  return (
    <div
      className={`frame${skin ? " frame--skinned" : ""}${className ? ` ${className}` : ""}`}
      style={{ ...vars, ...style } as CSSProperties}
      {...rest}
    >
      <div className="frame__slices" aria-hidden>
        <i data-slice="tl" />
        <i data-slice="t" />
        <i data-slice="tr" />
        <i data-slice="l" />
        <i data-slice="c" />
        <i data-slice="r" />
        <i data-slice="bl" />
        <i data-slice="b" />
        <i data-slice="br" />
      </div>

      {studs && (
        <>
          <span className="frame__stud" data-c="tl" aria-hidden />
          <span className="frame__stud" data-c="tr" aria-hidden />
          <span className="frame__stud" data-c="bl" aria-hidden />
          <span className="frame__stud" data-c="br" aria-hidden />
        </>
      )}

      <div className="frame__body">{children}</div>
    </div>
  );
}

/** The raised gold banner on a window's shoulder. */
export function Plate({
  children,
  className,
}: {
  readonly children: ReactNode;
  readonly className?: string;
}) {
  return (
    <div className={`plate${className ? ` ${className}` : ""}`}>
      <span>{children}</span>
    </div>
  );
}
