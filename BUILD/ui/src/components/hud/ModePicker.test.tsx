import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ModePicker } from "./ModePicker";

const AUTONOMY = {
  current: "ask",
  modes: [
    { id: "manual", label: "Manual", describe: "Nothing starts without you." },
    { id: "ask", label: "Ask", describe: "The crew asks before acting." },
    { id: "auto", label: "Auto", describe: "The crew acts within its grants." },
  ],
} as const;

describe("the autonomy mode picker", () => {
  it("returns the exact Engine mode id the user selected", async () => {
    const user = userEvent.setup();
    const onPick = vi.fn();
    render(<ModePicker autonomy={AUTONOMY} brain={null} gate={null} onPick={onPick} />);

    await user.selectOptions(screen.getByRole("combobox"), "auto");

    expect(onPick).toHaveBeenCalledWith("auto");
  });

  it("uses the Engine's selected-mode description until it has a brain-specific gate", () => {
    const { rerender } = render(
      <ModePicker autonomy={AUTONOMY} brain={null} gate={null} onPick={() => {}} />,
    );

    expect(screen.getByText("The crew asks before acting.")).toBeInTheDocument();

    rerender(
      <ModePicker
        autonomy={AUTONOMY}
        brain="Claude Code"
        gate="asks Epoch before each tool"
        onPick={() => {}}
      />,
    );

    expect(screen.getByText("Claude Code asks Epoch before each tool.")).toBeInTheDocument();
    expect(screen.queryByText("The crew asks before acting.")).not.toBeInTheDocument();
  });
});
