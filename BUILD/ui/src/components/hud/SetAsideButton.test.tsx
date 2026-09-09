import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SetAsideButton } from "./SetAsideButton";

describe("the next Quest action", () => {
  it("does not offer a Quest to put down when none exists", () => {
    const { container } = render(<SetAsideButton hasQuest={false} onSetAside={() => {}} />);

    expect(container).toBeEmptyDOMElement();
  });

  it("puts aside exactly the current Quest action when the user starts a new one", async () => {
    const user = userEvent.setup();
    const onSetAside = vi.fn();
    render(<SetAsideButton hasQuest onSetAside={onSetAside} />);

    await user.click(screen.getByRole("button", { name: "NEW" }));
    expect(onSetAside).toHaveBeenCalledOnce();
  });
});
