import { useEffect, useState } from "react";

import type { MachineView } from "../ipc/contracts";
import { fetchMachine } from "../ipc/launcher";

/**
 * What this computer is, measured once and read wherever it is needed.
 *
 * ## Why it is a hook rather than a fetch in each place
 *
 * Three surfaces ask the same question for three different reasons — the Machines list (*which
 * computers can this World use*), the Workshop (*will this download run here*) and the ship's
 * diagnostics (*how much of the card is gone*) — and the second copy of an effect is where two
 * answers start being possible. One decision, one place.
 *
 * ## `null` is two states, and they are not the same
 *
 * `machine` is `null` while nothing has been asked yet **and** when the ask failed. `asked`
 * separates them, because a gauge reading `—` before anything looked would be reporting an
 * absence nobody measured — the same distinction `HuggingFace` draws between *not installed*
 * and *unasked*.
 */
export function useThisMachine(): {
  machine: MachineView | null;
  asked: boolean;
} {
  const [machine, setMachine] = useState<MachineView | null>(null);
  const [asked, setAsked] = useState(false);

  useEffect(() => {
    let alive = true;
    void fetchMachine().then((next) => {
      if (!alive) return;
      setMachine(next);
      setAsked(true);
    });
    return () => {
      alive = false;
    };
  }, []);

  return { machine, asked };
}
