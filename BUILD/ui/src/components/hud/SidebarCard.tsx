/** Reusable measured readings in the World sidebars. */

import { Frame, Plate } from "./Frame";
import { Cursor, PixelIcon } from "./Pixel";
import type { Glyph } from "./Pixel";

interface SidebarCardProps {
  readonly title: string;
  readonly children: React.ReactNode;
  /** Nothing behind it yet: the frame stays, but it does not pretend to be live. */
  readonly dormant?: boolean;
  /** Takes the sidebar's spare height only when its content benefits from it. */
  readonly grows?: boolean;
  /**
   * The list has no natural length — crew, models, open work — so it keeps its own bounded
   * scroll. Without it one long list pushes every card below it off the rail.
   */
  readonly scrolls?: boolean;
}

/** A titled mini-panel. The World retains the data; this only keeps the shared frame honest. */
export function SidebarCard({ title, children, dormant, grows, scrolls }: SidebarCardProps) {
  return (
    <Frame
      corner={10}
      fill="var(--ep-window-2)"
      className={`${dormant ? "hud__card--dormant" : ""}${grows ? " hud__card--grows" : ""}${
        // A card whose list scrolls takes the rail's spare height rather than a fixed slice, so
        // no frame in the column is ever cut off. The height has to be handed down through the
        // Frame's own boxes, which is why the class lands here and not only on the list.
        scrolls ? " hud__card--fills" : ""
      }`}
    >
      <div className="hud__card">
        <Plate>{title}</Plate>
        <div className={`hud__card-list${scrolls ? " hud__card-list--scrolls ep-scroll" : ""}`}>
          {children}
        </div>
      </div>
    </Frame>
  );
}

interface SidebarRowProps {
  readonly glyph: Glyph;
  readonly label: string;
  readonly right?: string;
  readonly active?: boolean;
  readonly dim?: boolean;
  readonly onClick?: () => void;
}

/** A reading is a button only when the World supplied a real navigation action for it. */
export function SidebarRow({ glyph, label, right, active, dim, onClick }: SidebarRowProps) {
  const Tag = onClick ? "button" : "div";
  return (
    <Tag
      {...(onClick ? { type: "button" as const, onClick } : {})}
      className={`hud__row inset${active ? " hud__row--on" : ""}${dim ? " hud__row--dim" : ""}`}
    >
      {active && (
        <span className="hud__row-cursor">
          <Cursor shape="arrow" />
        </span>
      )}
      <PixelIcon glyph={glyph} size={16} tone={active ? "gold" : dim ? "dim" : "parchment"} />
      <span className="hud__row-label">{label}</span>
      {right && <span className="hud__row-right">{right}</span>}
    </Tag>
  );
}
