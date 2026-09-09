//! Every model on this machine, with what is known about how it runs.
//!
//! ## Why this is not in a surface
//!
//! It was. `models_and_loadouts`, `recommended_of` and `choose_loadout` lived in the desktop
//! shell, which meant the companion could not show a recipe at all — its Models page offered
//! *delete* and *time it* and nothing else, because the third thing was somewhere it could not
//! reach.
//!
//! Copying them across would have been worse than the gap: **two answers to one question.** The
//! rule this file exists to hold is *which row of a measured curve does Epoch mark*, and a
//! second spelling of it is how two surfaces come to disagree about the same model on the same
//! card — the exact failure `recommended_of`'s own comment warned about, one crate up.
//!
//! Nothing here touches a network, hashes a file, or loads a model. Everything on a row is
//! already on the disk or already in a file Epoch wrote when it measured something: opening a
//! Models deck must not cost a graphics card.

use serde::{Deserialize, Serialize};

use crate::loadout::{self, Loadout, Loadouts, Tried};
use crate::runtimes::Weights;

/// One model, as a deck draws it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelHere {
    pub name: String,
    /// `Ollama` or `Saved here`. The shelf decides what removing it means.
    pub from: String,
    pub path: String,
    pub bytes: u64,
    /// Whether it has a projector beside it, measured from the headers rather than the name.
    pub sees: bool,
    /// What it was trained to hold, which bounds what any loadout may ask for.
    pub trained_context: Option<u32>,
    /// What it will be loaded with the next time llama.cpp starts.
    pub loadout: Loadout,
    /// Whether that came from a search, or is the safe default nobody has improved on.
    pub measured: bool,
    /// The whole curve, when one has been mapped. Empty otherwise.
    pub readings: Vec<Tried>,
    /// Which row Epoch would mark, out of that curve.
    pub recommended: Option<Loadout>,
    /// The newest timed answer, if anything ever timed one.
    pub tokens_per_second: Option<f64>,
    pub measured_on: Option<String>,
    /// Which runtime the **curve** was mapped on. `None` when none has been.
    ///
    /// Separate from `measured_on`, which is the runtime of the last *timing*. They are two
    /// different measurements and can honestly disagree — and a curve that did not say which
    /// program it is about would be the gauge that identifies nobody, on a panel where the same
    /// model can now carry one from each.
    pub curve_on: Option<String>,
    /// What this model is **told** — flash attention, speculative decoding. A choice, never a
    /// measurement, which is why it is beside the curve rather than inside it (`tuning.rs`).
    pub tuning: crate::tuning::Tuning,
    /// GGUFs on this machine that could serve as this one's draft or MTP model.
    ///
    /// **Every one of them, and Epoch does not claim to know which is right.** Which draft goes
    /// with which model is not something a filename may answer (ADR-0024) and nothing has been
    /// measured about an MTP file's own header here — so the list is offered and the person
    /// chooses, rather than Epoch guessing and being confidently wrong about a file it has never
    /// opened.
    pub drafts: Vec<String>,
    /// The other rows that are **this same model, stored again**.
    ///
    /// Named rather than folded away: both files are really on the disk, and removing one row
    /// must not remove the other. What the deck was not saying is that 21.2 GB was one model —
    /// an `ollama create` of a GGUF the vault already held copies it, so two rows appeared with
    /// no hint that either could go.
    ///
    /// **Never a fold, and never a guess.** A row with the same *size* on a different physical
    /// file is a candidate; it becomes a claim only when Epoch already holds a hash for both and
    /// they agree. Hashing here is refused outright — this runs every time the deck opens, and
    /// ADR-0032 keeps hashing off a read path.
    pub same_weights_as: Vec<String>,
}

/// Which of these rows are the same weights stored more than once.
///
/// Same size, a different path — and, where Epoch already wrote a hash beside both, the same
/// hash. The hash is only ever read, never computed.
///
/// **A hard link is not a duplicate**, and this deliberately does not check for one. It cannot
/// arise: the list is Ollama's own manifests plus the `.gguf` files in the vault, which are two
/// disjoint sets — the shelf's links live somewhere neither of them looks. Reading the
/// filesystem's file identity was the first attempt and it needs a nightly API, which is a poor
/// trade for guarding a case that does not exist. If a third source is ever added here, this is
/// the assumption that has to be re-checked.
pub fn stored_twice(rows: &[ModelHere]) -> Vec<Vec<String>> {
    let hashes: Vec<Option<String>> = rows
        .iter()
        .map(|one| crate::catalogue::hash_beside(std::path::Path::new(&one.path)))
        .collect();

    rows.iter()
        .enumerate()
        .map(|(i, one)| {
            rows.iter()
                .enumerate()
                .filter(|(j, other)| {
                    *j != i
                        && other.bytes == one.bytes
                        && one.bytes > 0
                        && other.path != one.path
                        // Two recorded hashes that disagree settle it: not the same file,
                        // whatever the sizes say. One missing hash leaves size as the evidence,
                        // and the surface words it as what it is.
                        && match (&hashes[i], &hashes[*j]) {
                            (Some(a), Some(b)) => a == b,
                            _ => true,
                        }
                })
                .map(|(_, other)| other.name.clone())
                .collect()
        })
        .collect()
}

/// The smallest context worth loading anything with: a real turn, plus room to answer.
pub fn floor() -> u32 {
    (loadout::A_REAL_TURN + loadout::ROOM_TO_ANSWER).next_power_of_two()
}

/// How a model nobody has measured will be loaded.
///
/// The safe half of every trade: the compressed cache costs a model that did not need it about
/// three percent, and the full-precision one costs a model that did need it two thirds. An
/// unmeasured model is therefore merely not optimal rather than slow.
pub fn conservative_default() -> Loadout {
    loadout::conservative(floor())
}

/// What is known about every model a surface found.
///
/// The caller supplies the models because *where they live* is the surface's question — the
/// desktop keeps a vault beside itself, a lent machine keeps its own — while *what counts as a
/// model* is this crate's.
pub fn about(held: Vec<Weights>) -> Vec<ModelHere> {
    let library = crate::generative::Library::here();
    let known = Loadouts::load(library.root());
    let speeds = crate::speeds::Speeds::load(library.root());
    let card = crate::machine::Machine::measure().gpu.unwrap_or_default();
    let build = crate::runtimes::llama_build();
    let told = crate::tuning::Tunings::load(library.root());
    /*
        **Every other GGUF here is a possible draft, and Epoch does not narrow the list.**

        A draft or MTP model is a second file loaded beside the first, and which one belongs to
        which is not a question a *filename* may answer (ADR-0024). Nothing has been measured
        about what an MTP file's own header says, so the honest offer is the whole list with the
        person choosing — rather than a guess that would be confidently wrong about a file nobody
        opened. A model is left out of its own list: it cannot draft for itself.
    */
    let every: Vec<String> = held.iter().map(|one| one.path.clone()).collect();

    let mut rows: Vec<ModelHere> = held
        .into_iter()
        .map(|one| {
            let names = loadout::names(std::path::Path::new(&one.path));
            let best = names
                .as_deref()
                .and_then(|names| known.about(names, &card, &build));
            // The newest timed answer for this model, whichever runtime took it. A speed on a
            // row is only ever a measurement: an estimate belongs beside a model somebody does
            // not have, where there is nothing better to say.
            let timed = speeds.about(&one.name).into_iter().next();
            ModelHere {
                loadout: best
                    .map(|best| best.chose)
                    .unwrap_or_else(|| loadout::conservative(floor())),
                measured: best.is_some(),
                readings: best.map(|best| best.tried.clone()).unwrap_or_default(),
                recommended: best.and_then(|best| recommended_of(&best.tried)),
                tokens_per_second: timed.map(|one| one.tokens_per_second),
                measured_on: timed.map(|one| one.runtime.clone()),
                curve_on: best.map(|best| best.runtime.clone()),
                tuning: told.of(&one.name),
                drafts: every
                    .iter()
                    .filter(|at| *at != &one.path)
                    .cloned()
                    .collect(),
                trained_context: crate::gguf::read(std::path::Path::new(&one.path))
                    .ok()
                    .and_then(|header| header.trained_context())
                    .and_then(|n| u32::try_from(n).ok()),
                sees: one.sees_with.is_some(),
                name: one.name,
                from: one.from,
                path: one.path,
                bytes: one.bytes,
                same_weights_as: Vec::new(),
            }
        })
        .collect();

    // Filled in a second pass: whether a row is a second copy is a fact about the *list*, and
    // nothing about one file can answer it.
    let twins = stored_twice(&rows);
    for (row, also) in rows.iter_mut().zip(twins) {
        row.same_weights_as = also;
    }
    rows
}

/// What this machine can actually run, most used first.
///
/// **Shared for the same reason the rows above are.** The desktop had this and the companion did
/// not, so the machine that actually holds the weights could not be asked the one question
/// somebody standing at it wants answered — *what should I run here?* — while the Host answered
/// it about a different computer.
///
/// Costs a moment and a page of requests, which is why both surfaces put it behind a button.
///
/// ## The card, not what happens to be on it this second
///
/// Free memory was the obvious reading and it made the list contradict itself: timing a model
/// loads it, so re-reading the list a minute later answered *nothing fits* — every number
/// honest, and the whole page wrong about the question somebody asked. What they asked is
/// whether this machine **can** run a model, and that is a fact about the card. Free memory is
/// still the right reading for *will this run right now*, which is what WEIGH answers.
pub fn what_fits(held: &[Weights]) -> Result<Vec<crate::Suited>, String> {
    let card = crate::machine::Machine::measure()
        .vram_total
        .ok_or_else(|| "Epoch has not measured this machine's video memory".to_owned())?;
    let library = crate::generative::Library::here();
    let speeds = crate::speeds::Speeds::load(library.root());
    crate::suited_to(card, &installed(held), &speeds, 20, 80)
}

/// What this machine already holds, in the shape the fitter reads.
///
/// **Everything on the disk, not only what Ollama filed.** This asked Ollama alone once, on the
/// reasoning that Ollama is the runtime that reports a size — true of *runtimes*, and beside the
/// point for a file, whose size is right there on the disk. A GGUF the Workshop saved was
/// therefore invisible to *what fits on this machine*, which offered a DOWNLOAD for ten
/// gigabytes already here.
pub fn installed(held: &[Weights]) -> Vec<crate::Installed> {
    held.iter()
        .map(|one| crate::Installed {
            name: one.name.clone(),
            runtime: one.from.clone(),
            bytes: one.bytes,
        })
        .collect()
}

/// Which row of a measured curve Epoch marks.
///
/// The same rule as [`loadout::Map::recommended`], over a curve read back from disk rather than
/// one just taken: the largest context that costs no real speed, out of those that can hold a
/// turn.
pub fn recommended_of(tried: &[Tried]) -> Option<Loadout> {
    loadout::Map {
        readings: tried
            .iter()
            .map(|one| loadout::Reading {
                loadout: one.loadout,
                tokens_per_second: one.tokens_per_second,
                holds_a_turn: one.loadout.context >= loadout::A_REAL_TURN + loadout::ROOM_TO_ANSWER,
            })
            .collect(),
    }
    .recommended()
}

/// Take the user's pick out of a measured curve, and remember it.
///
/// Returns the sentence a surface shows. `held` is the same list a deck was drawn from, so a
/// path that is not in it is refused rather than acted on — the surface and the Engine agree on
/// what exists because they were looking at one list.
pub fn choose(held: &[Weights], path: &str, context: u32) -> Result<String, String> {
    let one = held
        .iter()
        .find(|held| held.path == path)
        .ok_or("that is not a model this machine reported holding")?;
    let names = loadout::names(std::path::Path::new(&one.path)).ok_or("that file is not there")?;
    let card = crate::machine::Machine::measure().gpu.unwrap_or_default();
    let build = crate::runtimes::llama_build();

    let library = crate::generative::Library::here();
    let mut known = Loadouts::load(library.root());
    let mut best = known
        .about(&names, &card, &build)
        .cloned()
        .ok_or("nothing has been measured for this model yet")?;
    let picked = best
        .tried
        .iter()
        .filter(|r| r.loadout.context == context && r.tokens_per_second.is_some())
        // Two rows can share a context — one per cache — and the faster of them is the one a
        // person clicking a context means.
        .max_by(|a, b| {
            a.tokens_per_second
                .partial_cmp(&b.tokens_per_second)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .copied()
        .ok_or("that setting is not one this machine measured")?;
    best.chose = picked.loadout;
    best.tokens_per_second = picked.tokens_per_second.unwrap_or_default();
    known.remember(best);
    known.save(library.root())?;
    Ok(format!(
        "{} will load with {} tokens of context on the {}. It takes effect the next time \
         llama.cpp starts.",
        one.name,
        picked.loadout.context,
        picked.loadout.cache.plainly(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadout::Cache;

    fn tried(context: u32, cache: Cache, rate: f64) -> Tried {
        Tried {
            loadout: Loadout { context, cache },
            tokens_per_second: Some(rate),
        }
    }

    fn row(name: &str, path: &str, bytes: u64) -> ModelHere {
        ModelHere {
            name: name.into(),
            from: "Ollama".into(),
            path: path.into(),
            bytes,
            sees: false,
            trained_context: None,
            loadout: conservative_default(),
            measured: false,
            readings: Vec::new(),
            recommended: None,
            tokens_per_second: None,
            measured_on: None,
            curve_on: None,
            tuning: crate::tuning::Tuning::default(),
            drafts: Vec::new(),
            same_weights_as: Vec::new(),
        }
    }

    /// **Named, never folded.** Both files are on the disk and deleting one row must not delete
    /// the other — the deck's job is to say that 21.2 GB is one model, not to hide half of it.
    #[test]
    fn the_same_weights_stored_twice_are_named_on_both_rows() {
        let rows = vec![
            row("qwen-ollama", "/nowhere/a.gguf", 10_624_771_968),
            row("qwen-saved", "/nowhere/b.gguf", 10_624_771_968),
            row("gemma", "/nowhere/c.gguf", 7_400_000_000),
        ];
        let twins = stored_twice(&rows);
        assert_eq!(twins[0], vec!["qwen-saved".to_string()]);
        assert_eq!(twins[1], vec!["qwen-ollama".to_string()]);
        assert!(twins[2].is_empty(), "a size nobody shares is not a twin");
    }

    #[test]
    fn a_row_is_never_its_own_twin_and_an_empty_file_is_nobodys() {
        let rows = vec![
            row("a", "/nowhere/a.gguf", 0),
            row("b", "/nowhere/b.gguf", 0),
        ];
        // Zero bytes is not evidence of anything, and two of them are not a pair.
        assert!(stored_twice(&rows).iter().all(Vec::is_empty));

        let alone = vec![row("only", "/nowhere/only.gguf", 100)];
        assert!(stored_twice(&alone)[0].is_empty());
    }

    #[test]
    fn the_marked_row_is_the_roomiest_that_costs_no_real_speed() {
        // Measured on this machine for gemma4-12b. 8192 is faster than everything and cannot
        // hold a turn, so it is not the answer however fast it reads.
        let curve = [
            tried(8_192, Cache::Q8_0, 35.00),
            tried(16_384, Cache::F16, 20.28),
            tried(16_384, Cache::Q8_0, 35.15),
            tried(24_576, Cache::Q8_0, 18.53),
        ];
        let marked = recommended_of(&curve).expect("a curve has a recommendation");
        assert_eq!(marked.context, 16_384);
        assert_eq!(marked.cache, Cache::Q8_0);
    }

    #[test]
    fn nothing_measured_recommends_nothing_rather_than_guessing() {
        assert_eq!(recommended_of(&[]), None);
    }

    #[test]
    fn a_model_this_machine_does_not_hold_is_refused_rather_than_chosen_for() {
        let said = choose(&[], "C:/nowhere/model.gguf", 16_384).unwrap_err();
        assert!(said.contains("reported holding"), "{said}");
    }
}
