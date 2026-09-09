import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { RemovalView } from "../ipc/contracts";
import { RemoveConfirm } from "./RemoveConfirm";

function plan(overrides: Partial<RemovalView> = {}): RemovalView {
  return {
    what: "Mage",
    files: ["mage.toml", "mage.png"],
    consequences: ["Mage stops living in default."],
    survives: ["Their work stays in History."],
    ...overrides,
  };
}

describe("a removal confirmation", () => {
  it("will not remove until the name is typed", () => {
    // A button that ends something gets pressed without reading, and the three lists above it
    // exist to be read.
    const onConfirm = vi.fn();
    render(
      <RemoveConfirm
        plan={plan()}
        busy={false}
        onCancel={vi.fn()}
        onConfirm={onConfirm}
      />,
    );

    const remove = screen.getByRole("button", { name: "REMOVE" });
    expect(remove).toBeDisabled();

    fireEvent.change(screen.getByLabelText(/Type/), {
      target: { value: "mage" },
    });
    expect(remove).toBeDisabled();

    fireEvent.change(screen.getByLabelText(/Type/), {
      target: { value: "Mage" },
    });
    expect(remove).toBeEnabled();
    fireEvent.click(remove);
    expect(onConfirm).toHaveBeenCalledOnce();
  });

  it("says what survives, not only what goes", () => {
    // The list people forget to write, and the one that stops a correct deletion from reading
    // as a broken one.
    render(
      <RemoveConfirm
        plan={plan()}
        busy={false}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(screen.getByText("Deleted")).toBeInTheDocument();
    expect(screen.getByText("Changes")).toBeInTheDocument();
    expect(screen.getByText("Survives")).toBeInTheDocument();
    expect(screen.getByText("Their work stays in History.")).toBeInTheDocument();
  });

  it("forgets a name typed for somebody else", () => {
    // A different plan is a different person, and the last one's name must not unlock this one.
    const { rerender } = render(
      <RemoveConfirm
        plan={plan()}
        busy={false}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByLabelText(/Type/), {
      target: { value: "Mage" },
    });
    expect(screen.getByRole("button", { name: "REMOVE" })).toBeEnabled();

    rerender(
      <RemoveConfirm
        plan={plan({ what: "Paladin" })}
        busy={false}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(screen.getByRole("button", { name: "REMOVE" })).toBeDisabled();
  });

  it("says it is asking rather than showing an empty plan", () => {
    render(
      <RemoveConfirm
        plan={null}
        busy={false}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(
      screen.getByText("Reading what that would remove…"),
    ).toBeInTheDocument();
  });
});
