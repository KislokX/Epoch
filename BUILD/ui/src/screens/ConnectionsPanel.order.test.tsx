/**
 * Where the answer is, relative to the configuration.
 *
 * The deck's whole subject is a status, and to find out what was connected you had to read
 * through installing, starting and per-runtime parameters first. That is the one rule this
 * project states about every screen — *what is happening, why, what should I do next* — failing
 * on the screen it matters most for.
 *
 * Asserted on the **document order** rather than on a snapshot: what is being held is that the
 * summary comes first, not what either part happens to say today.
 */

import { describe, expect, it, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { invoke } from "@tauri-apps/api/core";

vi.mock("../components/ThisMachine", () => ({
  ThisMachine: () => <div data-testid="machine">THIS MACHINE</div>,
}));
vi.mock("../components/Readiness", () => ({
  Readiness: (props: { omit?: readonly string[] }) => (
    <div data-testid="readiness" data-omit={(props.omit ?? []).join(",")}>
      EVERYTHING THAT CAN ANSWER
    </div>
  ),
}));
vi.mock("../components/LocalRuntimes", () => ({
  LocalRuntimes: () => <div data-testid="runtimes">RUNTIMES HERE</div>,
}));

import { ConnectionsPanel } from "./ConnectionsPanel";

/** The Engine, answering enough for the panel to draw. Nothing here is what is being tested. */
function engine() {
  vi.mocked(invoke).mockImplementation((command: string) => {
    if (command === "list_agents") return Promise.resolve([]);
    if (command === "list_backends")
      return Promise.resolve({ backends: [], problem: null });
    return Promise.resolve([]);
  });
}

function panel() {
  return render(
    <ConnectionsPanel providers={[]} probing={false} onProbe={() => {}} />,
  );
}

describe("what the Connections deck answers first", () => {
  it("puts what can answer above how it is configured", async () => {
    engine();
    panel();

    const summary = await screen.findByTestId("readiness");
    const detail = await screen.findByTestId("runtimes");

    // `compareDocumentPosition` reads the real order, not the order the file happens to declare.
    expect(
      summary.compareDocumentPosition(detail) &
        Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
  });

  it("leaves nothing out of the summary", async () => {
    /*
      `omit` was right in the old order — the panel directly above reported the three local
      runtimes *and could act on them*. Reversed, the reasoning reverses: a summary that leaves
      out the three programs you most want to know about is not a summary.

      It also removes a hazard. `omit` was filled by `LocalRuntimes` as it rendered; above it,
      the first paint would list all three and drop them a moment later — and a row that
      vanishes is a wrong instrument, not a tidy list.
    */
    engine();
    panel();
    await waitFor(() =>
      expect(screen.getByTestId("readiness").dataset.omit).toBe(""),
    );
  });
});
