//! A World's history outlives the session.
//!
//! The whole reason this matters: a Quest that died with the process left its **evidence**
//! behind and lost the reasoning. The file existed, the conversation that produced it did not —
//! which made History a claim about work nobody could go back and read (ADR-0025).

use std::path::PathBuf;

use epoch_engine::QuestLog;
use epoch_kernel::{CharacterId, Entry, Lifecycle};

struct Vault(PathBuf);

impl Vault {
    fn new(name: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("epoch-quests-{name}-{n}"));
        let _ = std::fs::remove_dir_all(&d);
        Self(d)
    }
}

impl Drop for Vault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn mage() -> CharacterId {
    CharacterId::new("mage").unwrap()
}

#[test]
fn a_quest_and_everything_in_its_chronicle_comes_back() {
    let v = Vault::new("round");
    let mut log = QuestLog::default();

    let id = log.inaugurate(
        "archipelago",
        &mage(),
        "make me a README",
        Lifecycle::default(),
    );
    let quest = log.active_mut("archipelago").unwrap();
    quest.record(
        1,
        Entry::Answered {
            character: mage(),
            content: "on it".into(),
            pace: None,
        },
    );
    quest.record(
        2,
        Entry::Produced {
            artifact: epoch_kernel::Artifact {
                kind: "capability".into(),
                reference: "mage".into(),
                summary: "created README.md".into(),
            },
        },
    );
    log.save(&v.0, "archipelago").unwrap();

    let (read, problem) = QuestLog::load(&v.0, "archipelago");
    assert_eq!(problem, None);

    let back = read
        .active("archipelago")
        .expect("the work being done is still the work");
    assert_eq!(back.id, id);
    assert_eq!(back.title, "make me a README");
    // The reasoning, not only the result: what was asked for, the stage that opened with it,
    // the answer, and the artifact.
    assert_eq!(back.chronicle.len(), 4);
    assert!(
        back.produced_evidence(),
        "and History can still say it produced something"
    );
}

#[test]
fn nothing_saved_is_the_ordinary_first_run_state() {
    let v = Vault::new("absent");
    let (log, problem) = QuestLog::load(&v.0, "archipelago");
    assert!(log.is_empty());
    assert_eq!(problem, None);
}

#[test]
fn history_does_not_travel_between_worlds() {
    // Each World is a different project, with its own conversations. A Chronicle appearing in
    // the wrong one would be worse than losing it.
    let v = Vault::new("apart");
    let mut log = QuestLog::default();
    log.inaugurate("archipelago", &mage(), "here", Lifecycle::default());
    log.save(&v.0, "archipelago").unwrap();

    let (elsewhere, _) = QuestLog::load(&v.0, "default");
    assert!(elsewhere.is_empty());
    assert!(elsewhere.active("archipelago").is_none());
}

#[test]
fn an_unreadable_history_is_reported_rather_than_shown_as_empty() {
    // Losing a history is not the same as never having had one. A surface showing an empty
    // Chronicle where there should be a month of work has told the user something false.
    let v = Vault::new("broken");
    let dir = v.0.join("worlds").join("archipelago");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("quests.json"), "{ not json").unwrap();

    let (log, problem) = QuestLog::load(&v.0, "archipelago");
    assert!(log.is_empty());
    assert!(problem.unwrap().contains("could not be read"));
    // And the file is still there to be recovered by hand.
    assert!(dir.join("quests.json").exists());
}
