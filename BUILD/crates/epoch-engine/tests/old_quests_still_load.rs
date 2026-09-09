//! A Quest file written before agents had sessions must still open.
//!
//! `sessions` is a new field on a type that is already on the user's disk — 156 KB of real
//! Chronicles in this developer's vault alone. `#[serde(default)]` is what makes that safe, and
//! this asserts it against **the shape the old writer actually produced** rather than against a
//! belief about serde.

use epoch_engine::quest::QuestLog;

#[test]
fn a_quest_written_before_sessions_existed_opens_with_none() {
    let vault = std::env::temp_dir().join("epoch-old-quests");
    let world = vault.join("worlds/default");
    std::fs::create_dir_all(&world).expect("temp vault");

    // Exactly the shape the previous writer produced: no `sessions` key at all.
    let before = r#"{
      "quests": {
        "q1785539145124-0001": {
          "id": "q1785539145124-0001",
          "intent": "summarise this repo",
          "title": "Summarise this repo",
          "world": "default",
          "inaugurated_by": "mage",
          "state": "open",
          "lifecycle": { "stages": [] },
          "stage": 0,
          "chronicle": [],
          "created_at": 1785539145124
        }
      },
      "active": {},
      "nonce": 1
    }"#;
    std::fs::write(world.join("quests.json"), before).expect("write");

    let (log, problem) = QuestLog::load(&vault, "default");

    assert!(
        problem.is_none(),
        "an old file is not a problem: {problem:?}"
    );
    let quest = log
        .history("default")
        .into_iter()
        .next()
        .expect("one quest");
    assert_eq!(quest.title, "Summarise this repo");
    assert!(
        quest.sessions.is_empty(),
        "nobody had a session, and that is not a failure"
    );

    let _ = std::fs::remove_dir_all(&vault);
}
