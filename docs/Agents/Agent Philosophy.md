# Agent Philosophy

> Owner ADRs: [[../ADR/0011-definition-runtime]], [[../ADR/0017-character-archetypes-world-packs]] · Principle: Runtime over Configuration
> **Behavioural identity lives in [CHARACTER_BIBLE.md](../../CHARACTER_BIBLE.md). Names, portraits and setting come from the active World Pack.**

## Agent vs Character
An **Agent** executes instructions. A **Character** has identity that shapes every decision through consistent behavior. The model (Claude/GPT/Gemini/Qwen/local) is infrastructure; the Character is the product.

## The engine knows archetypes, never names
```
character.researcher    character.coordinator
character.guardian      character.historian
```
The active World Pack projects an archetype into an actual character (name, portrait, sprite, voice). The engine cannot tell one pack's cast from another's. The official Epoch cast is a deliberate future milestone: [[../Milestones/Official Epoch Universe]].

## The triad ([[../Architecture/Definition Runtime]])
- **Character Definition** - immutable data (these files).
- **Character Runtime** - stateless resolver.
- **Character Instance** - the living party member (stateful).

## Bible -> Definition projection
| Bible concept | Projects into |
|---|---|
| Values, Personality, Decision/Communication Style | Prompt |
| Strengths, Weaknesses, Preferred approach, Avoids | Requested Capabilities + Prompt |
| Risk posture | Trust Policies |
| Visual + Animation language, Name, Portrait | **World Pack** |
| Routine, Idle behavior, Home place, Relationships | *world-simulation data - no schema home yet* |

## Character Definition schema
Archetype id · Role · Prompt · Requested Capabilities · Preferred Models (ranking hint only) · Knowledge Scope (DESIGN NOW) · Trust Policies (DESIGN NOW) · Automation Policies (VISION). Presentation comes from the World Pack.

## Phase-1 party
[[Researcher]] · [[Coordinator]] · [[Guardian]] · [[Historian]].

Models are interchangeable; World Packs are interchangeable; behaviour is constant.
