//! Editing a Definition must change the running World without a restart (ADR-0003's
//! data-driven mandate, ADR-0011's hot-reload safety).
//!
//! This covers the engine half of the loop: detect the edit, re-read it, respawn
//! inhabitants, and project the new behaviour. The transport half (the engine event
//! reaching the window) is a shell concern and is granted by
//! `crates/epoch-tauri/capabilities/world.json`.

use std::path::PathBuf;

use epoch_engine::{DefinitionRegistry, Simulation, WorldPackChain, WorldView};
use epoch_kernel::CharacterId;

/// The World this test's character lives in. Any id works — the roster is the character's.
const WORLD: &str = "test-world";

struct Vault(PathBuf);

impl Vault {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("epoch-hotreload-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("characters")).unwrap();
        Self(dir)
    }

    fn write_researcher(&self, first_activity: &str) {
        let body = format!(
            r#"
id = "mage"
name = "Mage"
archetype = "researcher"
role = "Turns goals into designs"
worlds = ["{WORLD}"]

[presence]
home = "research_lab"
idle = [
  {{ activity = "{first_activity}", seconds = 14 }},
  {{ activity = "looking around", seconds = 5 }},
]
"#
        );
        std::fs::write(self.0.join("characters/mage.toml"), body).unwrap();
    }

    fn definitions(&self) -> PathBuf {
        self.0.clone()
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What the World would show her doing, right now.
fn current_activity(registry: &DefinitionRegistry) -> String {
    let simulation = Simulation::populate(registry, WORLD);
    let view = WorldView::project(&WorldPackChain::default(), &simulation.cast());
    view.characters
        .iter()
        .find(|c| c.id == "mage")
        .expect("she must be in the World")
        .activity
        .clone()
}

#[test]
fn editing_a_definition_changes_the_running_world() {
    let vault = Vault::new("apply");
    vault.write_researcher("reading");

    let mut registry = DefinitionRegistry::load(vault.definitions());
    assert!(registry.problems().is_empty(), "{:?}", registry.problems());
    assert_eq!(current_activity(&registry), "reading");
    assert!(!registry.changed_on_disk(), "nothing has been edited yet");

    // The user edits her file, exactly as in the real vault.
    // The sleep is only to guarantee a distinct modification time on coarse filesystems.
    std::thread::sleep(std::time::Duration::from_millis(20));
    vault.write_researcher("rereading an old ADR");

    assert!(registry.changed_on_disk(), "the edit must be detected");

    registry.reload();
    assert!(registry.problems().is_empty(), "{:?}", registry.problems());
    assert_eq!(
        current_activity(&registry),
        "rereading an old ADR",
        "the edit must be applied, not just detected"
    );
    assert!(
        !registry.changed_on_disk(),
        "after reloading, the world is in sync again"
    );
}

#[test]
fn a_definition_that_becomes_invalid_does_not_destroy_the_world() {
    // Someone edits the file mid-thought and saves something broken. The World must report
    // it and keep standing, not crash or silently blank out.
    let vault = Vault::new("broken-edit");
    vault.write_researcher("reading");
    let mut registry = DefinitionRegistry::load(vault.definitions());
    assert_eq!(current_activity(&registry), "reading");

    std::thread::sleep(std::time::Duration::from_millis(20));
    std::fs::write(
        vault.definitions().join("characters/mage.toml"),
        "id = \"mage\"\nname = \"Mage\"\narchetype = \"researcher\"\nrole = \"r\"\n\n[presence]\nhome = \"research_lab\"\nidle = []\n",
    )
    .unwrap();

    assert!(registry.changed_on_disk());
    registry.reload();

    assert!(registry
        .character(&CharacterId::new("mage").unwrap())
        .is_none());
    assert_eq!(registry.problems().len(), 1);
    assert!(registry.problems()[0].contains("never frozen"));

    // And fixing the file brings her back.
    std::thread::sleep(std::time::Duration::from_millis(20));
    vault.write_researcher("reading again");
    registry.reload();
    assert!(registry.problems().is_empty());
    assert_eq!(current_activity(&registry), "reading again");
}
