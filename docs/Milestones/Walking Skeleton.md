# Milestone: Walking Skeleton — the first vertical slice

> Status: **Build Phase — active** · Declared 2026-07-25, immediately after Foundation Complete.

## The rule this milestone establishes

**Complete vertical slices over horizontal subsystem completion.**

One fully working Quest travelling through the entire engine beats ten isolated subsystems with no end-to-end experience. From this point forward, implementation prioritises slices.

## This is not a technical demo

The Walking Skeleton is the **first vertical slice of the real product**. Even containing only one Character, one Provider, one Tool and one complete turn, **the user experience must already communicate the product vision**.

It must already feel like Epoch.

## The first executable flow

```
World
  ↓
"The Researcher is walking toward the Laboratory..."
  ↓
The user joins the conversation.
  ↓
The Researcher uses Ollama.
  ↓
The Researcher executes a Terminal tool.
  ↓
An Activity is emitted.
  ↓
The Knowledge Engine derives a KnowledgeObject.
  ↓
The Researcher finishes the task.
  ↓
The character walks back into the World.
```

## What this single slice validates

| Subsystem | Exercised by |
|---|---|
| [[../Architecture/Provider Layer]] | one Ollama adapter, real streaming |
| [[../Architecture/Definition Runtime]] | resolve `character.researcher` → Instance |
| [[../Architecture/Definition Registry]] | load + validate one Definition from the vault |
| [[../Architecture/Context Composer]] | compose a real turn under a real budget |
| [[../Architecture/Execution Engine]] | one Terminal capability |
| [[../Architecture/Trust Engine]] | explain-before-act on a side-effecting tool |
| [[../Architecture/Activity Stream]] | coarse turn-level Activities |
| [[../Architecture/Knowledge Engine]] | derive one KnowledgeObject from the stream |
| [[../Architecture/World Simulation]] | travel, arrival, working, idle — real presence |
| [[../Architecture/World Packs]] | placeholder default pack resolving concepts |
| UI Shell | the World as home; join an existing conversation |

Nearly the whole foundation, proven end to end.

## Deliberately out of scope

Orchestration/Automation · Memory store · Cost & Telemetry · Integration auto-discovery · multiple Characters · multiple Providers · Project System breadth · the official Epoch universe.

## Definition of done

1. The flow above runs end to end against a real local model.
2. Every visual traces to a real Activity or real engine state (Living World Design Guide checklist passes).
3. The Terminal tool execution is Trust-gated with a real explain-before-act prompt.
4. A KnowledgeObject lands in the vault as markdown, derived from the stream — not written directly.
5. Presence is queryable headless: *"where is the Researcher?"* answers correctly mid-travel.
6. Closing and reopening the app restores continuity (Snapshot), or reconstructs cleanly if the snapshot is discarded.

## Architecture discipline from here

The architecture is **frozen** (see [[../Architecture/Readiness Review]]). Changes require **implementation evidence**, not speculation. Every ADR amendment after this point must cite what the build actually revealed.

Every milestone should make the World feel more alive.
