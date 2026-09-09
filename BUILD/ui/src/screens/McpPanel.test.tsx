/**
 * The MCP deck, and the one promise it makes about credentials.
 *
 * A server needs an id or a key before it will start, and until this panel could edit them the
 * only way to give one was to install through the Workshop. A server added by hand had no way at
 * all — which a character, asked for help, could only report as *I do not have access to that*.
 *
 * These assertions hold the shape that fixes it without giving a surface a token to carry: a
 * value goes out through its own one-way command, an ordinary save carries the credential's
 * **name** and never its value, and nothing ever draws one.
 */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { McpPanel } from "./McpPanel";
import type { McpServer, McpView } from "../ipc/contracts";

const SPOTIFY: McpServer = {
  id: "spotify",
  command: "npx.cmd",
  args: ["-y", "@xavifabregat/spotify-mcp"],
  env: { SPOTIFY_MARKET: "CR" },
  secrets: ["SPOTIFY_CLIENT_ID"],
  enabled: true,
};

/** The Engine, answering with one configured server. Returns what the panel asked it. */
function engine(servers: readonly McpServer[] = [SPOTIFY]) {
  const calls: { command: string; args: unknown }[] = [];
  const view: McpView = { servers, problem: null };
  vi.mocked(invoke).mockImplementation((command: string, args: unknown) => {
    calls.push({ command, args });
    if (command === "list_mcp") return Promise.resolve(view);
    if (command === "probe_mcp") return Promise.resolve([[], []]);
    return Promise.resolve(null);
  });
  return calls;
}

async function edit() {
  render(<McpPanel />);
  await waitFor(() => expect(screen.getByText("spotify")).toBeInTheDocument());
  await userEvent.click(screen.getByRole("button", { name: "EDIT" }));
}

describe("the MCP deck", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("shows a credential by name and never draws its value", async () => {
    // There is no shape in which a value comes back — `secrets` carries names — so the only
    // honest thing to show is that the variable exists and is held.
    engine();
    await edit();

    expect(screen.getByText("SPOTIFY_CLIENT_ID")).toBeInTheDocument();
    expect(screen.getByText("kept encrypted")).toBeInTheDocument();
    // And the plain configuration is editable, because that is what it is.
    expect(screen.getByDisplayValue("CR")).toBeInTheDocument();
  });

  it("sends a credential one way, never through the save that carries the rest", async () => {
    // The separation is the point. `save_mcp` round-trips an entry a surface may hold in full;
    // a value must not be in that entry, so it travels by itself and nothing reads it back.
    const calls = engine();
    await edit();

    // By label, not by placeholder. The placeholder is an example of what goes in the box; the
    // label is what the box *is* — and querying by the example is how a test keeps passing while
    // the screen teaches the wrong thing. It did: with two bare boxes, the first placeheld
    // `SPOTIFY_CLIENT_ID`, somebody pasted the id into the name box.
    await userEvent.type(
      screen.getByLabelText("VARIABLE"),
      "SPOTIFY_CLIENT_SECRET",
    );
    await userEvent.type(screen.getByLabelText("ITS VALUE"), "b1708c23eb");
    await userEvent.click(screen.getByRole("button", { name: "ADD" }));

    const sent = calls.find((c) => c.command === "save_mcp_secret");
    expect(sent?.args).toMatchObject({
      server: "spotify",
      name: "SPOTIFY_CLIENT_SECRET",
      value: "b1708c23eb",
    });
    expect(calls.some((c) => c.command === "save_mcp")).toBe(false);
  });

  it("saves an edit carrying the credential's name, so the Engine does not read it as removed", async () => {
    // The defect this replaced: the form could edit `id`, `command` and `args` and sent exactly
    // those, so everything else arrived empty and a working install was overwritten with it.
    // Now the form holds the whole entry — which is only safe because none of it is a value.
    const calls = engine();
    await edit();

    await userEvent.click(screen.getByRole("button", { name: "SAVE" }));

    const saved = calls.find((c) => c.command === "save_mcp");
    expect(saved?.args).toMatchObject({
      server: {
        id: "spotify",
        secrets: ["SPOTIFY_CLIENT_ID"],
        env: { SPOTIFY_MARKET: "CR" },
      },
    });
  });

  it("says why it did nothing, beside the button that did nothing", async () => {
    // Reported as "the ADD button does nothing", and from where the user was that is exactly
    // what it was: `add` returned silently on an incomplete row, and anything the Engine
    // refused went to a notice at the head of a panel the form sits far below. An error nobody
    // scrolls to is an error nobody has.
    engine();
    await edit();

    await userEvent.type(
      screen.getByLabelText("VARIABLE"),
      "SPOTIFY_CLIENT_ID",
    );
    await userEvent.click(screen.getByRole("button", { name: "ADD" }));

    expect(
      screen.getByText(/SPOTIFY_CLIENT_ID needs a value/),
    ).toBeInTheDocument();
    // And it did not pretend to store anything.
    expect(screen.getByLabelText("VARIABLE")).toHaveValue("SPOTIFY_CLIENT_ID");
  });

  it.each([
    ["SAVE", async () => {}],
    [
      "ADD",
      async () => {
        await userEvent.type(
          screen.getByLabelText("VARIABLE"),
          "SPOTIFY_CLIENT_ID",
        );
        await userEvent.type(screen.getByLabelText("ITS VALUE"), "b1708c23eb");
      },
    ],
  ])("keeps what the servers already said after %s", async (button, fill) => {
    // Changing anything turned every server's status to UNASKED, as though the deck had
    // forgotten what it knew. It had not: the remembered answer was discarded and never
    // re-read. Fixed in SAVE first and reported again from ADD, because the same three lines
    // were written in three places — which is why they are one now, and why this runs over
    // both buttons rather than the one that was reported.
    //
    // `probe_mcp` answers from memory and starts nothing, so there was never a reason to
    // leave the deck reporting a state that was not true.
    const calls = engine();
    await edit();
    await fill();
    await userEvent.click(screen.getByRole("button", { name: button }));

    await waitFor(() =>
      expect(
        calls.filter((c) => c.command === "probe_mcp").length,
      ).toBeGreaterThan(1),
    );
    // From memory, never by starting the servers: that stays a button somebody presses.
    expect(calls.some((c) => c.command === "refresh_mcp")).toBe(false);
  });

  it("tells the Launcher, because a server is also a capability the crew can be given", async () => {
    // Forgetting Spotify left `Spotify · 15` ticked in the character editor: a box granting a
    // connection nobody had any more, with a tool count beside it. Nothing was wrong in the
    // Engine — the Launcher reads the vocabulary once, on open, and this deck had no way to say
    // it was now false. The Workshop already had the wire; this deck simply never got one.
    const changed = vi.fn();
    engine();
    render(<McpPanel onChanged={changed} />);
    await waitFor(() =>
      expect(screen.getByText("spotify")).toBeInTheDocument(),
    );

    await userEvent.click(
      screen.getAllByRole("button", { name: "FORGET" })[0]!,
    );
    await userEvent.click(screen.getByRole("button", { name: "FORGET IT" }));

    await waitFor(() => expect(changed).toHaveBeenCalled());
  });

  it("does not offer a credential field for a server that does not exist yet", async () => {
    // The encrypted store keys on the server's name, so there is nowhere to put one until it
    // has one. Saying that beats a field that would fail for a reason nobody could see.
    engine([]);
    render(<McpPanel />);
    await userEvent.click(
      await screen.findByRole("button", { name: "ADD A SERVER" }),
    );

    expect(screen.getByRole("checkbox", { name: /SECRET/ })).toBeDisabled();
    expect(screen.getByText(/Save the server first/)).toBeInTheDocument();
  });
});
