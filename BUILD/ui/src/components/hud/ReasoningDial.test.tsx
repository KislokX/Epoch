/**
 * The rule this component exists to keep: **a brain with no measured ladder shows no control.**
 *
 * Verified against the real CLI, not assumed: `claude --effort max --model haiku` runs happily
 * and changes nothing, and the CLI's own model picker says *"Effort not supported for Haiku"*. A
 * slider that silently does nothing is worse than a missing one — it teaches the reader that the
 * instruments are decorative, which is the one thing the Launcher's rule forbids.
 */

import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ReasoningDial } from "./ReasoningDial";
import type { Dial } from "../../ipc/world";

const CLAUDE: Dial = {
  brain: "Claude Code",
  agent: true,
  agentId: "claude-code",
  chosen: null,
  rungs: [
    { id: "low", label: "Low" },
    { id: "medium", label: "Medium" },
    { id: "high", label: "High" },
    { id: "xhigh", label: "xHigh" },
    { id: "max", label: "Max" },
  ],
};

describe("ReasoningDial", () => {
  it("draws nothing when the brain has no ladder", () => {
    // Haiku. Not a disabled button, not a cold reading — nothing at all.
    const { container } = render(
      <ReasoningDial
        dial={{ brain: "Claude Code", agent: true, agentId: "claude-code", chosen: null, rungs: [] }}
        who="Mage"
        onChoose={() => {}}
      />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("draws nothing before the ladder has been asked for", () => {
    const { container } = render(<ReasoningDial dial={null} who="Mage" onChoose={() => {}} />);
    expect(container).toBeEmptyDOMElement();
  });

  it("reads AUTO when nothing was asked for", () => {
    // `null` is a real position — the brain's own default — and it is not the same as the least.
    render(<ReasoningDial dial={CLAUDE} who="Mage" onChoose={() => {}} />);
    expect(screen.getByText("AUTO")).toBeInTheDocument();
  });

  it("reads the rung's label when one was chosen", () => {
    render(
      <ReasoningDial dial={{ ...CLAUDE, chosen: "xhigh" }} who="Mage" onChoose={() => {}} />,
    );
    expect(screen.getByText("xHigh")).toBeInTheDocument();
  });

  it("puts the brain's own default at the left of the scale", async () => {
    render(<ReasoningDial dial={CLAUDE} who="Mage" onChoose={() => {}} />);
    await userEvent.click(screen.getByRole("button", { name: /How hard Mage deliberates/ }));

    const slider = screen.getByRole("slider");
    expect(slider).toHaveValue("0");
    // Five rungs plus Auto: six positions, so the maximum index is five.
    expect(slider).toHaveAttribute("max", "5");
  });

  it("reports the rung the user landed on, by id", async () => {
    const onChoose = vi.fn();
    render(<ReasoningDial dial={CLAUDE} who="Mage" onChoose={onChoose} />);
    await userEvent.click(screen.getByRole("button", { name: /How hard Mage deliberates/ }));

    // `fireEvent.change`, not `userEvent.type`: arrow keys on a range input are handled by the
    // browser itself, and user-event does not simulate that. Typing at it fires nothing at all,
    // which reads exactly like a broken handler.
    fireEvent.change(screen.getByRole("slider"), { target: { value: "4" } });

    // Position 4 is the fourth rung, because position 0 is the brain's own default.
    expect(onChoose).toHaveBeenLastCalledWith("xhigh");
  });

  it("reports null when the user returns to the brain's default", async () => {
    const onChoose = vi.fn();
    render(<ReasoningDial dial={{ ...CLAUDE, chosen: "low" }} who="Mage" onChoose={onChoose} />);
    await userEvent.click(screen.getByRole("button", { name: /How hard Mage deliberates/ }));

    fireEvent.change(screen.getByRole("slider"), { target: { value: "0" } });
    // `null`, never `"off"`: Claude Code has no such level, and asking for one would be Epoch
    // storing an instruction nobody can carry out.
    expect(onChoose).toHaveBeenLastCalledWith(null);
  });
});
