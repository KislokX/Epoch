import { useEffect, useState } from "react";

import {
  imageStudios,
  installStudio,
  startStudio,
  stopStudio,
  type ImageStudio,
} from "../ipc/launcher";

/**
 * The programs on this machine that **make** pictures.
 *
 * ## Why it is in Settings and not in Connections
 *
 * Connections answers *who does the thinking*, and everything listed there becomes a brain a
 * character can be assigned to. An image studio is not one — a character does not think in
 * pictures, it asks for one (ADR-0030) — and a diffusion server in the Brain dropdown would be a
 * brain nobody can hold a conversation with.
 *
 * It is machine configuration, which is what this deck is for: it belongs to a computer rather
 * than to a person, the same test ADR-0026 uses to keep `num_gpu` off a Character.
 *
 * ## The same three facts, deliberately
 *
 * **Installed** · **serving** · **neither**, drawn like the runtimes panel, because they have the
 * same three answers and somebody reading both should not have to learn two vocabularies.
 *
 * ## Serving is not the same as able to draw
 *
 * A fresh ComfyUI answers happily and holds no checkpoint. That is a real state with its own fix,
 * so it is said as its own sentence — the distinction LM Studio forced on the runtimes panel,
 * arrived at from the other direction.
 *
 * ## What Epoch will not answer for you
 *
 * The first run opens a wizard asking where to keep models and to accept its own licence. Epoch
 * opens the door and steps back — a licence accepted on somebody's behalf is not accepted — and
 * this says so **before** the press rather than leaving a window open behind Epoch.
 */
export function ImageStudios() {
  const [studios, setStudios] = useState<readonly ImageStudio[] | null>(null);
  const [said, setSaid] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);

  const read = () => {
    setAsking(true);
    void imageStudios().then((all) => {
      setStudios(all);
      setAsking(false);
    });
  };

  useEffect(read, []);

  const install = async (studio: ImageStudio) => {
    setSaid(null);
    const failed = await installStudio(studio.id);
    setSaid(
      failed ??
        `A terminal opened on the command that installs ${studio.name}. When it finishes, press ASK AGAIN.`,
    );
  };

  const start = async (studio: ImageStudio) => {
    setSaid(null);
    const failed = await startStudio(studio.id);
    setSaid(failed ?? `${studio.name} is opening. ${studio.firstRun}`);
  };

  const stop = async (studio: ImageStudio) => {
    setSaid(null);
    setSaid(await stopStudio(studio.id));
    void read();
  };

  return (
    <div className="rm">
      <div className="rdy__head">
        <span className="rm__label">Making pictures</span>
        <button
          type="button"
          className="btn btn--mini"
          disabled={asking}
          onClick={read}
        >
          {asking ? "ASKING…" : "ASK AGAIN"}
        </button>
      </div>

      {studios === null ? (
        // Unasked is not empty. A walk of the installer directories and a loopback probe take a
        // moment, and rendering nothing meanwhile would read as *there is nothing*.
        <p className="cc__hint">Asking this machine…</p>
      ) : (
        studios.map((studio) => (
          <div key={studio.id} className="lrt">
            <p className="verdict">
              <b className={`runs runs--${verdict(studio).tone}`}>
                {verdict(studio).word}
              </b>{" "}
              {studio.name}
            </p>
            <p className="cc__hint">{detail(studio)}</p>

            {!studio.installed && (
              <>
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() => void install(studio)}
                >
                  INSTALL {studio.name.toUpperCase()}
                </button>
                <p className="cc__hint">{studio.install}</p>
              </>
            )}

            {/*
              **Epoch knew how to start one and not how to stop it.**

              Which made *"it is running and holding memory"* something the user had to go and fix
              in Task Manager. Measured idle on this machine: 2.6 GB of system RAM, 0.03 GB
              reserved on the card, and asking it to free memory returns none of it — 2609 MB
              before and after. What stays is the interpreter, torch's CUDA context and the node
              modules, and none of that goes while the process lives.

              Offered only while it is answering, because that is the only state it means
              anything in — and the cost of pressing it is said, since it is the whole of the
              decision.
            */}
            {studio.serving && (
              <>
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() => void stop(studio)}
                >
                  STOP {studio.name.toUpperCase()}
                </button>
                <p className="cc__hint">
                  Measured on this machine: 2.6 GB back. Starting it again takes
                  about half a minute.
                </p>
              </>
            )}

            {studio.installed && !studio.serving && (
              <>
                <button
                  type="button"
                  className="btn btn--mini"
                  onClick={() => void start(studio)}
                >
                  START {studio.name.toUpperCase()}
                </button>
                {/*
                  Said before the press, not after. The wizard is the reason a first START looks
                  like it did nothing.
                */}
                <p className="cc__hint">{studio.firstRun}</p>
              </>
            )}
          </div>
        ))
      )}

      {said && <p className="notice">{said}</p>}
    </div>
  );
}

/** Installed · serving · neither — the three facts, in a word each. */
function verdict(studio: ImageStudio): { word: string; tone: string } {
  if (studio.serving) return { word: "SERVING", tone: "yes" };
  // Installed and not answering is an ordinary state, not a broken one: it is an application
  // somebody has not opened yet. Saying OFFLINE would send them to reinstall what they have.
  if (studio.installed)
    return { word: "INSTALLED · NOT RUNNING", tone: "unknown" };
  return { word: "NOT INSTALLED", tone: "no" };
}

/** The sentence under the verdict, and each state gets its own. */
function detail(studio: ImageStudio): string {
  if (studio.serving) {
    return studio.models.length === 0
      ? // Running and unable to draw: a real state with its own fix, and the one a person would
        // otherwise diagnose as "it is broken".
        `Answering at ${studio.endpoint}, holding no checkpoint yet — so it is running and cannot make a picture. Download one through its own Manager.`
      : `Answering at ${studio.endpoint}, holding ${studio.models.join(", ")}.`;
  }
  if (studio.installed) {
    return `Found at ${studio.foundAt ?? "this machine"}. It answers on ${studio.endpoint} once it is open.`;
  }
  return "Not on this machine. Epoch opens a terminal on the command and steps back.";
}
