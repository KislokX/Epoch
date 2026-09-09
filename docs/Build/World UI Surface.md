# World-owned UI surface

## The correction

`ui.frame.window` proved the important first property: a World Pack can replace Epoch's drawn
window without the frontend naming a file. It is not yet a complete World UI system. A window is
a nine-slice; a logo, a button icon, a terminal texture and a backdrop are not. Treating every
one as `window.png` would stretch images that must not stretch and would make the editor invent
image metadata it cannot know.

The next UI-authoring slice therefore extends the existing **concept -> resolved asset -> fallback**
pipeline rather than adding a parallel theme store. The user will own these assets in their World
and can replace or remove each one from the World Editor. Nothing in this design lets content
change layout, keyboard behavior, permissions or text legibility.

## Ownership and files

The editable UI belongs beside the other editable World data, not in Epoch source and not in a
shared default Pack:

```text
vault/worlds/<world-id>/
  ui.toml
  assets/ui/
    frame-window.png
    frame-terminal.png
    logo.png
    icon-quest.png
    surface-terminal.png
```

`ui.toml` is an overlay. Resolution order is:

1. this World's `ui.toml`;
2. the active Pack chain's `[[ui]]` declarations;
3. Epoch's CSS/SVG fallback.

Removing an override restores the Pack answer; removing a Pack answer restores Epoch's drawn
fallback. A missing or invalid image is reported and falls through. It can never make the World
blank or prevent entering it.

## Two asset roles, one vocabulary

Every declaration has a stable concept, but the renderer learns the role from the declaration:

| Role | Uses | Required metadata | Example concepts |
| --- | --- | --- | --- |
| `frame` | resizable window/panel borders | `corner`, `repeat`, `scale` | `ui.frame.window`, `ui.frame.dialogue`, `ui.frame.panel`, `ui.frame.terminal` |
| `image` | never-stretched bitmap/vector artwork | `fit` (`contain`, `cover`, `tile`) | `ui.brand.logo`, `ui.icon.quest`, `ui.icon.close`, `ui.surface.hud`, `ui.surface.terminal`, `ui.backdrop` |

The generic `frame` response remains the current `Skin` shape. `image` responses contain the
data URI, intrinsic dimensions and fit policy, but no fake corner values. Both responses travel
from the Engine as data URIs, never frontend filesystem paths.

The first shipped vocabulary is deliberately small and additive. A Pack may omit every entry;
Epoch supplies usable fallback art. Future concepts are added rather than renaming old ones, so
an exported World stays readable after an update.

## Editor interaction

The World Editor gains a `LAYOUT` instrument with four groups: **Frames**, **Surfaces**,
**Brand**, and **Icons**. Each row shows the concept name, its current source (`WORLD`, `PACK`,
or `EPOCH`), a preview and the only actions that are meaningful for that role:

- `IMPORT` / drag a supported image into the row;
- `REPLACE` a World override;
- `REMOVE OVERRIDE` to return to the Pack or fallback;
- for a `frame`, edit only the four corner insets, repeat mode and whole-number scale.

The browser turns selected files into bytes; the Engine validates format/size and selects the
destination under `assets/ui/`. The editor never receives a writable filesystem path. Changes
write `ui.toml` atomically, invalidate the current World UI cache and update the running HUD on
the next paint. There is no separate Save button or second source of truth.

## Boundaries and verification

- Icons and logos have text labels or accessible names supplied by code. Artwork cannot remove
  the label required to operate a control.
- User UI artwork is constrained by the same image-size limits and export validation as World
  artwork. A huge image cannot silently enter every HUD repaint.
- The resolver caches a resolved asset set once per World activation. Rendering dozens of Frames
  does not make dozens of IPC calls or repeatedly decode the same image.
- The migration is additive. Existing Packs that declare `[[ui]]` continue to provide frames;
  Worlds with no `ui.toml` retain today's CSS UI exactly.
- Tests will cover precedence, malformed artwork fallback, replacing/removing one override,
  no raw-path IPC, and a frame/image distinction that prevents a logo from being used as a
  border image.

This is intentionally a planned structural slice, not a cosmetic CSS shortcut. It begins only
after the current Fase 4 regression is accepted, because it changes the persisted World format
and deserves its own user verification.
