# Domain Kernel

> Status: Designing · Owner ADRs: 0005, 0006, 0010, 0011, 0012, 0013, 0015 (see [[../ADR/_index]])
> Horizon: IMPLEMENT NOW

The innermost layer of the AIOS Engine. Pure types, **zero I/O**, zero provider knowledge. Everything else depends inward on it (hexagonal core). Home of the canonical models.

## Purpose
Define the vocabulary the whole engine speaks. If it is not in the kernel, no subsystem may assume it.

## Contents
- **Conversation model** - Conversation, Message, ContentBlock, ToolCall, ToolResult, File, Image, Audio, StreamingEvent, Thinking, Metadata, Citation, Artifact.
- **Capability model** - Capability vocabulary + Profile schema ([[Capability System]]). Execution descriptors: SideEffects, RiskLevel, RequiredPermissions, Explainability, PreviewSupport, UndoSupport, Input/Output Schema.
- **Knowledge model** - KnowledgeObject, KnowledgeType, Metadata, Relationships, References, Lifecycle, Projection interface ([[Knowledge Engine]]).
- **Definition model** - Definition (trait), CharacterDefinition, ResolvedBinding, InstanceState, InstanceId ([[Definition Runtime]]).
- **Context model** - ContextBlock (Priority, Required, Compressible, Summarizable, Cacheable, Lifetime), ContextReport ([[Context Composer]]).
- **Scope model** - Scope (Application/Workspace/Project/Session), WorkspaceId, ProjectId, SessionId; PersistenceCategory (Persistent/Snapshot/Ephemeral) ([[Scope Model]], [[Persistence]]).
- **Activity model** - Activity (id, type, timestamp, scope ids, correlation/causation id, Visibility [Public/Internal], Importance [Debug/Normal/Critical], immutable payload) ([[Activity Stream]]). Activities are transient observations, never persisted by the engine as source of truth.

- **World concept vocabulary** - character archetypes (`character.researcher|coordinator|guardian|historian`), place concepts (`knowledge_center`, `research_lab`, `automation_hub`, `command_center`, `guild`), asset + animation concepts, and music **moods**. Versioned, additive-only. The engine never knows a character name, location name or franchise ([[World Packs]], [[../ADR/0017-character-archetypes-world-packs]]).

- **Presence model** - PresenceProfile (routine, idle behavior, home place, movement affinity - authored) and PresenceState (current place, destination, progress 0..1, speed, ETA, current activity - derived) ([[World Simulation]], [[../ADR/0018-world-simulation]]). Presence is separate from identity: Definitions describe who, Presence describes where.

## Responsibilities
- Own canonical types + their invariants. Version the models; define migration policy.
- Provide the shared language for IPC view models (never leaks provider/native types to UI).

## NOT responsible for
- Any I/O, transport, storage, provider/tool/integration logic.
- Selection, orchestration, execution, resolution, composition, persistence, distribution - those depend on the kernel, not vice versa.

## Communication
- Depended upon by every subsystem; depends on nothing.
- Providers translate native <-> kernel types. Runtime resolves Definitions. Composer builds Conversations from ContextBlocks. Subsystems emit Activities; Knowledge Engine derives KnowledgeObjects. Repositories store by scope + lifecycle. UI receives kernel-derived view models over IPC.

## Future scalability
- New modality / knowledge-type / definition-type / context-provider / scope / activity-type = extend a kernel type once; components opt in. Old components keep working.

## Risks
- Model too narrow -> Extensions / rigid schemas overused -> abstraction rot. *Mitigation:* deliberate superset; extensible Metadata; review each Extension use.
- Unversioned changes break stored data/definitions. *Mitigation:* version + migration from day one.

## Open questions
- Exact StreamingEvent enum + ToolCall/ToolResult schema (before first adapter).
- KnowledgeObject + InstanceState storage shapes (with Persistence).
- Character definition file format; generic Definition trait shape.
- ContextReport as own KnowledgeType vs. part of AgentActivity.
