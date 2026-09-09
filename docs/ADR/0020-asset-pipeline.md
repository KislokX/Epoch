# ADR-0020: Asset Pipeline & Renderables

- Status: Accepted
- Date: 2026-07-26
- Depends on: [[0016-asset-resolution]], [[0017-character-archetypes-world-packs]], [[0019-projection-pipeline]]
- Amends: [[0019-projection-pipeline]] (a Renderable **references** an Asset; it does not embed one)
- Related architecture: [[World Packs]], [[../Build/Build From Life]]
- Horizon: asset resolution + `sprite` renderable + path confinement + data-URI delivery **IMPLEMENT NOW**; animated sprite, tilemap **DESIGN NOW**; Import Asset workflow, procedural, 3D **VISION**

## Context — the evidence

ADR-0019 removed appearance from React and put it in the World manifest as inline `shape` geometry. Step 2.5's regression then made the remaining gap obvious:

> Appearance became **content**, but it did not become **assets**.

Inline geometry can only be hand-authored in TOML. A creator cannot bring something drawn in Aseprite, Inkscape or anything else. And "make the placeholder shapes prettier" would be polishing a bridge we intend to replace with pixel art — disposable work.

So the priority changes: before improving the renderer, make the pipeline genuinely content-driven.

## Decision

A World does not reference sprites. It references **Renderables**.

> A Laboratory is not a sprite. It is *something that can be rendered.*

A **Renderable** declares: a renderer kind, an **Asset reference**, and rendering settings.

```
research_lab -> Renderable { renderer = "sprite",   asset = "assets/buildings/laboratory.svg", scale, anchor }
research_lab -> Renderable { renderer = "shape",    shape = [ ... ] }              # no asset needed
research_lab -> Renderable { renderer = "tilemap",  asset = "assets/tilemaps/laboratory/" }
```

Today that Renderable may be primitive shapes. Tomorrow a sprite, then an animated sprite, a tilemap, a Spine animation, a procedural renderer, a 3D model. **The engine never changes. The World's declaration never changes shape. Only the Renderable implementation changes.**

### Assets are files inside the World

An Asset is a file living in the World's own directory, referenced by a **relative path**. Improving visuals becomes a content problem, not an engine problem.

### Assets reach the renderer as images, never as markup

The engine reads an Asset and delivers it to the renderer as a **data-URI image**. The renderer draws it with `<image>`.

This is a security decision, not a convenience. **A World is third-party content.** Inlining an Asset's SVG markup into the DOM would let any installed World execute script inside Epoch. An `<image>` fed a data URI does not run scripts.

It is also why pixel art costs nothing later: SVG, PNG and every future raster format travel the identical path.

### Paths are confined to the World

An asset reference is resolved **strictly within the World's directory**. Absolute paths, drive letters and any traversal that escapes the World (`../`) are rejected at load time and reported. A World cannot read the user's filesystem through an asset reference.

### `shape` remains the honest fallback

`shape` does not disappear. It is what a World uses when it has no asset yet, and it keeps a partially authored World renderable. New appearance goes into assets; `shape` stops being where appearance grows.

Resolution order is unchanged (ADR-0016): active World → parent → default → **visible placeholder**. A missing asset, an unreadable file or an unimplemented renderer all degrade visibly and are reported. Never a blank, never a crash.

## The validation goal

This ADR is proven when:

> A creator replaces the Laboratory's appearance by putting a file in the World and pointing a Renderable at it — **no Rust, no React, no recompile** — and the next World load shows the new Laboratory.

Anything less means visual identity still belongs to the application rather than to the World.

## "Import into World…" — the beginning of the Creator Experience (DESIGN NOW, no Phase-1 UI)

The feature is **not** called *Import Asset…*. It is called **Import into World…**

That is not a wording preference. Creators do not think *"I want to import an asset."* They think *"I want this to become my Laboratory"*, *"I want this tree to exist in my World"*, *"I want this character to live here."* **The user thinks in meaning.** The engine translates that meaning into Assets, Renderables and World configuration.

Which fixes the question the application asks itself. Never:

> *"How do I render this file?"*

Always:

> **"What does this represent in this World?"**

Everything else emerges from that answer.

```
Import into World... -> select file
                     -> What does this represent?
                        ( Laboratory · Library · Character · Tree · Forest · Bridge · Road
                        · Terrain · Decoration · Music · Ambient Sound · Weather · UI · Other )
                     -> preview -> adjust scale -> adjust anchor -> optional settings
                     -> Add to World
```

Epoch then copies the resource into the active World, creates or updates the Asset and the Renderable, updates the World's configuration, and reloads.

No manual file movement. No configuration editing. No programming.

**Creators should not feel they are replacing files. They should feel they are teaching the World what things are** — and not that they are customising Epoch, but that they are authoring a World.

This is not a convenience feature; it is where the Creator Experience starts. The easier it is to bring your own work into a World, the larger the ecosystem around Epoch becomes. The contract must support it now; the interface can come later.

## Why this is best

- Visual improvements become **cumulative content** instead of disposable code.
- Pixel art arrives without an engine change, because raster and vector share one path.
- The renderer becomes progressively more generic while Worlds become progressively more expressive.
- Third-party Worlds cannot execute code or read the filesystem.

## Alternatives considered

- **Keep geometry in the manifest** — content, but not authorable with real tools. Rejected on the evidence.
- **Inline SVG markup into the DOM** — smallest code, but hands arbitrary script execution to any installed World. Rejected on security.
- **Tauri asset protocol (`convertFileSrc`)** — better streaming and caching for large raster assets, but needs protocol scope configuration. **Deferred**: revisit when real pixel art shows data URIs straining, which is the evidence that would justify it.
- **Polish the vector placeholders first** — disposable work on a bridge to pixel art. Rejected.

## Consequences

- The World manifest gains asset references and rendering settings (scale, anchor) per Renderable.
- The engine gains asset resolution, path confinement, and a way to hand asset bytes to the renderer.
- The UI gains an `<image>`-based renderable and loses nothing else.
- Asset problems (missing, unreadable, escaping the World) join the existing World problem reporting.

## Assumptions

- Data URIs are adequate for Phase-1 asset sizes.
- A relative-path asset reference is expressive enough for single files now, and for directories when animated sprites and tilemaps arrive.

## Risks

- **Data-URI size** for large raster sets. *Mitigation:* resolve once and cache per concept; the Tauri asset protocol is the documented next step when evidence demands it.
- **Anchor and scale conventions drifting** between renderers. *Mitigation:* one convention — the same footprint-unit space `shape` already uses, base-centre origin.
- **Path confinement bugs.** *Mitigation:* canonicalise and verify containment; test the escape cases explicitly.

## Open questions

- Directory-shaped assets for animated sprites and tilemaps: manifest-per-directory, or convention?
- Whether Worlds may declare a palette that assets reference (carried from ADR-0019).
- Caching and invalidation once assets are watched for hot reload.
