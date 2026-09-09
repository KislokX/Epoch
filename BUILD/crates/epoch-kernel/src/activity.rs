//! Something that happened, said once and never asked about again (ADR-0015).
//!
//! ## Commands cause; Activities record
//!
//! Two kinds of interaction, and the difference is whether anybody is waiting. A **Command** is
//! synchronous because the caller needs the answer — the Composer asking a Provider for a turn,
//! an Instance asking the Execution Engine to run a tool. An **Activity** is fire-and-forget:
//! *this happened*, emitted alongside the Command, to whoever cares.
//!
//! ## The hard rule, and why this type is shaped by it
//!
//! **No subsystem may depend on an Activity existing after emission.** Repositories remain the
//! source of truth (ADR-0014), and persistence never subscribes. That is what keeps this cheap:
//! delivery may be lost, order across emitters is not promised, and nothing breaks — because
//! anything that would break has a repository to read instead.
//!
//! So an Activity carries **what a consumer needs to filter and display**, and deliberately not
//! enough to reconstruct state from. A payload is a sentence and a few ids, never a diff.
//!
//! ## Why it lives in the Kernel
//!
//! Every subsystem emits them and several consume them, so the shape has to be one shape. Zero
//! I/O here, like everything else: these are the words, not the bus.
//!
//! ## Token streaming is not here
//!
//! Deliberately (ADR-0015, clarified 2026-07-25). Tokens are the *response to a Command* and
//! travel on `StreamingEvent`; fanning hundreds of per-token events to every subscriber would
//! make the hottest path in the product the most expensive one. The stream carries coarse,
//! turn-level facts.

use serde::{Deserialize, Serialize};

/// Who an Activity is for.
///
/// **Not a permission.** It is a filter: `Internal` says *this is machinery*, and a surface for
/// people should not draw it. It is also what stops derivation recursing — a consumer that also
/// emits marks its own emissions `Internal` and excludes them from its input (ADR-0015, "no
/// self-derivation"). Derivation flows one way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Visibility {
    /// Something a person would recognise as having happened.
    #[default]
    Public,
    /// Machinery. Real, and not news.
    Internal,
}

/// How much an Activity matters, for a consumer deciding what to keep or show.
///
/// Ordered worst-last on purpose, so `>=` reads the way a filter is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Importance {
    /// Only useful when somebody is looking for a fault.
    Debug,
    /// The ordinary run of things.
    #[default]
    Normal,
    /// Something went wrong, or something irreversible happened.
    Critical,
}

/// What kind of thing happened.
///
/// **A closed set, and short on purpose.** The ADR names the vocabulary; adding to it is a
/// decision about what the product observes, not a convenience. An open `String` here would
/// make every consumer's filter a guess about spelling.
///
/// Each one is a *coarse, turn-level fact*. `ProviderRequestStarted` and `Completed` are two
/// Activities; the tokens in between are not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ActivityKind {
    /// A character began a turn.
    TurnStarted,
    /// A turn was composed — how much context it needed, and what was left out.
    ContextComposed,
    /// A model was asked, and answered or did not.
    ProviderRequestStarted,
    ProviderRequestCompleted,
    /// A capability ran, or was refused.
    ToolExecutionStarted,
    ToolExecutionCompleted,
    /// Trust answered a question about what somebody may do.
    TrustDecisionMade,
    /// A Quest changed state.
    QuestAdvanced,
    /// Knowledge derived something from the stream. Always `Internal` — see the module note.
    KnowledgeCreated,
    /// A machine, a runtime or an agent became reachable or stopped being.
    ReachabilityChanged,
}

impl ActivityKind {
    /// The word a person reads.
    pub const fn label(self) -> &'static str {
        match self {
            ActivityKind::TurnStarted => "turn started",
            ActivityKind::ContextComposed => "context composed",
            ActivityKind::ProviderRequestStarted => "asked a model",
            ActivityKind::ProviderRequestCompleted => "a model answered",
            ActivityKind::ToolExecutionStarted => "started a tool",
            ActivityKind::ToolExecutionCompleted => "finished a tool",
            ActivityKind::TrustDecisionMade => "trust decided",
            ActivityKind::QuestAdvanced => "a Quest advanced",
            ActivityKind::KnowledgeCreated => "knowledge derived",
            ActivityKind::ReachabilityChanged => "reachability changed",
        }
    }
}

/// Which things an Activity is about.
///
/// Not `Scope`: that word is already Trust's, and it means *how far a permission reaches*
/// (`Scope::World`). Two meanings for one word in one Kernel is how a filter ends up written
/// against the wrong one.
///
/// **Ids, never the objects.** An Activity that carried a Quest would be an Activity somebody
/// could be tempted to read state out of, which is the failure the hard rule exists to prevent.
/// A consumer that needs the Quest asks the repository, which is the source of truth.
///
/// Every field is optional because most Activities are about some of these and none is about
/// all of them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Subject {
    #[serde(default)]
    pub world: Option<String>,
    #[serde(default)]
    pub quest: Option<String>,
    #[serde(default)]
    pub character: Option<String>,
}

impl Subject {
    pub fn world(id: impl Into<String>) -> Self {
        Self {
            world: Some(id.into()),
            ..Self::default()
        }
    }

    pub fn with_quest(mut self, id: impl Into<String>) -> Self {
        self.quest = Some(id.into());
        self
    }

    pub fn with_character(mut self, id: impl Into<String>) -> Self {
        self.character = Some(id.into());
        self
    }
}

/// One immutable observation.
///
/// Built with [`Activity::new`] and adjusted with the `with_*` methods, so a call site reads as
/// the sentence it is recording. Nothing here has a setter: it is immutable once emitted, which
/// is the only guarantee the stream makes about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    /// Unique, and generated at emission.
    pub id: String,
    pub kind: ActivityKind,
    /// Milliseconds, as everything else in the Kernel counts time.
    pub at: u64,
    #[serde(default)]
    pub subject: Subject,
    /// One line a person could read. Never a diff, never state.
    pub said: String,
    /// What this was part of, so a consumer can group a turn's Activities together.
    ///
    /// **The thing that makes an in-memory stream reconstructable enough** to be worth reading
    /// live, without making it a source of truth. Two Activities sharing a correlation belong to
    /// one piece of work.
    #[serde(default)]
    pub correlation: Option<String>,
    /// Which Activity caused this one, when one did.
    #[serde(default)]
    pub causation: Option<String>,
    #[serde(default)]
    pub visibility: Visibility,
    #[serde(default)]
    pub importance: Importance,
}

impl Activity {
    /// One observation, at a moment, about something.
    pub fn new(kind: ActivityKind, at: u64, said: impl Into<String>) -> Self {
        Self {
            // Not a UUID crate: the pair is unique within one process, which is the only place
            // an in-memory stream exists. A dependency for an id nobody stores would be a
            // dependency that has not earned itself.
            id: format!("{at}-{}", next()),
            kind,
            at,
            subject: Subject::default(),
            said: said.into(),
            correlation: None,
            causation: None,
            visibility: Visibility::Public,
            importance: Importance::Normal,
        }
    }

    pub fn about(mut self, subject: Subject) -> Self {
        self.subject = subject;
        self
    }

    /// Part of the same piece of work as something else.
    pub fn part_of(mut self, correlation: impl Into<String>) -> Self {
        self.correlation = Some(correlation.into());
        self
    }

    /// Caused by another Activity, named by its id.
    pub fn caused_by(mut self, activity: &Activity) -> Self {
        self.causation = Some(activity.id.clone());
        self.correlation = activity
            .correlation
            .clone()
            .or_else(|| Some(activity.id.clone()));
        self
    }

    /// Machinery rather than news.
    pub fn internal(mut self) -> Self {
        self.visibility = Visibility::Internal;
        self
    }

    pub fn at_importance(mut self, importance: Importance) -> Self {
        self.importance = importance;
        self
    }
}

/// A counter, so two Activities emitted in the same millisecond are still two.
fn next() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNT: AtomicU64 = AtomicU64::new(0);
    COUNT.fetch_add(1, Ordering::Relaxed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_things_in_the_same_millisecond_are_still_two_things() {
        // A timestamp is not an id. Turns emit several Activities in a burst, and a consumer
        // grouping by id would silently merge them.
        let a = Activity::new(ActivityKind::TurnStarted, 1, "Mage began");
        let b = Activity::new(ActivityKind::TurnStarted, 1, "Robo began");
        assert_ne!(a.id, b.id);
        assert_eq!(a.at, b.at);
    }

    #[test]
    fn what_caused_it_carries_the_work_it_belongs_to() {
        // Correlation is what makes a live reading worth having: two Activities sharing one
        // belong to a single piece of work, and a consumer can group a turn without asking any
        // repository anything.
        let turn = Activity::new(ActivityKind::TurnStarted, 1, "Mage began");
        let asked =
            Activity::new(ActivityKind::ProviderRequestStarted, 2, "asked gemma4").caused_by(&turn);
        let answered =
            Activity::new(ActivityKind::ProviderRequestCompleted, 3, "answered").caused_by(&asked);

        assert_eq!(asked.causation.as_deref(), Some(turn.id.as_str()));
        // The whole chain belongs to the turn, not to its immediate parent — otherwise the
        // third link would name the second and the group would fall apart at depth.
        assert_eq!(asked.correlation, answered.correlation);
        assert_eq!(answered.correlation.as_deref(), Some(turn.id.as_str()));
    }

    #[test]
    fn an_activity_carries_ids_and_never_the_things_themselves() {
        // The failure the hard rule exists to prevent: an Activity somebody could read state
        // out of. A consumer that needs the Quest asks the repository, which is the truth.
        let one = Activity::new(ActivityKind::QuestAdvanced, 7, "moved to Needs Revision").about(
            Subject::world("archipelago")
                .with_quest("q-1")
                .with_character("mage"),
        );

        let wire = serde_json::to_string(&one).unwrap();
        assert!(wire.contains("q-1"));
        for absent in ["messages", "conversation", "transcript", "content"] {
            assert!(!wire.contains(absent), "an Activity is not state: {wire}");
        }
    }

    #[test]
    fn internal_is_a_filter_and_not_a_permission() {
        // It is what stops derivation recursing: a consumer that also emits marks its own
        // emissions internal and excludes them from its input. Derivation flows one way.
        let derived =
            Activity::new(ActivityKind::KnowledgeCreated, 9, "noted a decision").internal();
        assert_eq!(derived.visibility, Visibility::Internal);
        assert_eq!(
            Activity::new(ActivityKind::TurnStarted, 9, "began").visibility,
            Visibility::Public,
            "public is the default, because most of what happens is news"
        );
    }

    #[test]
    fn importance_is_ordered_so_a_filter_reads_the_way_it_is_written() {
        assert!(Importance::Critical > Importance::Normal);
        assert!(Importance::Normal > Importance::Debug);
        assert_eq!(Importance::default(), Importance::Normal);
    }

    #[test]
    fn every_kind_says_itself_in_words_a_person_reads() {
        // A consumer for people must never have to spell an enum. The label is here rather than
        // in a surface because two surfaces would eventually disagree.
        for kind in [
            ActivityKind::TurnStarted,
            ActivityKind::ContextComposed,
            ActivityKind::ProviderRequestStarted,
            ActivityKind::ProviderRequestCompleted,
            ActivityKind::ToolExecutionStarted,
            ActivityKind::ToolExecutionCompleted,
            ActivityKind::TrustDecisionMade,
            ActivityKind::QuestAdvanced,
            ActivityKind::KnowledgeCreated,
            ActivityKind::ReachabilityChanged,
        ] {
            assert!(!kind.label().is_empty());
            assert!(!kind.label().contains('_'), "{}", kind.label());
        }
    }

    #[test]
    fn a_turn_full_of_tokens_is_two_activities_and_not_two_hundred() {
        // Deliberate (ADR-0015, clarified 2026-07-25). Tokens are the response to a Command and
        // travel on `StreamingEvent`; fanning per-token events to every subscriber would make
        // the hottest path the most expensive one. There is no `TokenStreamed`.
        let vocabulary = serde_json::to_string(&ActivityKind::ProviderRequestStarted).unwrap();
        assert_eq!(vocabulary, "\"providerRequestStarted\"");
        // The proof is that the word does not exist: this fails to compile if it is ever added
        // under a name a consumer could filter on.
        assert!(!format!("{:?}", ActivityKind::ProviderRequestCompleted).contains("Token"));
    }
}
