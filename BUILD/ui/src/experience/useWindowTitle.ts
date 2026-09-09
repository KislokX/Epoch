/**
 * The window's title, which is part of the experience rather than chrome.
 *
 * In the Launcher you are in Epoch. Inside a World you are in *that World* — the title bar is
 * the one piece of the operating system that says where you are, and leaving it stuck on a
 * generic name is a small immersion leak of exactly the kind we fix before adding features.
 *
 * Fails silently: a title that cannot be set is never a reason to stop rendering.
 */

import { useEffect } from "react";

import { getCurrentWindow } from "@tauri-apps/api/window";

export function useWindowTitle(title: string): void {
  useEffect(() => {
    void (async () => {
      try {
        await getCurrentWindow().setTitle(title);
      } catch {
        // Running outside the shell, or the permission is absent. The title is not load-bearing.
      }
    })();
  }, [title]);
}
