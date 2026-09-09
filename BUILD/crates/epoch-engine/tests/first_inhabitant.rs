//! The first inhabitant, end to end against the real files that ship.
//!
//! Loads the actual vault definitions and the actual default World Pack, spawns whoever lives
//! there, and asserts what the World would show. No mocks: if this passes, someone genuinely
//! lives in Epoch — and presence is answerable without any UI at all (ADR-0018).
//!
//! Since ADR-0023 this also asserts the boundary that moved: her name and face come from the
//! vault, and the World she is standing in supplies neither.

use std::path::PathBuf;

use epoch_engine::{DefinitionRegistry, Simulation, WorldPack, WorldPackChain, WorldView};
use epoch_kernel::{ActivityClass, CharacterId};

fn build_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The crew this test reads.
///
/// ## It used to be the developer's own, and on a clean clone that is nobody
///
/// This read `BUILD/vault/definitions` — the vault of whichever machine ran it. That vault is
/// the **user's** data and left the tree when it should have; these four tests did not follow,
/// so `cargo test` on a fresh checkout failed with *cannot read …/vault/definitions*. Every one
/// of them was green here and red for everybody else, including CI.
///
/// `tests/fixtures/vault` exists for exactly this and says so in its own README: the Launcher's
/// tests were moved off the real vault for the same reason, and this file was missed. What the
/// fixture had to gain is what these assertions are about — a face and a second idle behaviour
/// — which is the right direction: the claim is about *authored content*, so the content the
/// claim is made against should be authored here rather than found on somebody's disk.
fn registry() -> DefinitionRegistry {
    DefinitionRegistry::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault/definitions"),
    )
}

fn chain() -> WorldPackChain {
    let pack = WorldPack::load(&build_root().join("packs/default/pack.toml"))
        .expect("the shipped default pack must load");
    WorldPackChain::new(vec![pack])
}

/// Whoever the fixture vault says lives in the default World.
///
/// Named by archetype rather than by a person, which is what the fixture is for: Epoch's real
/// cast is a deferred milestone, and a test that hardcoded a name would be the easiest place
/// for a placeholder to become one.
fn who() -> CharacterId {
    CharacterId::new("researcher").unwrap()
}

#[test]
fn the_shipped_definitions_load_cleanly() {
    let registry = registry();
    assert!(
        registry.problems().is_empty(),
        "shipped definitions must load without problems: {:?}",
        registry.problems()
    );

    let her = registry
        .character(&who())
        .expect("the fixture crew must exist");

    assert_eq!(her.name, "The Researcher");
    assert!(!her.role.trim().is_empty());
    assert!(
        !her.prompt.trim().is_empty(),
        "her authored identity should be present even before a Provider exists"
    );
    assert!(
        her.presence.idle.len() >= 2,
        "one idle behaviour is a pose, not a life"
    );
    assert!(
        registry.appearance(&who()).is_some(),
        "her face lives with her now, not with a World"
    );
}

#[test]
fn someone_is_already_here() {
    // The emotional goal of this step, asserted: a named inhabitant, in a named place,
    // doing something, without the user having done anything.
    //
    // Whoever the user has put in the default World — not a specific person. The claim is
    // that somebody is *there and legible*, which stays true however the roster is edited.
    let registry = registry();
    let view = WorldView::project(&chain(), &Simulation::populate(&registry, "default").cast());

    assert!(
        !view.characters.is_empty(),
        "the default World ships with somebody living in it"
    );

    for person in &view.characters {
        assert!(!person.id.trim().is_empty());
        assert!(
            !person.name.trim().is_empty(),
            "named by themselves, not by the pack"
        );
        assert!(
            !person.activity.trim().is_empty(),
            "'{}' is frozen",
            person.id
        );
        assert_eq!(
            person.class, "idle",
            "nothing is running, so nothing may look like work"
        );

        // They are somewhere the World actually shows, and that place is named — the default
        // World covers every concept, so nobody can be standing in a placeholder.
        let place = view
            .places
            .iter()
            .find(|p| p.concept.map(str::to_string) == Some(person.place.clone()))
            .unwrap_or_else(|| panic!("'{}' is in a place the World does not render", person.id));
        assert!(
            !place.is_placeholder,
            "'{}' stands in an unnamed place",
            person.id
        );
    }
}

#[test]
fn a_world_is_populated_only_by_the_people_who_live_in_it() {
    // The claim ADR-0023 exists to make true: the crew is shared, the rosters are not.
    //
    // Deliberately derived from whatever the vault currently says rather than naming who
    // lives where. The vault is the USER's data now — they can move anybody into any World
    // from the Launcher, and they did. A test that hardcodes the shipped roster is a test
    // that fails the first time the feature it covers is actually used.
    let registry = registry();

    for world in ["default", "archipelago"] {
        let present: Vec<String> = Simulation::populate(&registry, world)
            .cast()
            .iter()
            .map(|m| m.presence.character.to_string())
            .collect();

        for character in registry.characters() {
            let listed = present.iter().any(|id| id == character.id.as_str());
            assert_eq!(
                listed,
                character.lives_in(world),
                "'{}' is {} in '{world}' but their roster says otherwise",
                character.id,
                if listed { "present" } else { "absent" },
            );
        }
    }

    // And a World nobody has been assigned to still opens, with nobody in it.
    assert!(Simulation::populate(&registry, "a-world-that-does-not-exist").is_empty());
}

#[test]
fn a_face_is_projected_when_authored_and_absent_when_not() {
    // Honest rather than unfinished. Somebody with no artwork reaches the World with no mark,
    // and the renderer answers that with a visible stand-in instead of inventing a face
    // (ADR-0016). Somebody with artwork carries it themselves (ADR-0023).
    //
    // What is asserted here is the *correspondence* — authored artwork appears, absent
    // artwork stays absent — because that holds for any vault. It once also demanded the vault
    // contain somebody bare, so that the stand-in path was covered; that broke the day the
    // second shipped character was given a sprite through the UI, which is the user doing the
    // ordinary thing. A test may not require the user's own World to stay unfinished — and the
    // loop below already proves both directions for whoever is actually there.
    let registry = registry();

    let mut faces = 0;
    let mut bare = 0;
    for world in ["default", "archipelago"] {
        for person in
            WorldView::project(&chain(), &Simulation::populate(&registry, world).cast()).characters
        {
            let id = CharacterId::new(&person.id).unwrap();
            let authored = registry.character(&id).unwrap().appearance.is_some();
            assert_eq!(
                person.mark.is_some(),
                authored,
                "'{}' is drawn differently from what they authored",
                person.id
            );
            if authored {
                faces += 1;
            } else {
                bare += 1;
            }
        }
    }

    assert!(
        faces + bare > 0,
        "the vault should ship somebody to check this against"
    );
}

#[test]
fn she_is_never_frozen() {
    // Across a full idle cycle she must be seen doing more than one thing, and never
    // anything work-class — nothing is running.
    let registry = registry();
    let her = registry.character(&who()).unwrap().clone();
    let cycle = u64::from(her.presence.cycle_seconds());
    let instance =
        epoch_engine::CharacterInstance::spawn("archipelago", her, None, None, Default::default());

    let mut seen = std::collections::BTreeSet::new();
    for second in 0..cycle {
        let presence = instance.presence_at(second);
        assert_eq!(presence.class, ActivityClass::Idle);
        seen.insert(presence.activity);
    }

    assert!(
        seen.len() >= 2,
        "she should visibly do several things while idle, saw: {seen:?}"
    );
}
