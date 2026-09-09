//! What was replaced, so it can be put back.
//!
//! ## Why this exists before anything needs it
//!
//! `write_file` shipped declaring [`Reversal::Permanent`], and the code said why:
//!
//! > *a statement about Epoch rather than about filesystems: there is no undo store yet, so
//! > nothing here can put back what it replaced.*
//!
//! That made "Accept edits" an act of faith. A mode that waves file changes through is only
//! reasonable if a file change is recoverable, and it was not.
//!
//! This is the undo store. With it, [`Reversal::Undoable`] on the file capabilities stops being
//! a claim and becomes a fact — which is the only reason it may be declared at all
//! (`Descriptor::problems` refuses a capability whose reversal contradicts its effects).
//!
//! ## Per World, and never inside the World Pack
//!
//! Context does not travel between Worlds: each is a different project with its own
//! conversations and its own history. So this is keyed by World.
//!
//! It lives in the vault rather than in the pack folder, for the reason that moved the crew out
//! of packs (ADR-0023): **a pack is shipped content**, and an update replaces it. User history
//! written there is user history one release away from being deleted.
//!
//! ## What it is not
//!
//! Not version control. It remembers the last few changes so a mistake is recoverable in the
//! moment; it does not branch, merge or survive somebody editing the file by hand in between.
//! That last case is checked rather than assumed — see [`Journal::undo_last`].

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many changes are remembered. Beyond this the oldest is forgotten.
///
/// Small on purpose: this is "I did not mean that", not a history. Keeping hundreds of file
/// versions in a JSON file would make it slow to read and make it look like a backup, which it
/// is not.
const REMEMBERED: usize = 40;

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("nothing to undo")]
    Empty,
    #[error("cannot undo: {0}")]
    Blocked(String),
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Malformed(String),
}

/// One change that can be taken back.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    /// The file, as the project sees it — relative and forward-slashed, so an entry stays
    /// readable and stays valid if the project is moved.
    pub path: String,
    /// What was there before. `None` means the file did not exist, and undoing removes it.
    pub before: Option<String>,
    /// What was written. Checked before undoing, so a file edited by hand in the meantime is
    /// not silently reverted to a state nobody asked for.
    pub after: String,
    /// One line for a surface: `created README.md`, `replaced src/main.rs`.
    pub summary: String,
}

/// The last few changes in one World, oldest first.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Journal {
    changes: Vec<Change>,
}

impl Journal {
    /// Read this World's journal. Absent is the ordinary state and means nothing to undo.
    pub fn load(vault: &Path, world: &str) -> Self {
        match std::fs::read_to_string(path(vault, world)) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, vault: &Path, world: &str) -> Result<(), JournalError> {
        let path = path(vault, world);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| JournalError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let body = serde_json::to_string_pretty(self)
            .map_err(|e| JournalError::Malformed(e.to_string()))?;
        std::fs::write(&path, body).map_err(|source| JournalError::Write { path, source })
    }

    /// Remember a change. The oldest is forgotten once the journal is full.
    pub fn record(&mut self, change: Change) {
        self.changes.push(change);
        if self.changes.len() > REMEMBERED {
            let excess = self.changes.len() - REMEMBERED;
            self.changes.drain(..excess);
        }
    }

    /// The most recent change, if there is one. What a surface offers to undo.
    pub fn last(&self) -> Option<&Change> {
        self.changes.last()
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Put the most recent change back, resolving the path through the project root.
    ///
    /// **Refuses when the file no longer holds what was written.** Somebody may have edited it
    /// by hand, or another tool may have. Reverting anyway would replace their work with a
    /// version they never saw — undo turning into data loss is the one failure this must not
    /// have.
    ///
    /// Undoing an undo is not offered: the entry is removed rather than inverted, because a
    /// journal that grows every time you press undo is a loop, not a history.
    pub fn undo_last(
        &mut self,
        root: &crate::project::ProjectRoot,
    ) -> Result<String, JournalError> {
        let change = self.changes.last().ok_or(JournalError::Empty)?.clone();
        let path = root
            .confine(&change.path)
            .map_err(|why| JournalError::Blocked(format!("'{}' {why}", change.path)))?;

        let now = std::fs::read_to_string(&path).ok();
        match (&now, &change.before) {
            // The file is gone. Undoing a creation has already happened; undoing a replacement
            // cannot, because there is nowhere to put it back that we are sure about.
            (None, _) if change.before.is_none() => {}
            (None, Some(_)) => {
                return Err(JournalError::Blocked(format!(
                    "'{}' is no longer there",
                    change.path
                )));
            }
            (Some(current), _) if current != &change.after => {
                return Err(JournalError::Blocked(format!(
                    "'{}' has changed since then; undoing would discard that",
                    change.path
                )));
            }
            _ => {}
        }

        match &change.before {
            Some(before) => {
                std::fs::write(&path, before).map_err(|source| JournalError::Write {
                    path: path.clone(),
                    source,
                })?
            }
            // It did not exist before, so putting it back means removing it.
            None => {
                if path.exists() {
                    std::fs::remove_file(&path).map_err(|source| JournalError::Write {
                        path: path.clone(),
                        source,
                    })?;
                }
            }
        }

        self.changes.pop();
        Ok(match change.before {
            Some(_) => format!("put back {}", change.path),
            None => format!("removed {}", change.path),
        })
    }
}

fn path(vault: &Path, world: &str) -> PathBuf {
    // Per World, in the vault. Never in the pack folder — see the module docs.
    vault.join("worlds").join(world).join("undo.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::project::ProjectRoot;

    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("epoch-journal-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(d.join("project")).unwrap();
            std::fs::write(d.join("project/main.rs"), "one\ntwo\n").unwrap();
            Self(d)
        }
        fn root(&self) -> ProjectRoot {
            ProjectRoot::open(self.0.join("project").to_str().unwrap()).unwrap()
        }
        fn vault(&self) -> PathBuf {
            self.0.join("vault")
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn replacement(path: &str, before: &str, after: &str) -> Change {
        Change {
            path: path.into(),
            before: Some(before.into()),
            after: after.into(),
            summary: format!("replaced {path}"),
        }
    }

    #[test]
    fn nothing_recorded_means_nothing_to_undo() {
        let t = Tree::new("empty");
        let mut journal = Journal::load(&t.vault(), "archipelago");
        assert!(journal.is_empty());
        assert!(matches!(
            journal.undo_last(&t.root()),
            Err(JournalError::Empty)
        ));
    }

    #[test]
    fn a_replacement_goes_back() {
        let t = Tree::new("replace");
        let root = t.root();
        std::fs::write(root.path().join("main.rs"), "changed\n").unwrap();

        let mut journal = Journal::default();
        journal.record(replacement("main.rs", "one\ntwo\n", "changed\n"));
        assert_eq!(journal.undo_last(&root).unwrap(), "put back main.rs");
        assert_eq!(
            std::fs::read_to_string(root.path().join("main.rs")).unwrap(),
            "one\ntwo\n"
        );
        assert!(journal.is_empty(), "and it is not undoable twice");
    }

    #[test]
    fn undoing_a_creation_removes_the_file() {
        let t = Tree::new("create");
        let root = t.root();
        std::fs::write(root.path().join("new.md"), "hello\n").unwrap();

        let mut journal = Journal::default();
        journal.record(Change {
            path: "new.md".into(),
            before: None,
            after: "hello\n".into(),
            summary: "created new.md".into(),
        });
        assert_eq!(journal.undo_last(&root).unwrap(), "removed new.md");
        assert!(!root.path().join("new.md").exists());
    }

    #[test]
    fn a_file_changed_since_then_is_refused_rather_than_reverted() {
        // The one failure this must not have: undo turning into data loss. Somebody edited the
        // file by hand, and putting our version back would discard work they never saw us take.
        let t = Tree::new("moved on");
        let root = t.root();
        std::fs::write(root.path().join("main.rs"), "somebody else's work\n").unwrap();

        let mut journal = Journal::default();
        journal.record(replacement("main.rs", "one\ntwo\n", "what we wrote\n"));

        let err = journal.undo_last(&root).unwrap_err();
        assert!(matches!(err, JournalError::Blocked(ref why) if why.contains("has changed")));
        // Nothing was touched, and the entry is still there to be shown.
        assert_eq!(
            std::fs::read_to_string(root.path().join("main.rs")).unwrap(),
            "somebody else's work\n"
        );
        assert_eq!(journal.len(), 1);
    }

    #[test]
    fn a_file_that_vanished_is_refused_too() {
        let t = Tree::new("vanished");
        let root = t.root();
        std::fs::remove_file(root.path().join("main.rs")).unwrap();

        let mut journal = Journal::default();
        journal.record(replacement("main.rs", "one\ntwo\n", "what we wrote\n"));
        assert!(matches!(
            journal.undo_last(&root),
            Err(JournalError::Blocked(ref why)) if why.contains("no longer there")
        ));
    }

    #[test]
    fn undoing_walks_backwards_through_the_changes() {
        let t = Tree::new("stack");
        let root = t.root();
        let file = root.path().join("main.rs");

        let mut journal = Journal::default();
        std::fs::write(&file, "second\n").unwrap();
        journal.record(replacement("main.rs", "one\ntwo\n", "second\n"));
        std::fs::write(&file, "third\n").unwrap();
        journal.record(replacement("main.rs", "second\n", "third\n"));

        journal.undo_last(&root).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "second\n");
        journal.undo_last(&root).unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "one\ntwo\n");
        assert!(journal.is_empty());
    }

    #[test]
    fn the_journal_is_bounded_so_it_stays_a_regret_and_not_a_backup() {
        let mut journal = Journal::default();
        for i in 0..REMEMBERED + 10 {
            journal.record(replacement(
                "main.rs",
                &format!("{i}"),
                &format!("{}", i + 1),
            ));
        }
        assert_eq!(journal.len(), REMEMBERED);
        // The oldest went, not the newest — undo has to reach the most recent mistake.
        assert_eq!(journal.last().unwrap().before.as_deref(), Some("49"));
    }

    #[test]
    fn a_journal_is_per_world_and_survives_a_round_trip() {
        // Context does not travel between Worlds: each is a different project with its own
        // conversations and its own history.
        let t = Tree::new("round");
        let mut mine = Journal::default();
        mine.record(replacement("main.rs", "a", "b"));
        mine.save(&t.vault(), "archipelago").unwrap();

        assert_eq!(Journal::load(&t.vault(), "archipelago").len(), 1);
        assert!(Journal::load(&t.vault(), "default").is_empty());
    }

    #[test]
    fn a_journal_entry_cannot_reach_outside_the_project() {
        // The entry is data on disk, so it is treated like anything else a path could come
        // from: resolved through the project root, never trusted for being ours.
        let t = Tree::new("escape");
        let mut journal = Journal::default();
        journal.record(replacement("../../escape.txt", "a", "b"));
        assert!(matches!(
            journal.undo_last(&t.root()),
            Err(JournalError::Blocked(_))
        ));
    }
}
