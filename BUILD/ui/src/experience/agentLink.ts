/**
 * What an agent's status *means*, decided once.
 *
 * ## Why this file exists
 *
 * Four surfaces answered this question independently — the Launcher's CREW LINKS, the
 * Connections panel, the World's MODELS card, and the boot readout — and each one re-derived
 * "installed, signed in, or neither" from the raw fields. Adding a fifth state to that
 * arrangement meant finding four places, and the one nobody found kept showing the old answer
 * next to three that showed the new one.
 *
 * So the *decision* lives here and the surfaces choose only how much room to give it. A fifth
 * state is now one edit.
 *
 * ## The states, and why there are five
 *
 * Installed and signed-in have different fixes, so they never collapse (`CLAUDE.md`). `ready`
 * is the third fact that turned up when a CLI let Epoch read *which* sign-in the user chose
 * without letting it verify the credential works: more than nothing, less than `online`.
 *
 * `unknown` stays a real answer. No reading beats an invented one.
 */

import type { AgentStatus } from "../ipc/launcher";

export type AgentState =
  /** Not on this machine. The fix is an install. */
  | "absent"
  /** The agent confirmed a session itself. The fix is nothing. */
  | "online"
  /**
   * Epoch read which sign-in method was chosen and cannot verify the credential.
   *
   * Deliberately not `online`: a chosen method is not a working credential, as an exhausted
   * quota demonstrates. Deliberately not `unknown`: somebody configured this, and telling them
   * nothing is known would send them to check something that is fine.
   */
  | "ready"
  /** Installed, and nobody has signed in. The fix is a sign-in. */
  | "signedOut"
  /** The question could not be asked. Never rendered as a negative answer. */
  | "unknown";

export interface AgentLink {
  readonly state: AgentState;
  /** The short word a status column shows. */
  readonly label: string;
  /** The line under the name: an account, a method, or the agent's own explanation. */
  readonly detail: string;
  /** Whether the lamp is lit. Configured counts; unknown does not. */
  readonly lit: boolean;
}

export function agentLink(agent: AgentStatus): AgentLink {
  if (!agent.installed) {
    return {
      state: "absent",
      label: "ABSENT",
      // The agent's own words when it gave them — a measured reason always beats a written one.
      detail: agent.note ?? "not installed",
      lit: false,
    };
  }
  if (agent.signedIn === true) {
    return {
      state: "online",
      label: "ONLINE",
      detail: agent.account ?? "signed in",
      lit: true,
    };
  }
  if (agent.signedIn === false) {
    return { state: "signedOut", label: "SIGNED OUT", detail: "signed out", lit: false };
  }
  if (agent.method) {
    return {
      state: "ready",
      label: "READY",
      detail: agent.method,
      lit: true,
    };
  }
  return { state: "unknown", label: "—", detail: "could not ask", lit: false };
}
