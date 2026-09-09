//! Quests — the unit of work (ADR-0025).
//!
//! > **NPCs do not own work. Quests own work. NPCs contribute to Quests.**
//!
//! This is the aggregate root: the unit of persistence, the unit of projection, and what every
//! Experience Surface renders. Change a character's model, replace the character, add a
//! specialist — the Quest continues, because none of them ever held the work.
//!
//! The domain is `Intent → Quest → Execution → Evidence → History`. Everything else —
//! characters, models, Worlds, chats, providers — exists to support that cycle.
//!
//! ## What is deliberately not here
//!
//! No provider, no prompt, no model, no rendering. A Quest is what the work *is*; how any
//! particular stage gets done belongs to the Engine, and how it looks belongs to a surface.
//! The Kernel has zero I/O and this type keeps that true.
//!
//! Artifacts, approvals and tool runs are `Entry` variants rather than separate collections on
//! purpose: they are things that *happened*, in order, and the order is most of the meaning.

use serde::{Deserialize, Serialize};

use crate::{definition::CharacterId, Autonomy, Reasoning};

/// Stable identity of a Quest. Never derived from its goal — rewording what you asked for must
/// not make it a different piece of work.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct QuestId(String);

impl QuestId {
    /// Mint a fresh identity.
    ///
    /// Time-ordered on purpose: Quests are read back as a history, and an id that sorts by
    /// creation means the common ordering needs no index and no clock at read time.
    pub fn new(created_ms: u64, nonce: u32) -> Self {
        Self(format!("q{created_ms:013}-{nonce:04x}"))
    }

    pub fn from_raw(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for QuestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where a Quest currently is.
///
/// Progression belongs to the Runtime, never to a character: **NPCs do not decide where a Quest
/// goes; the Runtime does, based on this** (ADR-0025 §7). That is what keeps the Lifecycle a
/// declarative sequence — iteration happens because the state changed, not because the sequence
/// contains a loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestState {
    /// Inaugurated. The intent exists; nobody has started.
    Open,
    /// A specialist is working on the current stage right now.
    Working,
    /// Waiting for the user. The strongest and most common handoff.
    AwaitingApproval,
    /// Round again. The Runtime routes it back, carrying everything accumulated since.
    NeedsRevision,
    /// **It ended.** The user said so — the X on the Quest in Workflows.
    ///
    /// *Not* a claim that it went well. "Not every Quest can be a success, for obvious
    /// reasons" (owner, 2026-08-19), and a state that meant *success* would leave every
    /// abandoned piece of work either lying about itself or stuck open forever.
    ///
    /// Whether it went well is read from what it **left behind**, never from this: a Quest
    /// that produced no evidence produced nothing, and the History says so (ADR-0025 §7).
    Completed,

    // Failure is an outcome, not an exception path. Each of these is remembered as what it was:
    // a History that keeps only successes is propaganda (ADR-0025 §7).
    /// The work cannot be done as asked, and a specialist said why.
    Blocked,
    /// The user declined the plan.
    Rejected,
    /// The user stopped it.
    Abandoned,
    /// It was attempted and did not work.
    Failed,
}

impl QuestState {
    pub const fn id(self) -> &'static str {
        match self {
            QuestState::Open => "open",
            QuestState::Working => "working",
            QuestState::AwaitingApproval => "awaiting_approval",
            QuestState::NeedsRevision => "needs_revision",
            QuestState::Completed => "completed",
            QuestState::Blocked => "blocked",
            QuestState::Rejected => "rejected",
            QuestState::Abandoned => "abandoned",
            QuestState::Failed => "failed",
        }
    }

    /// Whether the Quest has stopped for good. Every ended Quest belongs to History — including,
    /// and especially, the ones that did not succeed.
    pub const fn is_ended(self) -> bool {
        matches!(
            self,
            QuestState::Completed
                | QuestState::Blocked
                | QuestState::Rejected
                | QuestState::Abandoned
                | QuestState::Failed
        )
    }
}

/// One authored step of a Lifecycle: a named stage, and who does it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    /// What this stage is called. Authored, never interpreted by the Engine.
    pub name: String,
    /// Who is responsible for it.
    pub character: CharacterId,
    /// Whether the Quest stops here for the user before going on.
    #[serde(default)]
    pub needs_approval: bool,
}

/// How a crew collaborates on a Quest — authored data, interpreted by the Runtime (ADR-0011).
///
/// **An ordered list, not a graph.** No conditions, no branches, no loops, no variables. The
/// user is not writing a workflow; they are arranging a team, and every motivating example is a
/// sequence even when a character appears in it twice. A surface may draw it as connected cards
/// because characters are naturally cards — that is presentation over a list.
///
/// Iteration is not expressed here. It happens because the Quest's state changed and the Runtime
/// routed it back (see [`QuestState::NeedsRevision`]). A sequence that secretly encoded retries
/// would be an execution language wearing a list's clothes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lifecycle {
    pub stages: Vec<Stage>,
}

impl Lifecycle {
    pub fn at(&self, index: usize) -> Option<&Stage> {
        self.stages.get(index)
    }

    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }
}

/// Something that happened, in order.
///
/// The Chronicle is not a conversation. It is the Quest's record: what was said, but also what
/// was approved, what was produced, what was run, and every time the Quest changed state. A chat
/// renders the conversational entries — that is a *projection*, and no surface sees all of this
/// (ADR-0025 §2b).
/*
    **`Eq` is gone from this enum, because a rate is a float.**

    `Answered` carries the tokens per second the backend reported, and `f64` has no total equality
    — NaN is not equal to itself. Nothing compares Entries for equality outside tests, which use
    `assert_eq!` and need only `PartialEq`. Keeping `Eq` would have meant storing a measured
    number as an integer to satisfy a trait nothing asked for, and the server's own figure and one
    derived from its token count do not agree: 11.428 reported against 11.61 derived, on the same
    response.
*/
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Entry {
    /// The orchestrator said something, optionally bringing explicitly supplied text along.
    ///
    /// An attachment is part of the user's message rather than a capability. It can only arrive
    /// through an explicit selection on the surface; carrying its contents here lets the same
    /// reference reach a model, an external agent, a handover and a later restart without giving
    /// any of them access to the user's filesystem.
    Said {
        content: String,
        #[serde(default)]
        attachments: Vec<TextAttachment>,
        /// Images the user shared with this message.
        ///
        /// Separate from `attachments` because they are a different kind of thing, not a text
        /// attachment with unusual contents: text is *carried* into a turn and an image is
        /// **referenced**. Putting a picture in the same list would mean either a `content`
        /// holding megabytes of base64 in every composed turn, or a field that is empty for
        /// half the values in it.
        ///
        /// `default` so a Quest written before images existed still reads.
        #[serde(default)]
        images: Vec<ImageAttachment>,
    },
    /// A character said something.
    Answered {
        character: CharacterId,
        content: String,
        /// Tokens per second, **as the backend that produced this measured its own decoding**.
        ///
        /// Epoch measures this everywhere — the bench, the context ladder, QUICK TEST, TIME IT
        /// — and until 2026-09-02 recorded none in the one place somebody lives with it. Asked
        /// *how fast was that reply?*, the honest answer was to open a terminal.
        ///
        /// **Reported, never timed here.** llama.cpp returns `timings.predicted_per_second` and
        /// Ollama `eval_count` over `eval_duration`. A stopwatch around the turn would instead
        /// measure the model loading and the prompt being processed, and call the total a
        /// generation rate.
        ///
        /// `None` is *nobody said*, never zero: an agent, a hosted model and a machine across the
        /// bridge all answer without reporting one, and a rate invented for them would be the
        /// gauge with nothing behind it. `default` so a Quest written before this reads.
        #[serde(default)]
        pace: Option<f64>,
    },
    /// A model-made digest of an earlier Chronicle prefix.
    ///
    /// The records it covers stay in the Chronicle. This is a context projection marker, never
    /// a rewritten past: `through` is the number of earlier records the digest replaces when a
    /// later turn is composed.
    Compacted {
        through: usize,
        character: CharacterId,
        summary: String,
    },
    /// The Quest moved. Carries why, so History can tell the whole story rather than the shape.
    Moved { to: QuestState, because: String },
    /// A stage began.
    StageStarted {
        stage: String,
        character: CharacterId,
    },
    /// A stage finished.
    StageFinished {
        stage: String,
        character: CharacterId,
    },
    /// The user answered a request for approval.
    Approved { granted: bool, note: Option<String> },
    /// **Evidence.** Something real that now exists because of this Quest.
    ///
    /// This is what makes History non-fictional: a Quest that produced no evidence produced
    /// nothing, and the History must say so. Narration is not evidence.
    Produced { artifact: Artifact },
    /// **The user gave a character a different brain, part-way through.**
    ///
    /// A record rather than a settings change, because canonical parameters are behavioural
    /// identity (ADR-0026): the same prompt on a different model is a different character
    /// answering. A Chronicle holding only the replies would show one person changing their
    /// mind, when what happened is that somebody swapped who was thinking.
    ///
    /// The Runtime never does this on its own — no failover, no "fastest available". It exists
    /// because the assigned brain could not be reached and the user answered the question, and
    /// `because` carries what they were answering so History has the whole cause.
    Reassigned {
        character: CharacterId,
        /// What they were assigned before, and what they are now — as the user reads them.
        from: String,
        to: String,
        /// Why the question was asked at all.
        because: String,
    },
}

/// Text the user deliberately attached to one message.
///
/// There is intentionally no path. A path is both private machine information and an authority
/// a character could mistake for permission to read more; Epoch keeps only the name the user saw
/// and the exact text they chose to share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextAttachment {
    pub name: String,
    pub content: String,
}

/// An image the user deliberately attached to one message.
///
/// **A reference, never the bytes.** A Chronicle is read whole on every open and composed from
/// on every turn; a megabyte of base64 inside one would be re-read and re-sent forever. The
/// bytes live once in the vault and this says which file they are.
///
/// No path, for the same two reasons `TextAttachment` has none: a path is private machine
/// information, and it is an authority a character could mistake for permission to read more.
/// `file` is a name the Engine chose inside a folder the Engine owns — it names nothing the
/// user has, and it cannot be pointed anywhere else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageAttachment {
    /// What the user's file was called. Shown, never used to find anything.
    pub name: String,
    /// What Epoch called it, chosen from the bytes (ADR-0024).
    pub file: String,
}

/// Something real a Quest left behind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    /// What kind of thing it is — `"file"`, `"commit"`, `"adr"`, `"test_run"`. Authored by
    /// whatever produced it; the Kernel does not interpret it.
    pub kind: String,
    /// Where it is, in that kind's terms: a path, a hash, an identifier.
    pub reference: String,
    /// One line on what it is, for a History nobody has to decode.
    pub summary: String,
}

/// One entry, and when it happened.
// `Eq` follows `Entry`, which gave it up to carry a measured rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Recorded {
    /// Milliseconds since the epoch. Supplied by the caller — the Kernel has no clock, because
    /// the Kernel has no I/O.
    pub at: u64,
    pub entry: Entry,
}

/// The durable, compacted prefix of a long Chronicle.
///
/// This is not an opaque cache: it is the new canonical representation of work Epoch has
/// deliberately reduced. The small structural roll-ups keep History truthful after literal
/// speech is replaced by the continuity brief.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestMemory {
    pub summary: String,
    pub covers_through: usize,
    pub compacted_at: u64,
    pub character: CharacterId,
    #[serde(default)]
    pub participants: Vec<CharacterId>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub revisions: usize,
    #[serde(default)]
    pub last_contributor: Option<CharacterId>,
}

/// Choices that belong to one conversation, not to the whole World or to a character.
///
/// An agent can work on two Quests at once.  Letting the mode or reasoning dial live on the
/// character made changing it in one conversation silently change the other.  Keeping the
/// overrides next to the Chronicle makes the boundary explicit and durable: selecting a Quest
/// restores exactly the controls that governed its next turn.
///
/// `None` means "use the World/character default".  It keeps existing Quest files compatible
/// and avoids copying mutable defaults into every historical record during migration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestSessionSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autonomy: Option<Autonomy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<Reasoning>,
}

/// The work.
// `Eq` follows `Entry`, which gave it up to carry a measured rate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Quest {
    pub id: QuestId,
    /// What the user actually asked for, in their words.
    ///
    /// Never rewritten. A character *inaugurates* a Quest — turning an intention into something
    /// the crew can act on — but the intention was the user's and stays theirs verbatim.
    pub intent: String,
    /// A short title, for lists and History. Derived by whoever inaugurated it.
    pub title: String,
    /// Which World this belongs to.
    pub world: String,
    /// Who inaugurated it.
    pub inaugurated_by: CharacterId,
    pub state: QuestState,
    /// How this crew collaborates on it. Copied in at inauguration, so editing the Lifecycle
    /// later cannot rewrite the history of work already under way.
    pub lifecycle: Lifecycle,
    /// Which stage is current. Meaningless once the Quest has ended.
    #[serde(default)]
    pub stage: usize,
    /// Everything that has happened, oldest first.
    #[serde(default)]
    pub chronicle: Vec<Recorded>,
    /// The compacted, durable prefix. The Chronicle keeps only the recent literal tail after a
    /// successful `/compact`, while this preserves the context and facts that tail depends on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<QuestMemory>,
    /// Per-conversation controls.  These are intentionally independent from the character's
    /// authored defaults: Quest A may be Manual/Low while Quest B is Auto/Medium.
    #[serde(default)]
    pub session: QuestSessionSettings,
    pub created_at: u64,
    /// Which conversation each **agent** is continuing, on its own side.
    ///
    /// The handle, never a copy of the history: an agent owns its own record, and a second copy
    /// here would be free to drift from it.
    ///
    /// **On the Quest, because a Quest is a conversation.** These lived in a `BTreeMap` in the
    /// desktop shell keyed by `(String, CharacterId)` — domain facts stored outside the domain,
    /// stringly typed, and lost on every restart with nothing said about it. Keyed by character
    /// alone they were worse: two pieces of work with the same person shared one agent session,
    /// so a new chat carried the old one's history.
    ///
    /// Persisted (ADR-0014), which is what lets somebody close Epoch mid-conversation and have
    /// the character remember tomorrow. A handle that turns out to be stale is the agent's to
    /// refuse, and it says so — which is a better failure than silently starting over.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub sessions: std::collections::BTreeMap<CharacterId, String>,
}

impl Quest {
    /// Inaugurate a Quest from an intention.
    ///
    /// *Inaugurate*, not create: the work existed from the moment the user said what they
    /// wanted. A character turns that into something the crew can act on — which is what makes
    /// them a designer of work rather than a router (ADR-0025 §2).
    pub fn inaugurate(
        id: QuestId,
        world: impl Into<String>,
        by: CharacterId,
        intent: impl Into<String>,
        title: impl Into<String>,
        lifecycle: Lifecycle,
        at: u64,
    ) -> Self {
        let intent = intent.into();
        let mut quest = Self {
            id,
            title: title.into(),
            world: world.into(),
            inaugurated_by: by,
            state: QuestState::Open,
            lifecycle,
            stage: 0,
            chronicle: Vec::new(),
            memory: None,
            session: QuestSessionSettings::default(),
            created_at: at,
            intent: intent.clone(),
            // Nobody has worked on it yet. A new Quest is a new conversation on every side.
            sessions: Default::default(),
        };
        // The first thing in every Chronicle is what the user actually asked for.
        quest.record(
            at,
            Entry::Said {
                content: intent,
                attachments: Vec::new(),
                images: Vec::new(),
            },
        );
        // And the first stage is whoever they asked. A Quest is never between stages: somebody
        // has it from the moment it exists.
        let first = quest.inaugurated_by.clone();
        quest.begin_stage(at, first);
        quest
    }

    /// **Begin a stage** — one character's session on this Quest.
    ///
    /// A stage is a session with one NPC (owner, 2026-08-19). It starts when somebody takes the
    /// work and ends when they hand it on, so nothing authors one in advance: a Quest's shape is
    /// the record of who worked on it, in order, and the user steers it a turn at a time —
    /// *"pass it to Paladin"* — rather than arranging it up front.
    ///
    /// **The only writer of `stage`.** The cursor and the Chronicle would otherwise be two
    /// authors of one truth, and the cursor is the one that can be wrong without anybody
    /// noticing. Written here means `stage` is always the number of stages begun, minus one.
    pub fn begin_stage(&mut self, at: u64, character: CharacterId) {
        let begun = self.stages_begun();
        self.stage = begun;
        self.record(
            at,
            Entry::StageStarted {
                // The number a person counts. Nothing authored a name, and inventing one —
                // "Planning", "Implementation" — would be Epoch deciding what the user's work
                // was about.
                stage: (begun + 1).to_string(),
                character,
            },
        );
    }

    /// **End the current stage** — this character has handed the work on.
    pub fn finish_stage(&mut self, at: u64, character: CharacterId) {
        self.record(
            at,
            Entry::StageFinished {
                stage: (self.stage + 1).to_string(),
                character,
            },
        );
    }

    /// How many stages this Quest has had, including the one under way.
    ///
    /// Counted from the record rather than stored, which is what makes `stage` checkable
    /// against something instead of merely believed.
    pub fn stages_begun(&self) -> usize {
        self.chronicle
            .iter()
            .filter(|record| matches!(record.entry, Entry::StageStarted { .. }))
            .count()
    }

    /// Whose session this is right now — the character of the stage under way.
    pub fn stage_owner(&self) -> Option<&CharacterId> {
        self.chronicle
            .iter()
            .rev()
            .find_map(|record| match &record.entry {
                Entry::StageStarted { character, .. } => Some(character),
                _ => None,
            })
    }

    /// Which conversation this character is continuing, if they have started one here.
    pub fn session(&self, who: &CharacterId) -> Option<&str> {
        self.sessions.get(who).map(String::as_str)
    }

    /// Remember the handle an agent handed back.
    ///
    /// Overwrites rather than appends: an agent has one conversation per Quest, and two handles
    /// for one conversation is a record that has already disagreed with itself.
    pub fn continues(&mut self, who: &CharacterId, session: impl Into<String>) {
        self.sessions.insert(who.clone(), session.into());
    }

    /// Deliberately start this agent's next turn in a new external session.
    ///
    /// The Chronicle remains the source of truth. This only drops the opaque handle held by an
    /// outside program after Epoch has persisted a compacted continuity record, so the next
    /// turn can begin small without making the work itself disappear.
    pub fn forget_session(&mut self, who: &CharacterId) -> Option<String> {
        self.sessions.remove(who)
    }

    pub fn record(&mut self, at: u64, entry: Entry) -> &mut Self {
        self.chronicle.push(Recorded { at, entry });
        self
    }

    /// Replace every literal record except the newest `keep_recent` with one durable memory.
    /// The caller supplies a summary only after a model produced one; a failed compaction never
    /// reaches this method and therefore cannot discard the original prefix.
    pub fn compact(
        &mut self,
        at: u64,
        character: CharacterId,
        summary: String,
        keep_recent: usize,
    ) -> Option<usize> {
        let through = self.chronicle.len().checked_sub(keep_recent)?;
        if through == 0 || summary.trim().is_empty() {
            return None;
        }

        let previous = self.memory.take();
        let mut participants = previous
            .as_ref()
            .map(|memory| memory.participants.clone())
            .unwrap_or_default();
        let mut artifacts = previous
            .as_ref()
            .map(|memory| memory.artifacts.clone())
            .unwrap_or_default();
        let mut revisions = previous
            .as_ref()
            .map(|memory| memory.revisions)
            .unwrap_or(0);
        let mut last_contributor = previous
            .as_ref()
            .and_then(|memory| memory.last_contributor.clone());
        let prior_coverage = previous
            .as_ref()
            .map(|memory| memory.covers_through)
            .unwrap_or(0);

        for record in self.chronicle.drain(..through) {
            match record.entry {
                Entry::Answered { character, .. }
                | Entry::StageStarted { character, .. }
                | Entry::StageFinished { character, .. } => {
                    if !participants.contains(&character) {
                        participants.push(character.clone());
                    }
                    last_contributor = Some(character);
                }
                Entry::Produced { artifact } => artifacts.push(artifact),
                Entry::Moved {
                    to: QuestState::NeedsRevision,
                    ..
                } => revisions += 1,
                _ => {}
            }
        }

        let covers_through = prior_coverage + through;
        self.memory = Some(QuestMemory {
            summary,
            covers_through,
            compacted_at: at,
            character,
            participants,
            artifacts,
            revisions,
            last_contributor,
        });
        Some(covers_through)
    }

    /// The latest valid context digest, if somebody deliberately made one.
    ///
    /// A record can only cover entries before itself. Rejecting an impossible future boundary
    /// keeps a malformed persisted file from silently hiding newer history.
    pub fn latest_compaction(&self) -> Option<(usize, usize, &CharacterId, &str)> {
        self.chronicle
            .iter()
            .enumerate()
            .rev()
            .find_map(|(at, recorded)| match &recorded.entry {
                Entry::Compacted {
                    through,
                    character,
                    summary,
                } if *through <= at => Some((at, *through, character, summary.as_str())),
                _ => None,
            })
    }

    /// Move the Quest, recording why.
    ///
    /// Always through here, never by assigning `state`: an unexplained transition is one History
    /// cannot account for, and accounting for them is the entire point of the Chronicle.
    pub fn move_to(&mut self, at: u64, state: QuestState, because: impl Into<String>) -> &mut Self {
        self.state = state;
        self.record(
            at,
            Entry::Moved {
                to: state,
                because: because.into(),
            },
        )
    }

    /// The **authored** stage being worked on, if the Quest is still going and something
    /// authored one.
    ///
    /// Always `None` today, and honestly so: a stage is a session with one NPC and nothing
    /// arranges them in advance. [`Lifecycle`] is kept for the Quest *templates* the World
    /// Editor reserves a door for — a plan, where this is the record. Ask
    /// [`stage_owner`](Self::stage_owner) for who actually has the work.
    pub fn current_stage(&self) -> Option<&Stage> {
        if self.state.is_ended() {
            return None;
        }
        self.lifecycle.at(self.stage)
    }

    /// Everyone who has actually contributed, in the order they first did.
    ///
    /// Derived from the Chronicle rather than stored: a participant list that could disagree
    /// with what happened would eventually disagree with what happened.
    pub fn participants(&self) -> Vec<CharacterId> {
        let mut seen = self
            .memory
            .as_ref()
            .map(|memory| memory.participants.clone())
            .unwrap_or_default();
        for record in &self.chronicle {
            let who = match &record.entry {
                Entry::Answered { character, .. }
                | Entry::StageStarted { character, .. }
                | Entry::StageFinished { character, .. } => character,
                _ => continue,
            };
            if !seen.contains(who) {
                seen.push(who.clone());
            }
        }
        seen
    }

    /// Who most recently worked on this Quest.
    ///
    /// Not `participants().last()`: that list is in order of *first* appearance, so a Quest that
    /// went Mage, Paladin, Mage still ends with Paladin. The question here is who has it now,
    /// which is a different question and has a different answer exactly when it matters.
    ///
    /// Derived from the Chronicle like everything else about a Quest. A stored "current owner"
    /// would be a second answer that could disagree with what actually happened.
    pub fn last_contributor(&self) -> Option<&CharacterId> {
        self.chronicle
            .iter()
            .rev()
            .find_map(|record| match &record.entry {
                Entry::Answered { character, .. }
                | Entry::StageStarted { character, .. }
                | Entry::StageFinished { character, .. } => Some(character),
                _ => None,
            })
            .or_else(|| {
                self.memory
                    .as_ref()
                    .and_then(|memory| memory.last_contributor.as_ref())
            })
    }

    /// Everything real this Quest left behind.
    pub fn artifacts(&self) -> Vec<&Artifact> {
        self.memory
            .iter()
            .flat_map(|memory| memory.artifacts.iter())
            .chain(self.chronicle.iter().filter_map(|r| match &r.entry {
                Entry::Produced { artifact } => Some(artifact),
                _ => None,
            }))
            .collect()
    }

    /// How many times the Quest has been sent round again.
    ///
    /// Part of the story History tells: "took three attempts" is true and worth knowing.
    pub fn revisions(&self) -> usize {
        self.memory
            .as_ref()
            .map(|memory| memory.revisions)
            .unwrap_or(0)
            + self
                .chronicle
                .iter()
                .filter(|r| {
                    matches!(
                        &r.entry,
                        Entry::Moved {
                            to: QuestState::NeedsRevision,
                            ..
                        }
                    )
                })
                .count()
    }

    /// Whether this Quest actually produced anything.
    ///
    /// The question History must be able to ask. A Quest that ended with no evidence produced
    /// nothing, however much was said along the way — and saying so is what keeps a World's
    /// memory from becoming a story about itself.
    pub fn produced_evidence(&self) -> bool {
        self.memory
            .as_ref()
            .is_some_and(|memory| !memory.artifacts.is_empty())
            || self
                .chronicle
                .iter()
                .any(|r| matches!(&r.entry, Entry::Produced { .. }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who(id: &str) -> CharacterId {
        CharacterId::new(id).unwrap()
    }

    fn lifecycle() -> Lifecycle {
        Lifecycle {
            stages: vec![
                Stage {
                    name: "Planning".into(),
                    character: who("mage"),
                    needs_approval: true,
                },
                Stage {
                    name: "Implementation".into(),
                    character: who("robo"),
                    needs_approval: false,
                },
                Stage {
                    name: "Documentation".into(),
                    character: who("marie"),
                    needs_approval: false,
                },
            ],
        }
    }

    fn quest() -> Quest {
        Quest::inaugurate(
            QuestId::new(1_700_000_000_000, 1),
            "default",
            who("mage"),
            "Add a blue button to the dashboard",
            "Blue button",
            lifecycle(),
            1_700_000_000_000,
        )
    }

    #[test]
    fn a_stage_is_one_characters_session_and_a_handover_is_the_boundary() {
        // The owner's own example, as a test (2026-08-19):
        //
        //   stage 1  mage writes a prompt, and the user says "pass it to Paladin"
        //   stage 2  paladin implements it, and the user says "have npc3 document it"
        //   stage 3  npc3 documents it
        //   X        the Quest ends
        //
        // Nothing here is authored in advance. The shape of a Quest is the record of who
        // worked on it, in order, and the user steers it a turn at a time.
        let mut q = quest();

        // Inauguration opens the first stage: a Quest is never between stages, and somebody
        // has it from the moment it exists.
        assert_eq!(q.stages_begun(), 1);
        assert_eq!(q.stage, 0);
        assert_eq!(q.stage_owner(), Some(&who("mage")));

        q.record(
            2,
            Entry::Answered {
                character: who("mage"),
                content: "Here is the prompt.".into(),
                pace: None,
            },
        );

        q.finish_stage(3, who("mage"));
        q.begin_stage(3, who("paladin"));
        assert_eq!(q.stage, 1);
        assert_eq!(q.stage_owner(), Some(&who("paladin")));

        q.finish_stage(4, who("paladin"));
        q.begin_stage(4, who("robo"));
        assert_eq!(q.stage, 2);
        assert_eq!(q.stage_owner(), Some(&who("robo")));

        // **The cursor is never believed on its own.** It is written in exactly one place and
        // is always the number of stages begun, minus one — so a Chronicle and a counter that
        // disagreed would be caught here rather than by somebody reading History and finding
        // the wrong person in it.
        assert_eq!(q.stage, q.stages_begun() - 1);

        // And everyone who held it is in the record, in the order they held it — derived from
        // what happened rather than maintained beside it.
        assert_eq!(
            q.participants(),
            vec![who("mage"), who("paladin"), who("robo")]
        );
    }

    #[test]
    fn ending_a_quest_claims_nothing_about_how_it_went() {
        // `Completed` means it ended — the user pressed X. Not that it succeeded: "not every
        // Quest can be a success, for obvious reasons" (owner, 2026-08-19). Whether it went
        // well is read from what it left behind.
        let mut ended = quest();
        ended.move_to(9, QuestState::Completed, "the user ended it");
        assert!(ended.state.is_ended());
        assert!(
            !ended.produced_evidence(),
            "and this one finished having produced nothing, which History must be able to say"
        );
    }

    #[test]
    fn a_quest_keeps_the_users_words_verbatim() {
        // A character inaugurates a Quest; the intention was the user's and stays theirs.
        let q = quest();
        assert_eq!(q.intent, "Add a blue button to the dashboard");
        assert_eq!(q.state, QuestState::Open);
        assert!(matches!(
            q.chronicle[0].entry,
            Entry::Said { ref content, .. } if content == "Add a blue button to the dashboard"
        ));
    }

    #[test]
    fn session_controls_are_durable_without_rewriting_character_defaults() {
        let mut q = quest();
        q.session.autonomy = Some(Autonomy::Manual);
        q.session.reasoning = Some(Reasoning::Low);

        let restored: Quest = serde_json::from_str(&serde_json::to_string(&q).unwrap()).unwrap();
        assert_eq!(restored.session.autonomy, Some(Autonomy::Manual));
        assert_eq!(restored.session.reasoning, Some(Reasoning::Low));
    }

    #[test]
    fn every_transition_is_accounted_for() {
        // An unexplained move is one History cannot tell the story of.
        let mut q = quest();
        q.move_to(1, QuestState::Working, "Mage began planning");
        q.move_to(2, QuestState::AwaitingApproval, "plan ready for review");

        let moves: Vec<&str> = q
            .chronicle
            .iter()
            .filter_map(|r| match &r.entry {
                Entry::Moved { because, .. } => Some(because.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(moves, vec!["Mage began planning", "plan ready for review"]);
    }

    #[test]
    fn iteration_is_a_state_change_and_not_a_loop_in_the_lifecycle() {
        // The Lifecycle never changes shape. The Quest goes round; the sequence does not know.
        let mut q = quest();
        let shape: Vec<String> = q.lifecycle.stages.iter().map(|s| s.name.clone()).collect();

        q.move_to(1, QuestState::Working, "implementing");
        q.move_to(2, QuestState::NeedsRevision, "tests failed");
        q.stage = 0;
        q.move_to(3, QuestState::Working, "planning again");
        q.move_to(4, QuestState::NeedsRevision, "still wrong");

        assert_eq!(q.revisions(), 2, "History knows it took several attempts");
        assert_eq!(
            q.lifecycle
                .stages
                .iter()
                .map(|s| s.name.clone())
                .collect::<Vec<_>>(),
            shape,
            "the Lifecycle is declarative and never grew a loop"
        );
    }

    #[test]
    fn a_failed_quest_is_ended_and_remembered_as_what_it_was() {
        // Failure is an outcome, not an exception. A History of only successes is propaganda.
        for state in [
            QuestState::Blocked,
            QuestState::Rejected,
            QuestState::Abandoned,
            QuestState::Failed,
        ] {
            let mut q = quest();
            q.move_to(9, state, "the story of why");
            assert!(q.state.is_ended(), "{state:?} must end the Quest");
            assert!(
                q.current_stage().is_none(),
                "an ended Quest has no current stage"
            );
        }
        // And `Completed` ends it too, while claiming nothing about how it went: whether a
        // Quest succeeded is read from what it left behind, never from its state.
        let mut done = quest();
        done.move_to(9, QuestState::Completed, "the user closed it");
        assert!(done.state.is_ended());
    }

    #[test]
    fn history_can_tell_whether_a_quest_actually_produced_anything() {
        // The question that keeps a World's memory from becoming a story about itself.
        let mut q = quest();
        q.record(
            1,
            Entry::Answered {
                character: who("mage"),
                content: "Here are three approaches".into(),
                pace: None,
            },
        );
        q.move_to(2, QuestState::Completed, "user was satisfied");
        assert!(!q.produced_evidence(), "talking is not evidence");
        assert!(q.artifacts().is_empty());

        let mut real = quest();
        real.record(
            1,
            Entry::Produced {
                artifact: Artifact {
                    kind: "file".into(),
                    reference: "src/Button.tsx".into(),
                    summary: "the blue button".into(),
                },
            },
        );
        assert!(real.produced_evidence());
        assert_eq!(real.artifacts()[0].reference, "src/Button.tsx");
    }

    #[test]
    fn participants_are_derived_from_what_happened() {
        // A stored list could disagree with the Chronicle, and eventually would.
        let mut q = quest();
        q.record(
            1,
            Entry::StageStarted {
                stage: "Planning".into(),
                character: who("mage"),
            },
        );
        q.record(
            2,
            Entry::Answered {
                character: who("mage"),
                content: "a plan".into(),
                pace: None,
            },
        );
        q.record(
            3,
            Entry::StageStarted {
                stage: "Implementation".into(),
                character: who("robo"),
            },
        );
        q.record(
            4,
            Entry::Approved {
                granted: true,
                note: None,
            },
        );

        // In the order they first contributed, each once, and the user's own line is not a
        // participant entry.
        assert_eq!(q.participants(), vec![who("mage"), who("robo")]);
    }

    #[test]
    fn the_work_outlives_whoever_is_doing_it() {
        // The whole point of ADR-0025: replace the specialist and the Quest carries on, because
        // the Quest was never theirs.
        let mut q = quest();
        q.record(
            1,
            Entry::StageStarted {
                stage: "Implementation".into(),
                character: who("robo"),
            },
        );
        q.lifecycle.stages[1].character = who("belthasar");

        assert_eq!(q.current_stage().map(|s| s.name.as_str()), Some("Planning"));
        // The intent, the stage inauguration opened, and the one recorded above.
        assert_eq!(q.chronicle.len(), 3, "what already happened is untouched");
        assert!(
            q.participants().contains(&who("robo")),
            "and who did it is still true"
        );
    }

    #[test]
    fn ids_sort_by_when_they_were_made() {
        // Quests are read back as a history, so the common ordering should need no index.
        let early = QuestId::new(1_700_000_000_000, 1);
        let later = QuestId::new(1_700_000_000_001, 0);
        assert!(early < later);
    }

    #[test]
    fn a_compaction_keeps_the_record_and_names_its_exact_prefix() {
        let mut q = quest();
        q.record(
            2,
            Entry::Answered {
                character: who("mage"),
                content: "First answer".into(),
                pace: None,
            },
        );
        let before = q.chronicle.clone();
        q.record(
            3,
            Entry::Compacted {
                through: before.len(),
                character: who("mage"),
                summary: "The user wants a blue button; Mage proposed one.".into(),
            },
        );

        let (at, through, character, summary) = q.latest_compaction().expect("the digest");
        assert_eq!(at, before.len());
        assert_eq!(through, before.len());
        assert_eq!(character, &who("mage"));
        assert!(summary.contains("blue button"));
        assert_eq!(&q.chronicle[..before.len()], before.as_slice());
    }

    #[test]
    fn physical_compaction_replaces_old_speech_but_keeps_history_facts() {
        let mut q = quest();
        q.record(
            1,
            Entry::Answered {
                character: who("mage"),
                content: "The plan is ready.".into(),
                pace: None,
            },
        )
        .record(
            2,
            Entry::Produced {
                artifact: Artifact {
                    kind: "file".into(),
                    reference: "plan.md".into(),
                    summary: "The plan".into(),
                },
            },
        )
        .record(
            3,
            Entry::Moved {
                to: QuestState::NeedsRevision,
                because: "review feedback".into(),
            },
        )
        .record(
            4,
            Entry::Said {
                content: "Please revise the final section.".into(),
                attachments: Vec::new(),
                images: Vec::new(),
            },
        )
        .record(
            5,
            Entry::Answered {
                character: who("mage"),
                content: "I am revising it now.".into(),
                pace: None,
            },
        );

        assert_eq!(
            q.compact(
                6,
                who("mage"),
                "The plan exists; review requested one revision.".into(),
                2,
            ),
            // One more than before: every Chronicle now opens with the intent *and* the stage
            // that began with it.
            Some(5)
        );
        assert_eq!(q.chronicle.len(), 2, "only the literal tail remains");
        let memory = q.memory.as_ref().expect("durable memory");
        assert_eq!(memory.covers_through, 5);
        assert!(memory.summary.contains("plan"));
        assert_eq!(q.participants(), vec![who("mage")]);
        assert_eq!(q.artifacts().len(), 1);
        assert!(q.produced_evidence());
        assert_eq!(q.revisions(), 1);
        assert_eq!(q.last_contributor(), Some(&who("mage")));
    }

    #[test]
    fn an_impossible_compaction_never_hides_history() {
        let mut q = quest();
        q.record(
            2,
            Entry::Compacted {
                through: 99,
                character: who("mage"),
                summary: "This cannot cover future history.".into(),
            },
        );

        assert!(q.latest_compaction().is_none());
    }

    #[test]
    fn forgetting_an_agent_session_does_not_forget_the_quest() {
        let mut q = quest();
        q.continues(&who("mage"), "session-that-has-grown");
        let chronicle = q.chronicle.clone();

        assert_eq!(
            q.forget_session(&who("mage")),
            Some("session-that-has-grown".into())
        );
        assert!(q.session(&who("mage")).is_none());
        assert_eq!(q.chronicle, chronicle, "a session handle is not the record");
    }
}
