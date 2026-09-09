/**
 * The four rules every window in Epoch has to get right.
 *
 * Each case here is a real inconsistency between the windows that existed before this component:
 * one closed on Escape and another did not, one closed on a scrim click and another did not,
 * none moved focus in, none put it back, and the one that forgot `pointer-events` rendered a ✕
 * that could be aimed at and clicked straight through to the World.
 */

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Overlay } from "./Overlay";

describe("Overlay", () => {
  it("closes on Escape", async () => {
    const onClose = vi.fn();
    render(
      <Overlay label="Missions" onClose={onClose}>
        <button type="button">Inside</button>
      </Overlay>,
    );

    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("closes when the scrim is pressed, and not when the window is", async () => {
    const onClose = vi.fn();
    render(
      <Overlay label="Missions" onClose={onClose}>
        <button type="button">Inside</button>
      </Overlay>,
    );

    await userEvent.click(screen.getByRole("button", { name: "Inside" }));
    expect(onClose).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("dialog"));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("can refuse to be dismissed by a stray click", async () => {
    // For a window in the middle of something — a form half filled in. Escape still works,
    // because a window you cannot leave at all is a trap.
    const onClose = vi.fn();
    render(
      <Overlay label="New World" onClose={onClose} dismissOnScrim={false}>
        <button type="button">Inside</button>
      </Overlay>,
    );

    await userEvent.click(screen.getByRole("dialog"));
    expect(onClose).not.toHaveBeenCalled();

    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("takes the focus, and gives it back", async () => {
    // A keyboard user tabbing through the World behind an open dialog is the World being
    // reachable when it should not be — the same defect as the click going through, one input
    // device over.
    const opener = document.createElement("button");
    opener.textContent = "Open";
    document.body.append(opener);
    opener.focus();
    expect(document.activeElement).toBe(opener);

    const { unmount } = render(
      <Overlay label="Missions" onClose={() => {}}>
        <button type="button">Inside</button>
      </Overlay>,
    );
    expect(document.activeElement).toBe(screen.getByRole("button", { name: "Inside" }));

    unmount();
    expect(document.activeElement).toBe(opener);
    opener.remove();
  });

  it("is a named dialog, so it is not announced as just 'dialog'", () => {
    render(
      <Overlay label="Missions" onClose={() => {}}>
        <p>Nothing focusable in here at all.</p>
      </Overlay>,
    );

    const dialog = screen.getByRole("dialog", { name: "Missions" });
    expect(dialog).toHaveAttribute("aria-modal", "true");
    // The class carries the rule that was forgotten: the HUD's band does not hand out clicks,
    // and every window turns them back on for itself.
    expect(dialog).toHaveClass("overlay");
  });
});
