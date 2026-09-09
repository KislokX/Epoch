# Step 2.5 — Geography: making the places belong to the world

> Inserted 2026-07-26 from implementation evidence, after Step 2 closed.
> Governed by [[../Build/Build From Life]] (rules 17, 18) and [[../UX/Design References]].

## Why this step exists

Step 2 succeeded architecturally and fell short experientially. The engine projects correctly, the runtime works, presence works, the World Pack works — and yet:

> **The implementation is rendering a UI that contains a world. We want a world that contains a UI.**

The places sit on a flat canvas. They do not belong to a geography. And a sharper piece of evidence:

> *"If I remove the Researcher from the current implementation, very little of the world still tells a story."*

Removing every character should still leave a believable world behind. The environment itself must communicate life.

**Goal, in one sentence:** *right now the world contains places; this step makes the places belong to the world.*

## Scope discipline

**No new architectural concepts. No new engine systems. No new runtime abstractions.** Every architectural decision already made is preserved exactly.

This is purely about the spatial experience.

## The key architectural finding

**Geography is presentation, so geography lives in the World Pack.**

- The engine continues to know only *which place a character is in* (ADR-0018). It never learns coordinates, terrain or distance.
- Terrain, place positions and paths are **World Pack data** (ADR-0016, ADR-0017). The engine carries them through its projection without interpreting them, exactly as it already carries labels.
- Camera, pan and zoom are **UI** concerns.

**Consequence: this step changes no engine logic.** It extends the pack manifest and builds the camera. That makes it the strongest validation ADR-0016 has had — a different pack means a genuinely different world, and the engine cannot tell.

It also answers the expansion question: future cities are **pack content**, not new screens or new code.

## Objectives

1. The world becomes significantly **larger than the viewport**.
2. **Camera movement is navigation** — pan and zoom are first-class interactions.
3. Buildings belong to **geography**, not to a flat canvas.
4. **Terrain connects locations** — forest, paths, water, elevation.
5. Places feel **discovered**, not presented.
6. Characters exist **inside the landscape**, not floating above UI elements.
7. The world **communicates scale**. Empty space and distance become meaningful.
8. Removing every character still leaves a **believable place**.

## Sub-steps

Each compiles, runs, and is independently visible.

### 2.5a · The world gains a map

**Goal.** Extend the World Pack manifest with a map: world dimensions, terrain, place positions, and paths between places. Engine loads and projects it; nothing interprets it.

**Introduces.** `packs/default/` map data · map types in `epoch-kernel` (pure data) · projection fields in `epoch-engine/world.rs`.

**Visible.** The five places sit at authored positions on a landscape that is bigger than the window. Distances between them are real and unequal.

### 2.5b · The camera becomes navigation

**Goal.** Pan (drag, and edge/keys) and zoom (wheel, and keys) over the world. Camera state is UI-only; it never touches the engine.

**Introduces.** `ui/src/hooks/useCamera.ts` · viewport transform in the world screen.

**Visible.** The user moves through the world instead of looking at all of it. Zooming out shows the whole region; zooming in shows a place and who is in it.

### 2.5c · Terrain and paths

**Goal.** Render the authored terrain and the paths that connect places, so the geography explains the topology.

**Visible.** Paths visibly run between places. Water and forest separate regions. It reads as somewhere, not as a diagram.

### 2.5d · Environmental storytelling from real state

**Goal.** Places communicate their state through the environment, driven by engine state that already exists (occupancy, current activity) — never invented.

**Visible.** The Laboratory is lit and open because someone is inside it; the other places are quiet and closed because nobody is. Removing her from the vault visibly changes the *building*, not just a label.

### 2.5e · The world without anyone in it

**Goal.** Verify objective 8 honestly: empty the vault of characters and confirm the world still reads as a place worth being in.

**Visible.** A believable, quiet, uninhabited world. Honest silence — not a broken screen.

## Explicitly out of scope

Building interiors (entering a place) · minimap · HUD panels from the Figma reference · new places beyond the five concepts · sprites or art beyond simple authored shapes · weather · music.

Interiors in particular are a large enough experience to deserve their own step once geography exists.

## How the references are used

Per [[../UX/Design References]]:

- The Figma prototype is an **interaction** reference, not a visual specification.
- The overworld image is a **spatial** reference, not an asset reference. No pixels are reproduced, traced or derived. We study *why* that world feels like a world — continuous terrain, real distance, places as landmarks — and converge on those principles.

## Definition of done

1. The world is larger than the viewport, and pan + zoom work.
2. Places sit in a landscape at authored positions, connected by visible paths.
3. Characters render inside the landscape, at world scale.
4. Place appearance reflects real engine state (occupancy), never invention.
5. With no characters loaded, the world still reads as a believable place.
6. **No engine logic changed.** Geography is entirely pack data; camera is entirely UI.
7. The [[../../LIVING_WORLD_DESIGN_GUIDE]] review checklist passes.

## Living Score question for this step

> Does the world feel like a place rather than a screen — and would it still feel like one with nobody in it?
