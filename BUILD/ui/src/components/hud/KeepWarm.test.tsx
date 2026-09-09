import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { KeepWarm } from "./KeepWarm";
import * as world from "../../ipc/world";
import type { Warmth } from "../../ipc/world";

function reading(over: Partial<Warmth> = {}): Warmth {
  return {
    model: "gemma4:12b",
    holding: false,
    chosen: false,
    resident: false,
    ...over,
  };
}

describe("the lamp over a conversation", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("draws nothing at all when there is no reading to draw", async () => {
    // An agent brain, a hosted model, a local server that will not answer. `null` is unasked,
    // never "not loaded" — and a lamp that cannot read is worse than no lamp.
    vi.spyOn(world, "fetchWarmth").mockResolvedValue(
      reading({ resident: null }),
    );
    const { container } = render(<KeepWarm characterId="mage" settled={0} />);
    await waitFor(() => expect(world.fetchWarmth).toHaveBeenCalled());
    expect(container.querySelector(".keepwarm")).toBeNull();
  });

  it("lights from the card and never from the switch", async () => {
    // The whole reason these are two facts. A model the user asked to hold, which llama.cpp's
    // router has since evicted for somebody else's turn: the switch is on, the card is empty,
    // and the lamp must say so.
    vi.spyOn(world, "fetchWarmth").mockResolvedValue(
      reading({ holding: true, chosen: true, resident: false }),
    );
    render(<KeepWarm characterId="mage" settled={0} />);

    const button = await screen.findByRole("button");
    expect(button).toHaveClass("keepwarm--on");
    expect(button).not.toHaveClass("keepwarm--lit");
    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(button.title).toMatch(/not loaded/i);
  });

  it("lights for a model somebody else loaded", async () => {
    // The mirror image: resident without Epoch having asked for it — opened in LM Studio's own
    // window. The reading is the card's, not Epoch's memory of what it did.
    vi.spyOn(world, "fetchWarmth").mockResolvedValue(
      reading({ holding: false, resident: true }),
    );
    render(<KeepWarm characterId="mage" settled={0} />);

    const button = await screen.findByRole("button");
    expect(button).toHaveClass("keepwarm--lit");
    expect(button).not.toHaveClass("keepwarm--on");
  });

  it("marks a choice and never the default", async () => {
    // Holding is the default now, so a gold button on every character in the World would mark
    // nothing — the owner noticed the button had changed shape before he noticed anything else.
    vi.spyOn(world, "fetchWarmth").mockResolvedValue(
      reading({ holding: true, chosen: false, resident: true }),
    );
    const plain = render(<KeepWarm characterId="mage" settled={0} />);
    const untouched = await plain.findByRole("button");
    expect(untouched).not.toHaveClass("keepwarm--on");
    expect(untouched).not.toHaveClass("keepwarm--off");
    // The lamp still says what is on the card, which is the fact that actually moves.
    expect(untouched).toHaveClass("keepwarm--lit");
    plain.unmount();

    vi.spyOn(world, "fetchWarmth").mockResolvedValue(
      reading({ holding: true, chosen: true, resident: true }),
    );
    const pinned = render(<KeepWarm characterId="mage" settled={1} />);
    expect(await pinned.findByRole("button")).toHaveClass("keepwarm--on");
    pinned.unmount();

    vi.spyOn(world, "fetchWarmth").mockResolvedValue(
      reading({ holding: false, chosen: true, resident: false }),
    );
    const released = render(<KeepWarm characterId="mage" settled={2} />);
    expect(await released.findByRole("button")).toHaveClass("keepwarm--off");
  });

  it("says when nobody chose this for the character", async () => {
    // **The default is holding now** (2026-08-28), so the sentence changed with it: this used
    // to read *"following the machine's setting"*, which was true when `concurrentCrew` decided
    // whether a model was held at all. It decides how many may be held; it does not decide this.
    vi.spyOn(world, "fetchWarmth").mockResolvedValue(
      reading({ holding: true, chosen: false, resident: true }),
    );
    render(<KeepWarm characterId="mage" settled={0} />);
    const button = await screen.findByRole("button");
    expect(button.title).toMatch(/stays loaded after each answer/i);
    expect(button.title).toMatch(/nobody has chosen/i);
  });

  it("a character nobody touched keeps its model, and says so", async () => {
    // The behaviour the owner corrected: releasing after every answer cost ~19 s a message on
    // this machine, to protect against a cost only *several* models have.
    vi.spyOn(world, "fetchWarmth").mockResolvedValue(
      reading({ holding: true, chosen: false, resident: false }),
    );
    render(<KeepWarm characterId="mage" settled={0} />);
    const button = await screen.findByRole("button");
    expect(button.title).toMatch(/not loaded/i);
    expect(button.title).toMatch(/stays loaded after each answer/i);
    expect(button.title).not.toMatch(/released after each answer/i);
  });

  it("asks the Engine for the opposite of what is set, and takes its answer", async () => {
    vi.spyOn(world, "fetchWarmth").mockResolvedValue(reading());
    const hold = vi
      .spyOn(world, "holdWarm")
      .mockResolvedValue(reading({ holding: true, chosen: true }));

    const user = userEvent.setup();
    render(<KeepWarm characterId="mage" settled={0} />);
    await user.click(await screen.findByRole("button"));

    expect(hold).toHaveBeenCalledWith("mage", true);
    // Not optimistic: the switch shows what came back. Turning it on loads nothing, so the
    // lamp stays where the card is.
    await waitFor(() =>
      expect(screen.getByRole("button")).toHaveAttribute("aria-pressed", "true"),
    );
    expect(screen.getByRole("button")).not.toHaveClass("keepwarm--lit");
  });
});
