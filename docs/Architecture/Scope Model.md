# Scope Model

> Status: Designed · Owner ADR: [[../ADR/0013-scope-model]] · Horizon: mixed (see ADR)

Foundational addressing hierarchy. Persistence, Projects, Settings, Knowledge and Trust all depend on it.

## The four scopes (nesting)
**Application > Workspace > Project > Session**

- **Application** (machine, not portable) - installed Providers, Integrations, Plugins, Global Settings. Runtime Capability Profiles are declared here (not stored).
- **Workspace** (the portable "world") - Character Definitions, Workspace Knowledge, Prompt Library, Templates, shared Automations, shared Workflows. **Unit of portability.**
- **Project** (an "adventure", inside a Workspace) - Conversations, Files, Snapshots, Project Knowledge (ADRs, PRDs, timelines, execution history, decisions), Context Reports, Timeline, Project Settings.
- **Session** (ephemeral runtime) - live executions; Character Instances live here; snapshots persist to the Project.

## Rules
- Projects **reference** Character Definitions, never copy.
- Custom behavior = explicit **override/fork** preserving the original.
- Knowledge splits by scope: Workspace Knowledge (reusable) vs Project Knowledge (adventure-specific). Both canonical KnowledgeObjects.
- Settings: Global (Application) + Project Settings. Trust Policies default at Workspace, overridable at Project.
- **Application/Workspace = the portability line.** A Workspace imports onto any Application.

## Mental model (JRPG)
Characters travel between quests. Projects are adventures. The Workspace is the world. The Application is the console.

## Cross-refs
Persistence + portability: [[Persistence]]. Definitions: [[Definition Runtime]]. Knowledge scoping: [[Knowledge Engine]].

## Risks
- Override/fork drift -> explicit fork lineage (DESIGN NOW).
- Scope leakage -> scope-tagged objects; repository enforces scope.

## Open questions
- Fork lineage + update propagation.
- Whether a Workspace export includes all Projects or is selectable.
