/**
 * Binding the camera to a window and to the user's hands.
 *
 * Everything that *decides* lives in `camera.ts` as pure functions. This file only measures
 * the window, listens to input, and asks those functions what the camera becomes. Keeping the
 * decision out of React is what lets the Experience Layer stay testable without a screen.
 *
 * ## The camera is derived, not stored
 *
 * The only state here is **where the user has moved the camera to** — their intent. Everything
 * else is computed from (world + arrival + window) on every render. That is the Experience
 * Layer rule applied to its own first component: it derives, it does not hold truth.
 *
 * It is also why the first frame cannot be missed. The camera existing used to depend on an
 * effect firing after a ResizeObserver had reported, which is an ordering problem with several
 * ways to lose. Now there is nothing to fire: if the World has an extent, there is a camera.
 */

import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
} from "react";

import {
  type Camera,
  type Frame,
  type Viewport,
  type WorldExtent,
  between,
  clamp,
  frame as viewFrame,
  lookAt,
  travelDuration,
  pan,
  toWorld,
  toWorldDistance,
  zoomBy,
} from "./camera";

/**
 * How much of the World the first frame shows, measured in **footprints of the Place you
 * arrive at** rather than in world units.
 *
 * Derived rather than fixed for the same reason the widest zoom is: a World whose Places are
 * larger should open proportionally, instead of inheriting a number calibrated against ours.
 *
 * At 5.5, arriving at the largest Place puts it and its immediate neighbours on screen while
 * everything further away stays out there — which is the whole point. You are standing
 * somewhere inside a larger world, not looking at all of it.
 */
const ARRIVAL_FOOTPRINTS = 5.5;

/** How far one pan key moves, as a fraction of what is visible. */
const KEY_STEP = 0.12;

/** Pixels of movement below which a gesture was a click, not a pan. */
const DRAG_SLOP = 4;

/**
 * Used for the single render before the window has been measured.
 *
 * Not a fixed-viewport assumption: the observer corrects it on the same frame it reports, and
 * the camera is re-derived. It exists so that "not yet measured" can never mean "no World".
 */
const UNMEASURED: Viewport = { width: 16, height: 9 };

export interface CameraBinding {
  readonly camera: Camera | null;
  readonly viewBox: string | null;
  /** What is currently visible, in world units. What the minimap draws as "you are here". */
  readonly view: Frame | null;
  readonly containerRef: RefObject<HTMLDivElement | null>;
  readonly surfaceRef: RefObject<SVGSVGElement | null>;
  readonly isDragging: boolean;
  /**
   * False while the camera is on its way somewhere.
   *
   * Arrival is the moment movement **ends**, not the moment it begins. A Place may not start
   * revealing itself until the World is still again — otherwise it opens while the user is
   * still approaching, and the journey never lands.
   */
  readonly isSettled: boolean;
  /**
   * Whether the last gesture actually moved the World.
   *
   * A drag that ends over a Place still fires a click. Without this, panning past a building
   * would visit it — the World would decide the user went somewhere they never asked to go.
   */
  readonly wasDragged: () => boolean;
  /** Point the camera somewhere in the World. The primitive `Visit` builds on. */
  readonly lookAtWorld: (at: { x: number; y: number }, span?: number) => void;
  readonly onPointerDown: (event: ReactPointerEvent) => void;
  readonly onPointerMove: (event: ReactPointerEvent) => void;
  readonly onPointerUp: (event: ReactPointerEvent) => void;
}

export function useCamera(
  world: WorldExtent | null,
  /**
   * Where the World opens, and how big the Place there is. Null until the World has said
   * where its centre of gravity is.
   */
  arrival: { x: number; y: number; footprint: number } | null,
): CameraBinding {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const surfaceRef = useRef<SVGSVGElement | null>(null);

  const [measured, setMeasured] = useState<Viewport | null>(null);
  /** Where the user has taken the camera. The only truth this hook keeps. */
  const [moved, setMoved] = useState<Camera | null>(null);
  const [isDragging, setDragging] = useState(false);
  const dragFrom = useRef<{ x: number; y: number } | null>(null);
  /** Pixels travelled during the current gesture, to tell a pan from a click. */
  const travelled = useRef(0);
  /** The element holding the pointer, once the gesture has become a drag. */
  const captured = useRef<{ element: Element; pointerId: number } | null>(null);
  /** The animation frame driving a journey, if one is under way. */
  const frame = useRef<number | null>(null);
  const [travelling, setTravelling] = useState(false);

  // The window's size is the one pixel measurement the Experience Layer needs. Observed
  // rather than assumed: the World has no canonical size and neither does the window.
  useEffect(() => {
    const element = containerRef.current;
    if (!element) return;

    // Measure once, now. A ResizeObserver reports asynchronously, and a frame spent on the
    // fallback aspect is a frame where the viewBox does not match its container and the World
    // is letterboxed. Cheap, and it removes the only remaining timing assumption here.
    const box = element.getBoundingClientRect();
    if (box.width > 0 && box.height > 0) {
      setMeasured({ width: box.width, height: box.height });
    }

    const observer = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const box = entry.contentRect;
      if (box.width <= 0 || box.height <= 0) return;
      setMeasured((current) =>
        current && current.width === box.width && current.height === box.height
          ? current
          : { width: box.width, height: box.height },
      );
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const viewport = measured ?? UNMEASURED;

  /**
   * The camera, derived.
   *
   * Arrive already somewhere: the World is not hidden and then revealed, it is simply seen
   * from where the user is standing, with the rest of it out there (Build From Life 1).
   *
   * Clamping on read rather than on write is what makes a window resize safe — the camera
   * cannot be left outside the World waiting for the user to nudge it back in.
   */
  const camera = useMemo<Camera | null>(() => {
    if (!world) return null;
    const wanted: Camera = moved ?? {
      x: arrival?.x ?? world.width / 2,
      y: arrival?.y ?? world.height / 2,
      span: arrival ? arrival.footprint * ARRIVAL_FOOTPRINTS : world.height / 2,
    };
    return clamp(wanted, world, viewport);
  }, [world, arrival, moved, viewport]);

  // Handlers read the *effective* camera, which is derived. A ref keeps them from having to
  // reconstruct it, and keeps them out of the dependency churn of every pan.
  const current = useRef<Camera | null>(null);
  current.current = camera;

  /**
   * Stop travelling and stay wherever the journey had got to.
   *
   * Any direct input cancels: a camera that keeps flying while the user is dragging has taken
   * control away from them, and the World is not allowed to do that.
   */
  const cancelTravel = useCallback(() => {
    if (frame.current !== null) cancelAnimationFrame(frame.current);
    frame.current = null;
    setTravelling(false);
  }, []);

  /**
   * Travel to a point, rather than cut to it.
   *
   * The camera *begins* the visit; it is not the visit (ADR-0022). A cut relocates the World
   * around the user; a movement means the user went somewhere. Time is one of the three
   * inputs the Experience Layer is allowed to derive from, which is what makes this
   * playback rather than state.
   */
  const lookAtWorld = useCallback(
    (at: { x: number; y: number }, span?: number) => {
      const from = current.current;
      if (!from || !world) return;
      const to = lookAt(from, at, world, viewport, span ?? from.span);

      cancelTravel();
      const duration = travelDuration(from, to);
      const started = performance.now();
      setTravelling(true);

      const step = () => {
        const t = (performance.now() - started) / duration;
        if (t >= 1) {
          frame.current = null;
          setMoved(to);
          // Arrival is the moment movement ends. Only now may the Place begin to open.
          setTravelling(false);
          return;
        }
        setMoved(between(from, to, t));
        frame.current = requestAnimationFrame(step);
      };
      frame.current = requestAnimationFrame(step);
    },
    [world, viewport, cancelTravel],
  );

  useEffect(() => cancelTravel, [cancelTravel]);

  // Wheel is attached natively, not through React: React's wheel handler is passive, and a
  // passive listener cannot stop the page from scrolling underneath the World. It goes on the
  // stage rather than the svg, because the stage exists from the first render.
  useEffect(() => {
    const element = containerRef.current;
    if (!element || !world) return;

    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      cancelTravel();
      const now = current.current;
      if (!now) return;
      const box = element.getBoundingClientRect();
      const size: Viewport = { width: box.width, height: box.height };
      const here = toWorld(
        { x: event.clientX - box.left, y: event.clientY - box.top },
        now,
        size,
      );
      setMoved(zoomBy(now, Math.exp(event.deltaY * 0.0012), here, world, size));
    };

    element.addEventListener("wheel", onWheel, { passive: false });
    return () => element.removeEventListener("wheel", onWheel);
  }, [world, cancelTravel]);

  /*
    Panning by keyboard — **WASD, and no longer the arrows.**

    The arrows moved the camera from a listener on `window`, which is the widest possible claim
    on a key: it wins wherever the focus is and whatever the focused element wanted. The `/`
    menu could not use Up and Down at all — a component cannot out-argue a window listener by
    being polite — and the visible result was a menu that ignored the arrows while the World
    slid sideways behind it.

    Two keys cannot both own the arrows, so one of them has to give them up, and the interface
    is the one that needs them: a list has no other way to be walked, while a camera has drag,
    wheel, Visit and now these four letters. **Arrows belong to whatever is in front of you.**

    Kept on `window` rather than moved onto the surface, because panning with nothing focused is
    the ordinary case — and W/A/S/D are given up by any field that wants them simply by being
    typed into, which the guard below is.
  */
  useEffect(() => {
    if (!world) return;
    const steps: Record<string, [number, number]> = {
      a: [-1, 0],
      d: [1, 0],
      w: [0, -1],
      s: [0, 1],
    };
    const onKey = (event: KeyboardEvent) => {
      // Somebody is writing. A letter that pans the World while a sentence is being typed is
      // not a shortcut, it is a bug — and this is the whole cost of moving off the arrows.
      const focused = document.activeElement;
      if (
        focused instanceof HTMLTextAreaElement ||
        focused instanceof HTMLInputElement ||
        focused instanceof HTMLSelectElement ||
        (focused instanceof HTMLElement && focused.isContentEditable)
      ) {
        return;
      }
      // A modifier means a real shortcut somewhere else — Ctrl+S is not a step south.
      if (event.ctrlKey || event.metaKey || event.altKey) return;
      const step = steps[event.key.toLowerCase()];
      const now = current.current;
      if (!step || !now) return;
      event.preventDefault();
      cancelTravel();
      const distance = now.span * KEY_STEP;
      setMoved(
        pan(now, step[0] * distance, step[1] * distance, world, viewport),
      );
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [world, viewport, cancelTravel]);

  /**
   * Pointer capture is taken **late** - only once the gesture is really a drag.
   *
   * Capturing on pointerdown retargets pointerup to the capturing element, so the browser
   * dispatches `click` on the nearest common ancestor of the two targets: the surface itself.
   * A Place's own hit target never sees it, and clicking a building does nothing. Hover and
   * keyboard keep working, which is what made this look like a Visit bug rather than a
   * pointer bug.
   */
  const onPointerDown = useCallback(
    (event: ReactPointerEvent) => {
      if (event.button !== 0) return;
      cancelTravel();
      dragFrom.current = { x: event.clientX, y: event.clientY };
      travelled.current = 0;
      captured.current = null;
    },
    [cancelTravel],
  );

  const wasDragged = useCallback(() => travelled.current > DRAG_SLOP, []);

  const onPointerMove = useCallback(
    (event: ReactPointerEvent) => {
      const from = dragFrom.current;
      const now = current.current;
      if (!from || !now || !world) return;
      travelled.current +=
        Math.abs(event.clientX - from.x) + Math.abs(event.clientY - from.y);

      // Now that it is a drag, take the pointer so it cannot escape the surface mid-gesture.
      if (travelled.current > DRAG_SLOP && captured.current === null) {
        captured.current = {
          element: event.currentTarget,
          pointerId: event.pointerId,
        };
        event.currentTarget.setPointerCapture(event.pointerId);
        setDragging(true);
      }

      // Dragging moves the World with the hand, so the camera moves the other way.
      const dx = -toWorldDistance(event.clientX - from.x, now, viewport);
      const dy = -toWorldDistance(event.clientY - from.y, now, viewport);
      dragFrom.current = { x: event.clientX, y: event.clientY };
      setMoved(pan(now, dx, dy, world, viewport));
    },
    [world, viewport],
  );

  const onPointerUp = useCallback(() => {
    dragFrom.current = null;
    setDragging(false);
    const held = captured.current;
    captured.current = null;
    if (held && held.element.hasPointerCapture(held.pointerId)) {
      held.element.releasePointerCapture(held.pointerId);
    }
  }, []);

  const view = camera ? viewFrame(camera, viewport) : null;
  const viewBox = view
    ? `${view.x} ${view.y} ${view.width} ${view.height}`
    : null;

  return {
    camera,
    viewBox,
    view,
    containerRef,
    surfaceRef,
    isDragging,
    isSettled: !travelling,
    wasDragged,
    lookAtWorld,
    onPointerDown,
    onPointerMove,
    onPointerUp,
  };
}
