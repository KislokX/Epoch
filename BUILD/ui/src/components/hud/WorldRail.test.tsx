/**
 * The World's rails, and the one thing they may never do to the cards in them.
 *
 * ## The defect this holds shut
 *
 * Measured 2026-08-25 in the running window (CDP, real DOM), with three cards open on each side:
 * the left rail was 699px tall and its cards held 227px, 148px and 1150px of content — while
 * being 114px, 83px and 486px tall. A flex item shrinks by default and a `.frame` does not clip,
 * so every card painted its own contents out over the card beneath it: CREW's second crew member
 * ran under the AGENTS plate, AGENTS' help text ran under WORKFLOWS.
 *
 * The rail's `overflow-y: auto` never engaged, because nothing ever overflowed it. The children
 * gave way first. That is why this looked like a layout that worked — right up until a second
 * crew member, or a fourth model, arrived.
 *
 * ## Why the stylesheet is read as text
 *
 * jsdom applies no stylesheet, so no rendered assertion can see this. The invariant is a
 * declaration, and the only place it exists is the file — so the file is what is asserted. Crude,
 * and it fails the moment somebody deletes the rule, which is the whole job.
 */

import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { SidebarCard } from "./SidebarCard";

// Read from disk, not imported. Vitest replaces every `.css` import with an empty string —
// `?raw` included, measured — so the stylesheet has to be read as what it is: a file.
import { readFileSync } from "node:fs";

const HUD = readFileSync("src/app/hud.css", "utf8");

/** The declarations inside one rule, whitespace collapsed. */
function block(selector: string): string {
  const at = HUD.indexOf(`\n${selector} {`);
  expect(at, `no rule for ${selector}`).toBeGreaterThan(-1);
  const open = HUD.indexOf("{", at);
  return HUD.slice(open + 1, HUD.indexOf("}", open)).replace(/\s+/g, " ").trim();
}

describe("a card in a World rail", () => {
  it("cannot be squeezed below its own contents", () => {
    // The whole defect in one declaration. `flex: none` is also what makes the rail's own
    // scroller real: without it the column is a scroller with nothing to scroll.
    expect(block(".hud__side > .frame")).toContain("flex: none");
  });

  it("bounds a list by its card, never by the window", () => {
    const rule = block(".hud__card-list--scrolls");
    expect(rule).toContain("overflow-y: auto");
    // It takes exactly the height its card was given.
    expect(rule).toContain("flex: 1");
    expect(rule).toContain("min-height: 0");
    /*
      And no slice of the viewport. `max-height: 32vh` measured a different box from the one the
      list is in: maximised at 2560x1369 it capped MODELS' list at 438px inside a card 761px
      tall, and drew the list's own edge across the panel — read, correctly, as the list being
      cut off. Stopping one list from eating the rail is the card's job, where the other two
      cards are; a second rule measuring the window could only ever disagree with it.
    */
    expect(rule).not.toMatch(/max-height/);
  });

  it("lets a scrolling card give way so no frame in the rail is ever cut off", () => {
    // A fixed slice left the right rail eight pixels taller than it could hold, so the Terminal's
    // bottom bevel was clipped and the rail grew a scrollbar to explain it. With the scrollbar
    // drawn as nothing, a clipped frame would be the only sign left — an instrument cut in half
    // with no explanation, which is worse than the widget it replaced.
    const rule = block(".hud__side > .frame.hud__card--fills");
    // Shrinks, and never grows past its own contents: sharing the rail's spare height gave CREW
    // 338px for two crew members and MODELS 761px for a 438px list. Spare height belongs at the
    // bottom of the rail, where it is World.
    expect(rule).toContain("flex: 0 1 auto");
    // The floor where giving way stops and the rail scrolls instead. A window too short for
    // three instruments is real, and squeezing them into nothing is where this started.
    expect(rule).toMatch(/min-height:\s*\d+px/);
  });

  it("lets every box in a filling card shrink on both axes", () => {
    // A flex item's automatic minimum size is its content's, on the main axis. `.frame` is a row
    // flex, so a body that would not give way pushed MODELS' rows 160px past the frame's right
    // edge — measured at 1236 → 1590 inside a frame ending at 1430. The row's label had
    // `text-overflow: ellipsis` the whole time and never got the chance to use it.
    //
    // Both axes on every box, because one that will not give is enough.
    for (const selector of [
      ".hud__card--fills > .frame__body",
      ".hud__card--fills .hud__card",
      ".hud__card-list--scrolls",
    ]) {
      const rule = block(selector);
      expect(rule, `${selector} must give way vertically`).toContain("min-height: 0");
      expect(rule, `${selector} must give way horizontally`).toContain("min-width: 0");
    }
    // And sideways scrolling is named rather than inherited: `overflow-y: auto` alone computes
    // `overflow-x: auto`, which is a scroller nobody asked for and now an invisible one.
    expect(block(".hud__card-list--scrolls")).toContain("overflow-x: hidden");
  });

  it("draws no scrollbar, and still scrolls", () => {
    // An immersion leak: a pixel-drawn thumb in a bevelled track is still a scrollbar, and this
    // one ran the full height of the rail *over* the frames. What says there is more is a row
    // clipped at the edge of a bounded list — the content itself, not a widget about it.
    const rule = block(".ep-scroll");
    expect(rule).toContain("scrollbar-width: none");
    expect(block(".ep-scroll::-webkit-scrollbar")).toContain("width: 0");
    // The class only says how it is *drawn*. Nothing here may turn scrolling off.
    expect(rule).not.toContain("overflow");
  });

  it("keeps a Workflow's second line on one line", () => {
    // `3 SAID · NO EVIDENCE` is wider than a 208px rail, so it wrapped and then escaped the
    // bevel. The title had the ellipsis treatment; its own subtitle did not.
    const rule = block(".hud__chat i");
    expect(rule).toContain("white-space: nowrap");
    expect(rule).toContain("text-overflow: ellipsis");
  });

  it("asks for the bounded scroll only when the card said it has an endless list", () => {
    const { rerender } = render(<SidebarCard title="Crew">rows</SidebarCard>);
    expect(screen.getByText("rows").className).not.toContain("hud__card-list--scrolls");

    rerender(
      <SidebarCard title="Crew" scrolls>
        rows
      </SidebarCard>,
    );
    const list = screen.getByText("rows");
    expect(list.className).toContain("hud__card-list--scrolls");
    // The World's own scrolling, which is drawn as nothing.
    expect(list.className).toContain("ep-scroll");
    // And the card itself, because the height is handed down through the Frame's own boxes.
    expect(list.closest(".frame")?.className).toContain("hud__card--fills");
  });
});
