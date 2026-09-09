# ADR-0022: Visit and the Experience Layer

- Status: Accepted
- Date: 2026-07-27
- Depends on: [[0003-engine-presentation-separation]], [[0018-world-simulation]], [[0021-places-as-compositions]]
- Amends: [[0021-places-as-compositions]] (a mark may declare the reveal level it belongs to)
- Related architecture: [[../../PRODUCT_ARCHITECTURE]], [[../../EXPERIENCE_CONSTITUTION]], [[../Build/Build From Life]]
- Horizon: `Visit`, Camera, Focus and reveal level 1 **IMPLEMENT NOW**; reveal level on marks, a Place's available depth, Experience State in the Engine **DESIGN NOW**; interiors, authored transitions, interactive systems **VISION**

## Context — the evidence

The architecture defined what is true (Engine), what a Place is made of (ADR-0021), and who owns animation (ADR-0018). One question had no owner: **how the user progressively experiences that reality.**

Building the camera made the gap concrete twice:

1. **`WorldMap.tsx` welded the viewport to the world's extent** — `viewBox="0 0 map.width map.height"`. Nothing in the architecture said that was wrong, because nothing owned the question "how much of the World is being experienced right now". A World with no canonical size was silently being shown all at once.

2. **The camera was React state initialised by effects.** It worked, then did not, in a way that was hard to reason about — the initial frame depended on a `ResizeObserver` having reported before an effect ran. The fix was not a better effect: it was recognising that the camera is a *derived value*, not a stored one. That generalises, and it is the core of this ADR.

Without an explicit contract, camera movement, place visitation, reveal, ambient transitions and contextual focus become scattered UI behaviour that nothing can hold to account.

## Decision

### 1. `Visit` is the only interaction

Users learn exactly one interaction with a Place:

```
Visit(PlaceId)
```

It expresses **intent** and nothing else — no rendering, no animation, no presentation. Its responsibility ends once the intended Place is known.

Addressed by **identity**, never by appearance or position. That is what lets `Visit` become a Command on the Activity Stream (ADR-0015) later without anything above it changing.

### 2. Five stages, one responsibility each

```
Visit      which Place the user intends to experience
Camera     changes point of view — never what exists
Focus      changes perception — dims, emphasises, composes
Reveal     how much of an existing Place is being experienced
Experience orchestrates presentation — atmosphere, transitions, emphasis
```

No stage may take another's responsibility. In particular the Camera decides **nothing** about availability: arriving somewhere is not the same as being able to do something there.

### 3. The Experience Layer derives; it is never a source of truth

Everything in the Experience Layer must be derivable from exactly three inputs:

```
Engine State  +  User Intent  +  Time
```

**If something cannot be traced to one of those, it does not belong in the Experience Layer.** It may not invent a fact and it may not create functionality.

Consequence, learned from the camera: **state in this layer holds user intent only.** Everything else is computed on read. A camera stored and mutated by effects had ordering bugs available to it; a camera derived from (world + arrival + window) has none.

### 4. Reveal is bounded by reality

Reveal is not distance, not graphical detail, not level of detail. It is **how much of a Place is currently being experienced**, and it has two independent inputs:

- **Proximity** — how close the camera has come. Continuous, Experience Layer, presentation.
- **Available depth** — what the Place can truthfully offer. Discrete, derived from Engine state.

```
shown = min(proximity, available depth)
```

The Experience Layer may **never** reveal more than exists. Approaching a Laboratory with no provider configured reveals *that it is dormant* — which is information, not an empty room. Without this formula, "Reveal is not distance" is an aspiration; with it, faking an interior is arithmetically impossible.

Valid at any reveal level: subtitle, occupants, atmosphere, authored emphasis, contextual particles.
Never valid: an interactive console with no implementation, a busy-looking laboratory with no provider, fabricated occupants, invented provider state.

### 5. Reveal is expressed as marks — there is no second architecture for interiors

A Place has **one identity** and is never replaced or duplicated. A mark may declare the reveal level it belongs to; the renderer draws marks whose level is at most the level currently shown.

```toml
[[places.research_lab.marks]]
key = "body"
role = "visual"
reveal = 0        # the building, seen from the World

[[places.research_lab.marks]]
key = "workshop"
role = "visual"
reveal = 2        # what is there once the user has actually arrived
```

An "interior" is therefore not a scene, not a second Place and not another document — it is a higher reveal level of the same composition. This is what keeps the promise that a Place never becomes another Place.

It also keeps the Asset Pipeline generic (ADR-0020): a higher reveal level is *another asset alongside*, never an edit to an existing one. Interaction happens **around** assets — highlights, rings, particles, labels, camera, sound — because a World's artwork may be PNG, SVG, pixel sprites or a format that does not exist yet, and none of them can be assumed to contain editable windows, doors or lights.

### 6. Experience State and Experience Playback are different things

| | What it is | Where it may live |
|---|---|---|
| **Experience State** | which Place is being visited, which reveal level is shown, which atmosphere applies | discrete, derivable, testable **headless** |
| **Experience Playback** | camera interpolation, fades, particle emission, audio mixing | per-frame — **never** leaves presentation |

ADR-0018 already drew this line for characters: the Engine owns authoritative states, the UI interpolates between them. This applies it to the Experience Layer unchanged.

Phase 1: Experience State lives in the presentation layer as pure modules with no framework inside them, so it stays testable and can move. It moves into the Engine when it needs Engine facts it cannot derive locally — a Place's available depth from `Provider::health()` (ADR-0007) is the first. Because it is addressed by `PlaceId` and expressed as plain values, that move changes nothing above it.

## Why this is best

- One interaction the user learns once, whose meaning can grow for years without retraining them.
- The reveal formula makes dishonesty impossible rather than discouraged — the strongest form of the causality rule so far.
- Interiors cost no new architecture, which removes the single biggest source of future duplication in the World.
- Derived-not-stored eliminates a whole class of ordering bug, with implementation evidence rather than preference behind it.

## Alternatives considered

- **Selection opens a panel.** Smallest change, and it is the interaction model of every tool Epoch is not. It also needs content that does not exist yet, so today it would be an empty panel waiting to be filled. Rejected.
- **Separate "outside" and "inside" Places.** Familiar from games, and it duplicates identity, position, naming and assets per Place forever. Rejected — reveal levels express the same thing with one identity.
- **Reveal keyed to camera distance alone.** Simplest to implement, and it silently reintroduces the fake interior: zooming into an unconfigured Laboratory would "reveal" an interactive level with nothing behind it. Rejected on honesty.
- **Experience State in the Engine now.** Cleaner on paper, and today it would be a Rust subsystem with one enum in it, ahead of the Provider that gives it a second value. Deferred with a named trigger instead.
- **Interaction affordances authored per Place.** More expressive, and a World that forgets to author a selection ring gets no feedback at all. Rejected: affordances are generic and sized from the Place's footprint, and a World enriches them rather than supplying them.

## Consequences

- The presentation layer gains an `experience/` module that owns camera, focus, reveal and visit — and owns no truth.
- `Place` gains an available-depth concept, derived, when the first capability can drive it.
- A mark gains an optional reveal level. Manifests stay valid: absent means level 0.
- Every future interaction must answer which of the five stages it belongs to.
- Interaction is addressed by `PlaceId` everywhere, which is what makes it promotable to a Command.

## Assumptions

- Three inputs (Engine State, User Intent, Time) are sufficient to derive every experience Epoch needs. If a fourth appears, this ADR is wrong and should be amended with the evidence.
- A single integer reveal level is expressive enough before interiors exist.
- Users prefer one growing interaction to several precise ones.

## Risks

- **The Experience Layer becoming a dumping ground.** *Mitigation:* the three-input rule is a test, not advice — anything untraceable is misplaced by definition.
- **Presentation logic leaking into the Engine** once Experience State moves there. *Mitigation:* only Experience State may move; Playback is defined as unable to.
- **Reveal levels proliferating.** *Mitigation:* a new level requires a real capability whose depth it expresses, exactly as a mark role requires a renderer that can draw it.
- **`min(proximity, depth)` feeling unresponsive** when a user approaches a dormant Place and little changes. *Mitigation:* dormancy must be *expressed*, not merely withheld — a dark Laboratory that visibly says what it is missing is honest and useful; silence is not.

## Open questions

- Does Focus belong to the Experience Layer or to the Camera? Dimming neighbours is composition, which is arguably the camera's business.
- How is atmosphere requested? `CONTENT_PHILOSOPHY.md` says a mood, never a track — so probably an `ambient_sound` mark carrying a mood concept, but no World has authored one yet.
- The Launcher has no Experience Layer today. If boarding a World is an authored transition, which surface owns it?
- Six places appear in the product references (bridge, library, laboratory, forge, communications, observatory); the Engine knows five concepts. Communications and Observatory map to capabilities described in `docs/Features/Chat.md` and `docs/Features/Timeline.md` but not yet built. The vocabulary grows when those capabilities do — additively (ADR-0017), never to fill a mockup.
