import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import type { ReadinessRow } from "../ipc/contracts";

const rows: ReadinessRow[] = [
  { id: "claude_code", name: "Claude Code", state: "ready", note: "Signed in.", act: "nothing" },
  { id: "ollama", name: "Ollama", state: "ready", note: "Answering, with 4 models.", act: "nothing" },
  {
    id: "llama_cpp",
    name: "llama.cpp",
    state: "needsYou",
    note: "Connection Failed.",
    act: "configure",
  },
  {
    id: "bridge:mac",
    name: "Ollama",
    state: "ready",
    note: "Answering, with 6 models.",
    act: "nothing",
  },
];

vi.mock("../ipc/launcher", () => ({
  fetchReadiness: vi.fn(async () => rows),
  signInAgent: vi.fn(async () => null),
}));

import { Readiness } from "./Readiness";

/**
 * The three programs on this machine were reported twice in one deck: once here as a reading,
 * once above with buttons that can start, install and stock them.
 *
 * Two lists saying *llama.cpp is not answering*, one of which can do something about it, is the
 * drift this component's own note warns against — reached from the other direction. The one
 * that can act keeps them.
 */
describe("what the readiness list leaves to the panel above it", () => {
  it("drops the rows that panel is already reporting", async () => {
    render(<Readiness omit={["ollama", "llama_cpp", "lm_studio"]} />);
    await waitFor(() => expect(screen.getByText("Claude Code")).toBeInTheDocument());
    expect(screen.queryByText("llama.cpp")).not.toBeInTheDocument();
  });

  it("keeps a paired machine's runtime, which no other panel covers", async () => {
    // `bridge:mac` is called "Ollama" too, and is a different computer. Omitting by *name*
    // would have taken it out with the local one — which is why this is by id.
    render(<Readiness omit={["ollama", "llama_cpp", "lm_studio"]} />);
    await waitFor(() => expect(screen.getByText("Claude Code")).toBeInTheDocument());
    expect(screen.getByText("Ollama")).toBeInTheDocument();
    expect(screen.getByText(/6 models/)).toBeInTheDocument();
  });

  it("counts what it shows, not what it was given", async () => {
    // "2 of 4" beside two rows is a count nobody can check. The count is a fact about this
    // list, which is the only list the reader can see.
    render(<Readiness omit={["ollama", "llama_cpp", "lm_studio"]} />);
    await waitFor(() =>
      expect(screen.getByText("2 of 2 measured and working")).toBeInTheDocument(),
    );
  });

  it("says it is measuring rather than rendering nothing", () => {
    // This probes — the agents' own programs, then every backend local and paired, which was
    // 4.18 s of network on 2026-08-21. Rendering `null` until it answered made a measurement in
    // flight look like a panel that had decided there was nothing to say.
    render(<Readiness />);
    expect(screen.getByText("Measuring…")).toBeInTheDocument();
  });

  it("shows everything when nothing is omitted", async () => {
    // The default, and what every other caller gets.
    render(<Readiness />);
    await waitFor(() =>
      expect(screen.getByText("3 of 4 measured and working")).toBeInTheDocument(),
    );
  });
});
