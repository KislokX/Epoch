# Trust Engine

> Status: Designing · Owner ADR: [[../ADR/0009-trust-engine]] · Horizon: mixed (see below)

## Purpose
Turn safety into **understandable autonomy**. Govern side-effecting execution so users understand and feel comfortable trusting the system — "explainable, not magical."

## Core concept — trust as a relationship
Trust binds **Agent × Provider × Tool × User** and can evolve. Not a binary per-tool flag.

## Capability declarations (kernel descriptors)
Every Executable Capability declares: **Side Effects · Risk Level · Required Permissions · Explainability · Preview Support · Undo Support**.

## Explain-before-act
Before any side-effecting execution, AIOS states: **what will happen · why · expected outcome · risks · undo?**

Approval scopes: **Approve Once · Always Allow for this Agent · Always Allow for this Project · Never Allow.**

## Autonomy modes
- **Explorer** — read-only, no side effects.
- **Builder** — normal dev; medium-risk needs confirmation.
- **Autonomous** — trusted env; approved agents run high-impact ops without repeated prompts, per policy.

## NOT responsible for
- Executing anything (Execution Engine does; Trust returns a verdict).
- Storing history/audit (Knowledge Engine + Persistence).

## Communication
- Execution Engine asks Trust for a verdict before side effects.
- Emits approval/denial/execution as Knowledge Objects ([[Knowledge Engine]]).
- UI renders the explain-before-act prompt from capability descriptors.

## Earn Complexity / Runway
- IMPLEMENT NOW: explain-before-act, approval scopes, Explorer + Builder modes, per-relationship policy store.
- DESIGN NOW: full relationship-trust evolution, Autonomous mode.
- VISION: adaptive/learned trust scoring.

## Risks
- Prompt injection escalating trust. *Mitigation:* trust changes require explicit in-app user action; observed/tool content NEVER grants permission.
- Prompt fatigue. *Mitigation:* scoped Always-Allow + autonomy modes + risk tiering.

## Open questions
- Concrete trust-evolution mechanism (manual now; scoring is VISION).
- Undo model: compensating action vs. snapshot/restore.
