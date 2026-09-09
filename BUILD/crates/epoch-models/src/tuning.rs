//! How a model is *told to run*, as opposed to how fast it was measured running.
//!
//! ## Why this is not part of `Loadout`
//!
//! A [`crate::loadout::Loadout`] is an axis of a curve — context against cache — and every value
//! of it is something `explore` actually ran and timed. What is here is a **choice**: flash
//! attention, and which kind of speculative decoding to use. Nobody measured a curve across
//! eleven speculation types, and pretending otherwise by widening `Loadout` would put settings
//! nobody ran into a structure whose whole meaning is *this was run*.
//!
//! So they are two files and two questions. The preset carries both.
//!
//! ## Everything here was read out of the program
//!
//! `llama-server --help` on build 10622 was asked for its flags and this is what it answered:
//! `--flash-attn [on|off|auto]`, `--spec-type` with eleven values, `--spec-draft-model`,
//! `--spec-draft-n-max/-n-min/-p-min`, `--spec-draft-ngl`. Nothing is named here that the program
//! did not name first, and nothing is shaped around one model — MTP is one value of `spec-type`
//! among eleven, and four of the others take no draft file at all.
//!
//! **Somebody else's flag is a measurement, not a memory.** When one of these stops being
//! accepted the honest failure is llama.cpp's own sentence, which is what the router prints.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// The speculative decoding kinds this build offers, in its own words.
///
/// Kept as the program's own strings rather than an enum: `--spec-type` grew from three values to
/// eleven between releases, and an enum here would silently drop whatever the next one adds. What
/// is validated is that the value is one this build listed — asked at the time, never remembered.
pub const SPEC_TYPES: [&str; 11] = [
    "none",
    "draft-simple",
    "draft-eagle3",
    "draft-mtp",
    "draft-dflash",
    "draft-dspark",
    "ngram-simple",
    "ngram-map-k",
    "ngram-map-k4v",
    "ngram-mod",
    "ngram-cache",
];

/// Which of those need a second model file beside the first.
///
/// The `ngram-*` kinds draft from the text already generated and take none; the `draft-*` kinds
/// load one. Derived from what each one *is* rather than from a list somebody keeps in step.
///
/// **`carries_its_own` is the half this was missing.** An MTP artefact has the draft head inside
/// it — measured 2026-08-31, `Qwen3.6-35B-A3B-MTP` is the plain file plus twenty `blk.40.*`
/// tensors — and ggml-org's own README runs it as
/// `llama-server -hf … --spec-type draft-mtp --spec-draft-n-max 2` with no `--spec-draft-model`
/// at all. Without this, Epoch would have refused `draft-mtp` to exactly the models that can do
/// it, for want of a file they do not need.
pub fn needs_a_draft(kind: &str) -> bool {
    needs_a_draft_file(kind, false)
}

/// The same question, asked of a model that may carry its own head.
pub fn needs_a_draft_file(kind: &str, carries_its_own: bool) -> bool {
    if kind == "draft-mtp" && carries_its_own {
        return false;
    }
    kind.starts_with("draft-")
}

/// Where the model's weights sit, when somebody has said.
///
/// ## Why this is a choice and not part of a `Loadout`
///
/// A [`crate::loadout::Loadout`] is an axis of a measured curve — context against cache — and
/// every value of it is something that was run and timed. This is the same kind of thing flash
/// attention is: a *choice*, written into a preset, that the search may then measure.
///
/// ## Read from `llama-fit-params`, never composed here
///
/// The pattern is a regular expression naming tensors and buffer types. Epoch does not write one
/// and does not parse one — it asks the program that will consume it, and passes on the answer
/// (`runtimes::fitted_params`). Measured 2026-08-31 for `Qwen3.6-35B-A3B-UD-IQ4_XS` at 32K on a
/// 12 GB card, llama.cpp's own recommendation was `-ngl 41` with twenty-two blocks' **MoE experts
/// only** moved to the CPU — attention stays on the card. Letting the runtime spill by whole
/// layers instead is what Epoch had been doing by saying nothing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    /// `--n-gpu-layers`. `None` leaves llama.cpp's own decision alone.
    #[serde(default)]
    pub gpu_layers: Option<i32>,
    /// `--override-tensor`. Opaque, and deliberately so.
    #[serde(default)]
    pub override_tensor: Option<String>,
}

impl Placement {
    pub fn on(&self) -> bool {
        self.gpu_layers.is_some() || self.override_tensor.is_some()
    }
}

/// Speculative decoding, as llama.cpp exposes it.
///
/// **General on purpose.** MTP is `kind = "draft-mtp"` with a draft file, and nothing in this
/// type knows that Gemma is the model that ships one. A future kind is a string this already
/// carries.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speculation {
    /// `--spec-type`. Empty or `none` is off, which is also what an absent `Tuning` means.
    #[serde(default)]
    pub kind: String,
    /// `--spec-draft-model`: the draft or MTP file. Required by the `draft-*` kinds and ignored
    /// by the rest, which is a fact about the kind rather than about any model.
    #[serde(default)]
    pub draft: Option<PathBuf>,
    /// `--spec-draft-n-max`: how many tokens to draft before verifying. llama.cpp's own default
    /// is 3, and `None` means *do not say*, which leaves it there.
    #[serde(default)]
    pub n_max: Option<u32>,
    /// `--spec-draft-n-min`.
    #[serde(default)]
    pub n_min: Option<u32>,
    /// `--spec-draft-p-min`: below this probability the draft is not worth verifying.
    #[serde(default)]
    pub p_min: Option<f64>,
    /// `--spec-draft-ngl`: how much of the draft model goes on the card.
    #[serde(default)]
    pub gpu_layers: Option<i32>,
}

impl Speculation {
    /// Whether this asks for anything at all.
    pub fn on(&self) -> bool {
        !self.kind.trim().is_empty() && self.kind != "none"
    }

    /// What is wrong with it, in a sentence somebody can act on. `None` when nothing is.
    ///
    /// **Checked here rather than by the server**, because the server's refusal arrives minutes
    /// later as a failed load in the middle of a benchmark — and a run that dies on a typo has
    /// cost more than it measured.
    pub fn quarrel(&self) -> Option<String> {
        if !self.on() {
            return None;
        }
        if !SPEC_TYPES.contains(&self.kind.as_str()) {
            return Some(format!(
                "{} is not a speculation this build knows. It offers: {}.",
                self.kind,
                SPEC_TYPES.join(", ")
            ));
        }
        match (&self.draft, needs_a_draft(&self.kind)) {
            (None, true) => Some(format!("{} needs a draft model beside it.", self.kind)),
            (Some(at), true) if !at.is_file() => {
                Some(format!("{} is not on this machine.", at.display()))
            }
            // A draft file with an ngram kind is not an error — it is a leftover from switching
            // kinds, and saying so is more use than refusing.
            (Some(_), false) => Some(format!(
                "{} drafts from the text it has already written, so the draft model is ignored.",
                self.kind
            )),
            _ => None,
        }
    }
}

/// What a model is told, beyond its context and its cache.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tuning {
    /// Where the weights sit. Empty until a search or a person has said.
    #[serde(default)]
    pub placement: Placement,
    /// `--flash-attn on|off`. **`None` is the program's own `auto`**, which is a real third state
    /// and not a default invented here.
    ///
    /// ## What is known about it, and where that knowledge stops
    ///
    /// Measured on this machine, on build 10622 with the CUDA backend: a **quantized V cache
    /// refuses to load without it** — `quantized V cache requires flash_attn to be enabled`. That
    /// is a fact about *this build and this backend*, and it is written down as one: whether ROCm,
    /// Vulkan, SYCL or Metal behave the same way is unmeasured, and Phase 13 exists partly to
    /// find out.
    ///
    /// It is also **not assumed to be bit-identical**. The arithmetic is the same attention in a
    /// different order, and floating point addition is not associative — so a benchmark that
    /// compares quality across `on` and `off` is comparing two runs, not one run twice.
    #[serde(default)]
    pub flash_attn: Option<bool>,
    #[serde(default)]
    pub speculation: Speculation,
}

impl Tuning {
    /// The preset lines this adds, in llama.cpp's own key names.
    ///
    /// **The preset takes any long-form flag** — measured: `flash-attn = on` was added by hand and
    /// the router logged `load: --flash-attn` while starting the child. So this needs no new
    /// mechanism, only more keys.
    ///
    /// Nothing is written for a value nobody chose. A preset that spelled out every default would
    /// make llama.cpp's own defaults impossible to get back to.
    pub fn lines(&self) -> String {
        let mut said = String::new();
        if let Some(on) = self.flash_attn {
            said.push_str(&format!("flash-attn = {}\n", if on { "on" } else { "off" }));
        }
        // Placement first: it is about the model rather than about how it is decoded, and a
        // preset reads better in the order somebody would say it.
        if let Some(layers) = self.placement.gpu_layers {
            said.push_str(&format!("n-gpu-layers = {layers}\n"));
        }
        if let Some(pattern) = &self.placement.override_tensor {
            said.push_str(&format!("override-tensor = {pattern}\n"));
        }
        if !self.speculation.on() {
            return said;
        }
        let spec = &self.speculation;
        said.push_str(&format!("spec-type = {}\n", spec.kind));
        if needs_a_draft(&spec.kind) {
            if let Some(at) = &spec.draft {
                said.push_str(&format!("spec-draft-model = {}\n", at.display()));
            }
        }
        if let Some(n) = spec.n_max {
            said.push_str(&format!("spec-draft-n-max = {n}\n"));
        }
        if let Some(n) = spec.n_min {
            said.push_str(&format!("spec-draft-n-min = {n}\n"));
        }
        if let Some(p) = spec.p_min {
            said.push_str(&format!("spec-draft-p-min = {p}\n"));
        }
        if let Some(n) = spec.gpu_layers {
            said.push_str(&format!("spec-draft-ngl = {n}\n"));
        }
        said
    }
}

/// Every model's tuning, as it is written down.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Tunings {
    #[serde(default)]
    kept: std::collections::BTreeMap<String, Tuning>,
}

pub fn path(library: &std::path::Path) -> PathBuf {
    library.join("tuning.json")
}

impl Tunings {
    /// Absent is the ordinary state: nothing tuned, and every model on llama.cpp's own defaults.
    pub fn load(library: &std::path::Path) -> Self {
        std::fs::read_to_string(path(library))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, library: &std::path::Path) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(self).map_err(|why| why.to_string())?;
        if let Some(home) = path(library).parent() {
            std::fs::create_dir_all(home).map_err(|why| why.to_string())?;
        }
        std::fs::write(path(library), raw).map_err(|why| why.to_string())
    }

    /// What a model is told. The default is *nothing*, which is llama.cpp deciding for itself.
    pub fn of(&self, model: &str) -> Tuning {
        self.kept.get(model).cloned().unwrap_or_default()
    }

    /// Say how one model should run. An empty tuning is removed rather than stored, so the file
    /// holds choices and never a list of things nobody chose.
    pub fn set(&mut self, model: &str, tuning: Tuning) {
        if tuning == Tuning::default() {
            self.kept.remove(model);
            return;
        }
        self.kept.insert(model.to_owned(), tuning);
    }

    pub fn all(&self) -> &std::collections::BTreeMap<String, Tuning> {
        &self.kept
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_chosen_writes_nothing() {
        // A preset that spelled out every default would make llama.cpp's own defaults impossible
        // to get back to, and `auto` is a real answer rather than an absent one.
        assert_eq!(Tuning::default().lines(), "");
    }

    #[test]
    fn flash_attention_has_three_states_and_none_is_one_of_them() {
        assert_eq!(
            Tuning {
                flash_attn: Some(true),
                ..Tuning::default()
            }
            .lines(),
            "flash-attn = on\n"
        );
        assert_eq!(
            Tuning {
                flash_attn: Some(false),
                ..Tuning::default()
            }
            .lines(),
            "flash-attn = off\n"
        );
        // Not "auto": saying nothing is how the program is left to decide, and writing the word
        // would be Epoch choosing the thing it is trying not to choose.
        assert!(!Tuning::default().lines().contains("flash-attn"));
    }

    #[test]
    fn mtp_is_one_value_among_eleven_and_nothing_here_knows_which_model_ships_one() {
        let tuned = Tuning {
            speculation: Speculation {
                kind: "draft-mtp".into(),
                draft: Some(PathBuf::from("C:/models/mtp-something.gguf")),
                n_max: Some(5),
                ..Speculation::default()
            },
            ..Tuning::default()
        };
        let said = tuned.lines();
        assert!(said.contains("spec-type = draft-mtp"), "{said}");
        assert!(
            said.contains("spec-draft-model = C:/models/mtp-something.gguf"),
            "{said}"
        );
        assert!(said.contains("spec-draft-n-max = 5"), "{said}");
        // Nothing that was not asked for.
        assert!(!said.contains("n-min"), "{said}");
        assert!(!said.contains("p-min"), "{said}");
    }

    #[test]
    fn a_kind_that_drafts_from_its_own_text_needs_no_file() {
        assert!(needs_a_draft("draft-mtp"));
        assert!(needs_a_draft("draft-eagle3"));
        assert!(!needs_a_draft("ngram-simple"));
        assert!(!needs_a_draft("ngram-cache"));

        let ngram = Speculation {
            kind: "ngram-simple".into(),
            ..Speculation::default()
        };
        assert_eq!(ngram.quarrel(), None, "it wants nothing beside it");
        assert!(ngram.on());
    }

    #[test]
    fn a_quarrel_is_raised_here_rather_than_by_the_server_minutes_later() {
        // A run that dies on a typo has cost more than it measured.
        let unknown = Speculation {
            kind: "draft-telepathy".into(),
            ..Speculation::default()
        };
        let said = unknown.quarrel().expect("that is not a kind");
        assert!(
            said.contains("draft-mtp"),
            "and it lists the real ones: {said}"
        );

        let missing = Speculation {
            kind: "draft-mtp".into(),
            ..Speculation::default()
        };
        assert!(missing
            .quarrel()
            .expect("no draft")
            .contains("needs a draft model"));

        // A leftover draft file after switching kinds is said, not refused.
        let leftover = Speculation {
            kind: "ngram-mod".into(),
            draft: Some(PathBuf::from("C:/models/old.gguf")),
            ..Speculation::default()
        };
        assert!(leftover.quarrel().expect("said").contains("ignored"));
    }

    #[test]
    fn nothing_chosen_is_not_stored() {
        // The file holds choices. A map full of empty tunings would make *nobody has tuned this*
        // indistinguishable from *somebody tuned it back to nothing*.
        let mut all = Tunings::default();
        all.set("a-model", Tuning::default());
        assert!(all.all().is_empty());

        all.set(
            "a-model",
            Tuning {
                flash_attn: Some(true),
                ..Tuning::default()
            },
        );
        assert_eq!(all.of("a-model").flash_attn, Some(true));
        assert_eq!(
            all.of("another"),
            Tuning::default(),
            "and an untouched model is untouched"
        );
    }
}
