/**
 * What the editor is doing right now (ADR-0028).
 *
 * ## This holds intent, never truth
 *
 * Which tool is selected, what is selected, what the last edits were called. Everything the
 * *World* is — where buildings stand, who lives where, what joins what — is read back from the
 * Engine after every change and never mirrored here. That is ADR-0022's rule applied to the
 * editor: state in a presentation layer holds user intent only, everything else is computed on
 * read.
 *
 * The alternative is an editor that keeps its own copy of the map. It works until one write
 * fails, and from then on the screen and the vault quietly disagree about the user's World.
 *
 * ## Every action goes through `change`
 *
 * Run the command, re-read, record what it was called. A failure leaves the file and the screen
 * agreeing — because neither moved — and puts the Engine's own sentence on screen rather than a
 * generic one.
 */

import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export type EditMode =
  | "select"
  | "place"
  | "road"
  | "crew"
  | "terrain"
  | "look"
  | "quest"
  | "preview";

export type Selection =
  | { readonly kind: "none" }
  | { readonly kind: "place"; readonly id: string }
  | { readonly kind: "character"; readonly id: string }
  | { readonly kind: "road"; readonly id: string };

export interface EditorApi {
  readonly mode: EditMode;
  readonly setMode: (m: EditMode) => void;
  readonly selection: Selection;
  readonly select: (s: Selection) => void;
  /** What the last edits were called, newest first. Real actions, never a placeholder log. */
  readonly history: readonly string[];
  /** How many steps back and forward the Engine can actually take. Measured, not assumed. */
  readonly depth: readonly [number, number];
  /** The Engine's own sentence when something was refused, or `null`. */
  readonly failed: string | null;
  readonly clearFailure: () => void;
  /** Run a change, re-read the World, and remember what it was called. */
  readonly change: (label: string, run: () => Promise<unknown>) => Promise<boolean>;
  readonly refresh: () => Promise<void>;
}

export function useEditor(onChanged: () => void): EditorApi {
  const [mode, setMode] = useState<EditMode>("select");
  const [selection, setSelection] = useState<Selection>({ kind: "none" });
  const [history, setHistory] = useState<readonly string[]>(["world loaded"]);
  const [depth, setDepth] = useState<readonly [number, number]>([0, 0]);
  const [failed, setFailed] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setDepth(await invoke<[number, number]>("history_depth"));
    onChanged();
  }, [onChanged]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const change = useCallback(
    async (label: string, run: () => Promise<unknown>) => {
      setFailed(null);
      try {
        await run();
        await refresh();
        setHistory((h) => [label, ...h].slice(0, 40));
        return true;
      } catch (error) {
        // The Engine's sentence, not ours. It names who is working, or which building is
        // nowhere — something the user can act on.
        setFailed(String(error).replace(/^Error: /, ""));
        return false;
      }
    },
    [refresh],
  );

  return {
    mode,
    setMode,
    selection,
    select: setSelection,
    history,
    depth,
    failed,
    clearFailure: () => setFailed(null),
    change,
    refresh,
  };
}
