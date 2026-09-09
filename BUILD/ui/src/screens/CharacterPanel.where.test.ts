import { describe, expect, it } from "vitest";

import { runsInTheCloud, whereItRuns } from "./CharacterPanel";
import type { ProviderStatus } from "../ipc/contracts";

const here: ProviderStatus = {
  id: "ollama",
  name: "Ollama",
  machine: null,
  endpoint: "http://localhost:11434",
  online: true,
  local: true,
  models: [],
  note: null,
};

const there: ProviderStatus = {
  id: "bridge:b1",
  // The **program**. A Bridge answered with the machine's name here until a machine could
  // offer three, and the crew editor's `Brain` list duly offered a hostname to think with.
  name: "Ollama",
  machine: "studio-mac.local",
  endpoint: "http://192.168.1.20:11500",
  online: true,
  // It costs no money and it is still somebody else's computer.
  local: false,
  models: ["gemma4:12b"],
  note: null,
};

describe("which machine a Service runs on", () => {
  it("says this PC only when the Provider says it is local", () => {
    expect(whereItRuns({ agent: null, provider: "ollama" }, [here, there])).toBe(
      "Runs on this PC.",
    );
  });

  it("names the paired machine, and that the conversation goes there", () => {
    // The line it replaced said "everything here runs on this PC" — true until a machine was
    // paired, and a lie the moment one was.
    const said = whereItRuns({ agent: null, provider: "bridge:b1" }, [here, there]);
    expect(said).toContain("studio-mac.local");
    expect(said).toContain("192.168.1.20");
    expect(said).toContain("travels there");
    expect(said).not.toContain("this PC");
  });

  it("does not name a machine that is not there", () => {
    // A backend switched off, or a machine unpaired. Saying nothing about the machine beats
    // naming one that no longer exists.
    expect(whereItRuns({ agent: null, provider: "bridge:gone" }, [here])).toContain(
      "not available",
    );
  });

  it("treats an agent as local, because that is what an agent is", () => {
    expect(whereItRuns({ agent: "claude-code", provider: null }, [here, there])).toBe(
      "Runs on this PC.",
    );
  });

  it("asks for a choice when nothing is chosen", () => {
    expect(whereItRuns({ agent: null, provider: null }, [here])).toContain("Pick what thinks");
  });
});

describe("a model that is not where it was asked from", () => {
  it("says so, however local the program running it is", () => {
    // **The assumption this field rested on, broken by a real thing.** Ollama runs on the
    // user's own computer, so its Provider is local — and `ollama pull gpt-oss:120b-cloud`
    // gives it a model it executes on Ollama's servers. Measured: that tag resolves in the
    // registry like any other, so the local daemon serves it and nothing about the Service
    // says the turn left.
    //
    // "Runs on this PC" about that is the sentence this whole field exists to keep true, said
    // about the one case where it is false.
    const said = whereItRuns(
      { agent: null, provider: "ollama", model: "gpt-oss:120b-cloud" },
      [here, there],
    );
    expect(said).toMatch(/not on this machine/);
    expect(said).not.toMatch(/Runs on this PC/);
  });

  it("leaves an ordinary local model alone", () => {
    // A narrowing, not a blanket suspicion. Everything that did run here still says so.
    expect(
      whereItRuns({ agent: null, provider: "ollama", model: "qwen3:14b" }, [here, there]),
    ).toBe("Runs on this PC.");
  });

  it("reads the tag Ollama itself uses, and nothing that merely looks like it", () => {
    expect(runsInTheCloud("gpt-oss:120b-cloud")).toBe(true);
    expect(runsInTheCloud("deepseek-v4-pro:cloud")).toBe(true);
    expect(runsInTheCloud("qwen3:14b")).toBe(false);
    // A repository whose *name* contains the word is not a cloud model.
    expect(runsInTheCloud("hf.co/somebody/cloud-model:Q4_K_M")).toBe(false);
    // And nothing assigned yet is not a cloud model either.
    expect(runsInTheCloud(null)).toBe(false);
  });
});

