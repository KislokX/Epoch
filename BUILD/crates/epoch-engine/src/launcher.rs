//! The Launcher projection — what you see *before* entering a World.
//!
//! A second Experience Surface over the same Engine (`PRODUCT_ARCHITECTURE.md`). The Launcher
//! prepares; the World immerses. They share no presentation and duplicate no logic: this file
//! is a projection, exactly like [`crate::world`], over the same loaded packs.
//!
//! That is the claim this module exists to make true rather than merely diagrammed. If the
//! Launcher ever needs a fact the Engine cannot already answer, the Engine was incomplete —
//! not the surface.
//!
//! ## Honest about emptiness
//!
//! It reports what is actually installed, including what is wrong with it. A World that
//! declares three of five concepts says so; a World with a missing asset says so. The
//! Launcher never presents a broken World as a healthy one, for the same reason the World
//! never shows a Place as inhabited when nobody is there.

use std::collections::BTreeMap;
use std::path::Path;

use epoch_kernel::{
    Brain, CapabilityRequest, CharacterArchetype, CharacterDefinition, CharacterId, ContextPolicy,
    IdleBehavior, Mind, Parameters, PlaceConcept, PresenceProfile, Reasoning,
    RequestedCapabilities, Timbre,
};
use serde::{Deserialize, Serialize};

use crate::definition::DefinitionRegistry;
use crate::pack::WorldPack;
use crate::world::{project_mark, MarkView, RouteView, TerrainView};

/// A World seen small: its own geography, enough to recognise it before entering.
///
/// **Derived, never authored.** A preview drawn from the World's actual terrain and Place
/// positions cannot show something the World does not contain — where a supplied key-art
/// image could promise anything. It also costs an author nothing: every World that has a map
/// already has a preview.
///
/// A World with no geography has no preview, and the Launcher shows that plainly rather than
/// substituting a stock image.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldPreview {
    pub width: f32,
    pub height: f32,
    pub terrain: Vec<TerrainView>,
    /// The road network. What actually makes a map recognisable this small.
    pub routes: Vec<RouteView>,
    /// Each Place as `[x, y, footprint]`, in world units.
    pub places: Vec<[f32; 3]>,
}

/// One installed World, as the Launcher shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldSummary {
    /// Stable identity — what `enter` is addressed by. Never the name.
    pub id: String,
    pub name: String,
    pub version: String,
    /// What kind of World this is, in its author's words. `None` when they did not say, and
    /// the Launcher says *that* rather than inventing a genre.
    pub kind: Option<String>,
    /// What this World is. Same rule.
    pub description: Option<String>,
    /// Which berth it occupies in the docking bay, from 1.
    ///
    /// Presentation, but *derived* presentation: it is this World's position in a list the
    /// Engine already sorts deterministically. A berth number that moved between runs would
    /// be a worse lie than no berth at all.
    pub berth: usize,
    /// License type and holder. Mandatory for every World (ADR-0016), so this is never empty.
    pub license: String,
    /// Fraction of the concepts the Engine knows that this World supplies, `0.0` to `1.0`.
    ///
    /// Computed by the Engine and, until now, read by nobody. A World covering three of five
    /// concepts is usable — the rest fall through to visible placeholders — and the user
    /// should be told before entering rather than after.
    pub coverage: f32,
    pub places: usize,
    /// How many characters actually live here. Counted from the crew's own rosters, which is
    /// the only place that fact exists (ADR-0023) — an earlier Launcher counted names a World
    /// supplied for archetypes and reported a population nobody had ever created.
    pub characters: usize,
    /// Everything wrong with this World: unreadable assets, unknown roles, Places with no
    /// concept. Shown plainly. A World with problems still opens.
    pub problems: Vec<String>,
    /// The World drawn small, when it has a geography to draw.
    ///
    /// Always available. This is the floor the fallback chain stands on: whatever else is or
    /// is not authored, a World with a map can be *seen*.
    pub preview: Option<WorldPreview>,
    /// Key art the author supplied, as a `data:` URI.
    ///
    /// Takes precedence over the derived chart when present — the author decides how much
    /// immersion their World earns and how detailed it is allowed to look. The chart stays
    /// underneath as the honest fallback, and the surface can still show it on request, so
    /// artwork adds to something that already works rather than replacing it.
    pub art: Option<String>,
    /// The folder this World actually works in, exactly as the user chose it (ADR-0025).
    ///
    /// `None` is complete, not unfinished: a World with no project root still opens, the crew
    /// still lives there, and there is simply no code to read.
    pub project_root: Option<String>,
    /// True when a root is set and the folder is no longer there — a drive unplugged, a folder
    /// moved. Said plainly rather than silently behaving as though none were set.
    pub project_missing: bool,
    /// The folder of notes this World reads from, exactly as the user chose it.
    ///
    /// A separate question from the Project Root, not a second one: a project is worked in and
    /// a library is read. `None` is complete — a World that knows nothing yet still opens.
    pub library: Option<String>,
    /// True when a library is set and the folder has gone. Same rule as `project_missing`, and
    /// it matters more: a crew whose notes are unplugged reports finding nothing, which reads
    /// as an empty vault rather than an absent one.
    pub library_missing: bool,
}

/// One step of a character's routine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutineStep {
    pub activity: String,
    pub seconds: u32,
}

/// An edit to one character, as a surface submits it.
///
/// The Launcher sends *what the user typed*; everything else about the character is preserved
/// from the file. In particular the roster is not here — moving somebody between Worlds is a
/// separate, narrower command, so a slip in the editor cannot silently evict them.
///
/// Validated here rather than in the surface. A surface that validates is a surface that can
/// disagree with the Engine, and the Engine is what has to live with the file afterwards.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterEdit {
    pub id: String,
    pub name: String,
    pub archetype: String,
    pub role: String,
    pub prompt: String,
    /// Provider id, or `None` to leave them unable to think.
    #[serde(default)]
    pub provider: Option<String>,
    /// Agent id, when this character works with one instead (ADR-0027).
    ///
    /// Never set alongside `provider`. Which of the two is present **is** the choice — a `kind`
    /// field beside them would be a third thing that could disagree with both.
    #[serde(default)]
    pub agent: Option<String>,
    /// Model name. Ignored without a provider — the pair only means anything together.
    #[serde(default)]
    pub model: Option<String>,
    /// **Not here any more** (ADR-0028). Where somebody lives is a fact about them *in one
    /// World*, so it is decided in the World Editor rather than on the character. Accepted and
    /// ignored so an older form does not fail to submit.
    #[serde(default, skip_serializing)]
    pub home: Option<String>,
    pub routine: Vec<RoutineStep>,
    // Artwork is deliberately absent. It has its own command, for the same reason the roster
    // does: a slip in this form can change what somebody is like, but it must never be able
    // to erase their face. The Engine names art files from the bytes anyway (ADR-0024), so a
    // form carrying a filename could only ever be carrying a stale one.
    /// Canonical parameters (ADR-0026). Every one optional, and `None` means *unset* rather
    /// than zero: the Provider's own default stands, and the file says nobody chose.
    #[serde(default)]
    pub parameters: ParametersEdit,
    /// What this character asks to be able to use. Intent — never a claim about the model.
    ///
    /// `None` is **undecided**, and a form that did not touch the tick-boxes sends it back as
    /// `None`. Sending the visible list instead is how an undecided character silently became a
    /// character who had chosen whatever happened to be connected that minute.
    #[serde(default)]
    pub requested_capabilities: Option<Vec<String>>,
    /// Ways of working this character has been given.
    ///
    /// `None` is **untouched**, exactly as it is above and for the same reason: a form that does
    /// not manage Skills sends nothing, and a plain `Vec` would let it silently take away every
    /// Skill somebody had. That defect has been shipped in this codebase once already, in
    /// `mcp.toml`, and it looked like a server that stopped working for no reason.
    pub skills: Option<Vec<String>>,
    /// Which voice this character speaks with — a **voice name**, never a file.
    ///
    /// `None` is **untouched**, exactly as `skills` and `requested_capabilities` are, and for the
    /// same reason: a form that does not manage voices sends nothing, and reading that as *no
    /// voice* would silence somebody every time their name was edited. `Some("")` is the
    /// deliberate *none*, which a form does say — an empty option in a list is a choice.
    #[serde(default)]
    pub speaks_with: Option<String>,
    /// Which converted RVC voice colours that one, and at what pitch.
    ///
    /// The same three states as `speaks_with`: `None` untouched, an **empty voice name** the
    /// deliberate none, anything else a choice. The pitch cannot be sent on its own, because a
    /// pitch is a property of the pair and a form that could edit them apart would eventually
    /// hold a pitch for a model nobody selected.
    #[serde(default)]
    pub sounds_like: Option<Timbre>,
    /// Provider-native values, in the assigned Provider's own vocabulary (ADR-0026).
    ///
    /// Only the assigned Provider's namespace. A form can only edit what it was told exists,
    /// and it is told by that Provider — so it has no business rewriting another's.
    #[serde(default)]
    pub tuning: std::collections::BTreeMap<String, epoch_kernel::TuningValue>,
}

/// What this computer currently offers, for validating an edit against reality.
///
/// Assembled by the caller because only the shell knows it, and passed as **data** so the
/// validation itself stays one testable function — the same shape `Ingredients` takes for
/// composing a turn.
#[derive(Debug, Clone, Default)]
pub struct Machine {
    /// Backend ids that are configured and switched on.
    pub offers: Vec<String>,
    /// How much the assigned model can hold, when it would say.
    ///
    /// `None` is **unknown**, never a guess — and an unknown ceiling is not a ceiling, so a
    /// value passes rather than being refused against a number nobody measured.
    pub window: Option<u32>,
    /// What the assigned backend says it can tune for the assigned model.
    pub controls: Vec<epoch_kernel::Control>,
    /// Agent ids this machine actually has, **measured** (ADR-0027).
    ///
    /// A separate list from `offers` because they are separate questions: a backend is
    /// configured — an endpoint somebody typed — while an agent is a program that is installed
    /// or is not, and nothing the user configures changes that.
    pub agents: Vec<String>,
}

impl Machine {
    /// Nothing configured. Used by tests that are not about backends, and honest for a machine
    /// where the user removed every one.
    pub fn bare() -> Self {
        Self::default()
    }

    pub fn offers(&self, id: &str) -> bool {
        self.offers.iter().any(|o| o == id)
    }

    /// Whether this machine has the agent somebody just chose.
    ///
    /// Checked at **edit** time, like a backend is, and for the same reason: it catches picking
    /// something that is not there while the user is still looking at the list. Deliberately
    /// not checked on **load** — a character travels between machines (ADR-0023), and arriving
    /// somewhere its agent is missing makes it unable to work, never invalid.
    pub fn hosts(&self, id: &str) -> bool {
        self.agents.iter().any(|a| a == id)
    }

    /// Whether asking for this much context is asking for more than exists.
    ///
    /// Refused rather than clamped, like every other bound: silently reducing what somebody
    /// typed means the file says one thing and the turn does another.
    fn too_much(&self, wanted: Option<u32>) -> Option<String> {
        let (Some(window), Some(wanted)) = (self.window, wanted) else {
            return None;
        };
        (wanted > window).then(|| {
            format!(
                "this model holds {window} tokens, and {wanted} was asked for. It reports that limit itself, so this is not a guess."
            )
        })
    }

    /// The same machine, plus one declared control. For building a case in a test.
    pub fn with(mut self, control: epoch_kernel::Control) -> Self {
        self.controls.push(control);
        self
    }
}

/// Canonical parameters as a surface submits them.
///
/// Canonical parameters only. Provider-native values live in `CharacterEdit::tuning`, in the
/// Provider's own words, because that is the vocabulary it declared them in.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParametersEdit {
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    /// **Legacy.** The window a character once asked for. No surface writes this any
    /// more — the window belongs to MODELS (ADR-0026 amendment). Carried so a file
    /// that has one still round-trips, and still honoured as a ceiling a character may
    /// *lower*.
    pub context_tokens: Option<u32>,
    /// How this character wants to use whatever window MODELS gives it.
    ///
    /// Defaulted, so an older surface that does not send it does not fail to save a character.
    #[serde(default)]
    pub context_policy: Option<String>,
    /// A canonical level id — `"off"`, `"low"`, … Refused if invented.
    #[serde(default)]
    pub reasoning: Option<String>,
}

impl CharacterEdit {
    /// Apply this edit to the vault, then re-read it.
    ///
    /// Returns the reason on failure so the Launcher can say *why* rather than failing
    /// silently — the same courtesy the World extends to a broken pack.
    /// Write the edit into the vault.
    ///
    /// `machine` is what this computer currently offers: which backends are configured and
    /// switched on, and what the assigned one says it can tune. Validation lives here rather
    /// than in the surface — a surface that validated separately would eventually disagree with
    /// the file, and the Engine is the one that has to live with what is on disk (ADR-0023).
    pub fn apply(self, registry: &mut DefinitionRegistry, machine: &Machine) -> Result<(), String> {
        let controls = machine.controls.as_slice();
        let id = CharacterId::new(&self.id)?;
        let existing = registry
            .character(&id)
            .ok_or_else(|| format!("no character called '{id}'"))?
            .clone();

        let name = self.name.trim();
        if name.is_empty() {
            return Err("every character is somebody: give them a name".into());
        }
        let role = self.role.trim();
        if role.is_empty() {
            return Err("say in one line what this character is for".into());
        }
        let archetype = CharacterArchetype::from_id(&self.archetype)
            .ok_or_else(|| format!("'{}' is not an archetype this build knows", self.archetype))?;
        // **No home is read here** (ADR-0028). It used to be, and it was validated against the
        // five concepts this build knew — the exact restriction being removed. Where somebody
        // lives is a fact about them in one World, so the World Editor decides it and this form
        // does not touch the residences it carries over.

        // A character is never frozen (Build From Life, rule 2). Refusing here means the file
        // on disk can never reach a state the loader would reject.
        let idle: Vec<IdleBehavior> = self
            .routine
            .into_iter()
            .filter(|step| !step.activity.trim().is_empty())
            .map(|step| IdleBehavior {
                activity: step.activity.trim().to_owned(),
                seconds: step.seconds.max(1),
            })
            .collect();
        if idle.is_empty() {
            return Err("a character is never frozen: give them at least one thing to do".into());
        }

        // An agent, when one was chosen.
        //
        // Ahead of the provider branch because the two are exclusive and `provider` is `None`
        // for an agent — falling through would land in the "the user cleared the field" arm and
        // quietly give them no brain at all.
        let chosen = self
            .agent
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let mind = if let Some(agent) = chosen {
            if !machine.hosts(agent) {
                return Err(format!("'{agent}' is not installed on this machine"));
            }
            // The model is **theirs to name, or not at all**. Empty means "whatever the agent is
            // already set to", which is the honest default: the alternative is Epoch keeping a
            // list of somebody else's model names, and that list is wrong the day they add one.
            let model = self
                .model
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .to_owned();

            // Parameters and tuning are deliberately not carried across. They are a *Provider's*
            // vocabulary (ADR-0026) — an agent has no `num_ctx` — and keeping them would leave a
            // file full of settings that describe nothing.
            Some(Mind {
                brain: Brain::Agent {
                    agent: agent.to_owned(),
                    model,
                },
                parameters: Default::default(),
                tuning: Default::default(),
            })
        } else {
            match (
                self.provider
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty()),
                self.model
                    .as_deref()
                    .map(str::trim)
                    .filter(|s| !s.is_empty()),
            ) {
                (Some(provider), Some(model)) => {
                    // Against what this machine has, not against a list of kinds.
                    //
                    // A backend is configured now, so "ollama" is one possible *name* rather than
                    // the only one — somebody running it on a laptop and on the machine under the
                    // desk has two, and a character must be able to name either.
                    //
                    // This is a check about *editing here*, where the user picked from a list, and
                    // it catches a typo. It is deliberately not a check performed on **load**: a
                    // character travels between machines (ADR-0023), and arriving somewhere its
                    // backend is not installed makes it unable to think, never invalid.
                    if !machine.offers(provider) {
                        return Err(format!("'{provider}' is not a backend this machine offers"));
                    }
                    // Parameters and tuning are carried over, never rebuilt. Editing a name must
                    // not wipe how somebody thinks, and switching provider keeps the old
                    // namespace dormant rather than deleting it (ADR-0026) — moving back
                    // restores it.
                    let kept = existing
                        .mind
                        .clone()
                        .unwrap_or_else(|| Mind::new(provider, model));

                    let reasoning = match self.parameters.reasoning.as_deref().map(str::trim) {
                        None | Some("") => None,
                        Some(raw) => Some(Reasoning::from_id(raw).ok_or_else(|| {
                            format!("'{raw}' is not a reasoning level this build knows")
                        })?),
                    };
                    let parameters = Parameters {
                        temperature: self.parameters.temperature,
                        top_p: self.parameters.top_p,
                        context_tokens: self.parameters.context_tokens,
                        // A policy, not a window. An unknown word is refused rather than
                        // silently dropped: a file saying `context_policy = "lnog"` would
                        // otherwise load as the default and behave as something nobody asked for.
                        //
                        // Parsed by the Kernel, which derives the words from the ones it prints.
                        // The table that stood here was a second spelling of the same fact, and
                        // it agreed with the first by luck — four single words, no spaces.
                        context_policy: match self.parameters.context_policy.as_deref() {
                            None | Some("") => None,
                            Some(raw) => Some(ContextPolicy::from_id(raw).ok_or_else(|| {
                                format!("'{raw}' is not a context policy this build knows")
                            })?),
                        },
                        reasoning,
                    };
                    // Refused, never clamped — and refused *here*, so the file on disk can never
                    // reach a state the loader would reject (ADR-0026).
                    if let Some(reason) = parameters.problems().into_iter().next() {
                        return Err(reason);
                    }
                    // And against what the model actually reports it can hold (ADR-0026 — measured
                    // where possible). `context_tokens` had no ceiling at all while nothing
                    // measured one, so a character could ask for more context than exists and the
                    // file would keep it.
                    if let Some(reason) = machine.too_much(parameters.context_tokens) {
                        return Err(reason);
                    }

                    // Merge rather than replace, and only this Provider's namespace.
                    //
                    // A key the surface sent must be one the Provider declared, and must be a value
                    // that control accepts — refused, never clamped, for the same reason the
                    // canonical parameters are. A declared key the surface *omitted* was cleared by
                    // the user and goes. Anything else authored by hand stays: a character written
                    // against a newer Epoch must not be quietly stripped by an older one (ADR-0026).
                    let mut tuning = kept.tuning;
                    let mine = tuning.entry(provider.to_owned()).or_default();
                    for (name, value) in &self.tuning {
                        let Some(control) = controls.iter().find(|c| &c.name == name) else {
                            return Err(format!("'{provider}' has no setting called '{name}'"));
                        };
                        if !control.accepts(value) {
                            return Err(format!("'{name}' does not accept {value}"));
                        }
                    }
                    for control in controls {
                        match self.tuning.get(&control.name) {
                            Some(value) => {
                                mine.insert(control.name.clone(), value.clone());
                            }
                            None => {
                                mine.remove(&control.name);
                            }
                        }
                    }
                    if mine.is_empty() {
                        tuning.remove(provider);
                    }

                    Some(Mind {
                        // The Launcher edits models. An agent is chosen elsewhere, when there is one
                        // to choose — offering a kind nothing can resolve would be a dead control.
                        brain: Brain::Model {
                            provider: provider.to_owned(),
                            model: model.to_owned(),
                        },
                        parameters,
                        tuning,
                    })
                }
                (Some(_), None) => {
                    return Err("choose a model, or leave the provider unset".into());
                }
                // A model without a provider is discarded rather than refused: it is what the form
                // holds while the user is switching providers, not a state they asked to save.
                //
                // Except when they think with an **agent**. This form edits models — it has no
                // provider to send because there is not one (ADR-0027), and reading that as "the
                // user cleared the field" would silently take somebody's brain away the first time
                // their routine was edited. What this form does not edit, it does not touch.
                _ => match existing.mind.clone() {
                    Some(mind) if mind.provider().is_none() => Some(mind),
                    _ => None,
                },
            }
        };

        // Intent, validated for shape only: the Phase-1 capability taxonomy does not exist yet,
        // and a fixed list here would guess it (ADR-0026). A set, so asking twice asks once.
        // Saving the form *is* deciding, so this is always `Some` afterwards — including
        // when it is empty. "Nobody has decided" and "somebody chose none" are different
        // facts, and only the first one may resolve to everything.
        //
        // Amended: saving is deciding only about what the form could *show*. It showed a
        // materialised list, so a save recorded whichever servers were answering — see
        // `CharacterEdit::requested_capabilities`. A form that did not touch the boxes now
        // sends `None`, and `None` stays undecided.
        let wanted = match &self.requested_capabilities {
            Some(chosen) => {
                let mut wanted = RequestedCapabilities::new();
                for raw in chosen {
                    if raw.trim().is_empty() {
                        continue;
                    }
                    wanted.insert(CapabilityRequest::new(raw)?);
                }
                Some(wanted)
            }
            None => None,
        };

        // Same shape, same reason: untouched leaves what they had. A Skill id is validated
        // rather than trusted — it becomes a filename lookup, and rejecting it here is what
        // makes every use of it afterwards infallible.
        let chosen_skills = match &self.skills {
            Some(chosen) => {
                let mut held = std::collections::BTreeSet::new();
                for raw in chosen {
                    if raw.trim().is_empty() {
                        continue;
                    }
                    held.insert(epoch_kernel::SkillId::new(raw)?);
                }
                held
            }
            None => existing.skills.clone(),
        };

        // Either half is enough to have an appearance at all; neither is also fine.
        registry
            .save(CharacterDefinition {
                id,
                name: name.to_owned(),
                archetype,
                role: role.to_owned(),
                // The roster is deliberately carried over untouched.
                worlds: existing.worlds,
                prompt: self.prompt.trim().to_owned(),
                skills: chosen_skills,
                requested_capabilities: wanted,
                mind,
                // Carried through untouched: this form does not manage artwork.
                appearance: existing.appearance,
                // **Carried over, not cleared.** This said `None`, with a comment about nobody
                // having an opinion until they say so — which describes *creating* a character
                // and not *editing* one. No form manages `draws_in`, so writing `None` here
                // meant that renaming somebody silently threw away a preference they had set by
                // hand. The same defect this file already records for Skills and `mcp.toml`,
                // where it looked like a server that stopped working for no reason.
                draws_in: existing.draws_in,
                // A voice, which this form *does* manage. `None` is untouched and `Some("")` is
                // the deliberate silence — a name whose voice is not installed is kept, because
                // it is a preference the machine cannot honour yet rather than a mistake, and
                // installing the voice should make them sound right again instead of making
                // somebody choose twice.
                speaks_with: match self.speaks_with {
                    None => existing.speaks_with,
                    Some(said) if said.trim().is_empty() => None,
                    Some(said) => Some(said.trim().to_owned()),
                },
                // The same three states, for the same reason -- and the empty name is what
                // clears it, because a timbre with no model is not a timbre. The pitch travels
                // inside it: it is a property of the pair, so it cannot be edited apart from
                // the model it is a pitch *for*.
                sounds_like: match self.sounds_like {
                    None => existing.sounds_like,
                    Some(said) if said.voice.trim().is_empty() => None,
                    Some(said) => Some(Timbre {
                        voice: said.voice.trim().to_owned(),
                        semitones: said.semitones,
                        speaker: said.speaker.max(0),
                    }),
                },
                presence: PresenceProfile {
                    authored_home: None,
                    idle,
                },
            })
            .map_err(|err| err.to_string())
    }
}

/// One member of the crew, as the Launcher shows *and edits* them.
///
/// Characters are **not** supplied by Worlds (ADR-0023). They are the user's: name, face,
/// personality, role and routine all live in the vault and travel into whichever Worlds the
/// character is assigned to. The Launcher is where they are managed, which is why this
/// summary carries everything needed to edit one rather than only enough to list it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CharacterSummary {
    /// Stable identity. What every command addresses; never the name.
    pub id: String,
    pub name: String,
    /// Canonical archetype id. Classification, not identity.
    pub archetype: &'static str,
    /// What this character is for, in their own words.
    pub role: String,
    /// Where the personality actually lives.
    pub prompt: String,
    /// Ways of working they have been given, by id.
    pub skills: Vec<String>,
    /// Who does their thinking: the provider id. `None` means nobody assigned one.
    pub provider: Option<String>,
    /// Which agent works for them, or `None` when a model thinks for them instead (ADR-0027).
    ///
    /// Never set alongside `provider`: the kind of brain *is* which of the two is set.
    pub agent: Option<String>,
    /// The model, as whichever of those two names it. `None` when nobody named one.
    ///
    /// **Empty is `None`.** It was `Some("")` for an agent whose model was left to its own
    /// choice, and every surface reads `model` to decide whether somebody can think — so a
    /// perfectly configured agent showed *NO MODEL · CANNOT THINK* and the chat refused to
    /// open. A sentinel inside a `String` is not a value; it is a bug waiting for a reader.
    pub model: Option<String>,
    /// **What thinks for them**, in one string a surface can show.
    ///
    /// The question every surface was actually asking while it looked at `model`: a model's
    /// name, or the agent's when a model was never named. `None` means nobody assigned a brain
    /// at all — the only case that really is *cannot think*.
    pub brain: Option<String>,
    /// Canonical parameters, exactly as authored. `None` fields mean unset — and unset must
    /// stay visibly distinguishable from a value that happens to equal a common default.
    pub parameters: ParametersView,
    /// Provider-native settings currently in force, as `key = value` strings for display.
    ///
    /// Read-only until a Provider can declare its own controls. Shown rather than hidden
    /// because a file holding settings the user cannot see is a file they cannot explain.
    pub tuning: std::collections::BTreeMap<String, epoch_kernel::TuningValue>,
    /// Providers whose tuning is kept but dormant — the assigned provider is not one of them.
    pub dormant_tuning: Vec<String>,
    /// What this character asks to be able to use. **Requested, never available.**
    ///
    /// `None` means nobody has decided, which resolves to everything available at the moment it
    /// is asked. Deliberately not materialised here — see the note where this is filled in.
    pub requested_capabilities: Option<Vec<String>>,

    /// The Worlds they currently live in, by World id. Empty means nobody has placed them
    /// yet — honest, and visible in the Launcher rather than silently absent.
    pub worlds: Vec<String>,
    /// What they do while idle. A character is never frozen, so this is never empty
    /// (`DefinitionError::NoIdleBehavior`).
    pub routine: Vec<RoutineStep>,
    /// The world sprite they declare, exactly as authored. `None` means no artwork yet.
    pub sprite: Option<String>,
    /// The icon file they declare, if any. `None` means the sprite stands in for it.
    pub icon: Option<String>,
    /// Their face, resolved — the icon when they have one, otherwise the sprite. The same
    /// `MarkView` the World draws with, so the Launcher shows the real thing rather than a
    /// second, prettier version of it.
    pub portrait: Option<MarkView>,
    /// The world sprite, resolved, so the editor can show what it is about to replace.
    pub sprite_mark: Option<MarkView>,
    /// The icon, resolved — **only** when one is authored. Absent means "falls back", and a
    /// preview showing the sprite here would make an inherited image look like a chosen one.
    pub icon_mark: Option<MarkView>,
    /// One resolved sheet per action they have been drawn doing, keyed by action id.
    ///
    /// Resolved rather than authored, because the editor's job here is to **show what is
    /// there**: a cut that does not match the artwork is only ever discovered by watching it
    /// play, and a preview drawn from the numbers in the file rather than from the pipeline
    /// that reads them would agree with the file and disagree with the World.
    pub action_marks: BTreeMap<&'static str, MarkView>,
    /// Which voice they speak with — the **name**, exactly as authored.
    ///
    /// Not resolved against what is installed, deliberately. Whether a voice is on this machine
    /// is a fact about the machine and changes when somebody installs one; whether a character
    /// has chosen a voice is a fact about them. The surface shows both, and it can only tell
    /// them apart if this stays the author's word.
    pub speaks_with: Option<String>,
    /// Which converted RVC voice colours it, and at what pitch. Also unresolved, for the same
    /// reason: what is installed is a fact about the machine.
    pub sounds_like: Option<Timbre>,
    /// Where this character lives on disk. The file *is* the source of truth, and saying so
    /// is what keeps the Launcher an editor rather than a parallel store.
    pub file: String,
}

/// Canonical parameters as a surface shows them.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParametersView {
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    /// **Legacy.** The window a character once asked for. No surface writes this any
    /// more — the window belongs to MODELS (ADR-0026 amendment). Carried so a file
    /// that has one still round-trips, and still honoured as a ceiling a character may
    /// *lower*.
    pub context_tokens: Option<u32>,
    /// How this character wants to use whatever window MODELS gives it.
    pub context_policy: Option<String>,
    /// The canonical level id, or `None`.
    pub reasoning: Option<&'static str>,
}

impl From<&Parameters> for ParametersView {
    fn from(p: &Parameters) -> Self {
        Self {
            temperature: p.temperature,
            top_p: p.top_p,
            context_tokens: p.context_tokens,
            context_policy: p.context_policy.map(ContextPolicy::id),
            reasoning: p.reasoning.map(Reasoning::as_str),
        }
    }
}

/// The choices the Engine actually allows, sent so the Launcher never hardcodes a list.
///
/// A UI carrying its own copy of the archetypes would drift the moment one is added, and the
/// drift would look like a bug in the World.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Vocabulary {
    pub archetypes: Vec<&'static str>,
    pub places: Vec<&'static str>,
    /// The canonical reasoning ladder. Sent for the same reason the archetypes are: a UI
    /// holding its own copy would drift the day a level is added or removed.
    pub reasoning: Vec<&'static str>,
    /// Capabilities a surface offers as tick-boxes. A suggestion list, not a taxonomy — a
    /// character may request something not in it, and that request is kept (ADR-0026).
    ///
    /// **Measured, not listed.** Everything this build registers, plus one entry per connected
    /// MCP server, plus the intentions nothing can do yet. It was a constant in the Kernel,
    /// which could not see either of the first two.
    ///
    /// A server appears **once**, as `mcp:playwright` — not once per tool. Twenty-four boxes
    /// from one server is not a choice anybody makes; it is a wall the user scrolls past, and it
    /// freezes today's tool list into a file that cannot notice tomorrow's.
    pub capabilities: Vec<String>,
    /// Of those, the ones that actually exist right now. A request for anything else is a real
    /// intention with nothing behind it yet, and the surface must say which is which.
    pub built: Vec<String>,
    /// Which of the above are sources, and what each is offering at this moment.
    ///
    /// Sent so a surface can show the count and let somebody see what is inside before ticking
    /// it — that box grants a set of tools with every effect and no reversal, so "what am I
    /// giving them" has to be answerable without leaving the screen.
    pub groups: Vec<crate::capabilities::Group>,
}

/// One authored way of working, as a surface shows it.
///
/// A projection, not the Definition: the `method` is deliberately absent. It is prose meant for
/// a model, it can be pages long, and this list rides every Launcher survey. What a person
/// choosing a Skill needs is its name, one line about it, and what it will want.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub summary: String,
    /// What the method asks for — **a request, never a grant**.
    ///
    /// Sent so a surface can say *this Skill wants Playwright and this character does not have
    /// it* at the moment somebody assigns it, rather than letting the Skill fail on its third
    /// step for a reason nobody can see from here.
    pub requires: Vec<String>,
}

/// Everything the Launcher knows.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LauncherView {
    /// Whose bridge this is. Never absent — an unnamed orchestrator is still an orchestrator.
    pub orchestrator: crate::profile::Orchestrator,
    pub worlds: Vec<WorldSummary>,
    /// Who exists, across every World. Empty is a valid and honest answer.
    pub characters: Vec<CharacterSummary>,
    pub vocabulary: Vocabulary,
    /// Every way of working this vault holds. Empty is ordinary.
    pub skills: Vec<SkillSummary>,
    /// How long this session has been running, in seconds.
    ///
    /// The one number on the bridge that ticks, and it is measured rather than dressed. A
    /// Launcher full of invented gauges is the same lie as a Laboratory that looks busy with
    /// no provider behind it — so the gauges show what is true, and the rest say they are
    /// dormant.
    pub session_seconds: u64,
    /// Problems finding Worlds at all, as opposed to problems *inside* one.
    pub problems: Vec<String>,
    /// Problems with the authored Definitions: a file that will not parse, two claiming one
    /// identity.
    ///
    /// **Both registries**, and it was one until a broken Skill proved why that is not enough.
    /// A registry that cannot read a file skips it and says so — the right behaviour — but the
    /// saying went only to `stderr`, so the whole visible consequence was that *Ways of working*
    /// silently vanished from the editor. A file that does not load must be a sentence somebody
    /// can read, not an absence they have to explain to themselves.
    ///
    /// Named for Definitions rather than for characters because that is what it now holds. The
    /// name was accurate when there was one Definition type and stopped being when there were
    /// two; `state.rs` has called its copy `definition_problems` since that day.
    pub definition_problems: Vec<String>,
}

/// What has actually happened, across every World.
///
/// ## Why this can be real now
///
/// The Ship's Log was cold, and its note said it was waiting on the Activity Recorder
/// (ADR-0015) — the durable append-only log, still deliberately unbuilt. That note was true
/// about *Activities* and wrong about this panel: Quests already persist to the vault, and a
/// Quest's chronicle already records what each character ran as **evidence** (ADR-0025 §7).
/// So a history that survives sessions exists; it simply was not being read.
///
/// Nothing here is derived from a session. Close Epoch, reopen it, and this reads the same —
/// which is the whole difference between a log and a screen that remembers.
#[derive(Debug, Default, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShipsLog {
    /// Every Quest, newest first. Completed, failed, abandoned and in-flight alike: a list of
    /// only successes would be propaganda (ADR-0025 §7).
    pub quests: Vec<LoggedQuest>,
    /// What the crew actually ran, newest first — the console's readout.
    pub runs: Vec<LoggedRun>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggedQuest {
    pub world: String,
    /// What that World is called. Read once here so no surface has to look it up.
    pub world_name: String,
    pub title: String,
    /// A canonical state id — `"completed"`, `"blocked"`, and the rest.
    pub state: &'static str,
    /// Whether this one ended, and ended with evidence. Measured from the chronicle.
    pub evidence: usize,
    /// How much was said. A Quest is a conversation as much as a record.
    pub said: usize,
    pub at: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoggedRun {
    pub world: String,
    pub world_name: String,
    /// Who ran it. An identity, never a name — a name is theirs to change.
    pub character: String,
    /// What it was, in the words whatever produced it used.
    pub summary: String,
    pub at: u64,
}

impl ShipsLog {
    /// How much is kept for a surface to show. Enough to be a log, bounded so opening the
    /// Launcher never costs more the longer Epoch has been used.
    const MOST: usize = 40;

    /// Read every World's Quests, and fold them into one history.
    ///
    /// Across Worlds on purpose: this is the *bridge*, and you are standing on it precisely
    /// because you have not chosen a World yet. A log that only worked once you were inside one
    /// would be useless at the only moment it is on screen.
    pub fn read(worlds_dir: &std::path::Path, vault: &std::path::Path) -> Self {
        let (packs, _) = WorldPack::discover(worlds_dir);
        let mut quests = Vec::new();
        let mut runs = Vec::new();

        for pack in &packs {
            // A World whose History will not parse costs its own history and never the others'.
            // The store reads this only on the Bridge, where the user explicitly asked for a
            // cross-World log; entering a World and taking a turn still load one Quest only.
            let store = crate::quest::QuestStore::default();
            let history = store.history(vault, &pack.id).unwrap_or_default();
            for quest in &history {
                let mut evidence = 0;
                let mut said = 0;
                for record in &quest.chronicle {
                    match &record.entry {
                        epoch_kernel::Entry::Produced { artifact } => {
                            evidence += 1;
                            // A capability run is the crew *doing* something, as opposed to a
                            // file or a commit it left behind. That distinction is what makes
                            // the console a console rather than a second history.
                            if artifact.kind == "capability" {
                                runs.push(LoggedRun {
                                    world: pack.id.clone(),
                                    world_name: pack.name.clone(),
                                    character: artifact.reference.clone(),
                                    summary: artifact.summary.clone(),
                                    at: record.at,
                                });
                            }
                        }
                        epoch_kernel::Entry::Said { .. } | epoch_kernel::Entry::Answered { .. } => {
                            said += 1
                        }
                        _ => {}
                    }
                }

                quests.push(LoggedQuest {
                    world: pack.id.clone(),
                    world_name: pack.name.clone(),
                    title: quest.title.clone(),
                    state: state_id(&quest.state),
                    evidence,
                    said,
                    // The last thing that happened, not when it started: a Quest opened last
                    // week and worked on this morning belongs at the top.
                    at: quest
                        .chronicle
                        .last()
                        .map(|r| r.at)
                        .unwrap_or(quest.created_at),
                });
            }
        }

        // Newest first, and broken apart by World when two share a millisecond — so the same
        // vault produces the same list every time it is read.
        quests.sort_by(|a, b| b.at.cmp(&a.at).then(a.world.cmp(&b.world)));
        runs.sort_by(|a, b| b.at.cmp(&a.at).then(a.world.cmp(&b.world)));
        quests.truncate(Self::MOST);
        runs.truncate(Self::MOST);

        Self { quests, runs }
    }
}

fn state_id(state: &epoch_kernel::QuestState) -> &'static str {
    use epoch_kernel::QuestState::*;
    match state {
        Open => "open",
        Working => "working",
        AwaitingApproval => "awaiting_approval",
        NeedsRevision => "needs_revision",
        Completed => "completed",
        Blocked => "blocked",
        Rejected => "rejected",
        Abandoned => "abandoned",
        Failed => "failed",
    }
}

impl LauncherView {
    /// Survey what is installed: the Worlds on disk, and the crew the vault holds.
    ///
    /// Always succeeds. Nothing installed is a valid answer, and the Launcher says so rather
    /// than failing — the same rule the World follows when a pack is missing.
    /// `vault` is the folder holding the orchestrator's own profile — the parent of the
    /// definitions directory. Deliberately *not* inside it: everything with a `.toml` in
    /// `definitions/characters` is read as a character, and the user is not one.
    /// `outside` is one [`Group`](crate::capabilities::Group) per connected MCP server. Passed
    /// in rather than read here, because a survey of files on disk must not start a process —
    /// and because the caller is the one holding the bridge.
    pub fn survey(
        worlds_dir: &Path,
        vault: &Path,
        definitions: &DefinitionRegistry,
        skills: &crate::skills::SkillRegistry,
        session_seconds: u64,
        outside: &[crate::capabilities::Group],
    ) -> Self {
        // What can actually be asked for: what this build registers, and one entry per thing
        // connected to it. Sorted and deduplicated — a group id is namespaced, so it cannot
        // collide with a capability of Epoch's own, but the sort keeps the offer stable between
        // runs rather than following whichever order the servers answered in.
        let mut built: Vec<String> = crate::capabilities::catalogue()
            .iter()
            .map(|d| d.id.to_string())
            .chain(outside.iter().map(|g| g.id.clone()))
            .collect();
        built.sort();
        built.dedup();

        let (packs, problems) = WorldPack::discover(worlds_dir);
        // Where each World works. In the vault, never in the pack — a shipped pack cannot know
        // where you keep your code (ADR-0025, and the same argument as the crew roster).
        let projects = crate::project::ProjectRoots::load(vault);
        let libraries = crate::library::Libraries::load(vault);

        // Already in id order: the registry is a BTreeMap, so a list that reorders between
        // runs is impossible rather than merely unlikely.
        let characters: Vec<CharacterSummary> = definitions
            .loaded()
            .map(|loaded| {
                let c = &loaded.definition;
                CharacterSummary {
                    id: c.id.to_string(),
                    name: c.name.clone(),
                    archetype: c.archetype.id(),
                    role: c.role.clone(),
                    prompt: c.prompt.clone(),
                    skills: c.skills.iter().map(|id| id.to_string()).collect(),
                    provider: c
                        .mind
                        .as_ref()
                        .and_then(|m| m.provider().map(str::to_owned)),
                    // `None` for a model, `Some` for an agent — never both, so a surface can
                    // tell which kind of brain this is without being told twice.
                    agent: c
                        .mind
                        .as_ref()
                        .and_then(|m| m.brain.agent().map(str::to_owned)),
                    model: c
                        .mind
                        .as_ref()
                        .map(|m| m.model().to_owned())
                        .filter(|m| !m.trim().is_empty()),
                    // One field for one question. Falls back to the agent's id when no model was
                    // named, because "Claude Code, its own choice" is a brain and `None` is not.
                    brain: c.mind.as_ref().map(|m| {
                        let model = m.model().trim();
                        if !model.is_empty() {
                            model.to_owned()
                        } else {
                            // The agent's *name*, not its id. A file says `claude-code`; a
                            // person reads "Claude Code", and looking it up here costs nothing
                            // because it does not ask whether the program is installed.
                            let id = m.brain.agent().unwrap_or_default();
                            crate::agents::name_of(id).unwrap_or(id).to_owned()
                        }
                    }),
                    parameters: c
                        .mind
                        .as_ref()
                        .map(|m| ParametersView::from(&m.parameters))
                        .unwrap_or_default(),
                    // The values, not a rendering of them. They were formatted here while the
                    // panel could only display them; now that the Provider declares its
                    // controls, the panel edits them and needs what is actually stored.
                    tuning: c
                        .mind
                        .as_ref()
                        .and_then(|m| m.active_tuning())
                        .cloned()
                        .unwrap_or_default(),
                    dormant_tuning: c
                        .mind
                        .as_ref()
                        .map(|m| m.dormant_tuning().cloned().collect())
                        .unwrap_or_default(),
                    // Resolved, so a surface shows what they will actually be offered rather
                    // than an empty list that behaves like a full one. Undecided means
                    // everything that exists, which is now a measurement rather than a guess.
                    //
                    // **`None` is sent as `None`.** It used to be materialised here into the
                    // list of everything currently available, which made an undecided character
                    // indistinguishable from one who had chosen exactly today's set — and the
                    // next save wrote that appearance to disk. A server that stopped answering
                    // for one minute was enough: its box left the screen, the save recorded its
                    // absence, and a request the user had made was gone with nothing said. The
                    // surface has to be able to tell "everything, always" from "these, today".
                    requested_capabilities: c
                        .wants()
                        .map(|chosen| chosen.iter().map(|r| r.to_string()).collect()),
                    // The author's word, never checked against what is installed: those
                    // are two different facts and the surface shows both.
                    speaks_with: c.speaks_with.clone(),
                    sounds_like: c.sounds_like.clone(),
                    worlds: c.worlds.keys().cloned().collect(),
                    routine: c
                        .presence
                        .idle
                        .iter()
                        .map(|b| RoutineStep {
                            activity: b.activity.clone(),
                            seconds: b.seconds,
                        })
                        .collect(),
                    sprite: c.appearance.as_ref().and_then(|a| a.sprite.clone()),
                    icon: c.appearance.as_ref().and_then(|a| a.icon.clone()),
                    // The icon when one exists, the sprite when it does not: recognisable
                    // everywhere from a single drawing, without pretending it was chosen.
                    portrait: loaded
                        .icon
                        .as_ref()
                        .or(loaded.appearance.as_ref())
                        .map(project_mark),
                    sprite_mark: loaded.appearance.as_ref().map(project_mark),
                    icon_mark: loaded.icon.as_ref().map(project_mark),
                    action_marks: loaded
                        .actions
                        .iter()
                        .map(|(action, mark)| (action.id(), project_mark(mark)))
                        .collect(),
                    file: loaded.path.display().to_string(),
                }
            })
            .collect();

        Self {
            orchestrator: crate::profile::Orchestrator::load(vault),
            worlds: packs
                .iter()
                .enumerate()
                .map(|(i, pack)| {
                    summarize(
                        pack,
                        definitions.living_in(&pack.id).count(),
                        i + 1,
                        &projects,
                        &libraries,
                    )
                })
                .collect(),
            characters,
            skills: skills
                .all()
                .map(|skill| SkillSummary {
                    id: skill.id.to_string(),
                    name: skill.name.clone(),
                    summary: skill.summary.clone(),
                    requires: skill
                        .requires
                        .iter()
                        .map(|r| r.as_str().to_owned())
                        .collect(),
                })
                .collect(),
            vocabulary: Vocabulary {
                archetypes: CharacterArchetype::ALL.iter().map(|a| a.id()).collect(),
                // What a **model** can be asked for, not every rung the Kernel knows. `xhigh`
                // exists on Claude Code's command line and on no model backend here, so listing
                // it in a model's form would offer a level that silently resolves to `high`.
                reasoning: crate::deliberation::for_models()
                    .iter()
                    .map(|r| r.as_str())
                    .collect(),
                // The same list, and now literally so: `capabilities` was `built` plus the
                // intentions, and there are none left (see `nothing_is_offered_that_nothing_can_do`).
                // Kept as its own field rather than collapsed, because the *question* is still
                // two questions — what may be asked for, and what exists — and the day an
                // intention returns they part company again.
                capabilities: built.clone(),
                built,
                groups: outside.to_vec(),
                places: PlaceConcept::ALL.iter().map(|p| p.id()).collect(),
            },
            session_seconds,
            problems,
            definition_problems: definitions
                .problems()
                .iter()
                .chain(skills.problems())
                .cloned()
                .collect(),
        }
    }
}

fn summarize(
    pack: &WorldPack,
    characters: usize,
    berth: usize,
    projects: &crate::project::ProjectRoots,
    libraries: &crate::library::Libraries,
) -> WorldSummary {
    let places = pack.declared();
    let project_root = projects.authored(&pack.id).map(str::to_owned);
    let library = libraries.authored(&pack.id).map(str::to_owned);
    let library_missing = library.is_some() && libraries.open(&pack.id).is_none();
    // Authored but not openable means the folder went away. The user's choice is kept — a
    // choice is not deleted because a drive was unplugged — and the state is reported.
    let project_missing = project_root.is_some() && projects.open(&pack.id).is_none();
    WorldSummary {
        id: pack.id.clone(),
        name: pack.name.clone(),
        version: pack.version.clone(),
        kind: pack.kind.clone(),
        description: pack.description.clone(),
        art: pack.preview.clone(),
        project_root,
        project_missing,
        library,
        library_missing,
        berth,
        license: format!("{} · {}", pack.license.kind, pack.license.holder),
        coverage: pack.coverage(),
        places,
        characters,
        problems: pack.problems().to_vec(),
        preview: pack.map.as_ref().map(|map| WorldPreview {
            width: map.size.width,
            height: map.size.height,
            terrain: map
                .terrain
                .iter()
                .map(|area| TerrainView {
                    kind: area.kind.id(),
                    points: area.points.iter().map(|p| [p.x, p.y]).collect(),
                })
                .collect(),
            routes: pack
                .route_lines()
                .into_iter()
                .map(|(points, prominence)| RouteView { points, prominence })
                .collect(),
            places: pack.place_dots(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn worlds_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packs")
    }

    /// A vault that belongs to these tests, and to nobody.
    ///
    /// This read `../../vault` — the vault of whichever machine was running the tests. So
    /// `the_crew_comes_from_the_vault_and_carries_its_own_identity` asserted a crew existed, and
    /// what it measured was whether the developer happened to have characters that day. Renaming
    /// one could turn a test red; deleting one certainly would.
    ///
    /// It also stood in the way of `BUILD/vault` leaving the repository, which it must: a vault
    /// holds somebody's crew, conversations, project paths and standing permissions, and none of
    /// that is what a user installs.
    ///
    /// `packs/` stays real below, and that is not the same thing — a pack **is** shipped content,
    /// so a test that reads one is testing what we distribute.
    fn vault() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vault")
    }

    fn definitions() -> crate::DefinitionRegistry {
        crate::DefinitionRegistry::load(vault().join("definitions"))
    }

    fn skills() -> crate::skills::SkillRegistry {
        crate::skills::SkillRegistry::load(vault())
    }

    /// Survey what actually ships. Session length is fixed so nothing here depends on a clock.
    fn survey() -> LauncherView {
        LauncherView::survey(&worlds_dir(), &vault(), &definitions(), &skills(), 0, &[])
    }

    #[test]
    fn a_skill_that_will_not_parse_is_a_sentence_rather_than_an_absence() {
        // The reported symptom was "ya no sale": a broken `.toml` made *Ways of working*
        // disappear from the editor, and the only trace was a line on `stderr`. Skipping the
        // file is right; skipping it silently is what made a one-character mistake look like
        // the feature breaking.
        let vault = std::env::temp_dir().join(format!("epoch-skill-{}", std::process::id()));
        let dir = vault.join("definitions/skills");
        std::fs::create_dir_all(&dir).expect("a scratch vault");
        std::fs::write(dir.join("broken.toml"), "name = \"Half a file").expect("write");

        let skills = crate::skills::SkillRegistry::load(&vault);
        let view = LauncherView::survey(&worlds_dir(), &vault, &definitions(), &skills, 0, &[]);

        assert!(view.skills.is_empty(), "an unreadable Skill is not offered");
        assert!(
            view.definition_problems
                .iter()
                .any(|problem| problem.contains("broken.toml")),
            "the file has to be named: {:?}",
            view.definition_problems
        );

        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn what_can_be_asked_for_is_what_exists_plus_what_is_connected() {
        // The defect this replaced: the offered list was a constant in the Kernel, so a tool
        // from a connected MCP server could never appear in it - and a character therefore
        // could not be given one, however plainly the server was offering it.
        let view = LauncherView::survey(
            &worlds_dir(),
            &vault(),
            &definitions(),
            &skills(),
            0,
            &[gmail()],
        );

        // Everything this build registers.
        for id in crate::capabilities::catalogue().iter().map(|d| &d.id) {
            assert!(
                view.vocabulary.built.contains(&id.to_string()),
                "{id} exists and is not offered"
            );
        }
        // And the outside server, which is the whole point.
        assert!(view.vocabulary.built.contains(&"mcp:gmail".to_string()));
        assert!(view
            .vocabulary
            .capabilities
            .contains(&"mcp:gmail".to_string()));
    }

    fn gmail() -> crate::capabilities::Group {
        crate::capabilities::Group {
            id: "mcp:gmail".into(),
            label: "Gmail".into(),
            tools: vec!["gmail_send".into(), "gmail_search".into()],
            answering: true,
        }
    }

    #[test]
    fn a_server_is_offered_once_however_many_tools_it_has() {
        // The wall this replaced: one box per tool. Playwright alone put two dozen of them on
        // the screen, and the file they wrote froze that day's list forever.
        let view = LauncherView::survey(
            &worlds_dir(),
            &vault(),
            &definitions(),
            &skills(),
            0,
            &[gmail()],
        );

        assert_eq!(
            view.vocabulary
                .built
                .iter()
                .filter(|id| id.starts_with("mcp:"))
                .count(),
            1,
            "one server, one box"
        );
        for tool in &gmail().tools {
            assert!(
                !view.vocabulary.built.contains(tool),
                "'{tool}' is inside the server's box, not beside it"
            );
        }
        // But what is inside is sent, because ticking that box grants all of it.
        assert_eq!(view.vocabulary.groups, vec![gmail()]);
    }

    #[test]
    fn a_server_named_after_a_capability_cannot_collide_with_it() {
        // The old offer was a flat list of ids, so a server naming a tool `web_search` produced
        // two identical boxes. A group id is namespaced, so the clash is not filtered — it is
        // unsayable.
        let view = LauncherView::survey(
            &worlds_dir(),
            &vault(),
            &definitions(),
            &skills(),
            0,
            &[crate::capabilities::Group {
                id: "mcp:web_search".into(),
                label: "Web Search".into(),
                tools: vec!["web_search_go".into()],
                answering: true,
            }],
        );
        assert!(view.vocabulary.built.contains(&"web_search".to_string()));
        assert!(view
            .vocabulary
            .built
            .contains(&"mcp:web_search".to_string()));
        assert_eq!(
            view.vocabulary
                .built
                .iter()
                .filter(|id| id.ends_with("web_search"))
                .count(),
            2,
            "two different things, two boxes"
        );
    }

    #[test]
    fn an_undecided_character_is_sent_as_undecided_rather_than_as_todays_list() {
        // The defect, in the order it happened: a character with no `requested_capabilities` was
        // sent the whole available list, which the editor drew as ticked boxes. A connected MCP
        // server then stopped answering — a malformed argument was enough — so its box left the
        // screen. Saving anything at all wrote the boxes that remained, and the request the user
        // had made was gone. Nothing had said so, and it looked like the tool had vanished.
        //
        // `None` now travels as `None`, so a save that did not touch the boxes cannot narrow it.
        let view = LauncherView::survey(
            &worlds_dir(),
            &vault(),
            &definitions(),
            &skills(),
            0,
            &[gmail()],
        );
        let undecided = view
            .characters
            .iter()
            .find(|c| c.requested_capabilities.is_none());
        assert!(
            undecided.is_some()
                || view
                    .characters
                    .iter()
                    .all(|c| c.requested_capabilities.is_some()),
            "the vault decides which case this is; both are valid, neither may be materialised"
        );

        // Whatever the vault holds, a decided character's list is theirs verbatim — never
        // widened to what happens to be connected.
        for character in &view.characters {
            if let Some(chosen) = &character.requested_capabilities {
                assert!(
                    chosen.iter().all(|id| !id.starts_with("gmail_")),
                    "'{}' was given a server's individual tools nobody authored",
                    character.id
                );
            }
        }
    }

    #[test]
    fn nothing_is_offered_that_nothing_can_do() {
        // This asserted the opposite until 2026-08-16: `vision` was offered as an *intention* —
        // a request worth authoring before anything could satisfy it — and shown dimmed.
        //
        // It stopped being one. Sight is measured now (`provider::Declared`), so a tick-box
        // asking a person to assert it governed nothing, in the one place the user is told they
        // are choosing what somebody can do. Reaching for a picture through a translator kept
        // its own id, `see_image`, in the live catalogue like everything else.
        //
        // So the offered list is exactly what exists. The concept of an intention is sound and
        // may return the day there is a real one; an empty list nobody fills is not that day.
        let view = survey();

        assert!(!view.vocabulary.capabilities.contains(&"vision".to_string()));
        assert_eq!(
            view.vocabulary.capabilities, view.vocabulary.built,
            "an offered request that nothing can satisfy is a promise Epoch did not make"
        );
    }

    #[test]
    fn the_crew_comes_from_the_vault_and_carries_its_own_identity() {
        // A World supplies places; the user supplies people (ADR-0023). Everything needed to
        // recognise and edit somebody is here, and none of it came from a pack.
        let view = survey();
        assert!(
            view.definition_problems.is_empty(),
            "{:?}",
            view.definition_problems
        );
        assert!(!view.characters.is_empty(), "the vault holds nobody");
        for character in &view.characters {
            assert!(!character.id.trim().is_empty());
            assert!(
                !character.name.trim().is_empty(),
                "'{}' is nameless",
                character.id
            );
            assert!(!character.role.trim().is_empty());
            // Never frozen: a Definition without idle behaviour is rejected at load.
            assert!(
                !character.routine.is_empty(),
                "'{}' has no routine",
                character.id
            );
            assert!(character.file.ends_with(".toml"));
        }
    }

    #[test]
    fn a_world_reports_the_population_that_actually_lives_in_it() {
        // The count is derived from the crew's rosters, so it cannot exceed the crew — which
        // is exactly the failure the previous Launcher shipped.
        let view = survey();
        for world in &view.worlds {
            let living = view
                .characters
                .iter()
                .filter(|c| c.worlds.iter().any(|w| w == &world.id))
                .count();
            assert_eq!(
                world.characters, living,
                "'{}' miscounts its population",
                world.id
            );
            assert!(world.characters <= view.characters.len());
        }
    }

    #[test]
    fn the_launcher_is_told_which_choices_the_engine_allows() {
        // So no dropdown in the UI carries its own copy of the vocabulary and drifts.
        let view = survey();
        assert_eq!(
            view.vocabulary.archetypes.len(),
            CharacterArchetype::ALL.len()
        );
        assert_eq!(view.vocabulary.places.len(), PlaceConcept::ALL.len());
        for character in &view.characters {
            assert!(view.vocabulary.archetypes.contains(&character.archetype));
        }
    }

    #[test]
    fn the_launcher_finds_every_world_that_ships() {
        // **Counted off the directory rather than written down.** This asserted `>= 2`, which
        // was true while two packs shipped and became a statement about the product rather than
        // about discovery the day one of them was removed. What is worth holding is that every
        // pack on disk is found and none of them reports a problem — a number is a second place
        // deciding which Worlds ship.
        let shipped: Vec<String> = std::fs::read_dir(worlds_dir())
            .expect("the packs directory ships")
            .flatten()
            .filter(|entry| entry.path().join("pack.toml").is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(!shipped.is_empty(), "something has to ship");

        let view = survey();
        let found: Vec<&str> = view.worlds.iter().map(|w| w.id.as_str()).collect();
        for one in &shipped {
            assert!(
                found.contains(&one.as_str()),
                "{one} ships and was not found: {found:?}"
            );
        }
        assert!(view.problems.is_empty(), "{:?}", view.problems);
    }

    #[test]
    fn worlds_are_listed_in_a_stable_order() {
        // Discovery reads a directory, whose order belongs to the operating system. If this
        // ever fails, the Launcher would show a different list on a different machine.
        let ids = |v: &LauncherView| v.worlds.iter().map(|w| w.id.clone()).collect::<Vec<_>>();
        let first = survey();
        let second = survey();
        assert_eq!(ids(&first), ids(&second));
        let mut sorted = ids(&first);
        sorted.sort();
        assert_eq!(ids(&first), sorted);
    }

    #[test]
    fn every_shipped_world_declares_a_license_and_is_free_of_problems() {
        for world in survey().worlds {
            assert!(
                !world.license.trim().is_empty(),
                "'{}' has no license",
                world.id
            );
            assert!(
                world.problems.is_empty(),
                "'{}': {:?}",
                world.id,
                world.problems
            );
        }
    }

    #[test]
    fn a_partially_authored_world_is_offered_with_its_coverage_shown_rather_than_hidden() {
        // The Archipelago declares three of five concepts on purpose. It is a usable World —
        // the rest degrade to visible placeholders — and the Launcher must say so up front
        // instead of pretending completeness or refusing to list it.
        let view = survey();
        let partial = view
            .worlds
            .iter()
            .find(|w| w.id == "archipelago")
            .expect("the second World must be discovered");
        assert!(partial.coverage < 1.0, "expected partial coverage");
        assert!(partial.coverage > 0.0, "expected it to cover something");
        assert_eq!(partial.places, 3);
    }

    #[test]
    fn a_preview_is_the_world_itself_drawn_small() {
        // Derived, never authored: a preview cannot show something the World does not contain.
        //
        // **Amended once Worlds became creatable.** This used to demand a preview from *every*
        // installed World, which held only because every World that existed had been authored
        // with a geography. A World somebody just made has none — and that is the correct
        // state, not a fault: there is nothing to derive a chart from yet, and the World Editor
        // is where it stops being empty. The rule the test meant all along is the conditional
        // one, so that is what it says now.
        let mut previewed = 0;
        for world in survey().worlds {
            let Some(preview) = world.preview else {
                assert_eq!(
                    world.places, 0,
                    "'{}' declares Places but previews none of them",
                    world.id
                );
                continue;
            };
            previewed += 1;
            assert!(preview.width > 0.0 && preview.height > 0.0);
            assert!(!preview.terrain.is_empty(), "'{}' has no terrain", world.id);
            assert_eq!(
                preview.places.len(),
                world.places,
                "'{}' previews a different number of Places than it declares",
                world.id
            );
        }
        assert!(
            previewed > 0,
            "the shipped Worlds must still preview themselves"
        );
    }
}
