# Knowledge Engine

> Status: Designing · Owner ADR: [[../ADR/0010-knowledge-engine]] · Horizon: kernel + markdown projection IMPLEMENT NOW; other projections DESIGN NOW / VISION

## Purpose
Make the system's activity **canonical knowledge**, not ad-hoc documentation. Same "Internal First" pattern as the Conversation Domain Model: stable internal representation, external formats are projections.

## Knowledge Kernel (IMPLEMENT NOW — lives in [[Domain Kernel]])
- **KnowledgeObject, KnowledgeType, Metadata, Relationships, References, Lifecycle, Projection interface.**
- Types: ArchitectureDecision, ToolExecution, ProviderInvocation, AgentActivity, AutomationRun, ProjectMilestone, ResearchFinding.

Every subsystem **emits Knowledge Objects** instead of writing docs directly.

## Projection Engine
- **Phase 1 (IMPLEMENT NOW):** Markdown · Obsidian folder structure · Wikilinks.
- **Deferred (DESIGN NOW / VISION):** Knowledge Graph · Semantic Memory · Vector Search · Timeline · JSON · HTML · API · Documentation Website. Built only when a real consumer exists.

## Responsibilities
- Own the KnowledgeObject contract + lifecycle.
- Project objects → target formats (pure functions of the objects).

## NOT responsible for
- Business logic that produces the events (subsystems do; they just emit).
- Long-term byte storage (Persistence) — Knowledge Engine defines shape, Persistence stores.

## Communication
- Subsystems call `emit(KnowledgeObject)`.
- Projection Engine renders to the vault (Phase 1) and future targets.
- Analogy: Providers translate external protocols → Conversation Model; Knowledge Engine translates internal Knowledge Objects → human/machine projections.

## Earn Complexity
Ships now as "emit object → markdown into vault." No graph/vector store built until a consumer needs it — retrofit-free because capture is structured from day one.

## Risks
- Schema too rigid. *Mitigation:* extensible Metadata + typed core; version schema.
- Projection drift. *Mitigation:* projections are pure functions; regenerate.

## Open questions
- Canonical object storage (SQLite table vs. event log) — decide with Persistence.
- Whether ADRs/arch docs themselves become generated projections (VISION).

---

## Amendment (ADR-0015)
The Knowledge Engine is a **consumer of the [[Activity Stream]]**. It subscribes to immutable Activities and derives persistent KnowledgeObjects (e.g. from ToolExecutionCompleted, ContextComposed, TrustDecisionMade). Subsystems no longer call `emit(KnowledgeObject)`; they emit Activities. Knowledge = what is worth remembering, derived from what happened.

### No self-derivation
The Knowledge Engine emits `KnowledgeCreated` with **Visibility: Internal** and **excludes its own emissions from its input filter**. Deriving knowledge from knowledge-creation events would recurse without bound. Derivation flows one way.