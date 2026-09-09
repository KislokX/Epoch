import { useEffect, useState } from "react";

import type { Direction, MarkView } from "../ipc/contracts";
import type { SheetCut } from "../ipc/launcher";
import { ImageDrop } from "./ImageDrop";
import { SheetPlayer } from "./SheetPlayer";
import { unevenly, useNaturalSize } from "./useNaturalSize";

/**
 * One action a character can be drawn doing: the sheet, how it is cut, and it playing.
 *
 * ## Nothing here is guessed
 *
 * The grid and the frame duration start empty and IMPORT stays disabled until they are
 * stated. A default frame rate would be a constant nobody measured, and the person who drew
 * the animation is the only one who knows how fast it runs — the same rule the Engine enforces
 * by refusing a sheet with no cut (ADR-0021 amendment).
 *
 * ## The loop this is built around
 *
 * State the grid, import, **watch it**, fix the grid. That last step re-saves the cut alone
 * rather than re-uploading the same file, because a typo in `rows` is two numbers rather than
 * a new import.
 */
export function AnimationSlot({
  label,
  hint,
  mark,
  busy,
  onChoose,
  onRecut,
  onClear,
}: {
  readonly label: string;
  readonly hint: string;
  /** What is there now, resolved. `null` means nobody has drawn them doing this. */
  readonly mark: MarkView | null;
  readonly busy: boolean;
  readonly onChoose: (dataUri: string, cut: SheetCut) => Promise<string | null>;
  readonly onRecut: (cut: SheetCut) => Promise<string | null>;
  readonly onClear: () => Promise<string | null>;
}) {
  const [form, setForm] = useState(() => blank(mark));

  // When the vault answers, the form follows it. What the editor shows must be what the file
  // holds — a form that kept its own numbers after a save would be a second source of truth.
  useEffect(() => setForm(blank(mark)), [mark]);

  /*
    **What the file measures, against what the form claims.**

    Four numbers that agree with each other is exactly what a wrong cut looks like: `163 ÷ 6` is
    27.17, so every frame after the first sits a fraction of a pixel further off and the last one
    shows part of its neighbour. Nothing in the form can catch that — only the image can.

    Reported, never refused. A sheet may genuinely carry an odd margin and the person who drew it
    is the one who knows; this says what was measured and leaves the decision where it belongs.
  */
  const natural = useNaturalSize(mark?.asset);
  const said = read(form);
  const cut = "cut" in said ? said.cut : null;
  const why = "why" in said ? said.why : null;
  const drawn = mark !== null;
  const changed =
    cut !== null && mark?.frames !== undefined && !same(cut, mark.frames);

  return (
    <div className="anim">
      <span className="anim__label">{label}</span>

      <div className="anim__preview">
        {mark ? (
          <SheetPlayer mark={mark} label={`${label}, playing`} />
        ) : (
          <div className="sheet sheet--none" role="img" aria-label={`no ${label} sheet`}>
            <span>not drawn</span>
          </div>
        )}
      </div>

      <p className="cc__hint">
        {drawn ? hint : "Nobody has drawn this. The still sprite stands in."}
      </p>

      {/* Measured from the file, and only when there is something to say about it. */}
      {drawn && natural && (
        <p className="cc__hint">
          {natural.width}×{natural.height} pixels
          {cut && ` · cells of ${round(natural.width / cut.columns)}×${round(natural.height / cut.rows)}`}
        </p>
      )}
      {drawn && cut && uneven(natural, cut) && (
        <p className="notice notice--warn">{uneven(natural, cut)}</p>
      )}

      <div className="anim__cut">
        <NumberField
          label="Cols"
          value={form.columns}
          onChange={(columns) => setForm({ ...form, columns })}
        />
        <NumberField
          label="Rows"
          value={form.rows}
          onChange={(rows) => setForm({ ...form, rows })}
        />
        <NumberField
          label="Frames"
          value={form.count}
          onChange={(count) => setForm({ ...form, count })}
          hint="0 means every cell"
        />
        <NumberField
          label="ms"
          value={form.milliseconds}
          onChange={(milliseconds) => setForm({ ...form, milliseconds })}
        />
      </div>

      <label className="anim__directions">
        <span>Row directions</span>
        <input
          type="text"
          value={form.directions}
          placeholder="south west east north"
          onChange={(e) => setForm({ ...form, directions: e.target.value })}
        />
      </label>
      {/*
        **What this field is for**, said where it is used rather than left to be inferred.

        A directional sheet is one file whose rows are directions, and the World picks the row
        from the facing it measured — so four directions is four rows, never four imports. Left
        empty the sheet is one loop, which is the right answer for standing about and the wrong
        one for walking.

        Written because the field looked optional next to four numbers, and a placeholder is not
        an explanation: the sheet that prompted this was a one-row idle carrying four direction
        words, which is a cut that cannot be true.
      */}
      <p className="cc__hint">
        {form.directions.trim()
          ? "One word per row, top row first. The World plays the row that matches which way they are walking."
          : "Empty means one loop, whichever way they face — right for standing, wrong for walking. A directional sheet names one direction per row."}
      </p>

      <div className="art__actions">
        <ImageDrop
          label={drawn ? "Replace sheet" : "Import sheet"}
          busy={busy || cut === null}
          // What is missing, in the same breath as the button that will not open. Without it
          // the control looked ordinary, did nothing, and explained nothing.
          why={why}
          className="btn btn--mini"
          onChoose={(data) =>
            cut ? onChoose(data, cut) : Promise.resolve(why ?? "state the grid first")
          }
        />
        {changed && cut && (
          <button
            type="button"
            className="btn btn--mini"
            disabled={busy}
            onClick={() => void onRecut(cut)}
          >
            Re-cut
          </button>
        )}
        {drawn && (
          <button
            type="button"
            className="btn btn--mini"
            disabled={busy}
            onClick={() => void onClear()}
          >
            Clear
          </button>
        )}
      </div>
    </div>
  );
}

function NumberField({
  label,
  value,
  onChange,
  hint,
}: {
  readonly label: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly hint?: string;
}) {
  return (
    <label className="anim__number">
      <span>{label}</span>
      <input
        type="number"
        min={0}
        value={value}
        title={hint}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );
}

interface CutForm {
  readonly columns: string;
  readonly rows: string;
  readonly count: string;
  readonly milliseconds: string;
  readonly directions: string;
}

/** The form as the vault currently answers it — empty when nobody has drawn this. */
function blank(mark: MarkView | null): CutForm {
  const frames = mark?.frames;
  if (!frames) {
    return { columns: "", rows: "", count: "", milliseconds: "", directions: "" };
  }
  return {
    columns: String(frames.columns),
    rows: String(frames.rows),
    count: String(frames.count),
    milliseconds: String(frames.milliseconds),
    directions: frames.directions.join(" "),
  };
}

/**
 * The form as a cut, or `null` when it is not one yet.
 *
 * `null` is what disables IMPORT, and the three things it insists on are the three the Engine
 * insists on: a grid with cells in it, and a duration. A direction word this build does not
 * know is refused here too, so the answer arrives while the user is still looking at the field
 * rather than as an error after an upload.
 */
export function parse(form: CutForm): SheetCut | null {
  const said = read(form);
  return "cut" in said ? said.cut : null;
}

/**
 * The form as a cut, **or the reason it is not one yet**.
 *
 * One function rather than a validator beside a parser, because the two would eventually
 * disagree about which forms are acceptable and the disagreement would show up as a button that
 * refuses a form nothing objects to.
 *
 * The reasons exist because the button had none. IMPORT is a `<label>` over a disabled
 * `<input>`, so seven different mistakes all looked the same from the outside: a control that
 * appears normal, does nothing when clicked, and says nothing about why. A disabled control that
 * cannot explain itself is worse than a missing one — somebody clicks it repeatedly and
 * concludes the feature is broken.
 */
export function read(form: CutForm): { cut: SheetCut } | { why: string } {
  const columns = whole(form.columns);
  const rows = whole(form.rows);
  const count = form.count.trim() === "" ? 0 : whole(form.count);
  const milliseconds = whole(form.milliseconds);

  if (!columns || !rows) return { why: "Say how many columns and rows the sheet has." };
  if (!milliseconds) return { why: "Say how long a frame lasts, in milliseconds." };
  if (count === null) return { why: "Frames must be a whole number, or empty for every cell." };

  const words = form.directions.trim().split(/\s+/).filter(Boolean);
  const directions: Direction[] = [];
  for (const word of words) {
    if (word !== "north" && word !== "east" && word !== "south" && word !== "west") {
      return { why: `"${word}" is not a direction — use north, east, south or west.` };
    }
    directions.push(word);
  }
  if (directions.length > rows) {
    return {
      why: `${directions.length} directions for ${rows} rows — there is one direction per row.`,
    };
  }
  if (count > columns * rows) {
    return { why: `${count} frames will not fit in a ${columns}×${rows} grid.` };
  }

  return { cut: { columns, rows, count, milliseconds, directions } };
}

/** The measured complaint, or nothing. Kept here so the JSX reads as one condition. */
function uneven(
  natural: { width: number; height: number } | null,
  cut: SheetCut,
): string | null {
  return unevenly(natural, cut.columns, cut.rows);
}

/** A cell size a person can read: whole pixels when it is whole, two decimals when it is not. */
function round(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

/** A whole number, or `null`. `0` is falsy on purpose everywhere it is used as a size. */
function whole(raw: string): number | null {
  const value = Number(raw);
  return Number.isInteger(value) && value >= 0 ? value : null;
}

function same(cut: SheetCut, frames: NonNullable<MarkView["frames"]>): boolean {
  return (
    cut.columns === frames.columns &&
    cut.rows === frames.rows &&
    cut.count === frames.count &&
    cut.milliseconds === frames.milliseconds &&
    cut.directions.join(" ") === frames.directions.join(" ")
  );
}
