/**
 * The World decision that says how much the crew may do on its own.
 *
 * Modes are an Engine reading, not a UI enum: a new mode must arrive here with its exact id,
 * label and description. This component never keeps a second selected value. The parent writes
 * the user's choice, then asks the Engine for the resulting `AutonomyView` again.
 *
 * The note describes the actual gate for the active brain when that fact is known. Otherwise it
 * falls back to the Engine's description of the selected mode. A generic claim about every
 * backend would be worse than no extra explanation: not every agent asks Epoch in the same way.
 */

import type { AutonomyView } from "../../ipc/world";

interface ModePickerProps {
  /** The World truth: selected mode plus every mode currently offered by the Engine. */
  readonly autonomy: AutonomyView;
  /** The active brain's name, if its reasoning reading has arrived. */
  readonly brain: string | null;
  /** What that brain's gate does in this mode, as reported by the Engine. */
  readonly gate: string | null;
  /** The exact Engine mode id the user selected. Persistence belongs to the parent. */
  readonly onPick: (mode: string) => void;
}

export function ModePicker({ autonomy, brain, gate, onPick }: ModePickerProps) {
  const selected = autonomy.modes.find((mode) => mode.id === autonomy.current);

  return (
    <label className="dlg__mode">
      <span className="dlg__mode-label">MODE</span>
      {/* One control: a row of buttons would grow whenever the Engine adds a mode. */}
      <select
        className="dlg__mode-pick"
        value={autonomy.current}
        onChange={(event) => onPick(event.target.value)}
      >
        {autonomy.modes.map((mode) => (
          <option key={mode.id} value={mode.id}>
            {mode.label}
          </option>
        ))}
      </select>
      <span className="dlg__mode-note">
        {/*
          This is not Epoch's `decide()`: Epoch has no descriptor or risk rating for an agent's
          own Write. An agent may invite Epoch into that decision before a tool (Claude), or a
          sandbox may report a blocked change so Epoch can offer a retry (Codex). This wording
          therefore comes from the Engine's per-agent gate reading, not a generic sentence.
        */}
        {gate && brain ? `${brain} ${gate}.` : selected?.describe}
      </span>
    </label>
  );
}
