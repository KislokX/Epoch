//! Importing an animation sheet, all the way to the file on disk and back out of the vault.
//!
//! **Written because nothing covered it.** `set_art` had tests for a portrait and none for
//! `ArtSlot::Doing` — the one slot that carries a cut, refuses an image without one, and files
//! under a different stem. The report was "a sheet still cannot be imported", and the first
//! question a report like that deserves is whether the Engine can do it at all.

use std::path::PathBuf;

use epoch_engine::{ArtSlot, DefinitionRegistry};
use epoch_kernel::{Action, CharacterId, Sheet};

/// A real 2x2 PNG, base64, as a `data:` URI — the shape the webview's `FileReader` produces.
///
/// A genuine file rather than a stub: `accept_image` sniffs the bytes and names the file from
/// what it finds, so anything that is not actually a PNG would prove nothing.
fn a_real_png() -> String {
    // 2x2 opaque red, written by hand so this test carries no fixture.
    const PIXELS: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEUlEQVR4nGP8z4AAT\
                          AwMDAwMAB8DAQGZfB1FAAAAAElFTkSuQmCC";
    format!("data:image/png;base64,{PIXELS}")
}

fn a_vault(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("epoch-sheet-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("definitions").join("characters")).expect("a vault");
    dir
}

/// Somebody who exists, written the way the vault writes them.
fn a_character(vault: &std::path::Path, id: &str) {
    std::fs::write(
        vault
            .join("definitions")
            .join("characters")
            .join(format!("{id}.toml")),
        // Shaped after a real one out of the vault. `worlds` is a **table**, not a list — and
        // writing it as a list is how the first draft of this test produced a character the
        // registry had never heard of, which from the outside reads exactly like a refused
        // import.
        format!(
            "id = \"{id}\"\nname = \"Mage\"\narchetype = \"researcher\"\n\
             role = \"Reads things\"\nprompt = \"You are Mage.\"\n\n\
             [worlds.default]\n\n\
             [[presence.idle]]\nactivity = \"reading\"\nseconds = 14\n"
        ),
    )
    .expect("a character on disk");
}

#[test]
fn a_sheet_and_its_cut_survive_the_whole_round_trip() {
    let vault = a_vault("roundtrip");
    a_character(&vault, "mage");

    let mut registry = DefinitionRegistry::load(vault.join("definitions"));
    let id = CharacterId::new("mage").expect("a valid id");
    let walk = Action::from_id("walk").expect("walking is something a character does");

    let cut = Sheet {
        columns: 4,
        rows: 4,
        count: 0,
        milliseconds: 120,
        directions: vec!["south".into(), "west".into(), "east".into(), "north".into()],
    };

    registry
        .set_art(
            &id,
            ArtSlot::Doing(walk),
            Some(&a_real_png()),
            Some(cut.clone()),
        )
        .expect("a sheet with a cut is exactly what this slot takes");

    // The file landed under its own stem, so slots never collide and clearing one cannot
    // delete another's picture.
    let written = vault
        .join("definitions")
        .join("characters")
        .join("mage-walk.png");
    assert!(written.is_file(), "{written:?}");

    // And the cut is in the character's own file, where a person can read it.
    let raw = std::fs::read_to_string(
        vault
            .join("definitions")
            .join("characters")
            .join("mage.toml"),
    )
    .expect("the definition was rewritten");
    assert!(raw.contains("mage-walk.png"), "{raw}");
    assert!(raw.contains("milliseconds = 120"), "{raw}");

    // Read back from disk, not from the copy in memory: what the editor shows next time is
    // whatever the file holds.
    let reloaded = DefinitionRegistry::load(vault.join("definitions"));
    let appearance = reloaded
        .character(&id)
        .expect("still there")
        .appearance
        .as_ref()
        .expect("now has artwork");
    let art = appearance.actions.get("walk").expect("drawn walking");
    assert_eq!(art.cut, cut);

    let _ = std::fs::remove_dir_all(&vault);
}

#[test]
fn an_action_will_not_take_a_picture_with_no_cut() {
    // A sheet nobody can cut draws as a strip. Refusing is the Engine's half of the rule the
    // editor enforces by keeping IMPORT disabled until the grid is stated.
    let vault = a_vault("nocut");
    a_character(&vault, "mage");

    let mut registry = DefinitionRegistry::load(vault.join("definitions"));
    let id = CharacterId::new("mage").expect("a valid id");
    let walk = Action::from_id("walk").expect("a real action");

    let refused = registry
        .set_art(&id, ArtSlot::Doing(walk), Some(&a_real_png()), None)
        .expect_err("a sheet needs a cut");
    assert!(format!("{refused}").contains("cut"), "{refused}");

    let _ = std::fs::remove_dir_all(&vault);
}

#[test]
fn clearing_one_action_leaves_the_others_drawn() {
    // Every slot lives under its own stem for exactly this reason.
    let vault = a_vault("clear");
    a_character(&vault, "mage");

    let mut registry = DefinitionRegistry::load(vault.join("definitions"));
    let id = CharacterId::new("mage").expect("a valid id");
    let cut = Sheet {
        columns: 2,
        rows: 1,
        count: 0,
        milliseconds: 200,
        directions: Vec::new(),
    };

    for action in ["walk", "idle"] {
        registry
            .set_art(
                &id,
                ArtSlot::Doing(Action::from_id(action).expect("a real action")),
                Some(&a_real_png()),
                Some(cut.clone()),
            )
            .expect("drawn");
    }

    registry
        .set_art(
            &id,
            ArtSlot::Doing(Action::from_id("walk").expect("a real action")),
            None,
            None,
        )
        .expect("cleared");

    let reloaded = DefinitionRegistry::load(vault.join("definitions"));
    let appearance = reloaded
        .character(&id)
        .expect("still there")
        .appearance
        .as_ref()
        .expect("still has artwork");
    assert!(!appearance.actions.contains_key("walk"), "cleared");
    assert!(
        appearance.actions.contains_key("idle"),
        "and the other kept"
    );
    assert!(
        vault
            .join("definitions")
            .join("characters")
            .join("mage-idle.png")
            .is_file(),
        "its file too"
    );

    let _ = std::fs::remove_dir_all(&vault);
}
