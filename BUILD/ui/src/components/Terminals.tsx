/**
 * Terminals, in the World.
 *
 * ## What this is for
 *
 * The thing Epoch cannot do on the user's behalf: **somebody else's sign-in.** An MCP server's
 * OAuth wizard asks for a Client ID and opens a browser; Epoch opens the door and never holds the
 * key. Before this existed, the answer to "that server needs re-authenticating" was *go and find
 * a terminal* — which is the moment the World stops being where the work happens.
 *
 * ## Minimising is not closing
 *
 * A terminal is a **process**. Putting its window away must never end it, and the only thing that
 * does is the X. So the two are drawn differently and say different things, and a minimised
 * window keeps its emulator mounted rather than being torn down and rebuilt — a rebuilt one
 * would lose whatever the shell printed while nobody was looking.
 *
 * ## The Engine owns which terminals exist
 *
 * This component holds **user intent only**: where a window sits, how big it is, whether it is
 * minimised. Which terminals are running is asked for on mount, so remounting rebuilds from
 * what is actually alive rather than from what this file remembers opening.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { WebLinksAddon } from "@xterm/addon-web-links";
import "@xterm/xterm/css/xterm.css";

import {
  closeTerminal,
  onTerminalOutput,
  openTerminal,
  openTerminalIds,
  resizeTerminal,
  shells,
  terminalScrollback,
  writeTerminal,
} from "../ipc/terminal";
import type { Shell } from "../ipc/terminal";
import { Frame } from "./hud/Frame";
import { PixelIcon } from "./hud/Pixel";
import { openLink } from "../ipc/world";
import { onTerminalAsked } from "../experience/askForTerminal";

/** Where a window sits and how it is shown. Intent, never derived state. */
interface Pane {
  readonly id: string;
  readonly label: string;
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
  readonly minimised: boolean;
  readonly maximised: boolean;
  /** Stacking, so clicking a window brings it forward. */
  readonly z: number;
}

const MIN_W = 380;
const MIN_H = 220;

/**
 * One emulator, bound to one terminal in the Engine.
 *
 * `xterm` is a real terminal emulator, and that is not decoration: the output of a pty is escape
 * sequences — colour, cursor movement, and the cursor-position *question* ConPTY asks before a
 * shell will start. Rendering the bytes as text would show a wizard's prompt as rubble, and
 * answering that question is something only an emulator does.
 */
function Screen({
  id,
  hidden,
  onEnded,
}: {
  readonly id: string;
  readonly hidden: boolean;
  readonly onEnded: () => void;
}) {
  const host = useRef<HTMLDivElement | null>(null);
  const term = useRef<Terminal | null>(null);
  const fit = useRef<FitAddon | null>(null);

  useEffect(() => {
    if (!host.current) return;
    const terminal = new Terminal({
      fontFamily: "var(--lx-mono), monospace",
      fontSize: 12,
      cursorBlink: true,
      // Reads as part of the World rather than as a white rectangle stapled onto it.
      theme: {
        background: "#0b0912",
        foreground: "#efe8da",
        cursor: "#e8b74a",
        selectionBackground: "#3a3358",
      },
      scrollback: 5000,
    });
    const fitter = new FitAddon();
    terminal.loadAddon(fitter);
    /*
      An address printed by a wizard is a thing to click. Through the Engine, never the WebView:
      it checks the scheme before anything opens, and these came out of somebody else's program.
    */
    terminal.loadAddon(new WebLinksAddon((_event, url) => void openLink(url)));
    terminal.open(host.current);
    term.current = terminal;
    fit.current = fitter;

    // Everything typed goes straight through. No interpretation here: a prompt expects control
    // characters, and a surface that filtered them would break the first wizard it met.
    const typed = terminal.onData((keys) => void writeTerminal(id, keys));

    /*
      **Copy and paste, the way the native terminal does it.**

      Ctrl+C is not copy at a prompt — it is *interrupt*, and taking that away would make a
      running command unstoppable. So the native rule applies: Ctrl+C copies only when there is a
      selection, and otherwise goes through as the interrupt. Ctrl+Shift+C and Ctrl+Shift+V are
      unambiguous and always do the obvious thing, and so does a right-click, which pastes if
      there is nothing selected and copies if there is.
    */
    terminal.attachCustomKeyEventHandler((event) => {
      if (event.type !== "keydown" || !event.ctrlKey) return true;
      const key = event.key.toLowerCase();
      if (key === "c" && (event.shiftKey || terminal.hasSelection())) {
        const chosen = terminal.getSelection();
        if (chosen) void navigator.clipboard?.writeText(chosen);
        // Without a selection, Ctrl+C stays the interrupt — only the shifted form is stolen.
        return !event.shiftKey ? !terminal.hasSelection() : false;
      }
      if (key === "v" && event.shiftKey) {
        void navigator.clipboard?.readText().then((text) => {
          if (text) void writeTerminal(id, text);
        });
        return false;
      }
      return true;
    });

    const menu = (event: MouseEvent) => {
      event.preventDefault();
      const chosen = terminal.getSelection();
      if (chosen) {
        void navigator.clipboard?.writeText(chosen);
        terminal.clearSelection();
        return;
      }
      void navigator.clipboard?.readText().then((text) => {
        if (text) void writeTerminal(id, text);
      });
    };
    host.current.addEventListener("contextmenu", menu);
    const hosted = host.current;

    let alive = true;
    void (async () => {
      // Whatever it printed before this window existed. A terminal reopened after a remount
      // starts from its history rather than from a blank square.
      const before = await terminalScrollback(id);
      if (!alive) return;
      if (before) terminal.write(before);
      fitter.fit();
      void resizeTerminal(id, terminal.cols, terminal.rows);
    })();

    return () => {
      alive = false;
      hosted.removeEventListener("contextmenu", menu);
      typed.dispose();
      terminal.dispose();
      term.current = null;
      fit.current = null;
    };
  }, [id]);

  // Output arrives for every terminal; each screen takes its own.
  useEffect(() => {
    let stop: (() => void) | null = null;
    let alive = true;
    void onTerminalOutput((which, chunk) => {
      if (which === id) term.current?.write(chunk);
    }).then((off) => {
      if (alive) stop = off;
      else off();
    });
    return () => {
      alive = false;
      stop?.();
    };
  }, [id]);

  // Refit whenever the window changes size — including when it comes back from being
  // minimised, where the element had no size to measure and the shell would otherwise keep
  // laying out its prompt for the size it last heard.
  useEffect(() => {
    if (hidden || !host.current) return;
    const refit = () => {
      try {
        fit.current?.fit();
        if (term.current) void resizeTerminal(id, term.current.cols, term.current.rows);
      } catch {
        // A window mid-drag can be zero-sized for a frame. Not a failure, and not worth a message.
      }
    };
    refit();
    const observer = new ResizeObserver(refit);
    observer.observe(host.current);
    return () => observer.disconnect();
  }, [id, hidden]);

  // A shell that exits on its own — `exit`, or a crash — should not leave a window pretending.
  useEffect(() => {
    const check = window.setInterval(async () => {
      const alive = await openTerminalIds();
      if (!alive.includes(id)) onEnded();
    }, 4000);
    return () => window.clearInterval(check);
  }, [id, onEnded]);

  return <div className="term__screen" ref={host} />;
}

export function Terminals() {
  const [menu, setMenu] = useState(false);
  const [available, setAvailable] = useState<readonly Shell[]>([]);
  const [panes, setPanes] = useState<readonly Pane[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  // Terminals sit above **everything**, the conversation included. One is opened because
  // somebody is about to do something in it, and a window that can end up behind another is a
  // window they have to go and find — which is the friction this whole feature removes.
  //
  // **The number alone never did it, and the windows are portalled for that reason.** Measured
  // in the window with `elementFromPoint`, which is the browser's own answer to the stacking
  // question: at a point inside both, the hit was `dlg__log`. The conversation was on top with
  // the terminal reading `z-index: 1001`, because this component mounts inside the HUD bar and
  // `.frame__body` is `position: relative; z-index: 1` — a stacking context. 1001 was 1001
  // *within that frame*, and the frame lost to `.hud__dialogue-slot` at 2.
  //
  // A z-index is a claim about siblings, never about the page. Asserting a global property from
  // a local number is the same mistake as a percentage resolving against the wrong box.
  const top = useRef(1000);
  /** How many have been opened this session, so two ids can never be the same. */
  const made = useRef(0);
  const drag = useRef<{ id: string; kind: "move" | "size"; x: number; y: number } | null>(null);

  useEffect(() => {
    void shells().then(setAvailable);
    // Rebuilt from what is actually running, not from what this file remembers opening.
    void openTerminalIds().then((ids) => {
      setPanes((held) => {
        const known = new Set(held.map((p) => p.id));
        const recovered = ids
          .filter((id) => !known.has(id))
          .map((id, at) => makePane(id, id.split("·")[0] ?? "Terminal", at));
        return recovered.length > 0 ? [...held, ...recovered] : held;
      });
    });
  }, []);

  /**
   * A new window opens in the middle of the screen, stepped so the next one is not hidden
   * exactly behind it.
   *
   * It used to open at a fixed corner, which put it over the crew panel on every screen size
   * and under nothing anybody was looking at. Centred is where somebody who just asked for a
   * terminal is looking.
   */
  function makePane(id: string, label: string, at: number): Pane {
    top.current += 1;
    const w = Math.min(820, Math.max(MIN_W, window.innerWidth - 160));
    const h = Math.min(460, Math.max(MIN_H, window.innerHeight - 220));
    const step = (at % 5) * 26;
    return {
      id,
      label,
      x: Math.max(12, Math.round((window.innerWidth - w) / 2) + step),
      y: Math.max(12, Math.round((window.innerHeight - h) / 2) - 40 + step),
      w,
      h,
      minimised: false,
      maximised: false,
      z: top.current,
    };
  }

  /**
   * A character proposed a command. Open a terminal and **type it**, without running it.
   *
   * The shell it suggested is a hint: the deck picks one this machine has, preferring a match,
   * and falls back rather than refusing. A command written as `bash` is still worth typing into
   * PowerShell — the user can read it before it runs, which is the whole arrangement.
   */
  useEffect(
    () =>
      onTerminalAsked(({ command, language }) => {
        void (async () => {
          const have = available.length > 0 ? available : await shells();
          if (have.length === 0) {
            setProblem("There is no shell on this machine to open.");
            return;
          }
          const wanted = (language ?? "").toLowerCase();
          const suited =
            have.find((s) => s.id === wanted) ??
            have.find((s) => wanted.startsWith("p") && s.id.includes("powershell")) ??
            have.find((s) => ["bash", "sh", "shell", "zsh"].includes(wanted) && s.id === "git_bash") ??
            have[0]!;
          // No trailing newline. Pressing Enter is the user's, and that is the line this
          // feature is built around rather than a detail of it.
          await start(suited, command);
        })();
      }),
    // `available` is read through a fresh fetch when it is empty, so this subscribes once and
    // never resubscribes mid-conversation.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [],
  );

  async function start(shell: Shell, typed?: string) {
    setMenu(false);
    setProblem(null);
    // Unique per window, so opening two of the same shell is two terminals rather than a name
    // collision the Engine has to refuse.
    //
    // A counter rather than a timestamp: the first version used `Date.now()`, and two windows
    // opened in the same millisecond — which is what a double-click is — got one id and the
    // second was rejected. Found by a test doing exactly that.
    made.current += 1;
    const id = `${shell.id}·${made.current}·${Date.now().toString(36)}`;
    const failure = await openTerminal(id, shell.id, 100, 28);
    if (failure) {
      setProblem(failure);
      return;
    }
    setPanes((held) => [...held, makePane(id, shell.label, held.length)]);

    if (typed) {
      // After the shell has drawn a prompt, or the characters land before there is anywhere to
      // put them and the line comes out mangled. A short wait rather than a handshake: the
      // worst case is the text appearing a moment later, and there is no correctness in it.
      window.setTimeout(() => void writeTerminal(id, typed), 900);
    }
  }

  const change = useCallback((id: string, into: Partial<Pane>) => {
    setPanes((held) => held.map((p) => (p.id === id ? { ...p, ...into } : p)));
  }, []);

  const raise = useCallback((id: string) => {
    top.current += 1;
    const z = top.current;
    setPanes((held) => held.map((p) => (p.id === id ? { ...p, z } : p)));
  }, []);

  const end = useCallback(async (id: string) => {
    await closeTerminal(id);
    setPanes((held) => held.filter((p) => p.id !== id));
  }, []);

  // Dragging and resizing are tracked on the window rather than per pane, so a pointer that
  // leaves a small header still moves the window it grabbed.
  useEffect(() => {
    const move = (e: PointerEvent) => {
      const held = drag.current;
      if (!held) return;
      const dx = e.clientX - held.x;
      const dy = e.clientY - held.y;
      drag.current = { ...held, x: e.clientX, y: e.clientY };
      setPanes((all) =>
        all.map((p) => {
          if (p.id !== held.id) return p;
          return held.kind === "move"
            ? { ...p, x: Math.max(0, p.x + dx), y: Math.max(0, p.y + dy) }
            : { ...p, w: Math.max(MIN_W, p.w + dx), h: Math.max(MIN_H, p.h + dy) };
        }),
      );
    };
    const stop = () => {
      drag.current = null;
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
    };
  }, []);

  const minimised = panes.filter((p) => p.minimised);
  const trayTabs = minimised.map((pane) => (
    <span key={pane.id} className="term__tab">
      {/*
        The same button as Travel and the top bar, because it sits on the same World bar and does
        the same kind of thing. Clicking it brings the window back; the X ends the program — both
        are needed here, since a minimised terminal is exactly the one somebody wants to end
        without first putting it back on screen.
      */}
      <button
        type="button"
        className="epbtn"
        onClick={() => {
          change(pane.id, { minimised: false });
          raise(pane.id);
        }}
      >
        <PixelIcon glyph="core" size={14} tone="parchment" />
        <span>{pane.label}</span>
      </button>
      <button
        type="button"
        className="epbtn term__x"
        title="Close — this ends the program"
        onClick={() => void end(pane.id)}
      >
        ×
      </button>
    </span>
  ));

  return (
    <>
      <div className="term__launch">
        <button
          type="button"
          className={`epbtn${menu ? " epbtn--primary" : ""}`}
          onClick={() => setMenu(!menu)}
        >
          Terminals
        </button>
        {menu && (
          <div className="term__menu">
            {/*
              Measured, never listed. A menu offering PowerShell Core on a machine without it
              sends somebody to debug a failure that is not where they are looking.
            */}
            {available.map((shell) => (
              <button key={shell.id} type="button" onClick={() => void start(shell)}>
                <b>{shell.label}</b>
                <em>{shell.program}</em>
              </button>
            ))}
            {available.length === 0 && <p className="term__none">No shell found on this machine.</p>}
          </div>
        )}
      </div>

      {problem && <p className="term__problem">{problem}</p>}

      {/*
        Rendered at the document root rather than here. The button belongs in the HUD bar; the
        windows belong over the World, and being a descendant of the bar is what stopped them
        getting there.
      */}
      {createPortal(
        <>
          {panes.map((pane) => (
        <Frame
          key={pane.id}
          corner={10}
          fill="var(--ep-window)"
          className={`term__win${pane.minimised ? " term__win--away" : ""}`}
          style={
            pane.maximised
              ? { left: 8, top: 8, right: 8, bottom: 64, width: "auto", height: "auto", zIndex: pane.z }
              : { left: pane.x, top: pane.y, width: pane.w, height: pane.h, zIndex: pane.z }
          }
          onPointerDown={() => raise(pane.id)}
          aria-hidden={pane.minimised}
        >
          <header
            className="term__bar"
            onPointerDown={(e) => {
              if (pane.maximised) return;
              drag.current = { id: pane.id, kind: "move", x: e.clientX, y: e.clientY };
            }}
          >
            <span className="term__title">{pane.label}</span>
            <span className="term__buttons">
              {/*
                Three, and the difference between them is the point. Minimise puts the window
                away and the process keeps running; maximise is presentation; the X ends the
                program. Labelled so that is readable rather than inferred from an icon.
              */}
              <button
                type="button"
                title="Minimise — the program keeps running"
                onClick={() => change(pane.id, { minimised: true })}
              >
                –
              </button>
              <button
                type="button"
                title={pane.maximised ? "Restore" : "Maximise"}
                onClick={() => change(pane.id, { maximised: !pane.maximised })}
              >
                ▢
              </button>
              <button
                type="button"
                className="term__x"
                title="Close — this ends the program"
                onClick={() => void end(pane.id)}
              >
                ×
              </button>
            </span>
          </header>
          <Screen
            id={pane.id}
            hidden={pane.minimised}
            onEnded={() => setPanes((held) => held.filter((p) => p.id !== pane.id))}
          />
          {!pane.maximised && (
            <span
              className="term__grip"
              onPointerDown={(e) => {
                e.stopPropagation();
                drag.current = { id: pane.id, kind: "size", x: e.clientX, y: e.clientY };
              }}
            />
          )}
            </Frame>
          ))}
        </>,
        document.body,
      )}

      {minimised.length > 0 && <Tray>{trayTabs}</Tray>}
    </>
  );
}

/**
 * Where a minimised terminal waits: **on the World's own bottom bar**, beside the places you can
 * travel to.
 *
 * A portal rather than a prop threaded through the dock. The dock's job is navigation and it has
 * no business knowing terminals exist; this is the deck putting its own tabs where the World's
 * bar is. If the bar is not there — the Launcher, a test — the tabs fall back to the corner
 * rather than disappearing, because a running process must always be reachable.
 */
function Tray({ children }: { readonly children: ReactNode }) {
  const [bar, setBar] = useState<Element | null>(null);
  useEffect(() => {
    setBar(document.querySelector(".hud__dock-body"));
  }, []);
  const tabs = <div className="term__tray">{children}</div>;
  return bar ? createPortal(tabs, bar) : <div className="term__tray term__tray--loose">{tabs}</div>;
}
