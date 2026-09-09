# ADR-0009: Trust Engine & Autonomy Modes

> **AMENDED by [[0027-brains-models-and-agents]]** — the gate must now reach work Epoch does not run. An agent may only be hosted as an agent when its permission decisions route back into `decide()`; otherwise it is offered as a model, visibly.

- Status: Accepted
- Date: 2026-07-22
- Depends on: [[0008-execution-engine]], [[0005-capability-first-architecture]]
- Related architecture: [[Trust Engine]], [[Execution Engine]]
- Horizon: explain-before-act + approval scopes + Explorer/Builder **IMPLEMENT NOW**; relationship-trust + Autonomous mode **DESIGN NOW**; adaptive trust scoring **VISION**

## Context
Capabilities touch the real machine (delete files, run shell, git push). The north star is game-like simplicity, and the philosophy is "explainable, not magical." Binary per-tool permissions are neither safe enough nor trustworthy enough. The goal is not just security — it is **understandable autonomy**.

## Decision
Execution with side effects is governed by a **Trust Engine**. Trust is a **relationship** between Agent × Provider × Tool × User that can evolve, not an isolated per-tool rule.

Every Executable Capability declares: **Side Effects, Risk Level, Required Permissions, Explainability, Preview Support, Undo Support**.

Before any side-effecting execution, AIOS explains: **what will happen, why, expected outcome, risks, whether it can be undone.**

User approval scopes: **Approve Once · Always Allow for this Agent · Always Allow for this Project · Never Allow.**

**Autonomy modes:**
- **Explorer** — read-only, no side effects.
- **Builder** — normal development; medium-risk actions require confirmation.
- **Autonomous** — in trusted environments, approved agents execute high-impact operations without repeated prompts, per the user's established trust policy.

Every approval, denial and execution becomes a Knowledge Object (ADR-0010).

## Why this is best
- Safe by default, transparent, and trust grows with use — matches "explainable, not magical."
- Autonomy modes give one legible dial for how much the system may do on its own.
- Reusing the single execution path (ADR-0008) means one gate secures everything.

## Alternatives considered
- **Trust the agent, run freely** — an LLM mistake or prompt injection runs destructive ops. Unacceptable. Rejected.
- **Sandbox everything** — fights the product; users want it to touch real projects/git/terminal. Rejected as default.
- **Binary per-tool permissions** — no evolution, no relationship context, poor UX. Rejected.

## Consequences
- Capabilities must honestly declare side-effect/risk/undo metadata (kernel descriptors).
- A trust-policy store keyed by relationship + scope is needed.
- Explain-before-act requires each capability to produce a human-readable preview.

## Assumptions
- Capability authors can classify side effects/risk accurately.
- Users understand three autonomy modes faster than fine-grained permission matrices.

## Risks
- Prompt injection escalating trust. *Mitigation:* trust changes require explicit user action in-app; observed/tool content can never grant permission.
- Over-prompting fatigue. *Mitigation:* scoped "Always Allow" + autonomy modes; risk-tiered prompting.

## Open questions
- How trust "evolves" concretely (manual scopes now; adaptive scoring is VISION).
- Undo model: per-capability compensating action vs. snapshot/restore.

---

## Amendment (2026-07-31, from implementing the capability contract)

Three corrections, each from building `epoch-kernel::capability` rather than from re-reading
this ADR.

### 1. Risk is derived, not declared

The Decision above says a capability *declares* Risk Level, and the Assumptions section flagged
the reason to doubt it: *"capability authors can classify side effects/risk accurately."*

They cannot, reliably — and the failure is worse than inaccuracy. A declared risk that
disagrees with the declared effects is **more** dangerous than no risk field, because the user
reads the number instead of the effects. `Deletes` + `Low` must not be a sentence anyone can
write.

So `Descriptor::risk()` is **computed** from the effects and the reversal. An author may raise
it with `at_least` — publishing to a remote is worse than the effects suggest — and there is
deliberately **no way to lower it**. "Trust me, deleting is fine here" is exactly the claim that
must not be expressible.

What a capability still declares: **Effects, Reversal, Parameters, Summary**. All four are facts
about itself. Risk is a judgement about consequences, and judgements belong to the one place
that can be tested.

### 2. Reaching the network is not read-only

`is_observation()` is false for a capability with `Network`, even one that only fetches. A
request is something that happened to somebody else, and what comes back is input nobody vetted.

This matters because Explorer mode is defined here as read-only. Letting "it only reads web
pages" into Explorer would put untrusted text in front of a character in the one mode the user
was promised could not hurt them.

### 3. Everything a capability returns is untrusted — and there is no flag for it

`Outcome.content` is always foreign: a file somebody else wrote, a page from the internet, the
output of a program. It reaches the model as **data, in a user-role message, wrapped** — never
in the system role.

Deliberately not a field. A `trusted: bool` would eventually be set wrongly by somebody in a
hurry, and the failure would be silent and total: a page that can instruct a character who can
run commands.

This has a consequence for **ADR-0012**, which gives Context Blocks metadata (Priority,
Required, Compressible, Summarizable, Cacheable, Lifetime) and no way to say *where this came
from or whether to believe it*. A block from a fetched page and a character's own prompt are
currently the same kind of thing to the Composer. A `Provenance` facet — authored · project ·
tool · network — is needed, and only authored content may occupy the system role. To be
recorded as an ADR-0012 amendment when the Composer is built, with the same evidence standard.

### Also settled: the schema question

ADR-0008 left open *"Schema format for Input/Output (JSON Schema vs. typed Rust + generated
schema)"*. Answered by ADR-0026's rule: canonical `Parameter` in the Kernel, rendered into
whatever a provider's tool-calling API wants **inside that provider**. The Kernel does not know
what JSON Schema is, for the same reason it does not know what `num_ctx` is.

---

## Amendment (2026-07-31, from using it)

### The autonomy ladder is renamed, and gains a rung

`Explorer · Builder · Autonomous` becomes **`Manual · Accept edits · Auto`**.

The old names describe *who the user is*. The new ones describe **what happens**, which is the
only thing somebody choosing a permission level actually wants to know. Evidence: the ladder
shipped, and the first person to meet it could not tell from the names which one would stop
before writing a file — the question every one of them exists to answer.

| | what runs without asking |
|---|---|
| **Manual** | nothing but reading. It states the action and waits. |
| **Accept edits** | reading and file changes. |
| **Auto** | everything this character was given. |

A fourth mode, `Plan`, existed for about an hour and was cut. It **refused** rather than asked,
and from the user's seat that is the same experience with one more word to learn — Manual
already plans, in the sense that matters: it says what it is about to do and stops.

What is given up is real and small: there is no longer a mode that *cannot* be clicked through.
Somebody who wants "nothing here can change, ever" now relies on not misclicking. Three modes
people understand beat four that need explaining.

Retired names still resolve on load (`plan`, `explorer`, `builder` → Manual). A rename that
bricks a stored permission setting fails in the worst direction: the file stops parsing, every
standing decision is dropped, and nothing looks wrong.

**Manual is the default**, not the middle. Anything looser is a decision about somebody's files
that they did not make.

### Accept edits is not a risk level

Every other rung is a risk ceiling. This one is about **what a capability touches**: effects
⊆ {Reads, Writes}. `write_file` is Medium risk and waved through; a hypothetical cheap
capability that also opened a socket would not be.

"Accept edits" is a promise about files. A capability that also deletes, executes or reaches
the network is not a file edit however little it costs, and somebody who ticked this box was
not agreeing to any of those.

### A mode says when to ask, never what exists

No mode shrinks the offer. Every capability a character requested and this build has is on the
table in all three; the mode decides only whether using it stops to ask first.

Taking something *off* the table is a standing `Deny`, which is a different decision made in a
different place. Keeping those two apart is what stops "be careful for a while" from silently
becoming "you no longer have this".

### Auto is freedom over the tools they have, never over which tools

A standing `Deny` still wins in Auto, and a capability the character never requested is not in
the offer to begin with. "Full freedom" is scoped to what was already granted; it is not a way
to acquire anything.

### Where the choice lives

At the bottom of the conversation, not in a settings screen. It is a decision made *while
working*, and a permission level you have to leave the World to change is one nobody changes.

**One control, not one button per mode.** A row of buttons grows every time a mode is added and
spends the width of the box on a choice made once a session.

The list of modes is sent **from the Engine**, for the same reason the archetypes are: a
hardcoded copy in a surface drifts the day a mode is added, and drift in the permission system
would look like a bug in the one place nobody should be guessing.
