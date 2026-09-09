import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { ApprovalPrompts } from "./ApprovalPrompts";
import type { CharacterView } from "../../ipc/contracts";
import type { Awaiting, Wanted } from "../../experience/useTurn";

const mage: CharacterView = {
  id: "mage",
  name: "Mage",
  archetype: "guardian",
  home: "tower",
  place: "tower",
  activity: "reading",
  action: "idle", actions: {}, class: "idle",
  mark: null,
  icon: null,
  speaksWith: null,
    soundsLike: null,
  routine: [],
};

const wanted: Wanted = {
  capability: "write_file",
  summary: "write a note",
  risk: "medium",
  reversal: "Delete the note",
  effects: ["writes a file"],
};

const awaiting: Awaiting = {
  capability: "write_file",
  what: "Create notes.md",
  risk: "medium",
  reversal: "Delete notes.md",
  preview: "+ hello",
};

function draw(options: Partial<React.ComponentProps<typeof ApprovalPrompts>> = {}) {
  const onAnswerWanted = vi.fn();
  const onAnswer = vi.fn();
  render(
    <ApprovalPrompts
      who={mage}
      wanted={null}
      awaiting={null}
      onAnswerWanted={onAnswerWanted}
      onAnswer={onAnswer}
      {...options}
    />,
  );
  return { onAnswerWanted, onAnswer };
}

describe("the words that require a decision", () => {
  it("granting a skill never quietly approves the call that follows", async () => {
    const user = userEvent.setup();
    const { onAnswerWanted, onAnswer } = draw({ wanted });

    await user.click(screen.getByRole("button", { name: "GIVE IT TO THEM" }));

    expect(onAnswerWanted).toHaveBeenCalledWith(true);
    expect(onAnswer).not.toHaveBeenCalled();
  });

  it("shows the exact change before it is allowed", () => {
    const { onAnswer } = draw({ awaiting });

    expect(screen.getByText("+ hello")).toHaveClass("dlg__diff");
    expect(screen.getByText(/Nothing has been done/)).toBeInTheDocument();
    expect(onAnswer).not.toHaveBeenCalled();
  });

  it("keeps one-time, standing, and refused permission distinct", async () => {
    const user = userEvent.setup();
    const { onAnswer } = draw({ awaiting });

    await user.click(screen.getByRole("button", { name: "ALLOW ONCE" }));
    await user.click(screen.getByRole("button", { name: "ALWAYS HERE" }));
    await user.click(screen.getByRole("button", { name: "NO" }));

    expect(onAnswer).toHaveBeenNthCalledWith(1, true, false);
    expect(onAnswer).toHaveBeenNthCalledWith(2, true, true);
    expect(onAnswer).toHaveBeenNthCalledWith(3, false, false);
  });
});
