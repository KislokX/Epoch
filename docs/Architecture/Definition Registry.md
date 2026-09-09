# Definition Registry

> Status: Designed · Owner ADR: [[../ADR/0011-definition-runtime]] · Horizon: IMPLEMENT NOW (load/validate/hot-reload); generic Definition trait DESIGN NOW

## Purpose
The **repository implementation for Definitions** over the vault-file Storage Adapter ([[Persistence]], ADR-0014) - not a parallel store. Definitions are authored, data-driven and hot-reloadable, realizing the ADR-0003 mandate. Knows files; not execution.

**One source of truth:** a Definition's file in the vault, reached through the Persistence Contract. The file is the editing surface (Obsidian); the repository watches it. The engine still speaks repositories in domain terms, never file paths (Internal First).

## Responsibilities
- Discover + load definition files from the vault.
- Validate against schema: prompt present? requested capabilities exist in the vocabulary? preferred models known? UI metadata well-formed?
- Watch + hot-reload on change.
- Version definitions; serve by DefinitionId.

## NOT responsible for
- Resolving or executing definitions ([[Definition Runtime]]).
- Holding instance state (Character Instance).
- Model selection (Capability System at resolve time).

## Definitions are INPUT, not Knowledge
A definition is authored config-as-data. Distinct from Knowledge Objects (emitted OUTPUT). A running Character emits AgentActivity; the definition itself is never a Knowledge Object.

## Communication
- Serves validated Definitions to the Runtime.
- Emits load/validate/reload events on the bus (UI reflects available characters).
- Depends on Domain Kernel (Definition/Character schema) + Config (vault paths).

## Horizon
- NOW: load + validate + hot-reload Character definitions.
- DESIGN NOW: generic Definition trait so Teams/Workflows/etc. load through the same Registry.

## Risks
- Invalid/broken definition files. *Mitigation:* validate on load; surface errors to UI; never crash the engine on a bad file.
- Schema drift. *Mitigation:* versioned schema + migration.

## Open questions
- File format (TOML/YAML/JSON) + folder layout in the vault.
- Precedence when a project overrides a global definition.
