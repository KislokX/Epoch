/**
 * Decisions the user deliberately made permanent in this World.
 *
 * A standing allow and a standing deny are both World truth, read back from the Engine. This
 * component does not rebuild their meaning from capability ids: `describe` is the Engine's own
 * sentence. Removing a decision likewise returns the whole exact record, so the parent can ask
 * the Engine to forget precisely the scope and person the user saw.
 */

import type { Standing } from "../../ipc/world";

interface StandingDecisionsProps {
  readonly decisions: readonly Standing[];
  /** Take back exactly the decision shown; persistence remains the parent's Engine work. */
  readonly onRevoke: (decision: Standing) => void | Promise<void>;
}

export function StandingDecisions({ decisions, onRevoke }: StandingDecisionsProps) {
  if (decisions.length === 0) return null;

  return (
    <details className="dlg__standing">
      <summary>
        {decisions.length} standing {decisions.length === 1 ? "decision" : "decisions"}
      </summary>
      <ul>
        {decisions.map((decision) => (
          <li
            key={`${decision.capability}-${decision.scope}-${decision.who ?? ""}`}
            className={decision.allowed ? "" : "dlg__standing--deny"}
          >
            <span>{decision.describe}</span>
            <button
              type="button"
              className="epbtn"
              title="Take this decision back"
              aria-label="Take this decision back"
              onClick={() => void onRevoke(decision)}
            >
              ✕
            </button>
          </li>
        ))}
      </ul>
    </details>
  );
}
