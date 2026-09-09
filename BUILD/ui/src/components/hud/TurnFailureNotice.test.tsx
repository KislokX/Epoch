import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { TurnFailureNotice } from "./TurnFailureNotice";

const SIGNED_OUT = {
  id: "codex",
  kind: "codex",
  name: "Codex",
  installed: true,
  version: "0.147.0",
  lookedIn: "C:\\Users\\someone\\AppData\\Local\\OpenAI\\Codex",
  note: null,
  signedIn: false,
  account: null,
} as const;

describe("a failed turn", () => {
  it("does not make an absent failure into interface furniture", () => {
    const { container } = render(
      <TurnFailureNotice failure={null} signedOut={null} onSignIn={() => {}} onDismiss={() => {}} />,
    );

    expect(container).toBeEmptyDOMElement();
  });

  it("offers the sign-in belonging to the measured signed-out agent", async () => {
    const user = userEvent.setup();
    const onSignIn = vi.fn();
    render(
      <TurnFailureNotice
        failure="Codex is not logged in"
        signedOut={SIGNED_OUT}
        onSignIn={onSignIn}
        onDismiss={() => {}}
      />,
    );

    expect(document.body).toHaveTextContent("Codex is installed but signed out.");
    await user.click(screen.getByRole("button", { name: "SIGN IN" }));
    expect(onSignIn).toHaveBeenCalledWith("codex");
  });

  it("keeps the underlying failure and shows recovery only for its observed trigger", () => {
    const { rerender } = render(
      <TurnFailureNotice
        failure="Ollama is not reachable"
        signedOut={null}
        onSignIn={() => {}}
        onDismiss={() => {}}
      />,
    );

    expect(document.body).toHaveTextContent("Ollama is not reachable");
    // The `ollama serve` hint moved to `UnreachableBrain`, which knows whether the address it
    // could not reach is on this computer — this notice never could, and offered it for a
    // paired machine asleep in another room.
    expect(screen.queryByText("ollama serve")).not.toBeInTheDocument();

    rerender(
      <TurnFailureNotice
        failure="llama-server process has terminated"
        signedOut={null}
        onSignIn={() => {}}
        onDismiss={() => {}}
      />,
    );
    expect(screen.getByText("setx OLLAMA_MAX_LOADED_MODELS 1")).toBeInTheDocument();
  });

  it("lets the user dismiss an old failure without taking another action", async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn();
    render(
      <TurnFailureNotice
        failure="something went wrong"
        signedOut={null}
        onSignIn={() => {}}
        onDismiss={onDismiss}
      />,
    );

    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
