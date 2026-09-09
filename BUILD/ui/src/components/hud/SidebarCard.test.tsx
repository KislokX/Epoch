import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SidebarCard, SidebarRow } from "./SidebarCard";

describe("World sidebar readings", () => {
  it("keeps a dormant frame for an unmeasured subsystem", () => {
    render(
      <SidebarCard title="Connected Models" dormant>
        <span>Nothing can be reached yet.</span>
      </SidebarCard>,
    );

    expect(screen.getByText("Connected Models").closest(".hud__card--dormant")).toBeInTheDocument();
  });

  it("does not make an informational row into a control", () => {
    render(<SidebarRow glyph="gem" label="offline" right="OFFLINE" dim />);

    expect(screen.queryByRole("button", { name: /offline/i })).not.toBeInTheDocument();
    expect(screen.getByText("offline").closest(".hud__row")).toHaveClass("hud__row--dim");
  });

  it("exposes a row as a button only when it has the supplied World action", async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(<SidebarRow glyph="star" label="Tower" active onClick={onClick} />);

    await user.click(screen.getByRole("button", { name: /Tower/i }));
    expect(onClick).toHaveBeenCalledOnce();
    expect(screen.getByText("Tower").closest(".hud__row")).toHaveClass("hud__row--on");
  });
});
