//! Removing things, and saying exactly what that will remove first.
//!
//! ## One rule, and everything here exists to keep it
//!
//! > **Deleting is an enumerated list of files Epoch created. It is never walking a directory.**
//!
//! That is not caution, it is arithmetic: a World's id appears in eight places and **two of them
//! are paths into the user's own documents** — `libraries.toml` points at an Obsidian vault,
//! `projects.toml` at a source tree. A recursive remove aimed at either would delete somebody's
//! notes or somebody's code, and no confirmation dialogue makes that recoverable.
//!
//! So nothing in this module takes a directory and empties it. Every removal names its files.
//!
//! ## Say it, then do it
//!
//! Each removal is two steps: a **[`Removal`] describing what would happen**, and a call that
//! performs exactly that. The surface shows the first and the user accepts it — *"three
//! characters stop living there"*, *"two conversations stay in History"* — because a warning
//! that only says "are you sure?" is a warning about nothing.
//!
//! The plan is also the test surface: what a deletion *claims* it will touch can be compared
//! against what it touched.
//!
//! ## What is never removed
//!
//! - **The library folder, and Epoch's folder inside it.** Epoch wrote those notes, and they sit
//!   inside a vault the user may have linked them from. Deleting a World removes Epoch's copy of
//!   the work, never the user's notes about it (owner, 2026-08-19).
//! - **The project root.** Same rule: the pointer goes, the folder never does.
//! - **Characters**, when a World goes. They live in the vault and travel into every World
//!   (ADR-0023); a World ending is not a person ending.
//! - **History.** A deleted character stays in the Quests they worked on, because History records
//!   what happened and is not rewritten by somebody leaving (ADR-0025).

use std::path::{Path, PathBuf};

/// What a removal would do, in the words a person needs before accepting it.
///
/// Built before anything is touched. Every file named here is one **Epoch created**; the
/// consequences are things that survive.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Removal {
    /// What is being removed, as a person reads it.
    pub what: String,
    /// Exactly which files go. Enumerated, never a directory to sweep.
    pub files: Vec<PathBuf>,
    /// What this changes without deleting — the sentences a confirmation must show.
    pub consequences: Vec<String>,
    /// What survives, said out loud. A deletion that stays silent about what it kept reads as
    /// having missed something.
    pub survives: Vec<String>,
}

impl Removal {
    /// Whether there is anything to remove at all.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty() && self.consequences.is_empty()
    }

    /// Delete exactly the files this plan named.
    ///
    /// Returns what could not be removed, each with its reason. A file that was already gone is
    /// not a failure — the outcome the caller wanted holds.
    pub fn carry_out(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for path in &self.files {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => problems.push(format!("{}: {err}", path.display())),
            }
        }
        problems
    }
}

/// Every file that could hold one character's artwork.
///
/// Enumerated from the slots rather than by listing the folder: a name that merely *starts with*
/// somebody's id is not necessarily theirs — `mage` and `mage-of-the-north` are two people, and
/// a prefix match would delete the second one's sprite with the first one's.
pub fn artwork_of(characters_dir: &Path, id: &str) -> Vec<PathBuf> {
    let mut stems = vec![id.to_owned(), format!("{id}-icon")];
    stems.extend(
        epoch_kernel::Action::ALL
            .into_iter()
            .map(|action| format!("{id}-{}", action.id())),
    );

    stems
        .into_iter()
        .flat_map(|stem| {
            crate::import::ImageFormat::ALL
                .into_iter()
                .map(move |format| characters_dir.join(format!("{stem}.{}", format.extension())))
        })
        .filter(|path| path.exists())
        .collect()
}

// **`SHIPPED` was here, and it is gone rather than corrected.** It was `"default"` — a typed id
// sparing the World Epoch ships from removal — and for two days after `default` stopped
// shipping it spared a World that did not exist while `archipelago`, the one that did, went
// unguarded. Measured through `world_files`: removing the Archipelago planned to delete its
// `pack.toml` from the program folder.
//
// Changing the string would have been the same bug waiting for the next rename. What replaced it
// is structural: a World somebody made keeps its pack in `vault/worlds/<id>/`, the shipped folder
// is never handed to this module, and so nothing here can plan to delete from it.

/// Every file and folder that belongs to one World, by name.
///
/// Enumerated, never swept — and the enumeration is the interesting part, because a World's id
/// appears in **eight** places and two of them are paths the user chose:
///
/// | where | what happens |
/// |---|---|
/// | `vault/worlds/<id>/` | deleted — its pack if somebody made it, its map, its Quests, its art |
/// | the shipped folder | **never** — it is not an argument, so it cannot be named |
/// | `vault/libraries.toml` | the **entry** goes; the folder it names is never touched |
/// | `vault/projects.toml` | the entry goes; the source tree is never touched |
/// | `vault/trust.toml` | its mode and every policy scoped to it — **security, not tidiness** |
/// | character files | `[worlds.<id>]` leaves each of them; the character stays |
/// | Quest files | inside `vault/worlds/<id>/`, so they go with it |
/// | the active World | closed first, or the app is holding a World that is gone |
///
/// The library is the one to read twice. Epoch wrote notes into `<library>/Epoch/`, and they are
/// **still not removed**: they sit inside a vault the user may have linked them from, so
/// deleting a World removes Epoch's copy of the work and never the user's notes about it.
pub fn world_files(vault: &Path, id: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect(&vault.join("worlds").join(id), &mut files);
    files
}

/// Every file under a directory, deepest first, and the directory itself last.
///
/// **This is the one place a directory is walked, and it walks a directory Epoch made** — a
/// World's own pack folder, or its folder in the vault. It is still enumeration rather than
/// `remove_dir_all`: the plan can be read before it runs, and what it will delete can be
/// counted, shown and tested.
fn collect(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, into);
        } else {
            into.push(path);
        }
    }
    into.push(dir.to_path_buf());
}

impl Removal {
    /// Delete the files this plan named, then the directories they were in.
    ///
    /// Directories come last and are removed only when empty — so a folder holding something
    /// this plan never named survives, with whatever it holds.
    pub fn carry_out_with_dirs(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for path in &self.files {
            let outcome = if path.is_dir() {
                // Never `remove_dir_all`. If something is still in here it was not in the plan,
                // and a plan is exactly what this module promises to carry out.
                std::fs::remove_dir(path)
            } else {
                std::fs::remove_file(path)
            };
            match outcome {
                Ok(()) => {}
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => problems.push(format!("{}: {err}", path.display())),
            }
        }
        problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("epoch-erase-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn artwork_is_found_by_slot_and_never_by_prefix() {
        // The failure this shape exists to prevent: `mage` and `mage-of-the-north` are two
        // people, and anything that matched a name prefix would take the second one's sprite
        // when the first one left.
        let dir = dir();
        for file in [
            "mage.png",
            "mage-icon.webp",
            "mage-walk.png",
            "mage-of-the-north.png",
            "paladin.png",
        ] {
            std::fs::write(dir.join(file), b"x").unwrap();
        }

        let found = artwork_of(&dir, "mage");
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names.len(), 3, "{names:?}");
        assert!(names.contains(&"mage.png".to_string()));
        assert!(names.contains(&"mage-icon.webp".to_string()));
        assert!(names.contains(&"mage-walk.png".to_string()));
        assert!(!names.iter().any(|n| n.starts_with("mage-of-the-north")));
        assert!(!names.iter().any(|n| n.starts_with("paladin")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_plan_removes_what_it_named_and_nothing_else() {
        let dir = dir();
        std::fs::write(dir.join("goes.png"), b"x").unwrap();
        std::fs::write(dir.join("stays.png"), b"x").unwrap();

        let plan = Removal {
            what: "Mage".into(),
            files: vec![dir.join("goes.png")],
            ..Removal::default()
        };
        assert!(plan.carry_out().is_empty());

        assert!(!dir.join("goes.png").exists());
        assert!(
            dir.join("stays.png").exists(),
            "a removal touches exactly what it said it would"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_was_already_gone_is_not_a_failure() {
        // The outcome the caller wanted already holds, and reporting it as an error would make
        // a second confirmation look like a broken one.
        let dir = dir();
        let plan = Removal {
            what: "Mage".into(),
            files: vec![dir.join("never-existed.png")],
            ..Removal::default()
        };
        assert!(plan.carry_out().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_world_you_made_goes_whole_and_the_shipped_folder_is_never_named() {
        let root = dir();
        let packs = root.join("packs");
        let vault = root.join("vault");
        // What shipped, beside the binary. And what somebody made, in the vault, pack and all.
        std::fs::create_dir_all(packs.join("archipelago")).unwrap();
        std::fs::write(packs.join("archipelago").join("pack.toml"), b"x").unwrap();
        let made = vault.join("worlds").join("mine");
        std::fs::create_dir_all(made.join("assets")).unwrap();
        std::fs::create_dir_all(made.join("quests")).unwrap();
        std::fs::write(made.join("pack.toml"), b"x").unwrap();
        std::fs::write(made.join("assets").join("art.png"), b"x").unwrap();
        std::fs::write(made.join("places.toml"), b"x").unwrap();
        let shipped_half = vault.join("worlds").join("archipelago");
        std::fs::create_dir_all(&shipped_half).unwrap();
        std::fs::write(shipped_half.join("places.toml"), b"x").unwrap();

        let mine = world_files(&vault, "mine");
        assert!(mine.iter().any(|p| p.ends_with("pack.toml")));
        assert!(mine.iter().any(|p| p.ends_with("art.png")));
        assert!(mine.iter().any(|p| p.ends_with("places.toml")));

        // **The shipped World's pack is not in the plan, and it cannot be.** Its map is the
        // user's and goes; the pack is the installer's and is put back on every update.
        let shipped = world_files(&vault, "archipelago");
        assert!(
            shipped.iter().all(|p| !p.starts_with(&packs)),
            "nothing under the shipped folder is ever named: {shipped:?}"
        );
        assert!(shipped.iter().any(|p| p.ends_with("places.toml")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_removal_never_reaches_the_folder_the_user_chose() {
        // **The failure this whole module exists to prevent.** `libraries.toml` names an
        // Obsidian vault and `projects.toml` a source tree — and Epoch even wrote notes into
        // the first one. None of it is Epoch's to delete.
        let root = dir();
        let packs = root.join("packs");
        let vault = root.join("vault");
        std::fs::create_dir_all(packs.join("mine")).unwrap();
        std::fs::create_dir_all(vault.join("worlds").join("mine")).unwrap();
        std::fs::write(vault.join("worlds").join("mine").join("places.toml"), b"x").unwrap();

        // Somebody's own notes, with Epoch's folder inside them.
        let notes = root.join("Documents").join("Whallet");
        std::fs::create_dir_all(notes.join("Epoch")).unwrap();
        std::fs::write(notes.join("my-own-note.md"), b"mine").unwrap();
        std::fs::write(notes.join("Epoch").join("a quest.md"), b"written by epoch").unwrap();

        let plan = Removal {
            what: "Mine".into(),
            files: world_files(&vault, "mine"),
            ..Removal::default()
        };
        assert!(plan.carry_out_with_dirs().is_empty());

        assert!(
            !vault.join("worlds").join("mine").exists(),
            "the World went"
        );
        assert!(notes.join("my-own-note.md").exists(), "their notes stayed");
        assert!(
            notes.join("Epoch").join("a quest.md").exists(),
            "and so did the notes Epoch wrote into their vault — this removes Epoch's copy of the work, never their notes about it"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directory_holding_something_the_plan_never_named_survives() {
        // Never `remove_dir_all`. Anything still in there was not in the plan, and a plan is
        // what this module promises to carry out — no more.
        let root = dir();
        let world = root.join("vault").join("worlds").join("mine");
        std::fs::create_dir_all(&world).unwrap();
        std::fs::write(world.join("places.toml"), b"x").unwrap();

        let plan = Removal {
            what: "Mine".into(),
            files: world_files(&root.join("vault"), "mine"),
            ..Removal::default()
        };
        // Something arrives after the plan was made — the shape of a race, and of a user with
        // a file manager open.
        std::fs::write(world.join("appeared-after.txt"), b"not in the plan").unwrap();

        let problems = plan.carry_out_with_dirs();
        assert_eq!(problems.len(), 1, "the directory could not be emptied");
        assert!(world.join("appeared-after.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
