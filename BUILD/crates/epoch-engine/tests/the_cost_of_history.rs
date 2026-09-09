//! What it costs to open a World with a lot of history in it.
//!
//! ## The claim being tested
//!
//! Missions lists every conversation a World has ever had. Each row needs a title, a state, who
//! took part, whether anything was produced and how much was said — and the last three are
//! **derived from the Chronicle**, so listing conversations reads every Quest file whole and
//! then throws the transcripts away.
//!
//! The roadmap's number for this is 1,000 Quests of 500 entries: active open under 200 ms, and
//! Missions never needing every Chronicle body at once. Whether that is a problem or a
//! hypothesis is a measurement, and the fix — an index beside the files — costs a second author
//! of a truth the files already hold, which this project does not spend without a number.
//!
//! ## What is measured, and what is not
//!
//! Reading and deriving: `QuestLog::history` plus the three derivations a row needs. The
//! projection into a view type is left out because it allocates in proportion to what is already
//! being paid for here — if the read is cheap the projection cannot rescue it, and if the read
//! is expensive the projection is not the reason.
//!
//! ```text
//! cargo test -p epoch-engine --test the_cost_of_history -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

use epoch_engine::{QuestDigest, QuestStore};
use epoch_kernel::{CharacterId, Entry, Lifecycle};

/// The shapes worth asking about: a used World, a heavy one, and the roadmap's claim.
const SHAPES: [(usize, usize); 3] = [(20, 40), (200, 200), (1000, 500)];

fn vault() -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("epoch-history-{n}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Write `quests` Quests of `entries` records each, through the real store.
fn fill(vault: &Path, quests: usize, entries: usize) {
    let mut log = QuestStore::default();
    let mage = CharacterId::new("mage").unwrap();
    let paladin = CharacterId::new("paladin").unwrap();

    for q in 0..quests {
        log.inaugurate(
            "default",
            &mage,
            &format!("piece of work {q}"),
            Lifecycle::default(),
        );
        let quest = log.active_mut("default").expect("just inaugurated");
        for e in 0..entries {
            // Alternating, so `participants` has real work to do and the derivations are not
            // measured against a Chronicle that answers on its first record.
            let at = e as u64;
            if e % 2 == 0 {
                quest.record(
                    at,
                    Entry::Said {
                        content: format!(
                            "and then what about {e}? here is a sentence of the \
                                          sort a person actually types into a chat box."
                        ),
                        attachments: Vec::new(),
                        images: Vec::new(),
                    },
                );
            } else {
                quest.record(
                    at,
                    Entry::Answered {
                        character: if e % 4 == 1 {
                            mage.clone()
                        } else {
                            paladin.clone()
                        },
                        content: format!(
                            "answer {e}, about as long as an answer tends to be \
                                          when somebody is explaining what they just did."
                        ),
                        pace: None,
                    },
                );
            }
        }
        log.save_active(vault, "default").expect("a Quest saves");
        log.set_aside("default");
    }
}

#[test]
#[ignore = "a measurement: it writes thousands of Quest files and reads them back"]
fn opening_a_world_with_history_costs_this_much() {
    println!("quests x entries    on disk        whole       digest");
    for (quests, entries) in SHAPES {
        let dir = vault();
        fill(&dir, quests, entries);

        let bytes: u64 = std::fs::read_dir(dir.join("worlds/default/quests"))
            .expect("a quests directory")
            .flatten()
            .filter_map(|e| e.metadata().ok().map(|m| m.len()))
            .sum();

        let log = QuestStore::default();
        let began = Instant::now();
        let history = log.history(&dir, "default").expect("history reads");
        // Exactly what one Missions row needs, and all of it derived from the Chronicle.
        let rows: usize = history
            .iter()
            .map(|quest| {
                quest.participants().len()
                    + usize::from(quest.produced_evidence())
                    + quest.chronicle.len()
            })
            .sum();
        let whole = began.elapsed();

        // The same answers, from the same files, without building the words.
        let began = Instant::now();
        let (digests, problems) = QuestDigest::read_all(&dir, "default");
        let narrow_rows: usize = digests
            .iter()
            .map(|digest| {
                digest.participants().len()
                    + usize::from(digest.produced_evidence())
                    + digest.records()
            })
            .sum();
        let narrow = began.elapsed();

        assert_eq!(history.len(), quests, "every Quest came back");
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(digests.len(), history.len(), "and the digest saw them all");
        assert_eq!(narrow_rows, rows, "with the same answers");
        println!(
            "{quests:>6} x {entries:<9} {:>7.1} MB {whole:>11?} {narrow:>12?}",
            bytes as f64 / 1_048_576.0
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
