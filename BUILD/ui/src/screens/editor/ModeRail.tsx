/**
 * The tool rail — what a click on the World means right now.
 *
 * TERRAIN was left cold here after its panel was built, so the one control that would have
 * opened it could not be pressed. A tool is cold when the thing behind it does not exist; the
 * moment it does, the rail is where that has to be said, or the feature is invisible.
 *
 * ## Modes are the design's real idea
 *
 * The reference design's insight is not the panels; it is that the World itself is the editor.
 * You do not fill in a form describing a building — you switch to PLACE and put one down. The
 * rail is what makes one canvas serve four gestures without four sets of controls.
 *
 * ## Six are live, two are cold
 *
 * QUEST and PREVIEW are in the design and have nothing behind them here. They keep
 * their place in the rail and lose their light, each naming the subsystem that will answer —
 * because a tool that is *coming* is different information from a tool that does not exist, and
 * hiding them would make the rail rearrange itself as Epoch grows.
 */

import type { EditMode } from "./useEditor";

interface Tool {
  readonly id: EditMode;
  readonly glyph: string;
  readonly label: string;
  readonly hint: string;
  /** What will light this up. `undefined` means it works today. */
  readonly pending?: string;
}

const TOOLS: readonly Tool[] = [
  {
    id: "select",
    glyph: "➤",
    label: "Select",
    hint: "click to select · drag a building to move it · drag the land to pan",
  },
  { id: "place", glyph: "⌂", label: "Place", hint: "click the land to build" },
  { id: "road", glyph: "≡", label: "Road", hint: "click one building, then another" },
  {
    id: "crew",
    glyph: "☗",
    label: "Crew",
    hint: "drag somebody onto a building to make it their home",
  },
  {
    id: "terrain",
    glyph: "▦",
    label: "Terrain",
    hint: "choose the picture the World stands on",
  },
  {
    id: "look",
    glyph: "◱",
    label: "Look",
    hint: "give this World its own windows",
  },
  {
    id: "quest",
    glyph: "✦",
    label: "Quest",
    hint: "",
    pending: "Quest lifecycles (ADR-0025) are not built yet.",
  },
  {
    id: "preview",
    glyph: "▶",
    label: "Preview",
    hint: "",
    pending: "Press PLAY to leave the editor and watch the World run.",
  },
];

export function ModeRail({
  mode,
  onMode,
}: {
  readonly mode: EditMode;
  readonly onMode: (m: EditMode) => void;
}) {
  return (
    <div className="pxpanel wed__rail">
      {TOOLS.map((tool) => (
        <button
          key={tool.id}
          type="button"
          className={`pxbtn wed__mode${mode === tool.id ? " pxbtn--on" : ""}`}
          disabled={Boolean(tool.pending)}
          title={tool.pending ?? tool.hint}
          onClick={() => onMode(tool.id)}
        >
          <span>{tool.glyph}</span>
          <span>{tool.label}</span>
        </button>
      ))}
    </div>
  );
}

/** What the current tool does, for the hint over the World. */
export function hintFor(
  mode: EditMode,
  roadFrom: string | null,
  roadCorners = 0,
  roadTo: string | null = null,
): string {
  if (mode === "road" && roadFrom && roadTo) {
    return `${roadCorners} corner${roadCorners === 1 ? "" : "s"} traced; click the land to bend it, then save route`;
  }
  if (mode === "road" && roadFrom && roadCorners > 0) {
    return `${roadCorners} corner${roadCorners === 1 ? "" : "s"} traced; click the land to bend it, or the destination to pave it`;
  }
  if (mode === "road") {
    return roadFrom
      ? "click the second building · click the same one to cancel"
      : "click the first building";
  }
  return TOOLS.find((t) => t.id === mode)?.hint ?? "";
}
