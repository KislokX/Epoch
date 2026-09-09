# ADR-0012: Context Composer (Context is composed, never concatenated)

> **AMENDED by [[0015-activity-stream]]** - read the Amendment at the end of this file before implementing. Text above the Amendment may be superseded.

- Status: Accepted
- Date: 2026-07-23
- Depends on: [[0005-capability-first-architecture]], [[0006-conversation-domain-model]], [[0010-knowledge-engine]], [[0011-definition-runtime]]
- Related architecture: [[Context Composer]], [[Domain Kernel]]
- Horizon: Composer + priority-tier strategy + compress/drop + ~5 providers + Context Report **IMPLEMENT NOW**; remaining providers, alternate strategies, LLM-summarization **DESIGN NOW**

## Context
Each Character Instance must produce, every turn, the input for a Provider. Naively that is prompt-string concatenation — untestable, unexplainable, and it leaks raw fragments to providers, breaking Internal First. We need structured, budget-aware, explainable context assembly.

## Decision
Introduce a **Context Composer**: it composes a canonical **Conversation** from structured **Context Blocks** contributed by pluggable **Context Providers**. New principle: **Context is composed, never concatenated.**

- **Stateless.** Pure function of (Instance state + registered Context Providers + resolved Model Profile) -> Conversation + Context Report. Same inputs -> same output.
- **Context Providers** are a plug-in pipeline (same extensibility pattern as Providers/Capabilities/Definitions). Phase-1 set: Character Definition, Conversation, Available Capabilities, Goals, Files. Deferred: Memory, Knowledge Objects, Workspace State, Execution Context, Trust Context.
- **Context Block metadata:** Priority, Required, Compressible, Summarizable, Cacheable, Lifetime.
- **Pluggable Context Selection Strategy.** Phase 1 = static priority tiers. Future strategies (semantic, temporal, project, hybrid ranking) plug in without changing the Composer.
- **Reduction pipeline (deterministic):** 1) compress eligible; 2) summarize eligible (DESIGN NOW); 3) drop lowest-priority optional. **Required blocks are never removed.**
- **Model-aware budget.** Composition runs after capability resolution; the Composer reads the resolved Model Profile for context window + tokenizer.
- **Context Report** emitted every turn as a KnowledgeObject: which blocks were included / summarized / compressed / excluded, and why.

## Why this is best
- Internal First preserved end-to-end: providers receive a fully composed Conversation, never fragments.
- Deterministic + explainable (Context Report) = Understandable Autonomy; testable without a model.
- Strategy + provider plug-ins mean smarter context later with no Composer change.

## Alternatives considered
- **String concatenation** - untestable, unexplainable, leaks fragments; violates Internal First. Rejected.
- **Relevance-only selection** - needs semantic memory/vector search that do not exist yet; violates Earn Complexity. Rejected (kept as a future Strategy).
- **Hardcoded priority logic** - no extension point; rejected in favor of a pluggable Strategy.

## Consequences
- ContextBlock + ContextReport become Domain Kernel types.
- Composition is coupled to model selection order (must run after resolution) - documented in the turn loop.
- LLM-summarization is a model call inside a turn (cost/latency/recursion) - deferred to DESIGN NOW; Phase 1 uses compress + drop.

## Assumptions
- Priority tiers + compress/drop give acceptable Phase-1 context quality.
- Model Profiles expose a usable context window + token estimator.

## Risks
- Summarization recursion/cost. *Mitigation:* DESIGN NOW; Phase 1 compress+drop; summarizer behind the Summarizable flag.
- Budget estimation inaccuracy (tokenizer mismatch). *Mitigation:* use the Model Profile tokenizer; conservative margin.
- Nondeterminism. *Mitigation:* stateless Composer; Context Report records every decision.

## Open questions
- Compression technique(s) for Phase 1 (structural/whitespace vs. block-specific).
- Whether the Context Report is its own KnowledgeType or part of AgentActivity.
- Caching semantics for Cacheable blocks.

---

## Amendment (ADR-0015, applied 2026-07-25)
The Context Composer does **not** emit KnowledgeObjects. It emits the **`ContextComposed` Activity**, carrying the Context Report as its payload. The Knowledge Engine subscribes and decides whether that becomes a persistent KnowledgeObject. Corrects the original "Context Report emitted every turn as a KnowledgeObject".
---

## Amendment (2026-07-31, from building it)

### The missing facet: Provenance

The Context Block metadata above — Priority, Required, Compressible, Summarizable, Cacheable,
Lifetime — says how *important* a block is and never says **where it came from or whether to
believe it**. A page fetched from the internet and a character's own prompt were, to the
Composer, the same kind of thing.

Added: `Provenance` — `authored · project · conversation · tool · network` — and one hard rule
the Composer **enforces** rather than documents:

> **Nothing from outside the user's machine may occupy the system role.**

A block that tries is *demoted*, not dropped: the content is still useful, it simply may not
give orders. Losing it would be a different kind of wrong.

The line is not "how much do we trust the source". It is **where the content came from relative
to the boundary the user chose**. A file inside the Project Root may instruct, because pointing
Epoch at a folder is a deliberate act; it is marked with its filename wherever it appears, so a
cloned repository's conventions can never influence somebody invisibly.

This is why `Message::tool_result` has no unwrapped constructor and why `Role::Tool` exists: the
defence had to be structural, because a page that can instruct a character who can run commands
is the whole attack.

### Composition had two authors, which is the failure this ADR names

`compose_for` built most of a conversation and the shell appended four more system messages
afterwards. Every one of those messages was right on its own; together they were not a design —
no budget, no ordering rule, and no way to answer "what was she told?".

That is *exactly* the concatenation this ADR forbids, arrived at by addition rather than by
anybody deciding to concatenate. Composition is now one function producing the whole block list,
and adding to what a character knows means adding a block with a tier and a provenance.

### Phase 1 is drop-only, and that was enough

Compress and summarize are not built. Dropping — least important first, oldest first within a
tier — keeps a long Quest working, and a summariser is a model call inside a turn: cost, latency
and recursion, all of which this ADR already deferred.

Two tiers are `Required` and never dropped: **who the character is**, and **the most recent
exchange**. Losing the middle of a long discussion degrades an answer; losing who somebody is
replaces them, and losing what was just said makes them answer a question nobody asked.

If the Required blocks alone overflow, the turn goes out **over budget** and the Report says so.
A truthful over-large turn beats a turn that has forgotten who is speaking.

### The budget is honest about what it does not know

Tokens are estimated at characters ÷ 4 — the usual rough figure, and deliberately rough. The
real answer is the resolved model's own tokenizer, which Epoch does not have. The window is the
character's requested `context_tokens` (ADR-0026), and the ceiling that should cap it is what
the model reports it can hold — **not yet asked for**, so today it is the request alone.

`Budget::of` reserves room for the answer. A budget equal to the whole window leaves nowhere for
the reply, and that failure looks like the model refusing to finish a sentence.

### The Report is shown, and only when it matters

Emitted every turn, and surfaced in the conversation **only when something was left out**. A
budget nobody is near is a number that trains people to stop reading the instruments — the same
rule the Launcher's cold instruments follow.

---

## Amendment (2026-07-31, from four days of a character refusing to work)

Characters with seven working capabilities kept answering "I cannot access files". Everything
this ADR describes was correct — tools declared, blocks composed, budget respected — and the
behaviour was wrong anyway. Two facets were missing, and neither could have been reasoned to.

### A projection that drops evidence falsifies the history it replays

`Entry::Produced` — the Chronicle's record of what a capability actually did (ADR-0025) — had no
arm in the block projection. A character that had just run `git clone` successfully was asked to
read what it cloned and said it could not, because **the clone was not in what it was sent.**

What the model received was fifteen of its own refusals and not one instance of itself acting.
It was reasoning correctly from a history we had falsified by omission. Telling it harder was
never going to work.

Evidence is now projected at its real position, as `Role::Tool` / `Provenance::Tool` — never
authoritative, and unable to instruct. The record itself is untouched; ADR-0025 forbids tidying
History, and nothing here tidies it.

> **A projection may drop what does not fit. It may not drop what would change the conclusion.**

### Role is a transport decision, and it must be measured per model

Moving the capability block last was not enough. The cause took measuring rather than reasoning:
a real 45-message Chronicle was replayed to `qwen3:14b` against a live Ollama, one variable at a
time, three samples each.

| change | tool called |
| --- | --- |
| nothing (control) | none, none, none |
| reasoning enabled | none, none, none |
| temperature 0.1 / temperature 0.8 | none ×3 each |
| older answers compressed to 800 / 400 chars | none, none |
| older answers compressed to 200 chars | `list_files` ×3 |
| **capability block sent as `user` instead of `system`** | `list_files` ×3 |

Not the wording, not the position, not the sampling, not reasoning. The **channel**: for this
model a system message is weighed against thirty turns of conversation and loses.

`Role` is which channel words reach the model *as* — not a claim about who wrote them. The block
is Epoch's, `Provenance::Authored` says so, the Report names it, and no surface attributes it to
the user: the Chronicle never contains it. Provenance remains the security facet; role is now
explicitly the delivery facet, and they are not the same question.

Because this is a property of a model rather than of context, it lives in the one function that
decides what a turn looks like — never taught to each Provider (ADR-0003).

### Compression is a cliff here, not a gradient

200 characters worked; 400 and 800 did not. Compression only helped once it had destroyed the
content. A character that stops remembering what it said is not a fixed character, so **the
compress stage stays unbuilt** — the deferral in the original decision holds, now for a measured
reason rather than a suspected one.

### Instruction strength is one dial, so both ends are measured together

The capability block had been written to defeat refusal — *"if you do not know what is in a
folder, list it"*. Delivered on the user channel that stopped being encouragement and became a
second request: asked only to clone a repository, the character cloned it, read the README and
summarised it. Nobody asked.

Obedience and restraint were then measured as one change, against the poisoned Chronicle and
against a clone-and-nothing-else request whose clone had already succeeded:

| wording | obedience | restraint |
| --- | --- | --- |
| before | acts 3 of 3 | acts anyway 2 of 3 |
| after | acts 3 of 3 | stops 3 of 3 |

> **A wording that buys restraint by giving back refusal is the old bug in a new coat.** Neither
> number may be quoted without the other.

### The table is runnable now (2026-08-18)

The measurements above were made by hand, once. They are now three probes anyone can run again
— `crates/epoch-engine/tests/composer_probes.rs`, ignored by default — because a measurement
that cannot be repeated is a memory, and the next person to touch this block would otherwise
rediscover the same two ditches.

They drive the real composer, the real turn loop and a real model; only the capabilities are
stand-ins, which record being reached for and answer plausibly. The question is whether the
character *reached*, and nothing should touch a disk to answer it.

Baseline on `gemma4:12b`, three runs each, temperature 0:

| probe | asks | result |
| --- | --- | --- |
| obedience | something only a tool can answer, under a Chronicle long enough to drown a system message | looked **3 of 3** |
| restraint | a clone, and nothing else | went further **0 of 3** |
| greeting | "hola" | acted **0 of 3** |

**The first version of the restraint probe measured nothing**, and it passed. Its Chronicle
ended with the character's own reply, so nothing had been asked and reaching for nothing was
guaranteed. Restraint is a property of what happens *inside* one turn — the clone succeeds and
the character keeps going — so it is now measured as "reached for anything besides the one thing
asked for". A probe whose healthy answer is zero has to be able to reach non-zero.

One observation kept rather than acted on: asked "hola", this model answers by acknowledging its
instructions rather than by greeting back. It reaches for nothing, which is what the probe
measures, but the reply belongs to the system prompt rather than to the person who spoke. That is
a Phase 6 question — a character with a personality answering a greeting as themselves.

### Consequence for this ADR

The Context Report was justified as explainability for the user. It was not enough to explain a
turn to *us*: three rounds of screenshots could not distinguish "never told about its tools"
from "told and declined". A per-turn trace of the exact provider request now exists as a
diagnostic (one file, overwritten, beside the vault) — not a log, and not persistence.
