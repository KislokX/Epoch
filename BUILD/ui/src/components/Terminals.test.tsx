/**
 * Terminals, and the one distinction that must never blur.
 *
 * A terminal is a **process**. Minimising its window puts the window away; only the X ends the
 * program. Getting that wrong in either direction is bad in a different way: a minimise that
 * killed the shell would lose a half-finished sign-in, and an X that only hid it would leave
 * something running that nobody can see, type into, or think to end.
 */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { Terminals } from "./Terminals";
import { askForTerminal } from "../experience/askForTerminal";

/**
 * `xterm` writes to a real canvas and measures the DOM, neither of which jsdom does. The
 * emulator is not what these assertions are about — the window chrome and what each control
 * does to the *process* are — so it is replaced by something that records nothing.
 */
vi.mock("@xterm/xterm", () => ({
  Terminal: class {
    cols = 80;
    rows = 24;
    loadAddon() {}
    open() {}
    write() {}
    dispose() {}
    clearSelection() {}
    hasSelection() {
      return false;
    }
    getSelection() {
      return "";
    }
    attachCustomKeyEventHandler() {}
    onData() {
      return { dispose() {} };
    }
  },
}));
vi.mock("@xterm/addon-fit", () => ({
  FitAddon: class {
    fit() {}
  },
}));
vi.mock("@xterm/addon-web-links", () => ({
  WebLinksAddon: class {},
}));
vi.mock("@xterm/xterm/css/xterm.css", () => ({}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

/**
 * jsdom has no `ResizeObserver`, and without it the window never rendered at all — the effect
 * threw on construction and took the component with it. Worth stubbing rather than removing the
 * observer: refitting on resize is what keeps a shell laying out its prompt for the size the
 * window actually is, including when it comes back from being minimised.
 */
beforeEach(() => {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
});

const SHELLS = [
  {
    id: "powershell",
    label: "Windows PowerShell",
    program: "C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
    args: [],
  },
  { id: "cmd", label: "Command Prompt", program: "C:\\WINDOWS\\System32\\cmd.exe", args: [] },
];

function engine(shells = SHELLS) {
  const calls: { command: string; args: unknown }[] = [];
  vi.mocked(invoke).mockImplementation((command: string, args: unknown) => {
    calls.push({ command, args });
    if (command === "terminal_shells") return Promise.resolve(shells);
    if (command === "terminal_open_ids") return Promise.resolve([]);
    if (command === "terminal_scrollback") return Promise.resolve(null);
    return Promise.resolve(undefined);
  });
  return calls;
}

async function openOne() {
  render(<Terminals />);
  await userEvent.click(screen.getByRole("button", { name: "Terminals" }));
  await waitFor(() => screen.getByText("Windows PowerShell"));
  await userEvent.click(screen.getByText("Windows PowerShell"));
  await waitFor(() => expect(screen.getByTitle(/Minimise/)).toBeInTheDocument());
}

describe("Terminals", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("offers only shells this machine has, and says which program each one is", async () => {
    // Measured, never listed. A menu offering PowerShell Core on a machine without it sends
    // somebody to debug a failure that is not where they are looking — and naming the resolved
    // program is what stops a terminal being a mystery box.
    engine();
    render(<Terminals />);
    await userEvent.click(screen.getByRole("button", { name: "Terminals" }));

    await waitFor(() => expect(screen.getByText("Windows PowerShell")).toBeInTheDocument());
    expect(screen.getByText("Command Prompt")).toBeInTheDocument();
    expect(screen.getByText(/powershell\.exe$/)).toBeInTheDocument();
    expect(screen.queryByText(/PowerShell Core/)).not.toBeInTheDocument();
  });

  it("says so plainly when there is no shell at all", async () => {
    engine([]);
    render(<Terminals />);
    await userEvent.click(screen.getByRole("button", { name: "Terminals" }));

    await waitFor(() =>
      expect(screen.getByText("No shell found on this machine.")).toBeInTheDocument(),
    );
  });

  it("minimising keeps the process running", async () => {
    // The whole point. A minimised terminal is still a shell somebody is halfway through using.
    const calls = engine();
    await openOne();

    await userEvent.click(screen.getByTitle(/Minimise/));

    expect(calls.some((c) => c.command === "terminal_close")).toBe(false);
    // And it waits somewhere it can be brought back from.
    expect(screen.getByRole("button", { name: "Windows PowerShell" })).toBeInTheDocument();
  });

  it("closing ends the program", async () => {
    const calls = engine();
    await openOne();

    await userEvent.click(screen.getByTitle(/Close — this ends the program/));

    await waitFor(() => {
      expect(calls.some((c) => c.command === "terminal_close")).toBe(true);
    });
    expect(screen.queryByTitle(/Minimise/)).not.toBeInTheDocument();
  });

  it("a minimised terminal can be ended without being put back on screen first", async () => {
    // Which is exactly the one somebody wants to end: it is out of the way and still running.
    const calls = engine();
    await openOne();
    await userEvent.click(screen.getByTitle(/Minimise/));

    const tray = screen.getByRole("button", { name: "Windows PowerShell" }).parentElement!;
    await userEvent.click(tray.querySelector(".term__x")!);

    await waitFor(() => {
      expect(calls.some((c) => c.command === "terminal_close")).toBe(true);
    });
  });

  it("a command a character proposed is typed, never run", async () => {
    // The security line, end to end. A character may propose; the proposal must not become a
    // shell running on one click of a button it also caused to appear. What Epoch removes is
    // having to find a terminal, not having to decide.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const calls = engine();
      render(<Terminals />);
      await waitFor(() => expect(calls.some((c) => c.command === "terminal_shells")).toBe(true));

      askForTerminal({ command: "npx -y @xavifabregat/spotify-mcp init", language: "bash" });

      await waitFor(() => expect(calls.some((c) => c.command === "terminal_open")).toBe(true));
      // The command lands after the shell has drawn a prompt.
      await vi.advanceTimersByTimeAsync(1200);

      const typed = calls.find((c) => c.command === "terminal_write");
      expect(typed).toBeDefined();
      const keys = (typed!.args as { keys: string }).keys;
      expect(keys).toBe("npx -y @xavifabregat/spotify-mcp init");
      expect(keys).not.toMatch(/[\r\n]/u);
    } finally {
      vi.useRealTimers();
    }
  });

  it("falls back to a shell this machine has when the suggested one is absent", async () => {
    // A command written as `bash` is still worth typing into PowerShell: the user reads it
    // before it runs, which is the whole arrangement. Refusing would be worse than adapting.
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const calls = engine([SHELLS[0]!]);
      render(<Terminals />);
      await waitFor(() => expect(calls.some((c) => c.command === "terminal_shells")).toBe(true));

      askForTerminal({ command: "ls -la", language: "bash" });

      const opened = await waitFor(() => {
        const found = calls.find((c) => c.command === "terminal_open");
        expect(found).toBeDefined();
        return found!;
      });
      expect((opened.args as { shell: string }).shell).toBe("powershell");
    } finally {
      vi.useRealTimers();
    }
  });

  it("opening two of the same shell is two terminals, not a name collision", async () => {
    // Two windows opened in the same millisecond — which is what a double-click is — shared one
    // id under a timestamp, and the Engine correctly refused the second. Found here.
    const calls = engine();
    const { container } = render(<Terminals />);
    for (const _ of [0, 1]) {
      await userEvent.click(screen.getByRole("button", { name: "Terminals" }));
      // From the menu specifically: an open window's title bar carries the same words.
      const entry = await waitFor(() => {
        const found = container.querySelector<HTMLElement>(".term__menu button");
        expect(found).not.toBeNull();
        return found!;
      });
      await userEvent.click(entry);
    }

    const opened = calls.filter((c) => c.command === "terminal_open");
    expect(opened).toHaveLength(2);
    const ids = opened.map((c) => (c.args as { id: string }).id);
    expect(new Set(ids).size).toBe(2);
  });

  it("a terminal window is not inside the HUD bar it was opened from", async () => {
    // **The z-index alone never did it.** This component mounts in the HUD bar, and a `Frame`
    // body is `position: relative; z-index: 1` \u2014 a stacking context. So the window\'s
    // `z-index: 1001` was 1001 *inside that frame*, and the frame lost to the conversation slot
    // at 2. Measured in the real window with `elementFromPoint`: at a point inside both, the hit
    // was `dlg__log`.
    //
    // Asserted structurally rather than by reading a number, because the number was right and
    // the answer was still wrong. What has to hold is that nothing between the window and the
    // document can trap it.
    engine();
    const { container } = render(<Terminals />);
    await userEvent.click(screen.getByRole("button", { name: "Terminals" }));
    const entry = await waitFor(() => {
      const found = container.querySelector<HTMLElement>(".term__menu button");
      expect(found).not.toBeNull();
      return found!;
    });
    await userEvent.click(entry);

    const win = await waitFor(() => {
      const found = document.querySelector<HTMLElement>(".term__win");
      expect(found).not.toBeNull();
      return found!;
    });
    expect(container.contains(win)).toBe(false);
    expect(win.parentElement).toBe(document.body);
  });
});
