/**
 * Folding a movement update into the World already on screen.
 *
 * The movement channel exists so a walk does not re-send every Place, mark and face ten times.
 * That saving is only safe if the fold is exact — anything it drops silently becomes a World
 * that disagrees with the Engine until the next full projection happens to arrive.
 */

import { describe, expect, it } from "vitest";

import { walked } from "./useWorld";
import type { CharacterView, PresenceView, WorldSnapshot } from "../ipc/contracts";

function somebody(id: string, overrides: Partial<CharacterView> = {}): CharacterView {
  return {
    id,
    name: id,
    archetype: "researcher",
    home: "tower",
    place: "tower",
    activity: "reading",
    action: "idle", actions: {}, class: "idle",
    mark: null,
    icon: null,
    speaksWith: null,
    soundsLike: null,
    routine: [],
    ...overrides,
  };
}

function world(characters: CharacterView[]): WorldSnapshot {
  return {
    status: "live",
    view: {
      packName: "Default World",
      map: null,
      places: [],
      characters,
    },
  };
}

const walking: PresenceView = {
  id: "mage",
  place: "tower",
  activity: "walking to The Library",
  action: "idle", class: "idle",
  journey: { from: "tower", to: "library", progress: 0.3, speed: 60, facing: "east" as const, etaSeconds: 7 },
};

describe("folding movement into the World", () => {
  it("updates the people it names and touches nobody else", () => {
    const before = world([somebody("mage"), somebody("paladin")]);
    const after = walked(before, [walking]);

    const [mage, paladin] = after.view.characters;
    expect(mage?.journey?.to).toBe("library");
    expect(mage?.activity).toBe("walking to The Library");
    // Everything the movement channel does not carry survives: a face is not re-sent ten times
    // a walk, which is the whole reason this channel exists.
    expect(mage?.name).toBe("mage");
    expect(mage?.home).toBe("tower");
    expect(paladin).toBe(before.view.characters[1]);
  });

  it("ignores somebody the World has never heard of", () => {
    // A person with no name and no face is not a person. Introductions are the full
    // projection's job; this channel only moves people who already exist.
    const before = world([somebody("mage")]);
    const after = walked(before, [{ ...walking, id: "stranger" }]);

    expect(after.view.characters).toHaveLength(1);
    expect(after.view.characters[0]?.id).toBe("mage");
  });

  it("clears the journey when somebody stops walking", () => {
    // The update carries no `journey` on arrival, and the field has to actually go away — a
    // stale one would leave a figure walking a road they finished.
    const arrived = world([somebody("mage", { journey: walking.journey })]);
    const after = walked(arrived, [
      { id: "mage", place: "library", activity: "reading", action: "idle", class: "idle" },
    ]);

    expect(after.view.characters[0]?.journey).toBeUndefined();
    expect(after.view.characters[0]?.place).toBe("library");
  });

  it("returns the same World when nothing moved", () => {
    // Identity, not equality: React re-renders on a new object, and a World that changed
    // without changing is a frame nobody needed.
    const before = world([somebody("mage")]);
    expect(walked(before, [])).toBe(before);
  });
});
