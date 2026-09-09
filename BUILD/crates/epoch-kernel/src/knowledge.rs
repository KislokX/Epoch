//! What a Quest was worth remembering — canonically, before anybody writes a word of it.
//!
//! ADR-0010's rule, and it is the one ADR-0006 already made for conversations: **knowledge is
//! the canonical representation and Markdown is one projection**. A subsystem that wrote
//! Markdown directly would trap what happened in one lossy format forever; a note in a vault
//! cannot be re-projected into a graph, a timeline or a search index, because by then the
//! structure has been thrown away and only prose remains.
//!
//! ## Evidence, never narration
//!
//! [`KnowledgeObject::of`] returns `None` for a Quest that produced nothing. That is not a
//! failure to record something — it is the record. ADR-0025 is explicit: a Quest that produced
//! no evidence produced nothing, and a History that says otherwise is fiction with a timestamp.
//! Talking is not evidence, and neither is a character reporting that it did something.
//!
//! This is why the type carries `Artifact`s rather than a summary. A summary is somebody's
//! account of the work; an artifact is the work.
//!
//! ## Why the failures are kept
//!
//! A Quest that was Blocked, Rejected, Abandoned or Failed is remembered as what it was, with
//! whatever it did produce along the way. A History that keeps only successes is propaganda —
//! and the failures are the ones somebody is most likely to search for later, because they are
//! the ones that will otherwise be repeated.
//!
//! ## No I/O, and no clock
//!
//! Everything here is a pure function of a [`Quest`]. Where it is written, in what format, and
//! whether it is written at all belong to the Engine (`epoch_engine::knowledge`).

use serde::{Deserialize, Serialize};

use crate::quest::{Artifact, Entry, Quest, QuestId, QuestState};
use crate::CharacterId;

/// What kind of thing is being remembered.
///
/// One variant, because there is one producer. ADR-0010 names several types it expects
/// eventually — ArchitectureDecision, ToolExecution, ResearchFinding — and writing them here
/// before anything emits them would be a taxonomy rather than a contract: names nobody has had
/// to make true yet, which is exactly the abstraction Earn Complexity refuses. Adding a variant
/// when its producer exists is additive, and the projection below already handles the parts
/// every kind shares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeKind {
    /// One piece of work, and what it left behind.
    QuestRecord,
}

impl KnowledgeKind {
    pub const fn id(self) -> &'static str {
        match self {
            KnowledgeKind::QuestRecord => "quest_record",
        }
    }
}

/// A decision the user made during the work, and what they said about it.
///
/// `Approval` rather than `Decision` because `trust::Decision` is a different thing entirely —
/// what the Trust engine concluded about a capability — and two types named the same in one
/// Kernel is a confusion waiting for somebody in a hurry.
///
/// Kept because it is the part of a Quest that explains the rest of it: a rejected plan
/// followed by a different approach reads as inconsistency until the refusal is visible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Approval {
    pub granted: bool,
    pub note: Option<String>,
}

/// Somebody who took part, by identity and by the name a person reads.
///
/// Both, because the two answer different questions: `id` is what nothing may key on a name
/// for (ADR-0023), and `name` is what a note in somebody's vault has to say. The Kernel cannot
/// resolve one into the other — it has no registry — so the caller supplies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Participant {
    pub id: CharacterId,
    pub name: String,
}

/// Something worth remembering, in canonical form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeObject {
    pub kind: KnowledgeKind,
    /// Which Quest this is about. The identity, so re-remembering the same work replaces its
    /// note rather than accumulating near-duplicates of it.
    pub quest: QuestId,
    pub world: String,
    /// The Quest's title, as whoever inaugurated it wrote it.
    pub title: String,
    /// **The user's own words, verbatim.** Never a rewrite, for the reason ADR-0025 gives for
    /// keeping `intent` untouched on the Quest: the request was theirs.
    pub intent: String,
    /// How it ended, as what it was.
    pub outcome: QuestState,
    /// Who contributed, in the order they first did.
    pub participants: Vec<Participant>,
    /// What now exists because of this work. Never empty — see [`KnowledgeObject::of`].
    pub evidence: Vec<Artifact>,
    /// What the user approved or refused along the way.
    pub decisions: Vec<Approval>,
    /// When the work began and when this was recorded, in milliseconds. Supplied by the caller;
    /// the Kernel has no clock.
    pub started_at: u64,
    pub remembered_at: u64,
}

impl KnowledgeObject {
    /// What this Quest is worth remembering, or `None` if it produced nothing.
    ///
    /// `None` is the common answer and a correct one. Most conversations are conversations.
    ///
    /// `name_of` turns an identity into the name a person reads. It is a parameter for the same
    /// reason `remembered_at` is: the Kernel has no registry and no clock.
    pub fn of(
        quest: &Quest,
        name_of: &dyn Fn(&CharacterId) -> String,
        remembered_at: u64,
    ) -> Option<Self> {
        let evidence: Vec<Artifact> = quest
            .chronicle
            .iter()
            .filter_map(|record| match &record.entry {
                Entry::Produced { artifact } => Some(artifact.clone()),
                _ => None,
            })
            .collect();
        if evidence.is_empty() {
            return None;
        }

        let mut participants: Vec<Participant> = Vec::new();
        let mut decisions = Vec::new();
        for record in &quest.chronicle {
            match &record.entry {
                Entry::Answered { character, .. }
                | Entry::StageStarted { character, .. }
                | Entry::StageFinished { character, .. } => {
                    if !participants.iter().any(|who| &who.id == character) {
                        participants.push(Participant {
                            id: character.clone(),
                            name: name_of(character),
                        });
                    }
                }
                Entry::Approved { granted, note } => decisions.push(Approval {
                    granted: *granted,
                    note: note.clone(),
                }),
                _ => {}
            }
        }
        // Whoever started it counts, even if they never spoke again.
        if !participants
            .iter()
            .any(|who| who.id == quest.inaugurated_by)
        {
            participants.insert(
                0,
                Participant {
                    id: quest.inaugurated_by.clone(),
                    name: name_of(&quest.inaugurated_by),
                },
            );
        }

        Some(Self {
            kind: KnowledgeKind::QuestRecord,
            quest: quest.id.clone(),
            world: quest.world.clone(),
            title: quest.title.clone(),
            intent: quest.intent.clone(),
            outcome: quest.state,
            participants,
            evidence,
            decisions,
            started_at: quest.created_at,
            remembered_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quest::{Lifecycle, Recorded};

    fn quest(state: QuestState, entries: Vec<Entry>) -> Quest {
        Quest {
            id: QuestId::from_raw("q_test"),
            intent: "arreglá el login".into(),
            title: "Fix the login".into(),
            world: "default".into(),
            inaugurated_by: CharacterId::new("mage").unwrap(),
            state,
            lifecycle: Lifecycle::default(),
            stage: 0,
            chronicle: entries
                .into_iter()
                .enumerate()
                .map(|(at, entry)| Recorded {
                    at: 1000 + at as u64,
                    entry,
                })
                .collect(),
            memory: None,
            session: Default::default(),
            created_at: 1000,
            sessions: Default::default(),
        }
    }

    /// A stand-in for the shell's registry: the name is the id, capitalised.
    fn named(who: &CharacterId) -> String {
        let raw = who.to_string();
        let mut chars = raw.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => raw,
        }
    }

    fn answered(who: &str) -> Entry {
        Entry::Answered {
            character: CharacterId::new(who).unwrap(),
            content: "done".into(),
            pace: None,
        }
    }

    fn produced(reference: &str) -> Entry {
        Entry::Produced {
            artifact: Artifact {
                kind: "file".into(),
                reference: reference.into(),
                summary: "it exists".into(),
            },
        }
    }

    #[test]
    fn a_quest_that_only_talked_is_not_remembered() {
        // The whole rule, in one assertion: talking is not evidence (ADR-0025). A note saying
        // "the crew discussed the login" is a record of nothing, and a vault full of them is
        // worse than an empty one — it buries the notes that mean something.
        let nothing = quest(
            QuestState::Completed,
            vec![
                Entry::Said {
                    content: "arreglá el login".into(),
                    attachments: Vec::new(),
                    images: Vec::new(),
                },
                answered("mage"),
            ],
        );

        assert!(KnowledgeObject::of(&nothing, &named, 9_000).is_none());
    }

    #[test]
    fn evidence_is_what_makes_a_quest_worth_remembering() {
        let real = quest(
            QuestState::Completed,
            vec![
                answered("mage"),
                produced("src/auth.rs"),
                answered("paladin"),
                produced("tests/auth.rs"),
            ],
        );

        let known = KnowledgeObject::of(&real, &named, 9_000).expect("it produced two files");

        assert_eq!(known.evidence.len(), 2);
        assert_eq!(known.evidence[0].reference, "src/auth.rs");
        // In the order they contributed, and nobody twice.
        assert_eq!(
            known
                .participants
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["Mage", "Paladin"]
        );
        // The user's words, untouched.
        assert_eq!(known.intent, "arreglá el login");
        assert_eq!(known.remembered_at, 9_000);
    }

    #[test]
    fn a_failure_that_produced_something_is_remembered_as_a_failure() {
        // A History that keeps only successes is propaganda (ADR-0025). The half-finished work
        // is also the part somebody will search for before trying again.
        let stopped = quest(
            QuestState::Blocked,
            vec![
                produced("src/auth.rs"),
                Entry::Approved {
                    granted: false,
                    note: Some("not against production".into()),
                },
            ],
        );

        let known = KnowledgeObject::of(&stopped, &named, 9_000).expect("it produced a file first");

        assert_eq!(known.outcome, QuestState::Blocked);
        assert_eq!(known.decisions.len(), 1);
        assert!(!known.decisions[0].granted);
        assert_eq!(
            known.decisions[0].note.as_deref(),
            Some("not against production")
        );
    }

    #[test]
    fn whoever_started_it_is_a_participant_even_if_they_never_spoke_again() {
        let handed_over = quest(
            QuestState::Completed,
            vec![answered("paladin"), produced("a.rs")],
        );

        let known = KnowledgeObject::of(&handed_over, &named, 9_000).expect("evidence");

        assert_eq!(
            known
                .participants
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["Mage", "Paladin"]
        );
    }
}
