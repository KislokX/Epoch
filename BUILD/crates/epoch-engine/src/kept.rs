//! Benchmark results, written down.
//!
//! ## Why they are kept at all
//!
//! A benchmark run is minutes per model. Losing it because a window closed would make the whole
//! thing a thing nobody does twice — and the point of a versioned suite is that a card measured
//! in March can be read in August.
//!
//! ## What replaces what
//!
//! A card is keyed by **model, card, build, runtime, context and suite version together**, and
//! only an exact match is replaced. Every one of those changes the answer, so a new reading of a
//! different configuration is a new row rather than an overwrite: two contexts of one model are
//! two facts, and a table that kept only the last would answer *how fast is it* with *how fast is
//! it at whatever somebody tried most recently*.
//!
//! That is the same rule `loadout::Loadouts` follows, arrived at the same way — it lost a
//! measurement to a key that was too narrow, and gained the runtime to its key when a curve could
//! be taken on two of them.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::card::Card;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Kept {
    #[serde(default)]
    cards: Vec<Card>,
}

pub fn path(library: &Path) -> PathBuf {
    library.join("benchmarks.json")
}

impl Kept {
    /// Never fails. An unreadable file is an empty memory, and a benchmark that refused to run
    /// because an old result would not parse would be losing the new one over the old one.
    pub fn load(library: &Path) -> Self {
        std::fs::read_to_string(path(library))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, library: &Path) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(self).map_err(|why| why.to_string())?;
        if let Some(home) = path(library).parent() {
            std::fs::create_dir_all(home).map_err(|why| why.to_string())?;
        }
        std::fs::write(path(library), raw).map_err(|why| why.to_string())
    }

    /// Keep one, replacing only a reading of exactly the same thing.
    pub fn remember(&mut self, card: Card) {
        self.cards.retain(|one| !same_question(one, &card));
        self.cards.push(card);
    }

    pub fn all(&self) -> &[Card] {
        &self.cards
    }

    /// Every card for one model, newest first.
    pub fn about(&self, model: &str) -> Vec<&Card> {
        let mut found: Vec<&Card> = self.cards.iter().filter(|it| it.model == model).collect();
        found.sort_by_key(|it| std::cmp::Reverse(it.conditions.at));
        found
    }

    /// Forget one model's results — everything about it, on every configuration.
    pub fn forget(&mut self, model: &str) {
        self.cards.retain(|it| it.model != model);
    }
}

/// Whether two cards are readings of the same question.
///
/// **Six things, and every one of them changes the answer.** A card is only replaced by a card
/// that would have replaced it in reality: same model, same graphics, same build, same runtime,
/// same context, same suite.
fn same_question(a: &Card, b: &Card) -> bool {
    a.model == b.model
        && a.conditions.gpu == b.conditions.gpu
        && a.conditions.build == b.conditions.build
        && a.conditions.runtime == b.conditions.runtime
        && a.conditions.context == b.conditions.context
        && a.conditions.suite == b.conditions.suite
        && a.conditions.tuning == b.conditions.tuning
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Conditions;

    fn card(model: &str, context: u32, at: u64) -> Card {
        Card {
            model: model.to_owned(),
            conditions: Conditions {
                suite: 1,
                gpu: "a card".to_owned(),
                build: "b1".to_owned(),
                runtime: "llama_cpp".to_owned(),
                context,
                tuning: crate::models::tuning::Tuning::default(),
                at,
            },
            speed: crate::bench::Measured::default(),
            answers: Vec::new(),
        }
    }

    #[test]
    fn measuring_the_same_thing_again_replaces_it() {
        let mut kept = Kept::default();
        kept.remember(card("m", 32_768, 1));
        kept.remember(card("m", 32_768, 2));
        assert_eq!(kept.all().len(), 1);
        assert_eq!(kept.all()[0].conditions.at, 2, "the newer reading");
    }

    #[test]
    fn two_contexts_of_one_model_are_two_facts() {
        /*
            A table that kept only the last would answer *how fast is it* with *how fast is it at
            whatever somebody tried most recently* — and the Limit phase exists precisely to
            produce several contexts of one model.
        */
        let mut kept = Kept::default();
        kept.remember(card("m", 32_768, 1));
        kept.remember(card("m", 131_072, 2));
        assert_eq!(kept.all().len(), 2);
        assert_eq!(kept.about("m").len(), 2);
        assert_eq!(kept.about("m")[0].conditions.at, 2, "newest first");
    }

    #[test]
    fn a_different_build_is_a_different_answer() {
        // A release that changes kernels changes the number, and a remembered measurement that
        // could not tell would go on describing a machine that had got faster.
        let mut kept = Kept::default();
        kept.remember(card("m", 32_768, 1));
        let mut newer = card("m", 32_768, 2);
        newer.conditions.build = "b2".to_owned();
        kept.remember(newer);
        assert_eq!(kept.all().len(), 2);
    }

    #[test]
    fn a_file_that_will_not_parse_is_an_empty_memory() {
        // Refusing to run because an old result is unreadable would lose the new one over the
        // old one.
        let here = std::env::temp_dir().join(format!("epoch-kept-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&here);
        std::fs::write(path(&here), "{ this is not json").expect("written");
        assert!(Kept::load(&here).all().is_empty());
        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    fn what_was_written_reads_back() {
        let here = std::env::temp_dir().join(format!("epoch-kept-ok-{}", std::process::id()));
        let mut kept = Kept::default();
        kept.remember(card("m", 32_768, 7));
        kept.save(&here).expect("saved");
        let read = Kept::load(&here);
        assert_eq!(read.all().len(), 1);
        assert_eq!(read.all()[0].conditions.at, 7);
        let _ = std::fs::remove_dir_all(&here);
    }
}
