> **ARCHIVED 2026-07-27. This is the FOUNDING BRIEF of the project. It is superseded in parts (its four-phase roadmap by ROADMAP.md and docs/Milestones, its Expectations for Claude by CLAUDE.md, its agent pipeline by CHARACTER_BIBLE.md and ADR-0017, its platform section by ADR-0001/ADR-0002) but it is not wrong: it is where Epoch started. Kept unchanged below the line.**

---

# AI Operating System Vision

> Philosophy: **AI should never be harder to use than a video game.**

## Mission
Build a desktop AI Operating System that unifies cloud models, local models and AI coding assistants into one beautiful, simple experience. Users should not need Docker, YAML, complex workflows or dozens of configuration steps.

## Core Principles
- Simplicity first.
- Great defaults.
- Local-first when possible.
- Provider-agnostic.
- Transparent agent collaboration.
- Beautiful, inspiring UI.

## Platform
- Windows 11 development environment.
- Desktop application.
- Backend: Rust.
- Frontend: React + TypeScript.
- Desktop shell: Tauri.

## Vision
This is **not** another workflow builder.

It is an AI Operating System that manages:
- Projects
- Agents
- Models
- Memory
- Knowledge
- Files
- Git
- Terminal
- Context
- Costs
- Automations

## Agent Philosophy
Agents are predefined roles instead of requiring users to build them.

Example pipeline:

User
→ the Researcher (architect / planner)
→ the Coordinator (implementation / coordination)
→ the Guardian (debugging / review)
→ the Historian (documentation)
→ User

Each agent owns:
- Role
- Prompt
- Tools
- Memory
- Personality
- Preferred model

Users may replace the underlying model without changing the workflow.

## Providers
Support both API and local execution.

Examples:
- OpenAI
- Anthropic
- Gemini
- Ollama
- OpenWebUI
- LM Studio
- OpenRouter
- Claude Code
- Codex CLI

Each provider should expose the same abstraction:
- Send message
- Stream response
- List models
- Tool support
- Vision
- Embeddings (where available)

## Automations
Simple visual automations, not node spaghetti.

Example:
Implementation finishes
→ the Guardian reviews
→ the Historian documents
→ Git commit
→ Notify user

## UI Direction
Inspired by SNES JRPGs (without copying copyrighted assets):
- Hub world
- Party of agents
- Quests instead of tasks
- Timelines
- Pixel-inspired charm with modern glass UI

## First-run Experience
The app should automatically detect:
- Ollama
- Claude Code
- Codex CLI
- OpenWebUI

Then ask only for optional API keys.

Goal:
From install to productive in under 5 minutes.

## Development Workflow
We will use **Obsidian** inside the project folder as our "second brain".

Claude should continuously help maintain:
- Architecture decisions
- ADRs
- Feature specs
- Meeting notes
- Roadmaps
- Ideas
- Research
- Technical debt
- TODOs

Documentation is a first-class deliverable.

## Roadmap

### Phase 1
- Project shell
- Tauri + React + Rust
- Unified chat
- Provider abstraction
- Ollama detection
- OpenAI + Anthropic APIs

### Phase 2
- Agent system
- Prompt manager
- Shared memory
- Project management

### Phase 3
- Automations
- Claude Code integration
- Codex CLI integration
- Git
- Terminal
- Context sharing

### Phase 4
- JRPG-inspired world
- Timelines
- Advanced memory
- Plugin ecosystem

## Expectations for Claude
Claude acts as a senior software architect.

Priorities:
1. Maintain clean architecture.
2. Avoid overengineering.
3. Prefer extensibility.
4. Keep documentation updated.
5. Explain important design decisions.
6. Suggest better approaches when appropriate.
7. Preserve the product vision.

Whenever uncertain, optimize for user simplicity rather than technical cleverness.



> Note (2026-07-25): character names are supplied by the active World Pack; the engine knows only archetypes (ADR-0017). The official Epoch cast is a deferred milestone.
