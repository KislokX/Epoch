//! The nervous system: emit, don't call (ADR-0015).
//!
//! ## What this is, and the four things it is not
//!
//! It is an **in-memory pub/sub of immutable Activities**. Subsystems say *this happened* and
//! carry on; whoever cares was listening.
//!
//! It is **not persistence**. Repositories remain the source of truth (ADR-0014), and
//! persistence never subscribes. It is **not event sourcing** — nothing is replayed and no state
//! is reconstructed from it. It is **not a queue**: delivery is synchronous, in-process, and a
//! subscriber that is slow makes its emitter slow, which is the honest arrangement for something
//! nobody may depend on. And it is **not a token channel** — streaming stays on `StreamingEvent`
//! (ADR-0015, clarified 2026-07-25).
//!
//! ## Why synchronous dispatch
//!
//! The ADR left it open: sync-dispatch or queued. Synchronous, because a queue is a thread, a
//! buffer and a backpressure policy for a delivery nothing is allowed to rely on — every one of
//! which would be complexity that has not earned itself. A subscriber's job is to *note* the
//! Activity, not to work on it; one that needs to work spawns its own thread and owns that
//! decision.
//!
//! The cost is real and worth naming: **an emitter is as slow as its slowest subscriber.** That
//! is a property somebody could regret, and the reason it is acceptable is the same rule that
//! makes the whole thing cheap — a subscriber that would block is a subscriber doing something
//! it should not be doing on this path.
//!
//! ## Why the emitter never fails
//!
//! [`ActivityStream::emit`] returns nothing. A subsystem that had to handle a failed emission
//! would be a subsystem depending on the Activity arriving, which is exactly the hard rule. A
//! poisoned lock is skipped rather than reported: losing an observation is allowed, and taking
//! down a turn over one is not.

use std::sync::{Arc, Mutex};

use epoch_kernel::{Activity, ActivityKind, Importance, Visibility};

/// Something that wants to know what happened.
///
/// A plain `Fn` rather than a trait, because every consumer so far is *one function that takes
/// an Activity* — the Knowledge Engine deriving, Observability recording, a surface pushing to
/// a window. A trait would be a vocabulary to agree on before anybody had a second method.
pub type Listener = Arc<dyn Fn(&Activity) + Send + Sync>;

/// What a subscriber is willing to hear.
///
/// **Filtering happens here rather than in the subscriber** for a reason the ADR gives: metadata
/// exists so consumers can filter. A stream where everybody hears everything and discards most
/// of it is one where the cost of adding an Activity kind is paid by every listener.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interest {
    /// Which kinds. Empty means all of them.
    pub kinds: Vec<ActivityKind>,
    /// The least important thing worth waking up for.
    pub at_least: Importance,
    /// Whether machinery counts as news for this listener.
    ///
    /// **This is the no-self-derivation rule, as a field.** A consumer that also emits marks its
    /// own emissions `Internal` and leaves this `false`, so derivation cannot recurse. Making it
    /// a filter rather than a convention means the rule is enforced by the thing that delivers.
    pub internal: bool,
}

impl Default for Interest {
    /// Everything a person would recognise as having happened.
    fn default() -> Self {
        Self {
            kinds: Vec::new(),
            at_least: Importance::Normal,
            internal: false,
        }
    }
}

impl Interest {
    /// Only these kinds.
    pub fn only(kinds: impl IntoIterator<Item = ActivityKind>) -> Self {
        Self {
            kinds: kinds.into_iter().collect(),
            ..Self::default()
        }
    }

    /// Machinery too — for Observability, which is the one consumer that wants everything.
    pub fn including_internal(mut self) -> Self {
        self.internal = true;
        self
    }

    pub fn from(mut self, at_least: Importance) -> Self {
        self.at_least = at_least;
        self
    }

    /// Whether this listener should hear it.
    pub fn wants(&self, activity: &Activity) -> bool {
        if activity.importance < self.at_least {
            return false;
        }
        if activity.visibility == Visibility::Internal && !self.internal {
            return false;
        }
        self.kinds.is_empty() || self.kinds.contains(&activity.kind)
    }
}

/// One subscription, kept so it can be dropped.
struct Subscription {
    interest: Interest,
    listen: Listener,
}

/// A durable, append-only record of Activities.
///
/// **DESIGN NOW, and deliberately unimplemented** (ADR-0015). It is named here so the shape a
/// future recorder must fit is fixed by something that compiles rather than by a paragraph — and
/// so the one property that matters is stated where somebody would go looking: a Recorder is a
/// *subscriber like any other*. Nothing may read from it to answer a question about the World.
/// The moment something does, the stream has become a source of truth and ADR-0014 is broken.
///
/// Left as a trait with no implementation because a log format and a retention policy are the
/// two open questions the ADR names, and answering them without a consumer would be inventing
/// requirements.
pub trait Recorder: Send + Sync {
    /// Write it down. Failure is the recorder's own business — an emitter never learns of it,
    /// for the same reason `emit` cannot fail.
    fn record(&self, activity: &Activity);
}

/// Where Activities are said.
///
/// Cloneable and cheap: it is an `Arc` around a list of subscribers, so a subsystem holds one
/// rather than being handed one per call.
#[derive(Clone, Default)]
pub struct ActivityStream {
    subscribers: Arc<Mutex<Vec<Subscription>>>,
}

impl ActivityStream {
    pub fn new() -> Self {
        Self::default()
    }

    /// Say that something happened.
    ///
    /// **Cannot fail, and returns nothing.** A caller that could handle a failure would be a
    /// caller depending on delivery. A poisoned lock is skipped: losing an observation is
    /// allowed by the hard rule, and taking down a turn over one is not.
    pub fn emit(&self, activity: Activity) {
        let Ok(subscribers) = self.subscribers.lock() else {
            return;
        };
        for subscriber in subscribers.iter() {
            if subscriber.interest.wants(&activity) {
                (subscriber.listen)(&activity);
            }
        }
    }

    /// Listen for some of it.
    ///
    /// Returns how many listeners there are, which is the only thing worth knowing: a stream
    /// with none is not an error, it is a Tuesday.
    pub fn subscribe(&self, interest: Interest, listen: Listener) -> usize {
        let Ok(mut subscribers) = self.subscribers.lock() else {
            return 0;
        };
        subscribers.push(Subscription { interest, listen });
        subscribers.len()
    }

    /// How many things are listening. For a surface that must say *nothing is watching* rather
    /// than imply it.
    pub fn listeners(&self) -> usize {
        self.subscribers.lock().map(|s| s.len()).unwrap_or(0)
    }
}

/// The last few Activities, for anything that wants to show what just happened.
///
/// ## Why a tail rather than a log
///
/// A log would be persistence, and persistence never subscribes. This is a **bounded window on
/// a transient stream**: it holds a fixed number of the most recent Activities, drops the oldest
/// without ceremony, and is empty on start because nothing has happened yet.
///
/// That is the honest shape for the one thing an in-memory stream is genuinely good for —
/// answering *what is going on right now* — and it cannot become a source of truth by accident,
/// because it visibly forgets.
pub struct Tail {
    kept: Arc<Mutex<Vec<Activity>>>,
    most: usize,
}

impl Tail {
    /// A tail that keeps at most `most` Activities.
    pub fn of(most: usize) -> Self {
        Self {
            kept: Arc::new(Mutex::new(Vec::new())),
            most: most.max(1),
        }
    }

    /// Attach it to a stream. Consumes nothing — the stream keeps its own reference.
    pub fn watching(&self, stream: &ActivityStream, interest: Interest) {
        let kept = Arc::clone(&self.kept);
        let most = self.most;
        stream.subscribe(
            interest,
            Arc::new(move |activity: &Activity| {
                let Ok(mut held) = kept.lock() else {
                    return;
                };
                held.push(activity.clone());
                // Oldest first out. `drain` rather than `remove` so a burst larger than the
                // window costs one shift instead of one per Activity.
                if held.len() > most {
                    let over = held.len() - most;
                    held.drain(..over);
                }
            }),
        );
    }

    /// What just happened, oldest first.
    pub fn recent(&self) -> Vec<Activity> {
        self.kept
            .lock()
            .map(|held| held.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::Subject;

    fn counting() -> (Listener, Arc<Mutex<Vec<String>>>) {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let into = Arc::clone(&heard);
        (
            Arc::new(move |a: &Activity| into.lock().unwrap().push(a.said.clone())),
            heard,
        )
    }

    #[test]
    fn nothing_listening_is_a_tuesday_and_not_an_error() {
        // The property that makes emission free to add anywhere: a subsystem never has to know
        // whether anybody cares.
        let stream = ActivityStream::new();
        assert_eq!(stream.listeners(), 0);
        stream.emit(Activity::new(ActivityKind::TurnStarted, 1, "Mage began"));
    }

    #[test]
    fn a_listener_hears_only_what_it_asked_for() {
        // Filtering lives in the delivery rather than in each subscriber, because a stream where
        // everybody hears everything makes adding a kind expensive for every listener.
        let stream = ActivityStream::new();
        let (listen, heard) = counting();
        stream.subscribe(
            Interest::only([ActivityKind::ToolExecutionCompleted]),
            listen,
        );

        stream.emit(Activity::new(ActivityKind::TurnStarted, 1, "began"));
        stream.emit(Activity::new(
            ActivityKind::ToolExecutionCompleted,
            2,
            "read a file",
        ));

        assert_eq!(*heard.lock().unwrap(), vec!["read a file".to_owned()]);
    }

    #[test]
    fn machinery_is_not_news_unless_somebody_asked_for_it() {
        // **The no-self-derivation rule, enforced by the thing that delivers.** A consumer that
        // also emits marks its own emissions internal; leaving `internal` false is what stops
        // derivation recursing, and making it a filter rather than a convention means nobody
        // has to remember.
        let stream = ActivityStream::new();
        let (people, for_people) = counting();
        let (everything, for_observability) = counting();
        stream.subscribe(Interest::default(), people);
        stream.subscribe(Interest::default().including_internal(), everything);

        stream.emit(Activity::new(ActivityKind::TurnStarted, 1, "began"));
        stream.emit(Activity::new(ActivityKind::KnowledgeCreated, 2, "derived").internal());

        assert_eq!(for_people.lock().unwrap().len(), 1);
        assert_eq!(for_observability.lock().unwrap().len(), 2);
    }

    #[test]
    fn debug_is_beneath_notice_until_somebody_is_looking_for_a_fault() {
        let stream = ActivityStream::new();
        let (ordinary, heard) = counting();
        let (digging, dug) = counting();
        stream.subscribe(Interest::default(), ordinary);
        stream.subscribe(Interest::default().from(Importance::Debug), digging);

        stream.emit(
            Activity::new(ActivityKind::ContextComposed, 1, "budget 8192")
                .at_importance(Importance::Debug),
        );
        stream.emit(
            Activity::new(ActivityKind::ToolExecutionCompleted, 2, "refused")
                .at_importance(Importance::Critical),
        );

        assert_eq!(*heard.lock().unwrap(), vec!["refused".to_owned()]);
        assert_eq!(dug.lock().unwrap().len(), 2);
    }

    #[test]
    fn the_tail_forgets_visibly_so_it_cannot_become_a_source_of_truth() {
        // A window on a transient stream, not a log. It drops the oldest without ceremony and
        // is empty on start, which is what stops anybody reading state out of it by accident.
        let stream = ActivityStream::new();
        let tail = Tail::of(3);
        tail.watching(&stream, Interest::default());
        assert!(tail.recent().is_empty(), "nothing has happened yet");

        for n in 1..=5u64 {
            stream.emit(Activity::new(
                ActivityKind::TurnStarted,
                n,
                format!("turn {n}"),
            ));
        }

        let recent: Vec<String> = tail.recent().into_iter().map(|a| a.said).collect();
        assert_eq!(
            recent,
            vec!["turn 3", "turn 4", "turn 5"],
            "oldest first out"
        );
    }

    #[test]
    fn a_turns_activities_can_be_grouped_without_asking_any_repository() {
        // What correlation is for. A live reading of *what is going on* has to be able to say
        // "these four things are one turn" from the stream alone — while still carrying only
        // ids, so nothing is tempted to read state out of it.
        let stream = ActivityStream::new();
        let tail = Tail::of(16);
        tail.watching(&stream, Interest::default());

        let turn = Activity::new(ActivityKind::TurnStarted, 1, "Mage began")
            .about(Subject::world("archipelago").with_character("mage"));
        let asked =
            Activity::new(ActivityKind::ProviderRequestStarted, 2, "asked gemma4").caused_by(&turn);
        let answered =
            Activity::new(ActivityKind::ProviderRequestCompleted, 3, "answered").caused_by(&asked);

        let group = turn.id.clone();
        stream.emit(turn);
        stream.emit(asked);
        stream.emit(answered);

        let together: Vec<Activity> = tail
            .recent()
            .into_iter()
            .filter(|a| a.correlation.as_deref() == Some(&group) || a.id == group)
            .collect();
        assert_eq!(together.len(), 3);
        assert_eq!(together[0].subject.character.as_deref(), Some("mage"));
    }

    #[test]
    fn emitting_cannot_fail_so_nothing_can_come_to_depend_on_delivery() {
        // The hard rule, as a signature. `emit` returns `()`; a caller that could handle a
        // failure would be a caller depending on the Activity arriving.
        let stream = ActivityStream::new();
        stream.subscribe(
            Interest::default(),
            // A listener that does nothing at all is a legitimate listener.
            Arc::new(|_| {}),
        );
        let nothing: () = stream.emit(Activity::new(ActivityKind::TurnStarted, 1, "began"));
        assert_eq!(nothing, ());
    }

    #[test]
    fn a_recorder_is_a_subscriber_like_any_other() {
        // DESIGN NOW: the trait exists so the shape is fixed by something that compiles. What
        // matters is that it attaches the same way everything else does — nothing may read from
        // a Recorder to answer a question about the World, or the stream has become the truth.
        struct Paper(Arc<Mutex<usize>>);
        impl Recorder for Paper {
            fn record(&self, _activity: &Activity) {
                *self.0.lock().unwrap() += 1;
            }
        }

        let written = Arc::new(Mutex::new(0));
        let paper: Arc<dyn Recorder> = Arc::new(Paper(Arc::clone(&written)));
        let stream = ActivityStream::new();
        stream.subscribe(
            Interest::default()
                .including_internal()
                .from(Importance::Debug),
            Arc::new(move |a: &Activity| paper.record(a)),
        );

        stream.emit(Activity::new(ActivityKind::TurnStarted, 1, "began"));
        stream.emit(Activity::new(ActivityKind::KnowledgeCreated, 2, "derived").internal());
        assert_eq!(*written.lock().unwrap(), 2, "a recorder hears everything");
    }
}
