/**
 * The application root — and the boundary between two Experience Surfaces.
 *
 * The Launcher prepares; the World immerses (`PRODUCT_ARCHITECTURE.md`). They are not a
 * screen and a sub-screen: they are two ways of experiencing one Engine, and this is the
 * only place that knows both exist.
 *
 * One window, deliberately. Crossing into a World should feel like crossing a boundary
 * (EXPERIENCE_CONSTITUTION XXI), and a second window would make it feel like launching
 * another application instead.
 */

import { useState } from "react";

import { Boot } from "../components/Boot";
import type { StartupResult } from "../experience/startup";
import { LauncherScreen } from "../screens/LauncherScreen";
import { WorldScreen } from "../screens/WorldScreen";
import { leaveWorld } from "../ipc/launcher";
import { useWindowTitle } from "../experience/useWindowTitle";

export function App() {
  /**
   * The World being experienced, or null while preparing.
   *
   * Held as identity **and** name: the id is what everything addresses, the name is only for
   * the title bar. Keeping them together here means no other component has to look one up
   * from the other.
   */
  const [entered, setEntered] = useState<{
    id: string;
    name: string;
    /**
     * Whether to open in the World Editor rather than in the World.
     *
     * True only for a World that was just made. It is empty — no land, no buildings, nobody —
     * so dropping the user into it would be dropping them into nothing. Creating a World is
     * something you do in order to make something *in* it, so that is where they arrive.
     */
    authoring?: boolean;
  } | null>(null);

  /**
   * Whether the cold start is still running.
   *
   * Here rather than in the Launcher because `App` mounts exactly once: leaving a World returns
   * to a bridge that is already awake, and replaying the sequence there would turn a boundary
   * into a toll. `Departure` already owns the boundary that does repeat.
   */
  const [booting, setBooting] = useState(true);
  /**
   * What the cold start already read, handed straight to the bridge.
   *
   * `null` when the sequence was skipped before it finished, and the Launcher then asks for
   * itself exactly as it did before any of this existed. That fallback is what keeps the
   * sequence presentation rather than a step the application depends on.
   */
  const [startup, setStartup] = useState<StartupResult | null>(null);

  // In the Launcher you are in Epoch; inside a World you are in that World.
  useWindowTitle(entered ? entered.name : "Epoch");

  // The bridge is not mounted until the start sequence hands over. That is the point: the work
  // it does is the work the Launcher would otherwise do while the user watched panels fill in.
  if (booting) {
    return (
      <Boot
        onDone={(read) => {
          setStartup(read);
          setBooting(false);
        }}
      />
    );
  }

  if (!entered) return <LauncherScreen onEntered={setEntered} startup={startup} />;

  return (
    <WorldScreen
      // Remounts on the World's identity, so entering a different World starts a genuinely
      // new experience rather than inheriting the last one's camera and visit.
      key={entered.id}
      authoring={entered.authoring ?? false}
      onLeave={() => {
        void leaveWorld();
        setEntered(null);
      }}
    />
  );
}
