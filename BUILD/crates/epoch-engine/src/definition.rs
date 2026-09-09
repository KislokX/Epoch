//! The Definition Registry.
//!
//! The repository implementation for Definitions over the vault-file Storage Adapter
//! (ADR-0014 clarification). Not a parallel store: a Definition's file in the vault **is**
//! the source of truth, and this watches it.
//!
//! Knows files; knows nothing about execution (ADR-0011).
//!
//! Hot reload is done by comparing modification times on each poll rather than by taking a
//! filesystem-watching dependency. For a handful of authored files that is sufficient, and
//! we have no evidence yet that it is not (Build From Life, rule 14).
//!
//! ## The registry is the crew (ADR-0023)
//!
//! Characters are the user's, not a World's. They are keyed by [`CharacterId`], carry their
//! own name and face, and declare which Worlds they live in. That makes this registry the
//! one place a character exists, and moving somebody between Worlds an edit to *them* —
//! which is both what the user means and the only version a shipped World Pack can survive.
//!
//! This module therefore also **writes**. It is still not a parallel store: it edits the same
//! file it reads, and re-reads afterwards, so the file remains the source of truth.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use epoch_kernel::{CharacterDefinition, CharacterId, PlaceId};

use crate::place::{resolve_actions, resolve_appearance, resolve_icon, Mark};

#[derive(Debug, thiserror::Error)]
pub enum DefinitionError {
    #[error("cannot read definition at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The parse failure, **boxed**.
    ///
    /// `toml::de::Error` is around 150 bytes, which made every `Result<_, DefinitionError>` in
    /// this module that size on the success path too — fourteen functions, all of which succeed
    /// almost always. Boxed, the whole enum fits in a machine word plus a `PathBuf`, and the
    /// error path pays for itself instead of the happy one.
    #[error("invalid definition at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    /// A character with no idle behaviour would stand frozen. Characters are never frozen
    /// (Build From Life, rule 2), so this is rejected rather than rendered.
    #[error("definition at {path} declares no idle behaviour; a character is never frozen")]
    NoIdleBehavior { path: PathBuf },
    #[error("definition at {path} declares an empty role")]
    EmptyRole { path: PathBuf },
    #[error("definition at {path} declares an empty name; every character is somebody")]
    EmptyName { path: PathBuf },
    #[error("definition at {path} names a provider but no model")]
    NoModel { path: PathBuf },
    /// Refused, never clamped (ADR-0026). A silently corrected `temperature = 7.0` would leave
    /// the file saying one thing and the model doing another — and the user with no way to
    /// discover which of the two is true.
    #[error("definition at {path}: {reason}")]
    BadParameter { path: PathBuf, reason: String },
    /// The file name is the id. Allowing them to disagree would let two files claim one
    /// character, and the survivor would depend on directory order.
    #[error("definition at {path} declares id '{id}' but is not named '{id}.toml'")]
    IdMismatch { path: PathBuf, id: CharacterId },
    #[error("two definitions claim the character '{id}': {first} and {second}")]
    DuplicateId {
        id: CharacterId,
        first: PathBuf,
        second: PathBuf,
    },
    #[error("no character called '{0}'")]
    Unknown(CharacterId),
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot serialise character '{id}': {source}")]
    Serialize {
        id: CharacterId,
        #[source]
        source: toml::ser::Error,
    },
}

/// Which of somebody's drawings is being addressed.
///
/// A closed set rather than a boolean, because there are no longer two. The still sprite and
/// the icon answer different questions — a body read at world scale is not a face read at
/// 44px — and each action is a third kind of answer again: a sheet of somebody moving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtSlot {
    /// The still picture that walks around the World, and the fallback for every action
    /// nobody has drawn.
    Sprite,
    /// The face that identifies them in a conversation, a roster, a list.
    Icon,
    /// A sheet for one action (ADR-0018 amendment: `idle`, `walk`, `think`, `work`).
    Doing(epoch_kernel::Action),
}

impl ArtSlot {
    /// The word a surface addresses this slot by.
    pub fn id(self) -> &'static str {
        match self {
            ArtSlot::Sprite => "sprite",
            ArtSlot::Icon => "icon",
            ArtSlot::Doing(action) => action.id(),
        }
    }

    /// Parse a slot name. `None` for a word this build does not know — refused at the door
    /// rather than trusted, like every other choice arriving from a surface.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "sprite" => Some(ArtSlot::Sprite),
            "icon" => Some(ArtSlot::Icon),
            other => epoch_kernel::Action::from_id(other).map(ArtSlot::Doing),
        }
    }
}

/// A loaded Definition, the file it came from, and their face already read.
#[derive(Debug, Clone)]
pub struct LoadedDefinition {
    pub definition: CharacterDefinition,
    pub path: PathBuf,
    /// The character's world sprite, resolved during load — the impure phase, done once.
    pub appearance: Option<Mark>,
    /// The image that identifies them, resolved the same way. `None` falls back to the sprite:
    /// the right person drawn imperfectly beats a blank, and beats an invented face outright.
    pub icon: Option<Mark>,
    /// One resolved sheet per action they have been drawn doing.
    ///
    /// Empty is the common answer and a complete one: an action nobody drew falls back to the
    /// still sprite, and a character with no sprite either falls back to the visible stand-in.
    pub actions: BTreeMap<epoch_kernel::Action, Mark>,
    /// What was wrong with this character that did not stop them loading: an unreadable
    /// sprite, mainly. Reported rather than silently swallowed — a face that vanished with no
    /// explanation is the kind of thing that gets blamed on the renderer for a week.
    pub problems: Vec<String>,
    modified: Option<SystemTime>,
}

/// Definitions currently available, and any files that failed to load.
///
/// A broken file never stops the engine: it is reported and skipped, and the rest of the
/// world carries on.
#[derive(Debug, Default)]
pub struct DefinitionRegistry {
    root: PathBuf,
    /// Ordered by id, so every listing this feeds is stable across machines and runs.
    characters: BTreeMap<CharacterId, LoadedDefinition>,
    problems: Vec<String>,
    /// How many `.toml` files the last load saw.
    ///
    /// Counted rather than inferred from `characters + problems`: one character can now
    /// contribute several problems, or load fine *and* report one. Inferring it would make
    /// `changed_on_disk` permanently true and the World would reload itself forever.
    files_seen: usize,
}

impl DefinitionRegistry {
    /// Load every character definition under `<root>/characters`.
    pub fn load(root: impl Into<PathBuf>) -> Self {
        let mut registry = Self {
            root: root.into(),
            characters: BTreeMap::new(),
            problems: Vec::new(),
            files_seen: 0,
        };
        registry.reload();
        registry
    }

    /// The folder every character — and every character's artwork — lives in.
    pub fn characters_dir(&self) -> PathBuf {
        self.root.join("characters")
    }

    /// Re-read every definition file, replacing what is held.
    pub fn reload(&mut self) {
        self.characters.clear();
        self.problems.clear();
        self.files_seen = 0;

        let dir = self.characters_dir();
        let paths = match authored_files(&dir) {
            Ok(paths) => paths,
            Err(problem) => {
                self.problems.push(problem);
                return;
            }
        };
        self.files_seen = paths.len();

        for path in paths {
            match load_one(&path, &dir) {
                Ok(mut loaded) => {
                    let id = loaded.definition.id.clone();
                    self.problems.append(&mut loaded.problems);
                    if let Some(first) = self.characters.get(&id) {
                        self.problems.push(
                            DefinitionError::DuplicateId {
                                id,
                                first: first.path.clone(),
                                second: loaded.path,
                            }
                            .to_string(),
                        );
                        continue;
                    }
                    self.characters.insert(id, loaded);
                }
                Err(err) => self.problems.push(err.to_string()),
            }
        }
    }

    /// True when any definition file changed on disk since it was read.
    ///
    /// Cheap enough to call on every simulation tick.
    pub fn changed_on_disk(&self) -> bool {
        // A file appearing or disappearing also counts as a change.
        let on_disk = std::fs::read_dir(self.characters_dir())
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("toml"))
                    .count()
            })
            .unwrap_or(0);
        if on_disk != self.files_seen {
            return true;
        }

        self.characters.values().any(|loaded| {
            let current = std::fs::metadata(&loaded.path)
                .and_then(|m| m.modified())
                .ok();
            current != loaded.modified
        })
    }

    /// Everyone, in id order.
    pub fn characters(&self) -> impl Iterator<Item = &CharacterDefinition> {
        self.characters.values().map(|l| &l.definition)
    }

    /// Everyone, with the file and face that came with them.
    pub fn loaded(&self) -> impl Iterator<Item = &LoadedDefinition> {
        self.characters.values()
    }

    pub fn character(&self, id: &CharacterId) -> Option<&CharacterDefinition> {
        self.characters.get(id).map(|l| &l.definition)
    }

    /// How a character looks in the World, already resolved. `None` is honest: nobody gave
    /// them a face.
    pub fn appearance(&self, id: &CharacterId) -> Option<&Mark> {
        self.characters.get(id).and_then(|l| l.appearance.as_ref())
    }

    /// Give a character one of their two images, or take it away.
    ///
    /// The bytes arrive from the surface and **the Engine names the file** from what they
    /// actually are, never from what they were called (ADR-0024). The two slots are separate
    /// files under separate stems, so replacing one cannot disturb the other.
    /// How tall this character stands, relative to a Place's footprint.
    ///
    /// A **scale**, never a resize: the file the user imported is never rewritten, so making
    /// somebody smaller and larger again costs nothing and loses nothing. `None` puts them back
    /// to the height everybody else stands at.
    ///
    /// Refused outside a sane range rather than clamped silently — somebody who typed `100`
    /// meant something, and a character who quietly became 3× instead would look like a bug in
    /// the slider.
    pub fn set_scale(
        &mut self,
        id: &CharacterId,
        scale: Option<f64>,
    ) -> Result<(), DefinitionError> {
        if let Some(s) = scale {
            if !(0.05..=3.0).contains(&s) {
                return Err(DefinitionError::BadParameter {
                    path: PathBuf::from(format!("{id}.toml")),
                    reason: format!("a scale of {s} is outside 0.05 to 3.0"),
                });
            }
        }
        let mut definition = self
            .character(id)
            .cloned()
            .ok_or_else(|| DefinitionError::Unknown(id.clone()))?;

        // Only meaningful for somebody who has artwork. Setting a size for a character with no
        // sprite would store a number that describes nothing.
        if let Some(appearance) = definition.appearance.as_mut() {
            appearance.scale = scale.unwrap_or(epoch_kernel::CHARACTER_SCALE);
        }
        self.save(definition)
    }

    /// Give a character one of their drawings, or take it away.
    ///
    /// `cut` says how to read a sheet, and it belongs to action slots only: the still sprite
    /// and the icon are pictures. Refused rather than ignored in both directions — a surface
    /// that sent the wrong pair should hear about it, and silently dropping a cut would leave
    /// a sheet on disk that draws as a strip.
    pub fn set_art(
        &mut self,
        id: &CharacterId,
        slot: ArtSlot,
        image: Option<&str>,
        cut: Option<epoch_kernel::Sheet>,
    ) -> Result<(), DefinitionError> {
        let refuse = |reason: String| DefinitionError::BadParameter {
            path: PathBuf::from(format!("{id}.toml")),
            reason,
        };
        match (slot, &cut, image.is_some()) {
            (ArtSlot::Doing(_), None, true) => {
                return Err(refuse(
                    "an action is drawn as a sheet, and a sheet needs a cut".into(),
                ))
            }
            (ArtSlot::Sprite | ArtSlot::Icon, Some(_), _) => {
                return Err(refuse(format!(
                    "the {} is one picture; it has no frames to cut",
                    slot.id()
                )))
            }
            _ => {}
        }

        let mut definition = self
            .character(id)
            .ok_or_else(|| DefinitionError::Unknown(id.clone()))?
            .clone();

        let dir = self.characters_dir();
        // Every slot lives under its own stem, so the drawings never collide and clearing one
        // cannot delete another's file.
        let stem = match slot {
            ArtSlot::Sprite => id.to_string(),
            ArtSlot::Icon => format!("{id}-icon"),
            ArtSlot::Doing(action) => format!("{id}-{}", action.id()),
        };

        let file =
            match image {
                Some(data) => Some(crate::import::accept_image(&dir, &stem, data).map_err(
                    |err| DefinitionError::Write {
                        path: dir.join(&stem),
                        source: std::io::Error::other(err.to_string()),
                    },
                )?),
                None => {
                    crate::import::forget_image(&dir, &stem);
                    None
                }
            };

        match (&mut definition.appearance, file) {
            (Some(appearance), file) => {
                match (slot, file) {
                    (ArtSlot::Sprite, file) => appearance.sprite = file,
                    (ArtSlot::Icon, file) => appearance.icon = file,
                    (ArtSlot::Doing(action), Some(file)) => {
                        appearance.actions.insert(
                            action.id().to_owned(),
                            epoch_kernel::ActionArt {
                                file,
                                cut: cut.unwrap_or_default(),
                            },
                        );
                    }
                    (ArtSlot::Doing(action), None) => {
                        appearance.actions.remove(action.id());
                    }
                }
                // Nothing left at all: they have no artwork, and saying so beats an empty table.
                if appearance.sprite.is_none()
                    && appearance.icon.is_none()
                    && appearance.actions.is_empty()
                {
                    definition.appearance = None;
                }
            }
            (None, Some(file)) => {
                let mut appearance = epoch_kernel::Appearance {
                    sprite: None,
                    icon: None,
                    // The height everybody already stands at, not 1.0 — a new arrival should
                    // not tower over the crew because nobody has told them how big to be.
                    scale: epoch_kernel::CHARACTER_SCALE,
                    anchor: [0.5, 1.0],
                    actions: BTreeMap::new(),
                };
                match slot {
                    ArtSlot::Sprite => appearance.sprite = Some(file),
                    ArtSlot::Icon => appearance.icon = Some(file),
                    ArtSlot::Doing(action) => {
                        appearance.actions.insert(
                            action.id().to_owned(),
                            epoch_kernel::ActionArt {
                                file,
                                cut: cut.unwrap_or_default(),
                            },
                        );
                    }
                }
                definition.appearance = Some(appearance);
            }
            // Clearing what was never there. Not an error — the outcome the caller wanted
            // already holds.
            (None, None) => {}
        }

        self.save(definition)
    }

    /// Change how an action's sheet is read, without touching the sheet.
    ///
    /// The editor's real loop: a mis-cut sheet is discovered by *watching it*, and the fix is
    /// two numbers rather than the same PNG uploaded again. Separate from
    /// [`set_art`](Self::set_art) because it is a different act — nothing is imported, nothing
    /// is named, and no file is written or removed.
    ///
    /// Refused for an action nobody has drawn: a cut describes artwork, and one stored for a
    /// sheet that does not exist would describe nothing.
    pub fn set_cut(
        &mut self,
        id: &CharacterId,
        action: epoch_kernel::Action,
        cut: epoch_kernel::Sheet,
    ) -> Result<(), DefinitionError> {
        let mut definition = self
            .character(id)
            .ok_or_else(|| DefinitionError::Unknown(id.clone()))?
            .clone();

        let art = definition
            .appearance
            .as_mut()
            .and_then(|a| a.actions.get_mut(action.id()))
            .ok_or_else(|| DefinitionError::BadParameter {
                path: PathBuf::from(format!("{id}.toml")),
                reason: format!("nobody has drawn them {}", action.id()),
            })?;
        art.cut = cut;

        self.save(definition)
    }

    /// **What removing this character would do**, before anything is touched.
    ///
    /// Two steps on purpose: this describes, the caller shows it, the user accepts, and only
    /// then does [`Removal::carry_out`] happen. A warning that only says *"are you sure?"* is a
    /// warning about nothing.
    ///
    /// `None` when there is nobody by that name.
    pub fn removal_of(&self, id: &CharacterId) -> Option<crate::erase::Removal> {
        let who = self.character(id)?;
        let mut files = vec![self.characters_dir().join(format!("{id}.toml"))];
        files.extend(crate::erase::artwork_of(
            &self.characters_dir(),
            id.as_str(),
        ));

        let mut consequences = Vec::new();
        let worlds: Vec<&str> = who.worlds.keys().map(String::as_str).collect();
        if !worlds.is_empty() {
            consequences.push(format!(
                "{} stops living in {}.",
                who.name,
                worlds.join(", ")
            ));
        }

        Some(crate::erase::Removal {
            what: who.name.clone(),
            files,
            consequences,
            // **Said out loud, because it will look like a mistake otherwise.** History records
            // what happened and is not rewritten by somebody leaving (ADR-0025), so their name
            // stays in every Quest they worked on. A person who deleted somebody and then found
            // them in a conversation would reasonably think the deletion failed.
            survives: vec![
                format!(
                    "Their work stays in History. {} is still named in the Quests they took part in.",
                    who.name
                ),
                "Any Quest still open that was theirs stays open — hand it to somebody else."
                    .to_owned(),
            ],
        })
    }

    /// Remove a character: their file, and the artwork Epoch named for them.
    ///
    /// **Nothing else.** Not their Quests, not the Worlds they lived in, not another character
    /// whose name merely begins the same way (see [`crate::erase::artwork_of`]).
    ///
    /// Returns whatever could not be removed, each with its reason. An already-missing file is
    /// not one of them.
    pub fn remove(&mut self, id: &CharacterId) -> Result<Vec<String>, DefinitionError> {
        let plan = self
            .removal_of(id)
            .ok_or_else(|| DefinitionError::Unknown(id.clone()))?;
        let problems = plan.carry_out();
        self.reload();
        Ok(problems)
    }

    /// How a character is identified — in a conversation, in a roster, in a list.
    ///
    /// Falls back to the sprite when no icon is authored, so a character with one drawing is
    /// still recognisable everywhere rather than only in the World.
    pub fn icon(&self, id: &CharacterId) -> Option<&Mark> {
        self.characters
            .get(id)
            .and_then(|l| l.icon.as_ref().or(l.appearance.as_ref()))
    }

    /// Everyone who currently lives in the given World.
    ///
    /// The roster, read from the side that owns it. A World with nobody in it is a valid and
    /// honest answer — the World still opens, and says it is empty.
    pub fn living_in<'a>(
        &'a self,
        world_id: &'a str,
    ) -> impl Iterator<Item = &'a CharacterDefinition> {
        self.characters
            .values()
            .map(|l| &l.definition)
            .filter(move |d| d.lives_in(world_id))
    }

    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    pub fn is_empty(&self) -> bool {
        self.characters.is_empty()
    }

    /// Write a character back to their own file, then re-read the vault.
    ///
    /// The whole file is rewritten rather than patched line by line. A World Pack is authored
    /// content whose comments belong to its author — a character file is the user's own data,
    /// edited through the Launcher, and round-tripping it whole is both simpler and the only
    /// version that cannot drift from the struct.
    pub fn save(&mut self, definition: CharacterDefinition) -> Result<(), DefinitionError> {
        let id = definition.id.clone();
        let body =
            toml::to_string_pretty(&definition).map_err(|source| DefinitionError::Serialize {
                id: id.clone(),
                source,
            })?;

        let dir = self.characters_dir();
        std::fs::create_dir_all(&dir).map_err(|source| DefinitionError::Write {
            path: dir.clone(),
            source,
        })?;

        let path = dir.join(format!("{id}.toml"));
        std::fs::write(&path, body).map_err(|source| DefinitionError::Write {
            path: path.clone(),
            source,
        })?;

        self.reload();
        Ok(())
    }

    /// Move a character into or out of a World.
    ///
    /// Idempotent, and ordered: the roster is sorted so the same set of Worlds always
    /// produces the same file, and a save never shows up as a spurious change.
    pub fn set_world(
        &mut self,
        id: &CharacterId,
        world_id: &str,
        lives_there: bool,
    ) -> Result<(), DefinitionError> {
        let mut definition = self
            .character(id)
            .cloned()
            .ok_or_else(|| DefinitionError::Unknown(id.clone()))?;

        // Moving in keeps whatever residence was already recorded — leaving a World and
        // coming back should not silently forget which building was theirs. Moving out drops
        // it, because a home in a World you no longer live in is not a fact about anybody.
        if lives_there {
            definition.worlds.entry(world_id.to_owned()).or_default();
        } else {
            definition.worlds.remove(world_id);
        }

        self.save(definition)
    }

    /// Say which building somebody lives in, in one World.
    ///
    /// Per World, because a `PlaceId` only means anything inside one (ADR-0028) — and it is
    /// written to the *character's* file, because the roster lives with the character
    /// (ADR-0023). Both halves of that are load-bearing: a shipped World Pack cannot know
    /// somebody the user invented, and "Mage lives in the Tower" is a fact about Mage.
    ///
    /// Moving somebody in is implied. Assigning a home in a World they do not live in and
    /// leaving them out of its roster would be a resident nobody can see, so the roster entry
    /// is created rather than the call refused.
    ///
    /// `None` clears it: they live in this World and it is not settled where. That is a real
    /// state, and it falls back to the archetype's own home the way it always did.
    pub fn set_home(
        &mut self,
        id: &CharacterId,
        world_id: &str,
        home: Option<PlaceId>,
    ) -> Result<(), DefinitionError> {
        let mut definition = self
            .character(id)
            .cloned()
            .ok_or_else(|| DefinitionError::Unknown(id.clone()))?;

        definition
            .worlds
            .entry(world_id.to_owned())
            .or_default()
            .home = home;
        self.save(definition)
    }

    /// Say where one of somebody's routine activities happens, in this World.
    ///
    /// The per-World half of a routine (ADR-0028): the activity is theirs and travels with them,
    /// the building is this World's. `None` puts it back to happening at home, and removes the
    /// entry rather than storing an empty one — a file should say what somebody decided, not
    /// carry a record of every decision they undid.
    pub fn set_routine_place(
        &mut self,
        id: &CharacterId,
        world_id: &str,
        activity: &str,
        place: Option<PlaceId>,
    ) -> Result<(), DefinitionError> {
        let mut definition = self
            .character(id)
            .cloned()
            .ok_or_else(|| DefinitionError::Unknown(id.clone()))?;

        let residence = definition.worlds.entry(world_id.to_owned()).or_default();
        match place {
            Some(place) => {
                residence.at.insert(activity.to_owned(), place);
            }
            None => {
                residence.at.remove(activity);
            }
        }
        self.save(definition)
    }

    /// Give a character a capability, or take it away.
    ///
    /// **Only ever called from a user command.** A model asking for `write_file` produces a
    /// question for the user, never a call to this — the moment a character can grant itself a
    /// capability, "what may this character do" stops being the user's answer. That is the same
    /// boundary the Trust Engine draws around policies, one level up: trust is granted by
    /// people, never by content (ADR-0009).
    ///
    /// Writes the character's own file, so the grant travels with them into every World. That
    /// is deliberate: what somebody is *for* is not a per-World fact (ADR-0023). Where they may
    /// use it is, and that is the Trust Engine's question, asked separately every time.
    /// `available` is everything that can currently be asked for — this build's capabilities plus
    /// one entry per connected MCP server. Passed in because only the caller holds the bridge,
    /// and because starting from a list that cannot contain an outside tool is how one gets
    /// silently dropped the first time somebody ticks a box.
    ///
    /// `groups` says which of those are sources and what is inside each, because granting a
    /// source has to leave the file consistent: a `mcp:playwright` sitting beside three of its
    /// own tool ids is two answers to one question, and the next reader has to guess which won.
    pub fn set_requested(
        &mut self,
        id: &CharacterId,
        capability: &str,
        wanted: bool,
        available: &[String],
        groups: &[crate::capabilities::Group],
    ) -> Result<(), DefinitionError> {
        let requested = epoch_kernel::CapabilityRequest::new(capability).map_err(|why| {
            DefinitionError::BadParameter {
                path: self.characters_dir().join(format!("{id}.toml")),
                reason: why,
            }
        })?;

        let mut definition = self
            .character(id)
            .cloned()
            .ok_or_else(|| DefinitionError::Unknown(id.clone()))?;

        // Granting one thing is also the moment somebody decided. Starting from what they
        // already had — which for an unconfigured character is everything available — means a
        // single grant never silently takes the rest away.
        let mut chosen = definition.wants().cloned().unwrap_or_else(|| {
            available
                .iter()
                .filter_map(|name| epoch_kernel::CapabilityRequest::new(name).ok())
                .collect()
        });
        if wanted {
            chosen.insert(requested.clone());
        } else {
            chosen.remove(&requested);
        }
        // Granting or revoking a whole source settles everything it contains. Without this,
        // revoking `mcp:playwright` from a character who had also been granted one of its tools
        // individually — which a mid-turn grant can do — would take the source away and leave
        // the tool behind, so the box would read off while the tool still worked.
        if let Some(group) = groups.iter().find(|g| g.id == requested.as_str()) {
            chosen.retain(|held| !group.tools.iter().any(|tool| tool == held.as_str()));
        }
        definition.requested_capabilities = Some(chosen);

        self.save(definition)
    }

    /// Take capability ids off everyone who asked for them, because they no longer exist.
    ///
    /// Called when an outside source is removed. A request normally *survives* something being
    /// unavailable — a character says what it wants and only a resolved Provider says what is
    /// there (ADR-0026), so a broken connection coming back finds its grants waiting. That is
    /// right for a server that is not answering and wrong for one that is **gone**, because the
    /// id names a slot rather than a program: install a different server under the same name and
    /// the kept request, and any standing decision behind it, attaches to a different third
    /// party's tools.
    ///
    /// **An undecided character is left alone.** `None` means nobody has chosen, and it resolves
    /// to everything available — so there is nothing to remove, and writing a list here would
    /// turn "hasn't decided" into "decided on today's tools", which is a data-loss bug this
    /// codebase has already had once.
    ///
    /// Returns who changed, so the caller can say so.
    pub fn revoke_all(&mut self, ids: &[String]) -> Result<Vec<CharacterId>, DefinitionError> {
        let doomed: std::collections::BTreeSet<&str> = ids.iter().map(String::as_str).collect();

        let affected: Vec<CharacterDefinition> = self
            .characters()
            .filter(|d| {
                d.wants()
                    .is_some_and(|held| held.iter().any(|r| doomed.contains(r.as_str())))
            })
            .cloned()
            .collect();

        let mut changed = Vec::new();
        for mut definition in affected {
            if let Some(held) = definition.requested_capabilities.as_mut() {
                held.retain(|r| !doomed.contains(r.as_str()));
            }
            changed.push(definition.id.clone());
            self.save(definition)?;
        }
        Ok(changed)
    }
}

/// Every authored `.toml` in a directory, sorted — or the reason there are none.
///
/// **The whole of what two Definition types share** (ADR-0011). That ADR deferred a generic
/// `Definition` trait until a second type existed; [`epoch_kernel::SkillDefinition`] is that
/// type, and it showed the trait was the wrong seam. A dozen fields about a person and five
/// about a method have an id and a name in common and nothing else — while *this*, the part
/// nobody would have called a design, is identical in both.
///
/// Sorted before loading so which of two conflicting files is reported as the duplicate does not
/// depend on the operating system.
///
/// ## A folder that was never made is empty, not broken
///
/// Found by installing the real `.exe` and starting it: the first thing a new user's diagnostics
/// said was **HULL 1 FAULT LOGGED**, and the fault was
/// `cannot read …\vault\definitions\characters: The system cannot find the path specified`.
/// Nothing was wrong. There is no crew on a fresh vault — which the same deck says one line
/// above, `CREW POSTED 0 / 0`.
///
/// So `NotFound` answers *none*, and **every other refusal is still an error**: a directory that
/// exists and cannot be read is a real fault with a real fix, and collapsing the two would be
/// the opposite mistake. The same three-way discipline `Store::reachable` needed, for the same
/// reason.
///
/// Neither a development build nor a test could have shown this — both run against a vault that
/// has always existed.
pub fn authored_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(format!("cannot read {}: {source}", dir.display())),
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("toml"))
        .collect();
    paths.sort();
    Ok(paths)
}

fn load_one(path: &Path, characters_dir: &Path) -> Result<LoadedDefinition, DefinitionError> {
    let raw = std::fs::read_to_string(path).map_err(|source| DefinitionError::Io {
        path: path.to_path_buf(),
        source,
    })?;

    let definition: CharacterDefinition =
        toml::from_str(&raw).map_err(|source| DefinitionError::Parse {
            path: path.to_path_buf(),
            source: Box::new(source),
        })?;

    if path.file_stem().and_then(|s| s.to_str()) != Some(definition.id.as_str()) {
        return Err(DefinitionError::IdMismatch {
            path: path.to_path_buf(),
            id: definition.id,
        });
    }
    if definition.name.trim().is_empty() {
        return Err(DefinitionError::EmptyName {
            path: path.to_path_buf(),
        });
    }
    if definition.role.trim().is_empty() {
        return Err(DefinitionError::EmptyRole {
            path: path.to_path_buf(),
        });
    }
    if definition.presence.idle.is_empty() {
        return Err(DefinitionError::NoIdleBehavior {
            path: path.to_path_buf(),
        });
    }

    // Deliberately **not** checked here: whether the named backend exists.
    //
    // It used to be, against a hardcoded list of one. Two things made that wrong. Backends are
    // configured now (ADR-0026), so "the Ollama under the desk" is legitimately called `desk` and
    // would have been rejected as a typo. And a character travels between machines (ADR-0023) —
    // arriving somewhere its backend is not installed makes it *unable to think*, never invalid.
    // Refusing to load the file would have deleted somebody from the roster for being away from
    // home. The launcher already states this rule for editing; load now agrees with it.
    if let Some(mind) = &definition.mind {
        // **A provider needs a model; an agent does not.**
        //
        // This checked both, and the cost was the worst kind: Epoch wrote a file it then
        // refused to read, so choosing an agent for somebody made them *vanish from the
        // roster*. The comment directly above already states the principle — refusing to load
        // would have deleted somebody for being away from home — and this was the same mistake
        // one field over.
        //
        // A provider with no model cannot be asked anything, so that stays refused. An agent
        // with no model uses its own, which is what most people want and what the panel offers.
        if mind.provider().is_some() && mind.model().trim().is_empty() {
            return Err(DefinitionError::NoModel {
                path: path.to_path_buf(),
            });
        }
        // Canonical parameters are checked; provider-native tuning is not, because the Kernel
        // does not know what it means — the Provider that declared it does, and it validates
        // against its own surface (ADR-0026).
        if let Some(reason) = mind.parameters.problems().into_iter().next() {
            return Err(DefinitionError::BadParameter {
                path: path.to_path_buf(),
                reason,
            });
        }
    }

    // Resolve: read the artwork once, here, so everything downstream stays pure. A face that
    // cannot be read costs the face, never the character (ADR-0016 graceful degradation).
    let mut appearance = None;
    let mut icon = None;
    let mut actions = BTreeMap::new();
    let mut problems = Vec::new();
    if let Some(authored) = &definition.appearance {
        let (mark, found) = resolve_appearance(authored, characters_dir);
        appearance = mark;
        problems.extend(
            found
                .into_iter()
                .map(|p| format!("character '{}': {p}", definition.id)),
        );

        let (drawn, found) = resolve_actions(authored, characters_dir);
        actions = drawn;
        problems.extend(
            found
                .into_iter()
                .map(|p| format!("character '{}': {p}", definition.id)),
        );

        if let Some(file) = &authored.icon {
            let (mark, found) = resolve_icon(file, characters_dir);
            icon = mark;
            problems.extend(
                found
                    .into_iter()
                    .map(|p| format!("character '{}' icon: {p}", definition.id)),
            );
        }
    }

    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();

    Ok(LoadedDefinition {
        definition,
        path: path.to_path_buf(),
        appearance,
        icon,
        actions,
        problems,
        modified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use epoch_kernel::{CapabilityRequest, Reasoning, TuningValue};

    use crate::launcher::{CharacterEdit, ParametersEdit, RoutineStep};

    /// **A first run has no crew, and that is not a fault.**
    ///
    /// Found by installing the real `.exe`: a brand-new vault has no `definitions/characters`,
    /// and the launcher's diagnostics opened on `HULL 1 FAULT LOGGED` with `The system cannot
    /// find the path specified` beneath it. An absence reported as a failure, on the first
    /// screen somebody ever sees.
    #[test]
    fn a_folder_that_was_never_made_holds_nothing() {
        let never = std::env::temp_dir().join(format!("epoch-absent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&never);
        assert!(!never.exists(), "the fixture must not exist");
        assert_eq!(
            authored_files(&never).expect("a missing folder is empty, not broken"),
            Vec::<PathBuf>::new()
        );
    }

    /// And a folder that exists and cannot be read still is a fault.
    ///
    /// A file where a directory should be is the portable way to make `read_dir` refuse for a
    /// reason that is not `NotFound` — chmod is not, and this suite runs on two operating
    /// systems.
    #[test]
    fn something_that_is_not_a_folder_is_still_reported() {
        let dir = std::env::temp_dir().join(format!("epoch-notdir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::write(&dir, b"not a directory").expect("a file where a folder should be");
        let why = authored_files(&dir).expect_err("this one really cannot be read");
        assert!(why.starts_with("cannot read "), "{why}");
        let _ = std::fs::remove_file(&dir);
    }

    struct Vault(PathBuf);

    impl Vault {
        fn new(name: &str) -> Self {
            // The counter is not decoration. Two tests picked the same name, and because this
            // wipes the directory first, they deleted each other's vault mid-run — a failure
            // that looks like a logic bug in whichever one lost the race. A name can now be
            // reused without the tests being able to collide.
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("epoch-defs-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("characters")).unwrap();
            Self(dir)
        }
        fn write(&self, file: &str, body: &str) {
            std::fs::write(self.0.join("characters").join(file), body).unwrap();
        }
    }

    impl Drop for Vault {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn id(raw: &str) -> CharacterId {
        CharacterId::new(raw).unwrap()
    }

    const GOOD: &str = r#"
id = "mage"
name = "Mage"
archetype = "researcher"
role = "Turns goals into designs"
prompt = "You explore before committing."
worlds = ["default"]

[presence]
home = "research_lab"
idle = [
  { activity = "reading", seconds = 12 },
]
"#;

    /// One real PNG, one pixel. Enough for the import to sniff a format and name a file.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    fn walk_cut() -> epoch_kernel::Sheet {
        epoch_kernel::Sheet {
            columns: 4,
            rows: 4,
            count: 16,
            milliseconds: 120,
            directions: vec!["south".into(), "west".into(), "east".into(), "north".into()],
        }
    }

    #[test]
    fn removing_somebody_takes_their_file_and_their_art_and_stops_there() {
        let vault = Vault::new("remove");
        vault.write("mage.toml", GOOD);
        // Somebody whose name begins the same way. The one thing this must not do.
        vault.write(
            "mage-of-the-north.toml",
            &GOOD.replace("mage", "mage-of-the-north"),
        );
        let mut registry = DefinitionRegistry::load(&vault.0);
        let mage = CharacterId::new("mage").unwrap();
        let neighbour = CharacterId::new("mage-of-the-north").unwrap();

        registry
            .set_art(&mage, ArtSlot::Sprite, Some(PNG), None)
            .unwrap();
        registry
            .set_art(
                &mage,
                ArtSlot::Doing(epoch_kernel::Action::Walk),
                Some(PNG),
                Some(walk_cut()),
            )
            .unwrap();
        registry
            .set_art(&neighbour, ArtSlot::Sprite, Some(PNG), None)
            .unwrap();

        // **The plan says what it will do**, and it is what a confirmation shows.
        let plan = registry.removal_of(&mage).expect("she is here");
        assert_eq!(plan.what, "Mage");
        assert_eq!(plan.files.len(), 3, "the file, the sprite, the walk sheet");
        assert!(
            plan.survives.iter().any(|line| line.contains("History")),
            "a deletion must say what it keeps: {:?}",
            plan.survives
        );

        assert!(registry.remove(&mage).unwrap().is_empty());

        assert!(registry.character(&mage).is_none());
        assert!(
            registry.character(&neighbour).is_some(),
            "a name that merely begins the same way is a different person"
        );
        assert!(
            vault
                .0
                .join("characters")
                .join("mage-of-the-north.png")
                .exists(),
            "and so is their artwork"
        );
    }

    #[test]
    fn removing_nobody_is_an_error_rather_than_a_quiet_success() {
        let vault = Vault::new("remove-nobody");
        let mut registry = DefinitionRegistry::load(&vault.0);
        assert!(registry
            .remove(&CharacterId::new("ghost").unwrap())
            .is_err());
    }

    #[test]
    fn a_sheet_lands_in_its_own_slot_and_survives_the_file() {
        let vault = Vault::new("sheets");
        vault.write("mage.toml", GOOD);
        let mut registry = DefinitionRegistry::load(&vault.0);
        let mage = CharacterId::new("mage").unwrap();

        registry
            .set_art(
                &mage,
                ArtSlot::Doing(epoch_kernel::Action::Walk),
                Some(PNG),
                Some(walk_cut()),
            )
            .expect("a walk sheet");

        // Read back off disk rather than out of memory: the point of a vault is that it is
        // what the next session opens.
        let reread = DefinitionRegistry::load(&vault.0);
        let appearance = reread
            .character(&mage)
            .expect("still here")
            .appearance
            .as_ref()
            .expect("artwork");
        let art = appearance.actions.get("walk").expect("a walk");
        assert_eq!(art.cut, walk_cut());
        // Named from the bytes, under this slot's own stem — so no two drawings collide and
        // clearing one cannot delete another (ADR-0024).
        assert!(art.file.starts_with("mage-walk"), "{}", art.file);
        assert!(
            appearance.sprite.is_none(),
            "a sheet is not the still picture"
        );

        // And it resolved: the sheet reached a Mark with a cut, through the same pipeline a
        // Place's artwork travels.
        let loaded = reread.loaded().find(|l| l.definition.id == mage).unwrap();
        let mark = loaded
            .actions
            .get(&epoch_kernel::Action::Walk)
            .expect("resolved");
        assert_eq!(mark.frames.as_ref().expect("a cut").columns, 4);
        assert!(mark.asset_data.is_some());
    }

    #[test]
    fn clearing_one_drawing_leaves_the_others_alone() {
        let vault = Vault::new("slots");
        vault.write("mage.toml", GOOD);
        let mut registry = DefinitionRegistry::load(&vault.0);
        let mage = CharacterId::new("mage").unwrap();

        registry
            .set_art(&mage, ArtSlot::Sprite, Some(PNG), None)
            .unwrap();
        registry
            .set_art(
                &mage,
                ArtSlot::Doing(epoch_kernel::Action::Walk),
                Some(PNG),
                Some(walk_cut()),
            )
            .unwrap();
        registry
            .set_art(
                &mage,
                ArtSlot::Doing(epoch_kernel::Action::Walk),
                None,
                None,
            )
            .unwrap();

        let appearance = registry
            .character(&mage)
            .unwrap()
            .appearance
            .as_ref()
            .expect("the sprite is still there");
        assert!(appearance.actions.is_empty());
        assert!(appearance.sprite.is_some());
    }

    #[test]
    fn a_cut_can_be_fixed_without_importing_the_sheet_again() {
        // The editor's real loop: state a grid, import, watch it, fix the grid. A typo in
        // `rows` is two numbers, not the same PNG uploaded a second time.
        let vault = Vault::new("recut");
        vault.write("mage.toml", GOOD);
        let mut registry = DefinitionRegistry::load(&vault.0);
        let mage = CharacterId::new("mage").unwrap();

        registry
            .set_art(
                &mage,
                ArtSlot::Doing(epoch_kernel::Action::Walk),
                Some(PNG),
                Some(walk_cut()),
            )
            .unwrap();
        let named = registry
            .character(&mage)
            .unwrap()
            .appearance
            .as_ref()
            .unwrap()
            .actions["walk"]
            .file
            .clone();

        registry
            .set_cut(
                &mage,
                epoch_kernel::Action::Walk,
                epoch_kernel::Sheet {
                    milliseconds: 90,
                    ..walk_cut()
                },
            )
            .unwrap();

        let art = &DefinitionRegistry::load(&vault.0)
            .character(&mage)
            .unwrap()
            .appearance
            .as_ref()
            .unwrap()
            .actions["walk"]
            .clone();
        assert_eq!(art.cut.milliseconds, 90);
        // The same file, untouched: nothing was imported and nothing was renamed.
        assert_eq!(art.file, named);
    }

    #[test]
    fn a_cut_for_something_nobody_drew_is_refused() {
        // A cut describes artwork. One stored for a sheet that does not exist describes
        // nothing, and would sit in the file waiting to disagree with whatever arrived later.
        let vault = Vault::new("recut-nothing");
        vault.write("mage.toml", GOOD);
        let mut registry = DefinitionRegistry::load(&vault.0);
        let refused = registry.set_cut(
            &CharacterId::new("mage").unwrap(),
            epoch_kernel::Action::Walk,
            walk_cut(),
        );
        assert!(refused.is_err());
    }

    #[test]
    fn a_sheet_with_no_cut_is_refused_rather_than_written() {
        // The rule ADR-0021 states about marks, enforced at the door instead of discovered at
        // load: a sheet nobody can cut draws as a strip, so it never reaches the vault.
        let vault = Vault::new("nocut");
        vault.write("mage.toml", GOOD);
        let mut registry = DefinitionRegistry::load(&vault.0);
        let mage = CharacterId::new("mage").unwrap();

        let refused = registry.set_art(
            &mage,
            ArtSlot::Doing(epoch_kernel::Action::Walk),
            Some(PNG),
            None,
        );
        assert!(refused.is_err());
        assert!(registry.character(&mage).unwrap().appearance.is_none());

        // And the other direction: a picture has no frames to cut. Refused rather than
        // ignored, because a surface that sent the wrong pair should hear about it.
        assert!(registry
            .set_art(&mage, ArtSlot::Icon, Some(PNG), Some(walk_cut()))
            .is_err());
    }

    #[test]
    fn an_action_this_build_does_not_know_costs_that_entry_and_nothing_else() {
        // A file written by a later Epoch must still open in this one, which is the whole
        // reason the map is keyed by string on disk.
        let vault = Vault::new("future");
        vault.write(
            "mage.toml",
            &format!(
                "{GOOD}
[appearance]
sprite = \"mage.png\"

                 [appearance.actions.dancing]
file = \"d.png\"
columns = 2
rows = 1
                 milliseconds = 100
"
            ),
        );
        let registry = DefinitionRegistry::load(&vault.0);
        let loaded = registry
            .loaded()
            .find(|l| l.definition.id.as_str() == "mage")
            .expect("the character still loads");
        // The word survives the round trip in the file — a later Epoch's animation is not
        // deleted by opening the vault in this one — and it simply resolves to nothing here.
        assert!(loaded
            .definition
            .appearance
            .as_ref()
            .unwrap()
            .actions
            .contains_key("dancing"));
        assert!(loaded.actions.is_empty());
        assert!(
            registry.problems().iter().any(|p| p.contains("dancing")),
            "{:?}",
            registry.problems()
        );
    }

    fn playwright() -> crate::capabilities::Group {
        crate::capabilities::Group {
            id: "mcp:playwright".into(),
            label: "Playwright".into(),
            tools: vec!["playwright_click".into(), "playwright_close".into()],
            answering: true,
        }
    }

    fn wanted_by(reg: &DefinitionRegistry, who: &str) -> Vec<String> {
        reg.character(&id(who))
            .unwrap()
            .wants()
            .expect("saving is deciding")
            .iter()
            .map(|r| r.as_str().to_owned())
            .collect()
    }

    #[test]
    fn granting_a_source_writes_the_source_not_the_tools_it_has_today() {
        let v = Vault::new("grant-source");
        v.write("mage.toml", GOOD);
        let mut reg = DefinitionRegistry::load(&v.0);

        let available = vec!["read_file".to_string(), "mcp:playwright".to_string()];
        reg.set_requested(
            &id("mage"),
            "mcp:playwright",
            true,
            &available,
            &[playwright()],
        )
        .unwrap();

        // What is written is the source. Writing today's two tool ids would freeze the grant on
        // the day the box was ticked, and nothing would say so when a third arrived.
        assert_eq!(
            wanted_by(&reg, "mage"),
            vec!["mcp:playwright", "read_file"],
            "an undecided character keeps everything else; the tick adds, never subtracts"
        );
    }

    #[test]
    fn revoking_a_source_takes_its_tools_with_it() {
        // A mid-turn grant can put a single tool id in the file — that is the escape hatch, and
        // it is honest. What it must not do is survive the source being switched off, or the box
        // would read "off" while the tool still worked.
        let v = Vault::new("revoke-source");
        v.write("mage.toml", GOOD);
        let mut reg = DefinitionRegistry::load(&v.0);
        let available = vec!["read_file".to_string(), "mcp:playwright".to_string()];

        reg.set_requested(
            &id("mage"),
            "playwright_click",
            true,
            &available,
            &[playwright()],
        )
        .unwrap();
        assert!(wanted_by(&reg, "mage").contains(&"playwright_click".to_string()));

        reg.set_requested(
            &id("mage"),
            "mcp:playwright",
            false,
            &available,
            &[playwright()],
        )
        .unwrap();
        assert_eq!(wanted_by(&reg, "mage"), vec!["read_file"]);
    }

    #[test]
    fn loads_a_valid_definition() {
        let v = Vault::new("good");
        v.write("mage.toml", GOOD);
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.problems().is_empty(), "{:?}", reg.problems());
        let d = reg.character(&id("mage")).unwrap();
        assert_eq!(d.name, "Mage");
        assert_eq!(d.role, "Turns goals into designs");
        assert_eq!(d.presence.idle.len(), 1);
    }

    #[test]
    fn two_characters_may_share_an_archetype() {
        // ADR-0023: the archetype classifies, the id identifies. Before this, the second
        // file would silently have replaced the first.
        let v = Vault::new("two");
        v.write("mage.toml", GOOD);
        v.write(
            "paladin.toml",
            &GOOD.replace("mage", "paladin").replace("Mage", "Paladin"),
        );
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.problems().is_empty(), "{:?}", reg.problems());
        assert_eq!(reg.characters().count(), 2);
    }

    /// The whole of ADR-0026 in one authored file: canonical parameters, provider-native
    /// tuning under its own namespace, and capabilities asked for rather than claimed.
    const TUNED: &str = r#"
id = "mage"
name = "Mage"
archetype = "researcher"
role = "Turns goals into designs"
prompt = "You explore before committing."
worlds = ["default"]
requested_capabilities = ["web_search", "code"]

[mind]
provider = "ollama"
model = "qwen3:14b"

[mind.parameters]
temperature = 0.7
reasoning = "high"

[mind.tuning.ollama]
num_ctx = 32768
repeat_penalty = 1.1

[mind.tuning.anthropic]
something_else = true

[presence]
home = "research_lab"
idle = [
  { activity = "reading", seconds = 12 },
]
"#;

    #[test]
    fn a_character_carries_canonical_parameters_and_provider_native_tuning() {
        let v = Vault::new("tuned");
        v.write("mage.toml", TUNED);
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.problems().is_empty(), "{:?}", reg.problems());

        let d = reg.character(&id("mage")).unwrap();
        let mind = d.mind.as_ref().unwrap();
        assert_eq!(mind.parameters.temperature, Some(0.7));
        assert_eq!(mind.parameters.reasoning, Some(Reasoning::High));
        // Unset stays unset: the Provider's own default, not a number Epoch invented.
        assert_eq!(mind.parameters.top_p, None);

        // `num_ctx` is Ollama's word, held in Ollama's namespace, never as a canonical field.
        assert_eq!(
            mind.active_tuning().and_then(|t| t.get("num_ctx")),
            Some(&TuningValue::Int(32768))
        );
        assert_eq!(mind.dormant_tuning().collect::<Vec<_>>(), vec!["anthropic"]);

        // Requested, not declared: this says what Mage wants, never what the model can do.
        // Authored, so it is a decision — and only what was written is asked for.
        assert!(d.capabilities_decided());
        let chosen = d.wants().expect("authored, so decided");
        assert!(chosen.contains(&CapabilityRequest::new("web_search").unwrap()));
        assert!(!chosen.contains(&CapabilityRequest::new("vision").unwrap()));
    }

    /// A machine with the one backend these tests assign, and nothing to tune.
    fn ollama_machine() -> crate::launcher::Machine {
        crate::launcher::Machine {
            offers: vec!["ollama".into()],
            window: None,
            controls: Vec::new(),
            agents: Vec::new(),
        }
    }

    #[test]
    fn provider_native_tuning_survives_a_save_that_cannot_see_it() {
        // The form edits canonical parameters and requested capabilities. It has no field for
        // provider-native tuning — that needs the Provider-declared surface — so the tuning is
        // carried through untouched. A surface that cannot show something must never be the
        // reason it is lost.
        let v = Vault::new("roundtrip");
        v.write("mage.toml", TUNED);
        let mut reg = DefinitionRegistry::load(&v.0);
        let before = reg.character(&id("mage")).unwrap().clone();

        let edit = CharacterEdit {
            id: "mage".into(),
            name: "Mage the Elder".into(),
            archetype: "researcher".into(),
            role: before.role.clone(),
            prompt: before.prompt.clone(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: Some("ollama".into()),
            agent: None,
            model: Some("qwen3:14b".into()),
            // What the form does carry: the user lowers the temperature and drops a request.
            parameters: ParametersEdit {
                temperature: Some(0.2),
                top_p: None,
                context_tokens: None,
                context_policy: None,
                reasoning: Some("high".into()),
            },
            requested_capabilities: Some(vec!["code".into()]),
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        edit.apply(&mut reg, &ollama_machine()).unwrap();

        let after = DefinitionRegistry::load(&v.0);
        let d = after.character(&id("mage")).unwrap();
        let mind = d.mind.as_ref().unwrap();
        assert_eq!(d.name, "Mage the Elder");
        assert_eq!(mind.parameters.temperature, Some(0.2));
        assert_eq!(mind.parameters.reasoning, Some(Reasoning::High));
        assert_eq!(d.wants().expect("decided").iter().count(), 1);

        // Untouched, both the active namespace and the dormant one.
        assert_eq!(
            mind.active_tuning(),
            before.mind.as_ref().unwrap().active_tuning()
        );
        assert_eq!(mind.dormant_tuning().collect::<Vec<_>>(), vec!["anthropic"]);
    }

    /// A voice is chosen, kept, and cleared — and what the form does not manage survives.
    ///
    /// **The second half is the one that found a defect.** `draws_in` was written as `None` on
    /// every save, with a comment about nobody having an opinion until they say so — true of
    /// *creating* a character and false of *editing* one. No form manages it, so renaming
    /// somebody silently threw away a preference set by hand. The same shape this file already
    /// records for Skills and for `mcp.toml`.
    #[test]
    fn a_voice_is_edited_and_a_preference_nobody_edits_is_not_destroyed() {
        let v = Vault::new("voice");
        v.write("mage.toml", TUNED);
        let mut reg = DefinitionRegistry::load(&v.0);

        // Set by hand in the vault, the only way it can be set today.
        let mut hand = reg.character(&id("mage")).unwrap().clone();
        hand.draws_in = Some("pixel art".into());
        reg.save(hand).unwrap();

        let plain = |reg: &DefinitionRegistry| {
            let before = reg.character(&id("mage")).unwrap();
            CharacterEdit {
                id: "mage".into(),
                name: before.name.clone(),
                archetype: "researcher".into(),
                role: before.role.clone(),
                prompt: before.prompt.clone(),
                home: None,
                // Never empty: a character is never frozen (Build From Life, rule 2), and the
                // form refuses one who is.
                routine: vec![RoutineStep {
                    activity: "reading".into(),
                    seconds: 12,
                }],
                provider: Some("ollama".into()),
                agent: None,
                model: Some("qwen3:14b".into()),
                parameters: ParametersEdit::default(),
                requested_capabilities: None,
                skills: None,
                speaks_with: None,
                sounds_like: None,
                tuning: Default::default(),
            }
        };

        // A form that says nothing about voices leaves both alone.
        plain(&reg).apply(&mut reg, &ollama_machine()).unwrap();
        let after = DefinitionRegistry::load(&v.0);
        let d = after.character(&id("mage")).unwrap();
        assert_eq!(d.speaks_with, None);
        assert_eq!(
            d.draws_in.as_deref(),
            Some("pixel art"),
            "a preference no form manages must survive an edit to the name"
        );

        // Choosing one.
        let mut reg = DefinitionRegistry::load(&v.0);
        let mut edit = plain(&reg);
        edit.speaks_with = Some("es_ES-davefx-medium".into());
        edit.apply(&mut reg, &ollama_machine()).unwrap();
        let after = DefinitionRegistry::load(&v.0);
        assert_eq!(
            after.character(&id("mage")).unwrap().speaks_with.as_deref(),
            Some("es_ES-davefx-medium")
        );

        // And the empty option is a real choice: silence, said on purpose.
        let mut reg = DefinitionRegistry::load(&v.0);
        let mut edit = plain(&reg);
        edit.speaks_with = Some("  ".into());
        edit.apply(&mut reg, &ollama_machine()).unwrap();
        let after = DefinitionRegistry::load(&v.0);
        assert_eq!(after.character(&id("mage")).unwrap().speaks_with, None);
    }

    #[test]
    fn the_form_cannot_write_a_parameter_the_loader_would_reject() {
        // Refused at the edit, so the file on disk never reaches a state that fails to load.
        let v = Vault::new("badedit");
        v.write("mage.toml", TUNED);
        let mut reg = DefinitionRegistry::load(&v.0);

        let mut edit = CharacterEdit {
            id: "mage".into(),
            name: "Mage".into(),
            archetype: "researcher".into(),
            role: "r".into(),
            prompt: String::new(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: Some("ollama".into()),
            agent: None,
            model: Some("qwen3:14b".into()),
            parameters: ParametersEdit {
                temperature: Some(9.0),
                top_p: None,
                context_tokens: None,
                context_policy: None,
                reasoning: None,
            },
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        assert!(edit
            .clone()
            .apply(&mut reg, &ollama_machine())
            .unwrap_err()
            .contains("temperature"));

        // And a reasoning level nobody declared is refused rather than stored as text.
        edit.parameters.temperature = None;
        edit.parameters.reasoning = Some("ultra".into());
        assert!(edit
            .apply(&mut reg, &ollama_machine())
            .unwrap_err()
            .contains("ultra"));
    }

    #[test]
    fn a_backend_this_machine_does_not_have_is_refused_when_editing_here() {
        // The user picked from a list, so a name that is not on it is a typo. Deliberately a
        // rule about *editing*, never about loading: a character travels between machines
        // (ADR-0023), and arriving somewhere its backend is absent makes it unable to think,
        // never invalid.
        let v = Vault::new("unknown-backend");
        v.write("mage.toml", TUNED);
        let mut reg = DefinitionRegistry::load(&v.0);

        let edit = CharacterEdit {
            id: "mage".into(),
            name: "Mage".into(),
            archetype: "researcher".into(),
            role: "r".into(),
            prompt: String::new(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: Some("the-machine-under-the-desk".into()),
            agent: None,
            model: Some("qwen3:14b".into()),
            parameters: ParametersEdit::default(),
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        let refused = edit.apply(&mut reg, &ollama_machine()).unwrap_err();
        assert!(refused.contains("the-machine-under-the-desk"), "{refused}");

        // And a second Ollama, configured, is a perfectly good name — id and kind are
        // different questions.
        let mut reg = DefinitionRegistry::load(&v.0);
        let machine = crate::launcher::Machine {
            offers: vec!["ollama".into(), "the-machine-under-the-desk".into()],
            window: None,
            controls: Vec::new(),
            agents: Vec::new(),
        };
        let edit = CharacterEdit {
            id: "mage".into(),
            name: "Mage".into(),
            archetype: "researcher".into(),
            role: "r".into(),
            prompt: String::new(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: Some("the-machine-under-the-desk".into()),
            agent: None,
            model: Some("qwen3:14b".into()),
            parameters: ParametersEdit::default(),
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        edit.apply(&mut reg, &machine).unwrap();
    }

    #[test]
    fn asking_for_more_context_than_the_model_holds_is_refused() {
        // Found by using it: the Advanced panel showed a *measured* window of 40960 beside a
        // free number box that happily took 41216, and the file kept it. `context_tokens` had
        // no ceiling at all while nothing measured one — and once something does, keeping the
        // field unbounded means showing the true limit and ignoring it in the same breath.
        let v = Vault::new("window");
        v.write("mage.toml", TUNED);
        let mut reg = DefinitionRegistry::load(&v.0);

        let mut machine = ollama_machine();
        machine.window = Some(40_960);

        let mut edit = CharacterEdit {
            id: "mage".into(),
            name: "Mage".into(),
            archetype: "researcher".into(),
            role: "r".into(),
            prompt: String::new(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: Some("ollama".into()),
            agent: None,
            model: Some("qwen3:14b".into()),
            parameters: ParametersEdit {
                context_tokens: Some(41_216),
                ..Default::default()
            },
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        let refused = edit.clone().apply(&mut reg, &machine).unwrap_err();
        assert!(refused.contains("40960"), "{refused}");
        // And it says the limit is the model's own, so the refusal is checkable rather than
        // something Epoch decided.
        assert!(refused.contains("not a guess"), "{refused}");

        // Inside it, fine.
        edit.parameters.context_tokens = Some(40_960);
        edit.clone().apply(&mut reg, &machine).unwrap();

        // And with nothing measured there is no ceiling to enforce: unknown is not zero, and
        // refusing against a number nobody measured would be the invented-bound problem wearing
        // the other hat.
        let mut reg = DefinitionRegistry::load(&v.0);
        edit.parameters.context_tokens = Some(999_999);
        edit.apply(&mut reg, &ollama_machine()).unwrap();
    }

    #[test]
    fn provider_tuning_is_checked_against_what_the_provider_declared() {
        use epoch_kernel::{Control, ControlKind, TuningValue};

        let v = Vault::new("tuning-edit");
        v.write("mage.toml", TUNED);
        let mut reg = DefinitionRegistry::load(&v.0);

        let control = Control {
            name: "num_ctx".into(),
            label: "Context window".into(),
            help: String::new(),
            kind: ControlKind::Whole {
                min: 512,
                max: 8192,
            },
            default: None,
            measured: true,
        };

        let mut edit = CharacterEdit {
            id: "mage".into(),
            name: "Mage".into(),
            archetype: "researcher".into(),
            role: "r".into(),
            prompt: String::new(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: Some("ollama".into()),
            agent: None,
            model: Some("qwen3:14b".into()),
            parameters: ParametersEdit::default(),
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };

        // A knob the Provider never declared is refused rather than written. A surface cannot
        // edit what it was not told exists, and a file may not hold what nothing can validate.
        edit.tuning.insert("num_gpu".into(), TuningValue::Int(1));
        assert!(edit
            .clone()
            .apply(&mut reg, &ollama_machine().with(control.clone()))
            .unwrap_err()
            .contains("num_gpu"));

        // Out of the declared bounds is refused, never clamped — the same rule the canonical
        // parameters follow, so the file on disk can never reach a state the loader rejects.
        edit.tuning.clear();
        edit.tuning
            .insert("num_ctx".into(), TuningValue::Int(999_999));
        assert!(edit
            .clone()
            .apply(&mut reg, &ollama_machine().with(control.clone()))
            .unwrap_err()
            .contains("num_ctx"));

        // Inside them it is written, under the Provider's own namespace.
        edit.tuning.insert("num_ctx".into(), TuningValue::Int(4096));
        edit.clone()
            .apply(&mut reg, &ollama_machine().with(control.clone()))
            .unwrap();
        let after = DefinitionRegistry::load(&v.0);
        let mind = after.character(&id("mage")).unwrap().mind.clone().unwrap();
        assert_eq!(mind.tuning["ollama"]["num_ctx"], TuningValue::Int(4096));

        // Cleared by the user, it goes — a declared knob the form omitted was turned off.
        // Anything authored by hand that the Provider never declared stays, so an older Epoch
        // cannot strip a character written against a newer one (ADR-0026).
        let mut reg = DefinitionRegistry::load(&v.0);
        edit.tuning.clear();
        edit.apply(&mut reg, &ollama_machine().with(control))
            .unwrap();
        let after = DefinitionRegistry::load(&v.0);
        let mind = after.character(&id("mage")).unwrap().mind.clone().unwrap();
        assert!(mind
            .tuning
            .get("ollama")
            .is_none_or(|m| !m.contains_key("num_ctx")));
    }

    #[test]
    fn an_impossible_parameter_is_refused_at_load_rather_than_corrected() {
        let v = Vault::new("badparam");
        v.write(
            "mage.toml",
            &TUNED.replace("temperature = 0.7", "temperature = 7.0"),
        );
        let reg = DefinitionRegistry::load(&v.0);
        assert!(
            reg.is_empty(),
            "a character who would answer differently than their file reads"
        );
        assert!(
            reg.problems()[0].contains("temperature"),
            "{:?}",
            reg.problems()
        );
    }

    #[test]
    fn a_file_that_disagrees_with_its_own_id_is_rejected() {
        // Otherwise two files could claim one character and the winner would depend on the
        // order the operating system happened to hand us.
        let v = Vault::new("mismatch");
        v.write("wizard.toml", GOOD);
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.is_empty());
        assert!(reg.problems()[0].contains("mage.toml"));
    }

    #[test]
    fn the_roster_lives_with_the_character() {
        let v = Vault::new("roster");
        v.write("mage.toml", GOOD);
        let reg = DefinitionRegistry::load(&v.0);
        assert_eq!(reg.living_in("default").count(), 1);
        assert_eq!(reg.living_in("archipelago").count(), 0);
    }

    #[test]
    fn a_character_can_be_moved_between_worlds_and_the_edit_survives_a_reload() {
        let v = Vault::new("move");
        v.write("mage.toml", GOOD);
        let mut reg = DefinitionRegistry::load(&v.0);

        reg.set_world(&id("mage"), "archipelago", true).unwrap();
        reg.set_world(&id("mage"), "default", false).unwrap();

        assert_eq!(reg.living_in("archipelago").count(), 1);
        assert_eq!(reg.living_in("default").count(), 0);

        // Written to disk, not merely held: a fresh registry sees the same thing.
        let fresh = DefinitionRegistry::load(&v.0);
        assert!(fresh.problems().is_empty(), "{:?}", fresh.problems());
        assert_eq!(fresh.living_in("archipelago").count(), 1);
    }

    #[test]
    fn saving_a_character_round_trips_every_field() {
        let v = Vault::new("roundtrip");
        v.write("mage.toml", GOOD);
        let mut reg = DefinitionRegistry::load(&v.0);

        let mut edited = reg.character(&id("mage")).unwrap().clone();
        edited.name = "Elowen".into();
        edited.role = "Engineer".into();
        edited.prompt = "You measure before you cut.".into();
        edited.presence.idle[0].activity = "welding".into();
        reg.save(edited).unwrap();

        let fresh = DefinitionRegistry::load(&v.0);
        assert!(fresh.problems().is_empty(), "{:?}", fresh.problems());
        let d = fresh.character(&id("mage")).unwrap();
        assert_eq!(d.name, "Elowen");
        assert_eq!(d.role, "Engineer");
        assert_eq!(d.prompt, "You measure before you cut.");
        assert_eq!(d.presence.idle[0].activity, "welding");
        assert_eq!(d.presence.idle[0].seconds, 12);
    }

    #[test]
    fn a_character_with_no_idle_behaviour_is_rejected() {
        // Executable form of "characters are never frozen".
        let v = Vault::new("frozen");
        v.write(
            "frozen.toml",
            "id = \"frozen\"\nname = \"F\"\narchetype = \"guardian\"\nrole = \"r\"\n\n[presence]\nhome = \"guild\"\nidle = []\n",
        );
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.is_empty());
        assert!(reg.problems()[0].contains("never frozen"));
    }

    #[test]
    fn a_mind_survives_a_round_trip() {
        let v = Vault::new("mind");
        v.write(
            "mage.toml",
            &format!("{GOOD}\n[mind]\nprovider = \"ollama\"\nmodel = \"qwen3:14b\"\n"),
        );
        let mut reg = DefinitionRegistry::load(&v.0);
        assert!(reg.problems().is_empty(), "{:?}", reg.problems());

        let mind = reg
            .character(&id("mage"))
            .unwrap()
            .mind
            .clone()
            .expect("a mind");
        assert_eq!(mind.provider(), Some("ollama"));
        assert_eq!(mind.model(), "qwen3:14b");

        // Saving anything else must not lose it.
        let mut edited = reg.character(&id("mage")).unwrap().clone();
        edited.name = "Elowen".into();
        reg.save(edited).unwrap();
        let fresh = DefinitionRegistry::load(&v.0);
        assert_eq!(
            fresh
                .character(&id("mage"))
                .unwrap()
                .mind
                .as_ref()
                .map(|m| m.model()),
            Some("qwen3:14b"),
        );
    }

    #[test]
    fn no_mind_is_a_valid_character_who_simply_cannot_think() {
        // The honest empty case. Defaulting a model would surprise the user with its cost.
        let v = Vault::new("mindless");
        v.write("mage.toml", GOOD);
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.problems().is_empty());
        assert!(reg.character(&id("mage")).unwrap().mind.is_none());
    }

    #[test]
    fn a_character_whose_backend_is_not_on_this_machine_still_loads() {
        // This used to be the opposite assertion, against a hardcoded list of one provider.
        // Two things made that wrong. Backends are configured now, so a backend the user called
        // `desk` is not a typo. And a character travels between machines (ADR-0023) — arriving
        // somewhere its backend is missing leaves it unable to think, never invalid. Refusing
        // the file would delete somebody from the roster for being away from home.
        let v = Vault::new("elsewhere");
        v.write(
            "mage.toml",
            &format!("{GOOD}\n[mind]\nprovider = \"desk\"\nmodel = \"qwen3:14b\"\n"),
        );
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.problems().is_empty(), "{:?}", reg.problems());
        assert_eq!(
            reg.character(&id("mage"))
                .unwrap()
                .mind
                .as_ref()
                .unwrap()
                .provider(),
            Some("desk")
        );
    }

    #[test]
    fn an_agent_brain_survives_an_edit_that_was_not_about_it() {
        // The hazard this guards: the Launcher edits *models*, so it sends no provider for a
        // character who works with an agent — and "no provider" already meant "the user cleared
        // the field". Renaming somebody would have quietly taken their brain away.
        let v = Vault::new("agentedit");
        v.write(
            "mage.toml",
            &format!("{GOOD}\n[mind]\nagent = \"claude_code\"\nmodel = \"opus\"\n"),
        );
        let mut reg = DefinitionRegistry::load(&v.0);
        let before = reg.character(&id("mage")).unwrap().clone();

        let edit = CharacterEdit {
            id: "mage".into(),
            name: "Mage the Elder".into(),
            archetype: "researcher".into(),
            role: before.role.clone(),
            prompt: before.prompt.clone(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            // What a model-editing form sends for somebody who does not think with a model.
            provider: None,
            agent: None,
            model: None,
            parameters: ParametersEdit::default(),
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        edit.apply(&mut reg, &ollama_machine()).unwrap();

        let after = DefinitionRegistry::load(&v.0);
        let d = after.character(&id("mage")).unwrap();
        assert_eq!(d.name, "Mage the Elder", "the edit still happened");
        assert_eq!(
            d.mind.as_ref().unwrap().brain.agent(),
            Some("claude_code"),
            "what the form does not edit, it does not touch"
        );
    }

    #[test]
    fn a_character_who_works_with_an_agent_round_trips_through_the_file() {
        // The kind is which name is present — no `kind =` line, and no migration for the files
        // that existed before agents did (ADR-0027).
        let v = Vault::new("agent");
        v.write(
            "mage.toml",
            &format!("{GOOD}\n[mind]\nagent = \"claude_code\"\nmodel = \"opus\"\n"),
        );
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.problems().is_empty(), "{:?}", reg.problems());

        let mind = reg.character(&id("mage")).unwrap().mind.clone().unwrap();
        assert_eq!(mind.brain.agent(), Some("claude_code"));
        // And the file keeps the shape TOML needs: names are values, tuning is a table, so the
        // values come first.
        let written = toml::to_string_pretty(reg.character(&id("mage")).unwrap()).unwrap();
        assert!(written.contains("agent = \"claude_code\""));
        assert!(!written.contains("provider ="));
    }

    #[test]
    fn a_character_with_no_name_is_rejected() {
        let v = Vault::new("nameless");
        v.write(
            "nameless.toml",
            &GOOD
                .replace("mage", "nameless")
                .replace("name = \"Mage\"", "name = \"  \""),
        );
        let reg = DefinitionRegistry::load(&v.0);
        assert!(reg.is_empty());
        assert!(reg.problems()[0].contains("every character is somebody"));
    }

    #[test]
    fn a_broken_file_does_not_take_the_world_down_with_it() {
        let v = Vault::new("mixed");
        v.write("mage.toml", GOOD);
        v.write("broken.toml", "this is not = valid toml [[[");
        let reg = DefinitionRegistry::load(&v.0);
        assert_eq!(reg.problems().len(), 1);
        assert!(reg.character(&id("mage")).is_some());
    }

    #[test]
    fn an_edit_on_disk_is_detected() {
        let v = Vault::new("reload");
        v.write("mage.toml", GOOD);
        let reg = DefinitionRegistry::load(&v.0);
        assert!(!reg.changed_on_disk());

        // Overwrite with a different role; mtime moves.
        std::thread::sleep(std::time::Duration::from_millis(20));
        v.write("mage.toml", &GOOD.replace("Turns goals", "Now edited"));
        assert!(reg.changed_on_disk());
    }

    /// **A vault that has never existed is empty, and reports nothing.**
    ///
    /// This test used to assert the opposite — one problem — and it was measuring the defect
    /// rather than the behaviour. Found by installing the real `.exe`: on a fresh machine the
    /// launcher's first screen read `HULL 1 FAULT LOGGED` above `cannot read
    /// …\definitions\characters`, while the line above it correctly said `CREW POSTED 0 / 0`.
    /// Nothing was broken; nobody had made a crew yet.
    ///
    /// The panic the old name worried about is still not possible, and *unreadable* is still a
    /// problem — see `something_that_is_not_a_folder_is_still_reported`. What changed is that
    /// **absent** stopped being reported as **broken**.
    #[test]
    fn a_vault_that_was_never_made_is_empty_and_says_nothing() {
        let reg = DefinitionRegistry::load(std::env::temp_dir().join("epoch-defs-nope"));
        assert!(reg.is_empty());
        assert_eq!(
            reg.problems().len(),
            0,
            "a first run has no crew, and that is not a fault: {:?}",
            reg.problems()
        );
    }

    #[test]
    fn an_unreadable_face_costs_the_face_and_not_the_character() {
        let v = Vault::new("noface");
        v.write(
            "mage.toml",
            &format!("{GOOD}\n[appearance]\nsprite = \"missing.png\"\n"),
        );
        let reg = DefinitionRegistry::load(&v.0);
        assert!(
            reg.character(&id("mage")).is_some(),
            "the character survived"
        );
        // Not an invisible sprite: `None`, which the renderer answers with a visible stand-in.
        assert!(reg.appearance(&id("mage")).is_none(), "and has no face");
        assert_eq!(reg.problems().len(), 1, "and the user is told why");
        assert!(reg.problems()[0].contains("mage"));
        // The registry must not mistake its own report for a file appearing on disk, or the
        // World would reload itself forever.
        assert!(!reg.changed_on_disk());
    }

    #[test]
    fn choosing_an_agent_replaces_a_model_brain_and_drops_its_settings() {
        // Parameters and tuning are a *Provider's* vocabulary (ADR-0026). An agent has no
        // `num_ctx`, and keeping them would leave a file full of settings describing nothing.
        let v = Vault::new("toagent");
        v.write(
            "mage.toml",
            &format!(
                "{GOOD}
[mind]
provider = \"ollama\"
model = \"qwen3:14b\"
"
            ),
        );
        let mut reg = DefinitionRegistry::load(&v.0);
        let before = reg.character(&id("mage")).unwrap().clone();

        let edit = CharacterEdit {
            id: "mage".into(),
            name: before.name.clone(),
            archetype: "researcher".into(),
            role: before.role.clone(),
            prompt: before.prompt.clone(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: None,
            agent: Some("claude-code".into()),
            model: Some("opus".into()),
            parameters: ParametersEdit::default(),
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        let machine = crate::launcher::Machine {
            agents: vec!["claude-code".into()],
            ..ollama_machine()
        };
        edit.apply(&mut reg, &machine).unwrap();

        let after = reg.character(&id("mage")).unwrap();
        assert!(matches!(
            after.mind.as_ref().unwrap().brain,
            epoch_kernel::Brain::Agent { .. }
        ));
        assert_eq!(after.mind.as_ref().unwrap().model(), "opus");
        assert!(after.mind.as_ref().unwrap().tuning.is_empty());
    }

    #[test]
    fn an_agent_this_machine_does_not_have_is_refused_while_the_list_is_still_on_screen() {
        // The same check a backend gets, at the same moment and for the same reason: it catches
        // choosing something that is not there. Never checked on *load* — a character travels
        // (ADR-0023), and arriving somewhere its agent is missing makes it unable to work
        // rather than invalid.
        let v = Vault::new("noagent");
        v.write("mage.toml", GOOD);
        let mut reg = DefinitionRegistry::load(&v.0);
        let before = reg.character(&id("mage")).unwrap().clone();

        let edit = CharacterEdit {
            id: "mage".into(),
            name: before.name.clone(),
            archetype: "researcher".into(),
            role: before.role.clone(),
            prompt: before.prompt.clone(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: None,
            agent: Some("codex".into()),
            model: None,
            parameters: ParametersEdit::default(),
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        assert!(edit.apply(&mut reg, &ollama_machine()).is_err());
    }

    #[test]
    fn what_epoch_writes_epoch_can_read_back() {
        // The bug this exists for, in one sentence: choosing an agent for somebody wrote
        // `model = ""`, and the loader refused it — so the character **vanished from the
        // roster**. A writer and a reader that disagree do not produce a warning; they produce
        // a person who is gone.
        //
        // Written through `apply` and read through `load`, both real, so the two can never
        // drift apart again without this failing.
        let v = Vault::new("roundtrip");
        v.write("mage.toml", GOOD);
        let mut reg = DefinitionRegistry::load(&v.0);
        let before = reg.character(&id("mage")).unwrap().clone();

        let edit = CharacterEdit {
            id: "mage".into(),
            name: before.name.clone(),
            archetype: "researcher".into(),
            role: before.role.clone(),
            prompt: before.prompt.clone(),
            home: None,
            routine: vec![RoutineStep {
                activity: "reading".into(),
                seconds: 12,
            }],
            provider: None,
            agent: Some("claude-code".into()),
            // Nobody named one. The agent's own choice stands — the common case.
            model: None,
            parameters: ParametersEdit::default(),
            requested_capabilities: None,
            skills: None,
            speaks_with: None,
            sounds_like: None,
            tuning: Default::default(),
        };
        let machine = crate::launcher::Machine {
            agents: vec!["claude-code".into()],
            ..ollama_machine()
        };
        edit.apply(&mut reg, &machine).unwrap();

        // The file, as it now sits on disk.
        let written = std::fs::read_to_string(v.0.join("characters/mage.toml")).unwrap();
        assert!(written.contains("agent = \"claude-code\""));
        assert!(
            !written.contains("model = \"\""),
            "an unset model is an absent key, never an empty string somebody has to interpret:
{written}"
        );

        // And read back by the loader that refused it before.
        let again = DefinitionRegistry::load(&v.0);
        assert!(again.problems().is_empty(), "{:?}", again.problems());
        assert!(
            again.character(&id("mage")).is_some(),
            "they are still on the roster"
        );
    }

    #[test]
    fn a_provider_with_no_model_is_still_refused() {
        // The half of the rule that was right. A provider cannot be asked anything without one,
        // so this stays a fault — and the fix above must not have loosened it.
        let v = Vault::new("halfmind");
        v.write(
            "mage.toml",
            &format!(
                "{GOOD}
[mind]
provider = \"ollama\"
"
            ),
        );
        let reg = DefinitionRegistry::load(&v.0);

        assert!(reg.character(&id("mage")).is_none());
        assert!(
            reg.problems().iter().any(|p| p.contains("no model")),
            "{:?}",
            reg.problems()
        );
    }
    #[test]
    fn removing_a_source_takes_it_off_everyone_who_asked_for_it() {
        // A request normally survives its capability being unavailable — that is ADR-0026, and
        // it is why a broken connection coming back finds its grants waiting. This is the one
        // case where it must not: the source is *gone*, and `mcp:spotify` names a slot rather
        // than a program. Install a different Spotify server under that name — which is exactly
        // what somebody does when the first turns out not to work — and the kept request
        // attaches to a different third party's tools.
        let v = Vault::new("revoke");
        v.write(
            "mage.toml",
            // Before `[presence]`, not appended after it. A key written after a table belongs
            // to that table — the first version of this test put the whole request list inside
            // the presence section, where it parsed cleanly and meant nothing.
            &GOOD.replace(
                "worlds = [\"default\"]",
                "worlds = [\"default\"]
requested_capabilities = [\"read_file\", \"mcp:spotify\", \"spotify_login\"]",
            ),
        );
        let mut reg = DefinitionRegistry::load(&v.0);

        let changed = reg
            .revoke_all(&["mcp:spotify".to_owned(), "spotify_login".to_owned()])
            .expect("the file is writable");

        assert_eq!(changed, vec![id("mage")]);
        let held = reg.character(&id("mage")).unwrap().wants().unwrap();
        let held: Vec<&str> = held.iter().map(|r| r.as_str()).collect();
        assert_eq!(held, vec!["read_file"], "only the server's ids leave");
    }

    #[test]
    fn somebody_who_never_chose_is_left_undecided() {
        // `None` means nobody has decided, and it resolves to everything available. There is
        // nothing to remove — and writing a list here would turn "hasn't decided" into "decided
        // on today's tools", which is a data-loss bug this codebase has already had once.
        let v = Vault::new("revoke-undecided");
        v.write("mage.toml", GOOD);
        let mut reg = DefinitionRegistry::load(&v.0);

        let changed = reg.revoke_all(&["mcp:spotify".to_owned()]).unwrap();

        assert!(changed.is_empty(), "nobody was touched: {changed:?}");
        assert!(reg.character(&id("mage")).unwrap().wants().is_none());
    }
}
