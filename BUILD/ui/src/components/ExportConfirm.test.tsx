import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { ExportConfirm, size } from "./ExportConfirm";
import type { ExportView } from "../ipc/contracts";

const ready: ExportView = {
  what: "The Archipelago",
  into: "archipelago-0.1.0.zip",
  carries: ["assets/preview.png", "land.png", "pack.toml", "places.toml"],
  // The map and the manifest are Epoch's own files. `land.png` is whatever the user dropped in.
  unvouched: ["land.png"],
  bytes: 4_200_000,
  leaves: [
    "Your conversations stay here.",
    "Your crew does not travel with a World.",
  ],
  problems: [],
};

describe("what sending a World says before it sends it", () => {
  it("puts what stays behind before what travels", () => {
    // A removal reads worst-first because its risk is taking too much. An export's risk is
    // sending too much, to somebody you cannot un-send it to — so the answer to *what am I not
    // handing over* comes first, and the file list is folded away behind it.
    render(
      <ExportConfirm plan={ready} busy={false} onCancel={vi.fn()} onConfirm={vi.fn()} />,
    );
    const leaves = screen.getByText(/crew does not travel/);
    const files = screen.getByText(/Travels —/);
    expect(leaves.compareDocumentPosition(files)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING,
    );
  });

  it("counts and weighs what will go, rather than describing it", () => {
    render(
      <ExportConfirm plan={ready} busy={false} onCancel={vi.fn()} onConfirm={vi.fn()} />,
    );
    expect(screen.getByText(/Travels — 4 files, 4\.2 MB/)).toBeInTheDocument();
    // Enumerated, not summarised: the list is what makes the count checkable.
    expect(screen.getByText("places.toml")).toBeInTheDocument();
  });

  it("says what it will be called, and leaves where to the user", () => {
    // **Where is the one thing about an export that is not Epoch's business** — it exists to
    // leave. Choosing a folder inside the vault and announcing the path turned *sending a World*
    // into *finding the World Epoch put somewhere*.
    render(
      <ExportConfirm plan={ready} busy={false} onCancel={vi.fn()} onConfirm={vi.fn()} />,
    );
    expect(screen.getByText(/One archive, archipelago-0\.1\.0\.zip/)).toBeInTheDocument();
    expect(screen.getByText(/You choose where it lands/)).toBeInTheDocument();
    // The button says a dialog is coming rather than that something is about to be written.
    expect(
      screen.getByRole("button", { name: "CHOOSE WHERE…" }),
    ).toBeInTheDocument();
  });

  it("offers no button at all when it cannot run", () => {
    // An export is a distribution, so CONTENT_PHILOSOPHY's hard rule applies here. The refusal
    // is the whole panel rather than a warning beside a button somebody can still press.
    render(
      <ExportConfirm
        plan={{
          ...ready,
          problems: ["This World declares no licence, and an export is a distribution."],
        }}
        busy={false}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(screen.getByText(/Cannot send The Archipelago/)).toBeInTheDocument();
    expect(screen.getByText(/declares no licence/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "CHOOSE WHERE…" })).not.toBeInTheDocument();
    // And nothing is counted either: a file count beside a refusal reads as a plan.
    expect(screen.queryByText(/Travels —/)).not.toBeInTheDocument();
  });

  it("says it is working rather than showing an empty plan", () => {
    // The panel opens on the user's intent and fills in when the disk has been read. A blank
    // frame in the meantime would read as *nothing would travel*.
    render(
      <ExportConfirm plan={null} busy={false} onCancel={vi.fn()} onConfirm={vi.fn()} />,
    );
    expect(screen.getByText(/Working out what would travel/)).toBeInTheDocument();
  });
});

describe("how big it is, in words", () => {
  it("lands each number at the scale it belongs to", () => {
    // A World is kilobytes when it is shapes and gigabytes when it has artwork, and one unit
    // for both makes one of them unreadable.
    expect(size(820)).toBe("820 bytes");
    expect(size(4_200)).toBe("4 KB");
    expect(size(4_200_000)).toBe("4.2 MB");
    expect(size(4_200_000_000)).toBe("4.2 GB");
  });

  it("names the files the declared licence was not written about", () => {
    // The pack's [license] is a statement its author made about the pack. A picture dropped in
    // afterwards leaves under that sentence without anybody having said so, and Epoch cannot
    // read what a picture is or who made it.
    render(
      <ExportConfirm plan={ready} busy={false} onCancel={vi.fn()} onConfirm={vi.fn()} />,
    );
    expect(screen.getByText("— licence not stated")).toBeInTheDocument();
    expect(screen.getByText(/artwork you imported/)).toBeInTheDocument();
  });

  it("says it and does not refuse over it", () => {
    // The hard rule governs what Epoch distributes; what somebody hands a friend from their own
    // vault is theirs. Refusing would be Epoch deciding a question it cannot even measure.
    render(
      <ExportConfirm plan={ready} busy={false} onCancel={vi.fn()} onConfirm={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: "CHOOSE WHERE…" })).toBeEnabled();
  });

  it("says nothing about licences when every file is the World's own", () => {
    render(
      <ExportConfirm
        plan={{ ...ready, unvouched: [] }}
        busy={false}
        onCancel={vi.fn()}
        onConfirm={vi.fn()}
      />,
    );
    expect(screen.queryByText(/artwork you imported/)).not.toBeInTheDocument();
  });
});
