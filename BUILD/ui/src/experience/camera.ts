/**
 * The camera — where the World is being looked at from.
 *
 * Part of the **Experience Layer**: it derives entirely from the World's extent, the size of
 * the window and what the user did. It is not a source of truth about anything, and it never
 * invents a fact — it only decides how much of a real World is currently in view.
 *
 * Pure functions, no React, no DOM. That is deliberate: the camera is the first piece of the
 * Experience Layer, and the layer must stay testable without a screen.
 *
 * ## The rule this file exists to enforce
 *
 * **The camera moves. The World does not.** The visible area is a window into a World that
 * has no canonical size: a small installation is a village, a large one is a city. Nothing
 * here may assume a fixed world size, a fixed viewport or a fixed number of Places.
 *
 * Everything is in **world units** except `Viewport`, which is pixels — the one place the two
 * spaces meet.
 */

/** Where the camera is looking, and how much it can see. */
export interface Camera {
  /** Centre of view, in world units. */
  readonly x: number;
  readonly y: number;
  /** How many world units are visible vertically. Smaller means closer. */
  readonly span: number;
}

/** How big the World is, in world units. Authored; never assumed. */
export interface WorldExtent {
  readonly width: number;
  readonly height: number;
}

/** How big the window is, in pixels. The only pixel measurement in the Experience Layer. */
export interface Viewport {
  readonly width: number;
  readonly height: number;
}

/** An SVG `viewBox`, in world units. */
export interface Frame {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
}

/**
 * Closest the camera may get, in world units of visible height.
 *
 * Roughly four footprints across, which is close enough to read a Place and still see that it
 * stands somewhere. Not a world-size assumption: it is a limit on proximity, and it holds for
 * a village and for a city alike.
 */
export const CLOSEST_SPAN = 520;

function aspect(viewport: Viewport): number {
  return viewport.height > 0 ? viewport.width / viewport.height : 1;
}

/**
 * Furthest the camera may get: exactly far enough to hold the whole World.
 *
 * Derived from the World rather than fixed, so a bigger World simply zooms out further.
 */
export function widestSpan(world: WorldExtent, viewport: Viewport): number {
  return Math.max(world.height, world.width / aspect(viewport));
}

/** The `viewBox` this camera looks through. */
export function frame(camera: Camera, viewport: Viewport): Frame {
  const height = camera.span;
  const width = camera.span * aspect(viewport);
  return { x: camera.x - width / 2, y: camera.y - height / 2, width, height };
}

/**
 * Keep the camera inside the World and within its proximity limits.
 *
 * When the view is wider than the World on an axis, the World is centred on that axis rather
 * than pinned to an edge: a world smaller than the window should sit in the middle of it, not
 * in a corner.
 */
export function clamp(camera: Camera, world: WorldExtent, viewport: Viewport): Camera {
  const widest = widestSpan(world, viewport);
  const span = Math.min(Math.max(camera.span, CLOSEST_SPAN), widest);

  const half = { x: (span * aspect(viewport)) / 2, y: span / 2 };
  const axis = (value: number, halfSize: number, extent: number) =>
    halfSize * 2 >= extent
      ? extent / 2
      : Math.min(Math.max(value, halfSize), extent - halfSize);

  return {
    x: axis(camera.x, half.x, world.width),
    y: axis(camera.y, half.y, world.height),
    span,
  };
}

/** Move the camera by a distance in **world units**. */
export function pan(
  camera: Camera,
  dx: number,
  dy: number,
  world: WorldExtent,
  viewport: Viewport,
): Camera {
  return clamp({ ...camera, x: camera.x + dx, y: camera.y + dy }, world, viewport);
}

/**
 * Zoom by a factor, keeping a fixed world point under the same place on screen.
 *
 * `anchor` is where the user is pointing, in world units. Zooming toward the cursor rather
 * than the centre is what makes moving through a world feel like looking rather than
 * operating a control.
 */
export function zoomBy(
  camera: Camera,
  factor: number,
  anchor: { x: number; y: number },
  world: WorldExtent,
  viewport: Viewport,
): Camera {
  const wanted = clamp({ ...camera, span: camera.span * factor }, world, viewport);
  // How much the span actually changed after clamping — the anchor must follow reality, not
  // the request, or the World drifts under the cursor at the limits.
  const ratio = wanted.span / camera.span;
  return clamp(
    {
      x: anchor.x + (camera.x - anchor.x) * ratio,
      y: anchor.y + (camera.y - anchor.y) * ratio,
      span: wanted.span,
    },
    world,
    viewport,
  );
}

/** Point the camera at somewhere in the World, optionally changing how close it is. */
export function lookAt(
  camera: Camera,
  at: { x: number; y: number },
  world: WorldExtent,
  viewport: Viewport,
  span = camera.span,
): Camera {
  return clamp({ x: at.x, y: at.y, span }, world, viewport);
}

/**
 * How long travelling between two camera positions should take, in milliseconds.
 *
 * Proportional to the distance covered **measured in screenfuls**, not a constant: crossing
 * the World should not take as long as stepping to the building next door, and neither should
 * feel instant. The point is that the movement reads as travel rather than relocation.
 */
export function travelDuration(from: Camera, to: Camera): number {
  const reach = Math.max(to.span, 1);
  const screens =
    Math.hypot(to.x - from.x, to.y - from.y) / reach + Math.abs(Math.log(to.span / from.span));
  return Math.min(900, Math.max(300, 300 + screens * 260));
}

/** Ease in, ease out. Sets off, travels, settles — rather than snapping at either end. */
function ease(t: number): number {
  return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2;
}

/** A camera part-way between two others. `t` runs 0..1. */
export function between(from: Camera, to: Camera, t: number): Camera {
  const k = ease(Math.min(Math.max(t, 0), 1));
  return {
    x: from.x + (to.x - from.x) * k,
    y: from.y + (to.y - from.y) * k,
    // Zoom interpolates geometrically: halving twice should feel like two equal steps.
    span: from.span * Math.pow(to.span / from.span, k),
  };
}

/** Turn a point on screen into a point in the World. */
export function toWorld(
  point: { x: number; y: number },
  camera: Camera,
  viewport: Viewport,
): { x: number; y: number } {
  const box = frame(camera, viewport);
  return {
    x: box.x + (point.x / Math.max(viewport.width, 1)) * box.width,
    y: box.y + (point.y / Math.max(viewport.height, 1)) * box.height,
  };
}

/** Turn a distance on screen into a distance in the World. */
export function toWorldDistance(pixels: number, camera: Camera, viewport: Viewport): number {
  return pixels * (camera.span / Math.max(viewport.height, 1));
}
