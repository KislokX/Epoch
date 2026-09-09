//! The default World Pack that actually ships must load, carry a license, and resolve
//! every concept the engine knows.
//!
//! This is the executable form of the `CONTENT_PHILOSOPHY.md` hard rule: shipped content
//! is checked, not trusted.

use std::path::PathBuf;

use epoch_engine::{WorldPackChain, WorldView};
use epoch_kernel::PlaceConcept;

/// `BUILD/packs/default/pack.toml`, reached from this crate's manifest directory.
fn default_pack_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("packs/default/pack.toml")
}

fn load_default_chain() -> WorldPackChain {
    let pack = epoch_engine::WorldPack::load(&default_pack_manifest())
        .expect("the shipped default pack must load");
    WorldPackChain::new(vec![pack])
}

#[test]
fn the_default_pack_loads_and_declares_a_license() {
    let pack = epoch_engine::WorldPack::load(&default_pack_manifest()).unwrap();
    assert_eq!(pack.id, "default");
    assert!(
        !pack.license.kind.trim().is_empty(),
        "license type must not be empty"
    );
    assert!(
        !pack.license.holder.trim().is_empty(),
        "license holder must not be empty"
    );
}

#[test]
fn the_default_pack_covers_every_concept_the_engine_knows() {
    // The default pack is the last link in the fallback chain, so a gap here would surface
    // as a visible placeholder in the shipped product.
    let view = WorldView::project(&load_default_chain(), &[]);

    for concept in PlaceConcept::ALL {
        let place = view
            .places
            .iter()
            .find(|p| p.concept == Some(concept.id()))
            .unwrap_or_else(|| panic!("no Place projects '{}'", concept.id()));
        assert!(
            !place.is_placeholder,
            "default World does not name the Place for '{}'",
            concept.id()
        );
    }

    // A World is no longer asked about people (ADR-0023): coverage is places alone, and full
    // coverage now genuinely means "this World described everywhere the engine knows".
    let pack = epoch_engine::WorldPack::load(&default_pack_manifest()).unwrap();
    assert_eq!(
        pack.coverage(),
        1.0,
        "default pack coverage must be complete"
    );
}

#[test]
fn the_shipped_world_loads_without_a_single_problem() {
    // Every problem here is something a user would see degraded: a missing asset, an
    // unknown role, a Place with no concept. The World we ship must have none.
    let pack = epoch_engine::WorldPack::load(&default_pack_manifest()).unwrap();
    assert!(
        pack.problems().is_empty(),
        "the default World reports problems: {:?}",
        pack.problems()
    );
}

#[test]
fn every_place_the_default_world_ships_can_actually_be_drawn() {
    // A World may declare more than this build can render, and that degrades honestly. But
    // the World we ship must not rely on it.
    let chain = load_default_chain();
    let view = WorldView::project(&chain, &[]);
    for place in &view.places {
        assert!(
            !place.marks.is_empty(),
            "'{}' has nothing drawn at all",
            place.id
        );
        for mark in &place.marks {
            assert!(
                mark.supported,
                "'{}' declares a '{}' mark with renderer '{}', which this build cannot draw",
                place.id, mark.role, mark.renderer
            );
        }
    }
}

#[test]
fn the_engine_never_leaks_a_concept_id_as_a_title() {
    // Internal First: the engine reasons in concepts, the projection carries names.
    // If a title equals its concept id, something bypassed the World.
    let chain = load_default_chain();
    let view = WorldView::project(&chain, &[]);

    assert_eq!(view.pack_name.as_deref(), Some("Default World"));
    for place in &view.places {
        assert_ne!(
            Some(place.title.as_str()),
            place.concept,
            "title for '{:?}' looks like a raw concept id",
            place.concept
        );
    }
}
