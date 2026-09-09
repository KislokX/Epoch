# ADR-0004: Provider / Tool / Integration trichotomy

- Status: Accepted
- Date: 2026-07-22
- Depends on: [[0003-engine-presentation-separation]]
- Related architecture: [[Provider Layer]], [[../ARCHITECTURE]]

## Context
Agents need to talk to LLMs, run capabilities (git, terminal, filesystem, docker), and connect to external AI apps (Claude Code, Codex, Cursor, LM Studio, OpenWebUI, MCP servers). A single generic "Tool" abstraction would collapse three very different responsibilities that evolve on different clocks. MCP is rising but still moving.

## Decision
Model **three independent first-class concepts**, each with its own interface:

- **Providers** — LLM backends. Send/stream a Conversation, list models, publish capability Profiles.
- **Tools** — capabilities the engine invokes (Git, Terminal, Filesystem, Docker, HTTP...).
- **Integrations** — external apps/agents, **auto-discovered**, exposing their own capabilities (Claude Code, Codex, Gemini CLI, Ollama, OpenWebUI, LM Studio, MCP servers).

**MCP is one integration mechanism behind the Integration interface — not the architectural foundation.**

## Why this is best
- Different change-rates isolated: a provider API change never touches tools; an ecosystem app never touches the provider contract.
- Enables Steam-style auto-discovery per category (detect installed integrations, expose capabilities, connect in a few clicks).
- Keeps AIOS free as MCP evolves — MCP can be swapped or extended without an architectural rewrite.

## Alternatives considered
- **MCP-native foundation** — bets the whole architecture on a still-moving standard. Rejected.
- **Single generic Tool trait** — collapses three change-rates into one; guarantees a painful future split. Rejected.

## Consequences
- Three registries/discovery paths to build and maintain.
- A capability model (ADR-0005) must span all three so orchestration reasons uniformly.
- Some backends blur lines (Ollama is both a Provider source and a discoverable Integration) — Integration discovers/installs, then hands a Provider config to the Provider layer.

## Assumptions
- The three categories stay conceptually distinct as the ecosystem grows.
- Auto-discovery is feasible for the target integrations (well-known install paths / CLIs / ports).

## Risks
- Boundary ambiguity (Provider vs. Integration for local runtimes). *Mitigation:* rule — Integration = discovery/lifecycle; Provider = protocol translation. A local runtime can appear in both roles via a thin handoff.

## Open questions
- Discovery mechanism per integration (filesystem probe, known port, CLI presence) — design in the Integration subsystem.
