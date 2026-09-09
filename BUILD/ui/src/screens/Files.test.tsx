import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { MadeRow } from "../ipc/world";

const made = vi.hoisted(() => ({ value: [] as MadeRow[] }));

vi.mock("../ipc/world", () => ({
  madeHere: vi.fn(async () => made.value),
  openMade: vi.fn(async () => null),
  // A thumbnail is an `<img src>` over Epoch's own scheme now, not bytes fetched here.
  pictureSrc: (file: string) => `epoch://picture/${file}`,
}));

import { Files } from "./Files";

const row = (over: Partial<MadeRow> = {}): MadeRow => ({
  kind: "image",
  reference: "a1b2c3.png",
  summary: "a lighthouse at dawn",
  quest: "q1",
  questTitle: "Cover art",
  at: Date.now() - 60_000,
  shown: true,
  ...over,
});

describe("what the Files window says a World has made", () => {
  it("says nothing rather than padding the list with effort", async () => {
    // ADR-0025: a Quest that produced no evidence produced nothing, and History has to be able
    // to say that. Counting conversations here would describe effort instead of results.
    made.value = [];
    render(<Files onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText(/Nothing yet/)).toBeInTheDocument(),
    );
    expect(screen.getByText("0 things made")).toBeInTheDocument();
  });

  it("names the conversation each thing came out of", async () => {
    // The whole reason this is enumerated from Quests rather than read out of a folder.
    made.value = [row()];
    render(<Files onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText("a lighthouse at dawn")).toBeInTheDocument(),
    );
    expect(screen.getByText("Cover art")).toBeInTheDocument();
    expect(screen.getByText("image")).toBeInTheDocument();
  });

  it("says 'earlier' for work whose record was compacted away", async () => {
    // A continuity brief replaced the record that carried the time. Printing the moment of
    // compaction would be printing a time the thing was not made at.
    made.value = [row({ at: null })];
    render(<Files onClose={() => {}} />);
    await waitFor(() => expect(screen.getByText("earlier")).toBeInTheDocument());
  });

  it("falls back to the reference when nothing summarised it", async () => {
    made.value = [row({ summary: "", reference: "src/auth.rs", shown: false })];
    render(<Files onClose={() => {}} />);
    await waitFor(() =>
      expect(screen.getByText("src/auth.rs")).toBeInTheDocument(),
    );
  });

  it("says it is reading rather than showing an empty list", async () => {
    made.value = [row()];
    render(<Files onClose={() => {}} />);
    expect(screen.getByText("reading…")).toBeInTheDocument();
  });
});
