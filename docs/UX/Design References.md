# Design References

> Long-term UX and visual direction for Epoch. **Not specifications.** Not implementation tasks.
> Governed by [[../../EXPERIENCE_CONSTITUTION]] and [[../../LIVING_WORLD_DESIGN_GUIDE]]; drives [[../Build/Build From Life]] rule 17.

Two references were provided on 2026-07-26. They represent the interaction model and the feeling Epoch should converge toward over time. Improve on them whenever implementation reveals something better, but keep their philosophy.

---

## 1. Interaction model — the Figma prototype

A React prototype (Figma Make export: React 19, Vite, Tailwind v4). It is a **reference for the interaction model**, not a codebase to import — its tooling differs from ours and vendoring it would confuse `BUILD/`.

### What it establishes

**The world is the background of everything.** A world map renders behind the entire interface. Over it sits a HUD layer that does not capture pointer events; only its individual panels do. Nothing ever replaces the world. This is the clearest statement of "the world is the primary interface", and it is the single most important thing to preserve.

**Persistent party presence.** Party frames sit top-left: all four characters, always visible, each showing state, one selected. Presence becomes ambient furniture rather than something you navigate to.

**The active Quest is always legible.** A badge names the current mission and its health ("build passing") — the answer to "what is happening?" without asking.

**A fast path always exists.** `⌘K` opens a command palette (called the Grimoire / Spellbook in the prototype). This is the escape hatch that keeps immersion from costing productivity. It is not optional.

**Observability as furniture.** A thin bottom status bar carries provider health and latency (Anthropic / OpenAI / Gemini / Ollama), token usage and elapsed time. Diagnostics live in the world's chrome, not behind a menu.

**Game-like muscle memory.** A number-key action bar (1–8) for the most common actions.

**Ambient, dismissable notification.** A single line, centre-top, that fades — never a stack of toasts.

### Where we deliberately diverge

The prototype opens deep views (knowledge, party builder, model library, projects) as **modal overlays** (`RPGModal`). We are replacing that with **in-world transitions**: clicking a building enters it, and its interior is still part of the same continuous world. See reference 2.

This is an evolution of the reference, not a rejection of it — and it is more faithful to the Experience Manifesto than the prototype was.

### Component vocabulary worth keeping

`PartyFrames` · `QuestLog` · `ChatWindow` · `ActionBar` · `Minimap` · `CommandPalette` · `Inspector` · `KnowledgeView` · `ModelLibrary` · `PartyBuilder` · `ProjectsView` · `OnboardingView`

These map cleanly onto subsystems we have already designed. When each is built, the mapping is in [[World]].

---

## 2. World presentation — the overworld reference

A single annotated screenshot of an SNES-era JRPG overworld, marked *"608x192 overworld (recommended)"*.

### ⚠ Boundary: reference only

**The image is a copyrighted screenshot (Square Enix).** It is a private direction reference and nothing more.

- It must **never** enter `BUILD/`, the default World Pack, or any distributed artifact.
- **No asset may be traced from, derived from, or reproduced from it.**
- Per [[../../CONTENT_PHILOSOPHY]]: the inspiration is welcome, the identity must be original.

What we take from it is **structure and feeling**, which are not protectable — and which is exactly what a direction reference is for.

### What it establishes

**Geography, not layout.** The world is continuous terrain — forest, water, cliffs, paths — rather than a grid of cards. Places sit *in* a landscape.

**The world is much larger than the viewport.** What you see is a window onto somewhere bigger. This requires **panning** and **zooming** as first-class interactions, not conveniences.

**Characters stand in the world at small scale.** They are part of the landscape, not overlaid portraits. Their size relative to buildings is what makes the world feel inhabited rather than diagrammed.

**Places are landmarks you approach and enter.** Towns and buildings are destinations connected by paths — a topology, not a menu.

**The world grows.** New regions are added to the same map over time; earlier areas remain. Nothing is replaced.

**Time is on screen.** A small corner label names the era. Time is part of the interface, not metadata.

### Implementation consequences (for the step that builds them)

- A world coordinate space independent of viewport size; camera pan + zoom over it.
- Places have world positions, not layout slots. Position is presentation data (a World Pack concern), while *which place a character is in* stays engine state (ADR-0018).
- Interiors are places too: entering a building is a camera/scene transition inside the same world, not a route change.
- The map must be authored by the active World Pack, so a different pack can supply a different geography without engine changes.

---

## Where the source material lives

The originating archive (`UI Design.7z`) is kept by the author outside the repository. Its lessons are recorded here so they survive independently of it. The Figma prototype source can be vendored under `docs/UX/reference/` if it ever needs version control; the screenshot must not be.
