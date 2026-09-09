/**
 * Who owns the arrow keys.
 *
 * The camera claimed them from a listener on `window`, which is the widest possible claim on a
 * key: it fires wherever the focus is, and a component cannot decline it by being polite. The
 * visible defect was a `/` menu that ignored Up and Down while the World slid sideways behind
 * it — and the same collision was available to every list Epoch will ever draw.
 *
 * These assert the settlement: **arrows belong to whatever is in front of you**, the camera
 * pans on W/A/S/D, and a letter never pans the World while somebody is writing a sentence.
 */

import { describe, expect, it, afterEach } from "vitest";
import { act, cleanup, renderHook } from "@testing-library/react";

import { useCamera } from "./useCamera";

// Big enough that the whole World does not fit on screen. With a small one the camera is
// clamped to showing everything, no key can move it, and every assertion below passes for a
// reason that has nothing to do with keys.
const WORLD = { width: 2000, height: 1200 };
const ARRIVAL = { x: 1000, y: 600, footprint: 4 };

function press(key: string) {
  act(() => {
    window.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true }));
  });
}

/** Mount the camera over a container the size of a window, as the World does. */
function camera() {
  const view = renderHook(() => useCamera(WORLD, ARRIVAL));
  return view;
}

afterEach(cleanup);

describe("panning by keyboard", () => {
  it("moves the World on D", () => {
    // The control. Without it the three assertions below all pass on a camera that never moves
    // for any key at all, and would go on passing after somebody deleted the handler.
    const view = camera();
    const before = view.result.current.camera;

    press("d");

    expect(view.result.current.camera).not.toEqual(before);
  });

  it("does not move the World with the arrow keys", () => {
    // The whole point. An arrow reaching the camera means it also reached past whatever list,
    // menu or dialogue the user was actually looking at.
    const view = camera();
    const before = view.result.current.camera;

    press("ArrowRight");
    press("ArrowDown");

    expect(view.result.current.camera).toEqual(before);
  });

  it("ignores a pan key while somebody is writing", () => {
    // The one cost of moving onto letters, and it must be paid here rather than by the user
    // discovering that typing "was" walks the World away from them.
    const view = camera();
    const field = document.createElement("textarea");
    document.body.append(field);
    field.focus();
    const before = view.result.current.camera;

    press("d");

    expect(view.result.current.camera).toEqual(before);
    field.remove();
  });

  it("ignores a pan key held with a modifier", () => {
    // Ctrl+S is not a step south.
    const view = camera();
    const before = view.result.current.camera;

    act(() => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "s", ctrlKey: true }),
      );
    });

    expect(view.result.current.camera).toEqual(before);
  });
});
