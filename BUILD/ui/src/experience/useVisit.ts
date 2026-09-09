/**
 * Binding Visit to the user's hands.
 *
 * Holds one thing: which Place the user intends to be at. Everything else — how close the
 * camera is, what may be revealed, what is emphasised — is derived from that plus the camera
 * plus the Engine (ADR-0022).
 *
 * Interaction is centralised here rather than living in components, and everything is
 * addressed by `PlaceId`. That is what lets `Visit` become a Command on the Activity Stream
 * later without anything above it changing.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import type { Camera } from "./camera";
import { playSfx } from "./sfx";
import { visitSpan } from "./visit";

export interface VisitBinding {
  /** The Place the user is at, or null when they are looking at the World as a whole. */
  readonly visiting: string | null;
  /** Which Place is under the pointer. Presentation only. */
  readonly hovering: string | null;
  readonly visit: (place: { id: string; x: number; y: number; footprint: number }) => void;
  readonly leave: () => void;
  readonly hover: (placeId: string | null) => void;
}

export function useVisit(
  lookAtWorld: (at: { x: number; y: number }, span?: number) => void,
  camera: Camera | null,
): VisitBinding {
  const [visiting, setVisiting] = useState<string | null>(null);
  const [hovering, setHovering] = useState<string | null>(null);
  /** Where the user was looking from before they set off. */
  const origin = useRef<Camera | null>(null);

  const here = useRef<Camera | null>(null);
  here.current = camera;

  const visit = useCallback(
    (place: { id: string; x: number; y: number; footprint: number }) => {
      // Only the first departure is remembered. Walking from one Place to another keeps the
      // way back to where the exploring began, rather than to the last building.
      setVisiting((current) => {
        if (current === null) origin.current = here.current;
        return place.id;
      });
      // The camera *begins* the visit; it is not the visit. Everything the Place reveals
      // waits until the camera has actually arrived.
      lookAtWorld({ x: place.x, y: place.y }, visitSpan(place.footprint));
      playSfx("select");
    },
    [lookAtWorld],
  );

  /**
   * Leaving is the return journey, not a cut.
   *
   * The camera travels back the way it came, at the same pace — `travelDuration` is symmetric
   * in its two endpoints, so the walk out takes exactly as long as the walk in. Snapping back
   * would undo the arrival it took a journey to earn.
   */
  const leave = useCallback(() => {
    const back = origin.current;
    origin.current = null;
    setVisiting(null);
    if (back) lookAtWorld({ x: back.x, y: back.y }, back.span);
  }, [lookAtWorld]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") leave();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [leave]);

  return { visiting, hovering, visit, leave, hover: setHovering };
}
