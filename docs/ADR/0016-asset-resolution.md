# ADR-0016: Asset Resolution & Theme Packs

- Status: Accepted
- Date: 2026-07-25
- Depends on: [[0003-engine-presentation-separation]], [[0005-capability-first-architecture]], [[0011-definition-runtime]], [[0013-scope-model]]
- Related architecture: [[Asset Resolution]], [[../../CONTENT_PHILOSOPHY]]
- Horizon: concept vocabulary + Theme Pack manifest + resolver + fallback chain + default pack **IMPLEMENT NOW**; pack Profile/coverage reporting + user-installed packs **DESIGN NOW**; community marketplace / pack inheritance **VISION**

## Context
Epoch's Living World must never depend on copyrighted assets, and the visual identity must belong to the user ([[../../CONTENT_PHILOSOPHY]]). If any engine code, animation or UI references a filename or a franchise, the art style becomes welded to the product and the legal exposure becomes structural.

## Decision
The engine references **abstract Asset Concepts**; an active **Theme Pack** resolves concepts into concrete assets. The engine never references filenames or franchises.

**Concept vocabulary (Domain Kernel, canonical + versioned)** — same pattern as the Capability vocabulary (ADR-0005). Concepts cover: character + animation states (e.g. `character.architect` + `walk.east`), buildings/places, tilesets, icons, UI elements, fonts, particles, weather, lighting, cursors, transitions, **sound effects**, and **music moods** (e.g. `mood.research`, `mood.peaceful_town`, `mood.night`). Music is requested as a *mood*, never a track.

**Theme Pack** = a complete experience (sprites, music, SFX, fonts, UI colors, windows, icons, particles, animations, weather, lighting, cursor, transitions) declared by a **manifest** that carries:
- pack identity + version
- **concept coverage** (which concepts it provides) — a Profile, queryable like any other (ADR-0005)
- **license metadata (mandatory)**

**Asset Resolver** — stateless, like the Character Runtime (ADR-0011): `resolve(concept, variant, pack_chain) -> AssetHandle`.

**Fallback chain (required):** active pack → base/parent pack → default pack → **visible placeholder**. A missing asset never crashes and never renders a blank space. Unresolved concepts are reported (Observability) so pack authors can fix coverage.

**Default pack rule (hard):** the official distribution ships only original, commissioned, open-source, public-domain/CC0, or explicitly redistributable assets. Never copyrighted game assets.

**Scope:** the active Theme Pack is an **Application-scope setting** (ADR-0013) with optional Workspace override. Packs are content, not Definitions — they carry no behavior.

## Why this is best
- Internal First applied to content: the engine speaks canonical concepts; assets are a projection.
- Removes structural legal exposure — inspiration stays, assets are replaceable.
- Same proven patterns reused (registry + manifest/Profile + stateless resolver + graceful degradation), so it costs little new machinery.
- Makes Epoch a platform: community Worlds without engine changes.

## Alternatives considered
- **Hardcoded assets referenced by filename** — welds art style to code, blocks theming, creates permanent legal risk. Rejected.
- **Skin system (colors/fonts only)** — too shallow for a Living World; sprites/music/animation are the identity. Rejected.
- **Ship a "Chrono-inspired" default pack using existing game assets** — copyright infringement. Rejected outright.

## Consequences
- A concept vocabulary must be curated and versioned before the first sprite is drawn; adding art later means adding concepts, not code.
- Every renderer/animation call goes through the resolver.
- Default assets must be commissioned or sourced under compatible licenses — a real cost and schedule item for Phase 1.
- Distribution policy needed: Epoch does not host or ship third-party packs it has not license-cleared; users install locally at their own discretion.

## Assumptions
- A finite concept vocabulary can express the World's visual/audio needs and extend cleanly.
- Pack authors can supply coverage honestly; unresolved concepts are detectable at load and at render.

## Risks
- **Concept vocabulary churn** breaking existing packs. *Mitigation:* version the vocabulary; additive changes only; deprecate rather than remove.
- **Incomplete packs** producing a broken-looking World. *Mitigation:* fallback chain + coverage reporting + placeholder that is obviously a placeholder.
- **Infringing community packs** damaging the project. *Mitigation:* mandatory license metadata; Epoch neither hosts nor endorses unvetted packs in Phase 1; clear authoring guidance.
- **Performance** of indirection at render time. *Mitigation:* resolve once, cache handles per pack activation.

## Open questions
- Pack file format + packaging (folder vs. archive) and install location.
- Pack inheritance / partial packs layered over a base (VISION).
- Whether Character *names and identity presentation* should also resolve through the pack — see the naming question raised alongside this ADR.

---

## Amendment - 2026-08-09: the interface is content too

A new concept namespace, and nothing else. No second resolver, no theme layer, no parallel
manifest — `ui.*` joins `character.*`, `place.*` and `tile.*` and travels the identical pipeline:
the Engine asks for a concept, the active pack answers, the fallback chain covers what nobody
declared, and the asset arrives as a `data:` URI (ADR-0020).

### Why not "themes"

Because a window belongs to a World. A sci-fi World that replaces its characters and its
buildings and then frames them in somebody else's dialog boxes is two Worlds at once. Introducing
a separate theme system would have made the interface the one thing a World Pack could not be
about — and re-created, beside the pack, every mechanism the pack already has.

### What a pack owns, and what it never owns

**The skin.** Not layout, not behaviour, not what is reachable by keyboard, not whether text can
be read. Those stay code, for the same reason CONTENT_PHILOSOPHY already says a pack cannot
change behaviour: a World is content, and content must not be able to make the product unusable.

### One image and one number, not nine files

A nine-slice is declared as the image plus the corner inset in its own pixels:

```toml
[[ui]]
concept = "ui.frame.window"
image = "ui/window.png"
corner = 12
```

Nine separate files is how a renderer thinks about a nine-slice; one image and "the frame is
twelve pixels" is how somebody draws one. The browser cuts it natively from exactly those two
facts, so the extra eight files would have bought nothing and cost every author eight files.

### Absent is the ordinary answer

Every World today declares no `ui.*` at all, and that is a **complete** World rather than an
unfinished one: Epoch draws its own windows and always will. An undeclared concept falls through
to the drawn one — the fallback chain doing its job, not an error being handled — and an
unreadable image costs the skin and nothing else.

### What this does not decide yet

Only `ui.frame.window` is consumed, deliberately. Buttons, tabs, checkboxes, scrollbars and
tooltips are named in this namespace and **not built**: a registry of widgets with nothing to put
in it is an abstraction for an imagined use, which Earn Complexity forbids. The first authored
window is what will say what the rest of the interface actually needs — including the two
questions no design can answer on its own: what the base scale is, and what a skinned window does
at a size its author never drew it at.
