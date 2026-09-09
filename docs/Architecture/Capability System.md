# Capability System

> Status: Designing · Owner ADR: [[../ADR/0005-capability-first-architecture]]

Cross-cutting kernel concept. The engine reasons in **capabilities, never names**.

## Purpose
Let every component declare what it can do, and let orchestration/agents request what they need — so selection, validation, degradation and UI are all driven by declared facts instead of hardcoded provider/model names.

## Profiles
Every major component publishes a structured **Profile**: capabilities, limits, characteristics.
- Provider Profile, Model Profile, Tool Profile, Integration Profile, Agent Profile.

## Responsibilities
- Define the capability vocabulary + Profile schema (lives in [[Domain Kernel]]).
- Provide capability-based **selection**: request (e.g. Vision + Tool-Calling + Long-Context) → best available component, weighted by user preference + availability.
- Provide pre-execution **validation** (can this agent run with the available components?).

## NOT responsible for
- Executing anything (Providers/Tools/Integrations do).
- Storing Profiles long-term (Persistence) or rendering them (UI reads results).

## Communication
- Reads Profiles from every registry (Provider/Tool/Integration/Agent).
- Orchestration + Agent Runtime query it to select/validate.
- UI queries derived capability facts to enable/disable features.

## Future scalability
- New capability = extend vocabulary once; components opt in. No special-casing.

## Risks
- Capability drift (stale Profile → wrong routing). *Mitigation:* version Profiles; runtime probing for local backends.
- Vocabulary explosion. *Mitigation:* curate coarse, composable capabilities.

## Assumptions
- A finite, extensible vocabulary can describe what orchestration must decide.

## Open questions
- Capability granularity.
- How user preferences weight against capability matches during selection.
