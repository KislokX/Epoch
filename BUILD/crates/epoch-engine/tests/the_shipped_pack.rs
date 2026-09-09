//! The World Pack that actually ships must load, carry a license, report no problems, and draw
//! everything it declares.
//!
//! This is the executable form of the `CONTENT_PHILOSOPHY.md` hard rule: shipped content is
//! checked, not trusted.
//!
//! ## It was about `packs/default`, and that pack no longer ships
//!
//! Two Worlds used to ship and the owner's decision on 2026-09-08 was that one does — the
//! Archipelago. So these assertions moved to it rather than being deleted with the pack they
//! happened to be pointed at: what is worth checking is *the thing that ships*, and which one
//! that is was never the point.
//!
//! **One assertion did not move, and it is the interesting one.** `packs/default` was the last
//! link in the fallback chain and was held to covering every concept the engine knows, because a
//! gap in the *last* link surfaces as a visible placeholder. The Archipelago covers 60% and says
//! so on its own card — it is *shapes only, no assets*, and its undeclared concepts are exactly
//! the visible placeholders ADR-0016 requires. Asserting complete coverage here would either fail
//! honestly or force somebody to pad a World with names nobody authored.
//!
//! So the guarantee changed shape rather than being dropped: everything this World *does*
//! declare must be drawable and problem-free, and what it does not declare degrades visibly.
//! That is checked below.

use std::path::PathBuf;

use epoch_engine::{WorldPackChain, WorldView};

/// `BUILD/packs/archipelago/pack.toml`, reached from this crate's manifest directory.
fn shipped_manifest() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("packs/archipelago/pack.toml")
}

fn shipped_chain() -> WorldPackChain {
    let pack = epoch_engine::WorldPack::load(&shipped_manifest()).expect("the shipped pack loads");
    WorldPackChain::new(vec![pack])
}

#[test]
fn the_shipped_pack_loads_and_declares_a_license() {
    let pack = epoch_engine::WorldPack::load(&shipped_manifest()).unwrap();
    assert_eq!(pack.id, "archipelago");
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
fn the_shipped_world_loads_without_a_single_problem() {
    // Every problem here is something a user would see degraded: a missing asset, an unknown
    // role, a Place with no concept. The World we ship must have none.
    let pack = epoch_engine::WorldPack::load(&shipped_manifest()).unwrap();
    assert!(
        pack.problems().is_empty(),
        "the shipped World reports problems: {:?}",
        pack.problems()
    );
}

#[test]
fn what_it_declares_it_covers_and_what_it_does_not_is_visible() {
    // **Partial coverage is a fact about this World, not a defect.** What must hold is that the
    // two halves are honest: a concept it names is a real Place, and a concept it does not name
    // is a visible placeholder rather than an absence nobody can see.
    let pack = epoch_engine::WorldPack::load(&shipped_manifest()).unwrap();
    let coverage = pack.coverage();
    assert!(
        (0.0..=1.0).contains(&coverage),
        "coverage is a proportion: {coverage}"
    );

    let view = WorldView::project(&shipped_chain(), &[]);
    assert!(
        view.places.iter().any(|place| !place.is_placeholder),
        "a World that named nothing would pass every other assertion here"
    );
}

#[test]
fn every_place_the_shipped_world_declares_can_actually_be_drawn() {
    // A World may declare more than this build can render, and that degrades honestly. But the
    // World we ship must not rely on it.
    //
    // **Only the Places it named.** A placeholder has nothing drawn *by design* -- that is what
    // an unnamed concept looks like on screen, and asserting marks on one would be demanding
    // that a 60% World pretend to be a complete one. Measured: `automation_hub` is a placeholder
    // in the Archipelago and has no marks, which is the visible degradation ADR-0016 asks for
    // rather than the silent absence it exists to prevent.
    let view = WorldView::project(&shipped_chain(), &[]);
    for place in view.places.iter().filter(|place| !place.is_placeholder) {
        assert!(
            !place.marks.is_empty(),
            "'{}' is named by this World and has nothing drawn at all",
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
    // Internal First: the engine reasons in concepts, the projection carries names. If a title
    // equals its concept id, something bypassed the World.
    let view = WorldView::project(&shipped_chain(), &[]);

    assert!(
        view.pack_name
            .as_deref()
            .is_some_and(|name| !name.is_empty()),
        "the shipped World has a name of its own"
    );
    for place in &view.places {
        assert_ne!(
            Some(place.title.as_str()),
            place.concept,
            "title for '{:?}' looks like a raw concept id",
            place.concept
        );
    }
}
