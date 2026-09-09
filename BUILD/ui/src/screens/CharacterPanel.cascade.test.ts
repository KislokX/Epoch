import { describe, expect, it } from "vitest";

import { brainsOn, machineFor, machinesWith } from "./CharacterPanel";

/**
 * `Provider · Brain · Model` — the device, the program that runs the model, the model.
 *
 * The single control this replaced had a real argument behind it: a separate machine dropdown
 * lets somebody hold a machine and a Service that is not on it. These tests exist because that
 * argument is *answered* here rather than ignored — the second list is derived from the first,
 * so the invalid pair is unsayable rather than merely refused.
 */
const ollamaHere = {
  id: "ollama",
  name: "Ollama",
  machine: null,
  local: true,
  endpoint: "http://localhost:11434",
  online: true,
  models: ["qwen3:14b"],
  note: null,
};
const macOllama = {
  id: "bridge:mac",
  name: "Ollama",
  machine: "studio-mac.local",
  local: false,
  endpoint: "http://192.168.1.20:11500",
  online: true,
  models: ["qwen3:27b"],
  note: null,
};
const macStudio = {
  id: "bridge:mac:lm_studio",
  name: "LM Studio",
  machine: "studio-mac.local",
  local: false,
  endpoint: "http://192.168.1.20:11500",
  online: true,
  models: ["google/gemma-4-e4b"],
  note: null,
};
const claudeCode = { id: "claude_code", name: "Claude Code", installed: true };

describe("which machine an edit is on", () => {
  it("derives it from what they are assigned to, never from a stored copy", () => {
    // State holds user intent only. A `machine` field saved beside the provider would be a
    // second copy of a fact the provider already carries, free to disagree with it.
    expect(machineFor({ provider: "bridge:mac:lm_studio" }, [macOllama, macStudio])).toBe(
      "studio-mac.local",
    );
    expect(machineFor({ provider: "ollama" }, [ollamaHere])).toBe("This PC");
  });

  it("puts an agent on this PC, because there is no remote arrangement for one", () => {
    expect(machineFor({ agent: "claude_code" }, [])).toBe("This PC");
  });

  it("says nothing rather than naming a machine that is not there", () => {
    // Assigned to a Service that is switched off, or a machine that was unpaired. Nothing
    // chosen and nothing offered are different states, and only one of them is a lie.
    expect(machineFor({ provider: "bridge:gone" }, [ollamaHere])).toBeNull();
    expect(machineFor({}, [ollamaHere])).toBeNull();
  });
});

describe("what can think on one machine", () => {
  it("offers only the programs that are actually on it", () => {
    // The whole point: a machine and a Brain that is not on it is not refused, it cannot be
    // held — the second list does not contain one.
    const there = brainsOn("studio-mac.local", [ollamaHere, macOllama, macStudio], [claudeCode]);
    expect(there.map((b) => b.id)).toEqual(["bridge:mac", "bridge:mac:lm_studio"]);
    expect(there.some((b) => b.kind === "agent")).toBe(false);
  });

  it("keeps a machine's several programs apart, which is why this level exists", () => {
    // The MacBook serves Ollama, LM Studio and llama.cpp at once — measured. A flat list of
    // `Studio Mac` and `Studio Mac · LM Studio` said nothing about which was the computer.
    expect(brainsOn("studio-mac.local", [macOllama, macStudio], [])).toHaveLength(2);
  });

  it("puts agents on this PC and only there", () => {
    const here = brainsOn("This PC", [ollamaHere], [claudeCode]);
    expect(here.map((b) => b.kind)).toEqual(["model", "agent"]);
  });

  it("offers nothing before a machine is picked", () => {
    // A Brain list filled in from every machine at once would be the flat list this replaced.
    expect(brainsOn(null, [ollamaHere, macStudio], [claudeCode])).toEqual([]);
  });
});

describe("a Brain that is not running", () => {
  it("says so, rather than showing a name with no models under it", () => {
    // Measured on this machine: Ollama switched off, `gemma4`, `gpt-oss` and `qwen3` still on
    // disk, and the Model list simply empty. Epoch was not lying — it was silent in the one
    // place somebody was looking, which is the same failure as a gauge nobody can explain.
    const [ollama] = brainsOn("This PC", [{ ...ollamaHere, online: false, models: [] }], []);
    expect(ollama!.online).toBe(false);
  });

  it("keeps an installed agent usable, because there is no server to be down", () => {
    const [, agent] = brainsOn("This PC", [ollamaHere], [claudeCode]);
    expect(agent!.online).toBe(true);
  });
});

describe("which machines can be picked at all", () => {
  it("includes this PC for its agents even with no Service configured", () => {
    // It was the group that never formed, and its agents vanished with it.
    expect(machinesWith([macOllama], [claudeCode])).toEqual(["This PC", "studio-mac.local"]);
  });

  it("does not list this PC twice when it also has a Service", () => {
    expect(machinesWith([ollamaHere, macOllama], [claudeCode])).toEqual([
      "This PC",
      "studio-mac.local",
    ]);
  });
});
