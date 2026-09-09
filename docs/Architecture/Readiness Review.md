# Architecture Readiness Review

> Conducted 2026-07-25, before implementation. Scope: 18 ADRs, 5 design constitutions, ~20 subsystems.
> Outcome: **FOUNDATION COMPLETE — architecture frozen.**

## Verdict

The review found **no defect requiring redesign**. Every blocker was an internal inconsistency or a missing contract detail — resolved by decision, not by new architecture. Notably, one fix **removed** a concept.

- Blockers found: 4 · **all resolved**
- Major: 5 · tracked below
- Minor: 5 · tracked below

## Blockers — resolved

**B1. ADR-0012 and ADR-0011 contradicted ADR-0015.**
ADR-0015 established that subsystems emit Activities and only the Knowledge Engine creates KnowledgeObjects, but only 0008 and 0010 were amended. 0012 still instructed emitting the Context Report as a KnowledgeObject; 0011 the resolved binding.
*Resolved:* amendments applied to 0011 and 0012. The Context Report travels on the existing `ContextComposed` Activity; the ResolvedBinding is recorded in the Instance and published on `AgentStarted`. All four amended ADRs now carry a header warning so a top-down reader cannot miss it. **No new concepts.**

**B2. Knowledge derivation could recurse.**
`KnowledgeCreated` is an Activity; the Knowledge Engine consumes Activities and produces KnowledgeObjects — an unbounded loop with nothing to stop it.
*Resolved:* general rule — **no self-derivation**. A consumer never derives from Activities it emitted itself. The Knowledge Engine emits `KnowledgeCreated` as `Visibility: Internal` and excludes its own emissions. **Uses existing Activity metadata; no new concepts.**

**B3. Token streaming through the Activity Stream.**
`TokenStreamed` was listed as an Activity, fanned out to UI, Knowledge, Observability, World Simulation and Plugins — making the hottest path in the product the most expensive. It also duplicated an existing mechanism.
*Resolved:* `TokenStreamed` **removed** from the Activity vocabulary. Token streaming uses the `StreamingEvent` channel already defined in the Domain Kernel (ADR-0006) and returned by `Provider::stream` (ADR-0007): Provider → Instance → UI. The Activity Stream carries only coarse turn-level facts (`ProviderRequestStarted` / `ProviderRequestCompleted`). Streaming is the response to a Command, not an observation. **Removes a concept; closes the backpressure open question.**

**B4. Definitions had two sources of truth.**
ADR-0014 made repositories authoritative; the Definition Registry claimed to be the source of truth over vault files with hot-reload. No tie-breaker.
*Resolved:* Definitions use the **vault-file Storage Adapter** (already permitted by ADR-0014), and the Definition Registry **is** their repository implementation over it — not a parallel store. One source of truth: the file in the vault, reached through the Persistence Contract. This is what makes hot-reload coherent. **No new concepts; Internal First unaffected.**

## Major — tracked, not blocking

**M5. Presence vs work ordering.** The Design Guide demands "characters never teleport" and "conversations happen in places"; it also demands "immersion never costs productivity." When work starts before a character has arrived, these conflict. *Recommended resolution (to confirm during the Walking Skeleton): work never waits for presence; presence is a projection catching up to work.*

**M6. Resolution timing.** ADR-0011 resolves at spawn; ADR-0012 composes "after capability resolution" inside the turn. Per-spawn or per-turn is unstated — it determines whether a dead provider kills the Instance or only the turn.

**M7. Provider selection ownership.** ADR-0007 says Orchestration selects, but Orchestration does not exist in Phase 1; ADR-0011 says the Character Runtime resolves. To reconcile when Orchestration is designed.

**M8. ExecutionContext contract.** Capabilities are caller-agnostic (ADR-0008) but Trust evaluates Agent × Provider × Tool × User (ADR-0009). The object carrying caller identity, scope and autonomy mode to Trust is unspecified.

**M9. Canonical Knowledge storage.** Open in both ADR-0010 and ADR-0014. The Knowledge Engine is IMPLEMENT NOW; the shape must be chosen at build time.

## Minor — tracked

- **m10.** Four independent kernel vocabularies (Capability, World concepts, KnowledgeType, Activity type) each versioned separately — consider one unified versioning policy.
- **m11.** ADR-0008's "Providers and Integrations become Executable Capabilities over time" creates two invocation paths with no horizon marked. Mark VISION or drop.
- **m12.** `PresenceProfile` scope unspecified (Workspace? Project? per-archetype or per-Instance?).
- **m13.** Autonomy mode scope unspecified (Trust Policies have one; the mode does not).
- **m14.** `AI_Operating_System_Vision.md` superseded by the five constitutions — drift risk; treat as historical.

## What passed without objection

No circular dependencies in the subsystem graph — the Kernel depends on nothing and everything points inward. Four suspected over-abstractions were checked (Observability↔Activity Stream, Registry↔Runtime, Simulation↔UI, Trust↔Execution) and each is justified by differing change-rates or headless requirements. Internal First holds end to end. The identity / presence / execution separation is clean. The causality rule (ADR-0018) is the system's strongest structural defence.

## Standing scope risk

~15 subsystems are IMPLEMENT NOW before any user-facing feature ships. Earn Complexity governs abstractions, not horizon volume. Mitigation adopted: **[[../Milestones/Walking Skeleton]]** — prove the spine with one thin vertical slice before building breadth.

---

## Freeze

The architecture is **frozen** as of 2026-07-25.

From this point, architectural change requires **implementation evidence**, not speculation. Every ADR amendment must cite what the build actually revealed. The Build Phase is the primary source of learning.
