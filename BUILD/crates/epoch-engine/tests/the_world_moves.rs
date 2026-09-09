//! The whole of travel, from a Definition to the JSON a window would receive.
//!
//! Every other test of this covers one decision. This one covers the *seam*: composing a real
//! World from the shipped pack, populating it with somebody who lives there, surveying the
//! geography the renderer draws from, sending them somewhere, and reading the projection.
//!
//! It exists because the parts were provably right and the join was not: presence is decided in
//! `simulation.rs`, the distance comes from `geography.rs`, and the shape that reaches a surface
//! is decided in `world.rs` — three files that each pass their own tests while disagreeing about
//! a Place.
//!
//! **Headless, on purpose.** ADR-0018 says presence must be answerable with no window open, and
//! this is what that claim is worth: the entire Living World, asserted with no UI at all.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use epoch_engine::geography::Geography;
use epoch_engine::simulation::{CharacterInstance, Effort, PresenceEvent, Simulation};
use epoch_engine::{WorldPack, WorldPackChain, WorldView};
use epoch_kernel::{
    CharacterArchetype, CharacterDefinition, CharacterId, IdleBehavior, PlaceId, PresenceProfile,
    Residence,
};

/// The World Epoch actually ships, loaded from the file it ships as.
fn shipped() -> WorldPackChain {
    let pack = WorldPack::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packs/archipelago/pack.toml"),
    )
    .expect("the shipped default World must load");
    WorldPackChain::new(vec![pack])
}

/// Somebody who lives in the shipped World and reads at the Library.
fn scholar() -> CharacterDefinition {
    CharacterDefinition {
        id: CharacterId::new("mage").expect("id"),
        name: "Mage".into(),
        archetype: CharacterArchetype::Researcher,
        role: "Turns goals into designs".into(),
        worlds: [(
            "archipelago".to_string(),
            Residence {
                home: Some(PlaceId::new("research_lab")),
                // The per-World half: reading happens at the Library *here* (ADR-0028).
                at: [("reading".to_string(), PlaceId::new("knowledge_center"))].into(),
            },
        )]
        .into(),
        prompt: String::new(),
        skills: Default::default(),
        requested_capabilities: None,
        mind: None,
        appearance: None,
        draws_in: None,
        speaks_with: None,
        sounds_like: None,
        presence: PresenceProfile {
            authored_home: None,
            idle: vec![IdleBehavior {
                activity: "reading".into(),
                // Long enough that the walk across the shipped World fits inside it.
                seconds: 300,
            }],
        },
    }
}

fn world_of(who: CharacterDefinition) -> (WorldPackChain, Simulation) {
    let chain = shipped();
    let mut sim = Simulation::of(vec![CharacterInstance::spawn(
        "archipelago",
        who,
        None,
        None,
        Default::default(),
    )]);
    sim.survey(Geography::of(&epoch_engine::world::resolved(
        &chain,
        &epoch_engine::places::WorldMap::default(),
    )));
    (chain, sim)
}

#[test]
fn a_routine_carries_somebody_across_the_world_epoch_ships() {
    let (chain, mut sim) = world_of(scholar());
    let start = Instant::now();

    // Her routine says reading happens at the Library and she is at the Laboratory, so she sets
    // out. No test fixture geography: these are the buildings in `packs/archipelago/pack.toml`, at
    // the distance somebody actually sees.
    let departure = sim.advance(start);
    assert_eq!(
        departure,
        vec![PresenceEvent::StartedTravelling {
            character: CharacterId::new("mage").expect("id"),
            to: PlaceId::new("knowledge_center"),
        }],
        "the shipped World is drawn, so there is somewhere to walk"
    );

    // What a window receives, mid-walk.
    let view = WorldView::project(&chain, &sim.cast());
    let json = serde_json::to_value(&view).expect("serialises");
    let mage = &json["characters"][0];

    // Where she still is: she has left and not arrived, so the Laboratory is the honest answer
    // and the Library does not hold her yet.
    assert_eq!(mage["place"], "research_lab");
    assert_eq!(mage["journey"]["from"], "research_lab");
    assert_eq!(mage["journey"]["to"], "knowledge_center");
    // camelCase on the wire, like every other field the frontend reads.
    assert!(mage["journey"]["etaSeconds"].is_number());
    assert!(mage["journey"]["eta_seconds"].is_null());
    // Walking on a routine is idle-class. Nothing is running, and the World may not imply that
    // anything is (Build From Life, rule 3).
    assert_eq!(mage["class"], "idle");
    // In the user's own words for their own building, never an id. Read off the World rather
    // than typed: the name belongs to whoever authored the pack, and hard-coding it made this a
    // test about one World's vocabulary instead of about the projection.
    let named = json["places"]
        .as_array()
        .expect("places")
        .iter()
        .find(|place| place["concept"] == "knowledge_center")
        .map(|place| place["title"].as_str().expect("a title").to_owned())
        .expect("the shipped World names its knowledge centre");
    assert_eq!(mage["activity"], format!("walking to {named}"));

    // The ETA is the real distance at walking pace, not a constant. Two buildings in the shipped
    // World are far enough apart to be a walk worth watching and short enough to be a walk.
    let eta = mage["journey"]["etaSeconds"].as_f64().expect("a number");
    assert!(
        (4.0..90.0).contains(&eta),
        "a walk across the shipped World should be seconds, not an errand: {eta}s"
    );

    // And she gets there. Arrival is an event, and only afterwards does the Library hold her.
    let arrival = sim.advance(start + Duration::from_secs(eta as u64 + 1));
    assert_eq!(
        arrival,
        vec![PresenceEvent::Arrived {
            character: CharacterId::new("mage").expect("id"),
            at: PlaceId::new("knowledge_center"),
        }]
    );

    // The Library holds her from the instant she arrives — the beat is about what she is
    // doing, never about where she is (ADR-0018 amendment).
    let landing =
        serde_json::to_value(WorldView::project(&chain, &sim.cast())).expect("serialises");
    assert_eq!(landing["characters"][0]["place"], "knowledge_center");
    assert_eq!(landing["characters"][0]["action"], "settle");

    // And then she gets on with what sent her here.
    sim.advance(start + Duration::from_secs(eta as u64 + 3));
    let there = serde_json::to_value(WorldView::project(&chain, &sim.cast())).expect("serialises");
    assert_eq!(there["characters"][0]["place"], "knowledge_center");
    assert_eq!(there["characters"][0]["activity"], "reading");
    assert_eq!(there["characters"][0]["action"], "idle");
    assert!(
        there["characters"][0]["journey"].is_null(),
        "the walk is over and the field goes away, or a figure keeps walking a road they finished"
    );
}

#[test]
fn work_travels_too_and_says_so_the_whole_way() {
    let (chain, mut sim) = world_of(scholar());
    let start = Instant::now();

    // A Quest handed to somebody in another building: the road carries work (ADR-0028).
    sim.work_at(
        &CharacterId::new("mage").expect("id"),
        &PlaceId::new("knowledge_center"),
        "thinking with qwen3:14b",
        Effort::Thinking,
        None,
        start,
    );

    let json = serde_json::to_value(WorldView::project(&chain, &sim.cast())).expect("serialises");
    let mage = &json["characters"][0];
    // **Work-class from the first step.** The user asked for something and it is under way; a
    // World that only admitted it on arrival would be a World that is behind.
    assert_eq!(mage["class"], "work");
    assert!(!mage["journey"].is_null());
}

#[test]
fn a_world_with_nothing_drawn_in_it_keeps_everybody_exactly_where_they_are() {
    // The state a brand-new World is in, and the one a half-built World stays in. Nobody moves,
    // nothing fails, and nobody ends up standing at the origin.
    let mut sim = Simulation::of(vec![CharacterInstance::spawn(
        "archipelago",
        scholar(),
        None,
        None,
        Default::default(),
    )]);
    // Deliberately no survey: this is a Simulation that has never been told what the World looks
    // like, which is every Simulation until a map is composed.
    assert!(sim.advance(Instant::now()).is_empty());

    let json = serde_json::to_value(WorldView::project(&WorldPackChain::default(), &sim.cast()))
        .expect("serialises");
    assert_eq!(json["characters"][0]["place"], "research_lab");
    assert!(json["characters"][0]["journey"].is_null());
}
