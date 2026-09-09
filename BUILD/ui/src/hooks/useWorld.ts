/**
 * React binding for the World contract.
 *
 * Holds no business logic — it only moves a contract into React state. The World renders
 * immediately with whatever is known, fills in when the engine answers, and then follows
 * the engine as presence changes.
 *
 * If it cannot follow, it says so. It never presents a frozen world as a live one.
 */

import { useCallback, useEffect, useState } from "react";

import { fetchWorld, watchWorld } from "../ipc/world";
import type { PresenceView, WorldSnapshot } from "../ipc/contracts";

/** The World before the engine has answered: renderable, and honest about being empty. */
const INITIAL: WorldSnapshot = {
  status: "live",
  view: { packName: null, map: null, places: [], characters: [] },
};

/**
 * Fold a movement update into the World already on screen.
 *
 * The movement channel carries only what moved, so this replaces four fields of the people it
 * names and leaves everybody — and everything — else exactly as it was. Somebody it names who is
 * not in the World is ignored rather than added: a person with no name and no face is not a
 * person, and the full projection is what introduces one.
 *
 * A new object every time, because React compares by identity and a mutated array would be a
 * World that changed without anything re-rendering.
 */
export function walked(current: WorldSnapshot, moved: readonly PresenceView[]): WorldSnapshot {
  if (moved.length === 0) return current;
  const byId = new Map(moved.map((who) => [who.id, who]));

  return {
    ...current,
    view: {
      ...current.view,
      characters: current.view.characters.map((character) => {
        const update = byId.get(character.id);
        if (!update) return character;
        return {
          ...character,
          place: update.place,
          activity: update.activity,
          class: update.class,
          journey: update.journey,
        };
      }),
    },
  };
}

export function useWorld(): WorldSnapshot & { readonly reread: () => void } {
  const [snapshot, setSnapshot] = useState<WorldSnapshot>(INITIAL);

  /**
   * Fetch the World again, now.
   *
   * The World Editor writes `places.toml` and the projection reads it — but the change comes
   * from *this* process rather than from presence advancing, so no `world:changed` is emitted
   * and the map on screen would keep describing the file as it was a moment ago.
   */
  const reread = useCallback(() => {
    void fetchWorld().then(setSnapshot);
  }, []);

  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;

    void fetchWorld().then((next) => {
      if (active) setSnapshot(next);
    });

    void watchWorld(
      (view) => {
        if (active) setSnapshot({ status: "live", view });
      },
      (moved) => {
        if (active) setSnapshot((current) => walked(current, moved));
      },
    ).then((watch) => {
      if (!active) {
        if (watch.following) watch.stop();
        return;
      }
      if (watch.following) {
        stop = watch.stop;
        return;
      }
      // We have a world but cannot follow it. Say that, rather than looking live.
      setSnapshot((current) =>
        current.status === "unavailable"
          ? current
          : { status: "static", view: current.view, note: watch.reason },
      );
    });

    return () => {
      active = false;
      stop?.();
    };
  }, []);

  return { ...snapshot, reread };
}
