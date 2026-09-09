//! Writing what a Quest left behind into the user's own library.
//!
//! The Engine half of `epoch_kernel::knowledge`: that module decides *what* is worth
//! remembering, this one decides where it goes and what it looks like when it gets there.
//! Markdown is **a projection** (ADR-0010) — the object above it stays canonical, so a graph, a
//! timeline or a search index can be projected later from the same facts without anybody
//! parsing prose back into structure.
//!
//! ## Why the library, and not Epoch's own vault
//!
//! Because the crew already reads the library. `search_notes` and `read_note` point at it, so a
//! note written here is a note the crew can find on a later Quest — the loop closes without a
//! second store, a second index, or a "knowledge" screen nobody asked for. And the user reads
//! it too: it is their Obsidian vault, and these notes link like any other.
//!
//! ## Epoch writes in one room of somebody else's house
//!
//! Everything goes under a single `Epoch/` folder inside the library, and nothing outside it is
//! ever touched. A user who wants it gone deletes one folder; a user who wants their own notes
//! left alone has that by construction rather than by promise.
//!
//! ## One Quest, one note, forever — under a name a person can read
//!
//! The file is named after the Quest, because a folder full of `q0000000001234-00ff.md` is a
//! folder nobody browses, and browsing is half of what a vault is for. **Identity stays in the
//! frontmatter**, so remembering the same work twice replaces its note rather than leaving two
//! versions of one history to disagree, and renaming a Quest moves its note instead of leaving
//! the old one behind.
//!
//! ## The crew get pages of their own
//!
//! A Quest's note links whoever worked on it, and a wikilink to somebody with no page is an
//! empty page — which is exactly what the first one produced. So each participant gets a page
//! saying who they are, rewritten whenever they take part in work worth remembering. It is the
//! difference between a graph of bare names and a vault that explains itself.

use std::path::{Path, PathBuf};

use epoch_kernel::{KnowledgeObject, QuestState};

/// The one folder inside the user's library that Epoch writes to.
pub const FOLDER: &str = "Epoch";

/// Where the crew's own pages live, inside that folder.
pub const CREW: &str = "Crew";

/// A file name that keeps a title readable and cannot escape its folder.
///
/// Everything a filesystem or a wiki link would choke on becomes a space; the result is
/// collapsed, trimmed and bounded. A title that is nothing but punctuation falls back to the
/// caller's own name for it, because a file called `.md` is worse than an ugly one.
fn as_file_name(title: &str, fallback: &str) -> String {
    let swept: String = title
        .chars()
        .map(|c| {
            if c.is_control() || FORBIDDEN.contains(&c) {
                ' '
            } else {
                c
            }
        })
        .collect();
    let tidy = swept.split_whitespace().collect::<Vec<_>>().join(" ");
    let bounded: String = tidy.chars().take(80).collect();
    let cleaned = bounded.trim_end_matches('.').trim();
    if cleaned.is_empty() {
        fallback.to_owned()
    } else {
        cleaned.to_owned()
    }
}

/// Characters a path or a wiki link cannot carry.
const FORBIDDEN: &[char] = &[
    '/', '\\', ':', '*', '?', '"', '<', '>', '|', '#', '^', '[', ']',
];

/// Where this object's note belongs, given a library root.
pub fn note_path(library: &Path, object: &KnowledgeObject) -> PathBuf {
    library.join(FOLDER).join(format!(
        "{}.md",
        as_file_name(&object.title, object.quest.as_str())
    ))
}

/// Write the note, creating the folder if this is the first one.
///
/// Returns where it went, so a caller can tell the user or open it. Failure is returned rather
/// than swallowed: a library on a disconnected drive is a real thing, and silently forgetting
/// what a Quest produced is the one outcome this subsystem exists to prevent.
pub fn remember(library: &Path, object: &KnowledgeObject) -> Result<PathBuf, String> {
    let path = note_path(library, object);
    let folder = path.parent().expect("a note always has a folder");
    std::fs::create_dir_all(folder)
        .map_err(|err| format!("cannot create {}: {err}", folder.display()))?;
    // A Quest renamed between endings would otherwise leave its old note behind, and two notes
    // claiming to be the same work is precisely what the identity in the frontmatter prevents.
    forget_earlier_notes_for(folder, object, &path);
    std::fs::write(&path, markdown(object))
        .map_err(|err| format!("cannot write {}: {err}", path.display()))?;
    Ok(path)
}

/// Drop any earlier note for this same Quest that its title has since moved away from.
///
/// Identity is the frontmatter, never the file name — which is what leaves the file name free
/// to be something a person can read.
fn forget_earlier_notes_for(folder: &Path, object: &KnowledgeObject, keeping: &Path) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    let stamp = format!("quest: {}", object.quest.as_str());
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path == keeping || path.extension().is_none_or(|kind| kind != "md") {
            continue;
        }
        if std::fs::read_to_string(&path).is_ok_and(|note| note.contains(&stamp)) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Write a page for each person who worked on remembered work.
///
/// Rewritten rather than written once: a character's role, brain and description are the user's
/// to change, and a page describing who somebody used to be is worse than none.
///
/// Problems are returned rather than raised. A crew page is context around the real record, and
/// losing one must never cost the note that says what a Quest produced.
pub fn remember_crew(library: &Path, crew: &[epoch_kernel::CharacterDefinition]) -> Vec<String> {
    let folder = library.join(FOLDER).join(CREW);
    if let Err(err) = std::fs::create_dir_all(&folder) {
        return vec![format!("cannot create {}: {err}", folder.display())];
    }
    let mut problems = Vec::new();
    for who in crew {
        let path = folder.join(format!("{}.md", as_file_name(&who.name, who.id.as_str())));
        if let Err(err) = std::fs::write(&path, crew_markdown(who)) {
            problems.push(format!("cannot write {}: {err}", path.display()));
        }
    }
    problems
}

/// One character's page.
///
/// Their role and their prompt, because between them they are what a reader needs to judge a
/// decision this character made — and because the vault is where somebody goes when a note
/// says "Mage decided this" and they do not know who Mage is.
pub fn crew_markdown(who: &epoch_kernel::CharacterDefinition) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    out.push_str("epoch: crew\n");
    out.push_str(&format!("character: {}\n", who.id));
    out.push_str(&format!("archetype: {}\n", who.archetype.id()));
    out.push_str("---\n\n");

    out.push_str(&format!("# {}\n\n", who.name));
    if !who.role.trim().is_empty() {
        out.push_str(&format!("{}\n\n", who.role.trim()));
    }

    // What thinks for them, when something does. Written down because "Mage decided this" reads
    // differently once you know whether Mage is a local model or a signed-in agent.
    match who.mind.as_ref() {
        Some(mind) => out.push_str(&format!("**Thinks with:** {}\n\n", mind.model())),
        None => out.push_str("**Thinks with:** nothing yet — no brain assigned.\n\n"),
    }

    if !who.prompt.trim().is_empty() {
        out.push_str("## How they work\n\n");
        for line in who.prompt.trim().lines() {
            out.push_str(&format!("> {line}\n"));
        }
        out.push('\n');
    }

    out.push_str(
        "---\n\nWritten by Epoch, and rewritten whenever they take part in work worth remembering.\n",
    );
    out
}

/// The note itself.
///
/// A pure function of the object, which is what keeps the projection disposable: change this
/// and every note can be regenerated, because nothing here is the source of anything.
pub fn markdown(object: &KnowledgeObject) -> String {
    let mut out = String::new();

    // Frontmatter, because this lands in Obsidian and Obsidian reads it. It is also the part
    // that keeps the note machine-readable without anybody parsing the prose underneath.
    out.push_str("---\n");
    out.push_str(&format!("epoch: {}\n", object.kind.id()));
    out.push_str(&format!("quest: {}\n", object.quest.as_str()));
    out.push_str(&format!("world: {}\n", object.world));
    out.push_str(&format!("outcome: {}\n", object.outcome.id()));
    out.push_str(&format!("started: {}\n", object.started_at));
    out.push_str(&format!("remembered: {}\n", object.remembered_at));
    out.push_str("---\n\n");

    out.push_str(&format!("# {}\n\n", object.title));

    // What the user asked, in their words, quoted so it cannot be mistaken for Epoch's summary
    // of what they asked.
    out.push_str("## Asked\n\n");
    for line in object.intent.lines() {
        out.push_str(&format!("> {line}\n"));
    }
    out.push('\n');

    out.push_str(&format!(
        "**Outcome:** {}\n\n",
        said_plainly(object.outcome)
    ));

    // Wikilinks, because this is a vault: every note a character worked on collects under their
    // name without anybody maintaining an index (ADR-0010 — Obsidian projection).
    //
    // The whole path, and the name as the label. `[[Mage]]` alone would resolve to whatever
    // else in the user's vault happens to carry that name, and their notes are not ours to
    // point at.
    if !object.participants.is_empty() {
        let names = object
            .participants
            .iter()
            .map(|who| {
                let file = as_file_name(&who.name, who.id.as_str());
                format!("[[{FOLDER}/{CREW}/{file}|{}]]", who.name)
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("**Worked on it:** {names}\n\n"));
    }

    // The evidence is the note. Everything above is context for it.
    out.push_str("## What now exists\n\n");
    for artifact in &object.evidence {
        out.push_str(&format!(
            "- `{}` ({}) — {}\n",
            artifact.reference, artifact.kind, artifact.summary
        ));
    }
    out.push('\n');

    if !object.decisions.is_empty() {
        out.push_str("## What you decided\n\n");
        for decision in &object.decisions {
            let verdict = if decision.granted {
                "allowed"
            } else {
                "refused"
            };
            match &decision.note {
                Some(note) => out.push_str(&format!("- **{verdict}** — {note}\n")),
                None => out.push_str(&format!("- **{verdict}**\n")),
            }
        }
        out.push('\n');
    }

    // Said once, at the bottom, because somebody will eventually edit one of these by hand and
    // wonder why their edit vanished.
    out.push_str("---\n\nWritten by Epoch when this Quest ended. Rewritten if it is reopened and ends again.\n");
    out
}

/// The outcome as a sentence, rather than as a state id.
///
/// The failures keep their own words. "Blocked" and "Failed" are different things that happened
/// and flattening them into "not completed" is how a History starts lying by omission.
///
/// **`Completed` claims an ending, never a success.** It read "finished, with evidence", which
/// is two claims and Epoch can only make one: the user pressed X, so the work is over. Whether
/// it went well is the evidence's job, and the evidence is listed directly underneath — a Quest
/// that produced nothing produces a note that says nothing was produced (ADR-0025 §7).
///
/// This sentence has now been wrong in three directions. It said "you ended the conversation"
/// about work that finished exactly as asked; then "with evidence" about work that may have
/// left none; then "you ended it", which names a cause Epoch does not always have — a Quest
/// reaches `Completed` when its last stage finishes as well as when somebody presses X, and
/// attributing both to the user is the same kind of invention as the first two.
///
/// What is measured is that it ended. One word, and the evidence underneath says the rest.
fn said_plainly(state: QuestState) -> &'static str {
    match state {
        QuestState::Completed => "finished",
        QuestState::Blocked => "blocked — it could not be done as asked",
        QuestState::Rejected => "rejected — the plan was declined",
        QuestState::Abandoned => "closed — you ended the conversation",
        QuestState::Failed => "failed — attempted, and it did not work",
        QuestState::NeedsRevision => "sent back for revision",
        QuestState::AwaitingApproval => "waiting on a decision",
        QuestState::Working => "still under way",
        QuestState::Open => "open, not started",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::knowledge::Approval;
    use epoch_kernel::quest::{Artifact, QuestId};
    use epoch_kernel::{CharacterId, KnowledgeKind};

    fn object() -> KnowledgeObject {
        KnowledgeObject {
            kind: KnowledgeKind::QuestRecord,
            quest: QuestId::from_raw("q0000000001234-00ff"),
            world: "default".into(),
            title: "Fix the login".into(),
            intent: "arreglá el login, tira 500".into(),
            outcome: QuestState::Completed,
            participants: vec![
                epoch_kernel::Participant {
                    id: CharacterId::new("mage").unwrap(),
                    name: "Mage".into(),
                },
                epoch_kernel::Participant {
                    id: CharacterId::new("paladin").unwrap(),
                    name: "Paladin".into(),
                },
            ],
            evidence: vec![Artifact {
                kind: "file".into(),
                reference: "src/auth.rs".into(),
                summary: "session check no longer panics on an expired token".into(),
            }],
            decisions: vec![Approval {
                granted: false,
                note: Some("not against production".into()),
            }],
            started_at: 1_000,
            remembered_at: 2_000,
        }
    }

    struct Dir(PathBuf);
    impl Dir {
        fn new(name: &str) -> Self {
            let d = std::env::temp_dir().join(format!("epoch-knowledge-{name}"));
            let _ = std::fs::remove_dir_all(&d);
            Self(d)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn the_note_leads_with_what_the_user_asked_in_their_own_words() {
        let note = markdown(&object());

        assert!(note.contains("> arreglá el login, tira 500"));
        assert!(note.contains("# Fix the login"));
        // Machine-readable without parsing the prose.
        assert!(note.contains("quest: q0000000001234-00ff"));
        assert!(note.contains("outcome: completed"));
    }

    #[test]
    fn the_evidence_is_the_note() {
        let note = markdown(&object());

        assert!(note.contains("`src/auth.rs`"));
        assert!(note.contains("session check no longer panics"));
    }

    #[test]
    fn a_refusal_is_written_down_with_its_reason() {
        // A rejected plan followed by a different approach reads as inconsistency until the
        // refusal is visible. It is also the part a person is most likely to need later.
        let note = markdown(&object());

        assert!(note.contains("**refused** — not against production"));
    }

    #[test]
    fn everybody_who_worked_on_it_is_linked_the_way_a_vault_links() {
        // Wikilinks rather than names, so every note a character worked on collects under them
        // in Obsidian without Epoch maintaining an index.
        let note = markdown(&object());

        // The whole path, and the name as the label: `[[Mage]]` alone would resolve to whatever
        // else in the user's vault carries that name.
        assert!(note.contains("[[Epoch/Crew/Mage|Mage]]"), "{note}");
        assert!(note.contains("[[Epoch/Crew/Paladin|Paladin]]"));
    }

    #[test]
    fn closing_a_conversation_claims_only_that_it_was_closed() {
        // The first real note said "stopped before it finished" about a Quest that had created
        // the file it was asked for. Closing is what Epoch watched; finishing is not.
        let mut closed = object();
        closed.outcome = QuestState::Abandoned;

        let note = markdown(&closed);
        assert!(
            note.contains("closed — you ended the conversation"),
            "{note}"
        );
        assert!(!note.contains("before it finished"));
        // And what exists still says so, whatever the ending was called.
        assert!(note.contains("`src/auth.rs`"));
    }

    #[test]
    fn a_failure_says_which_failure_it_was() {
        // "Not completed" would flatten four different endings into one, which is how a History
        // starts lying by leaving things out (ADR-0025).
        let mut blocked = object();
        blocked.outcome = QuestState::Blocked;

        assert!(markdown(&blocked).contains("blocked — it could not be done as asked"));
        assert!(markdown(&blocked).contains("outcome: blocked"));
    }

    #[test]
    fn epoch_writes_in_one_folder_and_the_same_quest_keeps_one_note() {
        let dir = Dir::new("one-folder");
        let mut object = object();

        let first = remember(&dir.0, &object).expect("written");
        assert_eq!(first.file_name().unwrap(), "Fix the login.md");

        // Renamed. The note follows the title — that is the point of naming it after the Quest
        // — and the old one goes, because two notes claiming to be the same work is exactly
        // what the identity in the frontmatter exists to prevent.
        object.title = "Fix the login, properly".into();
        let again = remember(&dir.0, &object).expect("written again");

        assert_eq!(again.file_name().unwrap(), "Fix the login, properly.md");
        assert!(
            !first.exists(),
            "the note under the old title is still there"
        );
        let notes: Vec<_> = std::fs::read_dir(dir.0.join(FOLDER))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|k| k == "md"))
            .collect();
        assert_eq!(notes.len(), 1, "one Quest keeps one note");
        assert!(std::fs::read_to_string(&again)
            .unwrap()
            .contains("# Fix the login, properly"));

        // And nothing was left anywhere else in the user's library.
        let stray: Vec<_> = std::fs::read_dir(&dir.0)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() != FOLDER)
            .collect();
        assert!(stray.is_empty(), "Epoch wrote outside its own folder");
    }

    #[test]
    fn a_title_that_would_not_survive_a_filesystem_still_becomes_a_readable_name() {
        // Titles come from whoever inaugurated the Quest, which is a model writing a sentence.
        // A slash in one must not become a folder, and a title of nothing but punctuation must
        // not become a file called `.md`.
        let mut object = object();
        object.title = "Fix: src/auth.rs <urgent?>".into();
        assert_eq!(
            note_path(Path::new("lib"), &object).file_name().unwrap(),
            "Fix src auth.rs urgent.md"
        );

        object.title = "***".into();
        assert_eq!(
            note_path(Path::new("lib"), &object).file_name().unwrap(),
            "q0000000001234-00ff.md",
            "a title with nothing in it falls back to the identity"
        );
    }

    #[test]
    fn a_character_gets_a_page_saying_who_they_are() {
        // The first note linked `[[mage]]` and Obsidian opened an empty page. A link to nothing
        // is worse than no link: it looks like knowledge that was lost.
        let dir = Dir::new("crew");
        let mage = epoch_kernel::CharacterDefinition {
            id: CharacterId::new("mage").unwrap(),
            name: "Mage".into(),
            archetype: epoch_kernel::CharacterArchetype::Researcher,
            role: "Turns goals into clean designs".into(),
            worlds: Default::default(),
            prompt: "You explore the solution space before committing.".into(),
            skills: Default::default(),
            requested_capabilities: Default::default(),
            mind: None,
            appearance: None,
            draws_in: None,
            speaks_with: None,
            sounds_like: None,
            presence: epoch_kernel::PresenceProfile {
                authored_home: None,
                idle: Vec::new(),
            },
        };

        let problems = remember_crew(&dir.0, std::slice::from_ref(&mage));
        assert!(problems.is_empty(), "{problems:?}");

        let page = std::fs::read_to_string(dir.0.join(FOLDER).join(CREW).join("Mage.md"))
            .expect("a page for Mage");
        assert!(page.contains("# Mage"));
        assert!(page.contains("Turns goals into clean designs"));
        assert!(page.contains("You explore the solution space"));
        // A character with no brain says so rather than leaving the reader to assume one.
        assert!(page.contains("no brain assigned"));
        // And the Quest's link points here, unambiguously.
        assert!(markdown(&object()).contains("[[Epoch/Crew/Mage|Mage]]"));
    }
}
