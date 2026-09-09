/**
 * The other half of the wire.
 *
 * `crates/epoch-engine/tests/contracts.rs` serialises each view with the real `Serialize` impl
 * and writes it to `__fixtures__/`. This reads those bytes back and asserts the **TypeScript
 * type accepts them** — which is the half that has never existed, and the reason a renamed Rust
 * field has always been discovered by a user seeing `undefined` in a panel.
 *
 * ## How it fails, and what that means
 *
 * Two ways, both useful:
 *
 * 1. **`tsc` fails.** The fixture has a field the type does not, or is missing one it requires.
 *    Somebody changed the wire and did not tell this side.
 * 2. **An assertion fails.** The shape is still assignable but a field the UI actually reads has
 *    moved or been renamed. `tsc` cannot catch that on its own, because a fixture parsed from
 *    JSON is `any` until something claims otherwise — so the claim is made here, explicitly.
 *
 * Neither means "the test is stale". Both mean the wire moved.
 */

import { describe, expect, it } from "vitest";

import world from "./__fixtures__/world_view.json";
import empty from "./__fixtures__/world_view_empty.json";
import type {
  Action,
  ActivityClass,
  Direction,
  FramesView,
  MarkView,
  WorldView,
} from "./contracts";

/**
 * Narrow the one thing a JSON import cannot type: a fixed-length array.
 *
 * `[f32; 2]` serialises as `[0.5, 1.0]`, and TypeScript infers `number[]` from a JSON module —
 * assignable to nothing that asks for `readonly [number, number]`. That is a limitation of
 * importing JSON, **not** a disagreement about the wire, and the fix belongs here rather than in
 * the type: widening `anchor` to `number[]` would weaken a real contract to make a test easier,
 * which is the wrong direction.
 *
 * Everything else stays structurally checked, which is the whole point of the file.
 */
function asMark(raw: (typeof world.places)[number]["marks"][number]): MarkView {
  const [x, y] = raw.anchor;
  if (x === undefined || y === undefined) {
    throw new Error("an anchor is two numbers; the wire sent fewer");
  }
  const frames = "frames" in raw && raw.frames ? asFrames(raw.frames) : undefined;
  return { ...raw, anchor: [x, y], frames };
}

/**
 * The cut, with its direction words checked rather than assumed — same reason `asAction` and
 * `asActivityClass` exist. A row labelled with a word TypeScript does not know would draw
 * somebody walking the wrong way, silently.
 */
function asFrames(raw: {
  columns: number;
  rows: number;
  count: number;
  milliseconds: number;
  directions: readonly string[];
}): FramesView {
  return { ...raw, directions: raw.directions.map(asDirection) };
}

function asDirection(raw: string): Direction {
  if (raw !== "north" && raw !== "east" && raw !== "south" && raw !== "west") {
    throw new Error(`the wire carries a direction TypeScript does not know: ${raw}`);
  }
  return raw;
}

/**
 * The union, checked rather than assumed.
 *
 * Rust sends `class` as a plain `&'static str` mapped from an `ActivityClass` enum; TypeScript
 * declares a closed union. They agree today, and nothing enforced it — so adding a third variant
 * on the Rust side would have produced a value no component handles, silently.
 *
 * Now it throws here instead, which is the whole reason to check a wire rather than trust it.
 */
function asActivityClass(raw: string): ActivityClass {
  if (raw !== "idle" && raw !== "work") {
    throw new Error(`the wire carries an activity class TypeScript does not know: ${raw}`);
  }
  return raw;
}

/**
 * The same guard for the action vocabulary, and for the same reason one exists for `class`.
 *
 * It has grown once already — `settle` and `talk` arrived with the beat that causes them — and
 * an unknown word must keep failing loudly here rather than reaching a renderer that would
 * silently draw nothing.
 */
const ACTIONS: readonly string[] = [
  "idle",
  "walk",
  "settle",
  "talk",
  "think",
  "work",
];

function asAction(raw: string): Action {
  if (!ACTIONS.includes(raw)) {
    throw new Error(`the wire carries an action TypeScript does not know: ${raw}`);
  }
  return raw as Action;
}

function asWorldView(raw: typeof world): WorldView {
  return {
    ...raw,
    places: raw.places.map((place) => ({ ...place, marks: place.marks.map(asMark) })),
    characters: raw.characters.map((who) => ({
      ...who,
      action: asAction(who.action),
      journey: { ...who.journey, facing: asDirection(who.journey.facing) },
      actions: Object.fromEntries(
        Object.entries(who.actions).map(([action, mark]) => [
          asAction(action),
          asMark(mark),
        ]),
      ),
      class: asActivityClass(who.class),
      mark: who.mark ? asMark(who.mark) : null,
      icon: who.icon ? asMark(who.icon) : null,
    })),
  };
}

describe("the IPC wire", () => {
  it("is what `WorldView` says it is", () => {
    // The conversion *is* the test: every field except the two-element arrays is checked
    // structurally, so a rename on the Rust side stops this file compiling and `npm run
    // typecheck` fails in CI before anything renders.
    const view = asWorldView(world);

    expect(view.packName).toBe("Default World");
    expect(view.map?.width).toBe(1920);
    expect(view.places).toHaveLength(1);
    expect(view.characters).toHaveLength(1);
  });

  it("carries the fields the World actually reads", () => {
    const view = asWorldView(world);
    // Not destructured: `noUncheckedIndexedAccess` is on, and it is right — an empty list is a
    // real possibility everywhere else in this codebase. Asserted rather than assumed.
    const place = view.places.at(0);
    const who = view.characters.at(0);
    if (!place || !who) throw new Error("the fixture must carry one of each");

    // Every one of these is read by a component. A rename that kept the type assignable — an
    // added optional, a widened union — would still break the screen, so they are named here.
    expect(place.id).toBe("tower");
    expect(place.title).toBe("Tower");
    expect(place.placement?.footprint).toBe(90);
    expect(place.marks[0]?.asset).toMatch(/^data:/);

    expect(who.id).toBe("mage");
    expect(who.place).toBe("tower");
    // `home` and `place` are separate facts on purpose: somebody who is out is genuinely
    // elsewhere, and both stay true (ADR-0018).
    expect(who.home).toBe("tower");
    // Reaching this line at all means the class survived `asActivityClass`, which is the real
    // assertion: a variant TypeScript does not know would have thrown on the way in.
    expect(who.class).toBe("idle");
  });

  it("delivers artwork as a `data:` URI and never a path", () => {
    const view = asWorldView(world);
    // ADR-0020: nothing the webview renders is fetched. A path here would mean the frontend had
    // been handed something it must go and read, which is the boundary that keeps it from
    // touching the filesystem at all.
    const assets = [
      ...view.places.flatMap((p) => p.marks.map((m) => m.asset)),
      ...view.characters.map((c) => c.mark?.asset),
    ].filter((a): a is string => typeof a === "string");

    expect(assets.length).toBeGreaterThan(0);
    for (const asset of assets) expect(asset.startsWith("data:")).toBe(true);
  });

  it("accepts a World with nothing in it", () => {
    // Valid, not a failure state: the projection is total and there is never a loading screen
    // instead of the World (Build From Life, rule 1).
    const nothing: WorldView = { ...empty, places: [], characters: [] };

    expect(nothing.packName).toBeNull();
    expect(nothing.map).toBeNull();
    expect(nothing.places).toHaveLength(0);
    expect(nothing.characters).toHaveLength(0);
  });
});
