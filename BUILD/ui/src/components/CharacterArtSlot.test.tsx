import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { CharacterArtSlot } from "./CharacterArtSlot";

describe("character art slot", () => {
  it("only offers clearing an image this character actually owns", () => {
    const onClear = vi.fn().mockResolvedValue(null);
    const { rerender } = render(
      <CharacterArtSlot
        label="Icon"
        hint="Their conversation face."
        portrait={<span>no face</span>}
        hasArt={false}
        fallback="The sprite stands in."
        busy={false}
        onChoose={vi.fn().mockResolvedValue(null)}
        onClear={onClear}
      />,
    );

    expect(screen.getByText("The sprite stands in.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Clear" })).not.toBeInTheDocument();

    rerender(
      <CharacterArtSlot
        label="Icon"
        hint="Their conversation face."
        portrait={<span>face</span>}
        hasArt
        busy={false}
        onChoose={vi.fn().mockResolvedValue(null)}
        onClear={onClear}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Clear" }));
    expect(onClear).toHaveBeenCalledOnce();
  });
});
