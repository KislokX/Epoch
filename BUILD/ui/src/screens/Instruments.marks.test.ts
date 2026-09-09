/**
 * Which mark a program gets.
 *
 * Reported as "el logo de llama": a backend named `llama` — which is what Epoch's own detection
 * and the add-a-backend form both produce — matched none of the rules and fell through to the
 * mark used for a program Epoch cannot place.
 *
 * The ordering is the whole of the fix, and it is the reason this is a test rather than a line:
 * `ollama` contains `llama`, so a bare check one line earlier gives Ollama llama.cpp's mark and
 * nobody notices which of the two is wrong.
 */

import { describe, expect, it } from "vitest";

import { lookFor } from "./Instruments";

describe("the mark a program gets", () => {
  it("gives a backend called llama the same mark as llama.cpp", () => {
    expect(lookFor({ id: "llama" })).toEqual(lookFor({ id: "llama_cpp" }));
    expect(lookFor({ id: "b1", name: "llama" })).toEqual(
      lookFor({ id: "llama_cpp", name: "llama.cpp" }),
    );
  });

  it("does not give Ollama llama.cpp's mark", () => {
    // `ollama` contains `llama`. This is the assertion the ordering exists for.
    expect(lookFor({ id: "ollama" })).not.toEqual(lookFor({ id: "llama_cpp" }));
    expect(lookFor({ id: "b2", name: "ollama" })).not.toEqual(lookFor({ id: "llama" }));
  });

  it("still falls back for a program it cannot place", () => {
    // The fallback is a real state and must keep working: something Epoch has never heard of
    // gets a mark that says so rather than borrowing somebody else's.
    const unknown = lookFor({ id: "some-new-thing", name: "Some New Thing" });
    for (const known of ["llama_cpp", "ollama", "codex", "gemini", "claude-code"]) {
      expect(unknown).not.toEqual(lookFor({ id: known }));
    }
  });
});
