/** The Manual permission prompt must survive the event/snapshot race at World startup. */

import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { EventCallback, EventName } from "@tauri-apps/api/event";

import { type AgentBridge, type AgentQuestion, useAgentDoor } from "./useAgentDoor";

type Listener = EventCallback<unknown>;

const QUESTION: AgentQuestion = {
  character: "mage",
  capability: "write_file",
  what: "Mage wants to use write_file",
  preview: '{\n  "file_path": "notes.txt"\n}',
  standing: true,
};

const CLOSED_BRIDGE: AgentBridge = {
  url: null,
  token: null,
  character: null,
  question: null,
};

function liveEvents() {
  const handlers = new Map<string, Listener>();
  vi.mocked(listen).mockImplementation(
    async <T,>(name: EventName, handler: EventCallback<T>) => {
      handlers.set(name, handler as EventCallback<unknown>);
      return () => handlers.delete(name);
    },
  );
  return {
    emit(name: string, payload: unknown = null) {
      handlers.get(name)?.({ event: name as EventName, id: 0, payload });
    },
  };
}

describe("useAgentDoor", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it("does not lose a fresh Manual question to an older empty bridge snapshot", async () => {
    const events = liveEvents();
    let resolveBridge: (bridge: AgentBridge) => void = () => undefined;
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "agent_bridge") {
        return new Promise<AgentBridge>((resolve) => {
          resolveBridge = resolve;
        });
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useAgentDoor());
    await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("agent_bridge"));

    act(() => events.emit("agent:asking", QUESTION));
    expect(result.current.question).toEqual(QUESTION);

    await act(async () => resolveBridge(CLOSED_BRIDGE));
    expect(result.current.question).toEqual(QUESTION);
  });

  it("shows a recovered pending question and removes it only when the agent settles", async () => {
    const events = liveEvents();
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "agent_bridge") {
        return Promise.resolve({ ...CLOSED_BRIDGE, question: QUESTION });
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useAgentDoor());
    await waitFor(() => expect(result.current.question).toEqual(QUESTION));

    act(() => events.emit("agent:settled"));
    expect(result.current.question).toBeNull();
  });

  it("answers the held question through Epoch's dedicated command", async () => {
    liveEvents();
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "agent_bridge") return Promise.resolve({ ...CLOSED_BRIDGE, question: QUESTION });
      if (command === "answer_agent") return Promise.resolve(true);
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useAgentDoor());
    await waitFor(() => expect(result.current.question).toEqual(QUESTION));

    await act(async () => result.current.answer(true, true));
    expect(invoke).toHaveBeenCalledWith("answer_agent", { approve: true, always: true });
    expect(result.current.question).toBeNull();
  });

  it("recovers a Manual question when its one asking event was missed during a live turn", async () => {
    const events = liveEvents();
    let bridge = CLOSED_BRIDGE;
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "agent_bridge") return Promise.resolve(bridge);
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useAgentDoor());
    await waitFor(() => expect(vi.mocked(invoke)).toHaveBeenCalledWith("agent_bridge"));
    expect(result.current.question).toBeNull();

    // No `agent:asking` is emitted. This simulates the exact WebView listener race that used
    // to leave Manual cancelled on screen with no Epoch approval prompt.
    bridge = { ...CLOSED_BRIDGE, question: QUESTION };
    await act(async () => events.emit("turn:started", { characterId: "mage", questId: "q1" }));
    await waitFor(() => expect(result.current.question).toEqual(QUESTION));

    bridge = CLOSED_BRIDGE;
    await act(async () => events.emit("turn:ended", { characterId: "mage", questId: "q1" }));
    await waitFor(() => expect(result.current.question).toBeNull());
  });
});
