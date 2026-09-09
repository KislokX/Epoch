//! Removing a World must not touch the user's own folders.
//!
//! ## Why this is a test and not a checklist item
//!
//! It was a checklist item: *remove a World, then confirm your Obsidian vault is still there,
//! `Epoch/` folder and all.* That is the one step in the whole removal that can destroy
//! something a person cannot get back, and the only thing standing between it and a mistake was
//! somebody remembering to look afterwards.
//!
//! A World's id appears in eight places, and two of them point **into the user's own documents**
//! — the library folder they chose, and the project root they chose. Removing a World forgets
//! those two *entries*. It must never follow them.
//!
//! ## What it builds
//!
//! A real vault on disk, in a temporary directory shaped exactly like the situation that worries
//! us: an Obsidian vault the user owns, with their own notes beside the folder Epoch writes
//! into, and a World pointing at it.
//!
//! ## What it asserts
//!
//! That every file Epoch made for that World is gone, that every file the *user* made is still
//! there byte for byte, and that the removal plan never so much as names one of them. The last
//! one matters most: a plan that lists a file has already decided to delete it, and asserting on
//! the aftermath alone would pass for a plan that failed to delete something it meant to.

use std::fs;
use std::path::{Path, PathBuf};

use epoch_engine::erase::{world_files, Removal};
use epoch_engine::library::Libraries;

/// A disposable directory that cleans up after itself even when an assertion fails.
struct Sandbox(PathBuf);

impl Sandbox {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("epoch-removal-{name}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a temporary directory");
        Self(path)
    }
    fn at(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("a parent directory");
    }
    fs::write(path, body).expect("a file");
}

#[test]
fn a_world_takes_its_own_files_and_nothing_of_yours() {
    let sand = Sandbox::new("documents");

    // ---- what the user owns. An Obsidian vault, with the folder Epoch writes into inside it.
    let library = sand.at("Documents/My Vault");
    write(&library.join(".obsidian/app.json"), "{}");
    write(&library.join("pepe.txt"), "mine");
    write(&library.join("Notes/reading list.md"), "- something");
    write(
        &library.join("Epoch/A quest that happened.md"),
        "# it happened",
    );

    // ---- what the user's code is. A project root, which is the other pointer that leads out.
    let project = sand.at("Code/my-project");
    write(&project.join("src/main.rs"), "fn main() {}");

    // ---- what Epoch owns: the vault, and this World's own folder and pack.
    let vault = sand.at("vault");
    let packs = sand.at("packs");
    write(&vault.join("worlds/test/map.toml"), "places = []");
    write(&vault.join("worlds/test/chronicles/one.json"), "{}");
    write(&packs.join("test/manifest.toml"), "name = 'Test'");
    // Another World, to prove the removal is about *one* of them.
    write(&vault.join("worlds/keeper/map.toml"), "places = []");

    let mut libraries = Libraries::load(&vault);
    libraries
        .set("test", Some(library.to_str().expect("a utf-8 path")))
        .expect("the folder exists");
    libraries.save(&vault).expect("saves");

    // ---- the plan, exactly as the shell builds it.
    let plan = Removal {
        what: "test".to_owned(),
        files: world_files(&packs, &vault, "test"),
        ..Default::default()
    };

    // **The plan itself must not name anything of the user's.** Asserting only on the aftermath
    // would also pass for a plan that intended to delete a note and merely failed to.
    for named in &plan.files {
        assert!(
            !named.starts_with(&library),
            "the plan names {named:?}, which is inside the user's own vault"
        );
        assert!(
            !named.starts_with(&project),
            "the plan names {named:?}, which is inside the user's own project"
        );
    }

    let problems = plan.carry_out_with_dirs();
    assert!(problems.is_empty(), "{problems:?}");

    // ---- Epoch's own files for that World are gone.
    assert!(!vault.join("worlds/test").exists(), "the World's folder");
    assert!(!packs.join("test").exists(), "the World's pack");

    // ---- and everything the user made is untouched, byte for byte.
    assert_eq!(
        fs::read_to_string(library.join("pepe.txt")).unwrap(),
        "mine"
    );
    assert!(library.join(".obsidian/app.json").exists(), "still a vault");
    assert!(library.join("Notes/reading list.md").exists());
    assert!(
        library.join("Epoch/A quest that happened.md").exists(),
        "the notes Epoch wrote are the user's too — they are in the user's vault"
    );
    assert!(project.join("src/main.rs").exists(), "the project root");

    // ---- the other World is still there.
    assert!(vault.join("worlds/keeper/map.toml").exists());
}

#[test]
fn forgetting_where_a_world_looked_does_not_follow_the_pointer() {
    // The library entry is a *pointer into the user's documents*. Clearing it is right; walking
    // it is the mistake this whole test file exists to prevent, and it is worth asserting on its
    // own — the entry and the folder are removed by different code, and only one of them should
    // ever be.
    let sand = Sandbox::new("pointer");
    let vault = sand.at("vault");
    let library = sand.at("Documents/My Vault");
    write(&library.join("pepe.txt"), "mine");

    let mut libraries = Libraries::load(&vault);
    libraries
        .set("test", Some(library.to_str().expect("a utf-8 path")))
        .expect("the folder exists");
    libraries.save(&vault).expect("saves");

    let mut libraries = Libraries::load(&vault);
    libraries.set("test", None).expect("forgets");
    libraries.save(&vault).expect("saves");

    assert!(
        library.join("pepe.txt").exists(),
        "the folder is still there"
    );
    assert!(library.exists());
}

#[test]
fn the_shipped_world_keeps_its_pack_because_everything_is_built_from_it() {
    // Removing the default World removes its conversations and its map; the pack stays, because
    // every other World is built from it. A plan that named it would delete the thing new Worlds
    // are made of.
    let sand = Sandbox::new("shipped");
    let vault = sand.at("vault");
    let packs = sand.at("packs");
    write(&packs.join("default/manifest.toml"), "name = 'Default'");
    write(&vault.join("worlds/default/map.toml"), "places = []");

    let files = world_files(&packs, &vault, "default");
    for named in &files {
        assert!(
            !named.starts_with(packs.join("default")),
            "the shipped pack must survive: {named:?}"
        );
    }
    assert!(
        files
            .iter()
            .any(|f| f.starts_with(vault.join("worlds/default"))),
        "its own conversations and map still go"
    );
}
