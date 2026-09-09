//! Character Definitions — authored, immutable identity (ADR-0011).
//!
//! A Definition is **input**: hand-written data a person edits. It is not knowledge, not
//! state, and never mutated by the runtime. Multiple live Instances may be resolved from
//! one Definition with no shared state.
//!
//! Presence data lives in [`PresenceProfile`] rather than in the Definition body, keeping
//! identity and presence separate contracts (ADR-0018).
//!
//! ## Identity is not classification (ADR-0023)
//!
//! [`CharacterId`] says *who* somebody is; [`CharacterArchetype`] says *what kind of worker*
//! they are. Two characters may share an archetype and remain two different people — exactly
//! as two Places may share a concept. Addressing a character by archetype was only ever
//! workable while at most one existed per archetype, and it stopped being workable the moment
//! the cast belonged to the user rather than to a World.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::concept::CharacterArchetype;
use crate::mind::{Mind, RequestedCapabilities};
use crate::place::PlaceId;

/// Stable identity of a character.
///
/// Never derived from the name: renaming somebody must not make them a different person, and
/// nothing anywhere in Epoch may key on a display name.
///
/// Constrained to a file-safe, URL-safe alphabet because a character owns a folder in the
/// vault. Validating on construction is what makes every downstream use infallible.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CharacterId(String);

impl CharacterId {
    /// Accept an authored id, or reject it with the reason.
    pub fn new(raw: &str) -> Result<Self, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("a character id cannot be empty".into());
        }
        if let Some(bad) = trimmed
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_' || *c == '-'))
        {
            return Err(format!(
                "character id '{trimmed}' contains '{bad}'; use lowercase letters, digits, '_' or '-'"
            ));
        }
        Ok(Self(trimmed.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CharacterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CharacterId {
    type Error = String;
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        Self::new(&raw)
    }
}

impl<'de> Deserialize<'de> for CharacterId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(d)?;
        Self::new(&raw).map_err(serde::de::Error::custom)
    }
}

/// How a sheet of artwork is cut into frames, as authored.
///
/// One shape for both authors of artwork — a World Pack declaring an animated mark, and a
/// character declaring what they look like while they walk. Two shapes would have drifted the
/// first time somebody compared them, and they describe exactly the same thing.
///
/// It lives in the Kernel because it is data with no behaviour: no file is named here, nothing
/// is read, and nothing is drawn. Validation belongs to the Engine, where a problem can be
/// reported (`place::resolve_frames`).
///
/// **Every number defaults to `0`**, so *not stated* and *stated wrongly* are one case and both
/// get told. A default frame duration would be a constant nobody measured — whoever drew the
/// animation knows how fast it runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sheet {
    /// Cells across.
    #[serde(default)]
    pub columns: u32,
    /// Cells down.
    #[serde(default)]
    pub rows: u32,
    /// How many cells are real, in reading order from the top left. `0` means all of them —
    /// the common case, and the one where saying it again could only disagree.
    #[serde(default)]
    pub count: u32,
    /// How long one cell is shown.
    #[serde(default)]
    pub milliseconds: u32,
    /// Which direction each **row** walks, when the sheet is directional. Empty means it is
    /// not: one loop, whichever way somebody is facing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directions: Vec<String>,
}

/// One authored sheet, for one action.
///
/// The file and the cut together, because neither means anything alone: a sheet nobody can cut
/// draws as a strip, and a cut with no sheet describes nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionArt {
    /// Image file name, relative to the vault's characters folder. Never an absolute path.
    pub file: String,
    /// Flattened, so a person reading the file sees one table per action rather than a table
    /// inside a table for no reason they could name.
    #[serde(flatten)]
    pub cut: Sheet,
}

/// What a character looks like — authored, and owned by the character.
///
/// A path and two numbers: no I/O happens here, and the kernel never learns what an image
/// is. The Engine resolves this against the character's folder, using the *same* pipeline a
/// Place's artwork travels through. A character's appearance is not a second renderer.
///
/// This used to be supplied by the active World. It moved here with the cast (ADR-0023):
/// your crew looks like your crew in every World they visit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Numbers here are `f64` — TOML's own float — while everything that draws uses `f32`.
///
/// Not pedantry: this is a file a person reads and the Launcher rewrites. Held as `f32` and
/// serialised back, an authored `0.34` returns as `0.3400000035762787`, because that is
/// honestly what the nearest `f32` is. The value was never wrong; the *round trip* was lossy
/// in the direction the user could see. Authored data keeps the author's precision, and the
/// narrowing happens once, at Resolve, where the renderer needs it.
pub struct Appearance {
    /// Image file name, relative to the vault's characters folder. Never an absolute path.
    ///
    /// **The World's image**: what walks around, seen at the size a Place is. Usually drawn
    /// full-body and readable at a distance.
    ///
    /// Optional, and independently of the icon: somebody may be recognisable in a conversation
    /// long before anyone has drawn them walking. A character with neither is complete, not
    /// unfinished — the World already knows how to draw a visible stand-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sprite: Option<String>,
    /// The image that *identifies* them — in a conversation, in the roster, in a list.
    ///
    /// A different job from the sprite, and usually a different drawing: a face read at 44px
    /// is not a body read at world scale. Separate so improving one cannot ruin the other.
    ///
    /// `None` falls back to the sprite. Not a placeholder — a full-body sprite shrunk into a
    /// portrait box is a real, if imperfect, likeness of the right person, and that beats a
    /// blank. Inventing a face is what we never do (ADR-0024).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Size relative to a Place footprint, like any other mark.
    ///
    /// Defaults to [`CHARACTER_SCALE`] rather than to 1.0, so an imported sprite arrives the
    /// same height as everybody already standing in the World.
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// Origin within the image, in its own 0..1 space. `[0.5, 1.0]` is bottom-centre — the
    /// point somebody actually stands on.
    #[serde(default = "feet")]
    pub anchor: [f64; 2],
    /// One sheet per action they have been drawn doing, keyed by canonical [`Action`] id.
    ///
    /// **Additive, never required.** A character with one still drawing is complete: an action
    /// nobody drew falls back to [`Appearance::sprite`], and a character with neither falls
    /// back to the visible stand-in. That is ADR-0016's chain, unchanged — animation is
    /// something a World gains, not something it needs.
    ///
    /// Keyed by `String` rather than by the enum so an id this build does not know costs one
    /// entry and is reported, the same way an unknown mark role costs one mark. A file written
    /// by a later Epoch must still open in this one.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<String, ActionArt>,
}

/// How tall a person is, relative to a Place's footprint.
///
/// **Measured from the World as it stands**, not chosen: this is the scale the default pack's
/// sprite already renders at, and it is the size everything else was drawn to look right
/// beside. An import that arrived at `1.0` was almost three times taller than the crew — which
/// is what "assembled from several different games" looks like, and it was the whole reason a
/// sprite import felt broken.
///
/// **A scale, not a resize.** The alternative was decoding the file and re-encoding it smaller
/// on the way into the vault, and that permanently destroys a 1080×1080 drawing to solve a
/// problem that is one number. The Engine names an imported file from its bytes; it does not
/// rewrite them (ADR-0024).
///
/// Height only: the `<image>` fits inside a square box with `preserveAspectRatio`, so a wide
/// sprite stays wide and a tall one stays tall. Everybody is the same height; nobody is forced
/// into the same box.
pub const CHARACTER_SCALE: f64 = 0.34;

fn default_scale() -> f64 {
    CHARACTER_SCALE
}

fn feet() -> [f64; 2] {
    [0.5, 1.0]
}

/// One thing a character does while idle, and roughly how long they do it.
///
/// This is **behaviour, not animation** (Build From Life, rule 2). The engine holds the
/// activity; the World Pack decides what it looks like. An authored idle behaviour is why
/// a character reads for a while, then looks up — instead of standing frozen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdleBehavior {
    /// What the character is doing, in their own world's terms (e.g. "reading").
    pub activity: String,
    /// Roughly how long this lasts, in seconds. The simulation treats it as a hint.
    ///
    /// **Where** it happens is deliberately not here. See [`Residence::at`].
    pub seconds: u32,
}

/// Where a character lives and what they do when nothing is asked of them.
///
/// Authored data, interpreted by the World Simulation. Routine-driven idle behaviour is
/// legitimate under the causality rule: it conveys true information — that the character
/// is here, and available (ADR-0018).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresenceProfile {
    /// Cycled in order while idle. Must not be empty — a character is never frozen.
    ///
    /// Identity, and therefore here: how somebody passes the time is theirs and travels with
    /// them. Where they live is not — that is [`CharacterDefinition::worlds`] (ADR-0028).
    pub idle: Vec<IdleBehavior>,
    /// The home this file used to carry, read so it is not lost.
    ///
    /// **Read, never written.** Residences replaced it (ADR-0028), but every character in the
    /// user's vault still says `[presence] home = "command_center"` — and simply removing the
    /// field made serde ignore it, so a Guardian authored into the Command Center silently
    /// moved to wherever their *archetype* lives. The data was in the file and the new build
    /// threw it away.
    ///
    /// So it is read and used as the residence for every World the character lives in, until
    /// one says otherwise. `skip_serializing` means the next save writes residences and this
    /// disappears on its own — a migration that costs the user nothing and no step.
    #[serde(default, rename = "home", skip_serializing)]
    pub authored_home: Option<PlaceId>,
}

impl PresenceProfile {
    /// Total length of one full idle cycle, in seconds.
    pub fn cycle_seconds(&self) -> u32 {
        self.idle.iter().map(|b| b.seconds.max(1)).sum()
    }

    /// Which idle behaviour is current, given how long the character has been idle.
    ///
    /// Deterministic: the same elapsed time always yields the same behaviour. The
    /// simulation projects real elapsed time — it does not roll dice (ADR-0018 causality).
    /// How much longer the current idle behaviour lasts, in seconds.
    ///
    /// Exists for one decision, and it is a rule rather than a preference: **a character does
    /// not set out for somewhere they would have to leave before arriving.** Without it, a
    /// routine of ten-second behaviours across a thirty-second World produces somebody who
    /// turns around every time they get halfway — technically caused, honestly derived, and
    /// unreadable.
    ///
    /// A walk that does not fit simply does not happen, and the author can see why: lengthen
    /// the behaviour, or move the Place closer. Nothing is invented to paper over it.
    pub fn idle_remaining_at(&self, elapsed_seconds: u64) -> u64 {
        if self.idle.is_empty() {
            return 0;
        }
        let cycle = u64::from(self.cycle_seconds());
        let mut offset = elapsed_seconds % cycle;
        for behavior in &self.idle {
            let span = u64::from(behavior.seconds.max(1));
            if offset < span {
                return span - offset;
            }
            offset -= span;
        }
        0
    }

    pub fn idle_at(&self, elapsed_seconds: u64) -> Option<&IdleBehavior> {
        if self.idle.is_empty() {
            return None;
        }
        let cycle = u64::from(self.cycle_seconds());
        let mut offset = elapsed_seconds % cycle;
        for behavior in &self.idle {
            let span = u64::from(behavior.seconds.max(1));
            if offset < span {
                return Some(behavior);
            }
            offset -= span;
        }
        self.idle.last()
    }
}

/// Where a character lives in one particular World.
///
/// Its own type rather than a bare `PlaceId` because a residence will grow — a starting mood, a
/// desk inside the building, who they answer to here — and every one of those is a fact about
/// somebody *in a World*, not about who they are.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Residence {
    /// The Place they start in and return to, in this World.
    ///
    /// `None` means nobody has said. The Simulation falls back to the Place their archetype
    /// calls home, which is a defensible answer rather than a guess — and it is what every
    /// character had before residences existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub home: Option<PlaceId>,
    /// Where each of their routine activities happens **in this World**, by activity name.
    ///
    /// ## Why this is here and not on the behaviour
    ///
    /// It was on `IdleBehavior` first, and that was the mistake ADR-0028 already caught once:
    /// a `PlaceId` means something only inside one World. `library` is a different building in
    /// every World and in most of them does not exist at all — so a routine that named one
    /// would send the same character to the right door in one World and to a stranger's in
    /// another. `home` moved out of the profile for exactly this reason; this had to follow it.
    ///
    /// The split is the same one identity always takes here: *reading for sixty seconds* is who
    /// somebody is and travels with them; *reading happens at the Library* is a fact about a
    /// World, authored per World, and absent in a World where nobody has said.
    ///
    /// **This is what makes idle movement legal.** The causality rule forbids wandering — no
    /// errands, no emergent behaviour, nothing the Engine decided on its own. A character
    /// walking to the Library is not the Engine improvising; it is this map, which somebody
    /// wrote, being carried out. The cause is a declared routine rule, one of the three
    /// ADR-0018 permits.
    ///
    /// Anything unlisted happens at home, which is what every character described before this
    /// existed — a vault written for the old shape behaves identically under the new one. A
    /// Place this World does not have is not an error and not a teleport: there is nowhere to
    /// walk to, so nobody walks.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub at: std::collections::BTreeMap<String, PlaceId>,
}

/// Read `worlds`, accepting the shape it had before residences existed.
///
/// `worlds = ["archipelago", "default"]` is every character file the user already has. Refusing
/// it would make the new build unable to read the old one, and a vault the user has been
/// editing by hand is not something to invalidate over a field they never asked for.
///
/// A list becomes membership with no stated home, which is exactly what it meant.
fn residences<'de, D>(d: D) -> Result<std::collections::BTreeMap<String, Residence>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Authored {
        Named(std::collections::BTreeMap<String, Residence>),
        Listed(Vec<String>),
    }

    Ok(match Authored::deserialize(d)? {
        Authored::Named(map) => map,
        Authored::Listed(ids) => ids
            .into_iter()
            .map(|id| (id, Residence::default()))
            .collect(),
    })
}

/// A member of the user's crew, as authored.
///
/// Field order is load-bearing: this type is serialised back to the user's own file when they
/// edit a character, and TOML requires every table after every value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CharacterDefinition {
    /// Who this is. Stable forever; the file is named after it.
    pub id: CharacterId,
    /// What they are called. The user's to choose, and the same in every World (ADR-0023).
    pub name: String,
    /// What kind of worker they are. Classification, not identity — several characters may
    /// share one archetype.
    pub archetype: CharacterArchetype,
    /// One line on what this character is for.
    pub role: String,
    /// Which Worlds they live in, and where they live in each.
    ///
    /// The roster lives **here**, not in the World: a World Pack is shipped content and cannot
    /// know that you invented somebody. Moving a character between Worlds is therefore an edit
    /// to the character, which is also how the user thinks about it (ADR-0023).
    ///
    /// **A map rather than a list** (ADR-0028). It was `Vec<String>`, and the home a character
    /// returned to sat on their `PresenceProfile` — one home shared by every World they lived
    /// in. That was survivable only while a home was a `PlaceConcept`, because every World had
    /// exactly one laboratory. A `PlaceId` is local: `building_1` is a different building in
    /// every World, and in most of them it does not exist at all.
    ///
    /// So residence is per World, and it lives in the same field as membership. The obvious
    /// alternative — keep the list and add a parallel `homes` table — is two fields that must
    /// agree about who lives where, and eventually will not.
    ///
    /// Placed after every scalar field because TOML requires tables last, and this file is
    /// written back to the user's own vault.
    #[serde(default, deserialize_with = "residences")]
    pub worlds: std::collections::BTreeMap<String, Residence>,
    /// System prompt. Where the personality actually lives.
    #[serde(default)]
    pub prompt: String,
    /// Ways of working this character has been given (`crate::skill`).
    ///
    /// Ids rather than the methods themselves, because a Skill belongs to nobody: several
    /// characters may hold the same one, and copying its text into each of their files would
    /// make editing it a search-and-replace across the crew.
    ///
    /// **Beside the prompt, never inside it.** The prompt is who somebody is and a Skill is how
    /// a job is done — a character who loses a Skill is the same person with one less method,
    /// and that only stays true while the two are separate fields.
    ///
    /// A set, so the same Skill cannot be given twice and the order a turn reads them in does
    /// not depend on how they were typed.
    #[serde(default)]
    pub skills: BTreeSet<crate::skill::SkillId>,
    /// What this character asks to be able to use — **intent, never a claim** (ADR-0026).
    ///
    /// On the character rather than on the [`Mind`] on purpose: what somebody is for outlives
    /// who is currently thinking for them. Unassign the model and the requests stay; they are
    /// part of the job, not part of the engine.
    ///
    /// What is actually *available* is answered by the resolved Provider at runtime, never by
    /// this file (ADR-0005).
    ///
    /// **`None` means nobody has decided yet**, and resolves to everything this build can do
    /// ([`CharacterDefinition::wants`]). `Some(∅)` means somebody decided *none*, and is
    /// respected.
    ///
    /// The distinction matters because the obvious alternative — empty means all — makes
    /// unticking every box grant everything, which is the worst possible reading of a
    /// deliberate act. Absent and empty are different facts, so they are different values.
    ///
    /// Safe as a default because a capability is only ever *intent*: the Trust Engine still
    /// asks before anything with an effect runs (ADR-0009). Somebody who has not thought about
    /// this yet gets a crew that can try, and a gate that stops them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_capabilities: Option<RequestedCapabilities>,
    /// Who does their thinking. `None` means nobody assigned one — and they cannot think.
    ///
    /// Deliberately not defaulted. Picking a model for the user would surprise them with
    /// whatever it costs: on this machine the difference between two locally installed models
    /// was seconds versus minutes on a cold start. An unassigned character says so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mind: Option<Mind>,
    /// How they look. `None` is honest: the World draws a visible stand-in rather than
    /// inventing a face.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub appearance: Option<Appearance>,
    /// The style this character draws in when nobody says otherwise.
    ///
    /// ## A preference, never a cage
    ///
    /// *"If nobody says, I usually draw like this."* Not *"this is all I can do"* — the request
    /// wins, then this, then whatever the machine draws with (ADR-0030's amendment). A Historian
    /// who prefers old engravings still draws pixel art when somebody asks for pixel art.
    ///
    /// ## Why it is identity rather than tuning
    ///
    /// ADR-0026's one test: *does this survive changing the engine?* It does. A character who
    /// draws in watercolour still draws in watercolour after ComfyUI is replaced with something
    /// else — the same reason `temperature` is canonical and `num_ctx` is not.
    ///
    /// It is a **Style name**, never a workflow: a character naming a file would break the rule
    /// the Asset Resolver has held since ADR-0016, and would stop travelling between machines
    /// the moment it left this one.
    ///
    /// `None` is the ordinary case — most characters have no opinion, and inventing one for them
    /// would be Epoch deciding something nobody wrote down.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draws_in: Option<String>,
    /// Who this character sounds like when they speak out loud.
    ///
    /// ## A voice name, never a file
    ///
    /// `es_ES-davefx-medium`, the name its publisher gave it — the same shape as [`draws_in`],
    /// which holds a Style name and never a workflow. The rule is ADR-0016's and it has not
    /// moved: **the engine references concepts, never filenames.**
    ///
    /// It matters more here than it looks. A Piper voice happens to be one file today, and one
    /// Kokoro model holds **fifty-four** voices — a field that held a path would show that as a
    /// single entry called `kokoro-v1_0.pth`, and could not name any of the people in it. What a
    /// character has is a voice; which file carries it is the engine's problem.
    ///
    /// ## Why it is identity rather than a machine setting
    ///
    /// ADR-0026's one test: *does this survive changing the engine?* It does. Somebody who
    /// sounds like this still sounds like this after Piper is replaced — the same reason
    /// `temperature` is canonical and `num_ctx` is not, and the reason this travels in the
    /// Character Pack into every World (ADR-0026).
    ///
    /// ## `None` is silence, and it is the ordinary case
    ///
    /// Nobody speaks until somebody chooses a voice for them. Picking one would surprise a user
    /// with a stranger's voice for a character they wrote, which is the same objection that
    /// keeps [`mind`] undefaulted — and inventing a voice is a worse guess than inventing a
    /// model, because it is a claim about *who somebody is*.
    ///
    /// A name here whose voice is not installed is **not** an error: it is a preference the
    /// machine cannot honour yet, and it stays in the file so that installing the voice makes
    /// the character sound right again rather than making the user choose twice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaks_with: Option<String>,
    /// Whose voice this character is *coloured* into, on top of the one above.
    ///
    /// ## Why one field and not two
    ///
    /// The RVC model and the pitch it is spoken at are meaningless apart. A pitch with no model
    /// changes nothing — Piper has no such control — and a model at the wrong pitch is the same
    /// voice half an octave out, which everybody hears as a bad model rather than as a setting.
    /// **The pitch is a property of the pair**, so it lives inside the thing that names the pair.
    ///
    /// ## It is identity, by ADR-0026's one test
    ///
    /// *Does this survive changing the engine?* It does, twice over: somebody who sounds like
    /// this still sounds like this after Piper is replaced, and after ONNX Runtime is. So it
    /// travels in the Character Pack like every other part of who somebody is.
    ///
    /// `None` is the ordinary case and means *speak in the Piper voice, unaltered*. A name whose
    /// model is not installed is a preference the machine cannot honour yet, exactly like
    /// [`CharacterDefinition::speaks_with`] — the file keeps it so that installing the voice
    /// makes the character sound right again instead of making somebody choose twice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sounds_like: Option<Timbre>,
    pub presence: PresenceProfile,
}

/// A converted RVC voice, and the pitch this character is spoken at through it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Timbre {
    /// The voice's **name**, never a path — the same rule
    /// [`CharacterDefinition::speaks_with`] follows, and for the same reason: which file carries
    /// a voice is the engine's problem, and a model that holds several speakers cannot be named
    /// by its filename at all.
    pub voice: String,
    /// Semitones. `0.0` is the model's own register.
    ///
    /// Not optional and not hidden behind a default that means *nothing*: a male Piper voice
    /// through a female model needs about `+12`, and the owner's own measurement is the reason
    /// this exists at all — the Pato Donald model was inconclusive at `0` and unmistakable at
    /// `+12`. A pair with no pitch is a pair somebody cannot make work.
    #[serde(default)]
    pub semitones: f32,
    /// Which voice inside a model that holds several. `0` for almost every model.
    #[serde(default, skip_serializing_if = "is_first")]
    pub speaker: i64,
}

fn is_first(speaker: &i64) -> bool {
    *speaker == 0
}

impl CharacterDefinition {
    /// Whether this character currently lives in the given World.
    pub fn lives_in(&self, world_id: &str) -> bool {
        self.worlds.contains_key(world_id)
    }

    /// Which Place they call home **in this World**.
    ///
    /// Falls back to the Place their archetype calls home when the residence says nothing —
    /// which is every character authored before residences existed, and every character the
    /// user has not placed yet. A defensible default rather than a guess: it is what the
    /// archetype has always meant, and the World Editor is where it gets decided for real.
    ///
    /// Answers for a World this character does not live in too. That is deliberate: callers
    /// ask "where would you be here", and refusing would push a `lives_in` check into every
    /// one of them.
    pub fn home_in(&self, world_id: &str) -> PlaceId {
        self.worlds
            .get(world_id)
            .and_then(|r| r.home.clone())
            // What the file used to say, before residences existed. Ahead of the archetype
            // default because an authored fact beats a defensible guess.
            .or_else(|| self.presence.authored_home.clone())
            .unwrap_or_else(|| PlaceId::new(self.archetype.home_place().id()))
    }

    /// Where one routine activity happens in this World, if anybody has said.
    ///
    /// `None` means home — the answer for every activity in every World until somebody decides
    /// otherwise, and the reason a World with no travel in it is complete rather than
    /// unfinished.
    pub fn routine_place_in(&self, world_id: &str, activity: &str) -> Option<PlaceId> {
        self.worlds.get(world_id)?.at.get(activity).cloned()
    }

    /// What this character asks for, or `None` when nobody has decided.
    ///
    /// **Undecided still means everything available — resolved by the Engine, not here.** This
    /// used to answer with a hardcoded list of what the Kernel believed was built, which was
    /// wrong in a way nobody could see: a character nobody had configured could never reach a
    /// tool from a connected MCP server, because the Kernel cannot know one exists. The
    /// intention was right and only the Engine can carry it out.
    ///
    /// A character nobody has configured should be able to try, not be silently useless — and
    /// the Trust Engine is what stops them, not an empty list somebody forgot to fill in.
    pub fn wants(&self) -> Option<&RequestedCapabilities> {
        self.requested_capabilities.as_ref()
    }

    /// Whether anybody has decided yet. A surface shows the difference.
    pub fn capabilities_decided(&self) -> bool {
        self.requested_capabilities.is_some()
    }
}

#[cfg(test)]
mod appearance_tests {
    use super::*;

    #[test]
    fn an_imported_sprite_arrives_the_same_height_as_the_crew() {
        // Paladin was authored at 1.0 and Mage at 0.34, so Paladin rendered nearly three times
        // taller — and it looked like a problem with the *image*, which is 1080×1080 against
        // Mage's 18×32. It was not: the `<image>` already fits inside a square box with
        // `preserveAspectRatio`, so both draw to the same height and the height is the scale.
        //
        // The fix is a default, not a resize. Re-encoding the file on the way in would destroy
        // a 1080×1080 drawing permanently to correct one number.
        let a: Appearance =
            serde_json::from_value(serde_json::json!({ "sprite": "newcomer.png" })).unwrap();
        assert_eq!(a.scale, CHARACTER_SCALE);
    }

    #[test]
    fn a_scale_somebody_chose_is_left_alone() {
        // The default is for people who have not decided. Somebody who deliberately made a
        // character enormous meant it.
        let a: Appearance =
            serde_json::from_value(serde_json::json!({ "sprite": "giant.png", "scale": 2.0 }))
                .unwrap();
        assert_eq!(a.scale, 2.0);
    }
}

#[cfg(test)]
mod residence_tests {
    use super::*;

    /// JSON rather than TOML because the Kernel has no format dependency — zero I/O is the
    /// boundary the crate exists to hold. What is under test is the *serde* shape, which is
    /// the same one TOML goes through.
    fn read(worlds: serde_json::Value) -> CharacterDefinition {
        serde_json::from_value(serde_json::json!({
            "id": "mage",
            "name": "Mage",
            "archetype": "researcher",
            "role": "Turns goals into designs",
            "worlds": worlds,
            "prompt": "You explore before committing.",
            "presence": { "idle": [{ "activity": "reading", "seconds": 10 }] },
        }))
        .expect("a character the vault could already hold")
    }

    #[test]
    fn the_shape_every_existing_character_file_uses_still_loads() {
        // `worlds = ["a", "b"]` is what is in the user's vault right now. A build that could
        // not read it would invalidate a folder they have been editing by hand, over a field
        // they never asked for.
        let d = read(serde_json::json!(["archipelago", "default"]));

        assert!(d.lives_in("archipelago"));
        assert!(d.lives_in("default"));
        assert!(!d.lives_in("frontier"));
    }

    #[test]
    fn a_character_lives_somewhere_different_in_each_world() {
        // The point of the change. One `home` on the character meant one home for every World
        // they lived in — and a `PlaceId` is local, so that home was wrong everywhere but one.
        let d = read(serde_json::json!({
            "archipelago": { "home": "building_1" },
            "frontier": { "home": "the_forge" },
        }));

        assert_eq!(d.home_in("archipelago"), PlaceId::new("building_1"));
        assert_eq!(d.home_in("frontier"), PlaceId::new("the_forge"));
    }

    #[test]
    fn saying_nothing_falls_back_to_what_the_archetype_has_always_meant() {
        // Every character authored before residences existed, and every character the user has
        // not placed yet. A defensible default, not a guess — and it is exactly where they
        // used to live.
        let d = read(serde_json::json!(["archipelago"]));
        assert_eq!(d.home_in("archipelago"), PlaceId::new("research_lab"));
    }

    #[test]
    fn the_home_already_written_in_the_users_file_is_not_lost() {
        // Found by using it: Paladin is authored into the Command Center and appeared in the
        // Library instead — where a Guardian's *archetype* lives. Removing the field made serde
        // ignore it, so a fact the user had written was replaced by a default.
        let d: CharacterDefinition = serde_json::from_value(serde_json::json!({
            "id": "paladin",
            "name": "Paladin",
            "archetype": "guardian",
            "role": "Keeps what already works working",
            "worlds": ["archipelago", "default"],
            "prompt": "",
            "presence": {
                "home": "command_center",
                "idle": [{ "activity": "checking the seals", "seconds": 12 }],
            },
        }))
        .expect("the file as it is on disk today");

        assert_eq!(d.home_in("archipelago"), PlaceId::new("command_center"));
        assert_eq!(d.home_in("default"), PlaceId::new("command_center"));
        assert_ne!(
            d.home_in("archipelago"),
            PlaceId::new(d.archetype.home_place().id()),
            "the archetype default is what this test exists to stop"
        );
    }

    #[test]
    fn a_residence_wins_over_the_home_the_file_used_to_carry() {
        // Once a World says where somebody lives, the old single home stops applying there —
        // otherwise placing a character in the editor would appear not to work.
        let d: CharacterDefinition = serde_json::from_value(serde_json::json!({
            "id": "paladin",
            "name": "Paladin",
            "archetype": "guardian",
            "role": "Keeps what already works working",
            "worlds": { "archipelago": { "home": "building_9" }, "default": {} },
            "prompt": "",
            "presence": {
                "home": "command_center",
                "idle": [{ "activity": "checking the seals", "seconds": 12 }],
            },
        }))
        .expect("a half-migrated file");

        assert_eq!(d.home_in("archipelago"), PlaceId::new("building_9"));
        assert_eq!(d.home_in("default"), PlaceId::new("command_center"));
    }

    #[test]
    fn a_residence_survives_a_round_trip_through_the_users_file() {
        // The vault is authored by hand (ADR-0014). What we write has to be what we can read.
        let before = read(serde_json::json!({ "archipelago": { "home": "building_1" } }));
        let text = serde_json::to_string(&before).expect("serialises");
        let after: CharacterDefinition = serde_json::from_str(&text).expect("reads back");
        assert_eq!(before, after);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::concept::PlaceConcept;

    fn profile() -> PresenceProfile {
        PresenceProfile {
            authored_home: None,
            idle: vec![
                IdleBehavior {
                    activity: "reading".into(),
                    seconds: 10,
                },
                IdleBehavior {
                    activity: "looking around".into(),
                    seconds: 5,
                },
            ],
        }
    }

    #[test]
    fn idle_behaviour_advances_and_wraps() {
        let p = profile();
        assert_eq!(p.cycle_seconds(), 15);
        assert_eq!(p.idle_at(0).unwrap().activity, "reading");
        assert_eq!(p.idle_at(9).unwrap().activity, "reading");
        assert_eq!(p.idle_at(10).unwrap().activity, "looking around");
        assert_eq!(p.idle_at(14).unwrap().activity, "looking around");
        // Wraps: the character keeps living rather than stopping at the end of the list.
        assert_eq!(p.idle_at(15).unwrap().activity, "reading");
    }

    #[test]
    fn idle_selection_is_deterministic() {
        // The same elapsed time always yields the same behaviour: presence is a projection
        // of real time, never a random walk.
        let p = profile();
        for t in [0u64, 3, 11, 27, 1_000] {
            assert_eq!(p.idle_at(t), p.idle_at(t));
        }
    }

    #[test]
    fn an_id_is_validated_on_construction_so_nothing_downstream_has_to() {
        assert_eq!(CharacterId::new("mage").unwrap().as_str(), "mage");
        assert_eq!(CharacterId::new("  paladin ").unwrap().as_str(), "paladin");
        assert!(CharacterId::new("").is_err());
        // A character owns a folder; an id that is not file-safe is refused at the door.
        assert!(CharacterId::new("Mage").is_err());
        assert!(CharacterId::new("my mage").is_err());
        assert!(CharacterId::new("../escape").is_err());
    }

    #[test]
    fn nobody_having_decided_is_not_the_same_as_somebody_choosing_none() {
        // The trap this avoids: "empty means all" makes unticking every box grant everything,
        // which is the worst possible reading of a deliberate act. Absent and empty are
        // different facts, so they are different values.
        let mut character = CharacterDefinition {
            id: CharacterId::new("mage").unwrap(),
            name: "Mage".into(),
            archetype: CharacterArchetype::Researcher,
            role: "r".into(),
            worlds: Default::default(),
            prompt: String::new(),
            skills: Default::default(),
            requested_capabilities: None,
            mind: None,
            appearance: None,
            draws_in: None,
            speaks_with: None,
            sounds_like: None,
            presence: profile(),
        };

        // Undecided says so, and says nothing more.
        //
        // It used to answer with a hardcoded list of what the Kernel believed this build could
        // do. That list could never contain a tool from a connected MCP server - the Kernel has
        // no I/O and cannot know one exists - so an unconfigured character could never reach
        // one. The intention was right: undecided means everything available, and the Trust
        // Engine still asks (ADR-0009). Only the Engine can say what "available" is.
        assert!(!character.capabilities_decided());
        assert_eq!(character.wants(), None);

        // Decided, and decided *none*. Respected exactly, and never confused with undecided.
        character.requested_capabilities = Some(Default::default());
        assert!(character.capabilities_decided());
        assert!(character.wants().expect("decided").is_empty());
    }

    #[test]
    fn two_characters_may_share_an_archetype_and_remain_two_people() {
        // The whole point of ADR-0023: archetype classifies, id identifies.
        let a = CharacterId::new("mage").unwrap();
        let b = CharacterId::new("paladin").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn a_zero_second_behaviour_cannot_stall_the_cycle() {
        let p = PresenceProfile {
            authored_home: None,
            idle: vec![IdleBehavior {
                activity: "blink".into(),
                seconds: 0,
            }],
        };
        assert_eq!(p.cycle_seconds(), 1);
        assert_eq!(p.idle_at(5).unwrap().activity, "blink");
    }
}
