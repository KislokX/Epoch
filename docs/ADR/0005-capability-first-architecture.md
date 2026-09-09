# ADR-0005: Capability-first architecture (Profiles)

- Status: Accepted
- Date: 2026-07-22
- Depends on: [[0003-engine-presentation-separation]]
- Related architecture: [[Capability System]], [[Provider Layer]]

## Context
Providers, models, tools, integrations and agents support wildly uneven features (streaming, vision, tool-calling, embeddings, JSON mode, long context, audio). The orchestration engine must decide what can run where without hardcoding provider or model names — which would rot as the ecosystem changes.

## Decision
Capabilities are a **first-class architectural concept**. Every major component publishes a structured **Profile** describing its capabilities, limits and characteristics:

- Providers, Models, Tools, Integrations, Agents all publish Profiles.

The orchestration engine **thinks in capabilities, never names**. Agents **request capabilities** (e.g. Vision, Tool-Calling, Streaming, Long-Context); the engine selects the most appropriate available component by capability, user preference and availability.

## Why this is best
- Enables graceful degradation instead of mid-workflow runtime failures.
- Enables automatic model/component selection and pre-execution agent validation.
- Intelligent UI: enable/disable features automatically from declared Profiles.
- Provider-agnostic and future-proof — new components integrate without special-casing.

## Alternatives considered
- **Try-and-fail at call time** — simple interface, but failures surface mid-workflow, poor UX, can't show users what a component can do beforehand. Rejected.
- **Fat interface with optional/unsupported methods** — leaks capability knowledge into every caller; interface bloats over time. Rejected.
- **Capabilities as provider-only metadata** — too narrow; tools, integrations and agents also need declared capabilities. Rejected.

## Consequences
- A shared capability vocabulary + Profile schema must live in the Domain Kernel, versioned.
- Every component author must declare an accurate Profile; stale Profiles cause wrong routing.
- Selection logic (capability request → best component) becomes a core engine service.

## Assumptions
- A finite, extensible capability vocabulary can describe what orchestration needs to decide.
- Local components can probe capabilities at runtime; remote ones carry declared Profiles.

## Risks
- Capability drift (component gains/loses a feature, Profile stale). *Mitigation:* version Profiles; runtime probing for local backends; Profiles updated with adapters.
- Vocabulary explosion. *Mitigation:* curate capabilities deliberately; prefer coarse, composable capabilities.

## Open questions
- Capability granularity (e.g. "vision" vs. "image-input + image-output") — refine per subsystem as needs appear.
- How user preferences weight against capability matches during selection.
