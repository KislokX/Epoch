/**
 * Missions — the window that could not be clicked.
 *
 * Every case here is a defect that actually happened, not one imagined afterwards:
 *
 * - the ✕ rendered, was aimed at, and the click went through to the World, because the HUD's band
 *   is `pointer-events: none` and this overlay never turned it back on for itself;
 * - the rows ran out through the bottom of the frame and took the page bar with them;
 * - and a closed conversation must read as what it was, because a History that keeps only
 *   successes is propaganda (ADR-0025 §7).
 */

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Missions } from "./Missions";
import type { QuestSummary } from "../ipc/world";

/** `n` conversations, newest first, the way the Engine returns them. */
function conversations(n: number): QuestSummary[] {
  return Array.from({ length: n }, (_, i) => ({
    id: `q${String(i).padStart(4, "0")}`,
    title: `Conversation ${i}`,
    state: i === 0 ? "open" : "abandoned",
    open: i === 0,
    current: i === 0,
    participants: ["mage"],
    saidCount: i,
    producedEvidence: i % 3 === 0,
  }));
}

describe("Missions", () => {
  it("shows fifteen at a time and numbers the pages", async () => {
    // 34 is what the user actually had when the page bar first appeared.
    render(<Missions all={conversations(34)} onOpen={() => {}} onClose={() => {}} />);

    expect(screen.getByText("Conversation 0")).toBeInTheDocument();
    expect(screen.getByText("Conversation 14")).toBeInTheDocument();
    expect(screen.queryByText("Conversation 15")).not.toBeInTheDocument();

    // 34 / 15 = three pages. A position somebody can point at — "it was on page three" is a
    // sentence, and an infinite scroll cannot be pointed at.
    expect(screen.getByRole("button", { name: "3" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "4" })).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "3" }));
    expect(screen.getByText("Conversation 30")).toBeInTheDocument();
    expect(screen.queryByText("Conversation 0")).not.toBeInTheDocument();
  });

  it("hides the page bar when everything fits", () => {
    render(<Missions all={conversations(15)} onOpen={() => {}} onClose={() => {}} />);
    expect(screen.queryByRole("navigation", { name: "Pages" })).not.toBeInTheDocument();
  });

  it("closes when the ✕ is pressed", async () => {
    // The defect: it rendered, and the click landed on the World behind it.
    const onClose = vi.fn();
    render(<Missions all={conversations(3)} onOpen={() => {}} onClose={onClose} />);

    await userEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("opens the conversation that was clicked, by identity", async () => {
    const onOpen = vi.fn();
    render(<Missions all={conversations(3)} onOpen={onOpen} onClose={() => {}} />);

    await userEvent.click(screen.getByText("Conversation 2"));
    // The id, never the row number: a list that renumbers must not reopen something else.
    expect(onOpen).toHaveBeenCalledWith("q0002");
  });

  it("says what an ended conversation was, without softening it", () => {
    render(<Missions all={conversations(3)} onOpen={() => {}} onClose={() => {}} />);

    expect(screen.getByText("OPEN")).toBeInTheDocument();
    // `abandoned`, not "archived". The record is not softened on the way to the screen.
    expect(screen.getAllByText("ABANDONED").length).toBe(2);
  });

  it("says nothing exists rather than showing an empty frame", () => {
    render(<Missions all={[]} onOpen={() => {}} onClose={() => {}} />);
    expect(screen.getByText(/Nothing yet/)).toBeInTheDocument();
  });
});
