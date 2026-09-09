> **ARCHIVED 2026-07-27. Superseded: its product principles moved to README.md, its architectural principles are stated in CLAUDE.md and docs/Architecture/Principles & Runway.md. Kept for provenance: this is where Internal First, Earn Complexity and the Architecture Runway were first written down together.**

---

# Product Principles

## North star
**AI should never be harder to use than a video game.**

## Product
- Simplicity beats cleverness. Great defaults beat configuration.
- Explainable, not magical. Understandable autonomy over hidden behavior.
- Local-first when possible. Provider-agnostic always.
- Steam-like onboarding: detect installed providers/models/assistants, connect in a few clicks.
- From install to productive in under 5 minutes.

## Architectural (constitution - see docs/Architecture/Principles & Runway.md)
- **Internal First** - canonical domain models internally (Conversations, Capabilities, Knowledge); everything external is a projection.
- **Earn Complexity** - complexity must solve a real problem in the current horizon; never added for imagined futures.
- **Architecture Runway** - every abstraction is classified IMPLEMENT NOW / DESIGN NOW / VISION.
- Engine owns all business logic; presentation is a thin, replaceable projection.

## Balance
Vision: 5 years. Implementation horizon: the next working release. Leave tomorrow possible while keeping today achievable.
