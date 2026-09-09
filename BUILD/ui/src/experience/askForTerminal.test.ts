/**
 * What a character is allowed to put a "run this" button under.
 *
 * The line these assertions hold is the security one: a character may **propose** a command, and
 * the proposal must stay small enough to read at a glance and obvious enough that the button
 * cannot appear under something that is not a command at all. Pressing Enter is always the
 * user's — nothing here produces a newline.
 */

import { describe, expect, it, vi } from "vitest";

import {
  askForTerminal,
  isShellLanguage,
  onTerminalAsked,
  runnableCommand,
} from "./askForTerminal";

describe("what may be offered to a terminal", () => {
  it("offers shell fences and nothing else", () => {
    for (const language of ["bash", "sh", "shell", "powershell", "pwsh", "cmd", "bat", "zsh"]) {
      expect(isShellLanguage(language)).toBe(true);
    }
    // A configuration snippet must not carry a "run this" button. It would be wrong every time,
    // and a button that is usually wrong teaches people to stop reading buttons.
    for (const language of ["json", "toml", "python", "ts", "rust", "yaml", "diff", null, ""]) {
      expect(isShellLanguage(language)).toBe(false);
    }
    expect(isShellLanguage("  BASH  ")).toBe(true);
  });

  it("takes a single command and strips the prompt a transcript carries", () => {
    expect(runnableCommand("npx -y @xavifabregat/spotify-mcp init")).toBe(
      "npx -y @xavifabregat/spotify-mcp init",
    );
    expect(runnableCommand("$ npm install")).toBe("npm install");
    expect(runnableCommand("PS C:\\Users\\me> node --version")).toBe("node --version");
    expect(runnableCommand("C:\\Users\\me> dir")).toBe("dir");
    // Comments and blank lines are what a person would drop by hand.
    expect(runnableCommand("# sign in first\n\nnpx -y thing init\n")).toBe("npx -y thing init");
  });

  it("refuses anything that is not one readable command", () => {
    // A script prefilled at a prompt runs the moment Enter is pressed, and nobody can check
    // five lines at a glance. The block simply stays a block.
    expect(runnableCommand("cd /tmp\nrm -rf build\nmake")).toBeNull();
    expect(runnableCommand("")).toBeNull();
    expect(runnableCommand("   \n  \n")).toBeNull();
    expect(runnableCommand("$")).toBeNull();
    expect(runnableCommand("# only a comment")).toBeNull();
  });

  it("never produces a newline, because pressing Enter is the user's", () => {
    for (const source of [
      "npm test",
      "$ npm test",
      "  npm test  ",
      "# note\nnpm test",
    ]) {
      expect(runnableCommand(source)).not.toMatch(/[\r\n]/u);
    }
  });

  it("carries the request to whoever is listening, and stops when they leave", () => {
    const heard = vi.fn();
    const stop = onTerminalAsked(heard);

    askForTerminal({ command: "npm test", language: "bash" });
    expect(heard).toHaveBeenCalledWith({ command: "npm test", language: "bash" });

    stop();
    askForTerminal({ command: "npm test", language: "bash" });
    expect(heard).toHaveBeenCalledTimes(1);
  });
});
