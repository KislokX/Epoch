/**
 * The page bar, and the clamping that keeps a panel from showing an empty page.
 *
 * Three panels use this now. The behaviour worth asserting is not the rendering — it is what
 * happens when the list *shrinks under a reader*, which is exactly what CLEAR does.
 */

import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Pager, pageOf, pages, slice } from "./Pager";

describe("paging", () => {
  it("always has at least one page, even with nothing in it", () => {
    // An empty list is a valid state everywhere in this product. A count of zero pages would make
    // every caller special-case it.
    expect(pages(0, 6)).toBe(1);
    expect(pageOf(0, 0, 6)).toBe(0);
    expect(slice([], 0, 6)).toEqual([]);
  });

  it("clamps a page that no longer exists", () => {
    // The CLEAR case: you are reading page four, the list shrinks to nine entries, and page four
    // is gone. Clamping on read shows page two rather than an empty frame that state has not
    // caught up with.
    expect(pageOf(3, 9, 6)).toBe(1);
    expect(pageOf(-2, 9, 6)).toBe(0);
  });

  it("cuts the list where the page is", () => {
    const all = Array.from({ length: 14 }, (_, i) => i);
    expect(slice(all, 0, 6)).toEqual([0, 1, 2, 3, 4, 5]);
    expect(slice(all, 1, 6)).toEqual([6, 7, 8, 9, 10, 11]);
    // The last page is short, not padded.
    expect(slice(all, 2, 6)).toEqual([12, 13]);
  });
});

describe("Pager", () => {
  it("draws nothing when everything fits", () => {
    // A bar with one button is furniture: it offers a choice that does not exist.
    const { container } = render(
      <Pager count={6} perPage={6} at={0} onGo={() => {}} label="Pages" />,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("numbers the pages and says which one is current", () => {
    render(<Pager count={14} perPage={6} at={1} onGo={() => {}} label="Pages" />);

    expect(screen.getByRole("button", { name: "3" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "4" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "2" })).toHaveAttribute("aria-current", "page");
  });

  it("cannot go back from the first page or on from the last", () => {
    const { rerender } = render(
      <Pager count={14} perPage={6} at={0} onGo={() => {}} label="Pages" />,
    );
    expect(screen.getByRole("button", { name: "Previous page" })).toBeDisabled();

    rerender(<Pager count={14} perPage={6} at={2} onGo={() => {}} label="Pages" />);
    expect(screen.getByRole("button", { name: "Next page" })).toBeDisabled();
  });

  it("reports the page that was asked for", async () => {
    const onGo = vi.fn();
    render(<Pager count={20} perPage={6} at={0} onGo={onGo} label="Pages" />);

    await userEvent.click(screen.getByRole("button", { name: "3" }));
    // Zero-based, because that is what `slice` takes. The label a person reads is one-based.
    expect(onGo).toHaveBeenCalledWith(2);
  });
});
