//! The orchestrator — the person the bridge belongs to.
//!
//! ## Why this is not in a World
//!
//! The same reason the crew is not (ADR-0023): you are not a property of whichever World you
//! happen to have open. Your name and your face live in the vault, beside your characters,
//! and every World sees the same you.
//!
//! ## Why it is not in the Definition registry either
//!
//! A `CharacterDefinition` describes somebody the Engine *runs*: an archetype, a routine, a
//! prompt, a home place. None of that is true of the user. Putting the user in the crew would
//! mean either giving them a routine they do not have, or making every one of those fields
//! optional for everyone else. This is a small, separate file, because it is a small,
//! separate thing.
//!
//! Absent is a valid state and the common one on first run: no file means an unnamed
//! orchestrator with no portrait, and the bridge says so rather than inventing a commander.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What the bridge calls you when you have not said.
///
/// A title rather than a name — it describes the chair, which is true of whoever is sitting
/// in it, where an invented name would be a claim about a person who has not introduced
/// themselves.
pub const UNNAMED: &str = "ORCHESTRATOR";

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot record your profile: {0}")]
    Serialize(String),
    #[error("{0}")]
    Import(#[from] crate::import::ImportError),
}

/// The orchestrator's profile, as authored.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    /// What to call you. Empty means unnamed, and the bridge shows [`UNNAMED`].
    #[serde(default)]
    pub name: String,
    /// Portrait file name, relative to the vault. `None` means no face, honestly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portrait: Option<String>,
}

/// The orchestrator, resolved: ready for the bridge to render.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Orchestrator {
    /// What to call them. Never empty — falls back to [`UNNAMED`].
    pub name: String,
    /// True when nobody has introduced themselves, so the bridge can say so.
    pub is_unnamed: bool,
    /// Their portrait as a `data:` URI, through the same Asset path everything else uses.
    /// `None` means no artwork, and the bridge draws a visible stand-in rather than a face.
    pub portrait: Option<String>,
    /// What went wrong reading the portrait, if anything. Reported, never swallowed.
    pub problems: Vec<String>,
}

impl Orchestrator {
    /// Read the profile from the vault. Always succeeds; absent is a valid answer.
    pub fn load(vault: &Path) -> Self {
        let path = profile_path(vault);
        let mut problems = Vec::new();

        let profile = match std::fs::read_to_string(&path) {
            Ok(raw) => match toml::from_str::<Profile>(&raw) {
                Ok(profile) => profile,
                Err(err) => {
                    problems.push(format!("profile: {err}"));
                    Profile::default()
                }
            },
            // Not existing is the ordinary first-run state, not a problem worth reporting.
            Err(_) => Profile::default(),
        };

        let portrait =
            profile.portrait.as_deref().and_then(|reference| {
                match crate::asset::resolve(vault, reference) {
                    Ok(resolved) => Some(resolved.data_uri),
                    Err(err) => {
                        problems.push(format!("portrait: {err}"));
                        None
                    }
                }
            });

        let name = profile.name.trim();
        Self {
            is_unnamed: name.is_empty(),
            name: if name.is_empty() {
                UNNAMED.to_owned()
            } else {
                name.to_owned()
            },
            portrait,
            problems,
        }
    }

    /// Change what the bridge calls you. Blank clears it back to unnamed.
    pub fn rename(vault: &Path, name: &str) -> Result<(), ProfileError> {
        let mut profile = read(vault)?;
        profile.name = name.trim().to_owned();
        write(vault, &profile)
    }

    /// Accept a portrait: write the image into the vault, then point the profile at it.
    pub fn set_portrait(vault: &Path, base64_data: &str) -> Result<(), ProfileError> {
        let name = crate::import::accept_image(vault, "portrait", base64_data)?;
        let mut profile = read(vault)?;
        profile.portrait = Some(name);
        write(vault, &profile)
    }

    /// Remove the portrait and go back to the stand-in.
    pub fn clear_portrait(vault: &Path) -> Result<(), ProfileError> {
        crate::import::forget_image(vault, "portrait");
        let mut profile = read(vault)?;
        profile.portrait = None;
        write(vault, &profile)
    }
}

fn profile_path(vault: &Path) -> PathBuf {
    vault.join("orchestrator.toml")
}

/// Read what is on disk, or an empty profile. A broken file is *not* silently replaced here —
/// it would be overwritten by the next save, and losing somebody's file to a typo in it is
/// worse than refusing.
fn read(vault: &Path) -> Result<Profile, ProfileError> {
    let path = profile_path(vault);
    match std::fs::read_to_string(&path) {
        Ok(raw) => toml::from_str(&raw).map_err(|err| ProfileError::Serialize(err.to_string())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Profile::default()),
        Err(source) => Err(ProfileError::Io { path, source }),
    }
}

fn write(vault: &Path, profile: &Profile) -> Result<(), ProfileError> {
    let body =
        toml::to_string_pretty(profile).map_err(|err| ProfileError::Serialize(err.to_string()))?;
    std::fs::create_dir_all(vault).map_err(|source| ProfileError::Write {
        path: vault.to_path_buf(),
        source,
    })?;
    let path = profile_path(vault);
    std::fs::write(&path, body).map_err(|source| ProfileError::Write { path, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    struct Vault(PathBuf);

    impl Vault {
        fn new(name: &str) -> Self {
            let d = std::env::temp_dir().join(format!("epoch-profile-{name}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
    }

    impl Drop for Vault {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn an_absent_profile_is_a_valid_first_run() {
        let v = Vault::new("absent");
        let me = Orchestrator::load(&v.0);
        assert_eq!(me.name, UNNAMED);
        assert!(
            me.is_unnamed,
            "the bridge must be able to say nobody introduced themselves"
        );
        assert!(me.portrait.is_none());
        assert!(me.problems.is_empty(), "not existing is not a fault");
    }

    #[test]
    fn a_name_survives_a_round_trip() {
        let v = Vault::new("name");
        Orchestrator::rename(&v.0, "  Kislok  ").unwrap();
        let me = Orchestrator::load(&v.0);
        assert_eq!(me.name, "Kislok");
        assert!(!me.is_unnamed);
    }

    #[test]
    fn a_blank_name_returns_to_unnamed_rather_than_showing_nothing() {
        let v = Vault::new("blank");
        Orchestrator::rename(&v.0, "Kislok").unwrap();
        Orchestrator::rename(&v.0, "   ").unwrap();
        let me = Orchestrator::load(&v.0);
        assert_eq!(me.name, UNNAMED);
        assert!(me.is_unnamed);
    }

    #[test]
    fn a_portrait_is_imported_and_delivered_as_a_data_uri() {
        // Same Asset path as everything else: never inlined as markup (ADR-0020).
        let v = Vault::new("portrait");
        Orchestrator::set_portrait(&v.0, PNG).unwrap();

        let me = Orchestrator::load(&v.0);
        let uri = me.portrait.expect("the portrait must reach the bridge");
        assert!(uri.starts_with("data:image/png;base64,"));
        assert!(me.problems.is_empty(), "{:?}", me.problems);
        assert!(v.0.join("portrait.png").is_file());
    }

    #[test]
    fn a_portrait_can_be_taken_back_off() {
        let v = Vault::new("clear");
        Orchestrator::set_portrait(&v.0, PNG).unwrap();
        Orchestrator::clear_portrait(&v.0).unwrap();

        let me = Orchestrator::load(&v.0);
        assert!(me.portrait.is_none());
        assert!(!v.0.join("portrait.png").exists(), "the file goes too");
    }

    #[test]
    fn renaming_does_not_lose_a_portrait_and_vice_versa() {
        // Two commands over one file: each must preserve what the other owns.
        let v = Vault::new("both");
        Orchestrator::set_portrait(&v.0, PNG).unwrap();
        Orchestrator::rename(&v.0, "Kislok").unwrap();

        let me = Orchestrator::load(&v.0);
        assert_eq!(me.name, "Kislok");
        assert!(me.portrait.is_some(), "renaming must not drop the portrait");
    }

    #[test]
    fn a_portrait_that_vanished_costs_the_portrait_and_not_the_profile() {
        let v = Vault::new("gone");
        Orchestrator::set_portrait(&v.0, PNG).unwrap();
        Orchestrator::rename(&v.0, "Kislok").unwrap();
        std::fs::remove_file(v.0.join("portrait.png")).unwrap();

        let me = Orchestrator::load(&v.0);
        assert_eq!(me.name, "Kislok", "still themselves");
        assert!(me.portrait.is_none());
        assert_eq!(me.problems.len(), 1, "and told why");
    }

    #[test]
    fn a_broken_profile_is_reported_rather_than_overwritten() {
        // Losing somebody's file to a typo inside it would be worse than refusing to save.
        let v = Vault::new("broken");
        std::fs::write(
            v.0.join("orchestrator.toml"),
            "this is not = valid toml [[[",
        )
        .unwrap();

        let me = Orchestrator::load(&v.0);
        assert_eq!(me.name, UNNAMED);
        assert_eq!(me.problems.len(), 1);

        assert!(Orchestrator::rename(&v.0, "Kislok").is_err());
        let raw = std::fs::read_to_string(v.0.join("orchestrator.toml")).unwrap();
        assert!(raw.contains("[[["), "the user's file must survive");
    }
}
