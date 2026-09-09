import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { UndoNotice } from "./UndoNotice";

const CREATED = { id: "archipelago#7", summary: "created notes.md" };

describe("the World undo notice", () => {
  it("does not draw when the Journal has nothing new to put back", () => {
    const { container, rerender } = render(
      <UndoNotice change={null} hidden={null} onUndo={() => {}} onDismiss={() => {}} />,
    );
    expect(container).toBeEmptyDOMElement();

    rerender(
      <UndoNotice change={CREATED} hidden={CREATED.id} onUndo={() => {}} onDismiss={() => {}} />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("names the World rather than the current dialogue", () => {
    render(<UndoNotice change={CREATED} hidden={null} onUndo={() => {}} onDismiss={() => {}} />);

    expect(screen.getByText(CREATED.summary).parentElement).toHaveTextContent(
      "Last change in this World",
    );
  });

  it("keeps undo and dismissal as different actions", async () => {
    const user = userEvent.setup();
    const onUndo = vi.fn();
    const onDismiss = vi.fn();
    render(<UndoNotice change={CREATED} hidden={null} onUndo={onUndo} onDismiss={onDismiss} />);

    await user.click(screen.getByRole("button", { name: "UNDO" }));
    expect(onUndo).toHaveBeenCalledOnce();
    expect(onDismiss).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(onDismiss).toHaveBeenCalledWith(CREATED.id);
    expect(onUndo).toHaveBeenCalledOnce();
  });

  it("a later change with the same wording is not already dismissed", () => {
    // The defect. "Read" was remembered as the sentence, so creating `notes.md`, putting the
    // notice away and creating `notes.md` again produced an identical summary — and the second
    // change, a real one the user might well want to undo, never appeared at all.
    const again = { id: "archipelago#8", summary: "created notes.md" };
    render(<UndoNotice change={again} hidden={CREATED.id} onUndo={() => {}} onDismiss={() => {}} />);

    expect(screen.getByText(again.summary)).toBeInTheDocument();
  });

  it("dismissing in one World does not hide a change in another", () => {
    // The same failure one level up: the id carries its World, so these are two changes.
    const elsewhere = { id: "default#7", summary: "created notes.md" };
    render(
      <UndoNotice change={elsewhere} hidden={CREATED.id} onUndo={() => {}} onDismiss={() => {}} />,
    );

    expect(screen.getByText(elsewhere.summary)).toBeInTheDocument();
  });
});
