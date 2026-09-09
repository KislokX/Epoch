# Architectural Principles & Runway

> Standing principles for AIOS. Part of the constitution. Every ADR references its Horizon.

## Internal First
Every subsystem communicates using **canonical domain models**. Everything leaving the Engine is a **projection**. Internal representations stay stable; external formats are disposable views.

Applies to:
- **Conversations** - Conversation Domain Model ([[../ADR/0006-conversation-domain-model]]); providers translate.
- **Capabilities** - capability Profiles ([[../ADR/0005-capability-first-architecture]]); engine reasons in capabilities, not names.
- **Knowledge** - Knowledge Objects ([[../ADR/0010-knowledge-engine]]); markdown/graph/etc. are projections.
- **Definitions** - Definitions resolved by the Runtime ([[../ADR/0011-definition-runtime]]).

## Runtime over Configuration
Behavior emerges from structured **Definitions interpreted by the Runtime**, not hardcoded ([[../ADR/0011-definition-runtime]]).

## Context is composed, never concatenated
Context emerges from structured Context Blocks composed into a canonical Conversation - never string assembly ([[../ADR/0012-context-composer]]).

## Commands cause; Activities record
Synchronous **Commands** make things happen (a caller needs a response). Asynchronous **Activities** report that something happened (fire-and-forget, immutable) via the [[Activity Stream]] ([[../ADR/0015-activity-stream]]). Observability, knowledge, automation and plugins subscribe to Activities without coupling to the core. No subsystem may depend on an Activity after emission; repositories remain the source of truth.

## The Engine owns reality; the UI owns animation
The Engine is authoritative for all observable state, including presence ([[../ADR/0018-world-simulation]]). The UI renders and interpolates between authoritative states - **animation never creates information**, it only visualises information the Engine already produced. No client ever invents reality.

## A Snapshot preserves continuity; it never defines reality
Snapshots capture *where a projection currently is*, never what is true. They are always disposable: if a snapshot is missing, lost or incompatible, the system reconstructs from canonical state and continues. Repositories remain the source of truth (ADR-0014).

## Earn Complexity
Complexity must solve a **real problem in the current implementation horizon** while still allowing future evolution. No abstraction added merely for imagined future use.

## Architecture Runway - three horizons
- **IMPLEMENT NOW** - required for the first shippable release.
- **DESIGN NOW** - contracts/interfaces defined today, implementation deferred.
- **VISION** - influences design today, no implementation yet.

Every abstraction states: why it exists, its horizon, its concrete value in that horizon, what future decisions it enables.

## The balance
- Long-term vision: **5 years.** Implementation horizon: **the next working release.**
- Rule: *leave tomorrow possible while keeping today achievable.*
