//! The map, as the user owns it (ADR-0028).
//!
//! `vault/worlds/<world>/places.toml` — what the user named their buildings, what artwork they
//! gave them, and which roads run between them.
//!
//! ## It overrides; it does not replace
//!
//! A World with no `places.toml` is **complete**, not unfinished: the pack's own declarations
//! render exactly as their author intended. This file only ever says *"and here is what I
//! called that one"*. That is ADR-0024's rule a third time — authored content wins, derived
//! rendering never goes away — and it is what lets somebody try a different pack without
//! losing the names they chose.
//!
//! ## The vault, never the pack
//!
//! A pack is shipped content and an update replaces it (ADR-0016). A name the user chose must
//! not be replaceable by somebody else's release. Same argument that moved the crew out of
//! packs (ADR-0023) and the project root before that.
//!
//! ## Identities are never recycled
//!
//! `building_1` … `building_9`, and the counter only ever goes up. Delete `building_2`, create
//! another, and it is `building_10`. Reusing the number would silently relocate every Quest in
//! History that happened in the old one — and History is evidence, not narration (ADR-0025).
//!
//! Readable on purpose. A hash would be safer against collision and unreadable in the text
//! editor the vault is meant to be opened in.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use epoch_kernel::{PlaceConcept, PlaceId};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum MapError {
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Malformed(String),
}

/// What the user said about one Place.
///
/// Every field optional, and that is the design: a Place named but not re-drawn keeps the
/// pack's artwork, and one re-drawn but not renamed keeps the pack's name. Overriding one thing
/// must never quietly discard another.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PlaceEntry {
    /// What it is called here. `None` leaves the pack's title alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// What kind of Place this is, if it is a kind the Engine can animate.
    ///
    /// **Optional** (ADR-0028). A Place with no concept exists, is visited, holds inhabitants,
    /// and no subsystem lights it up. That is an honest thing for a building to be, and it is
    /// the only way somebody can create "The Forge" before anyone has decided what a Forge
    /// does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concept: Option<PlaceConcept>,
    /// A file the user imported, resolved like any other artwork (ADR-0024). Name only — the
    /// Engine decides where it lives, and names the file from the bytes rather than from what
    /// was uploaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mark: Option<String>,
    /// The artwork as a `data:` URI, filled during load.
    ///
    /// **Never authored, never written back.** Exactly like `Mark::asset_data`: the file on
    /// disk is the truth and this is what the renderer can draw. Keeping it out of the file is
    /// what stops a vault from growing a megabyte of base64 inside a text file meant to be
    /// opened by hand.
    #[serde(skip)]
    pub art: Option<String>,
    /// Where the user put it. `None` leaves the pack's position alone — and for a Place the
    /// user built, means it exists but has not been put anywhere yet.
    ///
    /// A Place with nowhere to be is legal and visible *as* unplaced: `Place::placement` was
    /// already an `Option` because a partially authored World is allowed to exist. The editor
    /// says so; the World does not pretend it is at the origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spot: Option<Spot>,
}

/// Where the user put a Place, in world units.
///
/// Separate from the pack's `Placement` and deliberately smaller: no `z_order`, because draw
/// order already falls out of `y` and an authored override is the pack author's business, not
/// something a drag gesture should be able to invent.
///
/// `footprint` is optional for the same reason every other field of an entry is: moving a
/// building the pack drew must not quietly resize it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Spot {
    pub x: f32,
    pub y: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub footprint: Option<f32>,
}

/// How big a Place the user built is, until they say otherwise.
///
/// The smallest footprint the shipped pack uses. A new building that arrived larger than
/// everything around it would read as important, which is a claim the Engine has no business
/// making about a building somebody just named.
pub const DEFAULT_FOOTPRINT: f32 = 90.0;

/// A way between two Places.
///
/// It carries work: when a Quest is handed from somebody in one building to somebody in
/// another, the World answers by walking it (ADR-0028). It is never a constraint — a missing
/// road may not prevent a handoff, because scenery must not be able to veto work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Road {
    pub from: PlaceId,
    pub to: PlaceId,
    /// Corners the World owner traced between the two endpoints, in world units.
    ///
    /// The endpoints deliberately are not copied here: a road must follow a Place when that
    /// Place moves. Only the owner's intermediate intent is stored, which means a relocated
    /// building updates the road without a second edit and without two coordinates that can
    /// disagree about where its door is.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub via: Vec<[f32; 2]>,
}

impl Road {
    /// Whether this road joins these two Places, in either direction.
    ///
    /// Undirected because a path somebody can walk one way is a path they can walk back. If a
    /// one-way road ever means something, it will mean it because somebody decided that, not
    /// because `from` and `to` happened to be stored in that order.
    pub fn joins(&self, a: &PlaceId, b: &PlaceId) -> bool {
        (&self.from == a && &self.to == b) || (&self.from == b && &self.to == a)
    }
}

/// One World's map.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorldMap {
    #[serde(default)]
    pub places: BTreeMap<PlaceId, PlaceEntry>,
    #[serde(default)]
    pub roads: BTreeMap<String, Road>,
    /// The land the user painted, if they painted any.
    ///
    /// A filename, and **only** a filename. Unlike a Place's artwork this is not resolved when
    /// the map loads: a full-map illustration is megabytes, and `world:changed` re-sends the
    /// World's projection every time anybody's presence changes — every few seconds. Reading it
    /// eagerly would hold it in memory for a map that is mostly consulted for names and roads,
    /// and putting it in the projection would push it across IPC on every tick.
    ///
    /// So it is fetched once, on demand, by [`Self::land_data`]. Same rule the pack's backdrop
    /// already follows, and the same reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub land: Option<String>,
    /// How big that image is, in pixels — which **is** how big this World is.
    ///
    /// One pixel, one world unit. A rule anybody can hold: your map is the World, at the size
    /// you drew it, and what lines up in your image lines up with where the buildings stand.
    ///
    /// Stored rather than measured on demand, because the extent is needed on every projection
    /// and the image is megabytes. Written once, at import, from the file's own header.
    ///
    /// `None` when it could not be measured. The land still draws; the World simply keeps
    /// whatever extent it already had.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub land_size: Option<[f32; 2]>,
    /// How many Places have ever been created here.
    ///
    /// Stored rather than derived from `places.len()`, which is the whole point: deleting a
    /// building must not make the next one reuse its number.
    #[serde(default)]
    minted: u32,
}

impl WorldMap {
    /// Read this World's map. A missing or unreadable file is an empty map, never an error.
    ///
    /// The World always opens (Build From Life, rule 1). A map that could refuse to load would
    /// be a way for a text file to make a World unenterable.
    pub fn load(vault: &Path, world: &str) -> Self {
        let file = path(vault, world);
        let mut map: Self = match std::fs::read_to_string(&file) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_default(),
            Err(_) => return Self::default(),
        };

        // Read the artwork now, once, against the World's own folder. Resolving per projection
        // would touch the filesystem on every simulation tick; resolving here also gives the
        // right invalidation for free, because reloading the map re-reads its images.
        //
        // Confined to the World and delivered as a `data:` URI like every other Asset, so
        // imported artwork cannot execute script (ADR-0020). Unreadable artwork costs the
        // artwork and never the World.
        let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
        for entry in map.places.values_mut() {
            let Some(reference) = &entry.mark else {
                continue;
            };
            entry.art = crate::asset::resolve(&dir, reference)
                .ok()
                .map(|resolved| resolved.data_uri);
        }
        map
    }

    /// Where a World keeps the artwork its owner imported.
    pub fn art_dir(vault: &Path, world: &str) -> PathBuf {
        vault.join("worlds").join(world)
    }

    /// The land as a `data:` URI, read now.
    ///
    /// `None` when there is none and when it could not be read — both mean the same thing to a
    /// renderer that already knows how to draw a World from its own geography. Land that will
    /// not load costs the land, never the World.
    pub fn land_data(&self, vault: &Path, world: &str) -> Option<String> {
        let reference = self.land.as_deref()?;
        crate::asset::resolve(&Self::art_dir(vault, world), reference)
            .ok()
            .map(|resolved| resolved.data_uri)
    }

    /// Paint the land, or strip it back to the World's own geography.
    ///
    /// Size and file move together, always. Two fields that can disagree about which image is
    /// on the ground would eventually disagree, and the symptom would be a World stretched to
    /// the shape of a picture nobody can see any more.
    pub fn repaint(&mut self, file: Option<String>, size: Option<[f32; 2]>) {
        self.land_size = file.as_ref().and(size);
        self.land = file;
    }

    pub fn save(&self, vault: &Path, world: &str) -> Result<(), MapError> {
        let body = toml::to_string_pretty(self).map_err(|e| MapError::Malformed(e.to_string()))?;
        let file = path(vault, world);
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|source| MapError::Write {
                path: dir.to_path_buf(),
                source,
            })?;
        }
        std::fs::write(&file, body).map_err(|source| MapError::Write { path: file, source })
    }

    /// A Place identity nobody has used and nobody will use again.
    pub fn mint(&mut self) -> PlaceId {
        self.minted += 1;
        PlaceId::new(format!("building_{}", self.minted))
    }

    /// Build something new, somewhere.
    ///
    /// `at` is an `Option` rather than a default, and the surface always supplies one: only the
    /// caller knows how big this World is and where the user was looking. Guessing a position
    /// here would put every new building at the same coordinate — which is worse than nowhere,
    /// because a pile of overlapping buildings looks like a rendering bug rather than like
    /// work left to do.
    pub fn add(&mut self, name: impl Into<String>, at: Option<Spot>) -> PlaceId {
        let id = self.mint();
        self.places.insert(
            id.clone(),
            PlaceEntry {
                name: Some(name.into()),
                spot: at.map(|spot| Spot {
                    footprint: Some(spot.footprint.unwrap_or(DEFAULT_FOOTPRINT)),
                    ..spot
                }),
                ..PlaceEntry::default()
            },
        );
        id
    }

    /// Give a Place a different name. Its identity does not move.
    pub fn rename(&mut self, id: &PlaceId, name: impl Into<String>) {
        self.places.entry(id.clone()).or_default().name = Some(name.into());
    }

    /// Give a Place different artwork. Its identity does not move.
    pub fn redraw(&mut self, id: &PlaceId, mark: impl Into<String>) {
        self.places.entry(id.clone()).or_default().mark = Some(mark.into());
    }

    /// Put a Place somewhere. Its identity does not move; only the building does.
    ///
    /// Keeps whatever footprint it already had — dragging a building must not resize it, and
    /// the two gestures stay separate all the way down to the file.
    pub fn place_at(&mut self, id: &PlaceId, x: f32, y: f32) {
        let entry = self.places.entry(id.clone()).or_default();
        let footprint = entry.spot.and_then(|spot| spot.footprint);
        entry.spot = Some(Spot { x, y, footprint });
    }

    /// Change how big a Place is.
    ///
    /// Refused for a Place that is nowhere: a size without a position is a fact about a
    /// building that is not standing anywhere, and storing it would let a later drag silently
    /// resurrect a number the user has long forgotten choosing.
    pub fn resize(&mut self, id: &PlaceId, footprint: f32) -> bool {
        match self
            .places
            .get_mut(id)
            .and_then(|entry| entry.spot.as_mut())
        {
            Some(spot) => {
                spot.footprint = Some(footprint);
                true
            }
            None => false,
        }
    }

    /// Remove a Place, and every road that led to it.
    ///
    /// A road to a building that no longer exists is a road to nowhere, and something would
    /// eventually try to walk it. The identity itself is never reissued.
    pub fn remove(&mut self, id: &PlaceId) {
        self.places.remove(id);
        self.roads
            .retain(|_, road| &road.from != id && &road.to != id);
    }

    /// Join two Places. Returns the road's id.
    ///
    /// Joining the same pair twice does not make a second road — a duplicate would draw over
    /// itself and mean nothing either way.
    pub fn connect(&mut self, from: PlaceId, to: PlaceId) -> String {
        if let Some((id, _)) = self.roads.iter().find(|(_, r)| r.joins(&from, &to)) {
            return id.clone();
        }
        self.connect_with_via(from, to, Vec::new())
    }

    /// Join two Places along the corners their owner drew. Re-drawing an existing connection
    /// changes its line instead of creating an indistinguishable duplicate.
    pub fn connect_with_via(&mut self, from: PlaceId, to: PlaceId, via: Vec<[f32; 2]>) -> String {
        let via = normalize_via(via);
        if let Some((id, road)) = self.roads.iter_mut().find(|(_, r)| r.joins(&from, &to)) {
            road.via = via;
            return id.clone();
        }
        let id = format!("road_{}", self.roads.len() + 1);
        self.roads.insert(id.clone(), Road { from, to, via });
        id
    }

    /// Whether these two Places are joined.
    pub fn connected(&self, a: &PlaceId, b: &PlaceId) -> bool {
        self.roads.values().any(|r| r.joins(a, b))
    }

    /// Places that exist here but nothing leads to.
    ///
    /// The World Editor's job, not the World's: a road is required, and the way to keep that
    /// requirement from ever blocking work is to refuse to leave a building unreachable while
    /// it is being authored (ADR-0028). A World with one Place has no unreachable ones — there
    /// is nowhere to be cut off *from*.
    ///
    /// `also_joined` carries the pairs some other layer connects — today, the pack's own
    /// routes. It is a parameter rather than something read here because this document knows
    /// only the roads its owner drew, and counting only those made the warning contradict the
    /// map it sits under: five buildings reported unreachable while four roads were visibly
    /// drawn to them. An instrument that disagrees with the World teaches people to stop
    /// reading the instruments.
    pub fn unreachable(&self, also_joined: &[(String, String)]) -> Vec<PlaceId> {
        if self.places.len() < 2 {
            return Vec::new();
        }
        self.places
            .keys()
            .filter(|id| {
                let mine = self.roads.values().any(|r| &&r.from == id || &&r.to == id);
                let theirs = also_joined
                    .iter()
                    .any(|(from, to)| from == id.as_str() || to == id.as_str());
                !mine && !theirs
            })
            .cloned()
            .collect()
    }
}

/// Keep a traced route compact and well-formed before it reaches the vault.
///
/// Repeated points do not change the line yet make every future reader do more work. NaN and
/// infinity cannot be drawn meaningfully by SVG or TOML, so discarding them at the domain
/// boundary leaves an incomplete gesture as a harmless straight segment instead of a corrupted
/// map file.
fn normalize_via(via: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    let mut compact = Vec::with_capacity(via.len());
    for point in via {
        if !point[0].is_finite() || !point[1].is_finite() {
            continue;
        }
        if compact.last().is_some_and(|last: &[f32; 2]| *last == point) {
            continue;
        }
        compact.push(point);
    }
    compact
}

fn path(vault: &Path, world: &str) -> PathBuf {
    vault.join("worlds").join(world).join("places.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dir(PathBuf);
    impl Dir {
        fn new(tag: &str) -> Self {
            let d = std::env::temp_dir().join(format!("epoch-map-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_world_with_no_map_is_complete_rather_than_broken() {
        // The pack's own Places render exactly as authored. This file only ever adds.
        let d = Dir::new("empty");
        let map = WorldMap::load(&d.0, "archipelago");
        assert!(map.places.is_empty());
        assert!(map.roads.is_empty());
    }

    #[test]
    fn an_unreadable_map_still_opens_the_world() {
        // Build From Life rule 1. A text file must not be able to make a World unenterable.
        let d = Dir::new("broken");
        let file = path(&d.0, "archipelago");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "this is not toml {{{").unwrap();

        assert_eq!(WorldMap::load(&d.0, "archipelago"), WorldMap::default());
    }

    #[test]
    fn renaming_does_not_move_a_place() {
        // The reason identity exists at all. A road, a home and a Quest in History all point
        // here, and none of them may notice that the sign on the door changed.
        let mut map = WorldMap::default();
        let forge = map.add("The Forge", None);
        let library = map.add("The Library", None);
        map.connect(forge.clone(), library.clone());

        map.rename(&forge, "La Fragua");

        assert_eq!(map.places[&forge].name.as_deref(), Some("La Fragua"));
        assert!(
            map.connected(&forge, &library),
            "the road still joins the same two places"
        );
    }

    #[test]
    fn a_deleted_identity_is_never_handed_out_again() {
        // Reusing it would silently relocate whatever History recorded in the old building.
        let mut map = WorldMap::default();
        let first = map.add("The Forge", None);
        map.remove(&first);
        let second = map.add("The Smithy", None);

        assert_ne!(first, second);
        assert_eq!(second, PlaceId::new("building_2"));
    }

    #[test]
    fn deleting_a_place_takes_its_roads_with_it() {
        // A road to nowhere is something a character would eventually try to walk.
        let mut map = WorldMap::default();
        let a = map.add("The Forge", None);
        let b = map.add("The Library", None);
        map.connect(a.clone(), b.clone());

        map.remove(&b);

        assert!(map.roads.is_empty());
        assert!(!map.connected(&a, &b));
    }

    #[test]
    fn a_traced_road_keeps_only_its_authored_corners() {
        let mut map = WorldMap::default();
        let forge = map.add("The Forge", None);
        let library = map.add("The Library", None);

        map.connect_with_via(
            forge.clone(),
            library.clone(),
            vec![
                [100.0, 80.0],
                [100.0, 80.0],
                [f32::NAN, 3.0],
                [220.0, 180.0],
            ],
        );

        let road = map.roads.values().next().expect("the road was created");
        assert_eq!(road.via, vec![[100.0, 80.0], [220.0, 180.0]]);

        // A legacy command can observe an already traced road without flattening it.
        map.connect(library, forge);
        assert_eq!(map.roads.len(), 1);
        assert_eq!(map.roads.values().next().unwrap().via.len(), 2);
    }

    #[test]
    fn a_road_runs_both_ways_and_only_once() {
        let mut map = WorldMap::default();
        let a = map.add("The Forge", None);
        let b = map.add("The Library", None);

        let first = map.connect(a.clone(), b.clone());
        let again = map.connect(b.clone(), a.clone());

        assert_eq!(first, again, "the same pair is the same road");
        assert_eq!(map.roads.len(), 1);
        assert!(map.connected(&b, &a));
    }

    #[test]
    fn a_place_nothing_leads_to_is_reported_before_anybody_needs_to_walk_there() {
        // Roads are required and must never block work, so the refusal belongs to authoring
        // time — where it is help — rather than to the moment of a handoff, where it would be
        // scenery vetoing collaboration.
        let mut map = WorldMap::default();
        let a = map.add("The Forge", None);
        let b = map.add("The Library", None);
        let orphan = map.add("The Watchtower", None);
        map.connect(a, b);

        assert_eq!(map.unreachable(&[]), vec![orphan]);
    }

    #[test]
    fn a_road_the_pack_drew_still_leads_somewhere() {
        // The warning sits directly under the map. Counting only this document's roads made it
        // contradict what was drawn above it, which is worse than no warning at all.
        let mut map = WorldMap::default();
        let a = map.add("The Forge", None);
        let b = map.add("The Library", None);
        let served = map.add("The Watchtower", None);
        map.connect(a, b.clone());

        let by_the_pack = [(served.to_string(), b.to_string())];
        assert!(map.unreachable(&by_the_pack).is_empty());
    }

    #[test]
    fn one_place_on_its_own_is_not_unreachable() {
        // There is nowhere to be cut off from. Reporting it would be a warning nobody can act
        // on, which teaches people to ignore the warnings that matter.
        let mut map = WorldMap::default();
        map.add("The Forge", None);
        assert!(map.unreachable(&[]).is_empty());
    }

    #[test]
    fn a_place_may_exist_with_no_concept_at_all() {
        // "The Forge" is not one of the five the Engine knows how to animate, and never will
        // be. It still exists, is visited, and simply lights up for nobody.
        let mut map = WorldMap::default();
        let forge = map.add("The Forge", None);
        assert!(map.places[&forge].concept.is_none());
    }

    #[test]
    fn painted_land_is_a_filename_in_the_file_and_bytes_only_on_request() {
        // The rule that keeps a megabyte illustration out of a text file meant to be opened by
        // hand — and out of the projection, which is re-sent whenever anybody moves.
        let d = Dir::new("land");
        let mut map = WorldMap::default();
        map.repaint(Some("land.png".into()), Some([1920.0, 1080.0]));
        map.save(&d.0, "archipelago").unwrap();

        let raw = std::fs::read_to_string(path(&d.0, "archipelago")).unwrap();
        assert!(raw.contains("land.png"));
        assert!(
            !raw.contains("data:"),
            "the bytes never enter the vault file"
        );

        // And with no file behind it, asking for the bytes is a `None` rather than an error:
        // land that will not load costs the land, never the World.
        assert!(WorldMap::load(&d.0, "archipelago")
            .land_data(&d.0, "archipelago")
            .is_none());
    }

    #[test]
    fn stripping_the_land_leaves_the_world_its_own_geography() {
        // Authored artwork wins and the derived chart never goes away (ADR-0024). Clearing is
        // an absence, which is what the renderer already knows how to draw.
        let mut map = WorldMap::default();
        map.repaint(Some("land.png".into()), Some([1920.0, 1080.0]));
        map.repaint(None, None);
        assert!(map.land.is_none());
        assert!(
            map.land_size.is_none(),
            "the size goes with the picture, always"
        );
    }

    #[test]
    fn a_map_survives_the_file_it_is_written_to() {
        let d = Dir::new("roundtrip");
        let mut map = WorldMap::default();
        let a = map.add("The Forge", None);
        let b = map.add("The Library", None);
        map.connect(a.clone(), b);
        map.redraw(&a, "forge.png");

        map.save(&d.0, "archipelago").unwrap();
        let read = WorldMap::load(&d.0, "archipelago");

        assert_eq!(read, map);
        // Including the counter — otherwise reloading would start handing out used numbers.
        assert_eq!(read.clone().mint(), PlaceId::new("building_3"));
    }

    #[test]
    fn a_building_can_be_moved_without_being_resized() {
        // Two gestures, two facts, all the way down to the file. Dragging a building the pack
        // drew must not quietly hand it the default size.
        let mut map = WorldMap::default();
        let forge = map.add(
            "The Forge",
            Some(Spot {
                x: 10.0,
                y: 10.0,
                footprint: Some(150.0),
            }),
        );

        map.place_at(&forge, 400.0, 250.0);

        let spot = map.places[&forge].spot.unwrap();
        assert_eq!((spot.x, spot.y), (400.0, 250.0));
        assert_eq!(
            spot.footprint,
            Some(150.0),
            "it is the same building, somewhere else"
        );
    }

    #[test]
    fn moving_a_place_the_pack_drew_says_nothing_about_its_size() {
        // The entry did not exist until now, and `None` has to keep meaning "the pack decides".
        // Writing a footprint here would resize a building the user only dragged.
        let mut map = WorldMap::default();
        let lab = PlaceId::new("research_lab");

        map.place_at(&lab, 120.0, 80.0);

        assert_eq!(map.places[&lab].spot.unwrap().footprint, None);
        assert!(
            map.places[&lab].name.is_none(),
            "and it is still called what the pack called it"
        );
    }

    #[test]
    fn a_building_that_is_nowhere_cannot_be_given_a_size() {
        // A size with no position is a fact about a building that is not standing anywhere. Kept,
        // it would reappear the moment somebody dragged the Place — a number nobody remembers
        // choosing, applied at a moment nobody connected to it.
        let mut map = WorldMap::default();
        let forge = map.add("The Forge", None);

        assert!(!map.resize(&forge, 200.0));
        assert!(map.places[&forge].spot.is_none());
    }

    #[test]
    fn a_new_building_stands_where_it_was_put_at_the_size_of_the_others() {
        let mut map = WorldMap::default();
        let forge = map.add(
            "The Forge",
            Some(Spot {
                x: 300.0,
                y: 200.0,
                footprint: None,
            }),
        );

        let spot = map.places[&forge].spot.unwrap();
        assert_eq!((spot.x, spot.y), (300.0, 200.0));
        assert_eq!(spot.footprint, Some(DEFAULT_FOOTPRINT));
    }

    #[test]
    fn overriding_one_thing_leaves_the_others_to_the_pack() {
        // A Place renamed but not re-drawn keeps the pack's artwork. `None` means "the pack
        // decides", and it has to stay distinguishable from "the user chose nothing".
        let mut map = WorldMap::default();
        let id = PlaceId::new("research_lab");
        map.rename(&id, "El Taller");

        let entry = &map.places[&id];
        assert_eq!(entry.name.as_deref(), Some("El Taller"));
        assert!(entry.mark.is_none(), "artwork is still the pack's");
        assert!(
            entry.concept.is_none(),
            "and so is what kind of place it is"
        );
    }
}
