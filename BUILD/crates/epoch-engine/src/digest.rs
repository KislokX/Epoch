//! A Quest as a **list** needs it: every fact a row shows, and none of what was said.
//!
//! ## Why this is not an index
//!
//! Missions shows a row per conversation: a title, a state, who took part, whether anything was
//! produced, how much was said. Three of those are derived from the Chronicle, so listing them
//! read every Quest file whole and threw the transcripts away — 114 MB and 280 ms for a World
//! of a thousand Quests, measured, with the World's lock held across it
//! (`tests/the_cost_of_history.rs`).
//!
//! The obvious fix is a summary file beside each Quest. It is also the wrong one: it would be a
//! **second author of a truth the Quest already holds**, and the day the two disagree the list
//! is the one people read. So this reads the same file and simply declines to build what it
//! does not need. One source, a narrower view of it.
//!
//! ## What is dropped, and what is kept
//!
//! Dropped: the words. `content`, attachments and images are the bulk, and no row shows them.
//! Kept whole: **artifacts**, because the Bridge's console lists what the crew actually did, and
//! **memory**, because a compacted Quest keeps its participants and artifacts there — a digest
//! that ignored it would report a long conversation as having had nobody in it.
//!
//! ## The guarantee
//!
//! Two readers of one file shape can drift. What stops it here is not care but a test: every
//! field this produces is asserted equal to the same field derived from the whole `Quest`, over
//! a Chronicle holding one of everything.

use std::path::Path;

use epoch_kernel::quest::QuestMemory;
use epoch_kernel::{Artifact, CharacterId, QuestId, QuestState};
use serde::Deserialize;

/// One conversation, as a list shows it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestDigest {
    pub id: QuestId,
    pub title: String,
    pub world: String,
    pub state: QuestState,
    /// Present only on a compacted Quest, and load-bearing when it is: the participants and
    /// artifacts of everything that was compacted away live here and nowhere else.
    #[serde(default)]
    pub memory: Option<QuestMemory>,
    /// The Chronicle with its words left out.
    #[serde(default)]
    chronicle: Vec<Record>,
}

#[derive(Debug, Clone, Deserialize)]
struct Record {
    /// When it happened, in milliseconds.
    ///
    /// Kept because *what has this World made* is a question with an order to it, and the answer
    /// is worth nothing sorted by whichever Quest happens to be read first. It costs eight bytes
    /// on a record that is already being read.
    #[serde(default)]
    at: u64,
    entry: DigestEntry,
}

/// A Chronicle entry, keeping only what a row is derived from.
///
/// The variants that carry nothing a list needs collapse into [`DigestEntry::Other`], and their
/// fields are skipped rather than allocated — which is the whole saving.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DigestEntry {
    /// The user. Counted, never read.
    Said,
    Answered {
        character: CharacterId,
    },
    StageStarted {
        character: CharacterId,
    },
    StageFinished {
        character: CharacterId,
    },
    Produced {
        artifact: Artifact,
    },
    /// The user answered a request. Counted as a line in the conversation, and its note is not
    /// needed to count it.
    Approved,
    /// A compaction. Also a line, for the same reason.
    Compacted,
    /// Everything else a Chronicle can hold. A file written by a later Epoch lists here rather
    /// than failing to parse, which is the same rule an unknown action or mark role follows.
    #[serde(other)]
    Other,
}

impl QuestDigest {
    /// Read every Quest of one World as a digest.
    ///
    /// The same directory, the same files and the same order as
    /// [`QuestStore::history`](crate::QuestStore::history) — newest first, because a list of
    /// conversations reads downwards through time.
    ///
    /// A file that will not parse costs itself and is reported, never the rest of the World's
    /// history: one bad Quest must not empty Missions.
    pub fn read_all(vault: &Path, world: &str) -> (Vec<Self>, Vec<String>) {
        let directory = vault.join("worlds").join(world).join("quests");
        let mut digests = Vec::new();
        let mut problems = Vec::new();

        let Ok(entries) = std::fs::read_dir(&directory) else {
            // No directory is no history, which is what a new World has.
            return (digests, problems);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            match std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|raw| {
                    serde_json::from_slice::<QuestDigest>(&raw).map_err(|e| e.to_string())
                }) {
                Ok(digest) if digest.world == world => digests.push(digest),
                Ok(_) => {}
                Err(why) => problems.push(format!("{}: {why}", path.display())),
            }
        }
        digests.sort_by(|left, right| right.id.cmp(&left.id));
        (digests, problems)
    }

    /// Everyone who actually contributed, in the order they first did.
    pub fn participants(&self) -> Vec<CharacterId> {
        let mut seen: Vec<CharacterId> = self
            .memory
            .as_ref()
            .map(|memory| memory.participants.clone())
            .unwrap_or_default();
        for record in &self.chronicle {
            let who = match &record.entry {
                DigestEntry::Answered { character, .. }
                | DigestEntry::StageStarted { character }
                | DigestEntry::StageFinished { character } => character,
                _ => continue,
            };
            if !seen.contains(who) {
                seen.push(who.clone());
            }
        }
        seen
    }

    /// Whether it left anything real behind. Talking is not evidence (ADR-0025).
    pub fn produced_evidence(&self) -> bool {
        self.memory
            .as_ref()
            .is_some_and(|memory| !memory.artifacts.is_empty())
            || self
                .chronicle
                .iter()
                .any(|record| matches!(record.entry, DigestEntry::Produced { .. }))
    }

    /// Everything real it left behind, with when it landed.
    ///
    /// The timed half of [`Self::artifacts`]. Compacted work has no time of its own — a
    /// continuity brief replaced the records that had one — so it answers `None` rather than
    /// inventing the moment of compaction, which is not when the thing was made.
    pub fn made(&self) -> Vec<(Option<u64>, &Artifact)> {
        self.memory
            .iter()
            .flat_map(|memory| memory.artifacts.iter().map(|artifact| (None, artifact)))
            .chain(
                self.chronicle
                    .iter()
                    .filter_map(|record| match &record.entry {
                        DigestEntry::Produced { artifact } => Some((Some(record.at), artifact)),
                        _ => None,
                    }),
            )
            .collect()
    }

    /// Everything real it left behind, compacted work included.
    pub fn artifacts(&self) -> Vec<&Artifact> {
        self.memory
            .iter()
            .flat_map(|memory| memory.artifacts.iter())
            .chain(
                self.chronicle
                    .iter()
                    .filter_map(|record| match &record.entry {
                        DigestEntry::Produced { artifact } => Some(artifact),
                        _ => None,
                    }),
            )
            .collect()
    }

    /// How many records the Chronicle holds.
    pub fn records(&self) -> usize {
        self.chronicle.len()
    }

    /// How big the conversation is: every record it holds, plus everything compaction has
    /// already folded away.
    ///
    /// **Every record, not the conversational ones.** Written as a filter first — the entries a
    /// chat draws — and a test comparing it against the surface's own number caught it at once:
    /// four against seven. The existing meaning is the size of the whole Chronicle, and a list
    /// disagreeing with the conversation beside it about how long it is would be the kind of
    /// number nobody debugs and everybody stops trusting.
    pub fn said_count(&self) -> usize {
        self.memory
            .as_ref()
            .map(|memory| memory.covers_through)
            .unwrap_or(0)
            + self.chronicle.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::{Entry, Lifecycle, Quest};

    fn vault() -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("epoch-digest-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn who(id: &str) -> CharacterId {
        CharacterId::new(id).unwrap()
    }

    /// One Chronicle holding one of everything, so the digest is checked against the whole
    /// shape rather than against the two variants somebody remembered.
    fn everything() -> Quest {
        let mut quest = Quest::inaugurate(
            QuestId::new(1_700_000_000_000, 1),
            "default",
            who("mage"),
            "do the thing",
            "The thing",
            Lifecycle::default(),
            1,
        );
        quest.record(
            2,
            Entry::Answered {
                character: who("mage"),
                content: "Here is a plan.".into(),
                pace: None,
            },
        );
        quest.finish_stage(3, who("mage"));
        quest.begin_stage(3, who("paladin"));
        quest.record(
            4,
            Entry::Produced {
                artifact: Artifact {
                    kind: "capability".into(),
                    reference: "write_file".into(),
                    summary: "wrote src/main.rs".into(),
                },
            },
        );
        quest.record(
            5,
            Entry::Approved {
                granted: true,
                note: Some("go ahead".into()),
            },
        );
        quest.move_to(6, QuestState::Completed, "the user ended it");
        quest
    }

    fn save(vault: &Path, quest: &Quest) {
        let dir = vault.join("worlds").join(&quest.world).join("quests");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{}.json", quest.id.as_str())),
            serde_json::to_string_pretty(quest).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn a_digest_answers_exactly_what_the_whole_quest_answers() {
        // **The guarantee.** Two readers of one file shape drift unless something compares
        // them, and the list is the reader people would believe.
        let dir = vault();
        let quest = everything();
        save(&dir, &quest);

        let (digests, problems) = QuestDigest::read_all(&dir, "default");
        assert!(problems.is_empty(), "{problems:?}");
        let digest = &digests[0];

        assert_eq!(digest.id, quest.id);
        assert_eq!(digest.title, quest.title);
        assert_eq!(digest.state, quest.state);
        assert_eq!(digest.participants(), quest.participants());
        assert_eq!(digest.produced_evidence(), quest.produced_evidence());
        assert_eq!(digest.records(), quest.chronicle.len());
        assert_eq!(
            digest.artifacts().len(),
            quest.artifacts().len(),
            "the console lists what the crew did, so artifacts survive whole"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_compacted_quest_still_knows_who_was_in_it() {
        // The failure this guards: compaction moves participants and artifacts into `memory`,
        // so a digest that read only the literal tail would report a long conversation as
        // having had nobody in it and produced nothing.
        let dir = vault();
        let mut quest = everything();
        quest.compact(9, who("mage"), "It went like this.".into(), 1);
        assert!(quest.memory.is_some(), "the fixture really compacted");
        save(&dir, &quest);

        let (digests, _) = QuestDigest::read_all(&dir, "default");
        assert_eq!(digests[0].participants(), quest.participants());
        assert_eq!(digests[0].produced_evidence(), quest.produced_evidence());
        assert_eq!(digests[0].artifacts().len(), quest.artifacts().len());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn one_unreadable_quest_costs_itself_and_not_the_world() {
        let dir = vault();
        save(&dir, &everything());
        let quests = dir.join("worlds/default/quests");
        std::fs::write(quests.join("q_broken.json"), "{ not json").unwrap();

        let (digests, problems) = QuestDigest::read_all(&dir, "default");
        assert_eq!(digests.len(), 1, "the good one still lists");
        assert_eq!(problems.len(), 1, "and the bad one is reported");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
