/**
 * Terminals the user opens.
 *
 * Deliberately its own module rather than part of the World's IPC: nothing here is a capability,
 * nothing a model can reach opens one, and keeping the boundary visible in the file layout is
 * cheaper than remembering it. See `epoch_engine::terminal`.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** One shell this machine actually has. Measured by the Engine, never a list kept here. */
export interface Shell {
  readonly id: string;
  readonly label: string;
  /** The resolved program. Shown, because a terminal should not be a mystery box. */
  readonly program: string;
  readonly args: readonly string[];
}

export async function shells(): Promise<readonly Shell[]> {
  try {
    return await invoke<Shell[]>("terminal_shells");
  } catch {
    return [];
  }
}

export async function openTerminal(
  id: string,
  shell: string,
  cols: number,
  rows: number,
): Promise<string | null> {
  try {
    await invoke("terminal_open", { id, shell, cols, rows });
    return null;
  } catch (error) {
    return String(error);
  }
}

/** Keystrokes, verbatim. Control characters included — a prompt expects them. */
export async function writeTerminal(id: string, keys: string): Promise<void> {
  try {
    await invoke("terminal_write", { id, keys });
  } catch {
    // A terminal that has ended stops listening, and the window is about to say so. Throwing
    // here would take the whole World down over a keystroke into a dead shell.
  }
}

export async function resizeTerminal(id: string, cols: number, rows: number): Promise<void> {
  try {
    await invoke("terminal_resize", { id, cols, rows });
  } catch {
    // Same: a resize is advice, and a terminal that has gone does not need it.
  }
}

/** What it printed before this window was looking. Reopening one must not be a blank square. */
export async function terminalScrollback(id: string): Promise<string | null> {
  try {
    return await invoke<string | null>("terminal_scrollback", { id });
  } catch {
    return null;
  }
}

/** End it, and the program with it. **Minimising is not this.** */
export async function closeTerminal(id: string): Promise<void> {
  try {
    await invoke("terminal_close", { id });
  } catch {
    // Already gone is the outcome that was asked for.
  }
}

/** Which are still running, so a remount rebuilds from the Engine rather than from memory. */
export async function openTerminalIds(): Promise<readonly string[]> {
  try {
    return await invoke<string[]>("terminal_open_ids");
  } catch {
    return [];
  }
}

/** One chunk of output: `[terminal id, text]`. */
export function onTerminalOutput(
  handle: (id: string, chunk: string) => void,
): Promise<() => void> {
  return listen<[string, string]>("terminal:output", (event) =>
    handle(event.payload[0], event.payload[1]),
  );
}
