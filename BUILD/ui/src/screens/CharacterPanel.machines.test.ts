import { describe, expect, it } from "vitest";

import { byMachine, machineOf } from "./CharacterPanel";

/**
 * The full shape is Provider · Service · Model — the machine, the program that runs it, the
 * model. It stays one control because it is one choice, and grouping is how the machine becomes
 * visible without becoming separately selectable.
 */
describe("which machine a Service is on", () => {
  it("reads the host, not the label somebody typed", () => {
    // A Bridge's name happens to be its machine. An OpenAI-compatible Service is whatever the
    // user called it — `llama.cpp`, `the studio box` — and that says nothing about where it is.
    expect(
      machineOf({ local: false, endpoint: "http://192.168.1.20:11500" }),
    ).toBe("192.168.1.20");
    expect(
      machineOf({ local: false, endpoint: "https://studio-mac.local:1234/v1" }),
    ).toBe("studio-mac.local");
  });

  it("calls the local one what somebody standing at it calls it", () => {
    // `127.0.0.1` is an address, not a place. Agents live in this group too and have no
    // endpoint at all.
    expect(machineOf({ local: true, endpoint: "http://localhost:11434" })).toBe(
      "This PC",
    );
  });

  it("puts this PC first, whatever order the Services arrived in", () => {
    // Sorting by pairing time would bury the machine somebody is standing at, which is the one
    // they mean most often.
    const grouped = byMachine([
      { id: "bridge", local: false, endpoint: "http://192.168.1.20:11500" },
      { id: "ollama", local: true, endpoint: "http://localhost:11434" },
    ]);
    expect(grouped.map((g) => g.machine)).toEqual(["This PC", "192.168.1.20"]);
  });

  it("keeps several Services on one machine together", () => {
    // The case this exists for: Ollama and LM Studio on one computer are two Services at two
    // addresses, and a flat list of names says nothing about which box they are in.
    const grouped = byMachine([
      { id: "ollama", local: true, endpoint: "http://localhost:11434" },
      { id: "lmstudio", local: true, endpoint: "http://localhost:1234" },
      { id: "mac", local: false, endpoint: "http://192.168.1.20:11500" },
    ]);
    expect(grouped).toHaveLength(2);
    expect(grouped[0]!.offers.map((o) => o.id)).toEqual(["ollama", "lmstudio"]);
    expect(grouped[1]!.offers.map((o) => o.id)).toEqual(["mac"]);
  });

  it("says something rather than nothing for an address it cannot read", () => {
    // A label is never invented, but a group with an empty name would be a heading nobody can
    // account for.
    expect(machineOf({ local: false, endpoint: "" })).toBe("Somewhere else");
  });
});
