/**
 * Visiting a Place (ADR-0022).
 *
 * Users learn exactly one interaction: **Visit**. What it *means* grows over time; the
 * interaction never changes, so nobody relearns it.
 *
 * Pure functions, no React, no DOM — the same rule the camera follows. Nothing here holds
 * truth: a visit is user intent, and everything else is derived from it plus the camera plus
 * what the Engine says a Place can actually offer.
 */

import type { Camera } from "./camera";

/** How close visiting brings you, in footprints of the visited Place. */
const VISIT_FOOTPRINTS = 3;

/**
 * How much a Place can truthfully offer right now.
 *
 * Today every Place answers the same: you can arrive, read what it is for, and see who is
 * there. All of that is true of every Place, so it is a constant rather than a field — a
 * value with one possible answer is not yet information (Earn Complexity).
 *
 * It stops being a constant when a capability can drive it: a configured provider makes its
 * Laboratory offer more than an unconfigured one, and *that* is the moment this becomes
 * derived from `Provider::health()` (ADR-0007, ADR-0021 addendum).
 */
export const AVAILABLE_DEPTH = 1;

/** Where the camera should sit to have arrived at a Place. */
export function visitSpan(footprint: number): number {
  return footprint * VISIT_FOOTPRINTS;
}

/**
 * How close the camera has actually come, as a level.
 *
 * Intent alone is not arrival. Asking to visit while the camera is still far away should not
 * reveal anything — you have not got there yet.
 */
export function proximityLevel(camera: Camera, footprint: number): number {
  return camera.span <= visitSpan(footprint) * 1.35 ? 1 : 0;
}

/**
 * **The rule the whole contract rests on.**
 *
 * ```
 * shown = min(proximity, available depth)
 * ```
 *
 * The Experience Layer can never reveal more than exists. Approaching a Place that has
 * nothing configured behind it reveals *that* — which is information, not an empty room.
 * With this formula, faking an interior is not discouraged; it is arithmetically impossible.
 */
export function revealLevel(
  visitedId: string | null,
  placeId: string,
  camera: Camera | null,
  footprint: number,
  /**
   * Whether the camera has stopped moving.
   *
   * Arrival is the moment travel **ends**, not the moment the distance happens to be short
   * enough. Without this the Place would begin opening halfway through the journey, and the
   * journey would never land — the beat between stopping and revealing is what tells someone
   * they got there.
   */
  settled: boolean,
): number {
  if (visitedId !== placeId || !camera || !settled) return 0;
  return Math.min(proximityLevel(camera, footprint), AVAILABLE_DEPTH);
}
