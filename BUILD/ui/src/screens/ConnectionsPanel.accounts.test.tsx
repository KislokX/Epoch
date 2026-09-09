/**
 * A second sign-in of one agent program, and the one promise it makes.
 *
 * `CLAUDE_CONFIG_DIR` and `CODEX_HOME` isolate a sign-in completely — measured, not remembered —
 * so two accounts of one agent are two genuinely separate sessions. What this panel must never
 * become is the place a credential passes through: Epoch makes the folder, starts the agent's
 * own login in the agent's own window and steps back.
 *
 * So the assertions are about *what is sent*: a kind and a label the user typed, never a
 * password field and never a token. And the label is asked for rather than measured, because
 * Codex cannot be asked which account is signed in at all and a title Epoch invented would be
 * the gauge nobody can explain.
 */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { ConnectionsPanel } from "./ConnectionsPanel";
import type { AgentStatus } from "../ipc/launcher";

const CLAUDE: AgentStatus = {
  id: "claude-code",
  kind: "claude-code",
  name: "Claude Code",
  installed: true,
  version: "2.1",
  lookedIn: null,
  note: null,
  signedIn: true,
  account: "someonex@gmail.com",
};

const SECOND: AgentStatus = {
  ...CLAUDE,
  id: "claude-code-2",
  name: "work",
  account: "work@example.com",
};

const GEMINI: AgentStatus = {
  id: "gemini",
  kind: "gemini",
  name: "Gemini CLI",
  installed: true,
  version: "0.56",
  lookedIn: null,
  note: null,
  signedIn: null,
  account: null,
};

/** The Engine, answering with whichever accounts a case needs. */
function engine(agents: readonly AgentStatus[]) {
  const calls: { command: string; args: unknown }[] = [];
  vi.mocked(invoke).mockImplementation((command: string, args: unknown) => {
    calls.push({ command, args });
    if (command === "list_agents") return Promise.resolve(agents);
    if (command === "list_backends")
      return Promise.resolve({ backends: [], problem: null });
    if (command === "list_backend_keys") return Promise.resolve([]);
    if (command === "add_agent_account") return Promise.resolve("claude-code-2");
    if (command === "remove_agent_account")
      return Promise.resolve("Forgotten. Its sign-in is left where it is.");
    return Promise.resolve([]);
  });
  return calls;
}

function panel() {
  return render(
    <ConnectionsPanel providers={[]} probing={false} onProbe={() => {}} />,
  );
}

describe("a second account of one agent", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("sends the kind and the label the user typed, and nothing else", async () => {
    const user = userEvent.setup();
    const calls = engine([CLAUDE]);
    panel();

    await user.click(await screen.findByRole("button", { name: "ADD ACCOUNT" }));
    await user.type(
      screen.getByPlaceholderText(/work, personal/),
      "personal",
    );
    await user.click(screen.getByRole("button", { name: "ADD AND SIGN IN" }));

    await waitFor(() =>
      expect(
        calls.find((c) => c.command === "add_agent_account"),
      ).toBeDefined(),
    );
    expect(calls.find((c) => c.command === "add_agent_account")?.args).toEqual({
      kind: "claude-code",
      label: "personal",
    });
  });

  it("never draws a password field", async () => {
    const user = userEvent.setup();
    engine([CLAUDE]);
    const { container } = panel();

    await user.click(await screen.findByRole("button", { name: "ADD ACCOUNT" }));
    expect(container.querySelector('input[type="password"]')).toBeNull();
  });

  it("does not offer a second Gemini, because two could not be told apart", async () => {
    engine([GEMINI]);
    panel();

    await screen.findByText("Gemini CLI");
    expect(screen.queryByRole("button", { name: "ADD ACCOUNT" })).toBeNull();
  });

  it("offers to forget an added account, never the one that was already there", async () => {
    engine([CLAUDE, SECOND]);
    panel();

    await screen.findByText("work");
    // One FORGET, on the added row. The program's own sign-in is not Epoch's to remove.
    expect(screen.getAllByRole("button", { name: "FORGET" })).toHaveLength(1);
    expect(screen.getAllByRole("button", { name: "ADD ACCOUNT" })).toHaveLength(
      1,
    );
  });

  it("repeats what the Engine said became of a forgotten sign-in", async () => {
    const user = userEvent.setup();
    engine([CLAUDE, SECOND]);
    panel();

    await user.click(await screen.findByRole("button", { name: "FORGET" }));
    expect(
      await screen.findByText(/Its sign-in is left where it is/),
    ).toBeInTheDocument();
  });

  it("renames without changing the id a character's brain names", async () => {
    const user = userEvent.setup();
    const calls = engine([CLAUDE, SECOND]);
    panel();

    await user.click(await screen.findByRole("button", { name: "RENAME" }));
    const field = screen.getByPlaceholderText(/work, personal/);
    await user.clear(field);
    await user.type(field, "personal");
    await user.click(screen.getByRole("button", { name: "SAVE NAME" }));

    await waitFor(() =>
      expect(
        calls.find((c) => c.command === "rename_agent_account"),
      ).toBeDefined(),
    );
    expect(
      calls.find((c) => c.command === "rename_agent_account")?.args,
    ).toEqual({ id: "claude-code-2", label: "personal" });
  });
});
