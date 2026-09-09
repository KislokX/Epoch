import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";

import { ContextGauge } from "./ContextGauge";

function draw(knew: React.ComponentProps<typeof ContextGauge>["knew"]) {
  return render(
    <ContextGauge knew={knew} who="Mage">
      {(trigger) => <div data-testid="controls">{trigger}</div>}
    </ContextGauge>,
  );
}

describe("the context gauge", () => {
  it("keeps a cold reading distinct from a zero-token report", async () => {
    const user = userEvent.setup();
    draw(null);

    const button = screen.getByRole("button", { name: /context/i });
    expect(button).toHaveClass("dlg__ctxbtn--cold");
    expect(button).toHaveTextContent("—");

    await user.click(button);
    expect(screen.getByRole("dialog", { name: "Context" })).toHaveTextContent(
      "Nothing measured yet — this fills in when Mage takes a turn.",
    );
  });

  it("shows the measured percentage and admits dropped blocks", async () => {
    const user = userEvent.setup();
    draw({
      line: "The Quest and the latest evidence fit.",
      used: 128,
      budget: 512,
      dropped: 2,
      reserved: 128,
    });

    const button = screen.getByRole("button", { name: /context/i });
    expect(button).toHaveTextContent("25%");
    expect(button).toHaveClass("dlg__ctxbtn--over");

    await user.click(button);
    const report = screen.getByRole("dialog", { name: "Context" });
    expect(report).toHaveTextContent("128 / 512 (25%)");
    expect(report).toHaveTextContent("2 older blocks did not fit");
    expect(report).toHaveTextContent("Window 640 · 128 kept for the reply.");
  });

  it("reports a known token count without inventing an unknown window", async () => {
    const user = userEvent.setup();
    draw({ line: "Only usage is available.", used: 900, budget: 0, dropped: 0, reserved: 0 });

    expect(screen.getByRole("button", { name: /context/i })).toHaveTextContent("900");
    await user.click(screen.getByRole("button", { name: /context/i }));

    expect(screen.getByRole("dialog", { name: "Context" })).toHaveTextContent(
      "This agent reports what it used and not how much it can hold, so there is no percentage to show.",
    );
  });
});
