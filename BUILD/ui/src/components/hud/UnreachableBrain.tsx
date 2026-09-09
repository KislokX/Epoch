import { useEffect, useState } from "react";

import type { CharacterView, ProviderStatus } from "../../ipc/contracts";
import { fetchProviders, reassignBrain } from "../../ipc/launcher";

/**
 * The assigned Brain could not be reached, and the user answers.
 *
 * ## Why this is a question rather than a failover
 *
 * Epoch never chooses a Brain. Canonical parameters are behavioural identity (ADR-0026) and the
 * model is the same argument one field up: the same prompt on Gemma, Qwen and Claude reasons
 * differently. Falling to another model silently would be a character behaving differently with
 * nobody told — and a character nobody can explain is worse than a gauge nobody can explain.
 *
 * So when a machine is asleep, Mage does not quietly become another Mage. Epoch says which
 * machine, and offers three answers.
 *
 * ## Why there is no setting behind it
 *
 * A preferred-and-fallback pair per character was the other design. It was not built, on
 * purpose: nobody yet knows how often a lent machine is actually down, and two new fields on
 * every character would be paying for a problem that might happen once a week. This asks when it
 * happens, with the facts in front of the person, and adds nothing to configure. If the evidence
 * later says it happens constantly, the standing choice gets built *with that evidence*.
 *
 * ## What it shows
 *
 * Only Services that **answered**, with the models they reported. Offering a machine that is
 * also unreachable would be a second dead end presented as a way out — the same cold-instrument
 * rule the Launcher follows. Nothing is preselected: choosing is the whole point.
 */
export function UnreachableBrain({
  failure,
  who,
  onRetry,
  onChanged,
  onStop,
}: {
  /** The Engine's own words. `null` when nothing failed. */
  readonly failure: string | null;
  /** Whose brain it was, so the question names a person. */
  readonly who: CharacterView | null;
  /** Ask them again with what they were already asked. */
  readonly onRetry: () => void | Promise<void>;
  /** Reassigned; the caller decides whether to carry on. */
  readonly onChanged: () => void | Promise<void>;
  readonly onStop: () => void;
}) {
  const [services, setServices] = useState<readonly ProviderStatus[] | null>(
    null,
  );
  const [picking, setPicking] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const unreachable = failure !== null && isUnreachable(failure);

  useEffect(() => {
    if (!picking) return;
    let alive = true;
    // Probed when the picker opens rather than on mount: asking every Service whether it is
    // awake reaches the network, and a failure notice nobody acts on should not cost that.
    void fetchProviders().then((all) => {
      if (alive) setServices(all.filter((one) => one.online));
    });
    return () => {
      alive = false;
    };
  }, [picking]);

  if (!unreachable || !who) return null;

  const choose = async (backend: string, model: string) => {
    setBusy(true);
    const failed = await reassignBrain(who.id, backend, model, failure);
    setBusy(false);
    if (failed) {
      setRefused(failed);
      return;
    }
    setPicking(false);
    setRefused(null);
    await onChanged();
  };

  return (
    <div className="brainless">
      <p className="brainless__said">
        <b>{who.name.toUpperCase()} CANNOT BE REACHED</b>
        {/*
          The Engine's own sentence, verbatim. It already names the Service and the address it
          looked at, and rewriting it in friendlier words would be Epoch restating something it
          did not measure.
        */}
        {failure}
      </p>

      {/*
        Advice about *this* computer, and only when it is this computer — a paired machine
        asleep across the room is not started from here, and this used to be offered for both.

        **And it no longer names a command.** It said `ollama serve` for every local Service,
        which is wrong for two of the three this build can reach: the owner met it as
        `llama_cpp is not reachable` directly above an instruction to start Ollama. A notice
        that names the wrong program is worse than one that names none — it sends somebody to
        fix something that was never broken, and they come back to the same failure.

        The frontend must not map a Service id to a command either: the deck already knows how
        to start each runtime, measured per machine, and a second place deciding is how the two
        come to disagree. So this points at that control instead of restating it.
      */}
      {isHere(failure) && (
        <p className="brainless__note">
          It runs on this machine and is not running. BRIDGE &rarr; CONNECTIONS starts it,
          then try again.
        </p>
      )}

      {!picking ? (
        <div className="brainless__acts">
          <button type="button" className="epbtn epbtn--primary" onClick={() => void onRetry()}>
            TRY AGAIN
          </button>
          <button type="button" className="epbtn" onClick={() => setPicking(true)}>
            SOMEBODY ELSE&apos;S MACHINE
          </button>
          <button type="button" className="epbtn" onClick={onStop}>
            STOP
          </button>
        </div>
      ) : services === null ? (
        <p className="brainless__note">Asking which Services are awake…</p>
      ) : services.length === 0 ? (
        // Cold instrument: nothing answered, and saying so beats an empty list that reads as a
        // control that failed to load.
        <p className="brainless__note">
          Nothing else answered. Every Service configured here is offline too.
        </p>
      ) : (
        <>
          <p className="brainless__note">
            {who.name} will think with this instead, from now until you change it
            back. The Chronicle will say you changed it.
          </p>
          <ul className="brainless__list">
            {services.flatMap((service) =>
              service.models.map((model) => (
                <li key={`${service.id}/${model}`}>
                  <button
                    type="button"
                    className="epbtn"
                    disabled={busy}
                    onClick={() => void choose(service.id, model)}
                  >
                    {service.name} · {model}
                  </button>
                  {/* Where it runs, because that is the decision being made. */}
                  <em>{service.local ? "this machine" : service.endpoint}</em>
                </li>
              )),
            )}
          </ul>
        </>
      )}

      {refused && <p className="notice notice--warn">{refused}</p>}
    </div>
  );
}

/**
 * Whether this failure is a Service that did not answer.
 *
 * Matched on the Provider's own wording — `"{provider} is not reachable at {endpoint}"` — which
 * is the sentence the Engine already produces for every backend kind. A structured signal would
 * be better and the channel carries a string today; when it stops being a string, this is the
 * one place that has to change.
 */
export function isUnreachable(failure: string): boolean {
  return failure.includes("is not reachable at");
}

/** Whether the address it could not reach is on this computer. */
function isHere(failure: string): boolean {
  return failure.includes("localhost") || failure.includes("127.0.0.1");
}
