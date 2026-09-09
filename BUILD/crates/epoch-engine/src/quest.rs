//! The Quest log — where work is kept, and how a turn is composed from it (ADR-0025).
//!
//! The Kernel owns what a Quest *is*. This owns the collection of them, and the one impure
//! thing a Quest needs that the Kernel cannot have: a clock.
//!
//! ## Composition, not concatenation
//!
//! [`compose_for`] is the Context Composer (ADR-0012) in its smallest honest form. A character
//! about to speak does not receive "the conversation" — they receive a `Conversation` *composed
//! from the Quest*: their own standing instructions, then what has happened, attributed.
//!
//! That indirection is the point. When budgets, summarisation and selection strategies arrive,
//! they arrive here, and nothing above notices.
//!
//! ## One durable Quest at a time
//!
//! The filesystem adapter persists each Quest as its own JSON document. Opening a World reads
//! the most recently changed document; History is an on-demand projection over the Quest folder.
//! That leaves the domain independent of this adapter while keeping the active Chronicle small
//! enough to resume without deserialising a World's whole past (ADR-0014).

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use epoch_kernel::{
    CharacterDefinition, CharacterId, Conversation, Entry, Lifecycle, Message, Quest, QuestId,
    QuestState,
};

/// Milliseconds since the epoch.
///
/// The Kernel has no clock because it has no I/O, so time is supplied to it from here. A Quest
/// that timestamped itself would be a Quest that could not be replayed.
pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Every Quest, and which one each World is currently working on.
///
/// ## It survives the app closing
///
/// A Quest that died with the process left the evidence behind and lost the reasoning: the file
/// existed, the conversation that produced it did not. That made History a claim about work
/// nobody could go back and read (ADR-0025 — History emerges from evidence).
///
/// Stored **per World**, because context does not travel between them: each is a different
/// project with its own conversations. And in the vault rather than the World Pack folder, for
/// the reason that moved the crew out of packs (ADR-0023) — a pack is shipped content, and an
/// update replaces it.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct QuestLog {
    quests: BTreeMap<QuestId, Quest>,
    /// The Quest a World is currently **looking at**.
    ///
    /// One at a time on screen, and that has not changed — a Chronicle belongs to a Quest, not
    /// to a character, so the box shows one piece of work.
    ///
    /// What changed is that putting one down no longer ends it. Several Quests can be open at
    /// once and this points at the one in front of you; the others are still open, still
    /// resumable, and each keeps its own agent session. Amended from "one at a time,
    /// deliberately" by use: people run one piece of work while another is waiting, which is
    /// not the same as switching specialists mid-task.
    active: BTreeMap<String, QuestId>,
    /// Makes two Quests inaugurated in the same millisecond distinguishable.
    nonce: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum QuestError {
    #[error("cannot read {path}: {source}")]
    Read {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} does not exist")]
    Missing { path: std::path::PathBuf },
    #[error("cannot write {path}: {source}")]
    Write {
        path: std::path::PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{path} is not a readable Quest: {source}")]
    MalformedFile {
        path: std::path::PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("{0}")]
    Malformed(String),
}

impl QuestLog {
    /// Read one World's Quests back.
    ///
    /// Absent is the ordinary first-run state. **Unreadable is reported rather than silently
    /// discarded**: losing a history is not the same as never having had one, and a surface
    /// that shows an empty Chronicle where there should be a month of work has told the user
    /// something false.
    pub fn load(vault: &std::path::Path, world: &str) -> (Self, Option<String>) {
        let path = quests_path(vault, world);
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(_) => return (Self::default(), None),
        };
        match serde_json::from_str(&raw) {
            Ok(log) => (log, None),
            Err(err) => (
                Self::default(),
                Some(format!(
                    "{} could not be read ({err}); this World's history is not being shown. \
                     Nothing has been overwritten.",
                    path.display()
                )),
            ),
        }
    }

    /// Write one World's Quests.
    ///
    /// JSON rather than TOML: a Chronicle is machine-written, append-heavy and full of
    /// multi-line text with quotes in it. TOML is for what a person authors.
    pub fn save(&self, vault: &std::path::Path, world: &str) -> Result<(), QuestError> {
        let path = quests_path(vault, world);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| QuestError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let body =
            serde_json::to_string_pretty(self).map_err(|e| QuestError::Malformed(e.to_string()))?;
        std::fs::write(&path, body).map_err(|source| QuestError::Write { path, source })
    }

    /// Whether there is anything worth writing. Saves nothing on a World nobody has worked in.
    pub fn is_empty(&self) -> bool {
        self.quests.is_empty()
    }
}

/// The filesystem-backed collection of Quests for the World currently in memory.
///
/// A Quest is the aggregate root (ADR-0025), so one file per Quest is the smallest storage
/// unit that lets a turn, a handover and a compaction touch only the work they belong to. The
/// old [`QuestLog`] remains solely to read the alpha's monolithic file during its one-way
/// migration; it is deliberately not the runtime store any more.
///
/// `active` is process state, not another persisted source of truth. On a restart, Epoch opens
/// the most recently changed Quest. That costs one directory metadata scan and one Quest read,
/// rather than deserialising every Chronicle in a World just to resume one conversation.
#[derive(Debug, Default)]
pub struct QuestStore {
    active: BTreeMap<String, Quest>,
}

impl QuestStore {
    /// Open one World's current Quest. The first entry migrates the alpha format before reading
    /// anything from `quests/`; a failed migration leaves `quests.json` in place rather than
    /// showing an empty history that merely looks like success.
    pub fn load(vault: &Path, world: &str) -> (Self, Option<String>) {
        let mut store = Self::default();
        if let Err(err) = migrate_legacy(vault, world) {
            return (store, Some(err.to_string()));
        }
        if let Err(err) = recover_pending_writes(vault, world) {
            return (store, Some(err.to_string()));
        }

        let path = match newest_quest_path(vault, world) {
            Ok(Some(path)) => path,
            Ok(None) => return (store, None),
            Err(err) => return (store, Some(err.to_string())),
        };
        match read_quest(&path) {
            Ok(quest) if quest.world == world => {
                store.active.insert(world.to_owned(), quest);
                (store, None)
            }
            Ok(_) => (
                store,
                Some(format!(
                    "{} belongs to a different World; it was not opened.",
                    path.display()
                )),
            ),
            Err(err) => (store, Some(err.to_string())),
        }
    }

    /// Persisted identity is generated here, at the I/O boundary. UUIDv7 is sortable by its
    /// creation time and collision-resistant without a counter file shared by every Quest.
    pub fn inaugurate(
        &mut self,
        world: &str,
        by: &CharacterId,
        intent: &str,
        lifecycle: Lifecycle,
    ) -> QuestId {
        let at = now_ms();
        let id = QuestId::from_raw(format!("q_{}", uuid::Uuid::now_v7()));
        let quest = Quest::inaugurate(
            id.clone(),
            world,
            by.clone(),
            intent,
            title_from(intent),
            lifecycle,
            at,
        );
        self.active.insert(world.to_owned(), quest);
        id
    }

    pub fn active(&self, world: &str) -> Option<&Quest> {
        self.active.get(world)
    }

    pub fn active_mut(&mut self, world: &str) -> Option<&mut Quest> {
        self.active.get_mut(world)
    }

    pub fn active_id(&self, world: &str) -> Option<QuestId> {
        self.active(world).map(|quest| quest.id.clone())
    }

    /// The loaded Quest is the only one a turn may inspect synchronously. Callers that need a
    /// different identity must select it explicitly, which keeps an innocent lookup from
    /// loading a World's entire History behind the caller's back.
    pub fn get(&self, id: &QuestId) -> Option<&Quest> {
        self.active.values().find(|quest| &quest.id == id)
    }

    /// The same, to write on.
    ///
    /// **Not `active_mut`, and the difference is the whole of ADR-0034.** Work that outlasts the
    /// turn lands minutes later, on the Quest that asked for it — which by then may not be the one
    /// the user is looking at. Reaching for the active one there is the defect ADR-0025 recorded,
    /// arriving through a slower door.
    pub fn get_mut(&mut self, id: &QuestId) -> Option<&mut Quest> {
        self.active.values_mut().find(|quest| &quest.id == id)
    }

    /// Write only the Quest currently selected in this World.
    pub fn save_active(&self, vault: &Path, world: &str) -> Result<(), QuestError> {
        let Some(quest) = self.active(world) else {
            return Ok(());
        };
        save_quest(vault, quest)
    }

    /// Put work down without changing its file. The next intent creates another Quest; reopening
    /// Epoch deliberately returns to the most recently changed Quest, not a hidden pointer.
    pub fn set_aside(&mut self, world: &str) {
        self.active.remove(world);
    }

    /// Load exactly the Quest the user selected. History can be large; selecting one must not
    /// make every other Chronicle resident in memory.
    pub fn select(&mut self, vault: &Path, world: &str, id: &QuestId) -> Result<bool, QuestError> {
        recover_pending_writes(vault, world)?;
        let path = quest_path(vault, world, id)?;
        let quest = match read_quest(&path) {
            Ok(quest) => quest,
            Err(QuestError::Missing { .. }) => return Ok(false),
            Err(err) => return Err(err),
        };
        if quest.world != world {
            return Ok(false);
        }
        self.active.insert(world.to_owned(), quest);
        Ok(true)
    }

    /// End a Quest without removing its record, and **hand back what ended**.
    ///
    /// An inactive Quest is loaded, amended and put back immediately; closing a History item
    /// must not load every other conversation.
    ///
    /// It returned `bool` and the caller looked the Quest up afterwards to decide whether it
    /// was worth remembering — which found nothing, every time, for the least obvious reason:
    /// closing the *active* Quest sets it aside, and `get` only ever searched the active ones.
    /// So the work was ended and forgotten in the same call. Returning it removes the question
    /// rather than answering it, because the one moment this is knowable is here.
    ///
    /// `None` means there was nothing of that id in this World.
    pub fn close(
        &mut self,
        vault: &Path,
        world: &str,
        id: &QuestId,
    ) -> Result<Option<Quest>, QuestError> {
        if self.active_id(world).as_ref() == Some(id) {
            let quest = self
                .active_mut(world)
                .expect("just compared the active Quest id");
            if !quest.state.is_ended() {
                // **The X on a Quest in Workflows is how a Quest ends** (owner, 2026-08-19),
                // and ending is what `Completed` means — not that it went well. It used to
                // write `Abandoned`, so every note in the vault said "you ended the
                // conversation" about work that had finished exactly as asked.
                //
                // A Quest that already stopped for a stated reason keeps that reason: Blocked
                // and Failed are things that happened, and flattening them here is how a
                // History starts lying by omission.
                quest.state = QuestState::Completed;
            }
            let ended = quest.clone();
            self.save_active(vault, world)?;
            self.set_aside(world);
            return Ok(Some(ended));
        }

        let path = quest_path(vault, world, id)?;
        let mut quest = match read_quest(&path) {
            Ok(quest) => quest,
            Err(QuestError::Missing { .. }) => return Ok(None),
            Err(err) => return Err(err),
        };
        if quest.world != world {
            return Ok(None);
        }
        if !quest.state.is_ended() {
            quest.state = QuestState::Completed;
            save_quest(vault, &quest)?;
        }
        Ok(Some(quest))
    }

    /// Write one Quest back over its own file.
    ///
    /// For edits that are not a turn — forgetting an agent's session handle, for instance —
    /// where the Quest may not be the active one and there is nothing to record.
    pub fn write(&self, vault: &Path, quest: &Quest) -> Result<(), QuestError> {
        save_quest(vault, quest)
    }

    /// The History projection is intentionally on-demand. Opening a dialogue never calls this;
    /// only the Missions surface asks to enumerate all Quest files.
    pub fn history(&self, vault: &Path, world: &str) -> Result<Vec<Quest>, QuestError> {
        recover_pending_writes(vault, world)?;
        let mut quests = Vec::new();
        let directory = quests_dir(vault, world);
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(quests),
            Err(source) => {
                return Err(QuestError::Read {
                    path: directory,
                    source,
                })
            }
        };
        for entry in entries {
            let entry = entry.map_err(|source| QuestError::Read {
                path: directory.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
                continue;
            }
            let quest = read_quest(&path)?;
            if quest.world == world {
                quests.push(quest);
            }
        }
        quests.sort_by(|left, right| right.id.cmp(&left.id));
        Ok(quests)
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }
}

fn quests_dir(vault: &Path, world: &str) -> PathBuf {
    vault.join("worlds").join(world).join("quests")
}

fn legacy_quests_path(vault: &Path, world: &str) -> PathBuf {
    vault.join("worlds").join(world).join("quests.json")
}

fn quest_path(vault: &Path, world: &str, id: &QuestId) -> Result<PathBuf, QuestError> {
    let id = id.as_str();
    if id.is_empty() || id.contains(['/', '\\']) || id == "." || id == ".." {
        return Err(QuestError::Malformed(format!("invalid Quest id '{id}'")));
    }
    Ok(quests_dir(vault, world).join(format!("{id}.json")))
}

fn newest_quest_path(vault: &Path, world: &str) -> Result<Option<PathBuf>, QuestError> {
    let directory = quests_dir(vault, world);
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(QuestError::Read {
                path: directory,
                source,
            })
        }
    };
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in entries {
        let entry = entry.map_err(|source| QuestError::Read {
            path: directory.clone(),
            source,
        })?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let modified = entry
            .metadata()
            .map_err(|source| QuestError::Read {
                path: path.clone(),
                source,
            })?
            .modified()
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(current, current_path)| {
            modified > *current || (modified == *current && path > *current_path)
        }) {
            newest = Some((modified, path));
        }
    }
    Ok(newest.map(|(_, path)| path))
}

/// Finish or roll back the small two-file replacement protocol used by [`save_quest`]. A crash
/// can therefore leave a `.next` or `.previous` sibling, but cannot make the only durable copy
/// disappear. We validate before promotion: a malformed temporary file is a problem to show, not
/// permission to silently substitute an older conversation.
fn recover_pending_writes(vault: &Path, world: &str) -> Result<(), QuestError> {
    let directory = quests_dir(vault, world);
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => {
            entries
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| QuestError::Read {
                    path: directory.clone(),
                    source,
                })?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => {
            return Err(QuestError::Read {
                path: directory,
                source,
            })
        }
    };
    for entry in &entries {
        let next = entry.path();
        if next.extension().and_then(|extension| extension.to_str()) != Some("next") {
            continue;
        }
        let path = next.with_extension("");
        let previous = path.with_extension("json.previous");
        if path.exists() {
            std::fs::remove_file(&next).map_err(|source| QuestError::Write {
                path: next.clone(),
                source,
            })?;
            if previous.exists() {
                std::fs::remove_file(&previous).map_err(|source| QuestError::Write {
                    path: previous,
                    source,
                })?;
            }
            continue;
        }
        read_quest(&next)?;
        std::fs::rename(&next, &path).map_err(|source| QuestError::Write {
            path: next.clone(),
            source,
        })?;
        if previous.exists() {
            std::fs::remove_file(previous).map_err(|source| QuestError::Write { path, source })?;
        }
    }
    for entry in entries {
        let previous = entry.path();
        // A `.next` recovery may already have removed this sibling. The directory entries were
        // captured before that cleanup, so do not turn an otherwise successful recovery into a
        // spurious "file not found" error.
        if !previous.exists() {
            continue;
        }
        if previous
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("previous")
        {
            continue;
        }
        let path = previous.with_extension("");
        if path.exists() {
            std::fs::remove_file(&previous).map_err(|source| QuestError::Write {
                path: previous,
                source,
            })?;
            continue;
        }
        read_quest(&previous)?;
        std::fs::rename(&previous, &path).map_err(|source| QuestError::Write {
            path: previous,
            source,
        })?;
    }
    Ok(())
}

fn read_quest(path: &Path) -> Result<Quest, QuestError> {
    let raw = std::fs::read_to_string(path).map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound => QuestError::Missing {
            path: path.to_path_buf(),
        },
        _ => QuestError::Read {
            path: path.to_path_buf(),
            source,
        },
    })?;
    serde_json::from_str(&raw).map_err(|source| QuestError::MalformedFile {
        path: path.to_path_buf(),
        source,
    })
}

/// A write is recoverable even on a filesystem that cannot replace a populated destination in
/// one rename. The previous file is removed only after the new JSON has been written, parsed and
/// promoted; startup can therefore recover either side of an interrupted replacement.
fn save_quest(vault: &Path, quest: &Quest) -> Result<(), QuestError> {
    let path = quest_path(vault, &quest.world, &quest.id)?;
    let parent = path
        .parent()
        .expect("Quest paths always have a World directory");
    std::fs::create_dir_all(parent).map_err(|source| QuestError::Write {
        path: parent.to_path_buf(),
        source,
    })?;
    let next = path.with_extension("json.next");
    let previous = path.with_extension("json.previous");
    let body = serde_json::to_string_pretty(quest)
        .map_err(|error| QuestError::Malformed(error.to_string()))?;
    std::fs::write(&next, &body).map_err(|source| QuestError::Write {
        path: next.clone(),
        source,
    })?;
    serde_json::from_str::<Quest>(&body).map_err(|source| QuestError::MalformedFile {
        path: next.clone(),
        source,
    })?;

    if path.exists() {
        if previous.exists() {
            std::fs::remove_file(&previous).map_err(|source| QuestError::Write {
                path: previous.clone(),
                source,
            })?;
        }
        std::fs::rename(&path, &previous).map_err(|source| QuestError::Write {
            path: path.clone(),
            source,
        })?;
    }
    if let Err(source) = std::fs::rename(&next, &path) {
        let _ = std::fs::rename(&previous, &path);
        return Err(QuestError::Write { path: next, source });
    }
    if previous.exists() {
        std::fs::remove_file(previous).map_err(|source| QuestError::Write { path, source })?;
    }
    Ok(())
}

/// Convert the alpha's one-file store once. The old file is retained under an explicit legacy
/// name, so a migration is auditable and a failed later change never destroys the only copy.
fn migrate_legacy(vault: &Path, world: &str) -> Result<(), QuestError> {
    let legacy = legacy_quests_path(vault, world);
    if !legacy.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(&legacy).map_err(|source| QuestError::Read {
        path: legacy.clone(),
        source,
    })?;
    let log: QuestLog = serde_json::from_str(&raw).map_err(|source| QuestError::MalformedFile {
        path: legacy.clone(),
        source,
    })?;
    let selected = log.active.get(world).cloned();
    for quest in log.quests.values().filter(|quest| quest.world == world) {
        let destination = quest_path(vault, world, &quest.id)?;
        if destination.exists() {
            let existing = read_quest(&destination)?;
            if existing != *quest {
                return Err(QuestError::Malformed(format!(
                    "cannot migrate {}: {} already exists with different work",
                    legacy.display(),
                    destination.display()
                )));
            }
        } else {
            save_quest(vault, quest)?;
        }
    }
    // The selected conversation is written once more, last, making it the restart default
    // without persisting a second active-pointer file.
    if let Some(selected) = selected.and_then(|id| log.quests.get(&id)) {
        save_quest(vault, selected)?;
    }
    let backup = legacy.with_file_name("quests.legacy-v1.json");
    if backup.exists() {
        return Err(QuestError::Malformed(format!(
            "cannot finish migrating {}: {} already exists",
            legacy.display(),
            backup.display()
        )));
    }
    std::fs::rename(&legacy, backup).map_err(|source| QuestError::Write {
        path: legacy,
        source,
    })?;
    Ok(())
}

fn quests_path(vault: &std::path::Path, world: &str) -> std::path::PathBuf {
    vault.join("worlds").join(world).join("quests.json")
}

impl QuestLog {
    /// Turn an intention into a Quest.
    ///
    /// *Inaugurate*, not create: the work existed the moment the user said what they wanted. A
    /// character gives it a shape the crew can act on (ADR-0025 §2).
    pub fn inaugurate(
        &mut self,
        world: &str,
        by: &CharacterId,
        intent: &str,
        lifecycle: Lifecycle,
    ) -> QuestId {
        let at = now_ms();
        self.nonce = self.nonce.wrapping_add(1);
        let id = QuestId::new(at, self.nonce);

        let quest = Quest::inaugurate(
            id.clone(),
            world,
            by.clone(),
            intent,
            title_from(intent),
            lifecycle,
            at,
        );

        self.active.insert(world.to_owned(), id.clone());
        self.quests.insert(id.clone(), quest);
        id
    }

    /// What this World is working on, if anything.
    pub fn active(&self, world: &str) -> Option<&Quest> {
        self.active.get(world).and_then(|id| self.quests.get(id))
    }

    pub fn active_mut(&mut self, world: &str) -> Option<&mut Quest> {
        let id = self.active.get(world)?.clone();
        self.quests.get_mut(&id)
    }

    pub fn get(&self, id: &QuestId) -> Option<&Quest> {
        self.quests.get(id)
    }

    pub fn get_mut(&mut self, id: &QuestId) -> Option<&mut Quest> {
        self.quests.get_mut(id)
    }

    /// Every Quest in a World, newest first.
    ///
    /// This is History: completed, failed, abandoned and in-flight alike. A list of only
    /// successes would be propaganda (ADR-0025 §7).
    pub fn history(&self, world: &str) -> Vec<&Quest> {
        let mut found: Vec<&Quest> = self.quests.values().filter(|q| q.world == world).collect();
        found.sort_by(|a, b| b.id.cmp(&a.id));
        found
    }

    /// Put a World's active Quest down without ending it, so the next intent starts fresh.
    ///
    /// It stays **open**: it is still in `open_in`, still selectable, and still carries whatever
    /// an agent remembers of it. This is "start another conversation", not "finish this one".
    pub fn set_aside(&mut self, world: &str) {
        self.active.remove(world);
    }

    /// Look at a different Quest of this World.
    ///
    /// Refuses a Quest from somewhere else rather than moving it: a Quest belongs to one World
    /// because its context does (ADR-0025), and a surface handing over the wrong id should learn
    /// that here rather than produce a Chronicle from another project.
    pub fn select(&mut self, world: &str, id: &QuestId) -> bool {
        if self.quests.get(id).is_none_or(|q| q.world != world) {
            return false;
        }
        self.active.insert(world.to_owned(), id.clone());
        true
    }

    /// End one, as what it was.
    ///
    /// **Abandoned**, because that is the truth: the user stopped it. Not deleted, and not
    /// quietly marked complete — a History that kept only successes would be propaganda
    /// (ADR-0025 §7), and one that erased what you closed would be worse.
    pub fn close(&mut self, world: &str, id: &QuestId) -> bool {
        let Some(quest) = self.quests.get_mut(id) else {
            return false;
        };
        if quest.world != world {
            return false;
        }
        if !quest.state.is_ended() {
            quest.state = epoch_kernel::QuestState::Abandoned;
        }
        if self.active.get(world) == Some(id) {
            self.active.remove(world);
        }
        true
    }

    /// The Quests of this World that have not ended, newest first.
    ///
    /// What a surface lists as "the conversations you have going". Everything else — including
    /// these, later — is History.
    pub fn open_in(&self, world: &str) -> Vec<&Quest> {
        let mut found: Vec<&Quest> = self
            .quests
            .values()
            .filter(|q| q.world == world && is_open(q))
            .collect();
        found.sort_by(|a, b| b.id.cmp(&a.id));
        found
    }
}

/// A short title, from what the user said.
///
/// Deliberately crude, and deliberately *theirs*: the first line, trimmed to something that fits
/// a list. Asking a model to name the work would be a turn nobody requested, and inventing a
/// title would put words in their mouth on the one field History reads back most.
fn title_from(intent: &str) -> String {
    let first = intent.trim().lines().next().unwrap_or("").trim();
    // Empty intentions only reach here for a reference the user deliberately sent without a
    // written request. This is a truthful state label, not a model-invented title or attributed
    // user speech; it keeps the Quest discoverable until the user names the work.
    if first.is_empty() {
        return "Attached reference".to_owned();
    }
    if first.chars().count() <= 60 {
        return first.to_owned();
    }
    let cut: String = first.chars().take(57).collect();
    // Break on a word if there is one nearby, so a title does not end mid-syllable.
    match cut.rsplit_once(' ') {
        Some((head, _)) if head.chars().count() > 30 => format!("{head}…"),
        _ => format!("{cut}…"),
    }
}

/// Compose what a character should be given for their turn (ADR-0012).
///
/// Not "the conversation": a `Conversation` built *from* the Quest, for this speaker.
///
/// ## How other people's words are carried
///
/// A chat model has three roles and a Quest has as many voices as the crew has members. So:
///
/// - the character's own prior answers arrive as `assistant` — they are their own words;
/// - everybody else's arrive as `user`, **attributed by name**.
///
/// Folding another character's answer into `assistant` would let a specialist mistake a
/// colleague's plan for something they themselves decided — which is exactly the confusion a
/// crew is supposed to prevent. The attribution is not decoration; it is what keeps
/// contributions distinguishable when they are flattened into a format that has no room for
/// them.
///
/// Everything that is not speech — approvals, state transitions, artifacts — is left out for
/// now. It belongs in the composed context eventually, and putting it there before anything
/// produces it would be composing a shape rather than a fact.
pub fn compose_for(
    quest: &Quest,
    speaker: &CharacterDefinition,
    crew: &[&CharacterDefinition],
) -> Conversation {
    let mut conversation = Conversation::opening(&speaker.prompt);

    // Who else is here, and the one thing a speaker must not do.
    //
    // Observed, not theorised: asked "Mage, could you say hello to Paladin?", Mage said hello
    // *and then answered as Paladin*. Of course she did — nothing had told her Paladin exists,
    // so the only way to satisfy the request was to play both parts.
    //
    // That is not a roleplay quirk to prompt around. It is the model doing the reasonable thing
    // with the context it was given, and the fix belongs where the context is built. A
    // colleague is a real participant with their own mind (ADR-0023), and one character
    // ventriloquising another would make a crew into one model wearing several hats — the exact
    // thing Epoch exists not to be.
    if !crew.is_empty() {
        let roster = crew
            .iter()
            .map(|c| format!("- {} ({}): {}", c.name, c.id, c.role))
            .collect::<Vec<_>>()
            .join("\n");

        conversation.say(Message::system(format!(
            "You are {}. You are working with a crew on a shared piece of work.\n\n\
             The others here are:\n{roster}\n\n\
             They speak for themselves. Never write their lines, never answer on their behalf, \
             and never continue a conversation as though they had already replied. If the user \
             wants one of them, address them and stop — they will answer next. If the user asks \
             what you *think* another member would say, answer as yourself, with your own view.",
            speaker.name,
        )));
    }

    if let Some(memory) = &quest.memory {
        conversation.say(Message::user(format!(
            "[Earlier work compacted by {} across {} records]\n{}",
            memory.character, memory.covers_through, memory.summary
        )));
    }

    for record in &quest.chronicle {
        match &record.entry {
            Entry::Said {
                content,
                attachments,
                images,
            } => {
                conversation.say(Message::user(crate::context::user_message(
                    content,
                    attachments,
                    images,
                    // This composer is not given the turn's tools, so it cannot claim sight for
                    // anybody. `No` is the safe half: it asks for an honest "I cannot see it"
                    // rather than pointing at a capability that may not be there.
                    crate::context::CanLook::No,
                )));
            }
            Entry::Answered {
                character, content, ..
            } if character == &speaker.id => {
                conversation.say(Message::assistant(content));
            }
            Entry::Answered {
                character, content, ..
            } => {
                conversation.say(Message::user(format!("[{character}] {content}")));
            }
            _ => {}
        }
    }

    conversation
}

/// Whether a Quest can still be worked on.
pub fn is_open(quest: &Quest) -> bool {
    !quest.state.is_ended()
}

/// Crew members the user's message appears to be involving.
///
/// Naming a colleague is an **invitation**, not a request for an impression: "ask Paladin if
/// he's finished" wants Paladin to answer, not Mage's guess at what Paladin would say.
///
/// Deliberately a plain name match rather than a model call. Asking a model who was meant would
/// be a turn nobody requested, it would cost a load, and it would put participation — a Runtime
/// responsibility (ADR-0025 §7) — in a character's hands.
///
/// Being a heuristic, it may only ever *offer*. A surface shows the invitation and the user
/// takes it; nothing here brings somebody into a Quest on its own, because loading a second
/// model is the user's memory to spend.
pub fn mentioned<'a>(
    said: &str,
    crew: &[&'a CharacterDefinition],
    speaking: &CharacterId,
) -> Vec<&'a CharacterDefinition> {
    let haystack = said.to_lowercase();
    crew.iter()
        .filter(|c| &c.id != speaking)
        .filter(|c| {
            let name = c.name.to_lowercase();
            // Whole word only: "Robo" must not match inside "robot".
            haystack
                .match_indices(&name)
                .any(|(at, _)| is_word_boundary(&haystack, at, name.len()))
        })
        .copied()
        .collect()
}

fn is_word_boundary(text: &str, at: usize, len: usize) -> bool {
    let before = text[..at].chars().next_back();
    let after = text[at + len..].chars().next();
    let free = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
    free(before) && free(after)
}

/// Move a Quest to working, if it is not already.
pub fn begin_work(quest: &mut Quest, because: impl Into<String>) {
    if quest.state != QuestState::Working {
        quest.move_to(now_ms(), QuestState::Working, because);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::{CharacterArchetype, IdleBehavior, PresenceProfile};

    fn who(id: &str) -> CharacterId {
        CharacterId::new(id).unwrap()
    }

    fn character(id: &str, prompt: &str) -> CharacterDefinition {
        CharacterDefinition {
            id: who(id),
            name: id.into(),
            archetype: CharacterArchetype::Researcher,
            role: "r".into(),
            worlds: [("default".to_string(), Default::default())].into(),
            prompt: prompt.into(),
            skills: Default::default(),
            requested_capabilities: Default::default(),
            mind: None,
            appearance: None,
            draws_in: None,
            speaks_with: None,
            sounds_like: None,
            presence: PresenceProfile {
                authored_home: None,
                idle: vec![IdleBehavior {
                    activity: "reading".into(),
                    seconds: 5,
                }],
            },
        }
    }

    #[test]
    fn the_first_intent_inaugurates_a_quest_and_becomes_the_active_one() {
        // The work exists from the moment the user says what they want.
        let mut log = QuestLog::default();
        let id = log.inaugurate(
            "default",
            &who("mage"),
            "Add a blue button",
            Lifecycle::default(),
        );

        let quest = log
            .active("default")
            .expect("the World is working on something");
        assert_eq!(quest.id, id);
        assert_eq!(quest.intent, "Add a blue button");
        assert_eq!(quest.title, "Add a blue button");
    }

    #[test]
    fn closing_the_active_quest_hands_it_back_so_it_can_be_remembered() {
        // The defect this replaced was invisible from either side. Closing set the active Quest
        // aside, `get` only ever searched the active ones, and the Knowledge Engine asked for it
        // *after* the close — so every piece of work was ended and forgotten in one call, and a
        // test that looked the Quest up directly passed because it never took that path.
        let vault = std::env::temp_dir().join("epoch-close-hands-back");
        let _ = std::fs::remove_dir_all(&vault);
        // `QuestStore`, which is what the desktop shell closes through — the in-memory
        // `QuestLog` below has a `close` of its own, and testing that one would have proved
        // nothing about the path a user takes.
        let mut log = QuestStore::default();
        let id = log.inaugurate(
            "default",
            &who("mage"),
            "Write pepe.txt",
            Lifecycle::default(),
        );
        log.active_mut("default").unwrap().record(
            1,
            Entry::Produced {
                artifact: epoch_kernel::Artifact {
                    kind: "capability".into(),
                    reference: "pepe.txt".into(),
                    summary: "created pepe.txt".into(),
                },
            },
        );

        let ended = log
            .close(&vault, "default", &id)
            .expect("closing works")
            .expect("it was in this World");

        // Ended as what it was, and still holding what it produced. **`Completed` means it
        // ended**, never that it went well — the X is how a Quest ends, and whether the work
        // was any good is read from the evidence below it.
        assert_eq!(ended.state, QuestState::Completed);
        assert!(epoch_kernel::KnowledgeObject::of(&ended, &|who| who.to_string(), 2).is_some());
        // And it really is no longer the active one, which is what made the old shape fail.
        assert!(log.active("default").is_none());
        assert!(log.get(&id).is_none());

        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn one_quest_carries_several_specialists_and_the_thread_never_restarts() {
        // The correction this module exists for. Under the old per-character model these were
        // two conversations; here it is one piece of work that two people contributed to.
        let mut log = QuestLog::default();
        log.inaugurate(
            "default",
            &who("mage"),
            "Add a blue button",
            Lifecycle::default(),
        );

        let quest = log.active_mut("default").unwrap();
        quest.record(
            1,
            Entry::Answered {
                character: who("mage"),
                content: "Three options".into(),
                pace: None,
            },
        );
        quest.record(
            2,
            Entry::Said {
                content: "Option 1, hand it to Robo".into(),
                attachments: Vec::new(),
                images: Vec::new(),
            },
        );
        quest.record(
            3,
            Entry::Answered {
                character: who("robo"),
                content: "On it".into(),
                pace: None,
            },
        );

        let quest = log.active("default").unwrap();
        assert_eq!(quest.participants(), vec![who("mage"), who("robo")]);
        // Five: the intent, the stage that opened with it, and the three exchanges. A stage
        // is a session with one NPC, and inauguration begins the first one.
        assert_eq!(quest.chronicle.len(), 5, "one thread, not two");
    }

    #[test]
    fn a_speaker_hears_their_own_words_as_their_own_and_a_colleagues_as_a_colleagues() {
        // Folding another character's answer into `assistant` would let a specialist mistake a
        // colleague's plan for their own decision.
        let mut log = QuestLog::default();
        log.inaugurate(
            "default",
            &who("mage"),
            "Add a blue button",
            Lifecycle::default(),
        );
        let quest = log.active_mut("default").unwrap();
        quest.record(
            1,
            Entry::Answered {
                character: who("mage"),
                content: "Three options".into(),
                pace: None,
            },
        );
        quest.record(
            2,
            Entry::Answered {
                character: who("robo"),
                content: "I built it".into(),
                pace: None,
            },
        );

        let robo = character("robo", "You implement.");
        let composed = compose_for(log.active("default").unwrap(), &robo, &[]);

        let shape: Vec<(&str, &str)> = composed
            .messages
            .iter()
            .map(|m| (m.role.id(), m.content.as_str()))
            .collect();

        assert_eq!(
            shape,
            vec![
                ("system", "You implement."),
                ("user", "Add a blue button"),
                ("user", "[mage] Three options"),
                ("assistant", "I built it"),
            ]
        );
    }

    #[test]
    fn a_speaker_is_told_who_else_is_here_and_that_they_speak_for_themselves() {
        // The observed failure: asked to greet Paladin, Mage greeted him *and answered as him*.
        // Nothing had told her Paladin exists, so playing both parts was the only way to
        // satisfy the request.
        let mut log = QuestLog::default();
        log.inaugurate(
            "default",
            &who("mage"),
            "Say hello to Paladin",
            Lifecycle::default(),
        );

        let mage = character("mage", "You explore.");
        let paladin = character("paladin", "You keep things working.");
        let composed = compose_for(log.active("default").unwrap(), &mage, &[&mage, &paladin]);

        let roster = &composed.messages[1];
        assert_eq!(roster.role.id(), "system");
        assert!(roster.content.contains("paladin"), "the crew is named");
        assert!(
            roster.content.contains("Never write their lines"),
            "and the one thing a speaker must not do is said outright"
        );
        // The exception the user asked for survives: an opinion is still hers to give.
        assert!(roster.content.contains("answer as yourself"));
    }

    #[test]
    fn naming_a_colleague_is_an_invitation_and_not_an_impression() {
        let mage = character("mage", "p");
        let paladin = character("paladin", "p");
        let crew = [&mage, &paladin];

        let found = mentioned("Mage, can you say hello to Paladin?", &crew, &who("mage"));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].id, who("paladin"));

        // The speaker is never an invitation to themselves.
        assert!(mentioned("Mage, what do you think?", &crew, &who("mage")).is_empty());
        // Nobody named, nobody invited.
        assert!(mentioned("add a blue button", &crew, &who("mage")).is_empty());
    }

    #[test]
    fn a_handover_puts_what_was_addressed_to_someone_in_front_of_them() {
        // The gap this closes: offering "ASK PALADIN" only switched who you were talking to.
        // Paladin never received anything, so nothing could ever be *passed* between
        // specialists — no plan, no prompt, no files.
        //
        // Composing the Quest for Paladin is what delivers it. Mage's line to him arrives as
        // something said *to* him, attributed, and he answers it himself.
        let mut log = QuestLog::default();
        log.inaugurate(
            "default",
            &who("mage"),
            "Mage, can you say hello to Paladin?",
            Lifecycle::default(),
        );
        let quest = log.active_mut("default").unwrap();
        quest.record(
            1,
            Entry::Answered {
                character: who("mage"),
                content: "Hello, Paladin.".into(),
                pace: None,
            },
        );

        let mage = character("mage", "You explore.");
        let paladin = character("paladin", "You keep things working.");
        let composed = compose_for(log.active("default").unwrap(), &paladin, &[&mage, &paladin]);

        let last = composed.messages.last().unwrap();
        assert_eq!(
            last.role.id(),
            "user",
            "a colleague's line is something said to him"
        );
        assert_eq!(last.content, "[mage] Hello, Paladin.");
        // And nothing of his own is in there yet, because he has not spoken.
        assert!(!composed.messages.iter().any(|m| m.role.id() == "assistant"));
    }

    #[test]
    fn a_name_inside_a_longer_word_is_not_a_mention() {
        // "Robo" must not be found inside "robot", or half the sentences in a software project
        // would summon somebody.
        let robo = character("robo", "p");
        let crew = [&robo];
        assert!(mentioned("build a robotic arm", &crew, &who("mage")).is_empty());
        assert!(mentioned("robots are fine", &crew, &who("mage")).is_empty());
        assert_eq!(mentioned("ask Robo, please", &crew, &who("mage")).len(), 1);
    }

    #[test]
    fn composition_leaves_out_what_is_not_speech() {
        // Approvals, transitions and artifacts belong in composed context eventually. Putting
        // them there before anything produces them would be composing a shape, not a fact.
        let mut log = QuestLog::default();
        log.inaugurate(
            "default",
            &who("mage"),
            "Add a button",
            Lifecycle::default(),
        );
        let quest = log.active_mut("default").unwrap();
        quest.record(
            1,
            Entry::Approved {
                granted: true,
                note: None,
            },
        );
        quest.move_to(2, QuestState::Working, "started");

        let composed = compose_for(log.active("default").unwrap(), &character("mage", "p"), &[]);
        assert_eq!(
            composed.messages.len(),
            2,
            "the prompt and what the user said"
        );
    }

    #[test]
    fn a_title_is_the_users_own_words_and_never_invented() {
        assert_eq!(title_from("  Add a blue button  "), "Add a blue button");
        assert_eq!(title_from("First line\nsecond line"), "First line");

        let long = "Please add a blue button to the dashboard and make sure it matches the existing design system";
        let title = title_from(long);
        assert!(title.ends_with('…'));
        assert!(title.chars().count() <= 61);
        assert!(long.starts_with(title.trim_end_matches('…').trim_end()));
    }

    #[test]
    fn an_attachment_without_words_has_a_truthful_state_label() {
        assert_eq!(title_from(""), "Attached reference");
    }

    #[test]
    fn history_keeps_everything_that_happened_including_what_failed() {
        // A list of only successes would be propaganda.
        let mut log = QuestLog::default();
        let first = log.inaugurate("default", &who("mage"), "One", Lifecycle::default());
        log.get_mut(&first)
            .unwrap()
            .move_to(9, QuestState::Failed, "did not work");
        log.inaugurate("default", &who("mage"), "Two", Lifecycle::default());
        log.inaugurate(
            "archipelago",
            &who("mage"),
            "Elsewhere",
            Lifecycle::default(),
        );

        let history = log.history("default");
        assert_eq!(history.len(), 2, "the failed one is still history");
        assert_eq!(history[0].intent, "Two", "newest first");
        assert!(history.iter().any(|q| q.state == QuestState::Failed));
        assert_eq!(log.history("archipelago").len(), 1, "Worlds keep their own");
    }

    #[test]
    fn setting_a_quest_aside_leaves_it_intact_for_the_next_intent() {
        let mut log = QuestLog::default();
        let id = log.inaugurate("default", &who("mage"), "One", Lifecycle::default());
        log.set_aside("default");

        assert!(log.active("default").is_none());
        assert!(log.get(&id).is_some(), "put down, not thrown away");
        assert!(is_open(log.get(&id).unwrap()), "and not ended either");
    }

    /// An agent's conversation survives closing Epoch.
    ///
    /// This is the visible half: today an agent thread lived in a map in the desktop shell and
    /// was lost on every restart, silently — you came back tomorrow and the character had
    /// forgotten, with nothing on screen saying so. It lives on the Quest now, and the Quest is
    /// written to disk.
    #[test]
    fn an_agents_session_is_written_with_the_work_and_read_back() {
        let vault = std::env::temp_dir().join(format!("epoch-sessions-{}", now_ms()));
        std::fs::create_dir_all(&vault).expect("a temp vault");

        let mut log = QuestLog::default();
        let id = log.inaugurate(
            "default",
            &who("mage"),
            "Fix the parser",
            Lifecycle::default(),
        );
        log.get_mut(&id)
            .expect("just made")
            .continues(&who("mage"), "35c410cc-f20b-4198");
        log.save(&vault, "default").expect("written");

        // Reopened, as a restart would.
        let (read, problem) = QuestLog::load(&vault, "default");
        assert!(problem.is_none());
        assert_eq!(
            read.get(&id).and_then(|q| q.session(&who("mage"))),
            Some("35c410cc-f20b-4198"),
            "the conversation continues where it was"
        );

        let _ = std::fs::remove_dir_all(&vault);
    }

    /// One conversation per Quest per character, never two.
    #[test]
    fn a_second_handle_replaces_the_first_rather_than_joining_it() {
        let mut log = QuestLog::default();
        let id = log.inaugurate("default", &who("mage"), "One", Lifecycle::default());
        let quest = log.get_mut(&id).expect("just made");

        quest.continues(&who("mage"), "first");
        quest.continues(&who("mage"), "second");

        // Two handles for one conversation is a record that has already disagreed with itself.
        assert_eq!(quest.session(&who("mage")), Some("second"));
        assert_eq!(quest.sessions.len(), 1);
    }

    /// Two people on one Quest keep two conversations.
    #[test]
    fn each_character_continues_their_own_conversation() {
        // Seen in use: a model and an agent contributed to the same Quest and History could not
        // tell which did what (ADR-0025) — but their *sessions* are theirs alone.
        let mut log = QuestLog::default();
        let id = log.inaugurate("default", &who("mage"), "One", Lifecycle::default());
        let quest = log.get_mut(&id).expect("just made");

        quest.continues(&who("mage"), "mage-session");
        quest.continues(&who("paladin"), "paladin-session");

        assert_eq!(quest.session(&who("mage")), Some("mage-session"));
        assert_eq!(quest.session(&who("paladin")), Some("paladin-session"));
    }

    /// Several conversations at once, and going back to one.
    ///
    /// The correction use forced: putting work down to start something else left the first one
    /// unreachable, so "a new chat" and "abandon this" were the same button.
    #[test]
    fn a_world_can_have_several_conversations_open_and_return_to_one() {
        let mut log = QuestLog::default();
        let first = log.inaugurate("default", &who("mage"), "One", Lifecycle::default());
        log.set_aside("default");
        let second = log.inaugurate("default", &who("mage"), "Two", Lifecycle::default());

        assert_eq!(log.open_in("default").len(), 2, "both are still going");
        assert_eq!(log.active("default").map(|q| q.id.clone()), Some(second));

        assert!(log.select("default", &first));
        assert_eq!(log.active("default").map(|q| q.id.clone()), Some(first));
    }

    /// Closing is ending, and ending is remembered as what it was.
    #[test]
    fn closing_one_ends_it_and_keeps_it_in_history() {
        let mut log = QuestLog::default();
        let id = log.inaugurate("default", &who("mage"), "One", Lifecycle::default());

        assert!(log.close("default", &id));
        assert!(
            log.open_in("default").is_empty(),
            "no longer something you can continue"
        );
        // Not deleted, and not quietly marked complete: the user stopped it (ADR-0025 §7).
        assert_eq!(
            log.get(&id).map(|q| q.state),
            Some(epoch_kernel::QuestState::Abandoned)
        );
        assert_eq!(log.history("default").len(), 1);
        assert!(log.active("default").is_none());
    }

    /// A Quest belongs to one World, and selecting across them is refused.
    #[test]
    fn a_conversation_from_another_world_is_refused_rather_than_moved() {
        // Context does not travel between Worlds — each is a different project. Answering "yes"
        // here would produce a Chronicle from somebody else's codebase.
        let mut log = QuestLog::default();
        let elsewhere = log.inaugurate("other", &who("mage"), "One", Lifecycle::default());

        assert!(!log.select("default", &elsewhere));
        assert!(!log.close("default", &elsewhere));
        assert!(is_open(log.get(&elsewhere).unwrap()), "and left alone");
    }

    fn store_vault(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("epoch-{name}-{}", uuid::Uuid::now_v7()))
    }

    #[test]
    fn migrating_the_alpha_log_keeps_each_quest_and_leaves_an_auditable_backup() {
        let vault = store_vault("quest-migration");
        let mut legacy = QuestLog::default();
        let first = legacy.inaugurate("default", &who("mage"), "First", Lifecycle::default());
        legacy.set_aside("default");
        let second = legacy.inaugurate("default", &who("mage"), "Second", Lifecycle::default());
        legacy
            .save(&vault, "default")
            .expect("alpha fixture written");

        let (store, problem) = QuestStore::load(&vault, "default");
        assert!(
            problem.is_none(),
            "the migration should be ordinary startup"
        );
        assert_eq!(store.active_id("default"), Some(second.clone()));
        assert!(quest_path(&vault, "default", &first).unwrap().exists());
        assert!(quest_path(&vault, "default", &second).unwrap().exists());
        assert!(vault.join("worlds/default/quests.legacy-v1.json").exists());
        assert!(!legacy_quests_path(&vault, "default").exists());

        let _ = std::fs::remove_dir_all(vault);
    }

    #[test]
    fn migration_refuses_to_replace_a_different_existing_quest() {
        let vault = store_vault("quest-migration-conflict");
        let mut legacy = QuestLog::default();
        let id = legacy.inaugurate("default", &who("mage"), "Original", Lifecycle::default());
        legacy
            .save(&vault, "default")
            .expect("alpha fixture written");

        let conflicting = Quest::inaugurate(
            id.clone(),
            "default",
            who("mage"),
            "Different work",
            "Different work",
            Lifecycle::default(),
            42,
        );
        let destination = quest_path(&vault, "default", &id).expect("safe fixture path");
        std::fs::create_dir_all(destination.parent().expect("Quest folder"))
            .expect("Quest folder created");
        std::fs::write(
            &destination,
            serde_json::to_string_pretty(&conflicting).expect("fixture serialised"),
        )
        .expect("conflict written");

        let (_store, problem) = QuestStore::load(&vault, "default");
        assert!(problem
            .expect("conflict is visible")
            .contains("different work"));
        assert!(legacy_quests_path(&vault, "default").exists());
        assert_eq!(
            read_quest(&destination)
                .expect("conflict stays readable")
                .intent,
            "Different work"
        );

        let _ = std::fs::remove_dir_all(vault);
    }

    #[test]
    fn an_interrupted_replacement_recovers_the_quest_instead_of_forgetting_it() {
        let vault = store_vault("quest-recovery");
        let mut store = QuestStore::default();
        let id = store.inaugurate("default", &who("mage"), "Keep this", Lifecycle::default());
        store.save_active(&vault, "default").expect("first write");
        let path = quest_path(&vault, "default", &id).unwrap();
        let previous = path.with_extension("json.previous");
        std::fs::rename(&path, &previous).expect("simulate the first half of a replacement");

        let (reopened, problem) = QuestStore::load(&vault, "default");
        assert!(
            problem.is_none(),
            "the previous durable file is promoted back"
        );
        assert_eq!(reopened.active_id("default"), Some(id));
        assert!(path.exists(), "the Quest is readable again");

        let _ = std::fs::remove_dir_all(vault);
    }

    #[test]
    fn selecting_one_quest_reads_that_file_without_reopening_the_other_work() {
        let vault = store_vault("quest-select");
        let mut store = QuestStore::default();
        let first = store.inaugurate("default", &who("mage"), "First", Lifecycle::default());
        store.save_active(&vault, "default").expect("first saved");
        store.set_aside("default");
        let second = store.inaugurate("default", &who("mage"), "Second", Lifecycle::default());
        store.save_active(&vault, "default").expect("second saved");

        let mut reopened = QuestStore::default();
        assert!(reopened
            .select(&vault, "default", &first)
            .expect("one file can be selected"));
        assert_eq!(reopened.active_id("default"), Some(first));
        assert_ne!(reopened.active_id("default"), Some(second));

        let _ = std::fs::remove_dir_all(vault);
    }

    #[test]
    fn each_quest_keeps_its_own_session_controls_on_disk() {
        // Mode and reasoning are choices for this conversation, not global switches for the
        // World. Reopening either record must recover its own choices without borrowing the
        // other Quest's settings.
        let vault = store_vault("quest-session-controls");
        let mut store = QuestStore::default();

        let first = store.inaugurate("default", &who("mage"), "First", Lifecycle::default());
        {
            let quest = store.active_mut("default").expect("first active");
            quest.session.autonomy = Some(epoch_kernel::Autonomy::Manual);
            quest.session.reasoning = Some(epoch_kernel::Reasoning::Low);
        }
        store.save_active(&vault, "default").expect("first saved");
        store.set_aside("default");

        let second = store.inaugurate("default", &who("mage"), "Second", Lifecycle::default());
        {
            let quest = store.active_mut("default").expect("second active");
            quest.session.autonomy = Some(epoch_kernel::Autonomy::Auto);
            quest.session.reasoning = Some(epoch_kernel::Reasoning::High);
        }
        store.save_active(&vault, "default").expect("second saved");

        let mut reopened = QuestStore::default();
        assert!(reopened
            .select(&vault, "default", &first)
            .expect("first selected"));
        let first_settings = &reopened.active("default").expect("first restored").session;
        assert_eq!(
            first_settings.autonomy,
            Some(epoch_kernel::Autonomy::Manual)
        );
        assert_eq!(first_settings.reasoning, Some(epoch_kernel::Reasoning::Low));

        reopened.set_aside("default");
        assert!(reopened
            .select(&vault, "default", &second)
            .expect("second selected"));
        let second_settings = &reopened.active("default").expect("second restored").session;
        assert_eq!(second_settings.autonomy, Some(epoch_kernel::Autonomy::Auto));
        assert_eq!(
            second_settings.reasoning,
            Some(epoch_kernel::Reasoning::High)
        );

        let _ = std::fs::remove_dir_all(vault);
    }
}
