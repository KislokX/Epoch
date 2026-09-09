# The World (UX hub)

> Governed by the [[../../EXPERIENCE_CONSTITUTION]] (design constitution). Architecture unchanged.

The World is the permanent navigation hub and the primary UI. It is a **projection of the engine** — specifically of the [[../Architecture/Activity Stream]] and the canonical models ([[../Architecture/Domain Kernel]]). Presentation-only per [[../ADR/0003-engine-presentation-separation]].

## Principle
The user joins the world; they do not start it. Places are navigation; animation is observability; every visual maps to a real runtime entity.

## Places -> subsystems
| Place | Subsystem | Doc |
|---|---|---|
| Library | Knowledge Engine | [[../Architecture/Knowledge Engine]] |
| the Research Lab | Models & Integrations | [[../Architecture/Provider Layer]] |
| the Guild | Party Builder (Definitions) | [[../Architecture/Definition Registry]] |
| Command Center | Active Era (Project) | [[../Features/Projects]] |
| Automation Hub | Automations (Quests) | [[../Architecture/Automation Engine]] |
| the Settings hall | Settings | - |

## World vocabulary (presentation projection)
| World | Canonical (engine) |
|---|---|
| Workspace | Workspace ([[../Architecture/Scope Model]]) |
| Era | Project |
| Party | assigned Characters |
| Quest | Automation / Workflow |
| Chronicle | long Conversation |
| History | Timeline |
| Library | Knowledge Engine |

Hierarchy: Workspace -> Era -> Party -> Quest -> Knowledge.

## Design rules (from the manifesto)
1. Vocabulary is a projection - kernel keeps canonical names (Internal First).
2. The world is engine-driven or it is a lie - "while you were away" projects real Activities + Knowledge, never fabricated.
3. Every visual maps to a real architectural referent.

## Drives
The world renders from the Activity Stream (live) + repositories (state). Characters move because real Activities occur; "while you were away" narrates real Activities/Knowledge created by scheduled Quests or background runs.

## Long-term direction
[[Design References]] — the interaction model (Figma prototype) and world presentation (overworld reference): world larger than the viewport, panning, zooming, buildings entered rather than opened as modals, and the world always behind everything.

## Companion constitutions
- [CHARACTER_BIBLE.md](../../CHARACTER_BIBLE.md) - who lives in the World (identity, routines, relationships, presence).
- [LIVING_WORLD_DESIGN_GUIDE.md](../../LIVING_WORLD_DESIGN_GUIDE.md) - how the World moves and communicates (movement, places, animation, silence, time, restraint) + the design review checklist.

Everything user-facing calls Epoch simply **the World**.


