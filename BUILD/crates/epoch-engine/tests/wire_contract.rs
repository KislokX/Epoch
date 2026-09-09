//! The wire shape the presentation layer depends on.
//!
//! `ui/src/ipc/contracts.ts` declares these fields by hand. Nothing in the compiler links
//! the two, so this test is the seam: if the Rust projection changes shape, this fails and
//! the TypeScript contract must be updated with it.

use std::path::PathBuf;

use epoch_engine::{WorldPack, WorldPackChain, WorldView};
use serde_json::Value;

fn keys(value: &Value, what: &str) -> Vec<String> {
    let mut k: Vec<String> = value
        .as_object()
        .unwrap_or_else(|| panic!("{what} must be an object"))
        .keys()
        .cloned()
        .collect();
    k.sort();
    k
}

fn assert_exactly(value: &Value, what: &str, expected: &[&str]) {
    let mut want: Vec<String> = expected.iter().map(|s| (*s).to_string()).collect();
    want.sort();
    assert_eq!(
        keys(value, what),
        want,
        "{what} no longer matches the contract the UI declares"
    );
}

/// The World these contracts are checked against.
///
/// **A fixture, because one of them is about artwork and the shipped Worlds have none.**
/// `a_composed_place_serializes_its_parts_to_the_shape_the_ui_declares` asserts that an Asset
/// reaches the renderer as a `data:` URI (ADR-0020), and it was asserting that about the
/// building sprite the product used to ship. That artwork was evidence rather than decoration,
/// so the evidence moved to `tests/fixtures/pack` and the shipped Worlds carry nothing.
fn default_chain() -> WorldPackChain {
    let pack = WorldPack::load(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pack/pack.toml"),
    )
    .expect("the fixture World must load");
    WorldPackChain::new(vec![pack])
}

#[test]
fn world_view_serializes_to_the_shape_the_ui_declares() {
    let member = epoch_engine::CastMember {
        presence: epoch_kernel::PresenceState::idling(
            epoch_kernel::CharacterId::new("mage").unwrap(),
            epoch_kernel::CharacterArchetype::Researcher,
            epoch_kernel::PlaceId::new("research_lab"),
            "reading",
        ),
        name: "Mage".into(),
        home: epoch_kernel::PlaceId::new("research_lab"),
        appearance: None,
        icon: None,
        actions: Default::default(),
        speaks_with: None,
        sounds_like: None,
        routine: Vec::new(),
    };
    let view = WorldView::project(&WorldPackChain::default(), &[member]);
    let json = serde_json::to_value(&view).unwrap();

    assert_exactly(
        &json,
        "WorldView",
        &["packName", "map", "places", "characters"],
    );

    // No World in this chain, so there is no geography and no name — and that is a valid
    // world, which is why the UI declares both as nullable.
    assert!(json["map"].is_null());
    assert!(json["packName"].is_null());

    let place = &json["places"][0];
    assert_exactly(
        place,
        "PlaceView",
        &[
            "id",
            "concept",
            "title",
            "subtitle",
            "isPlaceholder",
            "placement",
            "marks",
            "anchors",
        ],
    );
    // Without a World a Place has identity and meaning but no position and nothing drawn.
    assert!(place["placement"].is_null());
    assert!(place["subtitle"].is_null());
    assert_eq!(place["marks"].as_array().unwrap().len(), 0);
    assert_eq!(place["anchors"].as_array().unwrap().len(), 0);
    assert!(place["id"].is_string());
    assert!(place["concept"].is_string());
    assert!(place["title"].is_string());
    assert!(place["isPlaceholder"].is_boolean());

    let character = &json["characters"][0];
    assert_exactly(
        character,
        "CharacterView",
        &[
            // `journey` is deliberately absent: it is skipped when nobody is walking, which is
            // this fixture and nearly every character nearly always. The TypeScript declares it
            // optional for exactly that reason.
            "routine",
            "soundsLike",
            "speaksWith",
            "id",
            "name",
            "archetype",
            "home",
            "place",
            "activity",
            "action",
            "class",
            "actions",
            "mark",
            "icon",
        ],
    );
    assert_eq!(character["class"], "idle");
    // The closed vocabulary a pack resolves — `walk.east`, never "walking to The Library".
    assert_eq!(character["action"], "idle");
    // Her name survived a projection through a World that supplies no cast at all — which is
    // every World, now (ADR-0023).
    assert_eq!(character["name"], "Mage");
}

#[test]
fn a_composed_place_serializes_its_parts_to_the_shape_the_ui_declares() {
    let view = WorldView::project(&default_chain(), &[]);
    let json = serde_json::to_value(&view).unwrap();

    let places = json["places"].as_array().unwrap();
    let place = places
        .iter()
        .find(|p| p["id"] == "research_lab")
        .expect("the default World declares the Laboratory");

    assert_exactly(
        &place["placement"],
        "PlacementView",
        &["x", "y", "footprint", "zOrder"],
    );

    let mark = &place["marks"][0];
    assert_exactly(
        mark,
        "MarkView",
        &[
            // `frames` is deliberately absent: it is skipped for a still picture, which is
            // every mark in the shipped World and nearly every mark anybody authors. The
            // TypeScript declares it optional for exactly that reason.
            "role",
            "renderer",
            "supported",
            "asset",
            "scale",
            "anchor",
            "shape",
        ],
    );

    let anchor = &place["anchors"][0];
    assert_exactly(
        anchor,
        "AnchorView",
        &["role", "supported", "x", "y", "w", "h"],
    );

    // The Laboratory draws from an Asset, delivered as a data URI and never as markup.
    let body = place["marks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["renderer"] == "sprite")
        .expect("the Laboratory draws from a sprite");
    // Deliberately not asserting a format. ADR-0020's claim is that raster and vector travel
    // the *identical* path, so pinning this to one MIME type would contradict the thing the
    // test exists to protect. It already caught that once: the Laboratory moved from an
    // authored SVG to pixel art and nothing but this line had to change.
    let asset = body["asset"].as_str().unwrap();
    assert!(
        asset.starts_with("data:image/") && asset.contains(";base64,"),
        "an Asset must reach the renderer as a data URI, never as markup (ADR-0020): {}",
        &asset[..asset.len().min(40)]
    );

    let shape_layer = &place["marks"][0]["shape"][0];
    assert_exactly(
        shape_layer,
        "ShapeLayerView",
        &["form", "tone", "x", "y", "w", "h", "r", "points"],
    );
}

#[test]
fn the_same_world_projects_to_identical_bytes_every_time() {
    // Place = Compose(Contributions). If this ever fails, something ambient leaked into
    // composition — hash order, a clock, the filesystem — and Places stopped being pure.
    let chain = default_chain();
    let first = serde_json::to_string(&WorldView::project(&chain, &[])).unwrap();
    let second = serde_json::to_string(&WorldView::project(&chain, &[])).unwrap();
    assert_eq!(first, second);
}

#[test]
fn a_cut_sheet_reaches_a_surface_in_the_shape_the_ui_declares() {
    // A third wire shape, pinned like the others. What crosses is **the cut, never the
    // playback**: how many cells, where, and how long each is shown. Which cell is on screen
    // at this millisecond is per-frame and stays in the renderer (ADR-0018's line, applied to
    // artwork instead of to people).
    let sheet = epoch_engine::place::Mark {
        key: None,
        role: epoch_kernel::MarkRole::Visual,
        renderer: epoch_kernel::RendererKind::AnimatedSprite,
        asset: Some("walk.png".into()),
        asset_data: Some("data:image/png;base64,iVBORw0KGgo=".into()),
        scale: 1.0,
        anchor: [0.5, 1.0],
        shape: Vec::new(),
        frames: Some(epoch_engine::place::Frames {
            columns: 4,
            rows: 4,
            count: 13,
            milliseconds: 120,
            directions: vec![
                epoch_kernel::Direction::South,
                epoch_kernel::Direction::West,
            ],
        }),
    };

    let json = serde_json::to_value(epoch_engine::world::project_mark(&sheet)).unwrap();
    assert_exactly(
        &json["frames"],
        "FramesView",
        &["columns", "rows", "count", "milliseconds", "directions"],
    );
    assert_eq!(json["frames"]["directions"][0], "south");
    // Drawable since the day something could play one. It read `false` while nothing could,
    // which was the honest answer for an instrument with nothing behind it — and the wrong one
    // to leave standing once there is.
    assert_eq!(json["supported"], true);
}

#[test]
fn the_movement_channel_matches_the_shape_the_ui_declares() {
    // A second wire shape, and a much smaller one on purpose: `world:moved` exists so a walk
    // does not re-send every Place, mark and face. Anything that creeps into it defeats the
    // reason it exists — so its fields are pinned exactly, like the projection's.
    let walking = epoch_kernel::PresenceState {
        character: epoch_kernel::CharacterId::new("mage").unwrap(),
        archetype: epoch_kernel::CharacterArchetype::Researcher,
        place: epoch_kernel::PlaceId::new("research_lab"),
        activity: "walking to The Library".into(),
        action: epoch_kernel::Action::Walk,
        class: epoch_kernel::ActivityClass::Idle,
        journey: Some(epoch_kernel::Journey {
            from: epoch_kernel::PlaceId::new("research_lab"),
            to: epoch_kernel::PlaceId::new("knowledge_center"),
            progress: 0.4,
            speed: 60.0,
            eta_seconds: 12.5,
            facing: epoch_kernel::Direction::East,
        }),
    };

    let json = serde_json::to_value(epoch_engine::world::PresenceView::of(&walking)).unwrap();
    assert_exactly(
        &json,
        "PresenceView",
        &["id", "place", "activity", "action", "class", "journey"],
    );
    assert_eq!(json["action"], "walk");
    assert_exactly(
        &json["journey"],
        "JourneyView",
        &["from", "to", "progress", "speed", "etaSeconds", "facing"],
    );

    // Standing still is the common case, and it costs nothing: the field is absent rather than
    // null, so a World where nobody is walking sends nothing about walking.
    let still = epoch_kernel::PresenceState::idling(
        epoch_kernel::CharacterId::new("mage").unwrap(),
        epoch_kernel::CharacterArchetype::Researcher,
        epoch_kernel::PlaceId::new("research_lab"),
        "reading",
    );
    let json = serde_json::to_value(epoch_engine::world::PresenceView::of(&still)).unwrap();
    assert_exactly(
        &json,
        "PresenceView",
        &["id", "place", "activity", "action", "class"],
    );
}

#[test]
fn a_configured_server_reaches_a_surface_with_every_field_present() {
    // The crash this exists for: `Configured` skips its empty fields when it serialises, which
    // is right for `mcp.toml` — a file full of empty tables is worse to read — and wrong for a
    // wire, where a missing key and an empty list are different things. A server with no
    // environment arrived with no `env` at all, `contracts.ts` declared the field required, and
    // the panel's first `.map` over it threw before anything rendered.
    //
    // One type cannot answer to a file format and to a surface at once: they disagree about what
    // "nothing" looks like. So the wire has its own projection, and this is what holds it there.
    let mut servers = epoch_engine::mcp::Servers::default();
    servers.remember(epoch_engine::mcp::Configured {
        id: "playwright".into(),
        command: "npx.cmd".into(),
        args: vec!["-y".into(), "@playwright/mcp@latest".into()],
        // Nothing configured, nothing held, and it came from the catalogue.
        source: Some("Playwright".into()),
        ..Default::default()
    });

    let wire = serde_json::to_value(servers.view()).expect("the view serialises");
    assert_exactly(&wire, "McpView", &["servers", "problem"]);

    let server = &wire["servers"][0];
    assert_exactly(
        server,
        "McpServer",
        &["id", "command", "args", "env", "secrets", "enabled"],
    );
    // Present and empty, which is the whole point — not absent.
    assert_eq!(server["env"], serde_json::json!({}));
    assert_eq!(server["secrets"], serde_json::json!([]));
    // And provenance never leaves the Engine: its absence is what lets `amend` read a saved
    // entry's missing `source` as *unchanged* rather than as *erased*.
    assert!(server.get("source").is_none(), "{server}");
}
