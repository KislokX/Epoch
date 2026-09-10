//! Moving a World somebody made out of the program folder, once.
//!
//! ## Why this exists
//!
//! Until 2026-09-10 a new World was written beside the binary — `packs/<id>/`, in the folder the
//! installer owns — because `Paths::packs` was the only place Worlds were looked for. The table at
//! the top of `paths.rs` said Worlds live in the vault. The code did not.
//!
//! Measured on an installed build, not reasoned about. A World made in v0.1.0 survived an update
//! only because the generated uninstaller removes `$INSTDIR\packs` with a plain `RMDir`, which
//! refuses a folder that is not empty — the user's own World was what kept it from being empty. A
//! plain uninstall afterwards left that World behind in a program folder with no program in it,
//! and the uninstaller's *delete application data* box, which removes `%APPDATA%\Epoch`, never
//! reached it.
//!
//! New Worlds are made in `vault/worlds/<id>/` now, beside their map and their Quests. This moves
//! the ones made before that.
//!
//! ## The rules it keeps
//!
//! **Enumerated, never swept** — the rule `erase` follows. Each entry of the old folder is renamed
//! into the new one by name; nothing is copied and nothing is deleted recursively. The old folder
//! goes only once it is empty, with `remove_dir`, so anything this did not move stays.
//!
//! **All or nothing, per World.** A name already taken in the vault moves nothing, and a rename
//! that fails puts back what had already moved. Half a World in each folder is worse than a whole
//! one in the wrong folder, because a World in the wrong folder is still found
//! (`WorldPack::discover_all` reads both).
//!
//! **Only what did not ship.** The installer's own Worlds are listed at build time from the folder
//! it bundles, so this tells `archipelago` from something a person made without reading a
//! sentence inside a file and trusting it.

use std::path::Path;

include!(concat!(env!("OUT_DIR"), "/shipped_worlds.rs"));

/// What moving the user's Worlds did, and what it would not do.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Relocated {
    /// Worlds now in the vault, by id.
    pub moved: Vec<String>,
    /// What stayed where it was, and why, in sentences.
    pub problems: Vec<String>,
}

/// Move every World in `shipped_packs` that did not ship into `my_worlds`.
///
/// `shipped` is passed rather than read from [`SHIPPED`] so a test says what shipped instead of
/// depending on what this repository's `packs/` holds today.
pub fn user_worlds(shipped_packs: &Path, my_worlds: &Path, shipped: &[&str]) -> Relocated {
    let mut done = Relocated::default();
    let Ok(entries) = std::fs::read_dir(shipped_packs) else {
        return done;
    };

    let mut candidates: Vec<std::path::PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.join("pack.toml").is_file())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !shipped.contains(&name))
        })
        .collect();
    candidates.sort();

    for old in candidates {
        let Some(id) = old.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        let new = my_worlds.join(&id);

        let names: Vec<std::ffi::OsString> = match std::fs::read_dir(&old) {
            Ok(entries) => entries.flatten().map(|entry| entry.file_name()).collect(),
            Err(err) => {
                done.problems.push(format!(
                    "The World '{id}' was left at {}: it could not be read ({err}).",
                    old.display()
                ));
                continue;
            }
        };

        // Checked before anything moves, so a collision leaves the World whole where it was.
        let taken: Vec<String> = names
            .iter()
            .filter(|name| new.join(name).exists())
            .map(|name| name.to_string_lossy().into_owned())
            .collect();
        if !taken.is_empty() {
            done.problems.push(format!(
                "The World '{id}' was left at {}: {} already holds {}, and nothing is overwritten.",
                old.display(),
                new.display(),
                taken.join(", ")
            ));
            continue;
        }

        if let Err(err) = std::fs::create_dir_all(&new) {
            done.problems.push(format!(
                "The World '{id}' was left at {}: {} could not be created ({err}).",
                old.display(),
                new.display()
            ));
            continue;
        }

        let mut moved_so_far = Vec::new();
        let mut failed = None;
        for name in &names {
            match std::fs::rename(old.join(name), new.join(name)) {
                Ok(()) => moved_so_far.push(name),
                Err(err) => {
                    failed = Some(format!("{} ({err})", name.to_string_lossy()));
                    break;
                }
            }
        }
        if let Some(why) = failed {
            // Put back what already moved, so the World is whole in one place.
            for name in moved_so_far {
                let _ = std::fs::rename(new.join(name), old.join(name));
            }
            done.problems.push(format!(
                "The World '{id}' was left at {}: {why} could not be moved, so nothing was.",
                old.display()
            ));
            continue;
        }

        // Only an empty folder goes. Anything that arrived in the meantime stays with it.
        let _ = std::fs::remove_dir(&old);
        done.moved.push(id);
    }
    done
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn place(who: &str) -> (PathBuf, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("epoch-relocate-{who}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let packs = root.join("packs");
        let worlds = root.join("vault").join("worlds");
        std::fs::create_dir_all(&packs).unwrap();
        (packs, worlds)
    }

    fn a_pack(at: &Path) {
        std::fs::create_dir_all(at.join("assets")).unwrap();
        std::fs::write(at.join("pack.toml"), b"[pack]").unwrap();
        std::fs::write(at.join("assets").join("preview.png"), b"PNG").unwrap();
    }

    fn cleanup(packs: &Path) {
        let _ = std::fs::remove_dir_all(packs.parent().unwrap());
    }

    #[test]
    fn a_world_somebody_made_moves_into_the_vault_and_the_old_folder_goes() {
        let (packs, worlds) = place("moves");
        a_pack(&packs.join("update-survives"));

        let done = user_worlds(&packs, &worlds, &["archipelago"]);

        assert_eq!(done.moved, ["update-survives"]);
        assert!(done.problems.is_empty(), "{:?}", done.problems);
        assert!(worlds.join("update-survives/pack.toml").is_file());
        assert!(worlds.join("update-survives/assets/preview.png").is_file());
        assert!(
            !packs.join("update-survives").exists(),
            "nothing is left in the program folder"
        );
        cleanup(&packs);
    }

    #[test]
    fn what_shipped_stays_where_the_installer_put_it() {
        let (packs, worlds) = place("shipped");
        a_pack(&packs.join("archipelago"));

        let done = user_worlds(&packs, &worlds, &["archipelago"]);

        assert!(done.moved.is_empty());
        assert!(packs.join("archipelago/pack.toml").is_file());
        assert!(!worlds.join("archipelago").exists());
        cleanup(&packs);
    }

    #[test]
    fn it_joins_the_map_and_quests_already_in_the_vault_without_touching_them() {
        // The common case: somebody who made a World and then built in it already has its vault
        // half. The pack joins it, and nothing already there is rewritten.
        let (packs, worlds) = place("joins");
        a_pack(&packs.join("home"));
        std::fs::create_dir_all(worlds.join("home/quests")).unwrap();
        std::fs::write(worlds.join("home/places.toml"), b"the map").unwrap();

        let done = user_worlds(&packs, &worlds, &[]);

        assert_eq!(done.moved, ["home"]);
        assert!(worlds.join("home/pack.toml").is_file());
        assert_eq!(
            std::fs::read(worlds.join("home/places.toml")).unwrap(),
            b"the map"
        );
        assert!(worlds.join("home/quests").is_dir());
        cleanup(&packs);
    }

    #[test]
    fn a_name_already_taken_moves_nothing_at_all() {
        // All or nothing. Half a World in each folder would be worse than a whole one in the
        // wrong folder, which is still found.
        let (packs, worlds) = place("taken");
        a_pack(&packs.join("home"));
        std::fs::create_dir_all(worlds.join("home/assets")).unwrap();

        let done = user_worlds(&packs, &worlds, &[]);

        assert!(done.moved.is_empty());
        assert_eq!(done.problems.len(), 1, "{:?}", done.problems);
        assert!(
            packs.join("home/pack.toml").is_file(),
            "the World stayed whole where it was"
        );
        assert!(packs.join("home/assets/preview.png").is_file());
        assert!(!worlds.join("home/pack.toml").exists());
        cleanup(&packs);
    }

    #[test]
    fn a_world_already_in_the_vault_is_never_overwritten() {
        let (packs, worlds) = place("twice");
        a_pack(&packs.join("home"));
        std::fs::create_dir_all(worlds.join("home")).unwrap();
        std::fs::write(worlds.join("home/pack.toml"), b"theirs").unwrap();

        let done = user_worlds(&packs, &worlds, &[]);

        assert!(done.moved.is_empty());
        assert_eq!(
            std::fs::read(worlds.join("home/pack.toml")).unwrap(),
            b"theirs"
        );
        assert!(packs.join("home/pack.toml").is_file());
        cleanup(&packs);
    }

    #[test]
    fn no_program_folder_is_nothing_to_do() {
        let root = std::env::temp_dir().join(format!("epoch-relocate-none-{}", std::process::id()));
        let done = user_worlds(&root.join("absent"), &root.join("worlds"), &[]);
        assert_eq!(done, Relocated::default());
    }

    #[test]
    fn the_shipped_list_is_read_off_the_folder_the_installer_bundles() {
        // Derived, never typed: every id here has a pack in the repository's `packs/`, and every
        // pack there is here. `erase::SHIPPED` was typed, and named a World that had stopped
        // shipping.
        let packs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packs");
        let mut on_disk: Vec<String> = std::fs::read_dir(&packs)
            .unwrap()
            .flatten()
            .filter(|entry| entry.path().join("pack.toml").is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        assert!(!SHIPPED.is_empty(), "a build that ships no World");
        assert_eq!(
            SHIPPED,
            on_disk.iter().map(String::as_str).collect::<Vec<_>>()
        );
    }
}
