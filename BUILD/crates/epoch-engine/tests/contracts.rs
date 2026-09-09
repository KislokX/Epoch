//! The wire, written down.
//!
//! ## What this file is for
//!
//! `ui/src/ipc/contracts.ts` is 679 lines of hand-maintained TypeScript mirroring types that live
//! in Rust, and **nothing has ever asserted that the two still agree**. A renamed field is
//! discovered today by a user seeing `undefined` in a panel — the audit's highest-value missing
//! test, and the cheapest to add.
//!
//! Each test here serialises a view exactly as Tauri would and writes it to
//! `ui/src/ipc/__fixtures__/`. A companion Vitest test reads each fixture and asserts the
//! TypeScript type accepts it. Rename a field on either side and one of the two fails in CI.
//!
//! ## Why fixtures on disk rather than a schema
//!
//! A generated schema would be a third description of the same thing, and the failure mode of
//! three descriptions is that two agree and the build believes them. A fixture is not a
//! description: it is *the actual bytes*, produced by the actual `Serialize` impl, and a
//! TypeScript compiler either accepts it or does not.
//!
//! ## What a failure here means
//!
//! Not "the test is stale". It means the wire changed, and the other side has not been told.
//! Regenerate deliberately — `cargo test -p epoch-engine --test contracts` writes the files —
//! and then make the TypeScript side true.

use std::path::PathBuf;

use epoch_engine::world::{
    CharacterView, FramesView, JourneyView, MapView, MarkView, PlaceView, PlacementView,
    RoutineStepView, WorldView,
};

/// Where the frontend's tests look.
fn fixtures() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../ui/src/ipc/__fixtures__")
}

/// Write one, formatted, so a diff is readable when it changes.
fn record(name: &str, value: &impl serde::Serialize) {
    let dir = fixtures();
    std::fs::create_dir_all(&dir).expect("the fixtures directory must be writable");
    let body = serde_json::to_string_pretty(value).expect("a view must serialise");
    std::fs::write(dir.join(format!("{name}.json")), format!("{body}\n"))
        .expect("a fixture must be writable");
}

/// A mark, as the asset pipeline delivers one (ADR-0020): a `data:` URI and nothing fetched.
fn mark() -> MarkView {
    MarkView {
        role: "body",
        renderer: "sprite",
        supported: true,
        asset: Some("data:image/png;base64,iVBORw0KGgo=".into()),
        scale: 1.0,
        anchor: [0.5, 1.0],
        shape: Vec::new(),
        frames: None,
    }
}

/// A mark whose asset is a sheet rather than a picture.
///
/// Beside the still one rather than instead of it, because both shapes are real and a fixture
/// that only carried one would let the other drift. This is the four-row walk every sprite
/// sheet in the world is drawn as.
fn animated() -> MarkView {
    MarkView {
        renderer: "animated_sprite",
        frames: Some(FramesView {
            columns: 4,
            rows: 4,
            count: 16,
            milliseconds: 120,
            directions: vec!["south", "west", "east", "north"],
        }),
        ..mark()
    }
}

#[test]
fn a_world_view_is_what_the_frontend_expects() {
    // Everything populated: an empty view would let an optional field disappear from the fixture
    // and take the assertion with it.
    let view = WorldView {
        pack_name: Some("Default World".into()),
        map: Some(MapView {
            width: 1920.0,
            height: 1080.0,
            terrain: Vec::new(),
            routes: Vec::new(),
        }),
        places: vec![PlaceView {
            id: "tower".into(),
            concept: Some("research_lab"),
            title: "Tower".into(),
            subtitle: None,
            is_placeholder: false,
            placement: Some(PlacementView {
                x: 320.0,
                y: 240.0,
                footprint: 90.0,
                z_order: 0,
            }),
            marks: vec![mark(), animated()],
            anchors: Vec::new(),
        }],
        characters: vec![CharacterView {
            id: "mage".into(),
            name: "Mage".into(),
            archetype: "researcher",
            home: "tower".into(),
            place: "tower".into(),
            activity: "reading".into(),
            action: "idle",
            class: "idle",
            mark: Some(mark()),
            icon: Some(mark()),
            // Populated for the same reason the journey below is: an empty map would let the
            // shape of what it holds vanish from the fixture and take the assertion with it.
            actions: [("walk", animated())].into_iter().collect(),
            // Populated, because an optional field that is `None` in the fixture is a field the
            // TypeScript side is never checked against.
            journey: Some(JourneyView {
                from: "tower".into(),
                to: "library".into(),
                progress: 0.4,
                speed: 60.0,
                eta_seconds: 6.0,
                facing: "east",
            }),
            // Populated rather than `None`: an optional field the fixture never carries is a
            // field the TypeScript side is never checked against, which is this file's whole
            // point -- and this one is a nested object, where a rename would go unnoticed
            // longest.
            sounds_like: Some(epoch_kernel::Timbre {
                voice: "pato".into(),
                semitones: 12.0,
                speaker: 0,
            }),
            speaks_with: Some("es_ES-davefx-medium".into()),
            routine: vec![RoutineStepView {
                activity: "reading".into(),
                seconds: 30,
                place: Some("library".into()),
            }],
        }],
    };

    record("world_view", &view);

    // The camelCase the frontend reads. `pack_name` in Rust, `packName` on the wire — the one
    // rename that happens silently on every field added without `rename_all` in mind.
    let json = serde_json::to_value(&view).expect("serialises");
    assert!(json.get("packName").is_some(), "the wire is camelCase");
    assert!(json.get("pack_name").is_none(), "and only camelCase");
    assert_eq!(json["characters"][0]["archetype"], "researcher");
    assert_eq!(json["places"][0]["placement"]["footprint"], 90.0);
    // `zOrder`, not `z_order`. The exact rename this test exists to catch.
    assert!(json["places"][0]["placement"]["zOrder"].is_i64());
    // The same rename, on the field travel added. `etaSeconds` is what the World reads.
    assert!(json["characters"][0]["journey"]["etaSeconds"].is_number());
    assert!(json["characters"][0]["journey"]["eta_seconds"].is_null());
}

#[test]
fn a_world_with_nothing_in_it_still_serialises() {
    // A World with no map and nobody home is **valid**, not a failure state (Build From Life,
    // rule 1). If this ever needs a special case, the projection has stopped being total.
    let empty = WorldView {
        pack_name: None,
        map: None,
        places: Vec::new(),
        characters: Vec::new(),
    };

    record("world_view_empty", &empty);

    let json = serde_json::to_value(&empty).expect("serialises");
    assert!(json["packName"].is_null());
    assert!(json["map"].is_null());
    assert_eq!(json["places"].as_array().map(Vec::len), Some(0));
}
