# ADR-0027: Brains — a character thinks with a model or works with an agent

- Status: Accepted (amended 2026-08-02, from building the split)
- Date: 2026-08-01
- Depends on: [[0003-engine-presentation-separation]], [[0005-capability-first-architecture]], [[0007-provider-abstraction]], [[0008-execution-engine]], [[0009-trust-engine]], [[0025-quests-own-the-work]], [[0026-characters-are-portable-assets]]
- Amends: [[0007-provider-abstraction]] (a Provider is no longer the only thing that can think for a character), [[0009-trust-engine]] (the gate now has to reach work Epoch does not run)
- Related architecture: [[../../PRODUCT_ARCHITECTURE]], [[../../CLAUDE]]
- Horizon: the `Brain` split, the Agent contract, the permission bridge and Claude Code **IMPLEMENT NOW**; Codex and agent-supplied skills in History **DESIGN NOW**; agents as Quest *stages* rather than as a character's brain **VISION**

## Context — the question that broke the existing shape

ADR-0026 settled what a character *is*: identity, canonical parameters, requested capabilities,
and a `Mind { provider, model }` naming who thinks for them. Every Provider behind ADR-0007 has
the same shape — hand it a canonical `Conversation`, receive text and tool calls, and **Epoch
owns the loop**: Epoch composes the context, Epoch judges every call, Epoch runs the capability,
Epoch records the evidence.

Then a user asked for something that does not fit: use **Claude Code** and **Codex** as a
character's brain, both as plain models *and* as the agents they are, with their own tools and
skills.

The second half is not a Provider with extra features. A coding agent is handed a goal and then
works — it plans, it reads, it edits, it runs commands, it asks the user for permission, and some
minutes later it is done. **The loop is its own.** Dressing that up as `take_turn(Conversation)
-> Answer` would be a lie the type system would help us tell: every caller would believe Epoch
was still driving.

There is also a commercial fact that shapes the whole decision. **A ChatGPT plan has no API.**
Anthropic and OpenAI both sell API access separately from their subscriptions, and the only
sanctioned way to spend a subscription programmatically is each vendor's own CLI signed in with
that account (`claude setup-token`, `codex login`). So "let a character use the plan I already
pay for" is not a Provider question at all — it is an *agent* question, and it arrives whether
or not the architecture was ready for it.

## Decision

### 1. A character has a **Brain**, and a Brain is one of two kinds

```rust
pub enum Brain {
    /// It thinks. Epoch owns the loop, the tools, the trust and the record.
    Model { provider: ProviderId, model: String },
    /// It thinks *and works*. The agent owns the loop; Epoch owns the gate and the record.
    Agent { agent: AgentId, model: String },
}
```

This replaces `Mind { provider, model }` as the field, keeps its job, and adds the distinction
that actually changes behaviour. It stays in the vault Definition and travels into every World
(ADR-0023), because which brain a character uses is part of who they are.

The two kinds are **visible in the interface**, not inferred. `Claude Code — as model` and
`Claude Code — as agent` are two entries, because they do different things and hiding that would
be the magic this product exists to avoid.

### 2. `Provider` is unchanged. `Agent` is a second, honestly different contract

ADR-0007 stands exactly as written for models. The new contract is not a variant of it:

```rust
pub trait Agent: Send + Sync {
    fn probe(&self) -> Presence;                  // installed? signed in? what can it do?
    fn models(&self) -> Vec<String>;              // what the CLI reports, never a constant
    fn work(&self, task: Task, watching: &mut dyn FnMut(Step)) -> Result<Done, AgentError>;
}
```

`work` takes a goal and a Project Root and streams [`Step`](0008-execution-engine.md)s — the same
`Step` the Terminal and the World already render. That reuse is the point: an agent's work must
be *indistinguishable in kind* from Epoch's own, or the World would have two vocabularies for
one thing.

### 3. The permission bridge is mandatory, and it is the reason MCP comes first

**An agent that runs its own permission prompt turns ADR-0009 into decoration.** A character set
to `Manual` would be writing files while the user's dropdown said it asks first. That is not a
degraded experience; it is a false statement about the user's own machine.

So an agent may only be offered *as an agent* when its permission decisions can be routed back
into Epoch's `decide()`. Both CLIs expose that as an **MCP tool** they call instead of prompting
(`--permission-prompt-tool` and its equivalent), which means:

> **Epoch must be an MCP server before Epoch can host an agent.**

The MCP server was previously scheduled as a nice-to-have for interoperability. It is now the
security precondition for this ADR, and the roadmap is reordered accordingly.

### 4. When the bridge is unavailable, the agent is offered **as a model only**

Never silently, and never as a working agent with someone else's gate:

```
◆ Claude Code    ONLINE · model only
  this version cannot route permissions to Epoch — offered as a model
```

Capability is detected at runtime, not assumed from a version number, and a CLI that stops
supporting the bridge loses the agent kind on the next probe. **An agent that quietly stopped
asking for permission and carried on working is the worst failure this system has**, so the
design prefers losing half the feature to concealing it.

### 5. An agent's work enters the record, or the World lies

Everything the agent does is projected through the paths that already exist: `Step::Using` /
`Step::Said` / `Step::Used` for the Terminal, and `Entry::Produced` for the Chronicle when
something durable was made. A file the agent wrote is evidence exactly as if a capability had
written it (ADR-0025).

Evidence remains **what was produced**, never what was narrated — including when the narration
comes from somebody else's agent. An agent that reports having done ten things and leaves one
file behind produced one thing.

### 6. Credentials are never entered in Epoch

The user runs `claude setup-token` or `codex login` themselves, in the vendor's own flow. Epoch
detects the result and never sees, stores or asks for the password. This is the same line
ADR-0026 draws for API keys, arriving from the other direction: there is no field to put it in.

## Why this is best

- **It names the difference instead of disguising it.** One trait per behaviour means a caller
  cannot mistake an agent for a model, and the compiler enforces which one it is holding.
- **Nothing already built is invalidated.** As a model, Claude Code and Codex are ordinary
  Providers behind ADR-0007, and every capability, gate, Composer rule and Terminal line built
  so far applies unchanged.
- **The gate survives contact with foreign work.** The bridge is what lets a user keep one mental
  model of permission across three very different kinds of brain.
- **It answers the subscription question honestly.** "Use the plan I already pay for" works
  through the vendor's sanctioned path, and only that path.

## Alternatives considered

- **Agent as a `Provider` with a flag.** Every call site would have to know which flag it was
  holding, and the flag would eventually be wrong somewhere. Rejected: it hides the one
  distinction that changes what happens.
- **Agent as a Quest *stage* rather than a character's brain.** Architecturally attractive —
  ADR-0025 already says the Quest owns work — and it is where this probably ends up. Rejected
  *for now* under Earn Complexity: the user's request is "let Mage think with Claude Code", and
  building a second execution topology before anyone has asked for one is speculation. Kept as
  VISION, and the `Agent` trait is deliberately shaped so a Quest could call it later.
- **Let the agent keep its own permission prompt.** Rejected: see §3. It is not a trade-off
  between convenience and safety; it makes a visible control mean nothing.
- **Reverse-engineer the ChatGPT web endpoints to spend a subscription.** Rejected outright.
  It violates the vendor's terms, risks the user's account, and would break continuously. Epoch
  does not ship a mechanism whose reliability depends on not being noticed.
- **Ship mcpo (MCP→OpenAPI proxy) to reach MCP servers.** Rejected: it exists because OpenWebUI
  speaks OpenAPI and needed a translator. Epoch speaks MCP natively from Rust, and adding a
  Python process is precisely the infrastructure `CLAUDE.md` forbids the user to learn. Its
  OAuth 2.1 handling is read as a *reference* — mechanisms, never code.

## Amendment 2026-08-02 — the split, as built

`Brain` exists in the Kernel and `Mind` carries it. Three things the ADR did not decide, decided
by writing it:

**The discriminant is which name is present, not a `kind` field.** `provider` for a model,
`agent` for an agent, exactly one of them, and both-or-neither is refused rather than guessed at.
The consequence is the good one: **there is no migration.** Every character file written before
agents existed already says what it means, because nothing about the old shape was wrong. The
alternative — `kind = "model"` on every file for the overwhelmingly common case — would have been
a line the user has to read and never has a reason to change.

**`provider()` returns an `Option`, not a string that might be an agent id.** A call site
reaching for a Provider is made to notice there might not be one. That is the enum earning its
keep at the only three places it matters: preparing a turn, resuming after approval, resuming
after a grant. All three now say *"X is set to work with an agent, and agents are not built
yet"* rather than resolving to something the user did not choose.

**What a form does not edit, it does not touch.** The Launcher edits models, so it sends no
provider for a character who works with an agent — and "no provider" already meant "the user
cleared the field". Renaming somebody would have silently taken their brain away. A non-model
brain is now carried through an edit untouched, with a test named after the hazard.

The Characters panel shows an agent brain as a **cold instrument**: framed, unlit, naming what it
is and why it cannot be asked anything. Not an empty dropdown, and not an option in the model
list — offering a kind nothing can resolve would be a dead control.

### It also fixed a defect the split exposed

Loading a character validated its provider against `KNOWN_PROVIDERS = ["ollama"]`. That was
correct when a provider id *was* a kind, and became wrong the moment backends were configurable
(ADR-0026): a backend the user named `desk` would have been rejected as a typo. Worse, it
contradicted ADR-0023 — a character travels between machines, and arriving somewhere its backend
is not installed leaves it **unable to think, never invalid**. Refusing the file would delete
somebody from the roster for being away from home. The check is gone; the constant survives as
`DEFAULT_PROVIDERS`, describing only what exists before anybody configures anything.

## Consequences

- `Mind.provider`/`model` becomes `Brain`. Existing vault files must keep loading, so the old
  shape deserialises into `Brain::Model` and is rewritten on next save.
- The MCP **server** moves ahead of the second cloud Provider in the roadmap.
- The Provider registry gains a sibling: an Agent registry, probed the same way and reported by
  the same cold-instrument rules in both surfaces.
- Two more things can now be true and must be shown truly: *which account is paying* and
  *whether a subscription was actually reachable*. Both are measured or absent.

## Assumptions

- Both CLIs continue to expose a non-interactive mode and a permission hook. If either drops the
  hook, that agent becomes model-only by the rule already written, with no code change.
- The `Step` vocabulary is rich enough to describe foreign work. If it is not, it grows here
  rather than each agent inventing its own.

## Risks

- **CLI protocols are not APIs.** They change without notice and will break us. *Mitigation:* one
  narrow trait, runtime detection rather than version constants, and visible breakage — the same
  bargain accepted for the DuckDuckGo search backend, and stated in the same words.
- **An agent working inside the Project Root can do more than Epoch's capabilities can.**
  *Mitigation:* the bridge, plus the fact that `run_command`'s risk model already treats
  arbitrary execution as the ceiling. Epoch does not pretend to sandbox what it did not spawn.
- **Two ways to use one CLI may confuse.** *Mitigation:* the two entries say what they do in the
  list itself, and the difference is real rather than cosmetic.

## Open questions

- Does an agent get the character's canonical parameters (ADR-0026)? `temperature` is meaningful
  to a model and mostly meaningless to an agent that manages its own prompts.
- How is an agent's own skill set surfaced? A user choosing between two agents would want to
  know what each brings, and neither CLI reports that in a stable way yet.
- Where do agent sessions live? Both CLIs return a session id worth resuming, and a Quest is the
  obvious owner — but that is the VISION path above, and answering it early would decide it.
