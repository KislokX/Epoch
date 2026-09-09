//! What actually drew, remembered by the hash of the model it drew with.
//!
//! ## Why this exists
//!
//! The Studio Panel can only *advise* on a model whose family Epoch reads out of its own tensors,
//! and that is six families against a set that grows faster than anybody reads release notes.
//! More detection rules would each need a real measurement and would still never close it.
//!
//! But when somebody draws successfully, Epoch is holding the whole answer it could not give:
//! this model file, these encoders, this `type` string, this VAE, and **a graph that ran**. That
//! is a measurement of exactly the thing the panel could not measure, and it is free, because it
//! already happened.
//!
//! ## What a recipe is, and what it is not
//!
//! It is **evidence that a combination ran**, not a claim that it is the best one or the only
//! one. Two recipes for one model are two things that worked, not a contradiction. The wording
//! everywhere downstream has to say that, or this becomes the closed list it replaces.
//!
//! ## It belongs to the machine
//!
//! ADR-0032's amendment settles it with ADR-0026's one test: *does this survive changing the
//! engine?* A recipe does not. It names **this machine's files**, by **this machine's hashes**,
//! against **this machine's server** — so it is `num_gpu` and not `temperature`, it lives beside
//! the Generative Library rather than in anybody's Character, and it never travels. The machine
//! that drew keeps it; the machine about to draw reads it.
//!
//! ## The model by hash, the parts by name
//!
//! Deliberately not the same for both, and the difference is what each answers.
//!
//! *Is this the same model?* is an identity question, and a filename is a claim while a hash is a
//! fact (ADR-0024, ADR-0032). So the key is the hash.
//!
//! The parts are what the **graph** must name, in the vocabulary the server reports them under —
//! re-offering them is the whole point, and a hash cannot be put into a `CLIPLoader`. A part that
//! was renamed or removed simply is not offered any more, which the panel already shows.
//!
//! ## And what did not run
//!
//! Kept in the same record with its outcome, never in a second store. Bounded twice so it cannot
//! lie: only a refusal **about the configuration** is worth keeping — an out-of-memory is the
//! card that day and would be false the next time it is free — and it is said as *this
//! combination was refused*, never *this model cannot*.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How many are kept for one model. Newest first, oldest dropped.
///
/// Small on purpose: this is *what worked*, not a log. A list long enough to scroll is one nobody
/// reads, and every entry after the first few says the same thing again.
const KEPT_PER_MODEL: usize = 6;

/// What happened when this combination was run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "outcome", content = "said")]
pub enum Outcome {
    /// A picture came out.
    Drew,
    /// The server refused the graph, in its own words — which are the only true account of it
    /// (ADR-0030's third amendment).
    Refused(String),
}

/// One combination that was run against one model, and what came of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Recipe {
    /// The model's own sha256, lowercase hex. Identity, never a filename.
    pub model: String,
    /// `checkpoint` or `diffusion`, as the panel row said.
    pub kind: String,
    /// The text encoders, as the server names them.
    #[serde(default)]
    pub clip: Vec<String>,
    /// The family string the CLIP loader was given.
    #[serde(default)]
    pub clip_type: String,
    #[serde(default)]
    pub vae: String,
    #[serde(flatten)]
    pub outcome: Outcome,
    /// When, in milliseconds since the epoch. Only ever used to order them.
    pub at: u64,
}

impl Recipe {
    /// Whether two recipes describe the same combination, whatever happened to them.
    ///
    /// So a combination that is run twice is one entry rather than a growing pile of identical
    /// ones — and so a refusal that is later fixed and re-run **replaces** its own refusal
    /// instead of sitting beside it saying the opposite.
    pub fn same_as(&self, other: &Recipe) -> bool {
        self.model == other.model
            && self.kind == other.kind
            && self.clip == other.clip
            && self.clip_type == other.clip_type
            && self.vae == other.vae
    }
}

/// Every recipe this installation holds.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipes {
    #[serde(default)]
    kept: Vec<Recipe>,
}

/// Where they live: beside the library they describe, one file per installation.
pub fn path(library: &Path) -> PathBuf {
    library.join("recipes.json")
}

impl Recipes {
    /// Read what this machine has learnt. Never fails: an unreadable file is an empty memory, and
    /// somebody's picture must not be blocked by a stray brace in a file of advice.
    pub fn load(library: &Path) -> Self {
        std::fs::read_to_string(path(library))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, library: &Path) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(self).map_err(|why| why.to_string())?;
        if let Some(dir) = path(library).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path(library), body).map_err(|why| why.to_string())
    }

    /// Keep one, newest first, replacing the same combination if it has been run before.
    pub fn remember(&mut self, recipe: Recipe) {
        self.kept.retain(|kept| !kept.same_as(&recipe));
        let model = recipe.model.clone();
        self.kept.insert(0, recipe);

        // Bounded per model rather than overall: a machine with twenty models should not lose
        // what it knows about the first one because it drew a lot with the twentieth.
        let mut seen = 0;
        self.kept.retain(|kept| {
            if kept.model != model {
                return true;
            }
            seen += 1;
            seen <= KEPT_PER_MODEL
        });
    }

    /// What is known about one model, newest first.
    pub fn about<'a>(&'a self, model: &str) -> Vec<&'a Recipe> {
        self.kept
            .iter()
            .filter(|kept| kept.model == model)
            .collect()
    }

    /// The most recent combination that actually drew, if any.
    ///
    /// What a surface offers, because it is the only kind of entry that can be *used*: a refusal
    /// is worth showing and is not worth filling a form in with.
    pub fn drew<'a>(&'a self, model: &str) -> Option<&'a Recipe> {
        self.about(model)
            .into_iter()
            .find(|kept| kept.outcome == Outcome::Drew)
    }
}

/// Whether a server's refusal is about **the configuration**, and so worth remembering.
///
/// Measured rather than reasoned: asked to load a two-encoder model with a family string from the
/// single loader's vocabulary, ComfyUI answers
/// `'stable_diffusion' not in ['sdxl', 'sd3', 'flux', …]`. That shape — a value, and the list it
/// is not in — is a fact about the combination and stays true tomorrow.
///
/// An out-of-memory is the opposite: true about the card at that moment, false the next time
/// nothing else is resident, and filing it as *this does not work* would be a gauge that lies.
pub fn is_about_the_configuration(said: &str) -> bool {
    let said = said.to_ascii_lowercase();
    if said.contains("out of memory") || said.contains("allocate") || said.contains("oom") {
        return false;
    }
    said.contains(" not in ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drew(model: &str, clip_type: &str) -> Recipe {
        Recipe {
            model: model.to_owned(),
            kind: "diffusion".into(),
            clip: vec!["clip_l.safetensors".into(), "t5xxl.safetensors".into()],
            clip_type: clip_type.to_owned(),
            vae: "flux-vae.safetensors".into(),
            outcome: Outcome::Drew,
            at: 1,
        }
    }

    #[test]
    fn what_drew_is_offered_and_a_refusal_is_not() {
        // A refusal is worth showing and is not worth filling a form in with.
        let mut kept = Recipes::default();
        kept.remember(Recipe {
            outcome: Outcome::Refused("'flux' not in ['sdxl']".into()),
            at: 2,
            ..drew("aaa", "flux")
        });
        assert!(kept.drew("aaa").is_none());

        kept.remember(drew("aaa", "flux2"));
        assert_eq!(kept.drew("aaa").unwrap().clip_type, "flux2");
        assert_eq!(kept.about("aaa").len(), 2, "both are remembered");
    }

    #[test]
    fn running_the_same_combination_again_replaces_its_own_verdict() {
        // Otherwise a refusal that was later fixed sits beside the fix saying the opposite, and
        // the reader has no way to tell which one is now true.
        let mut kept = Recipes::default();
        kept.remember(Recipe {
            outcome: Outcome::Refused("'flux' not in ['sdxl']".into()),
            ..drew("aaa", "flux")
        });
        kept.remember(drew("aaa", "flux"));
        assert_eq!(kept.about("aaa").len(), 1);
        assert!(kept.drew("aaa").is_some());
    }

    #[test]
    fn a_second_recipe_is_a_second_thing_that_worked() {
        // Not a contradiction. Two combinations ran; the newest is offered and both are kept.
        let mut kept = Recipes::default();
        kept.remember(drew("aaa", "flux"));
        kept.remember(drew("aaa", "flux2"));
        assert_eq!(kept.about("aaa").len(), 2);
        assert_eq!(kept.drew("aaa").unwrap().clip_type, "flux2");
    }

    #[test]
    fn one_model_cannot_push_another_models_answer_out() {
        let mut kept = Recipes::default();
        kept.remember(drew("bbb", "flux"));
        for n in 0..KEPT_PER_MODEL + 3 {
            kept.remember(drew("aaa", &format!("t{n}")));
        }
        assert_eq!(kept.about("aaa").len(), KEPT_PER_MODEL);
        assert_eq!(
            kept.about("bbb").len(),
            1,
            "somebody else's model is not evicted"
        );
    }

    #[test]
    fn only_a_refusal_about_the_configuration_is_worth_keeping() {
        // Measured against a real ComfyUI: a family string the chosen loader does not offer comes
        // back as a value and the list it is not in.
        assert!(is_about_the_configuration(
            "Value not in list: type: 'stable_diffusion' not in ['sdxl', 'sd3', 'flux']"
        ));
        // The card that day, not the combination. Filing this would be false the next time
        // nothing else is resident.
        assert!(!is_about_the_configuration(
            "CUDA out of memory. Tried to allocate 2.00 GiB"
        ));
        assert!(!is_about_the_configuration("connection timed out"));
    }

    #[test]
    fn an_unreadable_file_is_an_empty_memory_rather_than_a_refusal() {
        // A stray brace in a file of advice must not stop somebody drawing.
        let dir = std::env::temp_dir().join(format!("epoch-recipes-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(path(&dir), "{{{ not json").unwrap();
        assert_eq!(Recipes::load(&dir), Recipes::default());

        let mut kept = Recipes::default();
        kept.remember(drew("aaa", "flux"));
        kept.save(&dir).unwrap();
        assert_eq!(Recipes::load(&dir).drew("aaa").unwrap().clip_type, "flux");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
