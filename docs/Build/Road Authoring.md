# Road authoring

A road belongs to the World owner, not to a character and not to the renderer. It is stored in
that World's `places.toml` as one stable road record:

```toml
[roads.road_1]
from = "tower"
to = "library"
via = [[420.0, 280.0], [590.0, 280.0]]
```

`from` and `to` are Place identities. They are deliberately not coordinates: moving either
building moves the end of the road with it. `via` is only the intermediate corners explicitly
traced by the World owner. Epoch never generates a route through terrain on the user's behalf.

## Editor workflow

1. Choose **ROAD** and click an origin building.
2. Click empty land to add visible corners, then click a different building to create the road.
3. The **Roads** panel lists every road owned by the vault as `origin → destination`.
4. Click a row (or **Trace route** in a selected road) to load its existing corners into the
   stage. Add corners, undo trailing corners, then choose **Save route**.
5. The row's **X** removes only that road. Removing a Place also removes roads that lead to it.

The list is intentionally limited to vault-authored roads. Routes declared by a World Pack are
pack geography: the editor may draw them, but must not display a deletion affordance for content
it cannot write. This keeps a World Pack and its local map from appearing to disagree.

## What a pack may still declare, and what it no longer may *(2026-08-10)*

A pack route is `from`, `to` and `prominence`. **It has no `waypoints`.**

It used to. Corners let a shipped road bend around water, and they were authored against the
positions the pack itself held. [ADR-0028](../ADR/0028-the-map-belongs-to-the-user.md) moved
positions to the user and left the corners behind, anchored to nothing — a corner is only
meaningful relative to the two ends it bends between.

Measured, not argued: the default pack's corners were drawn for a 3,600 x 2,200 map. A user
painted 1,536 x 1,024 land, moved the buildings into it, and its roads left the buildings,
crossed open ocean to coordinates that no longer described anything, and came back. Nothing on
screen could explain it, because the data was correct and its meaning had expired.

So a corner belongs to whoever placed the buildings, and that is the user.

**The default pack now ships no roads at all.** A World arrives to be shaped: its land, terrain
and buildings are the author's, and where the roads run is the first decision its owner makes.
Pack routes remain supported — a pack may honestly say *that* two Places are joined and how
travelled the road is — and a pack authored before this change still loads, with its corners
read and dropped rather than refused. A World that will not open is a worse answer than one
whose roads run straight between real doors.

## When both draw the same pair

The user's road wins, and the pack's is not drawn.

It was the other way round, which failed silently: tracing a road between two Places the pack
already joined kept the pack's line, discarded every corner just drawn, and said nothing. It
read as a save that did not work. Replacement is per pair — drawing one road never removes the
World's others.

## Guarantees

- A pair of Places has at most one road; re-tracing replaces its `via` points instead of creating
  a second indistinguishable line.
- Invalid, repeated and non-finite corners are normalized at the Engine boundary before the file
  is written.
- The editor's *"no road leads to"* warning counts pack routes as well as vault roads. It sits
  directly under the map, and counting only one source made it contradict what was drawn above
  it — five buildings reported unreachable while four roads led to them.
- Roads never authorize or block a Quest handoff. They are the visible route a handoff can take,
  not a dependency that scenery can use to stop work.
- All writes use the editor's existing map history, reread the vault, and refresh the World. A
  failed edit leaves the map and screen at the same previous state.
