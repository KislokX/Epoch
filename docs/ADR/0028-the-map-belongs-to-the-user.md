# ADR-0028: The map belongs to the user

- Status: Accepted
- Date: 2026-08-05
- Depends on: [[0016-asset-resolution]], [[0021-places-as-compositions]], [[0023-characters-belong-to-the-user]], [[0024-imported-artwork]]
- Amends: [[0017-character-archetypes-world-packs]] (a World Pack no longer supplies the map), [[0021-places-as-compositions]] (`PlaceId` becomes a Kernel identity, and a Place may exist without a concept)
- Related architecture: [[../../PRODUCT_ARCHITECTURE]], [[../../CONTENT_PHILOSOPHY]], [[../../EXPERIENCE_CONSTITUTION]]
- Horizon: place identity, rename, sprite override and the vault store **IMPLEMENT NOW**; the World Editor, authored terrain and roads **IMPLEMENT NOW**; travel along roads **DESIGN NOW** (waits on `animated_sprite`); World export/import **DESIGN NOW**; a World marketplace **VISION**

## Context — the evidence

ADR-0023 moved the cast from the pack to the user. Its argument was that `CharacterArchetype`
was doing the work of an identity, and that the moment a user wants two researchers the second
silently replaces the first. It justified that move by citing Places as the case already done
correctly:

```
PlaceConcept        what capability this projects        classification
PlaceId             which Place this is                  identity
```

**That citation is not true of the build.** `PlaceId` exists — `crates/epoch-engine/src/place.rs:59` — but it lives in the Engine and is used only by the asset pipeline, to resolve a pack's declarations into a composition. It is a rendering handle. It is not in the Kernel, and nothing about a character's life refers to it.

Everything that governs behaviour is keyed on the concept:

```rust
// crates/epoch-kernel/src/presence.rs:38  — where somebody IS
pub place: PlaceConcept,
// crates/epoch-kernel/src/definition.rs:152 — where somebody LIVES
pub home: PlaceConcept,
```

And `PlaceConcept::ALL` is a closed set of five.

So Places have exactly the defect ADR-0023 named and fixed for characters, in the same words: **the classification is doing the work of the identity.** Two characters cannot live in two different laboratories, because "laboratory" *is* the address. ADR-0023 did not catch it because ADR-0021 said the split existed, and it read the document rather than the code.

A second, larger problem arrived from use rather than from reading. A user asked to rename a building and give it their own sprite, and then to create buildings and roads between them. Under the current model none of that is expressible:

- **A name cannot be changed**, because the name is the pack's and the pack is replaced on update.
- **A building cannot be created**, because a Place must be one of five concepts and there is no sixth. "The Forge" has nowhere to go.
- **A road cannot exist**, because there is no stable thing for it to point at.

The five frozen concepts were correct while the pack supplied the world. They are wrong the moment the user builds the world — and building the world is what Epoch is for. Keeping them removes customisation from a product whose stated goal is that anyone can orchestrate AI as easily as playing a video game.

## Decision

### 1. A Place has an identity, and it is not its name

Identity, name and classification are three separate things — the same split ADR-0023 made for characters, now made honestly for Places.

```toml
# vault/worlds/<world>/places.toml
[places.building_1]          # identity — assigned once, never changes, never reused
name    = "The Forge"        # free text, renamed as often as the user likes
concept = "research_lab"     # optional classification
mark    = "forge.png"        # the user's artwork, resolved like any other (ADR-0024)

[roads.road_1]
from = "building_1"          # points at identity
to   = "building_4"
```

`PlaceId` moves into the Kernel and becomes the address for presence, for a character's home, for a road's ends and for History.

**Identities are sequential and readable, and they are never recycled.** `building_1` can be opened and understood in a text editor, which a hash cannot; and refusing to reuse a deleted number means a Quest recorded in `building_2` never silently relocates into a building created later that happens to take the name back.

### 2. `PlaceConcept` stops being an address and becomes a classification — and it is optional

A concept says what a Place *projects*: a Laboratory wakes when a provider is installed (ADR-0021), a Command Center answers to the Quest system. That is real behaviour and it stays.

But a Place with **no** concept is now legal. It exists, it can be visited, it holds inhabitants, and no subsystem lights it up. That is an honest thing for a building to be, and it is the only way "The Forge" can exist before anybody decides what a Forge does.

The five concepts are no longer frozen and no longer exhaustive. They are the set the Engine currently knows how to animate.

### 3. The map is vault data, not pack data

`places.toml` lives in the Workspace — `vault/worlds/<world>/` — never in the pack.

A pack is shipped content and an update replaces it (ADR-0016). A name the user chose must not be replaceable by somebody else's release, and artwork the user imported must not vanish when they try a different pack. This is ADR-0024's rule applied again: **authored content overrides derived rendering, and derived rendering never goes away.** A World with no overrides renders exactly as the pack intends; a World with them is still complete if the pack changes.

### 4. The World Editor is where a World is authored, and it is part of the Launcher

Creating a Workspace is already the Launcher's job, and authoring a World *is* preparation — so
the editor is a mode of the Launcher rather than a fourth Experience Surface. Making a World
starts there: choose the terrain, place the buildings, give them sprites and names, decide who
lives where.

This answers "where does a new building go" by refusing the question. Neither the centre of the
map nor a random spot is an answer, because neither is a decision anybody made. **The user
places it**, and the position is authored data like everything else here.

**Editing happens only here, and getting here freezes the World.** The live World has no Rename
and no Change Sprite; right-clicking a building there does nothing but what the shell menu
already offers. `[Edit World]` moves the Workspace into a **frozen** state and the editor opens;
leaving it thaws.

The freeze is not a convenience. While the World is frozen the simulation clock does not
advance, so **no presence transition can happen against a map that is changing underneath it**.
That is what keeps ADR-0018's causality rule true during an edit: every transition still traces
to a real cause, because no transition is being resolved at all.

**A World with work in flight cannot be edited at all.** `[Edit World]` is refused while any
Quest is running, and the refusal **names the work** — *"Mage is still searching the web; finish
that before editing"* rather than *"a Quest is running"*. A refusal the user cannot act on is a
wall; one that says which character is doing what is a fact they can go and resolve. Same rule
as the cold instruments on the bridge: never a message with nothing measured behind it. That is stricter than deferring transitions until the thaw,
and it is better for the same reason the freeze itself is: it removes the question rather than
answering it. There is no state in which a handoff resolves against a map being rearranged,
because there is no handoff.

It also keeps the rule the crew already lives by intact. Characters go on working when the user
is not watching — that is the whole point of them — so the editor never interrupts work. It
waits for it.

This is worth more than tidiness: it removes a whole class of question by construction rather
than by handling it. *What happens when a building somebody is standing in is moved mid-turn?*
*What happens to a road a character is walking when it is deleted?* Neither can occur, so
neither needs an answer, and the World Simulation keeps exactly one source of change — real
Activities (ADR-0018).

The editor is still an editor over the vault, never a parallel store (ADR-0023): dragging a
building writes `places.toml`, and the file is the truth. Arranging by hand is the interface, not
a second store to reconcile.

### 5. Terrain is authored, and therefore it travels

A World's terrain becomes an image the user chooses, not a chart derived from its geography.

This is ADR-0024 applied a third time: **authored artwork overrides derived rendering, and
derived rendering never goes away.** A World with no terrain image is complete, not unfinished —
it renders the derived chart, exactly as it does today. A World with one renders that instead,
and the chart stays one step behind it.

Because it is authored rather than derived, terrain is part of what an export carries.

### 6. Roads carry work, and they can never block it

A road is not scenery and it is not a constraint. It is **how the World shows a handoff
happening**.

When the user hands a Quest from one character to another, the Quest moves — and ADR-0025 is
explicit that this is the strongest kind of cause precisely because it is *visible*. The World
answers by walking: the character's `animated_sprite` becomes `walking`, they travel to where
the receiving character is, and they return to idle on arrival. Every part of that traces to a
real Activity, so the causality rule of ADR-0018 holds without exception — nothing is invented
and nothing is staged.

That makes a road engine-driven rather than decorative, which is what the Living World Design
Guide requires of anything on screen.

**The invariant that keeps it honest:** the Quest decides that movement happens; the map decides
only what it looks like. If there is no road between two Places, the character still goes — they
simply cross open ground. A missing road may never prevent a handoff, because that would let
scenery veto work, and a World that can refuse to let people collaborate has stopped being a
projection of the Engine and started being a rule of its own.

### 7. Export carries appearance and nothing else

A World may be exported so that somebody else can make it theirs. What travels is **only what is visual**:

- terrain, roads, buildings, their names and their artwork
- character sprites, portraits, frame animations, icons
- music, sound, fonts, palette

What does **not** travel: credentials, provider configuration, project roots, Chronicles, Quests, History, Knowledge, trust decisions, and which model any character thinks with.

Stated as a structural guarantee rather than a filter, following ADR-0026's treatment of credentials: **the export type has no field for any of it.** A World Pack that cannot represent a project root cannot leak one, and that is held by the compiler rather than by a reviewer's attention.

An imported World arrives as a *base* — a place to start, which the importer then makes their own.

## Why this is best

It is the movement this codebase has already made once and found correct. ADR-0023's whole argument applies unchanged with "Place" substituted for "Character"; discovering that Places never actually received the split is a reason to finish the job, not to invent a different answer.

It also resolves the tension between ADR-0017 and the product. 0017 kept the Engine ignorant of names so that a pack could re-skin the world. That goal survives untouched — the Engine still never learns a name, only an identity and an optional concept. What changes is *who writes the names*, and the answer is the same as it was for the cast.

## Alternatives considered

- **Key a Place by its name.** Proposed during the discussion and rejected on its own example: a road from "The Forge" to "The Library" breaks the second time either is renamed, and so does every character living there and every Quest in History that happened there. Renaming would have to rewrite every reference in every file, and any reference that was missed fails silently — a character with no home, a road to nowhere. ADR-0022 already settled this: a Place has exactly one identity forever, addressed by identity and never by appearance. A name is appearance.

- **Keep the five concepts and let a user pick the closest one.** Rejected: it makes the user translate their idea into our vocabulary, which is the cognitive load this product exists to remove. It also produces four buildings all classified `research_lab` and therefore all lighting up when a provider is installed, which is worse than nothing lighting up.

- **Store overrides in the pack.** Rejected under ADR-0016: a pack is replaced on update, so the user's names would have a lifetime measured in releases.

- **Amend ADR-0017 in place rather than write this.** Rejected on the repository's own rule that a document answers exactly one question. 0017 answers *"how does the Engine avoid knowing about specific characters and places?"* — an answer that is still correct. This answers *"who owns the map?"*, which is a different question, and it is answered here exactly as ADR-0023 answered it for the cast. Both 0017 and 0021 receive a forward-pointing amendment note so neither is left asserting something the build no longer does.

## Consequences

- `PlaceId` moves from `epoch-engine` into `epoch-kernel` and becomes a domain identity.
- `PresenceState::place` and `PresenceProfile::home` change from `PlaceConcept` to `PlaceId`. This is the change that lets two characters live in two different laboratories.
- `PlaceConcept` becomes `Option<PlaceConcept>` wherever a Place declares one.
- A new vault store, `places.toml`, per Workspace, with the Launcher and the World both editing the file they read (ADR-0023's rule: an editor over the vault, never a parallel store).
- Roads become user data with identities of their own, rather than geometry the pack supplies.
- The World gains a right-click menu on a Place: its effective name, Rename, Change Sprite.
- The Launcher gains an authoring mode, reachable from the World through `[Edit World]`.
- A Workspace gains a **frozen** state: the simulation clock stops, the Engine does not.
- The live World gains nothing else: it renders `places.toml` and never writes it.
- Imported artwork — for a character or a building — is resized to a shared reference so a World cannot end up looking assembled from several different games.
- Terrain gains an authored image alongside the derived chart, imported through the pipeline ADR-0024 already built — bytes over IPC, named by the Engine from the bytes, never by the uploaded name.
- Roads become a reason for `animated_sprite` to exist. Travel along them is DESIGN NOW: the data lands with this ADR, the walking lands when sprites can walk.
- Existing Worlds have no `places.toml`. The pack's declarations are the starting state, and identities are assigned on first write — so nothing needs migrating and nothing breaks before the user touches anything.

## Assumptions

- A user who renames a building wants the building to stay the same building. Everything here rests on that; if renaming were meant to *replace* a Place, identity would be the wrong model.
- Sequential identities are readable enough that a vault stays inspectable by hand.

## Risks

- **`PlaceConcept` is load-bearing in the simulation.** Changing presence to `PlaceId` touches routines, idle behaviour and arrival. The causality rule of ADR-0018 must survive it: every transition still traces to a real cause.
- **An optional concept can be left empty by everybody**, leaving a World where nothing ever lights up. Mitigated by the pack's own Places arriving with their concepts already set — the user has to opt out, not opt in.
- **Export is a place where a leak is invisible until it has happened.** The structural guarantee is the mitigation, and it needs a test asserting the export type cannot be constructed with anything sensitive in it.

## What the build revealed (amendment, 2026-08-06)

Architecture changes require implementation evidence, so these are recorded with what the build
actually showed rather than with what seemed likely.

**A Place needed a position before it needed anything else.** Building one worked, naming it
worked, and it was invisible: a `Place` with no `placement` is not drawn. So `PlaceEntry` gained
a `Spot` — `x`, `y`, and an *optional* footprint, because dragging a building the pack drew must
not silently hand it a default size. Moving and resizing stayed two facts all the way to the
file. A Place standing nowhere remains legal and is reported while authoring, not fixed by
dropping it at the origin, which would hide unfinished work under a building in the corner of
every World.

**Roads were stored and never rendered.** `MapView.routes` came from the pack alone, so joining
two buildings changed a file and nothing else. User roads are now projected as straight lines —
a pack route carries authored waypoints because somebody drew a path through their geography; a
road drawn by joining two buildings says only *these two are connected*, and bending it would be
the World inventing terrain it was never told about. A World whose pack declares no geography
derives its extent from what stands in it, so the first building somebody places brings a map
with it.

**A character's height was a fraction of the building beside them.** `scale` multiplied the
Place's footprint, so enlarging a building enlarged whoever stood there, and walking to a smaller
one shrank them — their size changed because of where they were, which is not a fact about them.
Height is now a fraction of a fixed reference equal to the default footprint, so the drawing is
unchanged at that size and nobody's height depends on their location. *Position* still comes from
the Place: the spawn anchor is declared in footprint units, so a large building spaces people
further from its door, which genuinely is a fact about the building.

This resolves the last open question below in the direction it was leaning. The reference is a
constant today rather than a World's declaration — **Earn Complexity**: making it authored costs
a manifest field, a resolution path and a migration, and buys nothing until two Worlds want
different answers. The line to watch is the same one the question named: whichever way it is
authored, replacing one character must never rescale everyone.

**Imported artwork replaces; it does not stack.** For a building and for the land alike. Every
other field of an overlay entry leaves the pack's answer standing when absent, because absence
means *the pack decides* — but a present drawing is somebody saying what that thing looks like
now, and drawing it over the pack's would render two buildings, or two terrains, in one place.
Clearing restores the derived rendering, which was never removed (ADR-0024).

**Large artwork may not ride in the projection.** `world:changed` re-sends the World whenever
anybody's presence changes. A building's sprite is small and resolves when the map loads; the
land is a full-map illustration, so it is a filename in `places.toml` and its bytes come from
their own command, fetched once. Same rule the pack's backdrop already followed, now stated for
anything the user imports.

**Undo had to be real or absent.** A visible Undo that changes nothing teaches the user to
distrust every other control. It is a bounded in-memory stack of whole `WorldMap` snapshots for
the current editing session — whole snapshots rather than inverse operations, because a map is
kilobytes and an inverse operation has to be written for every edit and be right about how they
interact, and the first one that is wrong corrupts a World silently. The stack is dropped when
the editor closes: Undo is an offer to take back what you *just* did, not a version history.

**One gesture, one meaning, and the guard is an absence.** The renderer receives an editing
object or nothing at all; a running World is handed no way to be edited rather than handed one
and told not to use it. Two defects found by using it, both from a gesture doing a second job:
a right-drag moved a building because the grab handler did not check the button the way the
camera already did, and dragging painted a text selection across the World because that fix had
been written on the running World's stage rather than on the idea. Immersion leaks, found the
way they always are — by using the product.

## What the build revealed (amendment, 2026-08-10)

**A connection is not a route.** Joining two buildings correctly established that work may pass
between them, but rendering that fact as a straight line made the World author a path through
water, buildings and terrain the owner had plainly avoided with their eye. `Road` therefore
keeps only `via`: the world-unit corners the owner traced. Its `from` and `to` remain Place
identities and are resolved at projection time, so moving a building moves the corresponding end
of its route without writing two competing coordinates. A legacy road with no corners remains a
perfectly valid straight connection.

The World Editor records those corners one deliberate click at a time: choose a first building,
trace any bends on the land, then choose the destination. The unfinished stroke is a visual draft
only; it never reaches `places.toml` until both endpoints exist. Undoing a corner or cancelling
the draft changes no vault data. An existing road is removed from either endpoint's Connections
list, which keeps deletion an explicit authored action rather than an accidental gesture.

The same projected polyline guides a visible traveller when there is a route between their two
Places. The timer remains a duration over the journey rather than a new distance rule: roads
never decide whether work can happen or how long the Engine says it takes.

The running World's `LAYOUT` controls share the editor's visibility vocabulary. They hide a
drawn layer locally and never alter terrain, roads, buildings, people or labels in the vault.

## Open questions

The three questions this ADR opened were answered in review and moved into the Decision: a World
Editor places new buildings, roads carry handoffs, and terrain is authored and travels. What
remains:

- **What does a character stand on while walking between two Places with no road?** Open ground is the answer for *whether* they go; it is not yet an answer for what the ground is. Terrain is one image, so there is nothing underneath to ask.
- **Can two characters occupy the same Place while one is passing through?** Presence has a current Place and a destination; "in transit between them" is a third state the projection does not have a word for yet.
- **What does a frozen World look like?** It must be legibly stopped rather than appear broken — a character standing still because time is paused should not read as a character who has crashed.
- ~~**Where does the reference height live?**~~ **Resolved 2026-08-06** — see *What the build revealed*. A fixed reference equal to the default footprint, kept a constant until two Worlds want different answers. The original reasoning, unchanged: Measured from the World as it stands today: the default pack's character sprite is 18×32 and an imported one was 1080×1080 — thirty-four times taller, which is what "assembled from different games" looks like. So **height is the unit and width is free**: an import is scaled to 32px tall with its aspect preserved, and whatever width that produces is the width. A 1080×1080 sprite becomes 32×32; a wide one becomes 64×32. Constraining the width too would either stretch a sprite or pad it, and neither is a thing the author asked for — what makes a World look coherent is that everyone is the same height, not that everyone occupies the same box. What is still open is *where the number is authored* — it must be a World's declaration rather than whatever the first import happened to measure, or replacing one character would silently rescale everything.
