# HUD Redesign

The World HUD is being refreshed in small, independently verifiable slices. This document is
the implementation record for that work; [CODEX_NEWPLAN.md](../../CODEX_NEWPLAN.md) remains the
ordered product plan.

## Invariants

- The HUD is a projection over a live World, not a screen that replaces it. Only named panels
  receive pointer events; the gaps remain available to the World camera.
- Every window goes through `Frame`. A World Pack that supplies `ui.frame.window` owns its
  nine-slice frame; Epoch's CSS is only the fallback for a Pack that supplies no frame art.
- Presentation remains offline-first. Fonts are bundled by `@fontsource`; no view fetches a
  Google Font or another remote styling asset.
- A reading is shown only when the Engine can support it. Levels, XP and energy remain outside
  this redesign until they have a real source.

## Step 1 — fallback visual language

Delivered 2026-08-10.

`ui/src/app/hud.css` now defines the HUD palette as semantic CSS custom properties, then maps
the established `--ep-*` component vocabulary onto them. This lets the existing components move
together without introducing a second styling system.

The drawn `Frame`, `Plate`, `epbtn`, `epbar`, `SidebarCard` and sidebar rows share:

- square, gold-edged panels with a pixel shadow;
- stepped bar changes and a striped fill;
- physical press feedback on buttons; and
- an opaque titled panel surface for the Connected Models reading.

`Frame.frame--skinned` explicitly removes the fallback background, outline and shadow before it
uses the Pack's `border-image`. This keeps an authored `window.png` authoritative while the
ordinary no-art case stays complete.

The body face is `VT323`, bundled locally as `@fontsource/vt323` under OFL-1.1. `Press Start 2P`
remains the display face.

## Step 2 — offline interaction sound

Delivered 2026-08-10.

`ui/src/experience/sfx.ts` supplies the eight authored voices (hover, click, open, close,
select, Quest terminal, error and typing) entirely through Web Audio. The service owns one
shared context, but does not create it while the application mounts: `SfxGate` unlocks it only
after the user's first pointer or keyboard gesture. A scheduled sound therefore cannot cause an
unsolicited first sound, and a browser that refuses `resume()` remains silent rather than
breaking the interaction that prompted it.

The audio is intentionally an interaction layer rather than a component dependency:

- document-level `epbtn` hover/click listeners cover existing and future pixel buttons without
  adding wrappers that could intercept World input;
- overlays, travel selection, a failed turn, typing and closing a Quest each announce their
  corresponding meaningful event; and
- the top-bar sound control is a real persisted preference (`epoch.sfx.muted`), not a decorative
  status lamp. Its state is exposed through `useSfx()` and unmuting confirms itself with the
  select voice.

There are no audio assets, network fetches or remote fonts involved. `sfx.test.ts` uses a fake
AudioContext to hold the safety contract: nothing schedules before unlocking, authored notes use
the expected pitch/envelope, and mute persists and schedules nothing.

## Step 3 — crew allowance and context

Delivered 2026-08-10.

The Crew sidebar now renders a compact card per character rather than a generic status row. Its
two readings are intentionally different:

- **ALLOWANCE** appears only for a character assigned to Claude Code or Codex. It is the real
  remaining plan window already measured from that signed-in local CLI; Claude uses its five-hour
  window and Codex its one reported plan window. An unreadable agent reading stays `—`. A Provider
  or local model has no allowance meter at all, because it has no subscription plan to exhaust.
- **CONTEXT** is the room left in the last measured context (`100 - used / budget`) for the active
  Quest. It reduces as the Chronicle grows and a model compaction visibly restores it when Epoch
  recomposes the next turn. An external agent that has just started a fresh session becomes cold
  until it reports its own window again; claiming a full window before that report would be false.

This required widening the runtime record, not estimating in React. `World` now retains context
readings by `(questId, characterId)` for both model and agent turns, and `remembered_contexts`
returns the active Quest as one projection. A late answer from a Quest the user already left can
therefore never fill a card in the Quest now being viewed. Closing a Quest removes its ephemeral
readings with its agent session.

`CrewCard.test.tsx` holds two user-facing absences: local models do not acquire a fictitious
allowance bar, and a measured 250 / 1,000 context reports 75% room remaining.

## Step 4 — top bar

Delivered 2026-08-10.

The top frame is now a four-part measured layout: the owner's avatar/name and their real
`ORCHESTRATOR` role; Claude's two plan windows; Codex's plan window; and World facts plus
navigation. It keeps the account instruments already introduced in the earlier plan-usage work
but gives each group a stable place instead of allowing counts to mingle with allowance bars.

The former `LV 0` marker is removed. There is no level, XP or energy system behind it, and a
plausible zero is still a cold instrument when it is presented as a rank. Crew and Place counts
come from the live `WorldView`. The final pill reads `WORLD RUNNING` or `WORLD STOPPED` from the
same frozen/running state that gates the World Editor; it is the World clock, not an optimistic
claim that the engine connection is healthy. Sound remains a real preference control rather than
the reference's decorative `ON` lamp.

The grid folds the navigation row below the readings at narrow desktop widths; it does not cover
the World or make the top frame consume pointer events outside its own surface.

## Step 5 â€” side columns

Delivered 2026-08-10.

The two rails are now named as regions rather than incidental `div`s: **Crew and work** on the
left, **World instruments** on the right. Their content remains the six existing, live systems:

- Crew, Agents and Workflows on the left;
- System Map, Connected Models and Terminal on the right.

This is deliberately a structural and visual consolidation, not a second sidebar implementation.
Crew still opens the character conversation, the map still travels, provider state remains
`READY` / `NO MODELS` / `OFFLINE`, and the Terminal remains resizable. Each frame explicitly
keeps an opaque backplate, including an empty Workflows card, while the spaces *between* frames
remain pass-through World space for camera dragging.

## Step 6 â€” effects and polish

Delivered 2026-08-10.

The fallback HUD now carries a low-contrast, pixel-stepped scanline layer behind its instruments.
It never catches input or sits above text/buttons. `prefers-reduced-motion` disables it together
with the existing cursor, bar and terminal animations. The established physical button press,
bar motion and terminal activity cue remain the only motion tied to interaction or live work.

## Evidence

From `BUILD/` after Steps 4â€“6:

```text
npm --prefix ui test -- --run  → 30 files, 120 tests passed
npm --prefix ui run build      → TypeScript and Vite production build passed
cargo test -p epoch-tauri      → 5 tests passed
```

For mixed-end-of-line legacy stylesheet files, use this repository-aware check rather than the
default Git whitespace mode:

```text
git -c core.whitespace=cr-at-eol diff --check
```

## Next cuts

1. End-to-end visual regression and closure of the HUD redesign: a running/stopped World clock,
   real plan readings or their cold states, local-model Crew without a fictitious allowance,
   a solid empty Workflows card, sidebar/map/Terminal interaction, dock navigation and layout
   toggles, and a reduced-motion pass.

## Fase 4 correction pass

The visual-regression pass exposed six concrete corrections before Fase 4 can be accepted:

- Claude's two account bars mean **used**; Codex keeps its single **remaining charge** bar, while
  Crew allowance/context bars also mean **remaining**;
- allowance and agent/provider probes wait until the World has its first visual frame, and the
  Tauri allowance command moves its short-lived CLI work off the IPC runtime;
- Auto/Accept edits start Codex in the Project Root as well as passing `-C`, preserving the
  `workspace-write` boundary on Windows;
- `MODELS` lists signed-in account agents separately from provider model endpoints;
- Settings owns persistent, offline UI sound enablement and volume; and
- `Plate` no longer renders decorative square pips, while dialogue and real Crew meter text are
  enlarged for reading.

`World UI Surface.md` records the deliberately separate next persistence slice: editable
World-owned frames, surfaces, brand art and icons need typed assets, not an overextended
`window.png` convention.

## Fase 4 correction — turn identity and Codex writes

The next visual check found two behavioral defects that could not be repaired with a HUD-only
change:

- Every live turn event now carries both `characterId` **and** `questId`. The dialogue accepts a
  token, tool step, context measurement, compaction state, or ending only when both identities
  match the conversation currently shown. Selecting a different Quest changes that expected id
  before React waits for the new Chronicle, so a late answer from Quest A cannot flash in Quest B
  merely because the same character is speaking in both.
- Codex `Auto` passes its own `--approve-for-me` switch. Its normal `Manual` path stays
  read-only and sends file changes through Epoch's MCP door, where the exact action waits for a
  decision. A native sandbox denial is diagnostic fallback only, never the intended way to obtain
  permission.
  That switch selects Codex's `workspace-write` sandbox; its CLI rejects combining it with a
  separate `--sandbox workspace-write`. `Manual` still starts with `--sandbox read-only`, and no
  Epoch mode ever uses `danger-full-access`. A real probe created and read a file from inside the
  selected Project Root with this exact Auto command.
- Crew `ALLOWANCE` uses a red / dark-red pixel cadence. It remains a remaining-plan reading;
  `CONTEXT` remains a neutral, independent reading.

A World without a configured Project Root intentionally cannot create or modify project files.
The World surface must keep showing the setup action instead of pretending an arbitrary folder is
safe to use. This is distinct from a configured World whose Auto or accepted Manual write is
allowed inside its own root.

## Fase 4 correction — Quest-scoped live work

A running turn is now a short-lived record keyed by **both** its speaker and Quest. The Engine
emits `turn:started` synchronously before each worker begins, then emits the existing
`turn:ended` even if the player has opened another conversation. The Experience Surface stores
those lifecycle facts regardless of which window is visible, but only renders tokens, tool steps
and the final reload when their exact Quest is open.

That distinction protects three cases that previously regressed together:

- opening **NEW** while Quest A works cannot put Quest A's text or spinner in the empty Quest B;
- returning to A while it is still running restores its working state and STOP control; and
- returning after it finished reloads A's Chronicle and clears its spinner, even though the
  `turn:ended` event occurred off-screen.

The Crew meters also distinguish missing information from zero. `ALLOWANCE` has an explicit
red/dark-red cadence and can no longer lose its gradient when a palette alias is absent. When a
provider reports consumed context but not its context-window ceiling, `CONTEXT` displays a moving
cyan activity band and its exact token count; it does **not** invent a percentage. A provider that
does report a ceiling continues to receive a proportional Context meter.

### Codex Windows sandbox diagnostic

Epoch keeps Auto inside Codex's `workspace-write` sandbox. Because `codex exec` has no native
non-interactive approval callback, Manual remains native `read-only` and receives Epoch's MCP
file tools for this turn only. A write, edit, or delete reaches Epoch first, displays its exact
arguments, and runs only after approval. No mode selects an unrestricted sandbox.

If startup reports `codex-windows-sandbox-setup.exe`, Epoch retries once with the identical
Project Root and sandbox. The retry is limited to that helper bootstrap signature, before a tool
event exists; an ordinary failed command is never replayed because it could duplicate real work.
If the second attempt fails, Epoch surfaces the actual terminal event. Repairing or updating the
local Codex helper remains the safe remedy; disabling the sandbox is not.
