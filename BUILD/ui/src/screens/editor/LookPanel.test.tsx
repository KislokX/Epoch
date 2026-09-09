/**
 * The LOOK panel, and the two things about it that would be silent failures.
 *
 * A wrong inset is visible the moment somebody looks at the preview. These are not: a panel that
 * opens on defaults quietly discards numbers an author measured off their own artwork, and a
 * SAVE that sends stale ones writes something nobody chose.
 */

import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { Skin } from "../../experience/useSkin";
import { LookPanel } from "./LookPanel";

/** The default pack's own window, whose insets were measured off the artwork. */
const WINDOW: Skin = {
  image: "data:image/png;base64,iVBORw0KGgo=",
  corner: [11, 5, 12, 6],
  repeat: "stretch",
  scale: 1,
  minSize: [24, 24],
};

const inset = (side: string) =>
  screen.getByLabelText(`${side} inset`, { exact: false }) as HTMLInputElement;

describe("the LOOK panel", () => {
  it("says a World with no window of its own is complete, not unfinished", () => {
    render(
      <LookPanel
        current={undefined}
        onSave={vi.fn()}
        onClear={vi.fn()}
        sounds={{}}
        onSound={vi.fn()}
      />,
    );

    expect(screen.getByText(/Epoch draws its own windows/)).toBeInTheDocument();
    // Nothing to preview and nothing to tune: the numbers appear with the artwork.
    expect(screen.queryByLabelText(/top inset/)).not.toBeInTheDocument();
  });

  it("opens on the World's own numbers rather than on a guess that would undo them", () => {
    // The failure this prevents is silent. A panel that opened on `[12, 12, 12, 12]` would show
    // plausible values, and the first SAVE would overwrite insets somebody measured by eye.
    render(
      <LookPanel
        current={WINDOW}
        onSave={vi.fn()}
        onClear={vi.fn()}
        sounds={{}}
        onSound={vi.fn()}
      />,
    );

    expect(inset("top").value).toBe("11");
    expect(inset("right").value).toBe("5");
    expect(inset("bottom").value).toBe("12");
    expect(inset("left").value).toBe("6");
  });

  it("saves the numbers that are on screen, not the ones it started with", async () => {
    const onSave = vi.fn().mockResolvedValue(null);
    render(
      <LookPanel
        current={WINDOW}
        onSave={onSave}
        onClear={vi.fn()}
        sounds={{}}
        onSound={vi.fn()}
      />,
    );

    // Importing is what turns SAVE on: the numbers alone are not a change until there is
    // artwork to attach them to.
    expect(screen.queryByRole("button", { name: /USE THIS WINDOW/ })).not.toBeInTheDocument();

    const file = new File(["x"], "window.png", { type: "image/png" });
    const input = document.querySelector("input[type=file]") as HTMLInputElement;
    Object.defineProperty(input, "files", { value: [file] });
    fireEvent.change(input);

    const save = await screen.findByRole("button", { name: /USE THIS WINDOW/ });
    fireEvent.change(inset("top"), { target: { value: "3" } });
    fireEvent.click(save);

    expect(onSave).toHaveBeenCalledOnce();
    const call = onSave.mock.calls[0];
    expect(call).toBeDefined();
    const [, corner, repeat, scale] = call!;
    expect(corner).toEqual([3, 5, 12, 6]);
    expect(repeat).toBe("stretch");
    expect(scale).toBe(1);
  });
});
