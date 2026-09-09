//! Which file this is, as opposed to which model it is a version of.
//!
//! ## The collision that made this necessary
//!
//! `unsloth/Qwen3.6-35B-A3B-GGUF` and `unsloth/Qwen3.6-35B-A3B-MTP-GGUF` publish files with
//! **identical names**. Downloaded side by side on 2026-08-31, `Qwen3.6-35B-A3B-UD-IQ4_XS.gguf`
//! meant two different files — 17,730,509,792 bytes and 18,209,036,576 — and Epoch, which named a
//! model after its file's stem, showed them as one row with one tuning entry, one loadout, one set
//! of benchmark results and one shelf link.
//!
//! Renaming one of them worked and is not a fix. **A filename is a claim** (ADR-0024), and the
//! next person to download two variants of one model meets the same thing.
//!
//! ## Two questions, and they had one answer
//!
//! - **Which model is this?** `Qwen3.6-35B-A3B`. Several files are versions of it.
//! - **Which file is this?** The IQ4_XS one, from the MTP repository, 18.2 GB, with a `nextn`
//!   block in it.
//!
//! The first is what a person says out loud. The second is what a measurement is *about*, and
//! what every store here has to key on: a benchmark of one is not a benchmark of the other.
//!
//! ## Everything here is read from the file
//!
//! Nothing is parsed out of a name. The distinguishing facts come from the GGUF header — which is
//! a few kilobytes, so this stays usable on a read path (ADR-0032 keeps hashing off one):
//!
//! - **MTP**: `<arch>.nextn_predict_layers`. Measured by comparing the two artefacts above —
//!   the MTP file is the plain one plus twenty `blk.40.*` tensors including
//!   `blk.40.nextn.eh_proj`, and its header carries `nextn_predict_layers = 1` where the plain
//!   file carries nothing.
//! - **Vision**: a projector beside it, which is a fact about the folder rather than the file.
//!
//! ## What an id is, and what it is not
//!
//! An id has to be **stable** (the same file answers the same tomorrow), **unique** (two
//! different files never collide) and **independent of what else is on the machine** (adding a
//! third variant must not renumber the first two).
//!
//! So it is the stem plus whatever the header says distinguishes this file, and nothing else. A
//! file with no distinguishing features keeps exactly the id it had before this module existed —
//! which is nearly every model here, and not quite all of them: see [`Identity::id`] for the one
//! that turned out to have an MTP head nobody had noticed.
//!
//! **It is deliberately not a content hash.** A hash is the perfect answer and costs 3.4 s for a
//! 6.2 GB model here; the deck opens often. `recipes` hashes after a render, where the cost is
//! already paid, and this does not.

use std::path::Path;

use serde::{Deserialize, Serialize};

/// Something a file carries that another file of the same name might not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Feature {
    /// It carries a multi-token-prediction head, so `--spec-type draft-mtp` can use it.
    Mtp,
}

impl Feature {
    /// The tag that goes in an id and on a label.
    pub fn tag(self) -> &'static str {
        match self {
            Feature::Mtp => "MTP",
        }
    }
}

/// Which file this is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    /// The file's stem, which is what a person recognises and what every older store keyed on.
    pub stem: String,
    /// What the file says it is called. `None` where the header does not say.
    pub name: Option<String>,
    pub architecture: Option<String>,
    /// Sorted, so an id built from them is stable whatever order they were found in.
    pub features: Vec<Feature>,
    pub bytes: u64,
}

impl Identity {
    /// Read one, from the file's own header.
    ///
    /// **Never fails into a guess.** A file whose header cannot be read gets an identity with no
    /// features — which is *nothing distinguishing was found*, and it keeps the plain stem. That
    /// is the same answer as a file that genuinely has no features, and it is the honest one:
    /// inventing a variant tag for a file nobody could read would be worse than treating it as
    /// ordinary.
    pub fn read(path: &Path) -> Self {
        let stem = path
            .file_stem()
            .map(|it| it.to_string_lossy().into_owned())
            .unwrap_or_default();
        let bytes = std::fs::metadata(path).map(|it| it.len()).unwrap_or(0);

        let Ok(header) = crate::gguf::read(path) else {
            return Self {
                stem,
                name: None,
                architecture: None,
                features: Vec::new(),
                bytes,
            };
        };

        let mut features = Vec::new();
        // Measured: the MTP artefact of `Qwen3.6-35B-A3B` carries `nextn_predict_layers = 1` and
        // the plain one carries the key not at all. Read under the file's own architecture
        // namespace, so this is not a fact about Qwen.
        if header
            .about("nextn_predict_layers")
            .and_then(crate::gguf::Value::number)
            .is_some_and(|it| it > 0)
        {
            features.push(Feature::Mtp);
        }
        features.sort();

        Self {
            stem,
            name: header
                .said
                .get("general.name")
                .and_then(|it| it.text())
                .map(str::to_owned),
            architecture: header.architecture().map(str::to_owned),
            features,
            bytes,
        }
    }

    /// The key every store uses.
    ///
    /// **A file with nothing distinguishing keeps its bare stem.**
    ///
    /// That was expected to make the change free, and it very nearly was — with one measured
    /// exception worth naming rather than glossing. `Qwen3.8-27B-Uncensored-IQ2-M`, already on
    /// this machine and timed at 10.6 tok/s, turns out to carry `qwen35.nextn_predict_layers = 1`
    /// and four `blk.64.nextn.*` tensors: it has an MTP head, nobody knew, and Epoch had never
    /// offered it `draft-mtp`. Its id gains a tag, so its old timing is keyed under a name
    /// nothing looks up any more.
    ///
    /// That is the right trade and it is a real cost. The alternative — tagging only where a
    /// collision exists — is order-dependent: a third variant arriving would rename the first
    /// two, and an unrelated download would orphan measurements that had nothing to do with it.
    pub fn id(&self) -> String {
        if self.features.is_empty() {
            return self.stem.clone();
        }
        let tags: Vec<&str> = self.features.iter().map(|it| it.tag()).collect();
        format!("{} \u{00b7} {}", self.stem, tags.join(" \u{00b7} "))
    }

    /// Which model this is a version of, as a person would say it.
    ///
    /// The header's own `general.name` where there is one — `Qwen3.6-35B-A3B` for every quant and
    /// both variants — and the stem otherwise. **A grouping, never a key**: two artefacts sharing
    /// this is the normal case and the whole reason ids are separate.
    pub fn logical(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.stem)
    }

    /// Whether this file can be asked for `--spec-type draft-mtp`.
    pub fn has_mtp(&self) -> bool {
        self.features.contains(&Feature::Mtp)
    }
}

/// Whether two artefacts are the same file, as far as anything cheap can tell.
///
/// **Size and id, and it is stated as a likeness rather than proof.** Two files agreeing on both
/// are the same file for every purpose here; proving it needs a hash, and a hash on a read path
/// is what ADR-0032 keeps off one.
pub fn probably_the_same(a: &Identity, b: &Identity) -> bool {
    a.bytes == b.bytes && a.id() == b.id() && a.bytes > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(stem: &str, bytes: u64) -> Identity {
        Identity {
            stem: stem.to_owned(),
            name: Some("Qwen3.6-35B-A3B".to_owned()),
            architecture: Some("qwen35moe".to_owned()),
            features: Vec::new(),
            bytes,
        }
    }

    #[test]
    fn a_file_with_nothing_distinguishing_keeps_its_bare_stem() {
        /*
            What keeps the change nearly free: a model with no distinguishing feature keeps every
            key it had in `tuning.json`, `loadouts.json`, `optimized.json` and `benchmarks.json`.

            Nearly, not entirely — see `id`. One model already here turned out to have an MTP
            head nobody had noticed, and it does gain a tag.
        */
        assert_eq!(
            plain("Qwen3.6-35B-A3B-UD-IQ4_XS", 17_730_509_792).id(),
            "Qwen3.6-35B-A3B-UD-IQ4_XS",
        );
    }

    #[test]
    fn two_files_of_one_name_do_not_collide_once_one_carries_a_head() {
        /*
            The collision this module exists for, measured 2026-08-31: `unsloth/…-GGUF` and
            `unsloth/…-MTP-GGUF` publish `Qwen3.6-35B-A3B-UD-IQ4_XS.gguf` and the files are
            17,730,509,792 and 18,209,036,576 bytes.
        */
        let a = plain("Qwen3.6-35B-A3B-UD-IQ4_XS", 17_730_509_792);
        let b = Identity {
            features: vec![Feature::Mtp],
            bytes: 18_209_036_576,
            ..plain("Qwen3.6-35B-A3B-UD-IQ4_XS", 0)
        };
        assert_ne!(a.id(), b.id());
        assert_eq!(b.id(), "Qwen3.6-35B-A3B-UD-IQ4_XS · MTP");
        assert!(!probably_the_same(&a, &b));
    }

    #[test]
    fn four_variants_of_one_model_are_four_ids_and_one_name() {
        // The owner's four, coexisting without a rename: two quants times plain and MTP.
        let every = [
            plain("Qwen3.6-35B-A3B-UD-IQ4_XS", 17_730_509_792),
            Identity {
                features: vec![Feature::Mtp],
                bytes: 18_209_036_576,
                ..plain("Qwen3.6-35B-A3B-UD-IQ4_XS", 0)
            },
            plain("Qwen3.6-35B-A3B-UD-IQ2_M", 11_522_702_304),
            Identity {
                features: vec![Feature::Mtp],
                bytes: 11_882_969_376,
                ..plain("Qwen3.6-35B-A3B-UD-IQ2_M", 0)
            },
        ];
        let ids: std::collections::BTreeSet<String> = every.iter().map(Identity::id).collect();
        assert_eq!(ids.len(), 4, "{ids:?}");
        assert!(
            every.iter().all(|it| it.logical() == "Qwen3.6-35B-A3B"),
            "and all four are one model",
        );
    }

    #[test]
    fn an_id_does_not_depend_on_what_else_is_on_the_machine() {
        /*
            The alternative — disambiguating only where a collision exists — is order-dependent:
            downloading a third variant would rename the first two, and every measurement keyed on
            the old names would be orphaned by an unrelated download.
        */
        let alone = Identity {
            features: vec![Feature::Mtp],
            ..plain("m", 1)
        };
        assert_eq!(
            alone.id(),
            "m · MTP",
            "the same whether or not a plain `m` exists"
        );
    }

    #[test]
    fn a_file_nobody_could_read_is_ordinary_rather_than_a_guess() {
        // Inventing a variant tag for an unreadable header would be worse than treating it as
        // plain: it would key its measurements under something nothing else agrees with.
        let unreadable = Identity::read(Path::new("nothing-here.gguf"));
        assert!(unreadable.features.is_empty());
        assert_eq!(unreadable.id(), "nothing-here");
        assert_eq!(unreadable.bytes, 0);
    }

    #[test]
    fn features_are_sorted_so_an_id_is_stable() {
        // One feature today. The sort is what stops two of them producing two ids for one file
        // depending on the order the header happened to be read in.
        let mut it = plain("m", 1);
        it.features = vec![Feature::Mtp, Feature::Mtp];
        it.features.sort();
        it.features.dedup();
        assert_eq!(it.id(), "m · MTP");
    }
}
