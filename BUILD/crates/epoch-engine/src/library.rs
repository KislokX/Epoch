//! The Library — what this World's crew knows, as opposed to where it works.
//!
//! ## Why this is not a second Project Root
//!
//! They are the same *mechanism* and different *questions*, in exactly the way ADR-0025
//! separated the Project Root from Trust:
//!
//! > **Project Root** answers *where does this World work.*
//! > **Library** answers *what does this World already know.*
//!
//! A project is written to. A library is read, and nothing here can write to it — not by policy
//! but because no writing capability is ever constructed over it. Somebody's notes are years of
//! their thinking, and the first version of a feature that could rewrite them is not a version
//! anybody should have to trust.
//!
//! Folding the two together would have been one folder setting and one set of file tools. It
//! would also mean pointing a World at your vault to read it, and thereby handing `write_file`
//! and `run_command` the same folder — a permission decision made silently by a convenience.
//!
//! ## It is an Obsidian vault, and specifically so
//!
//! A vault is markdown in folders, which is why nothing here needs Obsidian to be installed or
//! running. What makes it *Obsidian* rather than a directory of files is the one thing
//! [`crate::capabilities::notes`] does specially: a `[[wikilink]]` names a note by its title,
//! from anywhere in the vault, and a reader that could not follow one would be handed a
//! reference on every page it could not use.
//!
//! ## Confinement is [`ProjectRoot`], deliberately
//!
//! The same type, doing the same job: resolve on disk, refuse anything outside, refuse the
//! `NEVER` list. A second implementation of containment is a second place for containment to be
//! wrong, and this one has been thought about hard (symlinks, Windows case, `../`).
//!
//! Its *name* is now narrower than its job. Renaming it across the build to say "a folder
//! something is allowed to reach" would touch a great deal and change nothing, so the honest
//! move is to say so here rather than to spend the churn.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::project::{ProjectError, ProjectRoot};

/// Which library each World reads from.
///
/// In the vault, keyed by World id. Per World for the same reason the Project Root is: one
/// global library would be a lie the moment a second World exists, and "my work notes" and "my
/// novel" are not the same second brain.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Libraries {
    libraries: BTreeMap<String, String>,
}

impl Libraries {
    pub fn load(vault: &Path) -> Self {
        // Missing means no World has a library yet, which is an ordinary state and not a
        // problem: a World with no library is complete, it simply knows nothing yet.
        match std::fs::read_to_string(path(vault)) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, vault: &Path) -> Result<(), ProjectError> {
        let body =
            toml::to_string_pretty(self).map_err(|e| ProjectError::Malformed(e.to_string()))?;
        std::fs::create_dir_all(vault).map_err(|source| ProjectError::Write {
            path: vault.to_path_buf(),
            source,
        })?;
        let path = path(vault);
        std::fs::write(&path, body).map_err(|source| ProjectError::Write { path, source })
    }

    /// What this World says its library is, exactly as authored. `None` means none set.
    pub fn authored(&self, world: &str) -> Option<&str> {
        self.libraries.get(world).map(String::as_str)
    }

    /// This World's library, opened. `None` when unset **or when it has gone missing** — a
    /// vault on a drive that is not plugged in is not a library, and saying so beats a crew
    /// that reports finding nothing in it.
    pub fn open(&self, world: &str) -> Option<ProjectRoot> {
        self.libraries
            .get(world)
            .and_then(|raw| ProjectRoot::open(raw).ok())
    }

    /// Set or clear a World's library. Validated here, so what is stored is known to have
    /// opened at least once.
    pub fn set(&mut self, world: &str, authored: Option<&str>) -> Result<(), ProjectError> {
        match authored.map(str::trim).filter(|s| !s.is_empty()) {
            Some(raw) => {
                let root = ProjectRoot::open(raw)?;
                self.libraries
                    .insert(world.to_owned(), root.as_authored().to_owned());
            }
            None => {
                self.libraries.remove(world);
            }
        }
        Ok(())
    }
}

fn path(vault: &Path) -> PathBuf {
    vault.join("libraries.toml")
}

/// What was found in a folder somebody is about to hand a World as its library.
///
/// Facts rather than a verdict, the same discipline as [`crate::project::ProjectScan`]: Epoch
/// does not decide whether a folder is a good vault. It says what is there, and the person who
/// chose it recognises their own notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScan {
    /// Whether `.obsidian/` is here — the one unambiguous sign of a real vault.
    ///
    /// Its absence is **not** a refusal. A folder of markdown is a perfectly good library, and
    /// a vault that has never been opened on this machine has no `.obsidian/` either.
    pub obsidian: bool,
    /// How many markdown notes are in it, counted to a cap.
    pub notes: usize,
    /// True when counting stopped at the cap, so `notes` reads as "at least".
    pub more: bool,
}

/// A resolved Windows path, written the way every other program expects to read it.
///
/// `std::fs::canonicalize` returns an **extended-length path** on Windows — `\\?\C:\Users\…`,
/// or `\\?\UNC\server\share` for a network location. That prefix is correct, it is what lets a
/// path exceed 260 characters, and Rust is right to produce it. It is also not a path most
/// programs accept.
///
/// Obsidian answered `Vault not found` for a vault that was sitting right there, and the URL in
/// its own error message is what said why. Nothing in Epoch had gone wrong: containment
/// resolves paths on purpose (see [`crate::project`]), and the resolved form is exactly what
/// must never be assumed to be presentable.
///
/// So this is a **presentation** step and lives here rather than in `ProjectRoot`: the resolved
/// path stays resolved everywhere it is compared, and is made plain only at the moment it is
/// handed to somebody else.
pub fn plainly(path: &str) -> String {
    // A network location loses more than the prefix: `\\?\UNC\server\share` is written
    // `\\server\share`, so the leading pair of backslashes has to be put back.
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    path.strip_prefix(r"\\?\").unwrap_or(path).to_owned()
}

/// How Obsidian is asked to open a vault.
///
/// `obsidian://open?path=<absolute path>` — **by path rather than by vault name.** A name would
/// have to match what Obsidian happens to call the vault in *its* configuration, which Epoch
/// cannot see and the user may have changed; a path is what the user actually chose here.
///
/// Pure, so the escaping is testable without a desktop. Percent-encoding is written out rather
/// than pulled in: it is fifteen lines against a dependency in a binary that ships to people,
/// and the alphabet of characters that survive a URI unescaped is not going to change.
pub fn obsidian_uri(path: &str) -> String {
    let path = plainly(path);
    let mut encoded = String::with_capacity(path.len() + 16);
    for byte in path.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(*byte as char)
            }
            // Everything else, including the separators and every non-ASCII byte of a name
            // written in somebody's own language.
            other => encoded.push_str(&format!("%{other:02X}")),
        }
    }
    format!("obsidian://open?path={encoded}")
}

/// The most notes a scan counts before answering "at least this many".
///
/// A scan runs while somebody is looking at a folder picker. Walking a 40,000-note vault to
/// produce a number nobody reads precisely is a frozen window for an exact answer that was
/// never the question.
const SCAN_CAP: usize = 2_000;

/// Look at a folder and say what is in it, without deciding anything.
pub fn scan(root: &ProjectRoot) -> LibraryScan {
    let mut notes = 0usize;
    let mut more = false;
    let mut pending = vec![root.path().to_path_buf()];

    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if path.is_dir() {
                // `.obsidian` holds settings and plugin code, never notes.
                if !name.starts_with('.') {
                    pending.push(path);
                }
                continue;
            }
            if name.ends_with(".md") {
                notes += 1;
                if notes >= SCAN_CAP {
                    more = true;
                    pending.clear();
                    break;
                }
            }
        }
    }

    LibraryScan {
        obsidian: root.path().join(".obsidian").is_dir(),
        notes,
        more,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("epoch-lib-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch");
        dir
    }

    #[test]
    fn a_library_that_has_moved_is_not_a_library() {
        // The same rule the Project Root follows. A path that no longer resolves must read as
        // *no library*, or the crew reports an empty vault and the user believes their notes
        // are unreadable rather than unplugged.
        let dir = scratch("gone");
        let mut libraries = Libraries::default();
        libraries
            .set("default", Some(&dir.to_string_lossy()))
            .expect("set");
        assert!(libraries.open("default").is_some());

        std::fs::remove_dir_all(&dir).expect("remove");

        assert!(libraries.open("default").is_none());
        // Still authored, though: what the user chose is not forgotten because a drive is out.
        assert!(libraries.authored("default").is_some());
    }

    #[test]
    fn the_settings_folder_of_a_vault_holds_no_notes() {
        // What actually happened: a folder picker opened inside a vault, `.obsidian` was one
        // double-click away, and it was chosen. The library then looked configured and the crew
        // found nothing in it — which reads as the feature being broken rather than as the
        // wrong folder. The scan is what lets the surface say so at the moment of choosing.
        let dir = scratch("settings");
        std::fs::write(dir.join("Note.md"), "# real").expect("write");
        let settings = dir.join(".obsidian");
        std::fs::create_dir_all(&settings).expect("mkdir");
        std::fs::write(settings.join("app.json"), "{}").expect("write");

        let inside = scan(&ProjectRoot::open(&settings.to_string_lossy()).expect("open"));

        assert_eq!(inside.notes, 0, "settings are not notes");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_vault_is_asked_for_by_path_with_everything_escaped() {
        // A Windows path is mostly characters a URI does not survive, and a real vault lives in
        // a folder with spaces in it more often than not. Asserted here because the failure
        // downstream is silent: Obsidian opens, finds nothing at the mangled path, and shows
        // its own picker — which reads as Epoch having done nothing at all.
        let uri = obsidian_uri(r"C:\Users\someone\Second Brain");

        assert_eq!(
            uri,
            "obsidian://open?path=C%3A%5CUsers%5Csomeone%5CSecond%20Brain"
        );
        // And a name in somebody's own language survives as its own bytes.
        assert!(obsidian_uri("/home/u/Diseño").ends_with("Dise%C3%B1o"));
    }

    #[test]
    fn an_extended_length_prefix_never_reaches_obsidian() {
        // The reported defect, and its own error message was the evidence: Obsidian answered
        // `Vault not found` for `\\?\C:\Users\someone\Documents\Whallet`, a vault that was
        // sitting right there. Nothing in Epoch had gone wrong — containment resolves paths on
        // purpose, and `canonicalize` on Windows resolves them into a form most programs do not
        // read. Resolved is for comparing; plain is for handing to somebody else.
        assert_eq!(
            obsidian_uri(r"\\?\C:\Users\someone\Documents\Whallet"),
            obsidian_uri(r"C:\Users\someone\Documents\Whallet")
        );
        // A network vault loses more than the prefix: the leading pair has to come back.
        assert_eq!(
            plainly(r"\\?\UNC\server\share\Notes"),
            r"\\server\share\Notes"
        );
        // And a path that was already plain is left exactly as it is.
        assert_eq!(plainly(r"C:\Notes"), r"C:\Notes");
    }

    #[test]
    fn a_folder_of_markdown_is_a_library_even_without_obsidian() {
        // Refusing one would turn "we support Obsidian" into "we support Obsidian's dotfolder",
        // and a vault that has never been opened on this machine has no `.obsidian` either.
        let dir = scratch("plain");
        std::fs::write(dir.join("Note.md"), "# hello").expect("write");
        std::fs::create_dir_all(dir.join("Daily")).expect("mkdir");
        std::fs::write(dir.join("Daily/2026-08-15.md"), "today").expect("write");

        let scanned = scan(&ProjectRoot::open(&dir.to_string_lossy()).expect("open"));

        assert_eq!(scanned.notes, 2);
        assert!(!scanned.obsidian);
        assert!(!scanned.more);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
