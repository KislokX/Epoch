/**
 * The editor's title bar.
 *
 * ## There is no SAVE
 *
 * Every edit writes `places.toml` and re-reads it — the editor is an editor *over the vault*,
 * never a parallel store (ADR-0023). A SAVE button would imply unsaved work that does not
 * exist, and would teach the user that not pressing it risks losing something. The lamp beside
 * the World's name says the true thing instead: this is on disk.
 *
 * ## UNDO actually undoes
 *
 * The reference design's UNDO popped a label off a list and changed nothing. That is the worst
 * kind of instrument — one that appears to work — so it is wired to a real timeline of map
 * snapshots in the Engine, and both buttons go dim when there is genuinely nothing to take
 * back. The count comes from the Engine, not from how many times this screen was clicked.
 *
 * ## PLAY is the way out
 *
 * Entering the editor stopped the World. PLAY starts it again, which is exactly what the word
 * means here — and it is why the button belongs beside BRIDGE rather than being called "close".
 */

import { invoke } from "@tauri-apps/api/core";

import type { EditorApi } from "./useEditor";

export function TopBar({
  editor,
  worldName,
  onPlay,
  onBridge,
}: {
  readonly editor: EditorApi;
  readonly worldName: string;
  readonly onPlay: () => void;
  readonly onBridge: () => void;
}) {
  const [back, forward] = editor.depth;

  return (
    <header className="pxpanel wed__top">
      <div className="wed__brand">
        <div className="pxinset wed__mark">◆</div>
        <div>
          <div className="pxtitle">Epoch World Editor</div>
          <div className="wed__ver">the World is stopped</div>
        </div>
      </div>

      <div className="pxinset wed__world">
        <span className="pxlabel">World</span>
        <b>{worldName}</b>
        {/* Lit because the last write succeeded — never because a screen was rendered. */}
        <span className="wed__saved" title="Every edit is already written to your vault">
          ●
        </span>
      </div>

      <div className="wed__actions">
        <button
          type="button"
          className="pxbtn"
          disabled={back === 0}
          title={back === 0 ? "Nothing to take back" : `Take back: ${editor.history[0]}`}
          onClick={() => void editor.change("undo", () => invoke("undo_edit"))}
        >
          Undo
        </button>
        <button
          type="button"
          className="pxbtn"
          disabled={forward === 0}
          title={forward === 0 ? "Nothing to do again" : "Do it again"}
          onClick={() => void editor.change("redo", () => invoke("redo_edit"))}
        >
          Redo
        </button>
        <button
          type="button"
          className="pxbtn"
          disabled
          title="Editor settings are not built yet."
        >
          Settings
        </button>
        <button type="button" className="pxbtn" onClick={onPlay} title="Start the World again">
          ▶ Play
        </button>
        <button
          type="button"
          className="pxbtn pxbtn--gold"
          onClick={onBridge}
          title="Leave the World and return to the bridge"
        >
          Bridge
        </button>
      </div>
    </header>
  );
}
