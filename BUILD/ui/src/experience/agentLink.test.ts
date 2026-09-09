import { describe, expect, it } from "vitest";

import { agentLink } from "./agentLink";
import type { AgentStatus } from "../ipc/launcher";

const base: AgentStatus = {
  id: "gemini",
  kind: "gemini",
  name: "Gemini CLI",
  installed: true,
  version: "0.56.0",
  lookedIn: "gemini.cmd",
  note: null,
  signedIn: null,
  account: null,
  method: null,
};

describe("what an agent's status means", () => {
  it("keeps installed and signed-in apart, because the fixes are different", () => {
    expect(agentLink({ ...base, installed: false }).state).toBe("absent");
    expect(agentLink({ ...base, signedIn: false }).state).toBe("signedOut");
  });

  it("names a sign-in it could read but not verify, and lights the lamp for it", () => {
    // Gemini CLI has no command that reports this and asking anyway is a billed prompt — but
    // the method the user chose is readable. Configured is more than nothing.
    const link = agentLink({ ...base, method: "Gemini API Key" });
    expect(link.state).toBe("ready");
    expect(link.detail).toBe("Gemini API Key");
    expect(link.lit).toBe(true);
  });

  it("does not promote a confirmed session down to a guessed one", () => {
    // ONLINE is what an agent earns by answering `auth status`. READY is weaker on purpose, and
    // an agent reporting a real account must not end up wearing the weaker word.
    const link = agentLink({ ...base, signedIn: true, account: "someonex@gmail.com" });
    expect(link.state).toBe("online");
    expect(link.detail).toBe("someonex@gmail.com");
  });

  it("still shrugs when there is nothing at all to read", () => {
    // No reading beats an invented one, and the lamp stays dark: an unlit row is honest about
    // knowing nothing, while a lit one would claim something was checked.
    const link = agentLink(base);
    expect(link.state).toBe("unknown");
    expect(link.label).toBe("—");
    expect(link.lit).toBe(false);
  });

  it("does not let an account stand in for a method", () => {
    // The bug this file was extracted after: an agent reporting a real account with an
    // unreadable session looked identical to one reporting a readable choice and no account.
    // They are different questions and only the second one is READY.
    expect(agentLink({ ...base, account: "someone@example.com" }).state).toBe("unknown");
  });

  it("prefers the agent's own words for why it is absent", () => {
    // A measured reason always beats a written one.
    const link = agentLink({
      ...base,
      installed: false,
      note: "`gemini.cmd` is not on this machine's PATH",
    });
    expect(link.detail).toContain("PATH");
  });
});
