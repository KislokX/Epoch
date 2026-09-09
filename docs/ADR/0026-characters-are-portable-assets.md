# ADR-0026: Characters are portable assets; Providers declare their own surface

- Status: Accepted (amended 2026-08-02, from building the secret store)
- Date: 2026-07-30
- Depends on: [[0005-capability-first-architecture]], [[0007-provider-abstraction]], [[0011-definition-runtime]], [[0014-persistence-contract]], [[0016-asset-resolution]], [[0023-characters-belong-to-the-user]]
- Related architecture: [[../../PRODUCT_ARCHITECTURE]], [[../../CHARACTER_BIBLE]], [[../../CONTENT_PHILOSOPHY]]
- Horizon: canonical parameters, requested capabilities, the declared Provider surface and the General/Advanced split **IMPLEMENT NOW**; Providers as configured entities with credentials, Character Packs, import/export **DESIGN NOW**; a character marketplace **VISION**

## Context — the evidence

ADR-0023 made a character's name, face, personality, role and routine live in the vault and travel into every World. Building the per-character model assignment added `Mind { provider, model }` to that Definition and proved the shape works: "Mage always uses Qwen, Robo always uses Claude" is one field, and swapping it changes nothing about who Mage is.

Then the question arrived that the shape did not answer: **where does `temperature` live?**

Three candidate answers were each wrong in an instructive way.

**1. On the Provider.** Then Mage and Paladin — both on Ollama — could not differ. That contradicts the thing per-character models had just established: two specialists on one backend are still two specialists.

**2. On the Character, using the backend's own names.** The first proposal was literally `{ "temperature": 0.7, "top_p": 0.9, "num_ctx": 32768 }`. `num_ctx` is an Ollama parameter name. It does not exist in Claude and is called something else in LM Studio. That object smuggles a provider into the Kernel — the exact failure the same proposal was rejecting two lines below, where it correctly refused to let `num_gpu` and `use_mlock` near a character.

**3. One parameter set for every Provider.** Rejected on product grounds before architectural ones. A local backend exposes a large tuning surface and hiding it makes Epoch worse than the tools it replaces; a hosted provider exposes a small opinionated one and inventing knobs for it would produce controls that do nothing. Both directions end in a control that lies.

The resolution is that there is not one question here. There are two, and they have different answers.

## Decision

### 1. Two vocabularies, separated by one test

> **Does this survive changing the engine?**

**Canonical Character parameters** — a small closed set in the Kernel, meaningful to any backend, present or future:

```
temperature      how much the character improvises
top_p            the same, by a different mechanism
context_tokens   how much history they are asked to hold
reasoning        Off · Low · Medium · High · Max — how much they deliberate
```

These are **behavioural identity**. A Guardian at 0.2 and a Researcher at 0.9 differ in who they are, not in what hardware they run on, and that difference must survive moving them from Ollama to a hosted provider. Each Provider translates them into its own API — Ollama emits `num_ctx`, another emits whatever it calls the same idea. Internal First, applied to parameters.

`reasoning` earns canonical status by appearing independently on both sides of the divide: a local backend has a think flag, a hosted one has effort levels. The concept is cross-provider and it is character identity. The **ladder** is canonical; each Provider maps it onto whatever its API actually offers, read from that API rather than assumed.

**Provider-native tuning** — an open set, namespaced by provider, meaningful only to that backend:

```toml
[mind.tuning.ollama]
num_ctx = 32768
repeat_penalty = 1.1
```

The Kernel never interprets these. They are opaque to everything except the Provider that declared them.

### 2. The Provider declares the surface; the Character holds the value

A Provider declares its own configuration surface at runtime — a list of controls, each with a name, a label, a kind (number with bounds, toggle, choice, text), a default and a help line. The UI renders that list generically.

The UI must not know that Ollama has `num_ctx`. If it did, adding a Provider would mean editing the frontend, and ADR-0003 puts no logic in the presentation layer. This is Runtime over Configuration applied to a Provider: behaviour emerges from a declaration, not from a hardcoded form.

**Where possible the surface is measured, not declared.** Ollama can report a model's real context length, so the bound on `context_tokens` is a fact about the loaded model rather than a constant we guessed. A control whose limits are invented is the same defect as an instrument with nothing behind it.

The **values** live on the Character, because they are per-character. The Provider owns the vocabulary and the schema; the Character owns the setting.

Changing a character's Provider **keeps** the old namespace rather than deleting it — moving Mage back restores his tuning. While dormant it is shown as dormant. Never silently applied, never silently destroyed.

### 3. Requested capabilities, never declared ones

A character does not declare what it can do. `vision = true` in a file does not give a blind model sight; it produces a lie that fails when somebody sends an image.

ADR-0005 already settled this — capability is not persistent, Profiles are runtime-declared — and ADR-0011 already had the right name: **Requested Capabilities**. This ADR only forces the vocabulary to be used.

The Character expresses **intent**: what it wants, and what it is allowed to use. The resolved Provider answers what is **available**. A surface shows both, so a request that cannot be met reads as an unmet request rather than breaking at the moment it matters.

### 4. Providers are configured entities, and credentials never enter the vault

A Provider gains identity and configuration of its own: endpoint URL, credentials, and the resource settings that belong to a machine rather than to a person — `num_gpu`, `num_thread`, `use_mmap`, `use_mlock`.

The load-bearing reason is not portability. It is that **a Character Pack must be structurally incapable of containing an API key.** If authentication lived on the Character, sharing a crew would leak a credential, and the defence would be discipline. With this split the field does not exist on that type, and the guarantee is one the compiler holds — the same bargain that keeps I/O out of `epoch-kernel`.

Credentials are therefore not vault files. They are Application scope (ADR-0013), stored where secrets are stored, and excluded from export by construction rather than by a filter.

### 5. Character Packs reuse the Pack contract

A crew is shared as a Pack with a manifest, exactly as a World is (ADR-0016) — identity, version, author, and **mandatory licence metadata**. A folder of bare character files has no identity and no licence, and inventing a second packaging format for the same idea is how a vocabulary rots.

Import **renames on collision, never overwrites**. `CharacterId` is unique in the registry; a pack must not be able to replace somebody the user created.

CONTENT_PHILOSOPHY's hard rule applies to what Epoch distributes, and applies again at export.

### 6. TOML is the source of truth; JSON is the exchange projection

The vault keeps TOML: a Definition is authored, hand-edited, commented, and the existing hot reload already watches it.

Export and packs are JSON. ADR-0014 already authorises this — *"Export/import format is a projection of the persistent stores"* — so this is the Domain → Projection pattern the whole system already uses, not a second store.

**Unknown keys survive a round trip.** A pack authored against a newer Provider must not be destroyed by an older Epoch reading and rewriting it.

### 7. General, and Advanced ▼

The default is that the user touches nothing. Advanced is collapsed, and it holds the Provider-declared surface — large for a local backend, small for a hosted one, and native in both cases because it came from the Provider rather than from a shared lowest common denominator.

## Why this is best

- The test — *does this survive changing the engine?* — is mechanical, so the boundary can be applied by anyone later without re-deriving the argument.
- The Character stays portable without making every backend look identical. Portability was never the same as uniformity, and treating them as one produced both of the rejected options.
- Secret containment becomes a type property instead of a review checklist.
- Nothing in a Character file claims an ability. Consistent with cold instruments, with `is_placeholder`, and with a World that never lies.

## Alternatives considered

**One parameter schema for all Providers.** Rejected: either it hides a local backend's real power, or it invents controls a hosted one does not have.

**All parameters provider-namespaced, no canonical set.** Coherent, and it loses behavioural identity: moving Mage to another backend would reset who he is. Temperature is personality; `num_ctx` is a resource decision.

**Provider-native tuning stored on the Provider.** Rejected in §2 — Mage and Paladin must be able to differ on one backend.

**Frontend knows each Provider's controls.** Rejected: logic in the presentation layer, and a new Provider would require a UI change.

**Move the vault to JSON for a single format.** Considered and declined; it costs comments in files the user edits by hand. Revisit only if maintaining two serialisations proves worse than that.

## Consequences

**Good**

- "Mage always uses Qwen, the Historian always uses DeepSeek, Robo always uses Claude" is a property of those characters, expressible without changing what they are.
- A crew becomes shareable content, which makes Epoch a platform for specialists the way World Packs make it one for places.
- The Composer's token budget gains an authored input: the character's requested `context_tokens`, clamped by what the model actually reports. Requested by the user, capped by measurement.

**Costs**

- Two serialisation formats to maintain.
- Every Provider must now declare a surface, so adding one is more work than a `take_turn` implementation.
- A canonical set that stays small will occasionally be wrong at the margin, and moving a parameter from native to canonical later is a migration.

## Assumptions

- The canonical set stays small. Growth is the signal that the boundary is being applied wrongly.
- A declared control list is expressive enough for the backends we care about. If one needs a control kind that does not fit, the list grows a kind — it does not gain an escape hatch to raw HTML.

## Risks

- **Canonical parameters that do not translate cleanly.** `top_p` is not identical everywhere. *Mitigation:* the Provider translates and the Context Report can say what it sent; a Provider that cannot honour a parameter says so rather than pretending.
- **A pack that names a Provider the user does not have.** *Mitigation:* imports as a character whose Mind is unresolved — visible and fixable, exactly like a character with no model today.

## Amendment 2026-08-02 — where credentials actually went

The open question below is answered, and building it moved one decision.

**Windows DPAPI, into `vault/secrets.dat`, encrypted to the logged-in account.** Not the
Credential Manager: it is meant for one credential per target with a username beside it, and this
is a small map keyed by our own names. DPAPI gives the same account-scoped key material without
that shape. The consequence worth stating is one we *want* — the file is unreadable on a
different machine or under a different account, so a credential does not travel with a backup.

Being exact about the ceiling, because a security claim that overstates is worse than none:
**code already running as this user can decrypt it**, since Windows decrypts for anything that
user runs. Every password manager on this platform has that same ceiling. What is removed is the
ordinary leak — plaintext in a repository, a sync folder, a screenshot, a support paste.

Two things the implementation added that the ADR did not anticipate:

**Nothing is cached.** Read, decrypt, use, drop. A decrypted map held for the life of the process
puts every key in the memory dump of a crash report, which is the kind of thing only discovered
afterwards.

**There is no command that reads a key back.** The type guarantee (`Secret` has no `Serialize`)
would have been undone by a `get_backend_key` command returning a `String`, so the surface is
told *that* a key exists and never what it is. The credential field is empty every time it opens
— and that is not the value being hidden, it is the value not being there. A masked field
showing dots for something the interface does not have would misrepresent where the key lives.

**Non-Windows refuses in words rather than falling back to a plain file.** A store that silently
degrades to plaintext is worse than no store, because the user believes the first thing they were
told.

The first user of it is not a hosted Provider: it is **Ollama reached across a network**, behind
a reverse proxy that asks for a bearer token. That is the same case that made backend `id` and
`kind` separate questions, and it meant the whole path — type, store, provider, surface — could
be proved before a hosted Provider exists to need it.

## Open questions

- ~~Whether `context_tokens` is genuinely canonical or a resource decision in disguise.~~ **Answered
  2026-08-31 — see the amendment above.** It was a resource decision in disguise. The concept was
  right and the unit was wrong: what belongs to the character is `context_policy`, not a window.
- Whether a Character Pack may carry artwork, and therefore whether it is a folder or a single file. Same question ADR-0023 left open for World export.

---

## Amendment, 2026-08-31 — the physical window is not the character's

**This ADR's own open question said so first:**

> Whether `context_tokens` is genuinely canonical or a resource decision in disguise. It survives
> an engine change *as a concept*, which is why it is here, but it is the weakest member of the
> set.

It is a resource decision in disguise, and building the Models deck proved it. The decision is
refined rather than reversed: the *concept* was right and the *unit* was wrong.

### What changed, and what measured it

`llama-server` takes `--ctx-size` when it **spawns the child that holds the model**. Measured
2026-08-30: rewriting a preset and unloading is not enough — the child keeps the argv it was born
with. So the window is a property of the loaded instance, and **two characters sharing one model
physically cannot have different windows.** A per-character number is not merely awkward there; it
is unsatisfiable, and Epoch would have to pick one of them and silently ignore the rest.

It is also unsatisfiable in a second way. `context_tokens = 131072` depends on the model, the
artefact, the card, the runtime, the build and the machine. Carried to a smaller card it is a
number nobody can honour — which is exactly what ADR-0023 built characters to survive.

### The line

> **NPC chooses the Brain. MODELS decides how that Brain runs. Context Composer decides what that
> Brain sees.**

Three responsibilities, and none of them holds a copy of another's answer.

**MODELS is the single physical authority.** Runtime, backend, artefact, context window, KV cache,
flash attention, speculative decoding, GPU layers, tensor placement, batch — everything that
decides how inference physically happens. A model has one **applied profile**, and a profile is a
complete unit rather than a context number with settings beside it.

**A character inherits it.** It selects a Brain and receives that Brain's applied profile whole. It
does not configure `llama.cpp` and it does not carry a window. Change the applied profile in MODELS
and every character on that Brain changes with it — four characters, one edit, no character file
touched.

**And the character keeps the half that is genuinely its own.** Not *how much window*, but *how to
use the window there is*:

```toml
context_policy = "long"   # compact · adaptive · long · custom
```

`Compact` is a character that works from little. `Long` is one that wants everything it can have.
Neither is a number, both survive moving from a 4070 to a 3090, and a Historian who wants a large
window still wants one after the hardware changes. That is behavioural identity — which is what
this ADR's canonical set was always for.

**`Adaptive`, not `Auto`.** MODELS keeps `AUTO` for choosing a physical profile. Two layers, two
words: this repository has already paid for one word meaning two things at `Manual` (ADR-0027),
`-ngl` against `-ot`, and a helper reused with a different meaning that cost a whole search.

### The domain rule

```
Required Context  ≤  Character Effective Use  ≤  Model Context Window
```

| | |
|---|---|
| **Required Context** | what the character needs to exist correctly — system prompt, identity, required skills. ADR-0012 already forbids dropping these. |
| **Character Effective Use** | what the Composer decides to use, governed by `context_policy`. |
| **Model Context Window** | decided exclusively by the applied profile in MODELS. |

**The policy governs the space between the floor and the ceiling, never the floor itself.** A
`Compact` preference of 8K on a character whose Required blocks cost 12K does not produce an 8K
character; it produces a 12K one. The optional capacity is `window − required`, and that is what
`compact`, `adaptive` and `long` divide.

No percentages are fixed here. `Compact` means *required plus a little of what is optional*, `Long`
means *required plus as much of it as is available*, and the heuristics that turn those into
numbers are implementation, measured where they can be.

### Incompatible is a configuration error, not a failed turn

When `Required Context > Model Context Window` the character cannot run. That must be found **when
the profile is applied**, against every character on that Brain — not on the next turn of whichever
one happens to speak first:

```text
Applying Gemma 4 · 32K
  ✓ Mage        requires 9K
  ✓ Guardian    requires 6K
  ⚠ Historian   requires 38K — cannot run with this configuration
```

An incompatible profile is never applied silently. This is the cold-instrument rule at the moment
of a decision rather than three layers inside a turn.

### What this does not change

The rest of the canonical set — `temperature`, `top_p`, `reasoning` — is untouched. Each still
passes the test this ADR set: *does it survive changing the engine?* A Guardian at 0.2 is a
Guardian at 0.2 on any backend. A Guardian at 131,072 tokens is a Guardian on precisely one
machine.

Provider-native tuning is untouched, and `num_ctx` was always an example of it. What this
amendment does is stop the *canonical* set from carrying the same quantity under a different name.

### Cost, named

A character can no longer express *I need at least this much window*. That is a real loss: it was
one way to say a Historian is different from a Guardian, and the replacement says it less
precisely.

It is the right trade because the precise version was **unenforceable**: it named a quantity the
runtime could not honour per character, on a machine that might not have it at all. A less precise
statement that is always true beats an exact one that is sometimes impossible.

