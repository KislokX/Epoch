//! Does what a Quest produced actually end up where the crew looks?
//!
//! The unit tests either side of this one assert the two halves: `epoch_kernel::knowledge`
//! decides what is worth remembering, and `epoch_engine::knowledge` decides what the note says.
//! Neither answers the question the feature exists for — **can somebody find it afterwards?**
//!
//! So this writes a real note into a real library and then goes looking for it with the same
//! capability a character uses, `search_notes`. Nothing is mocked: a folder on disk, a file
//! written by the Engine, and a search that has never heard of the Knowledge Engine.

use epoch_engine::capabilities;
use epoch_engine::capability::CapabilityRegistry;
use epoch_engine::project::ProjectRoot;
use epoch_kernel::knowledge::Approval;
use epoch_kernel::quest::{Artifact, QuestId};
use epoch_kernel::{
    Arguments, CapabilityId, CharacterId, KnowledgeKind, KnowledgeObject, QuestState, Value,
};

struct Dir(std::path::PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("epoch-knowledge-e2e-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a library folder");
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn produced_something() -> KnowledgeObject {
    KnowledgeObject {
        kind: KnowledgeKind::QuestRecord,
        quest: QuestId::from_raw("q0000000009999-0001"),
        world: "default".into(),
        title: "Fix the expired-token panic".into(),
        intent: "el login tira 500 cuando el token vence".into(),
        outcome: QuestState::Completed,
        participants: vec![epoch_kernel::Participant {
            id: CharacterId::new("mage").unwrap(),
            name: "Mage".into(),
        }],
        evidence: vec![Artifact {
            kind: "file".into(),
            reference: "src/auth.rs".into(),
            summary: "the session check no longer panics on an expired token".into(),
        }],
        decisions: vec![Approval {
            granted: true,
            note: None,
        }],
        started_at: 1_000,
        remembered_at: 2_000,
    }
}

fn library_capabilities(library: &std::path::Path) -> CapabilityRegistry {
    let root = ProjectRoot::open(library.to_str().expect("a utf-8 path")).expect("a library");
    let mut registry = capabilities::without_project(library);
    capabilities::for_library(&mut registry, root);
    registry
}

#[test]
fn a_character_can_find_what_an_earlier_quest_produced() {
    let dir = Dir::new("found");
    let object = produced_something();

    let written = epoch_engine::knowledge::remember(&dir.0, &object).expect("written");
    assert!(written.exists());

    // Now look for it the way a character would, months later, having never been told the note
    // exists. The words searched for are the user's own, because those are the words somebody
    // would remember.
    let registry = library_capabilities(&dir.0);
    let found = registry
        .get(&CapabilityId::new("search_notes").unwrap())
        .expect("the library offers search")
        .run(&Arguments::new().with("query", Value::Text("expired token".into())))
        .expect("a search runs");

    assert!(
        found.content.contains("Fix the expired-token panic"),
        "the note was not findable: {}",
        found.content
    );
    assert!(found.content.contains("expired token"));
}

#[test]
fn what_the_note_says_is_what_the_crew_reads_back() {
    let dir = Dir::new("read");
    let object = produced_something();
    epoch_engine::knowledge::remember(&dir.0, &object).expect("written");

    let registry = library_capabilities(&dir.0);
    let read = registry
        .get(&CapabilityId::new("read_note").unwrap())
        .expect("the library offers reading")
        .run(&Arguments::new().with(
            "note",
            Value::Text(format!(
                "{}/{}",
                epoch_engine::knowledge::FOLDER,
                "Fix the expired-token panic.md"
            )),
        ))
        .expect("a read runs");

    // The evidence, which is the point of the note at all.
    assert!(read.content.contains("src/auth.rs"), "{}", read.content);
    // And the user's own words, unrewritten.
    assert!(read
        .content
        .contains("el login tira 500 cuando el token vence"));
}

#[test]
fn nothing_is_written_anywhere_but_epochs_own_folder() {
    // The library belongs to the user. Epoch writes in one room of it and never anywhere else —
    // a guarantee worth a test rather than a promise in a comment, because the failure is
    // somebody's own notes being touched by software they let in.
    let dir = Dir::new("confined");
    std::fs::write(dir.0.join("my own note.md"), "mine").expect("a note of the user's own");

    epoch_engine::knowledge::remember(&dir.0, &produced_something()).expect("written");

    let top: Vec<String> = std::fs::read_dir(&dir.0)
        .expect("readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();

    assert!(top.contains(&"my own note.md".to_owned()));
    assert_eq!(
        top.len(),
        2,
        "Epoch added something outside its folder: {top:?}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.0.join("my own note.md")).unwrap(),
        "mine"
    );
}
