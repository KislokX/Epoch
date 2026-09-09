import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { BoundedControl } from "./ProviderControl";

describe("bounded provider control", () => {
  it("leaves an untouched bounded setting at the provider default and can restore it", () => {
    const onChange = vi.fn();
    const { rerender } = render(
      <BoundedControl
        label="Temperature"
        value={null}
        min={0}
        max={2}
        step={0.05}
        onChange={onChange}
      />,
    );

    expect(screen.getByText("provider default")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Clear Temperature" })).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Temperature"), { target: { value: "1.25" } });
    expect(onChange).toHaveBeenCalledWith(1.25);

    rerender(
      <BoundedControl
        label="Temperature"
        value={1.25}
        min={0}
        max={2}
        step={0.05}
        onChange={onChange}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Clear Temperature" }));
    expect(onChange).toHaveBeenLastCalledWith(null);
  });

});
