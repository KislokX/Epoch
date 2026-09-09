# ADR-0010: Knowledge Objects & Knowledge Engine

> **AMENDED by [[0015-activity-stream]]** - read the Amendment at the end of this file before implementing. Text above the Amendment may be superseded.

- Status: Accepted
- Date: 2026-07-22
- Depends on: [[0006-conversation-domain-model]], [[0008-execution-engine]]
- Related architecture: [[Knowledge Engine]], [[Domain Kernel]]
- Horizon: KnowledgeObject contract + Markdown/Obsidian projection **IMPLEMENT NOW**; other projections **DESIGN NOW / VISION**

## Context
Documentation should not be written ad hoc. The system's activity — decisions, executions, agent runs, milestones, findings — is knowledge. If subsystems write Markdown directly, knowledge is trapped in one lossy format and can never be re-projected (graph, search, timeline).

## Decision
Every subsystem emits **structured Knowledge Objects** rather than writing documentation directly. Knowledge is the **canonical representation**; Markdown is **one projection** — the same "Internal First" pattern as the Conversation Domain Model (ADR-0006).

**Knowledge Kernel (IMPLEMENT NOW)** — define in the Domain Kernel:
- KnowledgeObject, KnowledgeType, Metadata, Relationships, References, Lifecycle, Projection interface.
- Types include: ArchitectureDecision, ToolExecution, ProviderInvocation, AgentActivity, AutomationRun, ProjectMilestone, ResearchFinding.

**Projection Engine — Phase 1 (IMPLEMENT NOW)** implements only:
- Markdown, Obsidian folder structure, Wikilinks.

**Deferred projections (DESIGN NOW / VISION)** — documented, built when a real consumer exists:
- Knowledge Graph, Semantic Memory, Vector Search, Timeline, JSON, HTML, API, Documentation Website.

## Why this is best
- Stable internal representation; external formats are disposable views — future-proof and lossless.
- Ships a usable product now (markdown into the vault) without building a knowledge graph first — honors "Earn Complexity."
- Every execution/decision becomes queryable knowledge later with no retrofit.

## Alternatives considered
- **Full Knowledge Engine now** (graph/semantic as primary storage) — large upfront system touching everything, delays a working agent. Rejected (violates Earn Complexity).
- **Defer entirely, markdown-first** — retrofitting structured capture across all subsystems later is expensive and lossy. Rejected.

## Consequences
- Subsystems depend on the Knowledge Kernel to emit objects (small, universal obligation).
- Markdown is generated from objects, not hand-written — the doc-first workflow becomes "emit object → project to vault."
- New projections are additive; no subsystem changes when one is added.

## Assumptions
- A stable KnowledgeObject schema can represent current activity + near-future types.
- Markdown projection is sufficient for humans in Phase 1.

## Risks
- Schema too rigid for future types. *Mitigation:* extensible Metadata + typed core; version the schema.
- Projection drift (object model changes, projections stale). *Mitigation:* projections are pure functions of objects; regenerate.

## Open questions
- Storage of canonical objects (SQLite table vs. event log) — decide with Persistence subsystem.
- Whether ADRs/architecture docs themselves become generated projections eventually (VISION).

---

## Amendment (ADR-0015, 2026-07-23)
Knowledge is no longer emitted by each subsystem. The Knowledge Engine becomes a **consumer of the [[Activity Stream]]**: it subscribes to immutable Activities and decides which become persistent KnowledgeObjects. Activities = everything that happened (transient); Knowledge = what is worth remembering (persistent). Persistence never subscribes to the stream.

---

## Implementation note (2026-08-18)

Built as a **vertical slice**, and two things about it differ from the text above.

**The trigger is a Quest ending, not the Activity Stream.** The Amendment says the Knowledge
Engine consumes Activities, and it will — but the stream does not exist yet, and building a
pub/sub nervous system to deliver one event to one consumer is the abstraction Earn Complexity
refuses. `KnowledgeObject::of` is a pure function of a `Quest`, so when the stream arrives only
the *caller* moves. Nothing about the object or its projection changes.

**One type, not seven.** ADR-0010 names ArchitectureDecision, ToolExecution, ProviderInvocation
and others. Only `QuestRecord` exists, because only it has a producer. Writing the rest now
would be a taxonomy rather than a contract — names nobody has had to make true. Adding a variant
when something emits it is additive.

**Where the notes go was the user's decision**, and it is the one that makes the loop close: a
subfolder of the World's **library**, the user's own Obsidian vault. The crew already reads that
folder through `search_notes` and `read_note`, so a note written after one Quest is findable
during the next one with no second store and no index. Epoch writes inside `Epoch/` and nowhere
else — asserted, not promised, because the failure mode is somebody's own notes being touched.

**Evidence decides, and silence is the honest answer.** A Quest that produced no artifacts
produces no note (ADR-0025). Most conversations are conversations, and a vault full of "the crew
discussed the login" buries the notes that mean something.

Open question from above, now answered in part: canonical objects are **not** stored separately
yet. The Quest is the record and the note is a projection of it, regenerated whenever the Quest
ends again. A separate object store earns itself when a second projection exists.
