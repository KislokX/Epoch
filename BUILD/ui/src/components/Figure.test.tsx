import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { CharacterView, MarkView } from "../ipc/contracts";
import { Figure } from "./Figure";

const ASSET = "data:image/png;base64,iVBORw0KGgo=";

function still(): MarkView {
  return {
    role: "visual",
    renderer: "sprite",
    supported: true,
    asset: ASSET,
    scale: 0.34,
    anchor: [0.5, 1],
    shape: [],
  };
}

function walkSheet(): MarkView {
  return {
    ...still(),
    renderer: "animated_sprite",
    frames: {
      columns: 4,
      rows: 4,
      count: 16,
      milliseconds: 100,
      directions: ["south", "west", "east", "north"],
    },
  };
}

function someone(overrides: Partial<CharacterView> = {}): CharacterView {
  return {
    id: "mage",
    name: "Mage",
    archetype: "researcher",
    home: "tower",
    place: "tower",
    activity: "reading",
    action: "idle",
    actions: {},
    class: "idle",
    mark: null,
    icon: null,
    speaksWith: null,
    soundsLike: null,
    routine: [],
    ...overrides,
  };
}

describe("figure", () => {
  it("lets a keyboard user speak to an idle character", () => {
    const onTalkTo = vi.fn();
    render(
      <svg>
        <Figure
          character={someone()}
          at={{ x: 0, y: 0 }}
          onTalkTo={onTalkTo}
        />
      </svg>,
    );

    const target = screen.getByRole("button", { name: "Speak to Mage" });
    fireEvent.keyDown(target, { key: "Enter" });
    fireEvent.keyDown(target, { key: " " });

    expect(onTalkTo).toHaveBeenCalledTimes(2);
  });

  it("draws the sheet the action names, and falls back to the still picture", () => {
    // ADR-0016's chain, with one more link than it had: the action, then the still picture,
    // then the stand-in. A character drawn walking but not working keeps their face for work
    // rather than losing it.
    const { container, rerender } = render(
      <svg>
        <Figure
          character={someone({
            action: "walk",
            actions: { walk: walkSheet() },
            mark: still(),
            journey: {
              from: "tower",
              to: "library",
              progress: 0.4,
              speed: 60,
              etaSeconds: 6,
              facing: "east",
            },
          })}
          at={{ x: 0, y: 0 }}
        />
      </svg>,
    );
    expect(container.querySelector(".mark--sheet")).toBeInTheDocument();

    rerender(
      <svg>
        <Figure
          character={someone({
            action: "work",
            actions: { walk: walkSheet() },
            mark: still(),
          })}
          at={{ x: 0, y: 0 }}
        />
      </svg>,
    );
    expect(container.querySelector(".mark--sheet")).not.toBeInTheDocument();
    expect(container.querySelector(".mark--asset")).toBeInTheDocument();
  });

  it("plays the row the Engine says they are walking, and only that row", () => {
    // Somebody walking east must not cycle through north on the way. The row comes from the
    // journey the Engine measured, never from the coordinates this is drawing with.
    vi.useFakeTimers();
    const { container } = render(
      <svg>
        <Figure
          character={someone({
            action: "walk",
            actions: { walk: walkSheet() },
            journey: {
              from: "tower",
              to: "library",
              progress: 0.4,
              speed: 60,
              etaSeconds: 6,
              facing: "east",
            },
          })}
          at={{ x: 0, y: 0 }}
        />
      </svg>,
    );

    const image = container.querySelector(".mark--sheet image") as SVGImageElement;
    const size = 90 * 0.34;
    // "east" is the third row of this sheet, so the strip is lifted by two whole cells and
    // stays there while the column advances.
    expect(image.getAttribute("y")).toBe(String(-size - 2 * size));
    expect(image.getAttribute("x")).toBe(String(-size / 2));

    act(() => void vi.advanceTimersByTime(100));
    const moved = container.querySelector(".mark--sheet image") as SVGImageElement;
    expect(moved.getAttribute("y")).toBe(String(-size - 2 * size));
    expect(moved.getAttribute("x")).toBe(String(-size / 2 - size));
    vi.useRealTimers();
  });

  it("keeps the World's own gait for somebody with no sheet, and drops it for somebody with one", () => {
    // The fallback in one assertion: the CSS bob **is** the walk for anybody nobody drew
    // walking, and laying it over a sheet would make somebody bob to a rhythm their own
    // animation is not keeping.
    const journey = {
      from: "tower",
      to: "library",
      progress: 0.4,
      speed: 60,
      etaSeconds: 6,
      facing: "east",
    } as const;

    const { container, rerender } = render(
      <svg>
        <Figure
          character={someone({ action: "walk", mark: still(), journey })}
          at={{ x: 0, y: 0 }}
        />
      </svg>,
    );
    expect(container.querySelector(".figure--played")).not.toBeInTheDocument();

    rerender(
      <svg>
        <Figure
          character={someone({
            action: "walk",
            mark: still(),
            actions: { walk: walkSheet() },
            journey,
          })}
          at={{ x: 0, y: 0 }}
        />
      </svg>,
    );
    expect(container.querySelector(".figure--played")).toBeInTheDocument();
  });

  it("still shows a stand-in for somebody nobody has drawn", () => {
    const { container } = render(
      <svg>
        <Figure character={someone({ action: "work" })} at={{ x: 0, y: 0 }} />
      </svg>,
    );
    expect(container.querySelector(".figure__body--undeclared")).toBeInTheDocument();
  });
});
