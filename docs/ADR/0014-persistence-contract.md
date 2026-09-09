# ADR-0014: Persistence Contract & Workspace Portability

- Status: Accepted
- Date: 2026-07-23
- Depends on: [[0013-scope-model]], [[0006-conversation-domain-model]], [[0010-knowledge-engine]], [[0011-definition-runtime]]
- Related architecture: [[Persistence]], [[Scope Model]]
- Horizon: lifecycle categories + repository contract + Workspace model + export/import **IMPLEMENT NOW**; storage-tech selection + migration framework **DESIGN NOW**; cloud sync / multi-device **VISION**

## Context
Persistence must be a property of the domain, not of a database. We should model what persists and how it lives before choosing SQLite or any storage layout. Users should think in Workspaces, not databases.

## Decision
Classify every canonical object by **lifecycle**, and express persistence as a **contract** separate from storage technology.

**Lifecycle categories:**
- **Persistent** (source of truth): Definitions, Conversations, Knowledge, Projects, Settings, Trust Policies -> `PersistentStore`.
- **Snapshot** (optionally serialized/restored): Character Instances, Automation Runs -> `SnapshotStore`.
- **Ephemeral** (never stored, dies with the Session): Composed Context, Streaming State, Execution Queue, Active Tool Calls, Cancellation Tokens -> no store.

**Capability is not persistent.** Profiles are runtime-declared (ADR-0005); only capability-related *preferences* persist, as Settings. Context is ephemeral; the **Context Report** travels on the `ContextComposed` Activity; the Knowledge Engine decides whether it becomes a persistent KnowledgeObject.

**Ports/adapters:** the Persistence Contract (scope-aware repositories + the Workspace aggregate) is tech-agnostic. A **Storage Adapter** (SQLite, filesystem/vault, ...) implements it. Internal First for storage: the engine speaks repositories in domain terms, never SQL.

**Workspace portability:** a Workspace is an exportable/importable aggregate representing a complete AIOS environment (Definitions, Knowledge, Conversations, Files, Snapshots, Settings) without exposing the underlying storage.

**Storage-technology selection is deferred (DESIGN NOW).**

## Why this is best
- Persistence as a domain property keeps the model portable and testable; storage becomes a swappable detail.
- Lifecycle categories prevent accidentally persisting ephemeral runtime state or treating declared Profiles as stored truth.
- Workspace-first matches the user mental model and enables clean export/import.

## Alternatives considered
- **Persistence as a DB property** (schema-first) - couples domain to storage, kills portability. Rejected.
- **Persist everything uniformly** - would store ephemeral runtime state + stale Profiles. Rejected.
- **Choose SQLite now** - premature; violates "model the Workspace before choosing storage." Deferred.

## Consequences
- Repositories are scope-aware (ADR-0013).
- Snapshot format for Instances/Automation Runs must be defined in the Domain Kernel (InstanceState).
- Export/import format is a projection of the persistent stores.

## Assumptions
- The three lifecycle categories cover all canonical objects.
- A single Storage Adapter can back all persistent categories for Phase 1.

## Risks
- Storage choice later constrains performance. *Mitigation:* contract isolates it; adapter swappable.
- Migration across schema/adapter changes. *Mitigation:* versioned models (Domain Kernel) + migration framework (DESIGN NOW).

## Open questions
- Phase-1 Storage Adapter (likely SQLite + vault files) - decide at build time.
- Canonical Knowledge store shape (table vs event log) - carried from ADR-0010.
- Export/import file format + Application-scope exclusion.

---

## Clarification (applied 2026-07-25): Definitions have one store
Definitions are **Persistent**, and their **Storage Adapter is the vault filesystem**. The [[Definition Registry]] **is** the repository implementation for Definitions over that adapter - it is not a parallel store.

There is exactly one source of truth for a Definition: its file in the vault, reached through the Persistence Contract. This is what makes hot-reload coherent: the file is the editing surface (Obsidian), and the repository watches it. Internal First is unaffected - the engine still speaks repositories in domain terms, never file paths.

---

## Amendment (2026-08-09): Quest documents are independently durable

The filesystem adapter now stores the aggregate that owns the work as one document per Quest:

```text
vault/worlds/<world>/quests/<quest-id>.json
```

There is deliberately **no parallel `quests.json` index**. A master copy plus per-Quest copies
would create two sources of truth and a synchronisation failure. The active Quest is process
state; after a restart, the adapter selects the most recently changed Quest document. History is
an on-demand projection of the folder, never a prerequisite for opening the active conversation.

The alpha `quests.json` format migrates once, before normal loading: every Quest is written to
its independent destination, the active legacy Quest is written last, and the source is renamed
to `quests.legacy-v1.json` as an auditable backup. A destination that already exists is compared
instead of overwritten, so a partial migration cannot silently replace a newer Quest.

Writes use a recoverable replacement protocol: serialise and parse `<id>.json.next`, retain the
former file temporarily as `<id>.json.previous`, promote the verified new file, then clean up.
The next open finishes that protocol or reports malformed recovery material rather than inventing
an empty history. This is a filesystem implementation detail, not a requirement that the Domain
Kernel know about paths or JSON.

This removes the monolithic read/write cost from the active turn and handover. It is not yet a
claim that every large-history surface is free: paging, header caching and virtualised rendering
remain performance work to be measured before Epoch makes its 1,000-Quest promise.
