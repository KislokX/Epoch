/**
 * A turn failed, with the measured evidence and the smallest useful way forward.
 *
 * The parent asks the Engine whether an installed agent is actually signed out. This component
 * deliberately does not probe or infer that state: an agent's error text is not proof. The raw
 * failure also remains visible, because a friendly translation must not replace the evidence.
 */

import { useEffect } from "react";

import { playSfx } from "../../experience/sfx";
import type { AgentStatus } from "../../ipc/launcher";
import { isUnreachable } from "./UnreachableBrain";

interface TurnFailureNoticeProps {
  readonly failure: string | null;
  readonly signedOut: AgentStatus | null;
  readonly onSignIn: (agentId: string) => void;
  readonly onDismiss: () => void;
}

/**
 * A dismissal lets the conversation continue; recovery advice appears only when its trigger was
 * observed. Signing in stays in the agent's own window, so Epoch never handles a credential.
 */
export function TurnFailureNotice({
  failure,
  signedOut,
  onSignIn,
  onDismiss,
}: TurnFailureNoticeProps) {
  useEffect(() => {
    if (failure) playSfx("error");
  }, [failure]);

  if (!failure) return null;

  /*
    **A Service that did not answer is somebody else's question now.**

    `UnreachableBrain` renders above this one with the machine named and three answers, so
    repeating the same sentence here would say it twice — and the advice this used to add was
    wrong for half the cases anyway: `ollama serve` fixes a runtime on *this* computer and says
    nothing useful about a paired machine that is asleep across the room.
  */
  if (isUnreachable(failure)) return null;

  const backendDied =
    failure.includes("CUDA") ||
    failure.includes("0xc0000409") ||
    failure.includes("llama-server process has terminated");

  return (
    <p className="dlg__failed">
      {failure}
      {signedOut && (
        <>
          <br />
          {signedOut.name} is installed but signed out.{" "}
          <button
            type="button"
            className="epbtn dlg__signin"
            onClick={() => {
              onSignIn(signedOut.id);
            }}
          >
            SIGN IN
          </button>{" "}
          Its own sign-in opens in its own window — Epoch never sees the credential. Then say it
          again.
        </>
      )}
      {backendDied && (
        <>
          <br />
          The model backend died while loading rather than refusing — usually there was no room
          left on the graphics card, which happens when a second character loads a second model
          while the first is still resident. Run <code>setx OLLAMA_MAX_LOADED_MODELS 1</code> and
          restart Ollama so it unloads one before loading the next, or give this character a
          smaller model.
        </>
      )}
      <button
        type="button"
        className="dlg__dismiss"
        onClick={onDismiss}
        title="Put this away"
        aria-label="Dismiss"
      >
        ✕
      </button>
    </p>
  );
}
