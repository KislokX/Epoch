/**
 * What the Launcher keeps across a trip to a World.
 *
 * The defect these assertions exist to prevent: the Launcher unmounts on the way into a World
 * and mounts again on the way back, so its backend survey — which reaches every paired machine
 * over the network — started from nothing every time. For the first seconds after each return
 * `providers` was empty, and **the machine dropdown in the crew editor is derived from it**.
 *
 * Seen while driving the editor: a paired MacBook that was granted Compute and answering was not
 * shown as offline, it was *absent*, and the list read `["", "This PC"]`. An empty gauge is a
 * reading nobody can act on yet; a missing option is a machine nobody can choose at all.
 */

import { describe, expect, it } from "vitest";

import { byMachine, machinesWith } from "./CharacterPanel";
import type { ProviderStatus } from "../ipc/contracts";

function row(over: Partial<ProviderStatus>): ProviderStatus {
  return {
    id: "ollama",
    name: "Ollama",
    online: true,
    local: true,
    endpoint: "http://127.0.0.1:11434",
    models: [],
    note: null,
    ...over,
  } as ProviderStatus;
}

const here = row({});
const lent = row({
  id: "bridge:one",
  name: "Ollama",
  local: false,
  endpoint: "http://192.168.1.20:11500",
  machine: "studio-mac.local",
});

describe("the machine list", () => {
  it("names a lent machine from what the Engine said, not from its address", () => {
    // A Bridge is told the machine's real name at pairing time, and a name a person chose beats
    // a host parsed out of an IP.
    expect(byMachine([here, lent]).map((one) => one.machine)).toEqual([
      "This PC",
      "studio-mac.local",
    ]);
  });

  it("loses the lent machine entirely when the survey is empty", () => {
    // **The defect, stated as a test.** This is what the crew editor was handed for the first
    // seconds after every return from a World: no bridge rows, so no machine to pick. It is not
    // a wrong reading — it is a missing option, which is why the fix is to keep the last survey
    // rather than to soften what an empty one means.
    expect(machinesWith([], [{ id: "claude-code", name: "Claude Code", installed: true }])).toEqual(
      ["This PC"],
    );
  });

  it("offers a lent machine as soon as one row for it exists", () => {
    // Whether it is answering is a separate reading, carried on the row itself. Paired and
    // reachable are two facts, and only the second one is allowed to change from moment to
    // moment.
    expect(machinesWith([here, lent], [])).toContain("studio-mac.local");
  });
});
