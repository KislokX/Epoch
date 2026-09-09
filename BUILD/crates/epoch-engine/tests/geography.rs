//! The geography the shipped pack declares.
//!
//! Geography is pack data the engine carries but never interprets. These tests assert the
//! authored world is coherent, and — importantly — that the **engine does no geometry**:
//! everything checked here is data the pack declared, projected unchanged.
//!
//! ## It was written against `packs/default`, which no longer ships
//!
//! The owner's publication decision on 2026-09-08 was that one World ships, and it is the
//! Archipelago. Repointing the path was not enough: half of what this file asserted was
//! `packs/default`'s **composition** rather than a property of a World, and the two are only
//! the same thing while there is one World.
//!
//! Measured on the file that ships: 2200 × 1500, three placed Places, two routes, four kinds of
//! terrain. `packs/default` was wider than 2560, had five Places and shipped no roads at all.
//!
//! So every number written down here is derived from what the pack declares, and the two
//! assertions that could only ever be about the other World are gone rather than weakened —
//! each says below where its claim went, because a deleted assertion with no forwarding address
//! is how a guarantee quietly stops being one.

use std::path::PathBuf;

use epoch_engine::{PlacementView, WorldPack, WorldPackChain, WorldView};
use epoch_kernel::PlaceConcept;

fn chain() -> WorldPackChain {
    let pack = WorldPack::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packs/archipelago/pack.toml"),
    )
    .expect("the shipped pack must load");
    WorldPackChain::new(vec![pack])
}

fn view() -> WorldView {
    WorldView::project(&chain(), &[])
}

/// Where the World actually put something.
///
/// A placeholder has no placement, by design — that is what an unnamed concept looks like — so
/// every geometric question here is about the Places this World named.
fn sites(view: &WorldView) -> Vec<PlacementView> {
    view.places.iter().filter_map(|p| p.placement).collect()
}

#[test]
fn the_world_is_larger_than_the_ground_its_places_stand_on() {
    // **"Think world, not screen", derived rather than declared.** This asserted 2560 × 1440 —
    // true of `packs/default` and a sentence about one canvas. The Archipelago is 2200 wide,
    // which is narrower than a monitor and still a world: what makes it one is that there is
    // land its buildings are not standing on.
    let view = view();
    let sites = sites(&view);
    let map = view.map.expect("the shipped pack declares geography");
    assert!(
        !sites.is_empty(),
        "a World with no placed Place has no geography to check"
    );

    let widest = sites.iter().map(|s| s.x).fold(0.0, f32::max);
    let deepest = sites.iter().map(|s| s.y).fold(0.0, f32::max);
    assert!(
        map.width > widest && map.height > deepest,
        "the world is {}x{} and something stands at {widest},{deepest}",
        map.width,
        map.height
    );
}

#[test]
fn every_place_the_world_names_is_somewhere() {
    // A placeholder is *not* a Place with a missing position — it is the visible degradation
    // ADR-0016 requires for a concept this World never named. Asserting a placement on one
    // would demand that a 60% World pretend to be a complete one.
    let view = view();
    for place in view.places.iter().filter(|p| !p.is_placeholder) {
        assert!(
            place.placement.is_some(),
            "'{}' is named by this World and not placed anywhere in it",
            place.id
        );
    }
}

#[test]
fn places_are_drawn_in_a_stable_order_that_does_not_depend_on_authoring() {
    // Determinism, end to end: the same World must always project its Places in the same
    // order, or every downstream cache and every screenshot becomes unreliable.
    let order = || -> Vec<String> { view().places.iter().map(|p| p.id.clone()).collect() };
    assert_eq!(order(), order());

    // And nearer Places draw in front, which is what makes an overworld read as depth.
    let ys: Vec<f32> = sites(&view()).iter().map(|p| p.y).collect();
    assert!(
        ys.windows(2).all(|w| w[0] <= w[1]),
        "places are not ordered back to front: {ys:?}"
    );
}

#[test]
fn places_are_not_uniform_and_not_evenly_spaced() {
    // A row of identical, evenly spaced boxes is a widget strip, not a geography. Distance
    // communicates importance; scale communicates it too.
    //
    // **Counted off the World rather than written down.** This wanted four distinct `y` values,
    // which was the shape of a five-Place World and is unreachable in a three-Place one. What is
    // actually being claimed is that *no two* Places share a row or a size — a stronger property
    // than any threshold, and one that stays true whatever the World's size.
    let view = view();
    let sites = sites(&view);
    assert!(
        sites.len() >= 3,
        "fewer than three placed Places is not a geography: {}",
        sites.len()
    );

    let footprints: std::collections::BTreeSet<_> =
        sites.iter().map(|s| s.footprint.to_bits()).collect();
    assert_eq!(
        footprints.len(),
        sites.len(),
        "two Places share a size; scale is supposed to mean something"
    );

    let ys: std::collections::BTreeSet<_> = sites.iter().map(|s| s.y.to_bits()).collect();
    assert_eq!(
        ys.len(),
        sites.len(),
        "two Places share a row; a horizontal band is a widget strip"
    );

    // Distances between neighbours must genuinely vary.
    let mut distances: Vec<f32> = Vec::new();
    for (i, a) in sites.iter().enumerate() {
        for b in sites.iter().skip(i + 1) {
            distances.push(((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt());
        }
    }
    let min = distances.iter().cloned().fold(f32::MAX, f32::min);
    let max = distances.iter().cloned().fold(0.0, f32::max);
    assert!(
        max / min > 1.5,
        "distances are too uniform (min {min}, max {max}) — geography must mean something"
    );
}

#[test]
fn the_world_has_terrain_of_several_kinds() {
    let view = view();
    let map = view.map.unwrap();
    let kinds: std::collections::BTreeSet<_> = map.terrain.iter().map(|t| t.kind).collect();
    assert!(
        kinds.len() >= 4,
        "a believable world needs varied terrain; found {kinds:?}"
    );
    assert!(
        kinds.contains(&"water"),
        "no water: nothing separates anything"
    );
    for area in &map.terrain {
        assert!(
            area.points.len() >= 3,
            "terrain area '{}' is not a polygon",
            area.kind
        );
    }
}

// **`the_shipped_world_arrives_with_no_roads_at_all` was deleted, and this is where its claim
// went.** It asserted that a World arrives unshaped, because ADR-0028 moved positions to the
// user and a shipped road is a decision taken on their behalf. That was a true statement about
// `packs/default`, which was the *starter* World; the Archipelago is an authored World with two
// bridges, and holding it to that would be asking a place with islands to have no way across.
//
// The guarantee is unchanged and is tested where it actually happens: `pack.rs`'s
// `a_new_world_is_empty_and_loads` asserts a World the user creates has no Places and no map at
// all. A World somebody makes still arrives unshaped; a World somebody authored may be a place.

#[test]
fn the_library_is_further_from_everything_than_the_others_are_from_each_other() {
    // Isolation communicates focus — an authored statement from CONTENT, and one the Archipelago
    // makes too: measured, the knowledge centre is 919 units from its nearest neighbour while
    // the other two sit 580 apart.
    //
    // **The other half of this test was `packs/default`'s composition and is gone**: it asserted
    // no Place sits in the eastern or southern quarter, so that there is empty land where future
    // cities go. The Archipelago's knowledge centre is at 1750 of 2200 — deliberately out on its
    // own island — and *where a World leaves room* is the author's decision about their own map,
    // not a rule the engine may hold them to.
    let view = view();
    let library = view
        .places
        .iter()
        .find(|p| p.concept == Some(PlaceConcept::KnowledgeCenter.id()))
        .and_then(|p| p.placement)
        .expect("the shipped World names a knowledge centre");
    let others: Vec<_> = view
        .places
        .iter()
        .filter(|p| p.concept != Some(PlaceConcept::KnowledgeCenter.id()))
        .filter_map(|p| p.placement)
        .collect();

    let nearest = others
        .iter()
        .map(|s| ((s.x - library.x).powi(2) + (s.y - library.y).powi(2)).sqrt())
        .fold(f32::MAX, f32::min);
    let typical = {
        let mut d = Vec::new();
        for (i, a) in others.iter().enumerate() {
            for b in others.iter().skip(i + 1) {
                d.push(((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt());
            }
        }
        d.iter().cloned().fold(f32::MAX, f32::min)
    };
    assert!(
        nearest > typical,
        "the Library should be further from everything ({nearest}) than the others are from \
         each other ({typical})"
    );
}
