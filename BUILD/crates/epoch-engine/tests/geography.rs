//! The geography the default pack ships.
//!
//! Geography is pack data the engine carries but never interprets. These tests assert the
//! authored world is coherent, and — importantly — that the **engine does no geometry**:
//! everything checked here is data the pack declared, projected unchanged.

use std::path::PathBuf;

use epoch_engine::{WorldPack, WorldPackChain, WorldView};
use epoch_kernel::PlaceConcept;

fn chain() -> WorldPackChain {
    let pack = WorldPack::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packs/default/pack.toml"),
    )
    .expect("the shipped default pack must load");
    WorldPackChain::new(vec![pack])
}

fn view() -> WorldView {
    WorldView::project(&chain(), &[])
}

#[test]
fn the_default_pack_declares_a_world_larger_than_any_viewport() {
    let view = view();
    let map = view.map.expect("the default pack must declare geography");

    // "Think world, not screen": far bigger than a window, in world units.
    assert!(map.width >= 2560.0, "world is only {} wide", map.width);
    assert!(map.height >= 1440.0, "world is only {} tall", map.height);
}

#[test]
fn every_place_is_somewhere() {
    let view = view();
    for place in &view.places {
        assert!(
            place.placement.is_some(),
            "'{}' is named but not placed anywhere in the world",
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
    let ys: Vec<f32> = view()
        .places
        .iter()
        .filter_map(|p| p.placement)
        .map(|p| p.y)
        .collect();
    assert!(
        ys.windows(2).all(|w| w[0] <= w[1]),
        "places are not ordered back to front: {ys:?}"
    );
}

#[test]
fn places_are_not_uniform_and_not_evenly_spaced() {
    // A row of identical, evenly spaced boxes is a widget strip, not a geography.
    // Distance communicates importance; scale communicates it too.
    let view = view();
    let sites: Vec<_> = view.places.iter().filter_map(|p| p.placement).collect();

    let footprints: std::collections::BTreeSet<_> =
        sites.iter().map(|s| s.footprint.to_bits()).collect();
    assert!(
        footprints.len() >= 3,
        "places should differ in size; found {} distinct footprints",
        footprints.len()
    );

    // Not collinear: places must not sit on one horizontal band.
    let ys: std::collections::BTreeSet<_> = sites.iter().map(|s| s.y.to_bits()).collect();
    assert!(ys.len() >= 4, "places share too many identical y positions");

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
        max / min > 2.5,
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

#[test]
fn the_shipped_world_arrives_with_no_roads_at_all() {
    // This used to assert the opposite — four routes, two prominences, one of them bent — and
    // that was a real statement about the World until ADR-0028 moved positions to the user.
    // A road's shape is only meaningful between two ends somebody placed, so a shipped road
    // was a decision taken on the user's behalf about a map that had become theirs. The land,
    // the terrain and the buildings still arrive; where the roads run is the first thing their
    // owner decides, in World Editor.
    let view = view();
    let map = view.map.unwrap();
    assert!(
        map.routes.is_empty(),
        "the default pack ships roads again; a new World is meant to arrive unshaped"
    );
}

#[test]
fn the_library_is_isolated_and_the_world_has_room_to_grow() {
    // Two authored statements from CONTENT: isolation communicates focus, and empty land
    // is where cities go. Both are pack data, asserted as authored intent.
    let view = view();
    let map = view.map.as_ref().unwrap();

    let library = view
        .places
        .iter()
        .find(|p| p.concept == Some(PlaceConcept::KnowledgeCenter.id()))
        .and_then(|p| p.placement)
        .unwrap();
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
        "the Library should be further from everything than the others are from each other"
    );

    // Room to grow: no place sits in the eastern or southern third.
    let occupied_max_x = view
        .places
        .iter()
        .filter_map(|p| p.placement)
        .map(|s| s.x)
        .fold(0.0, f32::max);
    assert!(
        occupied_max_x < map.width * 0.75,
        "the world has no empty land left for future cities"
    );
}
