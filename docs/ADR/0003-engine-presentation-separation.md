# ADR-0003: Engine / Presentation separation

- Status: Accepted
- Date: 2026-07-22
- Depends on: [[0001-tauri]], [[0002-rust]]
- Related architecture: [[../ARCHITECTURE]], [[Domain Kernel]]

## Context
AIOS could put orchestration logic either in the Rust core or the React/TypeScript frontend. The product must survive many years and support future interfaces (CLI, web, mobile, headless server). Placing logic in the UI is the fastest path early — and the classic trap that turns a product into "another OpenWebUI clone" with logic welded to one screen.

## Decision
All business logic lives in the Rust core, the **AIOS Engine**: orchestration, providers, tools, integrations, memory, context, events, automations, plugins, filesystem, terminal, git. React is a **presentation layer only** and communicates **exclusively** through IPC commands and event subscriptions. The engine must be able to run headless.

To keep iteration fast despite a compiled core, prompts, workflows, automations and agent definitions are **data-driven and hot-reloadable**, not compiled into Rust.

## Why this is best
- Highest one-way door in the system — every future interface reuses the engine untouched.
- Kills logic-bleed-into-UI at the root, protecting the product identity (an OS, not a chat UI).
- Serves the north star: users see a "game," never the machinery; the machinery can be re-skinned freely.

## Alternatives considered
- **Logic in TypeScript** — fastest early iteration, but permanent lock-in to one frontend and guaranteed logic bleed. Rejected.
- **Split logic across both** — wasteful double abstraction, unclear ownership. Rejected.

## Consequences
- Frontend is thin and replaceable; a CLI/web/mobile shell is additive, not a rewrite.
- A clean IPC contract (commands + events) becomes a first-class artifact to design and version.
- Requires discipline: any "just do it in the UI" shortcut is a violation to reject in review.

## Assumptions
- Data-driven definitions keep iteration fast enough despite the compiled core.
- The IPC boundary can express everything the UI needs without leaking domain internals.

## Risks
- Chatty or leaky IPC could pull domain detail into the UI. *Mitigation:* IPC speaks in domain-kernel terms + view models, never provider/native types.
- Over-abstraction before a second frontend exists. *Mitigation:* design headless-capable, but don't build extra shells until needed.

## Open questions
- Exact IPC contract shape and versioning strategy — design with the Event/Message Bus subsystem.
