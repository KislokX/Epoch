/**
 * The Workshop, and the promises it must not break.
 *
 * Installing an MCP server runs a third party's program on this machine, and every tool it then
 * offers is registered with every effect and no reversal because MCP declares neither
 * (ADR-0008). These assertions are what keep the surface honest about that: the command is
 * visible before the button, a server this machine cannot run says so instead of offering, a
 * credential is never drawn in the clear, and what gets installed is exactly what was shown.
 */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { WorkshopPanel } from "./WorkshopPanel";
import type { WorkshopListing, WorkshopShelf, WorkshopShown } from "../ipc/contracts";

function listing(over: Partial<WorkshopListing> = {}): WorkshopListing {
  return {
    name: "com.pulsemcp/remote-filesystem",
    title: "Remote Filesystem",
    description: "MCP server for remote filesystem operations.",
    version: "0.1.5",
    publisher: "pulsemcp",
    repository: "https://github.com/pulsemcp/mcp-servers",
    subfolder: null,
    active: true,
    suggestedId: "remote_filesystem",
    offers: [],
    ...over,
  };
}

const READY: WorkshopShown = {
  listing: listing(),
  ready: {
    kind: "start",
    registry: "npm",
    package: "remote-filesystem-mcp-server",
    command: "npx.cmd",
    args: ["-y", "remote-filesystem-mcp-server@0.1.5"],
    inputs: [
      {
        name: "GCS_BUCKET",
        description: "Google Cloud Storage bucket name.",
        required: true,
        secret: false,
        default: null,
        placeholder: null,
        choices: [],
      },
      {
        name: "GCS_PRIVATE_KEY",
        description: "Service account private key.",
        required: false,
        secret: true,
        default: null,
        placeholder: null,
        choices: [],
      },
    ],
  },
  blocked: null,
  installedAs: [],
};

const BLOCKED: WorkshopShown = {
  listing: listing({ name: "com.notion/mcp", title: "Notion", suggestedId: "mcp" }),
  ready: null,
  blocked: "is a hosted server. Epoch connects to servers it starts.",
  installedAs: [],
};

function shelf(shown: readonly WorkshopShown[], present = ["npx"]): WorkshopShelf {
  return { shown, next: null, fetchedMs: null, problem: null, runtimes: { present } };
}

/** Answer the two commands this screen uses. `installed` is what the last install returned. */
function engine(page: WorkshopShelf, installed: string | Error = "remote_filesystem") {
  const calls: { command: string; args: unknown }[] = [];
  vi.mocked(invoke).mockImplementation((command: string, args: unknown) => {
    calls.push({ command, args });
    if (command === "workshop_search") return Promise.resolve(page);
    if (command === "workshop_install") {
      return installed instanceof Error ? Promise.reject(installed) : Promise.resolve(installed);
    }
    return new Promise<never>(() => {});
  });
  return calls;
}

async function openMcpDoor() {
  render(<WorkshopPanel onInstalled={() => {}} />);
  await userEvent.click(screen.getByRole("button", { name: /MCP WORKSHOP/ }));
}

describe("WorkshopPanel", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("opens the Models Workshop, which is no longer waiting on anything", async () => {
    // **This test used to assert the opposite**, and it earned its keep by failing the moment
    // the door was lit. It was cold — a frame with no light, naming the subsystem it needed —
    // and that subsystem now exists: what this machine is, what it has, and what a model
    // really weighs.
    engine(shelf([]));
    render(<WorkshopPanel onInstalled={() => {}} />);

    expect(screen.queryByText(/NOT YET BUILT/)).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /MODELS WORKSHOP/ }),
    ).toBeInTheDocument();
  });

  it("shows the exact command before the button, never behind it", async () => {
    engine(shelf([READY]));
    await openMcpDoor();

    await waitFor(() =>
      expect(screen.getByText(/npx\.cmd -y remote-filesystem-mcp-server@0\.1\.5/)).toBeInTheDocument(),
    );
    // And who wrote it, because installing runs their code.
    expect(screen.getByText(/pulsemcp/)).toBeInTheDocument();
    expect(screen.getByText("0.1.5")).toBeInTheDocument();
  });

  it("offers nothing it cannot run, and shows the Engine's reason", async () => {
    // The sentence is the Engine's. A second copy of "can this run" living here would eventually
    // disagree with the one the install path uses, and the disagreement would look like a button
    // that lies.
    engine(shelf([BLOCKED]));
    await openMcpDoor();

    await waitFor(() => expect(screen.getByText(/hosted server/)).toBeInTheDocument());
    expect(screen.queryByRole("button", { name: /INSTALL/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /SET UP/ })).not.toBeInTheDocument();
  });

  it("never draws a credential in the clear", async () => {
    engine(shelf([READY]));
    await openMcpDoor();

    await waitFor(() => screen.getByRole("button", { name: /SET UP · 2/ }));
    await userEvent.click(screen.getByRole("button", { name: /SET UP · 2/ }));

    // The plain one is readable; the credential is not, and says where it is going.
    expect(screen.getByText("GCS_BUCKET")).toBeInTheDocument();
    const secret = screen.getByText("GCS_PRIVATE_KEY").closest("label");
    expect(secret?.querySelector("input")).toHaveAttribute("type", "password");
    expect(screen.getByText("kept encrypted")).toBeInTheDocument();
  });

  it("installs exactly what was shown, offer included", async () => {
    // Sent back unchanged rather than re-fetched by name. What the user approved is the command
    // on screen; re-fetching would install whatever the catalogue says now.
    const calls = engine(shelf([READY]));
    await openMcpDoor();

    await waitFor(() => screen.getByRole("button", { name: /SET UP · 2/ }));
    await userEvent.click(screen.getByRole("button", { name: /SET UP · 2/ }));
    await userEvent.type(screen.getByText("GCS_BUCKET").closest("label")!.querySelector("input")!, "b");
    await userEvent.click(screen.getByRole("button", { name: "INSTALL" }));

    const install = await waitFor(() => {
      const found = calls.find((c) => c.command === "workshop_install");
      expect(found).toBeDefined();
      return found!;
    });
    const args = install.args as {
      listing: WorkshopListing;
      offer: { command: string; args: string[] };
      answers: Record<string, string>;
    };
    expect(args.offer.command).toBe("npx.cmd");
    expect(args.offer.args).toEqual(["-y", "remote-filesystem-mcp-server@0.1.5"]);
    expect(args.listing.name).toBe("com.pulsemcp/remote-filesystem");
    expect(args.answers).toEqual({ GCS_BUCKET: "b" });

    await waitFor(() => expect(screen.getByText("remote_filesystem")).toBeInTheDocument());
  });

  it("reports a refused install instead of appearing to have worked", async () => {
    engine(shelf([READY]), new Error("GCS_BUCKET needs a value"));
    await openMcpDoor();

    await waitFor(() => screen.getByRole("button", { name: /SET UP · 2/ }));
    await userEvent.click(screen.getByRole("button", { name: /SET UP · 2/ }));
    await userEvent.click(screen.getByRole("button", { name: "INSTALL" }));

    await waitFor(() => expect(screen.getByText(/GCS_BUCKET needs a value/)).toBeInTheDocument());
  });

  it("says what this machine can run, measured and not assumed", async () => {
    // Which is what makes a blocked card's sentence make sense. Docker absent is a fact.
    engine(shelf([READY], ["npx", "uvx"]));
    await openMcpDoor();

    await waitFor(() => expect(screen.getByText(/npx · uvx/)).toBeInTheDocument());
  });

  it("walks pages forward on the registry's cursor and back on its own trail", async () => {
    // The registry hands out forward cursors only, so there is no "previous" to ask for. Going
    // back means remembering where each page started.
    const calls = engine({ ...shelf([READY]), next: "com.x/second:1.0.0" });
    await openMcpDoor();

    await waitFor(() => expect(screen.getByText("PAGE 1")).toBeInTheDocument());
    expect(screen.getByRole("button", { name: /PREVIOUS/ })).toBeDisabled();

    await userEvent.click(screen.getByRole("button", { name: /NEXT/ }));
    await waitFor(() => expect(screen.getByText("PAGE 2")).toBeInTheDocument());
    expect(calls.at(-1)!.args).toMatchObject({ cursor: "com.x/second:1.0.0" });

    await userEvent.click(screen.getByRole("button", { name: /PREVIOUS/ }));
    await waitFor(() => expect(screen.getByText("PAGE 1")).toBeInTheDocument());
    // Back to the beginning is the *absence* of a cursor, not a remembered first one.
    expect(calls.at(-1)!.args).toMatchObject({ cursor: null });
  });

  it("starts a new search at the first page rather than deep in the old one", async () => {
    const calls = engine({ ...shelf([READY]), next: "com.x/second:1.0.0" });
    await openMcpDoor();

    await waitFor(() => screen.getByRole("button", { name: /NEXT/ }));
    await userEvent.click(screen.getByRole("button", { name: /NEXT/ }));
    await waitFor(() => expect(screen.getByText("PAGE 2")).toBeInTheDocument());

    await userEvent.click(screen.getByRole("button", { name: "pdf" }));

    await waitFor(() => expect(screen.getByText("PAGE 1")).toBeInTheDocument());
    expect(calls.at(-1)!.args).toMatchObject({ query: "pdf", cursor: null });
  });

  it("offers shortcuts as searches, and says they are searches", async () => {
    // Measured: the registry matches a server's *name*. `pdfassistant` says "redact" in its
    // description and does not appear in a search for `redact`. A chip that looked like a
    // category would quietly miss whatever is named differently.
    const calls = engine(shelf([READY]));
    await openMcpDoor();

    await waitFor(() => screen.getByRole("button", { name: "excel" }));
    await userEvent.click(screen.getByRole("button", { name: "excel" }));

    expect(calls.at(-1)!.args).toMatchObject({ query: "excel" });
    // The search box shows it, so what ran is never a hidden filter.
    expect(screen.getByRole("searchbox")).toHaveValue("excel");
    expect(screen.getByText(/registry matches server names/)).toBeInTheDocument();

    // And clicking it again clears it rather than becoming a mode nobody can leave.
    await userEvent.click(screen.getByRole("button", { name: "excel" }));
    expect(calls.at(-1)!.args).toMatchObject({ query: "" });
  });

  it("offers to remove what is already installed rather than a second copy", async () => {
    // How the third copy happened: the card kept offering INSTALL, and installing renames on
    // collision, so each click quietly produced `image_diff_2`, `image_diff_3`.
    engine(shelf([{ ...READY, installedAs: ["image_diff", "image_diff_2"] }]));
    await openMcpDoor();

    await waitFor(() =>
      expect(screen.getByRole("button", { name: "UNINSTALL image_diff" })).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "UNINSTALL image_diff_2" })).toBeInTheDocument();
    expect(screen.getByText(/installed as image_diff, image_diff_2/)).toBeInTheDocument();

    // Installing again stays possible for anybody who means it — it just stops being the
    // obvious thing to click.
    expect(screen.getByRole("button", { name: "INSTALL AGAIN" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /SET UP/ })).not.toBeInTheDocument();
  });

  it("removes a server through the Engine, credentials included", async () => {
    const calls = engine(shelf([{ ...READY, installedAs: ["image_diff"] }]));
    await openMcpDoor();

    await waitFor(() => screen.getByRole("button", { name: "UNINSTALL image_diff" }));
    await userEvent.click(screen.getByRole("button", { name: "UNINSTALL image_diff" }));

    await waitFor(() => {
      expect(calls.find((c) => c.command === "forget_mcp")?.args).toEqual({ id: "image_diff" });
    });
  });

  it("opens a source through the Engine, because the webview ignores a plain link", async () => {
    // `target="_blank"` does nothing in a Tauri webview: the link looked right and went nowhere.
    // The Engine checks the scheme, because these addresses come from a catalogue strangers write.
    const calls = engine(shelf([READY]));
    await openMcpDoor();

    await waitFor(() => screen.getByRole("button", { name: "SOURCE" }));
    await userEvent.click(screen.getByRole("button", { name: "SOURCE" }));

    expect(calls.at(-1)).toEqual({
      command: "open_link",
      args: { url: "https://github.com/pulsemcp/mcp-servers" },
    });
  });

  it("stays a place when the catalogue cannot be reached", async () => {
    const cold: WorkshopShelf = {
      shown: [],
      next: null,
      fetchedMs: null,
      problem: "the catalogue could not be reached, and nothing has been fetched yet.",
      runtimes: { present: [] },
    };
    engine(cold);
    await openMcpDoor();

    await waitFor(() =>
      expect(screen.getByText(/could not be reached/)).toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "BACK" })).toBeInTheDocument();
  });
});
