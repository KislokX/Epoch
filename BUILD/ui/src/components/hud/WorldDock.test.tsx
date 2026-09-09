import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { WorldDock } from "./WorldDock";

const TOWER = {
  id: "tower",
  title: "Tower",
  subtitle: "Where the crew gathers.",
  concept: "command_center",
  isPlaceholder: false,
  placement: { x: 10, y: 20, footprint: 80, zOrder: 1 },
  marks: [],
  anchors: [],
} as const;

describe("the World dock", () => {
  it("says when the authored World has no Place to visit", () => {
    render(
      <WorldDock
        places={[]}
        visitedId={null}
        peopleAt={() => 0}
        glyphFor={() => "core"}
        onVisit={() => {}}
      />,
    );

    expect(screen.getByText("This World has nowhere to go yet.")).toBeInTheDocument();
  });

  it("visits the exact authored Place and shows its measured occupancy", async () => {
    const user = userEvent.setup();
    const onVisit = vi.fn();
    render(
      <WorldDock
        places={[TOWER]}
        visitedId="tower"
        peopleAt={(place) => (place.id === "tower" ? 2 : 0)}
        glyphFor={() => "star"}
        onVisit={onVisit}
      />,
    );

    const tower = screen.getByRole("button", { name: /Tower/ });
    expect(tower).toHaveClass("epbtn--on");
    expect(tower).toHaveTextContent("2");
    expect(tower).toHaveAttribute("title", "Where the crew gathers.");
    await user.click(tower);
    expect(onVisit).toHaveBeenCalledWith(TOWER);
  });

  it("keeps map visibility local to the layout controls", async () => {
    const user = userEvent.setup();
    const onLayers = vi.fn();
    render(
      <WorldDock
        places={[TOWER]}
        visitedId={null}
        peopleAt={() => 0}
        glyphFor={() => "star"}
        onVisit={() => {}}
        layers={{
          terrain: true,
          roads: true,
          buildings: true,
          characters: true,
          labels: true,
          grid: false,
        }}
        onLayers={onLayers}
      />,
    );

    const roads = screen.getByRole("button", { name: "roads" });
    expect(roads).toHaveAttribute("aria-pressed", "true");
    await user.click(roads);
    expect(onLayers).toHaveBeenCalledWith(expect.objectContaining({ roads: false }));
  });
});
