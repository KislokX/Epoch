//! What Epoch is holding that a person might want back.
//!
//! ## Why this is a list and not a button
//!
//! *"Clear cache"* is a button that cannot say what it does, and the button that cannot say what
//! it does is the button nobody presses. What Epoch keeps is not one kind of thing:
//!
//! - **measurements** it can take again in a second, and
//! - **somebody's open conversation**, which looks exactly like a cache from here.
//!
//! An agent's session handle is an opaque string in a Quest file. It is indistinguishable from a
//! cache entry by inspection, and dropping it ends the thread that agent was holding on its own
//! side. So it gets its own line and its own sentence, and it is never swept up with the rest
//! (owner, 2026-08-19).
//!
//! ## Every entry says three things
//!
//! What it is, what it costs to lose, and how big it is **right now** — measured, not estimated.
//! A size nobody measured is the invented reading this project removes everywhere else, and
//! `0 B` next to something is how a person learns there is nothing to clear.

use std::path::{Path, PathBuf};

/// One thing the user can choose to erase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Erasable {
    /// Stable id, so a surface sends back a choice rather than a path.
    pub id: &'static str,
    /// What it is, in a person's words.
    pub what: &'static str,
    /// What is lost by erasing it — and for the free ones, that nothing is.
    pub cost: &'static str,
    /// Bytes held right now. Measured by walking exactly the files below.
    pub bytes: u64,
    /// The files this entry means. Enumerated, like every other removal here.
    pub files: Vec<PathBuf>,
}

/// Everything in the vault a person may choose to erase, measured.
///
/// **Ordered cheapest first**, so the list reads from *"this costs nothing"* down to *"this ends
/// a conversation"* rather than the other way round.
pub fn erasable(vault: &Path) -> Vec<Erasable> {
    let mut all = vec![
        entry(
            "workshop",
            "Models catalogue",
            "Nothing. It is fetched again the next time the Workshop is opened.",
            vec![vault.join("workshop-catalogue.json")],
        ),
        entry(
            "mcp-tools",
            "What connected servers said they can do",
            "Nothing. Each server is asked again on the next connection.",
            vec![vault.join("mcp-tools.json")],
        ),
        entry(
            "mcp-docs",
            "Notes fetched about connected servers",
            "Nothing they wrote. These were downloaded and are downloaded again.",
            listing(&vault.join("mcp-docs")),
        ),
        entry(
            "trace",
            "The last turn, written for debugging",
            "Nothing. It only exists when EPOCH_TRACE_DIR is set.",
            vec![vault.join("last-turn.json")],
        ),
        entry(
            "undo",
            "Undo history for the World editors",
            "The ability to undo edits you have already made to a World's map.",
            worlds(vault, "undo.json"),
        ),
    ];

    // **Last, and never bundled.** It looks like a cache — an opaque handle in a file — and it
    // is the far end of a conversation somebody is still having.
    all.push(Erasable {
        id: "sessions",
        what: "Agent conversation handles",
        cost: "Every agent forgets the thread it was holding. Your Chronicle is untouched — \
               Epoch keeps the record; this is the handle the agent keeps on its own side. The \
               next turn starts them fresh, which costs them the context they had.",
        // Measured as zero on purpose: these live *inside* Quest files, so there is no file to
        // weigh. Erasing them rewrites Quests rather than deleting anything.
        bytes: 0,
        files: Vec::new(),
    });

    all
}

fn entry(
    id: &'static str,
    what: &'static str,
    cost: &'static str,
    files: Vec<PathBuf>,
) -> Erasable {
    let files: Vec<PathBuf> = files.into_iter().filter(|p| p.exists()).collect();
    let bytes = files
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok().map(|m| m.len()))
        .sum();
    Erasable {
        id,
        what,
        cost,
        bytes,
        files,
    }
}

/// Every file directly inside a folder. One level: nothing here nests.
fn listing(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect()
        })
        .unwrap_or_default()
}

/// The same file inside every World's folder.
fn worlds(vault: &Path, name: &str) -> Vec<PathBuf> {
    std::fs::read_dir(vault.join("worlds"))
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path().join(name))
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault() -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("epoch-erasable-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn nothing_held_is_measured_as_nothing_rather_than_hidden() {
        // `0 B` beside a row is how somebody learns there is nothing to clear. A row that
        // vanished would leave them wondering whether Epoch is holding something it will not
        // name.
        let dir = vault();
        let all = erasable(&dir);
        assert!(all.len() >= 6, "every kind is listed, held or not");
        assert!(all.iter().all(|e| e.bytes == 0));
    }

    #[test]
    fn what_is_held_is_weighed_rather_than_estimated() {
        let dir = vault();
        std::fs::write(dir.join("workshop-catalogue.json"), vec![b'x'; 1234]).unwrap();

        let held = erasable(&dir);
        let workshop = held.iter().find(|e| e.id == "workshop").unwrap();
        assert_eq!(workshop.bytes, 1234);
        assert_eq!(workshop.files.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn agent_sessions_are_their_own_line_and_never_a_file() {
        // The one that looks like a cache and is somebody's open conversation. It is last, it
        // has no files to sweep, and its cost says what is actually lost.
        let dir = vault();
        let all = erasable(&dir);
        let sessions = all.last().unwrap();

        assert_eq!(sessions.id, "sessions");
        assert!(sessions.files.is_empty(), "they live inside Quest files");
        assert!(
            sessions.cost.contains("Chronicle is untouched"),
            "the record is Epoch's and stays: {}",
            sessions.cost
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
