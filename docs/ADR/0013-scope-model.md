# ADR-0013: Scope Model (Application / Workspace / Project / Session)

- Status: Accepted
- Date: 2026-07-23
- Depends on: [[0011-definition-runtime]], [[0010-knowledge-engine]]
- Related architecture: [[Scope Model]], [[Persistence]]
- Horizon: Application/Workspace/Project/Session + reference-not-copy **IMPLEMENT NOW**; override/fork mechanics + two-tier global library **DESIGN NOW**

## Context
ADR-0011 requires Character Definitions reusable across projects, but a Workspace was described as "a complete AIOS project." These need a scope hierarchy so Definitions, Knowledge, Conversations and Settings each have a clear home, reuse is natural, and Projects stay isolated.

## Decision
Four nesting scopes: **Application > Workspace > Project > Session.**

- **Application** (machine-specific, not portable): installed Providers, Integrations, Plugins, Global Settings. Runtime Capability Profiles are declared here (not stored).
- **Workspace** (the portable "world"): Character Definitions, Workspace Knowledge (reusable architecture/research/templates), Prompt Library, Templates, shared Automations, shared Workflows. **The unit of portability.**
- **Project** (an "adventure", belongs to a Workspace): Conversations, Files, Snapshots, Project Knowledge (ADRs, PRDs, timelines, execution history, decisions), Context Reports, Timeline, Project Settings.
- **Session** (ephemeral runtime): live executions. Character Instances live in a Session; their snapshots persist to the Project.

Rules:
- Projects **reference** Character Definitions, never copy them.
- Custom behavior = explicit **override/fork** that preserves the original Definition.
- Knowledge splits by scope: **Workspace Knowledge** (reusable) vs **Project Knowledge** (adventure-specific). Both are canonical KnowledgeObjects; only scope differs.
- The **Application/Workspace boundary is the portability line**: a Workspace imports onto any Application.

## Why this is best
- Preserves ADR-0011: Definitions belong to the Workspace, so they travel between Projects.
- Clean JRPG mental model: Characters travel between quests; Projects are adventures; the Workspace is the world.
- Portable Workspaces without leaking storage details (see ADR-0014).

## Alternatives considered
- **Workspace = Project (1:1)** - forces copying Definitions between workspaces; breaks ADR-0011. Rejected.
- **Two-tier global library + per-Workspace** - best reuse but most scoping complexity (global vs referenced vs local, versioning). Kept as DESIGN NOW evolution, not Phase 1.

## Consequences
- Every persistent object carries a scope; repositories are scope-aware (ADR-0014).
- Settings split: Global (Application) + Project Settings; Trust Policies default at Workspace with Project overrides.
- Project System (box) and Config/Settings must honor this hierarchy.

## Assumptions
- Reference-not-copy + override/fork covers Phase-1 customization needs.

## Risks
- Override/fork drift (forks diverge from originals). *Mitigation:* explicit fork lineage; DESIGN NOW mechanics.
- Scope leakage (project data bleeding to workspace). *Mitigation:* scope-tagged objects; repository enforces scope.

## Open questions
- Override/fork lineage + update propagation.
- Whether a Workspace export includes all its Projects or is selectable.
