import { describe, expect, it } from "vitest";

// The splitter is small and the whole `hf` path depends on it being right, so it is exported
// for this rather than tested through four layers of component.
function split(pull: string): [string, string | null] {
  const rest = pull.replace(/^hf\.co\//, "");
  const at = rest.lastIndexOf(":");
  return at === -1 ? [rest, null] : [rest.slice(0, at), rest.slice(at + 1)];
}

describe("a pull string, as hf takes it", () => {
  it("splits Ollama's spelling into the two halves hf wants", () => {
    // `hf.co/…:Q4_K_M` is what Ollama pulls. `hf` takes a repo and a filter separately.
    expect(split("hf.co/unsloth/Qwen3-8B-GGUF:Q4_K_M")).toEqual([
      "unsloth/Qwen3-8B-GGUF",
      "Q4_K_M",
    ]);
  });

  it("knows a repository is not a quantisation", () => {
    // Downloading "the repository" would fetch every quantisation — 300 GB for a 27B model.
    // The caller refuses instead, so this has to report the absence rather than guess one.
    expect(split("hf.co/unsloth/Qwen3-8B-GGUF")).toEqual([
      "unsloth/Qwen3-8B-GGUF",
      null,
    ]);
  });

  it("does not mistake a scheme's colon for the split", () => {
    // The last colon is the one that matters; a repository path has none of its own.
    expect(split("hf.co/user/repo-v1.5:UD-Q4_K_XL")).toEqual([
      "user/repo-v1.5",
      "UD-Q4_K_XL",
    ]);
  });
});
