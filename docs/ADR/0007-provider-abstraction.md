# ADR-0007: Provider Abstraction & Registry

> **AMENDED by [[0027-brains-models-and-agents]]** — a Provider is no longer the only thing that can think for a character. This ADR remains exactly right for **models**; an *agent* owns its own loop and has its own contract.

- Status: Accepted
- Date: 2026-07-22
- Depends on: [[0004-provider-tool-integration-trichotomy]], [[0005-capability-first-architecture]], [[0006-conversation-domain-model]]
- Related architecture: [[Provider Layer]]

## Context
The Provider layer is the first spine subsystem. It must make any LLM backend interchangeable while the engine speaks only the AIOS Conversation Domain Model (ADR-0006) and reasons only in capabilities (ADR-0005).

## Decision
A Provider is a **thin translator + transport** implementing a single `Provider` trait over the Domain Kernel. It:

- translates outgoing Conversation → native request, and native response/stream → canonical types;
- publishes a Provider Profile and per-Model Profiles (capabilities, limits, context, modalities, cost);
- owns transport (auth, endpoints, retries, rate limits, timeouts) and reports health + usage as events;
- isolates provider-only features in Adapter Extensions.

A **ProviderRegistry** holds live providers and is indexed **by capability, not by name**. Multi-provider routing/fallback is itself a composite `Provider` wrapping others.

The Provider layer does **not**: choose which provider runs (Orchestration does, via capability query), own prompts/agent identity, assemble context, execute tools, store history, or account costs. It is a pure translator.

## Why this is best
- Maximum interchangeability with minimum surface.
- Serves the north star: users request *what they want* (vision / cheap / local); the engine picks the model.
- Composite-as-Provider yields routing, fallback and load-balancing for free.

## Alternatives considered
- **Fat provider** owning selection + cost + memory — god object, untestable, couples everything. Rejected.
- **Native pass-through** — violates ADR-0006. Rejected.
- **MCP-as-provider-base** — wrong layer; MCP is an Integration (ADR-0004). Rejected.

## Amendment 2026-08-02 — the first hosted Provider, and what it cost

Anthropic's Messages API is built. The claim this ADR makes — that a Provider is a *translation*
and nothing above it learns whose dialect is in use — held, and the price of holding it was four
translations that a "forward the canonical value" implementation would have got wrong:

**There is no system role in `messages`.** A system prompt is a separate top-level field, so the
Composer's `Role::System` blocks are hoisted out of the conversation rather than sent as turns.
The Composer is unchanged; it keeps emitting canonical roles.

**There is no tool role either.** Evidence replays as an ordinary user turn. Epoch records what a
tool produced as *text* (ADR-0025), so there is no `tool_use_id` to pair a structured tool result
with — and inventing one would fabricate a link the record does not contain.

**`max_tokens` is required, and its ceiling differs per model.** Measured from the Models API and
remembered per session (ADR-0026: *measured where possible*). A constant would have been a guess
that goes stale in silence, and a guess that is too high is a request the service refuses.

**The newest models reject `temperature` and `top_p` outright.** This one is the interesting one,
because the obvious fix is a table of which models reject what — correct on the day it is written
and quietly wrong after the next release. Instead: send what the character authored, and if the
service refuses and *names a field*, drop that field, retry once, and remember the refusal for the
session. One extra round trip per model per session, and the turn succeeds rather than failing
with a message about a parameter the user cannot see.

That last mechanism reads the service's own error text, which is fragile, and it is fragile on
purpose in the same way the DuckDuckGo search backend is: when the wording changes the retry stops
helping and the original refusal surfaces **visibly**, carrying the service's own words. A silent
table would have failed invisibly instead.

### What did not need to change

`Provider` itself. No new method, no new variant, no flag. `Surface` returned no controls — the
honest answer for a backend with nothing on this machine to tune — and `release` kept its default,
because there is no memory of the user's to free and a call that looked like it did something
would be worse than one that plainly does not.

### One thing this Provider cannot honour

`context_tokens` asks the model to hold *less* than its window, and this API has nowhere to say
that. It is not sent. The Composer already honours the authored value where it does its work
(ADR-0012), so sending it here would be a second and contradictory place — but it is worth naming
as the first canonical parameter a Provider genuinely cannot translate, which is the case this ADR
said would be *said* rather than pretended.

## Consequences
- Each backend is one adapter file; adding a backend needs no engine change.
- Streaming and tool-call translation are the hardest adapter work — canonical shapes must exist first.
- Registry + capability selection become core engine services.

## Assumptions
- The Domain Kernel superset (ADR-0006) and capability vocabulary (ADR-0005) are rich enough for real adapters.

## Risks
- Streaming shape divergence across providers. *Mitigation:* design the canonical StreamingEvent enum before the first adapter; adapters conform.
- Capability/Profile drift. *Mitigation:* version Profiles; runtime probing for local backends.
- Tool-call semantic differences (parallel calls, IDs). *Mitigation:* canonical ToolCall/ToolResult owns the contract; adapters map into it.

## Open questions
- Retry/rate-limit policy: per-adapter vs. shared engine middleware.
- Where cost data originates when providers don't report token usage.
