# Milestone 1 — Walking Skeleton: implementation roadmap

> Agreed 2026-07-25. Governed by [[../Build/Build From Life]]. Milestone definition: [[Walking Skeleton]].
> Architecture is frozen ([[../Architecture/Readiness Review]]) — changes require implementation evidence.

## Where the code lives

All implementation lives under the top-level **`BUILD/`** folder ([[../Build/Build From Life]] rule 10). The vault documents; `BUILD/` is the system.

## Crate layout (resolves the ADR-0002 open question at scaffold time)

```
BUILD/
├─ crates/
│  ├─ epoch-kernel/     # Domain Kernel: pure types, zero I/O
│  ├─ epoch-engine/     # runtime, simulation, context, execution, trust, knowledge,
│  │                    #   activity, provider/, capabilities/, persistence/  (modules)
│  └─ epoch-tauri/      # shell: IPC commands + event bridge
├─ ui/                  # React, presentation only
├─ vault/               # definitions/characters/ · knowledge/  (runtime data)
└─ packs/default/       # default World Pack (archetype placeholders)
```

Directories are created **when the current step uses them** (rule 11) — not up front to mirror the diagram.

**Only `epoch-kernel` is split out on day one** — a crate boundary makes "depends on nothing" compiler-enforced rather than discipline-enforced. Providers and capabilities begin as **modules behind their traits**: the architecture is preserved, and they become crates when compile times or reuse demand it (Earn Complexity applied to layout).

---

## Step 1 · The World exists

**Goal.** Tauri + React shell opens directly into the World. One place rendered from a canonical concept (`research_lab`) resolved by the default World Pack.

**Why.** Establishes the Engine/Presentation split from the first line, and proves concept resolution *before* content exists to hide it. Deferring this guarantees someone hardcodes a name in week two. Also satisfies **"the very first frame matters"** — no loading screen, ever.

**Validates.** ADR-0003 (presentation only, IPC) · ADR-0016/0017 (concept → pack, fallback chain to visible placeholder).

**Introduces.** `crates/epoch-kernel` (concept types) · `crates/epoch-tauri` · `ui/` · `packs/default/`.

**User sees.** The app opens into the World. The Laboratory is there, labelled by the pack — not by code. Empty, but unmistakably a place, not a dashboard.

*The only step that does not yet produce life. Everything after this adds it.*

---

## Step 2 · Someone lives here

**Goal.** One Character Definition loaded from the vault; the Runtime resolves it; the World Simulation gives it a `PresenceState` (idle, at its home place); the UI renders it with **idle behaviors** from its `PresenceProfile`.

**Why.** First life. And honest life: idle = available = true information. Idle is behavior, not an animation loop.

**Validates.** ADR-0011 (Definition / Runtime / Instance) · ADR-0014 + B4 (Registry = repository over vault files) · ADR-0018 (`PresenceProfile` / `PresenceState`) · ADR-0017 (archetype, never a name).

**Introduces.** `epoch-engine/definition/`, `epoch-engine/simulation/` · `vault/definitions/characters/researcher.toml`.

**User sees.** The Researcher in the Laboratory — reading, looking around, shifting position. Never frozen. **Edit her file in Obsidian, save, and she changes live.** First smile.

---

## Step 3 · She moves on her own — the first *wow*

**Goal.** Simulation clock; **routine-driven** travel between places with `PresenceState` (progress / speed / ETA); event-driven coarse publication over IPC; the UI interpolates between authoritative states.

**Why.** This is the emotional milestone. The user does nothing, and the world lives anyway: she finishes reading, closes the book, walks to the Laboratory, settles into her work. Also makes *"characters never teleport"* real code and validates **"the Engine owns reality; the UI owns animation."**

**The causality line.** Movement and idle behavior are driven by a **declared routine** in `PresenceProfile` — authored data interpreted by the runtime, exactly like Runtime over Configuration. That is legitimate and true: it tells the user where she will be. But **"settles into her work" must read as idle-class, not work-class**. With no real Quest, the world must not imply work is happening. Work-class posture is reserved for real execution (Step 6).

**Validates.** ADR-0018 (clock, causality rule, coarse events) · ADR-0015 (presence Activities) · ADR-0003 (IPC events).

**Introduces.** `epoch-engine/activity/` · clock in `simulation/`.

**User sees.** She sets down what she was doing and walks across the world, unprompted. Mid-journey, a headless query answers correctly where she is — the engine is the truth, not the screen.

---

## Step 4 · The World speaks

**Goal.** `Provider` trait + Ollama adapter; Conversation domain model; `StreamingEvent` channel; the user **joins a conversation in a place**.

**Why.** The engine's reason to exist — and the Manifesto's central experience: you don't open an empty chat, you enter where she already is.

**Validates.** ADR-0006 (Conversation model) · ADR-0007 (Provider, capability-indexed registry) · ADR-0005 (Model Profile) · **B3** (token streaming uses `StreamingEvent`, never the Activity Stream).

**Introduces.** `epoch-engine/provider/{mod,ollama}/`.

**User sees.** Click the Researcher → the conversation opens **inside the Laboratory** → she answers, streaming, in her own voice, from her own Definition.

*Expected to resolve **M6*** (resolution timing: per-spawn vs per-turn) with real evidence.

---

## Step 5 · Context is composed

**Goal.** Context Composer with the five Phase-1 Context Providers, priority tiers, compress/drop reduction, Context Report on the `ContextComposed` Activity.

**Why.** Without it, Step 4 is a chat box. With it, it is Epoch.

**Validates.** ADR-0012 + its B1 amendment · ADR-0005 (model-aware budget).

**Introduces.** `epoch-engine/context/`.

**User sees.** A "what she knew" panel — the Context Report rendered: what was included, compressed, dropped, and why. Understandable Autonomy made visible.

---

## Step 6 · The World acts

**Goal.** Executable Capability contract; Terminal capability; Execution Engine; Trust explain-before-act.

**Why.** First real side effect on the machine — explained and authorised.

**Scope guard (agreed).** The Terminal capability ships **read-only in Milestone 1** (`ls`, `cat`, `git status`). The destructive path is designed but not enabled. Trust is validated identically — the user sees the explanation and approves — but a model mistake in week one cannot touch real files. Opened in Milestone 2, once the trust policy has been exercised.

**Validates.** ADR-0008 (caller-agnostic capability) · ADR-0009 (explain-before-act, approval scopes, Builder mode).

**Introduces.** `epoch-engine/{execution,trust,capabilities/terminal}/`.

**User sees.** The Researcher moves to where the work happens and asks in plain language — *"I'll run `ls`; read-only, nothing to undo"* — you approve, and the output appears. Trust stops being a modal dialog and becomes a character asking you something.

*Expected to resolve **M8*** (the ExecutionContext contract Trust needs) and **M5** (presence-vs-work ordering) — with evidence, as the phase requires.

---

## Step 7 · The World remembers

**Goal.** Activity Stream fully wired; Knowledge Engine subscribes and derives a `KnowledgeObject`; markdown projection into the vault.

**Why.** Closes the loop. The world's memory becomes visible — and openable in Obsidian.

**Validates.** ADR-0015 + **B2** (no self-derivation) · ADR-0010 + amendment · ADR-0014.

**Introduces.** `epoch-engine/knowledge/` · `vault/knowledge/`.

**User sees.** After the task, a new document appears in the Library — real markdown, openable outside the app. The world's history grew, and no one wrote it by hand.

*Expected to resolve **M9*** (canonical Knowledge storage shape).

---

## Step 8 · The full loop, and continuity

**Goal.** The complete slice end to end; snapshot / restore; the World visibly reflects completed work.

**Why.** It is the milestone's definition of done.

**Validates.** ADR-0018 (disposable snapshot) · the entire spine simultaneously.

**Introduces.** `epoch-engine/persistence/` (vault-file adapter + snapshot store).

**User sees.** The whole flow runs. Close the app mid-journey, reopen — **she is still walking**. Delete the snapshot, reopen — the world reconstructs cleanly and continues. Nothing resets.

---

## Life progression

| Step | The World… |
|---|---|
| 1 | exists |
| 2 | is inhabited |
| 3 | moves on its own ← **first wow** |
| 4 | speaks |
| 5 | thinks out loud |
| 6 | acts, and asks permission |
| 7 | remembers |
| 8 | persists |

Each row is independently shippable. None leaves the World in a dishonest state.

---

## Review findings expected to close during this milestone

**M5** presence-vs-work ordering (Step 6) · **M6** resolution timing (Step 4) · **M8** ExecutionContext contract (Step 6) · **M9** canonical Knowledge storage (Step 7).

Each must be recorded as an ADR amendment **citing what the build revealed** — not speculation.
