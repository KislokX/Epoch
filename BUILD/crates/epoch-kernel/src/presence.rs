//! Presence — where a character is and what they are visibly doing (ADR-0018).
//!
//! Derived state, never authored. The engine owns it; the UI only renders and interpolates.
//! `PresenceProfile` (in [`crate::definition`]) is the authored side.
//!
//! ## Travel arrived here, and this is the shape it took
//!
//! ADR-0018 named six fields — current place, destination, progress, speed, ETA, activity —
//! and for three phases only two of them existed. The missing four are [`Journey`], gathered
//! into one optional value rather than spread across the state as four independent `Option`s
//! that could disagree: a destination with no progress, or an ETA for a journey nobody is on,
//! are states nothing should be able to represent.

use serde::{Deserialize, Serialize};

use crate::concept::{CharacterArchetype, Direction};
use crate::definition::CharacterId;
use crate::place::PlaceId;

/// Whether what a character is doing is their own routine, or real work.
///
/// This distinction is load-bearing: idle-class behaviour must be visually
/// distinguishable from work-class, or the World implies work that is not happening
/// (Build From Life, rule 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivityClass {
    /// Routine behaviour. True information: the character is here and available.
    Idle,
    /// Real execution. Only ever set when something is genuinely running.
    Work,
    /// **Real work is under way and this character is not the one doing it.**
    ///
    /// Added when drawing stopped blocking the turn (ADR-0034), and the distinction is the
    /// owner's: *Mage is not working, ComfyUI is.* `Idle` would be false — there is work in
    /// flight and it was this character who asked for it. `Work` would be false in the other
    /// direction — it credits the character with what a renderer on the same machine is doing.
    ///
    /// A third value rather than a flag, because the honesty rule this enum exists for is about
    /// what a surface is allowed to claim, and *waiting* is a different claim from both.
    Waiting,
}

/// **Which** action a character is performing, in a vocabulary a renderer can resolve.
///
/// ADR-0016 named `character.architect` + `walk.east` as canonical concepts on the day it was
/// written, and for four phases the Engine has only ever published the sentence beside this —
/// *"walking to The Library"*, *"thinking with gemma4:12b"*. No renderer can turn that into
/// frames, and every one of those strings is assembled by a `format!()` somewhere else.
///
/// So this is set at the call sites that already know, and **never parsed back out of the
/// sentence**. Deriving it from the words would make two authors of one truth, which is the
/// defect this project has now removed from four other places.
///
/// The sentence stays beside it. They answer different questions: this one is *what do I draw*,
/// and the sentence is *what do I tell a person who is reading*.
///
/// ## Why these, and not others
///
/// Each has a cause the Engine can point at, which is the causality rule (ADR-0018) applied to
/// the vocabulary itself rather than only to the transitions.
///
/// `Sleep` is deliberately absent. Nothing causes it — there is no night — and a sleeping
/// character with no night to explain them is precisely the autonomous NPC behaviour ADR-0018
/// forbids. It arrives when its cause does.
//
// `Ord`, because artwork is filed by action and a map needs an order. Declaration order is the
// order a character is drawn in: standing, walking, thinking, working.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Routine, available. Derived from elapsed time against an authored behaviour.
    Idle,
    /// Travelling. The direction falls out of the leg being walked, so it is not carried here.
    Walk,
    /// **Just arrived, alone.** Taking in the place before doing what they came to do.
    ///
    /// A beat, never a state: it lands and then becomes whatever the journey was for. Without
    /// it a figure snaps from walking to working, which reads as an animation switching rather
    /// than as somebody deciding.
    Settle,
    /// **Just arrived, and the person they came for is here.**
    ///
    /// The same beat with a second person in it, and the difference between them is the whole
    /// point: a handover walks the receiver to the last contributor's Place, so two characters
    /// standing together *because the work passed between them* is a measured fact.
    ///
    /// It is **not** a conversation. No words pass between characters and no surface may
    /// pretend otherwise — this is the arrival, and a `Talk` that persisted would imply a
    /// conversation in progress that is not happening.
    Talk,
    /// **The turn has started and no tool has run yet.**
    ///
    /// A real state with an explicit cause, and the difference between *nothing is happening*,
    /// *the character is reasoning* and *execution has begun*. It becomes [`Action::Work`] the
    /// moment the first tool call, file change or command begins.
    Think,
    /// Something is genuinely running — a tool, a command, a file being written.
    Work,
}

impl Action {
    pub const ALL: [Action; 6] = [
        Action::Idle,
        Action::Walk,
        Action::Settle,
        Action::Talk,
        Action::Think,
        Action::Work,
    ];

    /// The canonical, stable identifier — the first half of `walk.east`, and the key a
    /// character's artwork is filed under.
    pub const fn id(self) -> &'static str {
        match self {
            Action::Idle => "idle",
            Action::Walk => "walk",
            Action::Settle => "settle",
            Action::Talk => "talk",
            Action::Think => "think",
            Action::Work => "work",
        }
    }

    /// Parse a canonical id, or `None` if no such action exists. Fallible like every other
    /// vocabulary here: a word this build does not know costs one authored entry, never a file.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
}

/// Somebody on their way somewhere.
///
/// ## Why the UI is trusted with speed and an ETA
///
/// The Engine publishes coarsely — on departure, on arrival, and occasionally in between
/// (ADR-0018) — and the UI draws sixty frames a second between those. Without knowing how fast
/// somebody is walking, the only way to fill that gap is to guess, and a guess is a client
/// inventing reality.
///
/// With `progress`, `speed` and `eta_seconds` the UI is *deriving* the same answer the Engine
/// would give: it interpolates towards a state the Engine has already committed to. When the
/// next authoritative state arrives it wins, always, and the drawing corrects itself.
///
/// ## Nothing here is a promise about arrival
///
/// An ETA is the Engine's arithmetic on a distance and a speed it chose. If the World is
/// reloaded, or the character is put to work mid-journey, the journey ends where it is. That is
/// why arrival is an *event* and never something the UI concludes from a progress bar reaching
/// the end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Journey {
    /// Where they set out from. Kept because the route is the pair — a traveller drawn from
    /// wherever the camera first saw them would slide sideways on every reconnect.
    pub from: PlaceId,
    /// Where they are going.
    pub to: PlaceId,
    /// How far along, `0.0..=1.0`.
    pub progress: f32,
    /// World units per second. The unit the map is drawn in, so the UI needs no conversion.
    pub speed: f32,
    /// Seconds left at this speed.
    pub eta_seconds: f32,
    /// Which way they are walking.
    ///
    /// Here rather than in a renderer, for the same reason `speed` is: it is a fact about the
    /// World's geography, and a client that worked it out from the coordinates it happens to be
    /// drawing with would be deciding something about the world rather than drawing it.
    ///
    /// It lives on the journey because a journey is the only thing that has ever caused
    /// somebody to face one way rather than another. A standing character has no measured
    /// facing yet, and inventing one would be the same lie one layer down.
    pub facing: Direction,
}

/// A character's current presence, as the engine knows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresenceState {
    /// **Who** is present. Two characters may share an archetype, so presence has to be
    /// addressed by identity or two people collapse into one (ADR-0023).
    pub character: CharacterId,
    /// What kind of worker they are. Carried so the World can style by role without
    /// looking the character up again.
    pub archetype: CharacterArchetype,
    /// **Where** they are. A character always has a current place (ADR-0018).
    ///
    /// An identity, not a concept (ADR-0028). It used to be a `PlaceConcept`, which made
    /// "laboratory" the address — so two characters could not stand in two different
    /// laboratories, for exactly the reason the comment on `character` above gives about
    /// archetypes. The classification was doing the work of the identity.
    pub place: PlaceId,
    /// What they are doing, in their world's terms. The human sentence.
    pub activity: String,
    /// The same thing, in a closed vocabulary a renderer can resolve to frames.
    ///
    /// Beside `class` rather than instead of it: `class` answers *may this look like work*,
    /// which is a rule about honesty, and `action` answers *which animation*. Collapsing them
    /// would put a renderer in charge of a distinction Build From Life rule 3 makes load-bearing.
    pub action: Action,
    pub class: ActivityClass,
    /// Where they are going, when they are going somewhere.
    ///
    /// `place` above stays the Place they **left** for the whole journey. A character always
    /// has a current place (ADR-0018) and "between two buildings" is not one, so the honest
    /// answer to *where is she?* is where she still is until she gets there. It also keeps
    /// occupancy true: somebody walking to the Library is not in the Library, and a Place that
    /// counted them would be lit up for a person who has not arrived.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub journey: Option<Journey>,
}

impl PresenceState {
    /// Presence for a character who is at home, doing their own thing.
    pub fn idling(
        character: CharacterId,
        archetype: CharacterArchetype,
        place: PlaceId,
        activity: impl Into<String>,
    ) -> Self {
        Self {
            character,
            archetype,
            place,
            activity: activity.into(),
            action: Action::Idle,
            class: ActivityClass::Idle,
            journey: None,
        }
    }

    /// Whether they are on their way somewhere.
    pub fn is_travelling(&self) -> bool {
        self.journey.is_some()
    }

    /// Where they will be when they get there — which is where they already are, if they are
    /// not going anywhere.
    ///
    /// The question a surface actually asks when it wants to point at somebody's business:
    /// *which building does this person belong to right now?* Answering it here keeps three
    /// surfaces from each writing their own version of the same `match`.
    pub fn bound_for(&self) -> &PlaceId {
        match &self.journey {
            Some(journey) => &journey.to,
            None => &self.place,
        }
    }
}
