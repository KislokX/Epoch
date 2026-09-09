/**
 * The editor's instrument primitives.
 *
 * Small on purpose. Every panel in the editor is a titled frame, a stack of fields, and a few
 * readings — so the vocabulary is fixed here once, and adding an instrument later is a matter
 * of composing these rather than writing another set of divs that drift from the last one.
 *
 * The one non-obvious member is `Cold`. It exists because half of this console is describing
 * subsystems that are not built, and the Launcher already settled how that is shown: keep the
 * frame, lose the light, read zero, and *name what will fill it*. A gauge nobody can explain
 * teaches the user to stop reading the gauges.
 */

import { useEffect, useState } from "react";
import type { ReactNode } from "react";

export function Panel({
  title,
  right,
  children,
  grow,
  cap,
}: {
  readonly title: string;
  readonly right?: ReactNode;
  readonly children: ReactNode;
  /** Take the leftover height. Exactly one panel per column should. */
  readonly grow?: boolean;
  /** A share of the column's height, for a list that must not eat the panel below it. */
  readonly cap?: string;
}) {
  return (
    <section
      className="pxpanel pxsection"
      style={grow ? { flex: 1, minHeight: 0 } : cap ? { maxHeight: cap, flex: "none" } : { flex: "none" }}
    >
      <header className="pxsection__head">
        <div>
          <span className="pxsection__pip" />
          <h2 className="pxtitle">{title}</h2>
        </div>
        {right}
      </header>
      <div className="pxsection__body">{children}</div>
    </section>
  );
}

export function Field({ label, children }: { readonly label: string; readonly children: ReactNode }) {
  return (
    <label className="pxfield">
      <span className="pxlabel">{label}</span>
      <div>{children}</div>
    </label>
  );
}

export function TextInput({
  value,
  onChange,
  mono,
  readOnly,
  onCommit,
}: {
  readonly value: string;
  readonly onChange?: (v: string) => void;
  readonly mono?: boolean;
  readonly readOnly?: boolean;
  /** Called on Enter or blur — the moment the user finished, not every keystroke. */
  readonly onCommit?: (v: string) => void;
}) {
  /*
    What is in the box while it is being typed in.

    It was a controlled input taking `value` from the Engine with only an `onCommit`, so every
    keystroke was thrown away and re-rendered as the old name. The field looked broken because
    it *was*: there was nowhere for a half-typed word to live.

    A commit-on-finish field genuinely needs two values — the truth, and the draft. The
    alternative is writing on every keystroke, which is one vault write per character.
  */
  const [draft, setDraft] = useState<string | null>(null);

  // Nothing being typed, or the Engine's answer changed underneath: show the truth.
  useEffect(() => {
    setDraft(null);
  }, [value]);

  const finish = () => {
    if (draft !== null && draft !== value) onCommit?.(draft);
    setDraft(null);
  };

  return (
    <input
      className={`pxinput${readOnly ? " pxinput--read" : mono ? " pxinput--mono" : ""}`}
      value={draft ?? value}
      readOnly={readOnly}
      onChange={(e) => {
        setDraft(e.target.value);
        onChange?.(e.target.value);
      }}
      onBlur={finish}
      onKeyDown={(e) => {
        // The editor is over a World that listens for keys. Nothing typed into a field is
        // also a command.
        e.stopPropagation();
        if (e.key === "Enter") finish();
        // Abandoned: back to what the Engine says, never a half-typed name.
        if (e.key === "Escape") setDraft(null);
      }}
    />
  );
}

export function Slider({
  value,
  min,
  max,
  step = 0.01,
  suffix,
  onChange,
  onCommit,
}: {
  readonly value: number;
  readonly min: number;
  readonly max: number;
  readonly step?: number;
  readonly suffix?: string;
  readonly onChange: (v: number) => void;
  /**
   * Called on release, not on every movement.
   *
   * Dragging writes the vault on each frame otherwise, and every write re-reads and re-projects
   * the World. Release is also the only moment the user decided anything.
   */
  readonly onCommit?: () => void;
}) {
  return (
    <div className="pxslider">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        onPointerUp={() => onCommit?.()}
        onKeyUp={() => onCommit?.()}
      />
      <b>
        {step < 1 ? value.toFixed(2) : Math.round(value)}
        {suffix ?? ""}
      </b>
    </div>
  );
}

export function Row({ k, v }: { readonly k: string; readonly v: ReactNode }) {
  return (
    <div className="pxrow">
      <span className="pxlabel">{k}</span>
      <span>{v}</span>
    </div>
  );
}

export function Chip({
  children,
  tone,
}: {
  readonly children: ReactNode;
  readonly tone?: "gold" | "leaf";
}) {
  return <span className={`pxchip${tone ? ` pxchip--${tone}` : ""}`}>{children}</span>;
}

/**
 * A reading with nothing behind it yet.
 *
 * Says so, and says which subsystem will answer. This is the difference between an instrument
 * panel and a screenshot of one.
 */
export function Cold({ children }: { readonly children: ReactNode }) {
  return <p className="pxcold">{children}</p>;
}
