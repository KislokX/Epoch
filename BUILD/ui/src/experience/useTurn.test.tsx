/**
 * `useTurn` — the context gauge, and who it belongs to.
 *
 * The defect this file was opened for: the remembered-window fetch lived inside the effect that
 * registers the event listeners, whose only dependency is a callback with no dependencies of its
 * own. So it ran **once**, at mount, while nobody was being spoken to — and asked about `null`
 * forever after. Walking to the bridge and back blanked a number measured minutes earlier.
 *
 * The second case here is the one the first fix did not cover, and is why this is a test rather
 * than a memory: a gauge that survives a character change is describing the wrong conversation.
 */

import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, renderHook, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { EventCallback, EventName } from "@tauri-apps/api/event";

import type { QuestSummary } from "../ipc/world";
import { useTurn } from "./useTurn";

const QUEST_A: QuestSummary = {
  id: "q-a",
  title: "Quest A",
  state: "open",
  open: true,
  current: true,
  participants: ["mage"],
  producedEvidence: false,
  saidCount: 1,
  chronicle: [],
};

const QUEST_B: QuestSummary = {
  ...QUEST_A,
  id: "q-b",
  title: "Quest B",
};

type Listener = EventCallback<unknown>;

/** A tiny, explicit Tauri event bus. Tests emit only the Engine events they need. */
function liveEvents() {
  const handlers = new Map<string, Listener>();
  vi.mocked(listen).mockImplementation(
    async <T,>(name: EventName, handler: EventCallback<T>) => {
      handlers.set(name, handler as EventCallback<unknown>);
      return () => {
        handlers.delete(name);
      };
    },
  );

  return {
    emit(name: string, payload: unknown) {
      handlers.get(name)?.({ event: name as EventName, id: 0, payload });
    },
  };
}

/** Answer `remembered_context` with a window; everything else stays unanswered. */
function windowOf(per: Record<string, { used: number; budget: number } | null>) {
  vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
    if (command === "remembered_context") {
      const who = (args as { character: string }).character;
      return Promise.resolve(per[who] ?? null);
    }
    if (command === "active_quest") return Promise.resolve(null);
    // Anything else: asked, and nothing has come back. The honest default.
    return new Promise<never>(() => {});
  });
}

describe("useTurn — the context gauge", () => {
  beforeEach(() => {
    // A block body, not an expression: returning the mock from a `beforeEach` makes Vitest wait
    // on it as if it were a hook's teardown, and the whole suite times out at ten seconds.
    vi.mocked(invoke).mockReset();
    vi.mocked(listen).mockReset();
  });

  it("asks about the character being spoken to, not about nobody", async () => {
    windowOf({ mage: { used: 24_691, budget: 200_000 } });

    const { result } = renderHook(() => useTurn("mage"));

    await waitFor(() => expect(result.current.knew).not.toBeNull());
    expect(result.current.knew?.used).toBe(24_691);
    expect(result.current.knew?.budget).toBe(200_000);
  });

  it("asks again when the character changes", async () => {
    windowOf({
      mage: { used: 24_691, budget: 200_000 },
      paladin: { used: 900, budget: 8_192 },
    });

    const { result, rerender } = renderHook(({ who }) => useTurn(who), {
      initialProps: { who: "mage" },
    });
    await waitFor(() => expect(result.current.knew?.used).toBe(24_691));

    rerender({ who: "paladin" });
    await waitFor(() => expect(result.current.knew?.used).toBe(900));
  });

  it("shows no reading for somebody who has none, rather than the last one's", async () => {
    // The gauge belongs to the conversation. Carrying Mage's number over to Paladin would be the
    // instrument describing a conversation nobody is looking at — worse than reading nothing,
    // because a wrong reading is still believed.
    windowOf({ mage: { used: 24_691, budget: 200_000 }, paladin: null });

    const { result, rerender } = renderHook(({ who }) => useTurn(who), {
      initialProps: { who: "mage" },
    });
    await waitFor(() => expect(result.current.knew?.used).toBe(24_691));

    rerender({ who: "paladin" });
    await waitFor(() => expect(result.current.knew).toBeNull());
  });

  it("asks nothing when nobody is being spoken to", async () => {
    windowOf({});
    renderHook(() => useTurn(null));

    await waitFor(() => {
      const asked = vi.mocked(invoke).mock.calls.map(([command]) => command);
      expect(asked).not.toContain("remembered_context");
    });
  });

  it("keeps an off-screen Quest's lifecycle with that Quest, never the new chat", async () => {
    const events = liveEvents();
    let active: QuestSummary | null = QUEST_A;
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "remembered_context") return Promise.resolve(null);
      if (command === "active_quest") return Promise.resolve(active);
      if (command === "set_quest_aside") {
        active = null;
        return Promise.resolve(null);
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTurn("mage"));
    await waitFor(() => expect(result.current.quest?.id).toBe("q-a"));
    await waitFor(() => expect(vi.mocked(listen)).toHaveBeenCalledWith("turn:ended", expect.any(Function)));

    act(() => events.emit("turn:started", { characterId: "mage", questId: "q-a" }));
    expect(result.current.thinking).toBe(true);

    await act(async () => {
      await result.current.setAside();
    });
    expect(result.current.quest).toBeNull();
    expect(result.current.thinking).toBe(false);

    active = QUEST_B;
    await act(async () => {
      await result.current.reread("q-b");
    });
    expect(result.current.quest?.id).toBe("q-b");

    act(() => events.emit("turn:token", { characterId: "mage", questId: "q-a", token: "wrong box" }));
    act(() => events.emit("turn:ended", {
      characterId: "mage",
      questId: "q-a",
      answer: "finished A",
      error: null,
      invited: [],
    }));
    expect(result.current.writing).toBe("");
    expect(result.current.thinking).toBe(false);
    expect(result.current.quest?.id).toBe("q-b");

    active = {
      ...QUEST_A,
      chronicle: [{ who: "mage", content: "finished A", attachments: [], images: [], kind: "answered" }],
    };
    await act(async () => {
      await result.current.reread("q-a");
    });
    expect(result.current.quest?.chronicle?.[0]?.content).toBe("finished A");
    expect(result.current.thinking).toBe(false);
  });

  it("settles a shown Quest when its original speaker finishes after the user visits a colleague", async () => {
    const events = liveEvents();
    let active: QuestSummary | null = QUEST_A;
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "remembered_context") return Promise.resolve(null);
      if (command === "active_quest") return Promise.resolve(active);
      return Promise.resolve(null);
    });

    const { result, rerender } = renderHook(({ who }) => useTurn(who), {
      initialProps: { who: "mage" },
    });
    await waitFor(() => expect(result.current.quest?.id).toBe("q-a"));
    await waitFor(() => expect(vi.mocked(listen)).toHaveBeenCalledWith("turn:ended", expect.any(Function)));

    act(() => events.emit("turn:started", { characterId: "mage", questId: "q-a" }));
    expect(result.current.thinking).toBe(true);

    // The spoken words remain Mage's; only the visible lifecycle belongs to the Quest. A user
    // can visit Paladin while Mage's work is in flight without the shared Quest spinning forever.
    rerender({ who: "paladin" });
    active = {
      ...QUEST_A,
      chronicle: [{ who: "mage", content: "finished A", attachments: [], images: [], kind: "answered" }],
    };
    act(() => events.emit("turn:ended", {
      characterId: "mage",
      questId: "q-a",
      answer: "finished A",
      error: null,
      invited: [],
    }));

    await waitFor(() => expect(result.current.thinking).toBe(false));
    await waitFor(() => expect(result.current.quest?.chronicle?.[0]?.content).toBe("finished A"));
  });
  it("re-reads when work finishes after the turn that asked for it ended", async () => {
    // **ADR-0034.** A picture takes longer than the sentence that asked for it, so the Engine
    // files it minutes later and there is no `turn:ended` coming to notice. Without this the
    // evidence is on the Quest and the conversation does not show it until something else
    // happens to reload — which is a picture that exists and cannot be seen.
    const events = liveEvents();
    let active: QuestSummary | null = QUEST_A;
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "active_quest") return Promise.resolve(active);
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTurn("mage"));
    await waitFor(() => expect(result.current.quest?.id).toBe("q-a"));
    await waitFor(() =>
      expect(vi.mocked(listen)).toHaveBeenCalledWith(
        "world:finished",
        expect.any(Function),
      ),
    );

    // The job landed, and what it made is in the Quest by the time the event arrives.
    active = {
      ...QUEST_A,
      chronicle: [
        {
          who: null,
          content: "a small orange fox",
          attachments: [],
          images: [{ name: "fox.png", file: "fox.png" }],
          kind: "produced",
        },
      ],
    };
    await act(async () => {
      events.emit("world:finished", {
        characterId: "mage",
        questId: "q-a",
        what: "drawing a picture",
      });
    });
    await waitFor(() =>
      expect(result.current.quest?.chronicle?.[0]?.images?.[0]?.file).toBe(
        "fox.png",
      ),
    );
  });

  it("leaves another conversation alone when work finishes in this one", async () => {
    // The job knows which Quest asked; the person may be reading somebody else by now. Filing it
    // correctly and then redrawing the wrong conversation would be the same defect one layer up.
    const events = liveEvents();
    let active: QuestSummary | null = QUEST_A;
    let reads = 0;
    vi.mocked(invoke).mockImplementation((command: string) => {
      if (command === "active_quest") {
        reads += 1;
        return Promise.resolve(active);
      }
      return Promise.resolve(null);
    });

    const { result } = renderHook(() => useTurn("mage"));
    await waitFor(() => expect(result.current.quest?.id).toBe("q-a"));
    await waitFor(() =>
      expect(vi.mocked(listen)).toHaveBeenCalledWith(
        "world:finished",
        expect.any(Function),
      ),
    );

    const before = reads;
    await act(async () => {
      events.emit("world:finished", {
        characterId: "mage",
        questId: "q-somewhere-else",
        what: "drawing a picture",
      });
    });
    expect(reads).toBe(before);
  });
});
