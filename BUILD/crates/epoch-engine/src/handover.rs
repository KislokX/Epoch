//! Bringing somebody into work that is already under way.
//!
//! ## Why an agent needs this and a model does not
//!
//! A model is composed a fresh conversation every turn (ADR-0012), so handing a Quest to one is
//! nothing: the Composer hands the new speaker the whole Chronicle and they answer. An agent is
//! the opposite — it keeps *its own* session, Epoch holds only the handle, and that session
//! contains exactly what that agent has been told and nothing else.
//!
//! So a colleague joining work already in progress arrives knowing nothing about it. Before this,
//! handing a Quest to an agent was refused outright with *"say what you want done"* — which is
//! true, and left the crew unable to work together at all.
//!
//! ## What travels is the Quest, never a prompt
//!
//! ADR-0025, exactly: goal, what has happened, what was said, by whom. This composes that from
//! the Chronicle — the record — rather than asking anybody to summarise it. A summary would be
//! narration, and narration is the one thing History is not made of.
//!
//! ## Only what they have not been told
//!
//! Somebody who has already worked on this Quest has their own session containing their part of
//! it. Re-sending everything would tell them things they said themselves, in the third person.
//! So the briefing starts after their last contribution — derived from the Chronicle, like
//! `participants`, rather than stored as a marker that could disagree with what happened.

use epoch_kernel::{CharacterId, Entry, Quest, Recorded};

/// Everything in this Quest the given character has not been told.
///
/// The whole Chronicle for somebody who has never taken a turn on it; everything after their
/// last one otherwise.
pub fn unseen_by<'a>(quest: &'a Quest, who: &CharacterId) -> &'a [Recorded] {
    let last = quest.chronicle.iter().rposition(|record| {
        matches!(
            &record.entry,
            Entry::Answered { character, .. }
                | Entry::StageStarted { character, .. }
                | Entry::StageFinished { character, .. }
            if character == who
        )
    });
    match last {
        Some(at) => &quest.chronicle[at + 1..],
        None => &quest.chronicle,
    }
}

/// What to tell somebody who is being handed this Quest.
///
/// `None` when there is nothing they have not already seen — which is the honest answer, and the
/// caller's cue that there is nothing to hand over rather than an empty briefing to send.
///
/// Written as a report *to* them: who asked for what, what colleagues have said, and what exists
/// because of it. Every line traces to a Chronicle entry; nothing is inferred, and no line is
/// invented to smooth the story over.
pub fn briefing(
    quest: &Quest,
    to: &CharacterId,
    crew: &dyn Fn(&CharacterId) -> String,
) -> Option<String> {
    let unseen = unseen_by(quest, to);
    if unseen.is_empty() {
        return None;
    }

    let mut lines = Vec::new();
    if let Some(memory) = &quest.memory {
        lines.push(format!(
            "Earlier work was compacted by {} across {} records:\n{}",
            memory.character, memory.covers_through, memory.summary
        ));
    }
    for record in unseen {
        match &record.entry {
            Entry::Said {
                content,
                attachments,
                images,
            } => {
                lines.push(format!("The user asked: {}", content.trim()));
                for attachment in attachments {
                    lines.push(format!(
                        "The user attached {} as reference:\n{}",
                        attachment.name, attachment.content
                    ));
                }
                // Named, and named as unseeable. A briefing that mentioned an image without
                // that would hand the next specialist a picture they will describe from its
                // filename — the same failure the Composer's note exists to prevent, arriving
                // through the one door that does not go through the Composer.
                for image in images {
                    lines.push(format!(
                        "The user shared an image called {}. Nobody can see it — Epoch cannot show a picture to a model yet.",
                        image.name
                    ));
                }
            }
            Entry::Answered {
                character, content, ..
            } => {
                // Somebody's own words, attributed. This is the part that makes a handover a
                // handover rather than a rumour: the new arrival can see who said what.
                lines.push(format!("{} said: {}", crew(character), content.trim()));
            }
            Entry::Produced { artifact } => {
                // Evidence, named. A Quest is what it left behind (ADR-0025).
                lines.push(format!(
                    "Already produced: {} ({})",
                    artifact.summary.trim(),
                    artifact.reference.trim()
                ));
            }
            // Deliberately not narrated: state changes and approvals are Epoch's bookkeeping,
            // and reading them out would tell a colleague about the machinery rather than about
            // the work.
            // A brain change belongs to the same bookkeeping. It is a real fact about the
            // Quest and the Chronicle keeps it, but telling the next specialist *which machine
            // was down an hour ago* is describing Epoch's plumbing rather than the work.
            Entry::Moved { .. }
            | Entry::StageStarted { .. }
            | Entry::StageFinished { .. }
            | Entry::Approved { .. }
            | Entry::Reassigned { .. }
            | Entry::Compacted { .. } => {}
        }
    }

    if lines.is_empty() {
        return None;
    }

    Some(format!(
        "You are being brought into work that is already under way.\n\n\
         The goal: {}\n\n\
         What has happened so far:\n{}\n\n\
         Carry on from here. Do the part that is yours; if you need somebody else, say so and \
         stop — the user will decide.",
        quest.intent.trim(),
        lines
            .iter()
            .map(|line| format!("- {line}"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::{Artifact, Lifecycle, QuestId};

    fn who(id: &str) -> CharacterId {
        CharacterId::new(id).expect("id")
    }

    fn named(id: &CharacterId) -> String {
        // Whatever the caller's roster says. Here, capitalised, so the assertions read like the
        // sentence a person would receive.
        let raw = id.as_str();
        let mut chars = raw.chars();
        match chars.next() {
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            None => raw.to_owned(),
        }
    }

    fn quest() -> Quest {
        let mut quest = Quest::inaugurate(
            QuestId::from_raw("q1"),
            "default",
            who("mage"),
            "add a section to the readme",
            "add a section to the readme",
            Lifecycle::default(),
            0,
        );
        quest.record(
            1,
            Entry::Said {
                content: "add a section to the readme".into(),
                attachments: Vec::new(),
                images: Vec::new(),
            },
        );
        quest.record(
            2,
            Entry::Answered {
                character: who("mage"),
                content: "Done — I added it.".into(),
                pace: None,
            },
        );
        quest.record(
            3,
            Entry::Produced {
                artifact: Artifact {
                    kind: "file".into(),
                    reference: "README.md".into(),
                    summary: "a new section".into(),
                },
            },
        );
        quest.record(
            4,
            Entry::Said {
                content: "now ask Paladin to check it".into(),
                attachments: Vec::new(),
                images: Vec::new(),
            },
        );
        quest
    }

    #[test]
    fn somebody_new_is_told_the_whole_story() {
        let note = briefing(&quest(), &who("paladin"), &named).expect("there is work to hand over");

        // The goal, so they know what this is for.
        assert!(note.contains("add a section to the readme"));
        // Who said what, attributed. Not "it was done" — Mage did it, and a colleague joining
        // must be able to tell the difference.
        assert!(note.contains("Mage said: Done — I added it."));
        // What actually exists, which is the only part of a Quest that is not narration.
        assert!(note.contains("README.md"));
        // And the same rule everybody else works under.
        assert!(note.contains("the user will decide"));
    }

    #[test]
    fn somebody_who_has_already_worked_on_it_is_told_only_what_is_new() {
        // Mage's own session already holds her part. Re-sending it would tell her what she said
        // herself, in the third person.
        let note = briefing(&quest(), &who("mage"), &named).expect("something happened since");

        assert!(note.contains("now ask Paladin to check it"));
        assert!(
            !note.contains("Done — I added it."),
            "she does not need to be told what she said: {note}"
        );
    }

    #[test]
    fn nothing_new_is_nothing_to_hand_over() {
        // Not an empty briefing. The caller has to be able to tell the difference between "here
        // is the story" and "there is no story yet", because sending an agent an empty report
        // would have it invent a task to fill the silence.
        let mut quest = quest();
        quest.record(
            5,
            Entry::Answered {
                character: who("paladin"),
                content: "Checked; it reads well.".into(),
                pace: None,
            },
        );
        assert!(briefing(&quest, &who("paladin"), &named).is_none());
    }

    #[test]
    fn epochs_own_bookkeeping_is_not_read_out_to_anybody() {
        // State changes and approvals are how Epoch runs a Quest, not what the Quest is about.
        // A colleague briefed on them would be told about the machinery instead of the work.
        let mut quest = Quest::inaugurate(
            QuestId::from_raw("q2"),
            "default",
            who("mage"),
            "tidy the config",
            "tidy the config",
            Lifecycle::default(),
            0,
        );
        quest.record(
            1,
            Entry::Approved {
                granted: true,
                note: Some("write access".into()),
            },
        );
        quest.record(
            2,
            Entry::Moved {
                to: epoch_kernel::QuestState::Working,
                because: "somebody started".into(),
            },
        );

        let note = briefing(&quest, &who("paladin"), &named).expect("the goal is always there");

        // What the work is for: always present, because a Quest begins with the user's words.
        assert!(note.contains("tidy the config"));
        // And nothing about how Epoch is running it.
        assert!(
            !note.contains("write access"),
            "approvals are not the work: {note}"
        );
        assert!(
            !note.contains("somebody started"),
            "state changes are not the work: {note}"
        );
    }
}
