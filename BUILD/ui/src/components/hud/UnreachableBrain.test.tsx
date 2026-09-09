import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

const reassignBrain = vi.fn(async () => null);

vi.mock("../../ipc/launcher", () => ({
  reassignBrain: (...args: unknown[]) =>
    (reassignBrain as unknown as (...a: unknown[]) => Promise<null>)(...args),
  fetchProviders: vi.fn(async () => [
    {
      id: "ollama",
      name: "Ollama",
      endpoint: "http://localhost:11434",
      online: true,
      local: true,
      models: ["gemma4:12b"],
      note: null,
    },
    {
      id: "bridge:asleep",
      name: "Studio Mac",
      endpoint: "http://192.168.1.31:11500",
      online: false,
      local: false,
      models: ["qwen3.8:27b"],
      note: "not reachable",
    },
  ]),
}));

import { UnreachableBrain, isUnreachable } from "./UnreachableBrain";

const mage = {
  id: "mage",
  name: "Mage",
} as unknown as Parameters<typeof UnreachableBrain>[0]["who"];

const DOWN =
  "studio-mac.local is not reachable at http://192.168.1.20:11500";

function show(props: Partial<Parameters<typeof UnreachableBrain>[0]> = {}) {
  return render(
    <UnreachableBrain
      failure={DOWN}
      who={mage}
      onRetry={vi.fn()}
      onChanged={vi.fn()}
      onStop={vi.fn()}
      {...props}
    />,
  );
}

describe("a Service that did not answer", () => {
  it("names the machine and offers three answers, never a substitute", () => {
    // Epoch does not choose a Brain. The model is part of who somebody is (ADR-0026), so a
    // silent fall to another one is a character behaving differently with nobody told.
    show();
    expect(screen.getByText(/MAGE CANNOT BE REACHED/)).toBeInTheDocument();
    // The Engine's own sentence, verbatim — it already names the address it looked at.
    expect(screen.getByText(new RegExp(DOWN.slice(0, 30)))).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "TRY AGAIN" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /SOMEBODY ELSE/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "STOP" })).toBeInTheDocument();
  });

  it("never names a program it did not measure", () => {
    // The owner met `llama_cpp is not reachable at http://127.0.0.1:8080` directly above
    // *"start it with `ollama serve`"* — a true failure with the wrong fix under it, which
    // sends somebody to start a program that was never the problem.
    //
    // The frontend is not the place to map a Service to a command either: the deck already
    // knows how to start each runtime, measured per machine.
    show({ failure: "llama_cpp is not reachable at http://127.0.0.1:8080" });
    expect(screen.queryByText(/ollama/i)).toBeNull();
    expect(screen.getByText(/CONNECTIONS/)).toBeInTheDocument();
  });

  it("says nothing about a failure that is not a Service being asleep", () => {
    // A CUDA crash and a machine that is off are different faults with different answers.
    const { container } = show({ failure: "the model backend died while loading" });
    expect(container).toBeEmptyDOMElement();
  });

  it("offers only Services that actually answered", async () => {
    // Offering a second machine that is also asleep would be a dead end presented as a way
    // out — the same cold-instrument rule the Launcher follows.
    show();
    fireEvent.click(screen.getByRole("button", { name: /SOMEBODY ELSE/ }));

    await waitFor(() =>
      expect(screen.getByRole("button", { name: /Ollama · gemma4:12b/ })).toBeInTheDocument(),
    );
    expect(screen.queryByRole("button", { name: /Studio Mac/ })).not.toBeInTheDocument();
  });

  it("records why, so the Chronicle keeps the cause and not just the change", async () => {
    // Handoffs have visible causes (ADR-0025). The reason travels with the reassignment.
    const onChanged = vi.fn();
    show({ onChanged });
    fireEvent.click(screen.getByRole("button", { name: /SOMEBODY ELSE/ }));
    const pick = await screen.findByRole("button", { name: /Ollama · gemma4:12b/ });
    fireEvent.click(pick);

    await waitFor(() =>
      expect(reassignBrain).toHaveBeenCalledWith("mage", "ollama", "gemma4:12b", DOWN),
    );
    await waitFor(() => expect(onChanged).toHaveBeenCalled());
  });

  it("nothing is preselected, because choosing is the point", () => {
    // The picker opens closed. A default would be Epoch choosing a Brain by omission.
    show();
    expect(screen.queryByRole("button", { name: /Ollama/ })).not.toBeInTheDocument();
  });
});

describe("which failures are this question", () => {
  it("matches the Provider's own wording and nothing else", () => {
    expect(isUnreachable("Ollama is not reachable at http://localhost:11434")).toBe(true);
    expect(isUnreachable("Ollama refused the request: 404")).toBe(false);
    expect(isUnreachable("no model chosen, and Ollama offers none")).toBe(false);
  });
});
