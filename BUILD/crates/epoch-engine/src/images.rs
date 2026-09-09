//! Styles, and the workflows that serve them.
//!
//! ## Two things, and keeping them apart is the whole design
//!
//! A **workflow** belongs to *this machine*: it names checkpoints, LoRAs and custom nodes that
//! are installed here, and it is compiled against the ComfyUI that will run it. A **Style** is
//! the product's word — `pixel art`, `realistic` — and it is what a character asks for
//! (ADR-0030's amendment).
//!
//! A character asking for `pixel_v3.json` would break a rule Epoch has held since the Asset
//! Resolver: the engine references concepts and never filenames, and music is requested as a
//! mood rather than a track. *Pixel art* is a mood.
//!
//! ## A Style exists when something that can draw it arrives
//!
//! Six names used to ship, chosen by the owner, arriving as names with nothing behind them. That
//! was a hypothesis and using the product refuted it (ADR-0030's second amendment): a closed set
//! answers somebody who wants *watercolour* with *"that style does not exist"* — a sentence about
//! Epoch's array delivered as a sentence about the world. Watercolour exists. What was missing
//! was something to paint it with, and a closed list cannot tell the difference.
//!
//! So a Style is **derived**: it exists because a workflow serves it, and it stops existing when
//! the last one is taken away. Attaching is the real cause, and the name is whatever the person
//! attaching it typed.
//!
//! **`GENERAL` survives and it is the only one.** It is not a style; it is the absence of one,
//! and it exists the moment any model does — which is what `BUILD ME ONE` attaches to.
//!
//! It never falls back to another Style: substituting *pixel art* with *realistic* changes what
//! somebody asked for, and with Styles derived that would mean drawing with something the user
//! never asked for *and* never installed.
//!
//! ## The source is kept
//!
//! What arrived is stored beside what it compiled to. A workflow that could not be read at all
//! is *still stored*, because the fix is often installing a node and pressing again — and there
//! is nothing to press if the only thing kept was a conversion that never happened.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use epoch_assets::workflow::{Imported, Opening, Schema, Unreadable};

/// The one Style that is not a style: the absence of one.
///
/// It exists the moment anything can draw at all, so it is the only name Epoch may put in a
/// file nobody has typed into. Every other Style is somebody's word for a workflow they
/// attached.
pub const GENERAL: &str = "General";

/// A workflow this machine has, as it is remembered between runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Kept {
    /// What a person calls it. Taken from the file's name at import and editable after.
    pub name: String,
    /// Stable, and not the name: renaming a workflow must not detach every Style pointing at it.
    pub id: String,
    /// The file, under `images/workflows/`. Named by Epoch from the bytes — never by whatever
    /// the upload claimed (ADR-0024).
    pub file: String,
    /// What it turned out to mean, last time it was compiled here.
    ///
    /// `None` when it has never compiled on this machine — usually a custom node that is not
    /// installed. Kept anyway, with `problem` saying why.
    #[serde(default)]
    pub opening: Option<Opening>,
    /// Why it did not compile, in the server's terms and Epoch's words.
    #[serde(default)]
    pub problem: Option<String>,
}

/// A Style, and what serves it here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    /// The word a character asks for. `pixel art`, matched without case.
    pub name: String,
    /// Workflow ids that can serve it, best first.
    ///
    /// A list rather than one, because *pixel art that can also take a reference picture* and
    /// *pixel art that cannot* are two workflows serving one word, and which is wanted depends
    /// on what was asked for.
    #[serde(default)]
    pub workflows: Vec<String>,
}

/// Everything about making pictures that belongs to this machine.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Images {
    #[serde(default)]
    pub styles: Vec<Style>,
    #[serde(default)]
    pub workflows: Vec<Kept>,
    /// Which Style is used when nobody says. Empty means the first one that can serve.
    #[serde(default)]
    pub usually: String,
    /// Which machine draws. Empty is **this** one.
    ///
    /// Otherwise the id of a paired machine (ADR-0029), and the picture is made on its card. It
    /// is stored rather than decided each time because it is a **choice**, and Epoch does not
    /// make it: *recommending is not choosing* is the rule the Workshop already follows about
    /// which model fits, and where a picture is drawn deserves the same. A machine that stopped
    /// serving is reported, never quietly swapped for another — silently drawing somewhere
    /// else is a different answer to a different question.
    #[serde(default)]
    pub draw_on: String,
    /// The file could not be read. Reported rather than replaced, and never saved over.
    #[serde(default, skip_serializing)]
    pub problem: Option<String>,
}

fn path(vault: &Path) -> PathBuf {
    vault.join("images.toml")
}

/// Where an imported workflow's own bytes live.
pub fn workflows_dir(vault: &Path) -> PathBuf {
    vault.join("images").join("workflows")
}

impl Images {
    /// What ships: one name, and nothing behind it.
    pub fn first_run() -> Self {
        Self {
            styles: vec![Style {
                name: GENERAL.to_owned(),
                workflows: Vec::new(),
            }],
            workflows: Vec::new(),
            usually: GENERAL.to_owned(),
            // This machine, until somebody says otherwise.
            draw_on: String::new(),
            problem: None,
        }
    }

    /// Read what this machine has. Never fails: an unreadable file is reported and the shipped
    /// names stand, so a stray character cannot leave somebody unable to draw at all.
    pub fn load(vault: &Path) -> Self {
        match std::fs::read_to_string(path(vault)) {
            Err(_) => Self::first_run(),
            Ok(raw) => match toml::from_str::<Images>(&raw) {
                Ok(mut found) => {
                    found.settle();
                    found
                }
                Err(err) => Self {
                    problem: Some(format!(
                        "images.toml could not be read ({err}). Using the shipped Styles; your \
                         file is untouched."
                    )),
                    ..Self::first_run()
                },
            },
        }
    }

    /// Write it back.
    ///
    /// Refuses to save over a file it could not read, for the reason `Backends` does: reporting
    /// a malformed file rather than replacing it is pointless if the next save destroys it.
    pub fn save(&self, vault: &Path) -> Result<(), String> {
        if let Some(problem) = &self.problem {
            return Err(format!(
                "not overwriting a file that could not be read: {problem}"
            ));
        }
        let raw = toml::to_string_pretty(self).map_err(|err| err.to_string())?;
        std::fs::create_dir_all(vault).map_err(|err| err.to_string())?;
        std::fs::write(path(vault), raw).map_err(|err| err.to_string())
    }

    /// Take a workflow in and remember it.
    ///
    /// **Stored even when it does not compile.** A graph needing a node nobody installed is a
    /// workflow somebody will want to try again after installing it, and there is nothing to try
    /// again if the file was refused at the door.
    pub fn take_in(
        &mut self,
        vault: &Path,
        name: &str,
        source: &serde_json::Value,
        schema: &Schema,
    ) -> Result<String, String> {
        let id = free_id(&self.workflows, name);
        let file = format!("{id}.json");
        let dir = workflows_dir(vault);
        std::fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        std::fs::write(
            dir.join(&file),
            serde_json::to_string_pretty(source).map_err(|err| err.to_string())?,
        )
        .map_err(|err| err.to_string())?;

        let (opening, problem) = match Imported::of(source.clone(), schema) {
            Ok(taken) => (Some(taken.opening), None),
            Err(why) => (None, Some(why.to_string())),
        };

        self.workflows.push(Kept {
            name: name.to_owned(),
            id: id.clone(),
            file,
            opening,
            problem,
        });
        Ok(id)
    }

    /// Read one back, compiled against the ComfyUI that is here now.
    ///
    /// Always from the source. The stored `opening` is a summary for a screen; a run compiles
    /// again, because the server may have gained a node since — which is the entire reason the
    /// original is kept.
    pub fn open(&self, vault: &Path, id: &str, schema: &Schema) -> Result<Imported, WhyNot> {
        let kept = self
            .workflows
            .iter()
            .find(|kept| kept.id == id)
            .ok_or_else(|| WhyNot::Unknown(id.to_owned()))?;
        let raw = std::fs::read_to_string(workflows_dir(vault).join(&kept.file))
            .map_err(|err| WhyNot::Gone(kept.name.clone(), err.to_string()))?;
        let source: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|err| WhyNot::Gone(kept.name.clone(), err.to_string()))?;
        Imported::of(source, schema).map_err(|why| WhyNot::Unreadable(kept.name.clone(), why))
    }

    /// Make the list say only what is true, and say it once.
    ///
    /// **A Style exists because something serves it.** So one with nothing attached is not a
    /// cold instrument waiting to be wired — it is a category somebody chose on a Tuesday, and
    /// the rule protects readings rather than catalogues (ADR-0030's second amendment). It is
    /// dropped, here, on the way in and on the way out.
    ///
    /// This is also the migration, and it is the honest one: the five names that shipped dark
    /// never had anything behind them, so removing them takes nothing away. A name somebody
    /// actually attached a workflow to survives, because it has a cause.
    ///
    /// `GENERAL` is exempt and always present: it is the absence of a style rather than one of
    /// them, and `BUILD ME ONE` needs somewhere to put what it just made.
    ///
    /// `usually` is checked here too. A default pointing at a Style that no longer exists would
    /// resolve to nothing on the one path that runs when nobody said anything — the quietest
    /// place a wrong answer could hide.
    fn settle(&mut self) {
        self.styles.retain(|style| {
            !style.workflows.is_empty() || style.name.eq_ignore_ascii_case(GENERAL)
        });
        if !self
            .styles
            .iter()
            .any(|style| style.name.eq_ignore_ascii_case(GENERAL))
        {
            self.styles.insert(
                0,
                Style {
                    name: GENERAL.to_owned(),
                    workflows: Vec::new(),
                },
            );
        }
        if !self
            .styles
            .iter()
            .any(|style| style.name.eq_ignore_ascii_case(self.usually.trim()))
        {
            self.usually = GENERAL.to_owned();
        }
    }

    /// Attach a workflow to a Style, **naming the Style into existence if it is new**.
    ///
    /// This is the real cause the second amendment asks for: a Style comes into being because a
    /// file arrived and somebody said what it draws. Epoch never invents the name and never
    /// guesses it from the workflow — a graph called *SNES Pixel Art* is probably pixel art, and
    /// *probably* is not a thing to act on when the consequence is silently deciding what
    /// somebody's `pixel art` means.
    ///
    /// Detaching the last workflow removes the Style, for the same reason: nothing draws it any
    /// more, so claiming it exists would be the closed list all over again with one name in it.
    ///
    /// Matching is exact but case-insensitive, so `Watercolour` and `watercolour` are one Style
    /// rather than two. Resemblance is never matched — that is how a realistic picture gets
    /// drawn for somebody who asked for pixel art.
    pub fn attach(&mut self, style: &str, id: &str, attach: bool) -> Result<(), String> {
        let wanted = style.trim();
        if wanted.is_empty() {
            return Err("a Style needs a name".to_owned());
        }
        if attach && !self.workflows.iter().any(|kept| kept.id == id) {
            return Err(format!("no workflow here is called '{id}'"));
        }
        match self
            .styles
            .iter_mut()
            .find(|known| known.name.eq_ignore_ascii_case(wanted))
        {
            Some(known) => {
                known.workflows.retain(|held| held != id);
                if attach {
                    known.workflows.insert(0, id.to_owned());
                }
            }
            None => {
                if !attach {
                    // Nothing to take away. Not an error: the end state the caller asked for is
                    // the one that already holds.
                    return Ok(());
                }
                self.styles.push(Style {
                    name: wanted.to_owned(),
                    workflows: vec![id.to_owned()],
                });
            }
        }
        self.settle();
        Ok(())
    }

    /// Which workflow should serve this Style, if any.
    ///
    /// **Never another Style's.** Substituting *pixel art* with *realistic* changes what somebody
    /// asked for, and a picture nobody wanted is worse than a sentence saying it is not
    /// installed.
    pub fn serving(&self, style: &str) -> Option<&Kept> {
        let wanted = self
            .styles
            .iter()
            .find(|known| known.name.eq_ignore_ascii_case(style.trim()))?;
        wanted
            .workflows
            .iter()
            .find_map(|id| self.workflows.iter().find(|kept| &kept.id == id))
    }

    /// Every Style, and whether anything here can serve it.
    pub fn lit(&self) -> Vec<(String, bool)> {
        self.styles
            .iter()
            .map(|style| (style.name.clone(), self.serving(&style.name).is_some()))
            .collect()
    }
}

/// Read a workflow out of what a file input handed across IPC.
///
/// Base64, because the frontend never touches the filesystem (ADR-0024) and bytes are what a
/// `<input type="file">` can offer. A `data:` prefix is tolerated: the same input produces one
/// or not depending on how the page read it, and refusing over a prefix would be a papercut
/// with no purpose.
///
/// Decoding lives here rather than in the shell for the reason every other decode does: one
/// crate owns turning somebody's bytes into something Epoch will act on.
pub fn from_base64(raw: &str) -> Result<serde_json::Value, String> {
    use base64::Engine as _;
    let payload = raw.split(',').next_back().unwrap_or_default();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|err| format!("that file could not be read ({err})"))?;
    serde_json::from_slice(&bytes).map_err(|err| format!("that is not a ComfyUI workflow: {err}"))
}

/// Why a stored workflow could not be opened.
#[derive(Debug, Clone, PartialEq)]
pub enum WhyNot {
    /// No workflow answers to that id.
    Unknown(String),
    /// The file it named is not there any more, or is not readable.
    Gone(String, String),
    /// It is there and this ComfyUI cannot run it.
    Unreadable(String, Unreadable),
}

impl std::fmt::Display for WhyNot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WhyNot::Unknown(id) => write!(f, "no workflow here is called '{id}'"),
            WhyNot::Gone(name, err) => {
                write!(
                    f,
                    "'{name}' is remembered but its file could not be read ({err})"
                )
            }
            WhyNot::Unreadable(name, why) => write!(f, "'{name}': {why}"),
        }
    }
}

/// A stable id nothing else is using.
///
/// From the name, so a folder of them is readable — and never *equal* to the name, because a
/// rename must not detach every Style pointing at it.
fn free_id(kept: &[Kept], name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let stem = stem.trim_matches('-').to_owned();
    let stem = if stem.is_empty() {
        "workflow".to_owned()
    } else {
        stem
    };
    let mut id = stem.clone();
    let mut next = 2;
    while kept.iter().any(|one| one.id == id) {
        id = format!("{stem}-{next}");
        next += 1;
    }
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nowhere() -> PathBuf {
        std::env::temp_dir().join(format!("epoch-images-{}", std::process::id()))
    }

    #[test]
    fn one_name_ships_and_it_is_the_absence_of_a_style() {
        // Six shipped once, and five of them sat permanently dark on a fresh install. That was
        // defended as the cold-instrument rule and the defence does not hold: a dark reading is
        // a quantity not yet wired, and a dark `Anime` was never a quantity at all.
        let images = Images::first_run();
        assert_eq!(images.styles.len(), 1);
        assert_eq!(images.styles[0].name, GENERAL);
        assert!(images.styles[0].workflows.is_empty());
        // A shipped workflow would quietly become the house style, and nobody chose that.
        assert!(images.workflows.is_empty());
    }

    #[test]
    fn a_style_exists_because_something_draws_it_and_stops_when_nothing_does() {
        let mut images = Images::first_run();
        images.workflows.push(Kept {
            name: "Wet Media XL".into(),
            id: "wet-media-xl".into(),
            file: "wet-media-xl.json".into(),
            opening: None,
            problem: None,
        });

        // Nobody asked Epoch whether watercolour is a real style. Something that paints it
        // arrived, so it is one.
        assert!(images.serving("watercolour").is_none());
        images.attach("watercolour", "wet-media-xl", true).unwrap();
        assert!(
            images.serving("Watercolour").is_some(),
            "and the case is theirs"
        );

        // Take the last one away and the Style goes with it. Keeping the name would be the
        // closed list again with one entry in it.
        images.attach("watercolour", "wet-media-xl", false).unwrap();
        assert!(
            !images
                .styles
                .iter()
                .any(|style| style.name.eq_ignore_ascii_case("watercolour")),
            "nothing draws it any more"
        );
        // General is the exception, and the only one.
        assert_eq!(images.styles.len(), 1);
        assert_eq!(images.styles[0].name, GENERAL);
    }

    #[test]
    fn a_style_is_never_named_for_a_workflow_that_is_not_here() {
        let mut images = Images::first_run();
        // Otherwise a typo invents a Style that draws nothing — exactly the state the whole
        // design exists to make unreachable.
        assert!(images
            .attach("watercolour", "nothing-like-it", true)
            .is_err());
        assert_eq!(images.styles.len(), 1);
        // And a name with nothing in it is not a name.
        assert!(images.attach("   ", "nothing-like-it", true).is_err());
    }

    #[test]
    fn a_default_pointing_at_a_style_that_went_away_falls_back_to_general() {
        // The quietest place a wrong answer could hide: the path that runs when nobody said
        // anything at all.
        let mut images = Images::first_run();
        images.workflows.push(Kept {
            name: "Wet Media XL".into(),
            id: "wet-media-xl".into(),
            file: "wet-media-xl.json".into(),
            opening: None,
            problem: None,
        });
        images.attach("watercolour", "wet-media-xl", true).unwrap();
        images.usually = "watercolour".into();

        images.attach("watercolour", "wet-media-xl", false).unwrap();
        assert_eq!(images.usually, GENERAL);
    }

    #[test]
    fn a_style_with_nothing_behind_it_serves_nothing_and_never_borrows() {
        let mut images = Images::first_run();
        images.workflows.push(Kept {
            name: "Realistic SDXL".into(),
            id: "realistic-sdxl".into(),
            file: "realistic-sdxl.json".into(),
            opening: None,
            problem: None,
        });
        images.attach("Realistic", "realistic-sdxl", true).unwrap();

        assert!(images.serving("Realistic").is_some());
        // Asked for pixel art, given nothing — never the realistic one. A picture nobody wanted
        // is worse than a sentence saying it is not installed.
        assert!(images.serving("Pixel Art").is_none());
        // And the word is matched the way a person would say it.
        assert!(images.serving("  realistic ").is_some());

        let lit = images.lit();
        assert_eq!(lit.iter().filter(|(_, on)| *on).count(), 1);
        // And `Pixel Art` is not on the list at all: with Styles derived, a row that cannot draw
        // cannot exist. The honest report of its absence moves to the sentence a character says
        // at the moment somebody wants it.
        assert!(!lit.iter().any(|(name, _)| name == "Pixel Art"));
    }

    #[test]
    fn an_unreadable_file_is_reported_and_never_saved_over() {
        let dir = nowhere().join("unreadable");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(path(&dir), "this is not toml at all {{{").unwrap();

        let images = Images::load(&dir);
        assert!(images.problem.is_some());
        // `General` still stands, so nobody is left unable to draw over a stray brace.
        assert_eq!(images.styles.len(), 1);
        assert_eq!(images.styles[0].name, GENERAL);
        assert!(images.save(&dir).is_err(), "the user's file is untouched");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_names_that_shipped_dark_are_dropped_and_an_earned_one_is_kept() {
        // The migration, and it takes nothing away: a Style that shipped with nothing behind it
        // never drew anything. One somebody actually attached a workflow to has a cause, so it
        // survives — that is the whole difference the derived rule turns on.
        let dir = nowhere().join("upgrade");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            path(&dir),
            concat!(
                "usually = \"General\"\n\n",
                "[[workflows]]\nname = \"SNES Pixel Art\"\nid = \"snes\"\nfile = \"snes.json\"\n\n",
                "[[styles]]\nname = \"General\"\nworkflows = []\n\n",
                "[[styles]]\nname = \"Anime\"\nworkflows = []\n\n",
                "[[styles]]\nname = \"Pixel Art\"\nworkflows = [\"snes\"]\n",
            ),
        )
        .unwrap();

        let images = Images::load(&dir);
        let names: Vec<&str> = images.styles.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["General", "Pixel Art"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_id_is_stable_and_is_not_the_name() {
        let kept = vec![Kept {
            name: "SNES Pixel Art".into(),
            id: "snes-pixel-art".into(),
            file: "snes-pixel-art.json".into(),
            opening: None,
            problem: None,
        }];
        // A second one with the same name gets its own id rather than colliding.
        assert_eq!(free_id(&kept, "SNES Pixel Art"), "snes-pixel-art-2");
        assert_eq!(free_id(&kept, "  "), "workflow");
    }

    /// A workflow this ComfyUI cannot run is still worth keeping.
    #[test]
    fn a_workflow_that_does_not_compile_is_stored_anyway() {
        let dir = nowhere().join("stored");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut images = Images::first_run();
        // A node this machine does not have.
        let source = serde_json::json!({
            "nodes": [{ "id": 1, "type": "IPAdapterApply", "inputs": [] }],
            "links": []
        });
        let id = images
            .take_in(&dir, "Reference Portrait", &source, &Schema::default())
            .expect("stored");

        let kept = &images.workflows[0];
        assert_eq!(kept.id, id);
        assert!(kept.opening.is_none());
        assert!(
            kept.problem.as_deref().unwrap().contains("IPAdapterApply"),
            "named, so it can be searched for: {:?}",
            kept.problem
        );
        // The file is there, so installing the node and pressing again is possible.
        assert!(workflows_dir(&dir).join(&kept.file).is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
