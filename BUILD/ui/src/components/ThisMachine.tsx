import type { MachineView } from "../ipc/contracts";
import { useThisMachine } from "./useThisMachine";

/**
 * The machine Epoch is running on, measured.
 *
 * ## Why it belongs beside the remote ones
 *
 * The Machines deck answers *which computers can this World use*, and the first answer is
 * always this one. Showing the paired machines' hardware while saying nothing about the machine
 * the user is sitting at would be a list with a hole in the middle of it.
 *
 * It also appears in the Models Workshop, where it answers a different question — *will this
 * download run here* — and that is the reason it is not moved: the two readings share a source
 * (`this_machine`) but not a purpose, and a Workshop that had to send somebody to another deck
 * to find out whether a model fits would have stopped being a Workshop.
 *
 * ## The firewall switch that used to be here
 *
 * It allowed inbound TCP 11501, which existed so a remote could dial *this* machine. That
 * direction is gone: a router that stops a wireless client from starting a connection broke it
 * entirely, and the arrangement that survives has this machine reach out instead. Nothing
 * listens on 11501 any more, so a switch opening it would be an instrument nobody could
 * explain — it would grant something no part of Epoch uses.
 *
 * If that direction returns, so does the switch. `pairing::port_is_open` and `open_the_port`
 * are still in the Engine for exactly that reason.
 *
 * ## Unknown is a reading
 *
 * A machine with no NVIDIA card has no VRAM number, and an Apple machine has none to have — its
 * memory is unified. Splitting system memory into an invented "video" share would be describing
 * a division the hardware does not have, so those lines are simply absent and the card says why.
 */
export function ThisMachine() {
  const { machine, asked } = useThisMachine();

  return (
    <div className="rm">
      <span className="rm__label">This machine</span>
      {!asked ? (
        <p className="cc__hint">Measuring…</p>
      ) : machine ? (
        <>
          <p className="brg__machine">{describe(machine)}</p>
          <p className="cc__hint">
            Where a turn runs unless a character names another machine. Read from
            the hardware itself, never from a setting.
          </p>
        </>
      ) : (
        // Cold instrument: the Engine did not answer, which is different from a machine with
        // nothing in it.
        <p className="cc__hint">
          Epoch could not read this machine&apos;s hardware, so it is not saying
          anything about it.
        </p>
      )}

    </div>
  );
}

/** The hardware in one line, with every absent number simply absent. */
function describe(machine: MachineView): string {
  const parts: string[] = [];
  if (machine.gpu) {
    parts.push(
      machine.vramFree != null && machine.vramTotal != null
        ? `${machine.gpu} · ${gb(machine.vramFree)} of ${gb(machine.vramTotal)} ${
            machine.unified ? "unified memory " : ""
          }free`
        : machine.gpu,
    );
  }
  // On a unified machine the total above *is* the system memory, and saying it twice reads as
  // two pools — which is exactly the impression this wording exists to avoid.
  const alreadySaid = machine.unified && machine.vramTotal != null;
  if (machine.ramTotal != null && !alreadySaid) {
    parts.push(`${gb(machine.ramTotal)} system memory`);
  }
  if (parts.length === 0) {
    // A desktop with an AMD or Intel card and no NVIDIA driver. A Mac used to land here too and
    // no longer does: it was never unmeasurable, only asked the wrong question.
    return "No graphics card Epoch can ask about, and no memory reading.";
  }
  return parts.join(" · ");
}

function gb(bytes: number): string {
  return `${(bytes / 1_000_000_000).toFixed(1)} GB`;
}
