import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { StandingDecisions } from "./StandingDecisions";

const ALLOW = {
  capability: "write_file",
  scope: "world",
  who: null,
  allowed: true,
  describe: "Allow Mage to edit files in this World.",
} as const;

const DENY = {
  capability: "shell",
  scope: "character",
  who: "mage",
  allowed: false,
  describe: "Never let Mage run shell commands.",
} as const;

describe("standing decisions", () => {
  it("does not draw a cold decisions panel when the Engine has none", () => {
    const { container } = render(<StandingDecisions decisions={[]} onRevoke={() => {}} />);

    expect(container).toBeEmptyDOMElement();
  });

  it("keeps an Engine denial visibly distinct and names both decisions", () => {
    render(<StandingDecisions decisions={[ALLOW, DENY]} onRevoke={() => {}} />);

    expect(screen.getByText("2 standing decisions")).toBeInTheDocument();
    expect(screen.getByText(DENY.describe).closest("li")).toHaveClass("dlg__standing--deny");
    expect(screen.getByText(ALLOW.describe)).toBeInTheDocument();
  });

  it("returns the exact displayed decision when the user takes it back", async () => {
    const user = userEvent.setup();
    const onRevoke = vi.fn();
    render(<StandingDecisions decisions={[ALLOW]} onRevoke={onRevoke} />);

    await user.click(screen.getByRole("button", { name: "Take this decision back" }));

    expect(onRevoke).toHaveBeenCalledWith(ALLOW);
  });
});
