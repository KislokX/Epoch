# Persistence

> Status: Designed · Owner ADR: [[../ADR/0014-persistence-contract]] · Horizon: contract + Workspace + export/import IMPLEMENT NOW; storage-tech DESIGN NOW
> Depends on [[Scope Model]].

## Purpose
Model **what persists and how it lives** as a property of the domain, before choosing any storage technology. Users think in **Workspaces**, not databases.

## Lifecycle categories
| Category | Examples | Contract |
|---|---|---|
| **Persistent** (source of truth) | Definitions*, Conversations, Knowledge, Projects, Settings, Trust Policies | `PersistentStore` |
| **Snapshot** (optional serialize/restore) | Character Instances, Automation Runs | `SnapshotStore` |
| **Ephemeral** (never stored; dies with Session) | Composed Context, Streaming State, Execution Queue, Active Tool Calls, Cancellation Tokens | none |

Notes:
- **Capability is not persistent** - Profiles are runtime-declared ([[Capability System]]); only capability *preferences* persist (as Settings).
- **Context is ephemeral**; the **Context Report** travels on the `ContextComposed` Activity, and the Knowledge Engine decides whether to derive a persistent KnowledgeObject from it ([[Knowledge Engine]]).

## Ports / adapters
- **Persistence Contract** - scope-aware repositories + the Workspace aggregate. Tech-agnostic.
- **Storage Adapter** - physical impl (SQLite, filesystem/vault, ...) behind the contract. Swappable, **deferred (DESIGN NOW)**.
- Internal First for storage: the engine speaks repositories in domain terms, never SQL.

## Workspace portability
A Workspace is an exportable/importable aggregate = a complete AIOS environment (Definitions, Knowledge, Conversations, Files, Snapshots, Settings), without exposing storage. Application scope (installed providers) is machine-specific and excluded from export.

## NOT responsible for
- Business logic (subsystems own it; they persist via repositories).
- Defining object shapes (Domain Kernel does).
- Projecting knowledge to markdown (Knowledge Engine).

## Horizon
- NOW: lifecycle categories, repository contract, Workspace/Project models, export/import, Instance snapshot.
- DESIGN NOW: Storage Adapter selection (likely SQLite + vault files), migration framework, Automation Run snapshots.
- VISION: cloud sync, multi-device, remote workspaces.

## Risks
- Storage choice constrains performance later -> contract isolates it; adapter swappable.
- Migration across schema/adapter changes -> versioned kernel models + migration framework (DESIGN NOW).

## Open questions
- Phase-1 Storage Adapter.
- Canonical Knowledge store shape (table vs event log) - from [[Knowledge Engine]].
- Export/import file format.

---

## Definitions: one store (clarified 2026-07-25)
*Definitions use the **vault-file Storage Adapter**, and the [[Definition Registry]] is their repository implementation over it - not a parallel store. One source of truth: the file in the vault, reached through this contract. The file is the editing surface (Obsidian); the repository watches it for hot-reload.