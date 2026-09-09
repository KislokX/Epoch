import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { EditorFailureNotice } from "./EditorFailureNotice";

describe("editor failure notice", () => {
  it("keeps the Engine's refusal visible and dismissible with a real button", () => {
    const onDismiss = vi.fn();
    const { rerender } = render(
      <EditorFailureNotice failure="Mage is already thinking." onDismiss={onDismiss} />,
    );

    const button = screen.getByRole("button", { name: "Dismiss failure: Mage is already thinking." });
    expect(button).toHaveTextContent("Mage is already thinking.");
    fireEvent.click(button);
    expect(onDismiss).toHaveBeenCalledOnce();

    rerender(<EditorFailureNotice failure={null} onDismiss={onDismiss} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });
});
