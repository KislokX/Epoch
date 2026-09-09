//! The World Simulation (ADR-0018).
//!
//! Maintains observable presence as a **deterministic projection of real engine state**.
//! It owns time; the UI only renders and interpolates.
//!
//! The causality rule: every presence transition traces to a real cause — an Activity, an
//! Instance state change, or a **declared routine rule**. Nothing here wanders. What it
//! does today is interpret authored idle behaviour against elapsed time, which is honest
//! information: the character is here, and available.
//!
//! ## Time, and who owns it
//!
//! The Simulation owns time (ADR-0018) and [`Simulation::advance`] is where it passes. The
//! shell supplies a heartbeat and nothing else: it decides *when to ask*, never what changed.
//! Every decision below is made here, and every one of them can be made in a test by handing
//! this function an instant — which is the whole reason travel is expressed as a departure time
//! and a duration rather than as a position that something has to keep nudging.
//!
//! ## The causality rule, as code rather than as a promise
//!
//! Nothing in this module can move somebody without a reason, because there is no method that
//! takes only a destination. There are exactly three ways a character's presence changes, and
//! they are the three ADR-0018 permits:
//!
//! - [`CharacterInstance::work_at`] and [`CharacterInstance::start_working`] — an Instance state
//!   change. Something is genuinely running.
//! - [`CharacterInstance::stop_working`] — the same, ending.
//! - [`Simulation::advance`] — a **declared routine rule**: an authored [`IdleBehavior`] that
//!   names a Place, carried out against the clock.
//!
//! There is no fourth. No wandering, no errands, no autonomous decisions — and no way to add
//! one by accident, because a caller has nothing to call.

use std::time::{Duration, Instant};

use epoch_kernel::{CharacterDefinition, Journey, PlaceId, PresenceState};

use crate::definition::DefinitionRegistry;
use crate::geography::Geography;
use crate::place::Mark;

/// How fast somebody walks, in world units per second.
///
/// **Measured against the World as it is drawn.** The shipped map is 3600x2200 and its two
/// furthest buildings are about 1750 units apart, so this is a walk of just under thirty
/// seconds — long enough to be a journey somebody notices and can watch, short enough that
/// asking for something never feels like waiting for a bus.
///
/// One speed for everybody, for now. Movement affinity is authored data in ADR-0018 and it can
/// live on the profile the day somebody has a reason for one character to be quicker than
/// another; inventing a difference before then would be the World making up a fact about a
/// person.
pub const WALKING_SPEED: f32 = 60.0;

/// The coarsest progress worth telling anybody about.
///
/// Publication is event-driven and deliberately not per frame (ADR-0018). The UI interpolates
/// between authoritative states using the speed and ETA it was given, so these updates exist to
/// *correct* it rather than to drive it: a tenth of the way is close enough that a correction is
/// never a visible jump, and rare enough that a walk costs a handful of messages instead of one
/// per tick.
const PROGRESS_STEP: f32 = 0.1;

/// How long the beat between arriving and beginning lasts.
///
/// **Chosen, not measured** — and saying so matters, because most numbers in this file are
/// measured and this one cannot be. It is long enough to read as somebody taking in a place at
/// sixty frames a second, and short enough that it is a twentieth of the longest walk in the
/// shipped World rather than something a person waits through.
///
/// It never delays work. The turn started when it started; this is presence catching up with
/// a figure that has just stopped moving (ADR-0018 — the Engine owns reality, and the beat is
/// part of that reality rather than a pause laid over it).
const BEAT: Duration = Duration::from_millis(1200);

/// A live character in the World.
///
/// The only stateful part of the triad (ADR-0011): the Definition is immutable, the runtime
/// is a stateless resolver, and this is the inhabitant.
#[derive(Debug, Clone)]
pub struct CharacterInstance {
    /// Snapshotted at spawn, so editing the file affects new instances only — running
    /// characters never mutate mid-turn (ADR-0011 hot-reload safety).
    definition: CharacterDefinition,
    /// Their face, already resolved. Snapshotted with the Definition for the same reason:
    /// a character does not change appearance halfway through being looked at.
    appearance: Option<Mark>,
    /// One sheet per action they have been drawn doing. Empty is complete: an action nobody
    /// drew falls back to the still sprite, and then to the visible stand-in (ADR-0016).
    actions: std::collections::BTreeMap<epoch_kernel::Action, Mark>,
    /// The image that identifies them, when they authored one of their own.
    icon: Option<Mark>,
    /// Where they live **in the World this Simulation is for** (ADR-0028).
    ///
    /// On the Instance rather than on the Definition because a `PlaceId` is local: the same
    /// character lives in a different building in every World, and most of those buildings do
    /// not exist in the others. Snapshotted at spawn like everything else here.
    home: epoch_kernel::PlaceId,
    /// **Where they actually are.** Distinct from `home`, and that is the point of this phase.
    ///
    /// Presence read `home` for every character at every moment, which was correct exactly as
    /// long as nobody could go anywhere: the two questions — *where do you live* and *where are
    /// you* — had one answer, so one field held both. The moment a character can walk they are
    /// different questions, and a surface that pointed at somebody's house while they were at
    /// the Library would be a World lying about where its people are.
    at: epoch_kernel::PlaceId,
    /// Where each of their routine activities happens **in this World** (ADR-0028).
    ///
    /// Resolved once at spawn, beside `home` and for the same reason: a `PlaceId` is local to a
    /// World, so this is a fact about somebody *here* rather than about who they are. Reading it
    /// per tick would ask the same question of the same two values several times a second.
    routine: std::collections::BTreeMap<String, PlaceId>,
    /// What they are doing right now.
    doing: Doing,
}

/// What an inhabitant is doing — routine, or real work.
///
/// The distinction is load-bearing and, until now, half-fictional: `ActivityClass::Work` has
/// existed since Presence was built and **has never once been true**, because nothing in Epoch
/// could work. This enum is where that changes.
///
/// Routine behaviour must never be dressed as work (Build From Life, rule 3), and the inverse
/// matters just as much: work must not look like routine, or the user cannot tell that they
/// asked for something and it is happening.
#[derive(Debug, Clone)]
enum Doing {
    /// Nothing was asked. Presence is derived from elapsed time against the authored routine.
    Idle { since: Instant },
    /// Something real is running. The activity is not authored — it describes actual work.
    ///
    /// `effort` is the difference between reasoning and executing, and it is **given** rather
    /// than read back out of `activity`: the caller knows which one it started, and a parser
    /// looking for the word "thinking" would be a second author of the same truth.
    Working { activity: String, effort: Effort },
    /// **Just arrived, and not yet doing the thing they came for.**
    ///
    /// The beat that stops a figure snapping from walking to working. `then` is what it was
    /// all for, exactly as it was on the walk — this changes when the purpose starts, never
    /// what it is.
    Arriving {
        beat: Beat,
        /// What this place is called, carried over from the walk that ended here.
        ///
        /// Taken at departure like `Leg::toward`, and for the same reason: a presence read has
        /// no geography, and asking it to find one would either widen its arguments or leave a
        /// surface joining an id back to a name.
        here: String,
        since: Instant,
        then: Box<Doing>,
    },
    /// On the way somewhere, and what they will do when they get there.
    ///
    /// `then` is what makes arrival mean something. A walk is never the point: somebody is
    /// going to the Library *to read*, or to the Command Center *to work*, and holding the
    /// destination without holding the purpose would leave the Simulation with a character
    /// standing in a doorway and nothing to do next.
    ///
    /// It also carries the class: travelling in order to work is work-class movement, and
    /// travelling because a routine said so is idle-class. The Design Guide requires those to
    /// be tellable apart, and this is where the difference is decided rather than guessed.
    Travelling { leg: Leg, then: Box<Doing> },
}

/// Reasoning, or running something.
///
/// A two-value type rather than an [`Action`](epoch_kernel::Action) parameter, because only two
/// of the four are things a caller may *start* — `Idle` and `Walk` are decided here, from the
/// routine and from a leg. Taking the wider enum would let a caller ask somebody to start
/// walking without a road, and then this file would have to decide what that meant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    /// The turn has started; no tool has run.
    Thinking,
    /// A tool, a command, a file — something is executing.
    Running,
    /// **Somebody else is doing it, and this character is waiting for them.**
    ///
    /// Added when drawing stopped blocking the turn (ADR-0034). The first reading of §5 was that
    /// a character with a job running should show as *working* — and the owner corrected it, in
    /// one sentence that is more precise than the ADR: Mage is not working, ComfyUI is.
    ///
    /// `IDLE` was wrong, because there is work in flight and the card said nothing about it.
    /// `WORKING` would be wrong in the other direction, because it credits the character with
    /// what a renderer on the same machine is doing. Neither is a small difference in a product
    /// whose rule is that presence never claims more than is true.
    Waiting,
}

impl Effort {
    fn action(self) -> epoch_kernel::Action {
        match self {
            Effort::Thinking => epoch_kernel::Action::Think,
            Effort::Running => epoch_kernel::Action::Work,
            // Waiting is not work, and it is not idle either. It draws as thinking because that
            // is the honest picture of somebody with nothing to do but wait for an answer.
            Effort::Waiting => epoch_kernel::Action::Think,
        }
    }
}

/// Whether one character is standing at a given Place, according to a snapshot.
///
/// Somebody who has *left* for that Place is not at it — presence keeps a traveller at the
/// Place they set out from until they arrive (ADR-0018), so this reads the same answer every
/// surface does rather than a second, more generous one.
fn stands_at(
    others: &[(epoch_kernel::CharacterId, PlaceId)],
    who: &epoch_kernel::CharacterId,
    place: &PlaceId,
) -> bool {
    others.iter().any(|(id, at)| id == who && at == place)
}

/// Change what somebody is *for* without disturbing where they are in getting there.
///
/// A walk in progress is never cancelled and a beat is never cut short — only their purpose
/// changes. Ending the walk would put somebody back where they set out from, which is the
/// teleport ADR-0018 names first; ending the beat would make a request look like a person
/// deciding to stop looking around.
fn keeping_the_arrival(current: Doing, purpose: Doing) -> Doing {
    match current {
        Doing::Travelling { leg, .. } => Doing::Travelling {
            leg,
            then: Box::new(purpose),
        },
        Doing::Arriving {
            beat, here, since, ..
        } => Doing::Arriving {
            beat,
            here,
            since,
            then: Box::new(purpose),
        },
        _ => purpose,
    }
}

/// Which arrival this was.
///
/// Two, and the difference between them is not decoration: it is whether somebody else is
/// standing here **because the work came from them**. Decided on arrival rather than on
/// departure, because the giver may have walked somewhere else while the receiver crossed the
/// map — and with nobody to arrive to, a handover is simply an arrival.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Beat {
    /// Arrived alone.
    Settle,
    /// Arrived, and the person the work came from is still here.
    Talk,
}

impl Beat {
    fn action(self) -> epoch_kernel::Action {
        match self {
            Beat::Settle => epoch_kernel::Action::Settle,
            Beat::Talk => epoch_kernel::Action::Talk,
        }
    }
}

/// One walk, from the moment it began.
///
/// A departure and a duration, never a position. A position would have to be advanced by
/// something, at some rate, and would drift differently depending on how often it was asked —
/// which is the difference between a deterministic projection and a thing that happens to be
/// running.
#[derive(Debug, Clone)]
struct Leg {
    from: PlaceId,
    to: PlaceId,
    /// What the destination is called, taken at departure.
    ///
    /// Composed here because here is where the World was in hand. A presence read has no
    /// geography and should not need one - asking it to look a title up would either widen its
    /// arguments or leave a surface joining an id back to a name, which is the same lookup
    /// written somewhere worse.
    toward: String,
    /// Who they are walking to meet, when the walk has one.
    ///
    /// Set only by a handover: the work came from somebody, and this is the somebody. Carried
    /// so arrival can ask whether they are still standing there — a question that cannot be
    /// answered at departure, and must not be guessed.
    meeting: Option<epoch_kernel::CharacterId>,
    /// Which way this walk points, measured once with the geography in hand.
    ///
    /// At departure rather than per read, for the same reason `toward` is taken here: a
    /// presence read has no geography, and asking it to find one would either widen its
    /// arguments or leave a surface working the answer out from coordinates.
    facing: epoch_kernel::Direction,
    started: Instant,
    /// How long the whole walk takes at [`WALKING_SPEED`].
    seconds: f32,
    /// How far along it was when anybody was last told. Coarse publication needs to know what
    /// the listener already believes.
    published: f32,
}

impl Leg {
    /// How far along, `0.0..=1.0`.
    fn progress(&self, now: Instant) -> f32 {
        if self.seconds <= 0.0 {
            return 1.0;
        }
        (now.saturating_duration_since(self.started).as_secs_f32() / self.seconds).clamp(0.0, 1.0)
    }
}

/// Something that happened to somebody's presence, worth telling a surface about.
///
/// The vocabulary is ADR-0018's, unchanged. These are **coarse and meaningful**: a walk of
/// thirty seconds produces a departure, about ten corrections and an arrival, rather than one
/// message per frame for something the UI can already draw.
#[derive(Debug, Clone, PartialEq)]
pub enum PresenceEvent {
    StartedTravelling {
        character: epoch_kernel::CharacterId,
        to: PlaceId,
    },
    ProgressUpdated {
        character: epoch_kernel::CharacterId,
        progress: f32,
    },
    Arrived {
        character: epoch_kernel::CharacterId,
        at: PlaceId,
    },
    StartedWorking {
        character: epoch_kernel::CharacterId,
    },
    BecameIdle {
        character: epoch_kernel::CharacterId,
    },
}

impl CharacterInstance {
    /// Resolve a Definition into a live inhabitant.
    ///
    /// This is the stateless Runtime step: same Definition in, same starting presence out.
    pub fn spawn(
        world_id: &str,
        definition: CharacterDefinition,
        appearance: Option<Mark>,
        icon: Option<Mark>,
        actions: std::collections::BTreeMap<epoch_kernel::Action, Mark>,
    ) -> Self {
        Self {
            // **Resolved once, here.** A `PlaceId` means something only inside one World, and
            // the Instance is the per-World runtime object (ADR-0011) — so it is the thing that
            // knows where its character lives. Resolving on every read would ask the same
            // question of the same two values a few times a second and get the same answer.
            at: definition.home_in(world_id),
            home: definition.home_in(world_id),
            routine: definition
                .presence
                .idle
                .iter()
                .filter_map(|behavior| {
                    let place = definition.routine_place_in(world_id, &behavior.activity)?;
                    Some((behavior.activity.clone(), place))
                })
                .collect(),
            definition,
            appearance,
            icon,
            actions,
            doing: Doing::Idle {
                since: Instant::now(),
            },
        }
    }

    /// Where this character lives **in this World**.
    pub fn home(&self) -> &epoch_kernel::PlaceId {
        &self.home
    }

    /// Put them to work on something real.
    ///
    /// The activity is a description of actual work, never an authored idle behaviour. Nothing
    /// may call this speculatively: the causality rule (ADR-0018) says every presence
    /// transition traces to a real cause, and "work" is the strongest claim presence can make.
    ///
    /// Work starts immediately, wherever they happen to be — including mid-stride. The user
    /// waiting on scenery would be scenery vetoing work.
    ///
    /// **A walk in progress is never cancelled; only its purpose changes.** The first version of
    /// this ended the walk and put them back at the Place they set out from, which is a teleport
    /// — the one thing ADR-0018 names first. Somebody halfway down the road keeps walking, and
    /// what waits at the other end is now work.
    pub fn start_working(&mut self, activity: impl Into<String>, effort: Effort) -> PresenceEvent {
        let working = Doing::Working {
            activity: activity.into(),
            effort,
        };
        self.doing =
            keeping_the_arrival(std::mem::replace(&mut self.doing, working.clone()), working);
        PresenceEvent::StartedWorking {
            character: self.definition.id.clone(),
        }
    }

    /// Walk somewhere in order to work there, and work when you arrive.
    ///
    /// The cause is the same as [`start_working`](Self::start_working) — something is genuinely
    /// running — and the movement is therefore work-class, which is what makes it look
    /// different from a character strolling to the Library on their routine.
    ///
    /// Returns `None` and starts the work immediately when there is nowhere to walk: the Place
    /// is not in this World, has never been positioned, or is where they already are. Scenery
    /// may not veto work (`places.rs`), so an unbuilt World gets on with it.
    pub fn work_at(
        &mut self,
        place: &PlaceId,
        activity: impl Into<String>,
        effort: Effort,
        meeting: Option<epoch_kernel::CharacterId>,
        world: &Geography,
        now: Instant,
    ) -> PresenceEvent {
        let working = Doing::Working {
            activity: activity.into(),
            effort,
        };

        // **Where the next walk starts from.** Somebody already on the road starts the new leg
        // from where that road ends, not from where they are standing — because where they are
        // standing is a point on a line, and the only thing a leg can begin at is a Place.
        //
        // So walks queue rather than replace: finish this one, then set out again. Cutting in
        // would mean either a leg beginning at a Place they have left, or a position this
        // contract cannot express — and both are the teleport ADR-0018 forbids.
        let from = match &self.doing {
            Doing::Travelling { leg, .. } => leg.to.clone(),
            _ => self.at.clone(),
        };

        match self.leg_from(&from, place, meeting, world, now) {
            Some(next) => {
                let to = next.to.clone();
                let onward = Doing::Travelling {
                    leg: next,
                    then: Box::new(working),
                };
                self.doing = match std::mem::replace(&mut self.doing, onward.clone()) {
                    Doing::Travelling { leg, .. } => Doing::Travelling {
                        leg,
                        then: Box::new(onward),
                    },
                    _ => onward,
                };
                PresenceEvent::StartedTravelling {
                    character: self.definition.id.clone(),
                    to,
                }
            }
            // Nowhere to walk, or already there. Work starts now, wherever they are — which may
            // be mid-stride, and that walk is not cancelled either.
            None => self.start_working_now(working),
        }
    }

    /// **The first tool has begun.** Reasoning becomes execution, in place.
    ///
    /// A promotion rather than a new state: the character is already working — the same turn,
    /// the same journey if they are on one — and what changed is that something is now running
    /// rather than being decided. The sentence changes with it, because *"reading src/main.rs"*
    /// is what a person watching wants and *"thinking with gemma4:12b"* has stopped being true.
    ///
    /// Does nothing to somebody who is idle. A tool running for a character who was never put
    /// to work would be a presence transition with no cause on this side of it, and the cause
    /// is the turn — which is what `start_working` already recorded.
    pub fn began_running(&mut self, activity: impl Into<String>) -> Option<PresenceEvent> {
        let running = |activity: String| Doing::Working {
            activity,
            effort: Effort::Running,
        };
        let activity = activity.into();
        match &mut self.doing {
            Doing::Working { .. } => {
                self.doing = running(activity);
            }
            // Mid-stride, or mid-arrival, and what waits at the other end is now a tool
            // rather than a thought. Neither the walk nor the beat is touched: ending the walk
            // would put them back where they set out from, which is the teleport ADR-0018
            // names first, and cutting the beat short would make a tool starting look like a
            // person deciding to stop looking around.
            Doing::Travelling { then, .. } | Doing::Arriving { then, .. }
                if matches!(**then, Doing::Working { .. }) =>
            {
                **then = running(activity);
            }
            _ => return None,
        }
        Some(PresenceEvent::StartedWorking {
            character: self.definition.id.clone(),
        })
    }

    /// Start work that needs no journey, keeping any journey already under way.
    fn start_working_now(&mut self, working: Doing) -> PresenceEvent {
        self.doing =
            keeping_the_arrival(std::mem::replace(&mut self.doing, working.clone()), working);
        PresenceEvent::StartedWorking {
            character: self.definition.id.clone(),
        }
    }

    /// Back to their routine. The idle clock restarts, so they resume at the top rather than
    /// wherever the cycle would have been had they never been interrupted — they just got back.
    ///
    /// They do **not** walk home. Going home is a routine decision, and the routine will make it
    /// on the next tick if the authored behaviour says so — sending them here would be this
    /// method inventing an errand, which is exactly the thing the causality rule forbids.
    ///
    /// A walk still under way survives, for the same reason it survives
    /// [`start_working`](Self::start_working): ending it would put them back where they set out
    /// from, and that is a teleport. The reason for the journey ended; the journey did not.
    /// Somebody else is doing the work now, and this character is waiting on them.
    ///
    /// Distinct from [`Self::began_running`] because the claim is different: that one says *this
    /// character is doing it*, and this says *this character asked for it and is waiting*. The
    /// sentence names what is being waited on, because a wait nobody can explain is the thing
    /// ADR-0034 lists as its own risk.
    pub fn began_waiting(&mut self, on: impl Into<String>) -> Option<PresenceEvent> {
        let waiting = Doing::Working {
            activity: on.into(),
            effort: Effort::Waiting,
        };
        match &self.doing {
            // Already waiting on the same thing: nothing changed, and an event would be noise.
            Doing::Working {
                effort: Effort::Waiting,
                activity,
            } if *activity
                == match &waiting {
                    Doing::Working { activity, .. } => activity.clone(),
                    _ => String::new(),
                } =>
            {
                None
            }
            _ => {
                self.doing = waiting;
                Some(PresenceEvent::StartedWorking {
                    character: self.definition.id.clone(),
                })
            }
        }
    }

    pub fn stop_working(&mut self) -> PresenceEvent {
        let idle = Doing::Idle {
            since: Instant::now(),
        };
        self.doing = keeping_the_arrival(std::mem::replace(&mut self.doing, idle.clone()), idle);
        PresenceEvent::BecameIdle {
            character: self.definition.id.clone(),
        }
    }

    /// Whether they are between two Places right now.
    ///
    /// Distinct from "where are they": somebody walking away from the Library still answers
    /// *the Library* to that question, and answers *no* to this one.
    fn on_the_road(&self) -> bool {
        matches!(self.doing, Doing::Travelling { .. })
    }

    pub fn is_working(&self) -> bool {
        match &self.doing {
            Doing::Working { .. } => true,
            // On the way to work is work, and so is the beat on arrival: the user asked for
            // something and it is under way the whole time.
            Doing::Travelling { then, .. } | Doing::Arriving { then, .. } => {
                matches!(**then, Doing::Working { .. })
            }
            Doing::Idle { .. } => false,
        }
    }

    /// Where they are, whether or not they are on their way somewhere else.
    pub fn at(&self) -> &epoch_kernel::PlaceId {
        &self.at
    }

    /// Plan a walk, or find there is no walk to make.
    ///
    /// `None` covers three different situations on purpose — already there, not a Place this
    /// World has, and a Place nobody has positioned — because every caller must do the same
    /// thing about all three, and a caller that could tell them apart would eventually treat
    /// one of them as an error the user has to fix.
    fn leg_to(&self, place: &PlaceId, world: &Geography, now: Instant) -> Option<Leg> {
        // A routine walk meets nobody: it is somebody going to read, not somebody being handed
        // work. Arriving alone is what makes `Settle` the honest beat for it.
        self.leg_from(&self.at, place, None, world, now)
    }

    /// The same, from somewhere other than where they are standing — the end of a walk they are
    /// still on.
    fn leg_from(
        &self,
        from: &PlaceId,
        place: &PlaceId,
        meeting: Option<epoch_kernel::CharacterId>,
        world: &Geography,
        now: Instant,
    ) -> Option<Leg> {
        if place == from {
            return None;
        }
        let distance = world.distance(from, place)?;
        Some(Leg {
            from: from.clone(),
            to: place.clone(),
            meeting,
            toward: world.title(place).unwrap_or(place.as_str()).to_owned(),
            // A positioned pair, because `distance` above already answered with one. South is
            // unreachable in practice and is the direction a character faces when they are
            // drawn facing the viewer — the least wrong answer if it ever were reached.
            facing: world
                .bearing(from, place)
                .unwrap_or(epoch_kernel::Direction::South),
            started: now,
            seconds: distance / WALKING_SPEED,
            // Nobody has been told anything about this walk yet, and departure is itself the
            // first thing they will hear.
            published: 0.0,
        })
    }

    pub fn definition(&self) -> &CharacterDefinition {
        &self.definition
    }

    /// Current presence.
    ///
    /// Working presence is not time-derived: it is a fact about something that is running, and
    /// it stays true until the work ends rather than advancing on a clock.
    pub fn presence(&self) -> PresenceState {
        self.presence_now(Instant::now())
    }

    /// Presence at a given instant. The clock is a parameter so behaviour over time can be
    /// asserted without a test sleeping through it.
    pub fn presence_now(&self, now: Instant) -> PresenceState {
        match &self.doing {
            Doing::Working { activity, effort } => PresenceState {
                character: self.definition.id.clone(),
                archetype: self.definition.archetype,
                place: self.at.clone(),
                activity: activity.clone(),
                action: effort.action(),
                // **Waiting is its own claim.** Something real is under way — it traces to a job,
                // so the causality rule is met — and this character is not the one doing it.
                class: match effort {
                    Effort::Waiting => epoch_kernel::ActivityClass::Waiting,
                    _ => epoch_kernel::ActivityClass::Work,
                },
                journey: None,
            },
            Doing::Idle { since } => {
                self.presence_at(now.saturating_duration_since(*since).as_secs())
            }
            // Standing where they arrived, doing the beat. **`class` still comes from what the
            // journey was for**, so a character who walked here to work is work-class through
            // the pause — the user asked for something and it is under way (Build From Life 3
            // is about never dressing routine as work, not about pretending work stopped).
            Doing::Arriving {
                beat, here, then, ..
            } => PresenceState {
                character: self.definition.id.clone(),
                archetype: self.definition.archetype,
                place: self.at.clone(),
                activity: match beat {
                    Beat::Settle => format!("arriving at {here}"),
                    Beat::Talk => "taking over the work".to_owned(),
                },
                action: beat.action(),
                class: match **then {
                    Doing::Working { .. } => epoch_kernel::ActivityClass::Work,
                    _ => epoch_kernel::ActivityClass::Idle,
                },
                journey: None,
            },
            Doing::Travelling { leg, then } => {
                let progress = leg.progress(now);
                PresenceState {
                    character: self.definition.id.clone(),
                    archetype: self.definition.archetype,
                    // Where they still are. They have left and not arrived, and a Place does
                    // not hold somebody who is on the road to it.
                    place: leg.from.clone(),
                    activity: format!("walking to {}", leg.toward),
                    // Walking is walking whatever waits at the far end. What the journey is
                    // *for* is `class`, one line down — the honesty rule — and it is not the
                    // animation.
                    action: epoch_kernel::Action::Walk,
                    // **The distinction the Design Guide requires.** Walking because work is
                    // waiting looks different from walking because a routine said so, and the
                    // difference is decided here rather than guessed by a renderer.
                    class: match **then {
                        Doing::Working { .. } => epoch_kernel::ActivityClass::Work,
                        _ => epoch_kernel::ActivityClass::Idle,
                    },
                    journey: Some(Journey {
                        from: leg.from.clone(),
                        to: leg.to.clone(),
                        progress,
                        speed: WALKING_SPEED,
                        eta_seconds: (leg.seconds * (1.0 - progress)).max(0.0),
                        facing: leg.facing,
                    }),
                }
            }
        }
    }

    /// Let time pass for this one character.
    ///
    /// Returns what a surface deserves to hear about, and `None` for the overwhelmingly common
    /// case where nothing worth saying happened. That is what keeps publication coarse: the
    /// heartbeat can run as often as it likes without turning into traffic.
    fn advance(
        &mut self,
        now: Instant,
        world: &Geography,
        // Where everybody else is standing, so an arrival can ask whether the person the work
        // came from is still here. A snapshot rather than a borrow of the Simulation: this
        // method is mutating one inhabitant while the question is about the others.
        others: &[(epoch_kernel::CharacterId, PlaceId)],
    ) -> Option<PresenceEvent> {
        /*
            **Which way they are facing, from the road rather than from the journey.**

            A leg's bearing is the straight line between two Places. What somebody walks is an
            authored road with corners, and the two disagree: measured on this World, the
            Command Center sits 320 units east of the Tower and 126 north of it, so the leg
            bears *east* — while the stretch of road in front of the Tower runs straight down. A
            character walking south down it was drawn facing east, which is the World saying
            something untrue about itself.

            Recomputed here rather than in the renderer because a facing is information
            (ADR-0018): the Engine owns which way somebody is going and the UI draws the row it
            is told. Here specifically, because this is the one method holding the clock and the
            geography together — and it is the heartbeat, so the turn stays as coarse as
            everything else presence publishes.

            Its own pass rather than a line inside the arm below, because that match reads
            `&self.doing` and turning somebody is a write. Falls back to the leg's own bearing
            when no road joins the two: a World with nothing authored between them really is a
            straight line.
        */
        if let Doing::Travelling { leg, .. } = &mut self.doing {
            let progress = leg.progress(now);
            if let Some(turned) = world.facing_along(&leg.from, &leg.to, progress) {
                leg.facing = turned;
            }
        }

        match &self.doing {
            // Work does not advance on a clock. It ends when the thing that is running ends,
            // and nothing here may decide that.
            Doing::Working { .. } => None,
            // The beat. It lands, and then they get on with what they came for.
            Doing::Arriving { since, then, .. } => {
                if now.saturating_duration_since(*since) < BEAT {
                    return None;
                }
                let next = (**then).clone();
                let event = match &next {
                    Doing::Working { .. } => PresenceEvent::StartedWorking {
                        character: self.definition.id.clone(),
                    },
                    // Back to their routine, which is where a routine walk was always going.
                    _ => PresenceEvent::BecameIdle {
                        character: self.definition.id.clone(),
                    },
                };
                self.doing = next;
                Some(event)
            }
            Doing::Travelling { leg, then } => {
                let progress = leg.progress(now);

                if progress >= 1.0 {
                    let arrived = leg.to.clone();
                    // What they came here to do. Arrival is not the end of anything on its own.
                    let mut next = (**then).clone();
                    // **A queued walk starts when it starts, not when it was planned.**
                    //
                    // Its departure time was set at the moment somebody asked for it, which was
                    // partway through the walk it is waiting behind — so by its own clock it
                    // would begin already half finished, and the figure would jump down the
                    // second road the instant they reached the end of the first.
                    if let Doing::Travelling { leg, .. } = &mut next {
                        leg.started = now;
                        leg.published = 0.0;
                        self.at = arrived.clone();
                        self.doing = next;
                        return Some(PresenceEvent::Arrived {
                            character: self.definition.id.clone(),
                            at: arrived,
                        });
                    }

                    // **Whether anybody is here to arrive to, asked now rather than earlier.**
                    //
                    // A handover walks the receiver to the last contributor's Place, and that
                    // contributor may have walked somewhere else while the receiver crossed the
                    // map. Checked at departure it would be a promise; checked here it is a
                    // fact — and with nobody standing there, a handover is simply an arrival.
                    let beat = match &leg.meeting {
                        Some(who) if stands_at(others, who, &arrived) => Beat::Talk,
                        _ => Beat::Settle,
                    };
                    let here = leg.toward.clone();
                    self.at = arrived.clone();
                    self.doing = Doing::Arriving {
                        beat,
                        here,
                        since: now,
                        then: Box::new(next),
                    };
                    return Some(PresenceEvent::Arrived {
                        character: self.definition.id.clone(),
                        at: arrived,
                    });
                }
                if progress - leg.published < PROGRESS_STEP {
                    return None;
                }
                if let Doing::Travelling { leg, .. } = &mut self.doing {
                    leg.published = progress;
                }
                Some(PresenceEvent::ProgressUpdated {
                    character: self.definition.id.clone(),
                    progress,
                })
            }
            Doing::Idle { since } => self.follow_routine(now, *since, world),
        }
    }

    /// Carry out the authored routine, which is the only reason an idle character ever moves.
    ///
    /// Every step traces to data somebody wrote: which behaviour is current comes from elapsed
    /// time against the authored cycle, and where it happens comes from the behaviour's own
    /// `at`. Nothing is chosen here.
    fn follow_routine(
        &mut self,
        now: Instant,
        since: Instant,
        world: &Geography,
    ) -> Option<PresenceEvent> {
        let elapsed = now.saturating_duration_since(since).as_secs();
        let profile = &self.definition.presence;
        let behavior = profile.idle_at(elapsed)?;
        // Unlisted means home, which is what every character described before a routine could
        // name a Place at all.
        let wanted = self
            .routine
            .get(&behavior.activity)
            .cloned()
            .unwrap_or_else(|| self.home.clone());
        if wanted == self.at {
            return None;
        }

        let leg = self.leg_to(&wanted, world, now)?;
        // A walk that would outlast the reason for making it does not begin (see
        // `PresenceProfile::idle_remaining_at`).
        //
        // **Except going home.** The guard exists so nobody sets out for a *destination
        // behaviour* they would have to abandon halfway. Home is not a destination behaviour —
        // it is where the routine lives, the place every behaviour happens unless one says
        // otherwise. Held to the same rule, somebody who walked across the map to take over a
        // Quest would be stranded there for good the moment their behaviours were shorter than
        // the walk back, which reads as broken rather than as careful.
        let going_home = wanted == self.home;
        if !going_home && leg.seconds > profile.idle_remaining_at(elapsed) as f32 {
            return None;
        }

        let to = leg.to.clone();
        self.doing = Doing::Travelling {
            leg,
            // **The same idle clock.** The cycle keeps running across the walk, so somebody who
            // sets out to read arrives still inside the behaviour that sent them - rather than
            // arriving to a routine that has been paused and restarted around a journey.
            then: Box::new(Doing::Idle { since }),
        };
        Some(PresenceEvent::StartedTravelling {
            character: self.definition.id.clone(),
            to,
        })
    }

    /// Presence at a given number of elapsed idle seconds. Exposed so behaviour over time
    /// can be asserted without sleeping in tests.
    pub fn presence_at(&self, elapsed_seconds: u64) -> PresenceState {
        let profile = &self.definition.presence;
        let activity = profile
            .idle_at(elapsed_seconds)
            .map(|b| b.activity.as_str())
            // The Registry rejects definitions with no idle behaviour, so this is
            // unreachable in practice. Presence must still exist rather than panic.
            .unwrap_or("present");

        PresenceState::idling(
            self.definition.id.clone(),
            self.definition.archetype,
            // Where they are - which is their home right up until the first time somebody walks
            // anywhere, and is a different question afterwards.
            self.at.clone(),
            activity,
        )
    }
}

/// One inhabitant as the World needs them: where they are, who they are, what they look like.
///
/// Presence alone is not enough to draw somebody, and never was — the missing half used to be
/// fetched from the active World Pack. It comes from the character now (ADR-0023), so it
/// travels with them instead of being looked up per World.
#[derive(Debug, Clone)]
pub struct CastMember {
    pub presence: PresenceState,
    pub name: String,
    /// What walks around the World, standing still.
    pub appearance: Option<Mark>,
    /// What they look like doing each thing they have been drawn doing.
    ///
    /// Beside the still picture rather than instead of it: the still one is what a renderer
    /// falls back to for every action that is not in here, which is most of them for most
    /// characters and all of them for a character with one drawing.
    pub actions: std::collections::BTreeMap<epoch_kernel::Action, Mark>,
    /// What identifies them where a face is what matters — a conversation, a roster. Falls
    /// back to the sprite, so one drawing makes somebody recognisable everywhere.
    pub icon: Option<Mark>,
    /// Which building is **theirs** in this World, as opposed to where they happen to be.
    ///
    /// Two different questions, and the editor is where the difference matters: assigning a
    /// residence changes where somebody returns to, not where they are standing right now. A
    /// surface that could only see `presence.place` would have to guess which one it was
    /// looking at, and would be wrong every time somebody was out.
    pub home: epoch_kernel::PlaceId,
    /// What they do when nothing is asked, and where each of those happens **here**.
    ///
    /// Both halves together because a surface that offers to change the second needs to show the
    /// first, and joining them anywhere else would mean a second place that knows how a routine
    /// is resolved (ADR-0028: the activity is theirs, the building is this World's).
    pub routine: Vec<(String, u32, Option<epoch_kernel::PlaceId>)>,
    /// Which voice they speak with — the name they authored. `None` is silence, and is the
    /// ordinary case.
    pub speaks_with: Option<String>,
    /// Which converted RVC voice colours it, and at what pitch. `None` is the Piper voice
    /// unaltered, which is the ordinary case even for somebody who speaks.
    pub sounds_like: Option<epoch_kernel::Timbre>,
}

/// Everyone currently living in the World.
#[derive(Debug, Default)]
pub struct Simulation {
    instances: Vec<CharacterInstance>,
    /// Where the Places are.
    ///
    /// Held rather than passed in per call, because *this* is the World these people are living
    /// in: a Simulation handed a different geography on alternate ticks would have characters
    /// walking distances that changed underneath them. It is replaced when the map is, which is
    /// the same moment the renderer's copy changes.
    world: Geography,
}

impl Simulation {
    /// Populate a World with the characters who live in it.
    ///
    /// The roster is read from the characters, not from the World: a World Pack is shipped
    /// content and cannot know who the user invented (ADR-0023). A World nobody lives in is
    /// valid, opens, and says it is empty.
    pub fn populate(definitions: &DefinitionRegistry, world_id: &str) -> Self {
        Self {
            instances: definitions
                .loaded()
                .filter(|l| l.definition.lives_in(world_id))
                .map(|l| {
                    CharacterInstance::spawn(
                        world_id,
                        l.definition.clone(),
                        l.appearance.clone(),
                        l.icon.clone(),
                        l.actions.clone(),
                    )
                })
                .collect(),
            // Filled in by `survey` once the World's map is composed. Populating and drawing are
            // two different moments, and the crew exists before the map is read.
            world: Geography::default(),
        }
    }

    /// Build directly from instances. Used where the cast is assembled by hand.
    pub fn of(instances: Vec<CharacterInstance>) -> Self {
        Self {
            instances,
            world: Geography::default(),
        }
    }

    /// Tell the Simulation what the World looks like.
    ///
    /// Called when a World is entered and whenever its map changes. A Simulation with no
    /// geography is not broken: nobody can walk anywhere, everybody stays where they are, and
    /// the World behaves exactly as it did before travel existed. That is the honest failure
    /// for a World that has not been drawn yet.
    pub fn survey(&mut self, world: Geography) {
        self.world = world;
    }

    /// Let time pass.
    ///
    /// **The one place the clock is read for everybody.** The shell calls this on a heartbeat
    /// and emits whatever comes back; it makes no decisions of its own, which is what keeps
    /// presence answerable without a window (ADR-0018: `where is the Researcher?` must work
    /// headless).
    ///
    /// An empty result is the normal case and means nothing worth publishing happened.
    pub fn advance(&mut self, now: Instant) -> Vec<PresenceEvent> {
        let world = &self.world;
        // Read where everybody is **before** anybody moves, so an arrival asks about a World
        // that is one consistent instant rather than one where the answer depends on who was
        // advanced first.
        //
        // **Only the people actually standing somewhere.** Presence keeps a traveller at the
        // Place they set out from until they arrive (ADR-0018), which is the honest answer to
        // *where is she* and the wrong answer to *is she here* — somebody halfway down the road
        // is not somebody you can arrive to, and drawing a `Talk` at them would be the World
        // showing a meeting that is not happening.
        let others: Vec<(epoch_kernel::CharacterId, PlaceId)> = self
            .instances
            .iter()
            .filter(|i| !i.on_the_road())
            .map(|i| (i.definition.id.clone(), i.at.clone()))
            .collect();
        self.instances
            .iter_mut()
            .filter_map(|instance| instance.advance(now, world, &others))
            .collect()
    }

    /// Send somebody to work somewhere, walking there first if the World has anywhere to walk.
    ///
    /// On the Simulation rather than on the Instance because the geography lives here, and a
    /// caller that had to fetch it to start a turn would eventually be handed the wrong one.
    ///
    /// `now` is a parameter for the same reason [`advance`](Self::advance) takes one: a
    /// subsystem that reads the clock in one place and is handed it in another cannot be
    /// asserted against a timeline. The first version called `Instant::now()` here, and a test
    /// that departed at `start` and arrived ten seconds later missed by the microseconds
    /// between the two reads.
    pub fn work_at(
        &mut self,
        who: &epoch_kernel::CharacterId,
        place: &PlaceId,
        activity: impl Into<String>,
        effort: Effort,
        // Who the work came from, when it came from somebody. The receiver walks to *them*,
        // so this is what makes an arrival a handover rather than a commute.
        meeting: Option<epoch_kernel::CharacterId>,
        now: Instant,
    ) -> Option<PresenceEvent> {
        let world = self.world.clone();
        let instance = self.get_mut(who)?;
        Some(instance.work_at(place, activity, effort, meeting, &world, now))
    }

    /// Current presence of every inhabitant.
    pub fn presences(&self) -> Vec<PresenceState> {
        self.instances.iter().map(|i| i.presence()).collect()
    }

    /// Everyone, ready to project.
    pub fn cast(&self) -> Vec<CastMember> {
        self.instances
            .iter()
            .map(|i| CastMember {
                presence: i.presence(),
                name: i.definition.name.clone(),
                appearance: i.appearance.clone(),
                actions: i.actions.clone(),
                icon: i.icon.clone().or_else(|| i.appearance.clone()),
                home: i.home.clone(),
                speaks_with: i.definition.speaks_with.clone(),
                sounds_like: i.definition.sounds_like.clone(),
                routine: i
                    .definition
                    .presence
                    .idle
                    .iter()
                    .map(|behavior| {
                        (
                            behavior.activity.clone(),
                            behavior.seconds,
                            i.routine.get(&behavior.activity).cloned(),
                        )
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn instances(&self) -> &[CharacterInstance] {
        &self.instances
    }

    /// Find somebody by identity, to change what they are doing.
    pub fn get_mut(&mut self, id: &epoch_kernel::CharacterId) -> Option<&mut CharacterInstance> {
        self.instances.iter_mut().find(|i| &i.definition.id == id)
    }

    pub fn get(&self, id: &epoch_kernel::CharacterId) -> Option<&CharacterInstance> {
        self.instances.iter().find(|i| &i.definition.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::{
        ActivityClass, CharacterArchetype, CharacterId, IdleBehavior, PresenceProfile,
    };
    use std::time::Duration;

    fn researcher() -> CharacterDefinition {
        CharacterDefinition {
            id: CharacterId::new("mage").unwrap(),
            name: "Mage".into(),
            archetype: CharacterArchetype::Researcher,
            role: "Turns goals into designs".into(),
            worlds: [("default".to_string(), Default::default())].into(),
            prompt: String::new(),
            skills: Default::default(),
            requested_capabilities: Default::default(),
            mind: None,
            appearance: None,
            draws_in: None,
            speaks_with: None,
            sounds_like: None,
            presence: PresenceProfile {
                authored_home: None,
                idle: vec![
                    IdleBehavior {
                        activity: "reading".into(),
                        seconds: 10,
                    },
                    IdleBehavior {
                        activity: "looking around".into(),
                        seconds: 4,
                    },
                ],
            },
        }
    }

    /// **Waiting is neither, and the owner said it in one sentence.**
    ///
    /// *"La tarjeta de Mage dice IDLE mientras su trabajo corre, porque no está trabajando él,
    /// está trabajando ComfyUI."* `Idle` is false — a job of theirs is in flight. `Work` is false
    /// the other way — it credits the character with what a renderer on the same machine is
    /// doing. This enum exists to keep a surface from claiming more than is true, so it needed a
    /// third value rather than a choice between two wrong ones.
    #[test]
    fn a_character_waiting_on_somebody_else_is_neither_idle_nor_working() {
        let mut instance =
            CharacterInstance::spawn("default", researcher(), None, None, Default::default());
        assert_eq!(instance.presence().class, ActivityClass::Idle);

        instance.began_waiting("waiting on ComfyUI at http://127.0.0.1:8188");
        let waiting = instance.presence();
        assert_eq!(waiting.class, ActivityClass::Waiting);
        assert!(
            waiting.activity.contains("ComfyUI"),
            "a wait nobody can explain is the risk ADR-0034 names: {}",
            waiting.activity
        );
        // It draws as thinking rather than as work: there is nothing of theirs to animate.
        assert_eq!(waiting.action, epoch_kernel::Action::Think);

        // And it is distinct from actually running something.
        instance.began_running("running cargo test");
        assert_eq!(instance.presence().class, ActivityClass::Work);

        instance.stop_working();
        assert_eq!(instance.presence().class, ActivityClass::Idle);
    }

    #[test]
    fn an_inhabitant_starts_at_home_doing_something() {
        let instance =
            CharacterInstance::spawn("default", researcher(), None, None, Default::default());
        let p = instance.presence_at(0);
        assert_eq!(p.place, epoch_kernel::PlaceId::new("research_lab"));
        assert_eq!(p.activity, "reading");
        assert_eq!(p.class, ActivityClass::Idle);
    }

    #[test]
    fn two_characters_can_live_in_two_buildings_of_the_same_kind() {
        // The capability this refactor exists for, and it was unreachable before (ADR-0028).
        //
        // Home was a `PlaceConcept`, so "laboratory" *was* the address: a World with two
        // laboratories had one home between them, and both researchers stood in whichever one
        // the renderer happened to match first. The classification was doing the work of the
        // identity — the same defect ADR-0023 named for characters, one field below the comment
        // explaining why it must never happen.
        let mut west = researcher();
        west.worlds.insert(
            "default".into(),
            epoch_kernel::Residence {
                home: Some(epoch_kernel::PlaceId::new("building_1")),
                at: Default::default(),
            },
        );

        let mut east = researcher();
        east.id = CharacterId::new("scholar").unwrap();
        east.worlds.insert(
            "default".into(),
            epoch_kernel::Residence {
                home: Some(epoch_kernel::PlaceId::new("building_4")),
                at: Default::default(),
            },
        );

        let a = CharacterInstance::spawn("default", west, None, None, Default::default())
            .presence_at(0);
        let b = CharacterInstance::spawn("default", east, None, None, Default::default())
            .presence_at(0);

        assert_ne!(a.place, b.place, "two homes, and they stayed two");
        assert_eq!(a.place, epoch_kernel::PlaceId::new("building_1"));
        assert_eq!(b.place, epoch_kernel::PlaceId::new("building_4"));

        // Same kind of worker, same kind of building, different buildings. Classification did
        // not collapse them.
        assert_eq!(a.archetype, b.archetype);
    }

    #[test]
    fn a_home_the_engine_has_never_heard_of_is_still_a_home() {
        // "The Forge" is not one of the five concepts and never will be. Under the old model it
        // could not be authored at all; the loader rejected it as "not a place this build
        // knows". The user creates Places now, so there is no such list to fail against.
        let mut smith = researcher();
        smith.worlds.insert(
            "default".into(),
            epoch_kernel::Residence {
                home: Some(epoch_kernel::PlaceId::new("the_forge")),
                at: Default::default(),
            },
        );

        let p = CharacterInstance::spawn("default", smith, None, None, Default::default())
            .presence_at(0);
        assert_eq!(p.place, epoch_kernel::PlaceId::new("the_forge"));
    }

    #[test]
    fn presence_changes_over_time_without_the_character_moving() {
        // She is never frozen: what she is doing advances even though she stays home.
        let instance =
            CharacterInstance::spawn("default", researcher(), None, None, Default::default());
        assert_eq!(instance.presence_at(2).activity, "reading");
        assert_eq!(instance.presence_at(11).activity, "looking around");
        assert_eq!(instance.presence_at(14).activity, "reading");
    }

    #[test]
    fn idle_presence_is_never_work_class() {
        // Routine behaviour must never imply work that is not happening.
        let instance =
            CharacterInstance::spawn("default", researcher(), None, None, Default::default());
        for t in [0u64, 5, 12, 30, 900] {
            assert_eq!(instance.presence_at(t).class, ActivityClass::Idle);
        }
    }

    #[test]
    fn the_world_can_be_populated_and_queried() {
        let sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            researcher(),
            None,
            None,
            Default::default(),
        )]);
        let presences = sim.presences();
        assert_eq!(presences.len(), 1);
        assert_eq!(presences[0].archetype, CharacterArchetype::Researcher);
        assert_eq!(presences[0].character.as_str(), "mage");
    }

    #[test]
    fn working_presence_is_work_class_and_says_what_is_actually_running() {
        // The first time `ActivityClass::Work` has ever been true. Until something could run,
        // nothing could honestly claim it.
        let mut instance =
            CharacterInstance::spawn("default", researcher(), None, None, Default::default());
        assert_eq!(instance.presence().class, ActivityClass::Idle);

        instance.start_working("thinking with qwen3:14b", Effort::Thinking);
        let p = instance.presence();
        assert_eq!(p.class, ActivityClass::Work);
        assert_eq!(p.activity, "thinking with qwen3:14b");

        // Work does not advance on a clock: it is a fact about something that is running.
        assert_eq!(instance.presence().activity, "thinking with qwen3:14b");

        instance.stop_working();
        assert_eq!(instance.presence().class, ActivityClass::Idle);
    }

    #[test]
    fn reasoning_becomes_execution_the_moment_a_tool_runs() {
        // The whole reason `Action::Think` exists: *nothing is happening*, *she is reasoning*
        // and *something is running* are three different facts, and until now the World could
        // only tell the first from the other two.
        let mut instance =
            CharacterInstance::spawn("default", researcher(), None, None, Default::default());
        assert_eq!(instance.presence().action, epoch_kernel::Action::Idle);

        instance.start_working("thinking with qwen3:14b", Effort::Thinking);
        assert_eq!(instance.presence().action, epoch_kernel::Action::Think);
        // Work-class from the first instant, because the user asked for something and it is
        // under way. The class and the action answer different questions.
        assert_eq!(instance.presence().class, ActivityClass::Work);

        instance.began_running("reading src/main.rs");
        let p = instance.presence();
        assert_eq!(p.action, epoch_kernel::Action::Work);
        // And the sentence goes with it: "thinking with qwen3:14b" stopped being true.
        assert_eq!(p.activity, "reading src/main.rs");

        instance.stop_working();
        assert_eq!(instance.presence().action, epoch_kernel::Action::Idle);
    }

    #[test]
    fn a_tool_running_for_somebody_idle_changes_nothing() {
        // A presence transition with no cause on this side of it. The cause of work is a turn
        // starting, and that is `start_working` — never a step arriving out of nowhere.
        let mut instance =
            CharacterInstance::spawn("default", researcher(), None, None, Default::default());
        assert!(instance.began_running("reading src/main.rs").is_none());
        assert_eq!(instance.presence().action, epoch_kernel::Action::Idle);
        assert!(!instance.is_working());
    }

    #[test]
    fn work_never_becomes_routine_by_waiting() {
        // The failure this guards: idle presence is derived from elapsed time, so a working
        // character whose clock kept running would silently drift back into her routine while
        // a real request was still in flight.
        let mut instance =
            CharacterInstance::spawn("default", researcher(), None, None, Default::default());
        instance.start_working("reading a repository", Effort::Running);
        for _ in 0..3 {
            assert_eq!(instance.presence().class, ActivityClass::Work);
            assert_eq!(instance.presence().activity, "reading a repository");
        }
        assert!(instance.is_working());
    }

    // ------------------------------------------------------------------ travel
    //
    // The contract ADR-0018 wrote in 2026-07 and nothing exercised until now. Every test below
    // is about one of its rules: characters never teleport, a character always has a current
    // place, movement traces to a cause, and idle movement is tellable from work.

    /// A World with two buildings a known distance apart.
    /// Somebody else, so a handover has two people in it.
    ///
    /// Their home is the Library, which is where the work is being handed over *from* in the
    /// tests below.
    fn librarian() -> CharacterDefinition {
        let mut who = researcher();
        who.id = CharacterId::new("paladin").unwrap();
        who.name = "Paladin".into();
        who.worlds = [(
            "default".to_string(),
            epoch_kernel::Residence {
                home: Some(PlaceId::new("library")),
                ..Default::default()
            },
        )]
        .into();
        who
    }

    #[test]
    fn arriving_to_somebody_is_a_different_beat_from_arriving_alone() {
        // The whole reason `Talk` exists, and the whole reason it is checked on arrival: two
        // characters standing together **because the work passed between them** is a measured
        // fact, and the giver may have walked off while the receiver crossed the map.
        let mut sim = Simulation::of(vec![
            CharacterInstance::spawn("default", researcher(), None, None, Default::default()),
            CharacterInstance::spawn("default", librarian(), None, None, Default::default()),
        ]);
        sim.survey(two_places());
        let start = Instant::now();

        // Mage is handed work that came from Paladin, who is standing in the Library.
        sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("library"),
            "thinking with qwen3:14b",
            Effort::Thinking,
            Some(CharacterId::new("paladin").unwrap()),
            start,
        );
        sim.advance(start + Duration::from_secs(10));

        let met = sim.instances()[0].presence_now(start + Duration::from_secs(10));
        assert_eq!(met.action, epoch_kernel::Action::Talk);
        // Work-class throughout: the turn started when it started, and the beat is presence
        // catching up with somebody who has just stopped moving.
        assert_eq!(met.class, ActivityClass::Work);

        // **A beat, never a state.** A `Talk` that persisted would imply a conversation in
        // progress, and no words pass between characters.
        sim.advance(start + Duration::from_secs(12));
        let after = sim.instances()[0].presence_now(start + Duration::from_secs(12));
        assert_eq!(after.action, epoch_kernel::Action::Think);
        assert_eq!(after.activity, "thinking with qwen3:14b");
    }

    #[test]
    fn arriving_to_somebody_who_has_gone_is_simply_arriving() {
        // Checked on arrival rather than on departure — which is the only place it *can* be
        // checked truthfully, because this is exactly the case that makes the two differ.
        let mut sim = Simulation::of(vec![
            CharacterInstance::spawn("default", researcher(), None, None, Default::default()),
            CharacterInstance::spawn("default", librarian(), None, None, Default::default()),
        ]);
        sim.survey(two_places());
        let start = Instant::now();

        sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("library"),
            "thinking with qwen3:14b",
            Effort::Thinking,
            Some(CharacterId::new("paladin").unwrap()),
            start,
        );
        // Paladin leaves for the Laboratory while Mage is on the road.
        sim.work_at(
            &CharacterId::new("paladin").unwrap(),
            &PlaceId::new("research_lab"),
            "working",
            Effort::Running,
            None,
            start + Duration::from_secs(1),
        );
        sim.advance(start + Duration::from_secs(10));

        let alone = sim.instances()[0].presence_now(start + Duration::from_secs(10));
        assert_eq!(alone.action, epoch_kernel::Action::Settle);
        assert_eq!(alone.activity, "arriving at The Library");
    }

    fn two_places() -> Geography {
        use crate::place::{Place, Placement};
        let at = |x: f32| Placement {
            x,
            y: 0.0,
            footprint: 90.0,
            z_order: 0,
        };
        let make = |id: &str, title: &str, x: f32| Place {
            id: PlaceId::new(id),
            concept: None,
            title: title.into(),
            subtitle: None,
            is_placeholder: false,
            placement: Some(at(x)),
            marks: Vec::new(),
            anchors: Vec::new(),
        };
        // 600 units apart: ten seconds at walking pace, which keeps the arithmetic in these
        // tests something a reader can check in their head.
        Geography::of(&[
            make("research_lab", "The Laboratory", 0.0),
            make("library", "The Library", 600.0),
        ])
    }

    /// Somebody whose routine takes them to the Library and back.
    ///
    /// Two halves, and they live in two places on purpose: the behaviours are theirs and travel
    /// into every World; *where* reading happens is a fact about this World (ADR-0028).
    fn commuter() -> CharacterDefinition {
        let mut who = researcher();
        who.presence.idle = vec![
            IdleBehavior {
                activity: "reading".into(),
                seconds: 60,
            },
            IdleBehavior {
                activity: "writing up".into(),
                seconds: 60,
            },
        ];
        who.worlds.insert(
            "default".into(),
            epoch_kernel::Residence {
                home: Some(PlaceId::new("research_lab")),
                at: [("reading".to_string(), PlaceId::new("library"))].into(),
            },
        );
        who
    }

    #[test]
    fn an_idle_character_walks_because_their_routine_says_so() {
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            commuter(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();

        // The first behaviour happens at the Library and she is at home, so she sets out. The
        // cause is a line somebody wrote in her file, which is what makes this legal at all.
        let events = sim.advance(start);
        assert_eq!(
            events,
            vec![PresenceEvent::StartedTravelling {
                character: CharacterId::new("mage").unwrap(),
                to: PlaceId::new("library"),
            }]
        );

        // On the road: still at the Laboratory, because she has left and not arrived. A Place
        // does not hold somebody who is walking towards it.
        let mid = sim.instances()[0].presence_now(start + Duration::from_secs(5));
        assert_eq!(mid.place, PlaceId::new("research_lab"));
        assert_eq!(mid.class, ActivityClass::Idle, "a routine walk is not work");
        let journey = mid.journey.expect("she is on her way");
        assert_eq!(journey.to, PlaceId::new("library"));
        assert!(
            (journey.progress - 0.5).abs() < 0.01,
            "half way at five seconds"
        );
        assert!((journey.eta_seconds - 5.0).abs() < 0.01);
        assert_eq!(journey.speed, WALKING_SPEED);
        // Said in the user's own words for their own building, never an id.
        assert_eq!(mid.activity, "walking to The Library");

        // Ten seconds is the whole walk. Arrival is an event, not something a progress bar
        // reaching the end lets a surface conclude on its own.
        let arrival = sim.advance(start + Duration::from_secs(10));
        assert_eq!(
            arrival,
            vec![PresenceEvent::Arrived {
                character: CharacterId::new("mage").unwrap(),
                at: PlaceId::new("library"),
            }]
        );

        // She is here, and taking the place in before she starts. The Library holds her from
        // the instant she arrives — the beat is about what she is doing, never about where.
        let landing = sim.instances()[0].presence_now(start + Duration::from_secs(10));
        assert_eq!(landing.place, PlaceId::new("library"));
        assert_eq!(landing.action, epoch_kernel::Action::Settle);
        assert_eq!(landing.class, ActivityClass::Idle);
        assert!(landing.journey.is_none());

        // And now the Library really does hold her, doing the thing that sent her.
        sim.advance(start + Duration::from_secs(12));
        let there = sim.instances()[0].presence_now(start + Duration::from_secs(12));
        assert_eq!(there.place, PlaceId::new("library"));
        assert_eq!(there.activity, "reading");
        assert_eq!(there.action, epoch_kernel::Action::Idle);
        assert!(there.journey.is_none());
    }

    #[test]
    fn nobody_sets_out_for_somewhere_they_would_have_to_leave_before_arriving() {
        // Ten seconds of walking against a six-second behaviour. Allowed, this is somebody who
        // turns around every time they get halfway - caused, derived, and unreadable.
        let mut brief = commuter();
        brief.presence.idle[0].seconds = 6;
        // Six seconds of reading against a ten-second walk to the Library.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            brief,
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());

        assert!(sim.advance(Instant::now()).is_empty(), "she stays put");
        assert_eq!(sim.instances()[0].at(), &PlaceId::new("research_lab"));
    }

    #[test]
    fn somebody_left_across_the_map_always_finds_their_way_home() {
        // The failure this exists to prevent, and it only appears when you follow the whole
        // cycle: a character walks somewhere to take over a Quest, finishes, and their routine
        // is made of behaviours shorter than the walk back. Under the "does it fit" rule they
        // would stay there for good.
        let mut brief = researcher();
        brief.presence.idle = vec![IdleBehavior {
            activity: "reading".into(),
            // Six seconds against a ten-second walk. Nothing fits.
            seconds: 6,
        }];
        // Home is the Laboratory and no activity names anywhere else, so the only walk her
        // routine can ever ask for is the walk back.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            brief,
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();

        // Sent to the Library to work, and finished there.
        sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("library"),
            "working",
            Effort::Thinking,
            None,
            start,
        );
        sim.advance(start + Duration::from_secs(10));
        // Past the arrival beat, so the routine is running again rather than paused on a
        // figure who has only just stopped moving.
        sim.advance(start + Duration::from_secs(12));
        sim.get_mut(&CharacterId::new("mage").unwrap())
            .expect("here")
            .stop_working();
        assert_eq!(sim.instances()[0].at(), &PlaceId::new("library"));

        // The very next tick sends her home, guard or no guard.
        let events = sim.advance(start + Duration::from_secs(13));
        assert_eq!(
            events,
            vec![PresenceEvent::StartedTravelling {
                character: CharacterId::new("mage").unwrap(),
                to: PlaceId::new("research_lab"),
            }]
        );

        // Ten seconds of walk from the thirteenth second, then the beat on the doorstep.
        sim.advance(start + Duration::from_secs(23));
        assert_eq!(sim.instances()[0].at(), &PlaceId::new("research_lab"));
        assert_eq!(
            sim.advance(start + Duration::from_secs(25)),
            vec![PresenceEvent::BecameIdle {
                character: CharacterId::new("mage").unwrap(),
            }]
        );
        // And having got home, she stays: there is nowhere else her routine names.
        assert!(sim.advance(start + Duration::from_secs(26)).is_empty());
    }

    #[test]
    fn a_world_nobody_has_drawn_yet_simply_has_nowhere_to_walk() {
        // No geography at all: the state every World is in before its map is composed, and the
        // state a half-built World stays in. Nothing moves, nothing panics, nobody is stranded
        // at the origin.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            commuter(),
            None,
            None,
            Default::default(),
        )]);

        assert!(sim.advance(Instant::now()).is_empty());
        let p = sim.presences().remove(0);
        assert_eq!(p.place, PlaceId::new("research_lab"));
        assert!(p.journey.is_none());
    }

    #[test]
    fn a_tool_starting_mid_stride_does_not_end_the_walk() {
        // Ending it would put her back where she set out from, which is the teleport ADR-0018
        // names first. The promotion changes what waits at the far end, never the road.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            researcher(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();
        sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("library"),
            "thinking with qwen3:14b",
            Effort::Thinking,
            None,
            start,
        );

        sim.get_mut(&CharacterId::new("mage").unwrap())
            .expect("here")
            .began_running("cloning whallet");

        let midway = sim.instances()[0].presence_now(start + Duration::from_secs(3));
        assert_eq!(midway.action, epoch_kernel::Action::Walk);
        let journey = midway.journey.expect("still on the road");
        assert_eq!(journey.from, PlaceId::new("research_lab"));
        assert_eq!(journey.to, PlaceId::new("library"));

        // And what she does once she has landed is the tool, not the thought it replaced.
        sim.advance(start + Duration::from_secs(10));
        sim.advance(start + Duration::from_secs(12));
        let there = sim.instances()[0].presence();
        assert_eq!(there.action, epoch_kernel::Action::Work);
        assert_eq!(there.activity, "cloning whallet");
    }

    #[test]
    fn walking_to_work_looks_different_from_walking_on_a_routine() {
        // The Design Guide's requirement, and the reason `class` is decided by what waits at
        // the other end rather than by the fact of walking.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            researcher(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();

        let event = sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("library"),
            "thinking with qwen3:14b",
            Effort::Thinking,
            None,
            start,
        );
        assert_eq!(
            event,
            Some(PresenceEvent::StartedTravelling {
                character: CharacterId::new("mage").unwrap(),
                to: PlaceId::new("library"),
            })
        );

        let on_the_way = sim.instances()[0].presence_now(start + Duration::from_secs(3));
        assert_eq!(on_the_way.class, ActivityClass::Work);
        // Work-class, and still **walking**: what she is going to do is not what to draw.
        assert_eq!(on_the_way.action, epoch_kernel::Action::Walk);
        assert!(on_the_way.is_travelling());
        // Already work, before she gets there: the user asked for something and it is under way.
        assert!(sim.instances()[0].is_working());

        sim.advance(start + Duration::from_secs(10));
        sim.advance(start + Duration::from_secs(12));
        let working = sim.instances()[0].presence_now(start + Duration::from_secs(12));
        assert_eq!(working.place, PlaceId::new("library"));
        assert_eq!(working.activity, "thinking with qwen3:14b");
        assert_eq!(working.class, ActivityClass::Work);
    }

    #[test]
    fn being_asked_something_halfway_is_answered_without_anybody_jumping() {
        // The teleport this exists to prevent, and it was in the first version of this file:
        // work started "where they are standing", which ended the walk and put them back at the
        // Place they set out from. On screen that is a figure halfway down the road vanishing
        // and reappearing at a door — the first thing ADR-0018 forbids.
        //
        // A walk in progress is never cancelled. Only its purpose changes.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            commuter(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();
        sim.advance(start);
        assert!(sim.instances()[0].presence_now(start).is_travelling());

        let event = sim
            .get_mut(&CharacterId::new("mage").unwrap())
            .expect("she is here")
            .start_working("thinking with qwen3:14b", Effort::Thinking);

        assert_eq!(
            event,
            PresenceEvent::StartedWorking {
                character: CharacterId::new("mage").unwrap()
            }
        );

        // Still walking, still on the same road, from the same door — and now the walk is work,
        // so the World draws it differently.
        let midway = sim.instances()[0].presence_now(start + Duration::from_secs(5));
        assert!(midway.is_travelling());
        assert_eq!(midway.place, PlaceId::new("research_lab"));
        assert_eq!(
            midway.journey.as_ref().map(|j| j.to.clone()),
            Some(PlaceId::new("library"))
        );
        assert_eq!(midway.class, ActivityClass::Work);

        // And what waits at the other end is the work.
        sim.advance(start + Duration::from_secs(10));
        sim.advance(start + Duration::from_secs(12));
        let there = sim.instances()[0].presence_now(start + Duration::from_secs(12));
        assert_eq!(there.place, PlaceId::new("library"));
        assert_eq!(there.activity, "thinking with qwen3:14b");
    }

    #[test]
    fn a_turn_that_ends_mid_walk_does_not_snatch_anybody_back() {
        // The same teleport, from the other side: a Quest that finishes before its contributor
        // arrives. The reason for the journey ended; the journey did not.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            researcher(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();
        sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("library"),
            "working",
            Effort::Thinking,
            None,
            start,
        );

        sim.get_mut(&CharacterId::new("mage").unwrap())
            .expect("here")
            .stop_working();

        let midway = sim.instances()[0].presence_now(start + Duration::from_secs(5));
        assert!(midway.is_travelling(), "she keeps walking");
        // Idle-class now: nothing is running, so the World must stop drawing this as work.
        assert_eq!(midway.class, ActivityClass::Idle);

        sim.advance(start + Duration::from_secs(10));
        assert_eq!(sim.instances()[0].at(), &PlaceId::new("library"));
    }

    #[test]
    fn a_second_destination_waits_for_the_first_to_be_reached() {
        // Somebody already on the road starts the next leg from where that road ends. Cutting in
        // would mean a leg beginning at a Place they have already left — a position this
        // contract cannot express, and a jump on screen.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            commuter(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();
        sim.advance(start);

        // Halfway to the Library, and asked to work at the Laboratory she just left.
        sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("research_lab"),
            "working",
            Effort::Thinking,
            None,
            start + Duration::from_secs(5),
        );

        // Still on the first road, unchanged.
        let midway = sim.instances()[0].presence_now(start + Duration::from_secs(6));
        let journey = midway.journey.expect("still walking");
        assert_eq!(journey.from, PlaceId::new("research_lab"));
        assert_eq!(journey.to, PlaceId::new("library"));

        // Arrives, and sets straight back off the other way.
        sim.advance(start + Duration::from_secs(10));
        let back = sim.instances()[0].presence_now(start + Duration::from_secs(11));
        let second = back.journey.expect("the second leg");
        assert_eq!(second.from, PlaceId::new("library"));
        assert_eq!(second.to, PlaceId::new("research_lab"));
        // **From the beginning of it.** This walk was planned five seconds into the previous
        // one; timed from then it would begin half finished, and the figure would jump down the
        // second road the moment they reached the end of the first.
        assert!(
            second.progress < 0.15,
            "the second walk starts when it starts, not when it was planned: {}",
            second.progress
        );

        sim.advance(start + Duration::from_secs(21));
        sim.advance(start + Duration::from_secs(23));
        let working = sim.instances()[0].presence_now(start + Duration::from_secs(23));
        assert_eq!(working.place, PlaceId::new("research_lab"));
        assert_eq!(working.activity, "working");
        assert_eq!(working.class, ActivityClass::Work);
    }

    #[test]
    fn a_place_this_world_does_not_have_never_strands_anybody() {
        // Scenery may not veto work. A Quest pointed at a building that does not exist here
        // gets on with it, in place, rather than waiting for a walk that can never happen.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            researcher(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());

        let event = sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("atlantis"),
            "reading a repository",
            Effort::Running,
            None,
            Instant::now(),
        );
        assert_eq!(
            event,
            Some(PresenceEvent::StartedWorking {
                character: CharacterId::new("mage").unwrap()
            })
        );
        let p = sim.instances()[0].presence();
        assert_eq!(p.place, PlaceId::new("research_lab"));
        assert_eq!(p.class, ActivityClass::Work);
    }

    #[test]
    fn a_walk_is_published_about_ten_times_rather_than_every_tick() {
        // Publication is coarse on purpose (ADR-0018). Ticked four times a second for the whole
        // ten-second walk, this is 40 opportunities to say something.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            commuter(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();

        let mut said = Vec::new();
        for quarter in 0..44u32 {
            said.extend(sim.advance(start + Duration::from_millis(u64::from(quarter) * 250)));
        }

        let departures = said
            .iter()
            .filter(|e| matches!(e, PresenceEvent::StartedTravelling { .. }))
            .count();
        let arrivals = said
            .iter()
            .filter(|e| matches!(e, PresenceEvent::Arrived { .. }))
            .count();
        let corrections = said
            .iter()
            .filter(|e| matches!(e, PresenceEvent::ProgressUpdated { .. }))
            .count();

        assert_eq!(departures, 1);
        assert_eq!(arrivals, 1);
        // A tenth at a time, and never one per tick.
        assert!(
            (8..=10).contains(&corrections),
            "about ten corrections, got {corrections}"
        );
    }

    #[test]
    fn stopping_work_does_not_send_anybody_home() {
        // Going home is a routine decision. Deciding it here would be this method inventing an
        // errand, which is the whole thing the causality rule exists to forbid - the routine
        // will do it on the next tick if the author said so.
        let mut sim = Simulation::of(vec![CharacterInstance::spawn(
            "default",
            researcher(),
            None,
            None,
            Default::default(),
        )]);
        sim.survey(two_places());
        let start = Instant::now();
        sim.work_at(
            &CharacterId::new("mage").unwrap(),
            &PlaceId::new("library"),
            "working",
            Effort::Thinking,
            None,
            start,
        );
        sim.advance(start + Duration::from_secs(10));
        assert_eq!(sim.instances()[0].at(), &PlaceId::new("library"));

        let event = sim
            .get_mut(&CharacterId::new("mage").unwrap())
            .expect("here")
            .stop_working();

        assert_eq!(
            event,
            PresenceEvent::BecameIdle {
                character: CharacterId::new("mage").unwrap()
            }
        );
        // Still at the Library. Nobody teleported home the instant the work ended.
        let p = sim.instances()[0].presence();
        assert_eq!(p.place, PlaceId::new("library"));
        assert_eq!(p.class, ActivityClass::Idle);
    }

    #[test]
    fn presence_is_addressed_by_who_rather_than_by_what_kind() {
        // Two researchers are two people. Keyed by archetype they would have been one.
        let mut second = researcher();
        second.id = CharacterId::new("paladin").unwrap();
        second.name = "Paladin".into();
        let sim = Simulation::of(vec![
            CharacterInstance::spawn("default", researcher(), None, None, Default::default()),
            CharacterInstance::spawn("default", second, None, None, Default::default()),
        ]);
        let cast = sim.cast();
        assert_eq!(cast.len(), 2);
        assert_ne!(cast[0].presence.character, cast[1].presence.character);
        assert_eq!(cast[0].presence.archetype, cast[1].presence.archetype);
    }
}
