//! Finding the speculative decoding that is actually fastest, on this machine, for this model.
//!
//! ## Why this cannot be answered from a table
//!
//! Speculative decoding drafts several tokens cheaply and then verifies them with the real model.
//! Where the draft is right, several tokens cost one pass; where it is wrong, the pass is wasted.
//! **Whether that is a win depends on the model, the card, the quantisation, how much of the
//! model is on the card, and the text being written** — which is five things nobody's published
//! number knows about your computer.
//!
//! So it is measured, one configuration at a time, and the answer is *for this machine*.
//!
//! ## The one thing everyone gets wrong, and this file refuses to
//!
//! > *No asumir que 3 de 4 tokens aceptados significa 4x.*
//!
//! An acceptance rate is not a speed-up. It cannot be, and the first live run of the benchmark
//! runner proved it on this hardware: `ngram-simple` on a 270M model accepted **100%** of what it
//! drafted and was **2.47×** faster, not 40×. An ngram speculator only drafts where it already
//! has a match, so a perfect rate is ordinary there and says nothing about time saved.
//!
//! Every speed-up here is two measured medians divided. Acceptance is recorded beside it because
//! it explains *why*, and it is never used to compute anything.
//!
//! ## Two stages, not one grid
//!
//! Eleven types times six draft lengths is sixty-six configurations, and each one is a model load
//! — minutes for a large model. Sixty-six of those is an evening, for an answer nobody waits for.
//!
//! So it is done the way somebody tuning by hand would:
//!
//! 1. **Sweep the draft length** on one type, from short to long. This is where the sweet spot
//!    lives and it is the axis the owner asked to start conservatively on.
//! 2. **Compare the types** at whichever length won.
//!
//! That is `lengths + types` runs rather than `lengths × types`. It assumes the best length is
//! roughly the best length for every type, which is an assumption and is written down as one —
//! it is not measured, and the day somebody wants the full grid, the ladder below is where it
//! goes.
//!
//! ## And more drafting is not more speed
//!
//! `--spec-draft-n-max` is how many tokens to guess before checking. Guess too few and the
//! verification pass is barely amortised; guess too many and every miss throws away more work.
//! llama.cpp's own default is 3. The ladder deliberately goes below and above it, because the
//! point is to find out rather than to confirm.

use serde::{Deserialize, Serialize};

use crate::bench::{Ask, Measured};
use crate::models::tuning::{Speculation, Tuning, SPEC_TYPES};

/// How many tokens to draft, tried in this order.
///
/// **Short first**, so a machine where speculation is a loss says so early and cheaply. `3` is
/// llama.cpp's own default and is in the middle of the ladder rather than at the start of it: a
/// sweep that began at the default would be checking a belief rather than measuring an axis.
pub const LENGTHS: [u32; 5] = [1, 2, 3, 5, 8];

/// One configuration, run and recorded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tried {
    /// How it reads on a row: `off`, `ngram-simple n=3`.
    pub label: String,
    pub tuning: Tuning,
    pub measured: Measured,
    /// The same configuration asked the **same question three times**.
    ///
    /*
        **Two numbers because there are two workloads, and one of them is where this pays.**

        Measured 2026-08-31 on `gemma4:12b`, build 10622, one card. Of the five ngram types this
        build offers, exactly one — `ngram-mod` — ever drafted anything at all; the other four
        passed their flag to the child, were accepted, and drafted nothing on any prompt. And when
        `ngram-mod` did draft:

        | | drafted / accepted | rate |
        |---|---|---|
        | repeating a paragraph it had just written | 128 / 128 | **182.5 t/s** |
        | a question it had not seen | *nothing* | **41.3 t/s** |
        | the repeat again | 128 / 24 | 47.7 t/s |

        Baseline was 47.0. So on unseen text it is a **loss**, and on repeated text it is nearly
        four times faster — and an average of those two would describe neither.

        `measured` is the honest headline: three different questions, which is what stops a run
        priming the next and is what a benchmark must do. `repeated` is the best case, labelled as
        one, because it is not a trick — a Quest that re-reads a file, an agent looping over a
        diff, a model quoting its own plan back all live there.
    */
    pub repeated: Measured,
    /// What the machine was doing while it ran.
    pub load: crate::models::load::Load,
    /// This configuration's median generation rate over the baseline's, on unseen text. `None`
    /// where either was never measured — **never 1.0**, which would read as *no difference*.
    pub speedup: Option<f64>,
    /// The same, on text the model has already written. The best case, and it is a real case.
    pub speedup_repeating: Option<f64>,
    /// Whether it wrote the same text as the baseline did.
    ///
    /// `None` for the baseline itself and where either side produced no digest to compare.
    /// Speculative decoding is supposed to change the speed and nothing else, so a `false` here
    /// is more serious than a slow result: it means this configuration is a different model.
    pub same_answers: Option<bool>,
    /// Every run finished, and it produced a rate.
    pub stable: bool,
    /// What went wrong, in the server's own words.
    pub failures: Vec<String>,
}

impl Tried {
    pub fn generation(&self) -> Option<f64> {
        self.measured.generation()
    }

    /// Tokens kept per draft attempt: accepted over the number of runs that drafted.
    ///
    /// Reported beside the acceptance *rate* because they answer different questions — a rate of
    /// 0.5 at a draft length of 8 keeps four tokens a pass and the same rate at 2 keeps one.
    pub fn accepted_per_draft(&self) -> Option<f64> {
        let accepted: u64 = self.measured.runs.iter().filter_map(|r| r.accepted).sum();
        let passes = self
            .measured
            .runs
            .iter()
            .filter(|r| r.drafted.is_some_and(|it| it > 0))
            .count();
        (passes > 0).then(|| accepted as f64 / passes as f64)
    }
}

/// A whole sweep: the baseline, everything tried against it, and which one won.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sweep {
    pub model: String,
    /// Every configuration in the order it was run, the baseline first.
    pub tried: Vec<Tried>,
    /// The label of the fastest configuration that was stable and wrote the same text.
    ///
    /// `None` where nothing beat the baseline, which is a real and common answer and must not be
    /// dressed up as a winner.
    pub best: Option<String>,
}

impl Sweep {
    /// Which configuration to actually use.
    ///
    /// **Three conditions, and none of them is negotiable.** It has to be stable (every run
    /// finished), it has to have written the same text as the baseline, and it has to be faster
    /// than the baseline by enough to be a result rather than noise.
    ///
    /// The margin is 2%: two runs of one configuration on this machine differ by about that much
    /// on their own, so a 1% win is a coin toss with a decimal point on it. Below the margin the
    /// honest answer is the baseline, because *off* costs no memory and has nothing to go wrong.
    pub fn pick(&self) -> Option<&Tried> {
        let baseline = self.tried.first()?.generation()?;
        self.tried
            .iter()
            .skip(1)
            .filter(|it| it.stable)
            // `Some(false)` is disqualifying; `None` is not. A configuration whose answers could
            // not be compared is unproven rather than wrong — the same rule as everywhere else
            // here — and it stays in only because it must still beat the baseline on speed.
            .filter(|it| it.same_answers != Some(false))
            .filter(|it| it.generation().is_some_and(|it| it > baseline * 1.02))
            .max_by(|a, b| {
                a.generation()
                    .unwrap_or(0.0)
                    .partial_cmp(&b.generation().unwrap_or(0.0))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// The table, as a person reads it.
    pub fn table(&self) -> Vec<String> {
        let baseline = self.tried.first().and_then(Tried::generation);
        self.tried
            .iter()
            .map(|it| {
                let rate = it
                    .generation()
                    .map(|it| format!("{it:>7.1} t/s"))
                    .unwrap_or_else(|| "  no answer".to_owned());
                let speedup = match (it.speedup, baseline) {
                    (Some(x), _) => format!("{x:>5.2}x"),
                    (None, Some(_)) => "    \u{2014}".to_owned(),
                    (None, None) => " 1.00x".to_owned(),
                };
                let acceptance = it
                    .measured
                    .acceptance()
                    .map(|it| format!("accept {:>5.1}%", it * 100.0))
                    .unwrap_or_else(|| "nothing drafted".to_owned());
                let warning = if !it.stable {
                    "  UNSTABLE"
                } else if it.same_answers == Some(false) {
                    "  DIFFERENT TEXT"
                } else {
                    ""
                };
                let best_case = it
                    .speedup_repeating
                    .map(|x| format!("  repeating {x:.2}x"))
                    .unwrap_or_default();
                format!(
                    "{:<22} {rate} {speedup}  {acceptance}{best_case}{warning}",
                    it.label
                )
            })
            .collect()
    }
}

/// What to try, for one model on one build.
///
/// **Derived from what the build listed and what the model has**, never from a list of types
/// somebody keeps in step by hand. `offered` is `--spec-type`'s own vocabulary, read from the
/// program; `draft` is a draft file if one exists for this model and `None` if not.
///
/// A `draft-*` type with nothing to draft from is not offered at all — measured 2026-08-31, the
/// **plain** `Qwen3.6-35B-A3B` GGUF carries no MTP tensors, so `draft-mtp` there is a
/// configuration that cannot be run rather than one that is slow.
///
/// **Two ways to have something to draft from**, and the second was missing until the MTP
/// artefact arrived: a separate draft file beside the model, or an MTP head inside it.
/// `carries_mtp` is read from the model's own header (`artifact::Identity`), and ggml-org's own
/// README runs an MTP build with `--spec-type draft-mtp` and no `--spec-draft-model` at all.
pub fn candidates(
    offered: &[String],
    draft: Option<&std::path::Path>,
    carries_mtp: bool,
) -> Vec<Tuning> {
    let usable: Vec<&String> = offered
        .iter()
        .filter(|it| it.as_str() != "none")
        .filter(|it| !crate::models::tuning::needs_a_draft_file(it, carries_mtp) || draft.is_some())
        .collect();

    let Some(first) = usable.first() else {
        return Vec::new();
    };

    // Stage one: the draft length, on the first usable type.
    let mut every: Vec<Tuning> = LENGTHS
        .iter()
        .map(|n| speculating(first, draft, Some(*n)))
        .collect();

    // Stage two: every other type. The length is filled in by `resweep` once stage one has an
    // answer — here they carry llama.cpp's own default, so a caller that runs this list straight
    // through still measures something meaningful rather than nothing.
    every.extend(
        usable
            .iter()
            .skip(1)
            .map(|kind| speculating(kind, draft, None)),
    );
    every
}

/// Every usable kind at one length, first included.
///
/*
    **`at_length` skips the first kind and this does not, and the difference cost a whole run.**

    `at_length` exists for the sweep's second stage, where the first kind has already been
    measured across the whole ladder and re-measuring it would be a duplicate row. The optimizer
    borrowed it to mean *one length per kind* — a different question with the same shape — and
    silently never tried the first kind at all.

    Measured 2026-08-31: the search of the MTP artefact ran ten configurations and `draft-mtp` was
    not among them. `draft-mtp` sorts first in what that model can use, so the one technique the
    artefact exists for was the one thing the search skipped.

    > A helper written for one caller's meaning will be reused by another with a different one,
    > and the shape of the return value will not object.
*/
pub fn every_kind(
    offered: &[String],
    draft: Option<&std::path::Path>,
    carries_mtp: bool,
    n_max: u32,
) -> Vec<Tuning> {
    offered
        .iter()
        .filter(|it| it.as_str() != "none")
        .filter(|it| !crate::models::tuning::needs_a_draft_file(it, carries_mtp) || draft.is_some())
        .map(|kind| speculating(kind, draft, Some(n_max)))
        .collect()
}

/// The second stage's list: every type other than the one already swept, at the winning length.
pub fn at_length(
    offered: &[String],
    draft: Option<&std::path::Path>,
    carries_mtp: bool,
    n_max: u32,
) -> Vec<Tuning> {
    let usable: Vec<&String> = offered
        .iter()
        .filter(|it| it.as_str() != "none")
        .filter(|it| !crate::models::tuning::needs_a_draft_file(it, carries_mtp) || draft.is_some())
        .collect();
    usable
        .iter()
        .skip(1)
        .map(|kind| speculating(kind, draft, Some(n_max)))
        .collect()
}

fn speculating(kind: &str, draft: Option<&std::path::Path>, n_max: Option<u32>) -> Tuning {
    Tuning {
        speculation: Speculation {
            kind: kind.to_owned(),
            draft: draft.map(std::path::Path::to_path_buf),
            n_max,
            ..Speculation::default()
        },
        ..Tuning::default()
    }
}

/// How a configuration reads on a row.
pub fn label(tuning: &Tuning) -> String {
    let kind = tuning.speculation.kind.as_str();
    if kind.is_empty() || kind == "none" {
        return "off".to_owned();
    }
    match tuning.speculation.n_max {
        Some(n) => format!("{kind} n={n}"),
        // llama.cpp's default is 3 and this does not say so, because saying it would be Epoch
        // reporting a number it did not send. The flag was genuinely not passed.
        None => format!("{kind} n=default"),
    }
}

/// Whether the build offers this type at all.
pub fn known(kind: &str) -> bool {
    SPEC_TYPES.contains(&kind)
}

/// Run one sweep.
///
/// `apply` puts a configuration into effect and answers when the server is ready for it — writing
/// the preset, restarting the router, waiting for it to answer. It lives outside this function
/// because it is the only part that differs between runtimes, and because a sweep that owned it
/// could not be tested without a server.
///
/// **The baseline is measured first and never assumed.** A speed-up quoted against a number from
/// a previous sitting is comparing two afternoons.
#[allow(clippy::too_many_arguments)]
pub fn sweep(
    at: &str,
    model: &str,
    ask: &Ask,
    every: &[Tuning],
    apply: &dyn Fn(&Tuning) -> Result<(), String>,
    watching: &dyn Fn(&str, usize, usize),
) -> Sweep {
    let total = every.len() + 1;
    let mut tried = Vec::with_capacity(total);

    let baseline = run_one(at, model, ask, &Tuning::default(), None, apply, &|n, of| {
        watching("off", n, of)
    });
    let reference = baseline.generation();
    // The repeating baseline is its own reference. Comparing a repeating run against a
    // cold-text baseline would report the *prompt cache* as a speed-up from speculation.
    let repeating_reference = baseline.repeated.generation();
    let prints = baseline.measured.fingerprints();
    tried.push(baseline);
    watching("off", 1, total);

    for (n, tuning) in every.iter().enumerate() {
        let name = label(tuning);
        watching(&name, n + 2, total);
        let mut one = run_one(at, model, ask, tuning, reference, apply, &|_, _| {});
        one.same_answers = same(&prints, &one.measured.fingerprints());
        one.speedup_repeating = repeating_reference
            .zip(one.repeated.generation())
            .and_then(|(before, after)| (before > 0.0).then(|| after / before));
        tried.push(one);
    }

    let mut sweep = Sweep {
        model: model.to_owned(),
        tried,
        best: None,
    };
    sweep.best = sweep.pick().map(|it| it.label.clone());
    sweep
}

/// Whether two configurations wrote the same text.
///
/// `None` unless both sides produced a digest for every run, because a comparison across
/// different numbers of runs is not a comparison. **Not `false`**: a configuration whose answers
/// could not be checked is unproven, and calling that *different* would disqualify it for
/// something nobody measured.
fn same(baseline: &[String], other: &[String]) -> Option<bool> {
    if baseline.is_empty() || baseline.len() != other.len() {
        return None;
    }
    Some(baseline == other)
}

#[allow(clippy::too_many_arguments)]
fn run_one(
    at: &str,
    model: &str,
    ask: &Ask,
    tuning: &Tuning,
    reference: Option<f64>,
    apply: &dyn Fn(&Tuning) -> Result<(), String>,
    watching: &dyn Fn(usize, usize),
) -> Tried {
    let label = label(tuning);
    if let Err(why) = apply(tuning) {
        return Tried {
            label,
            tuning: tuning.clone(),
            measured: Measured::default(),
            repeated: Measured::default(),
            load: crate::models::load::Load::default(),
            speedup: None,
            speedup_repeating: None,
            same_answers: None,
            stable: false,
            failures: vec![why],
        };
    }

    // The watch begins before the first request, so the model *loading* is inside the window —
    // which is where a spilling model shows itself.
    let watch = crate::models::load::Watch::begin();
    let measured = crate::bench::measure(at, model, ask, watching);
    /*
        **The best case, measured on purpose rather than by accident.**

        Three different questions is what an honest headline needs — it is what stops one run
        priming the next, and the whole file says so. It also means an ngram speculator, which
        drafts only from text it has already seen, essentially never fires. Reporting that alone
        would say *speculation does nothing here*, and on this machine that is false in a way
        somebody would act on: measured, `ngram-mod` repeating a paragraph it had just written ran
        at 182.5 t/s against a 47.0 baseline.

        So the same configuration is asked one question three times, and the two numbers are kept
        apart and both labelled. Neither is the answer on its own, and an average of them would
        describe neither.
    */
    let again = Ask {
        prompts: ask.prompts.first().cloned().into_iter().collect(),
        ..ask.clone()
    };
    let repeated = crate::bench::measure(at, model, &again, &|_, _| {});
    let load = watch.stop();

    let (finished, asked) = measured.stability();
    let over = |after: Option<f64>| {
        reference
            .zip(after)
            .and_then(|(before, after)| (before > 0.0).then(|| after / before))
    };
    Tried {
        label,
        tuning: tuning.clone(),
        speedup: over(measured.generation()),
        speedup_repeating: None,
        stable: finished == asked && measured.generation().is_some(),
        failures: measured.failures(),
        same_answers: None,
        measured,
        repeated,
        load,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::Run;

    fn ran(rate: f64, prints: &[&str]) -> Measured {
        Measured {
            runs: prints
                .iter()
                .map(|print| Run {
                    generation: Some(rate),
                    fingerprint: Some((*print).to_owned()),
                    ..Run::default()
                })
                .collect(),
        }
    }

    fn tried(label: &str, rate: f64, stable: bool, same_answers: Option<bool>) -> Tried {
        Tried {
            label: label.to_owned(),
            tuning: Tuning::default(),
            measured: ran(rate, &["a", "a", "a"]),
            repeated: ran(rate, &["a", "a", "a"]),
            load: crate::models::load::Load::default(),
            speedup: None,
            speedup_repeating: None,
            same_answers,
            stable,
            failures: Vec::new(),
        }
    }

    #[test]
    fn the_ladder_goes_below_and_above_llama_cpps_own_default() {
        // A sweep that started at the default would be checking a belief rather than measuring
        // an axis, and the whole point is that more drafting is not more speed.
        assert!(LENGTHS.contains(&3), "the default is on the ladder");
        assert!(LENGTHS.iter().any(|it| *it < 3), "and so is shorter");
        assert!(LENGTHS.iter().any(|it| *it > 3), "and longer");
        assert_eq!(LENGTHS.first(), Some(&1), "short first: a loss shows early");
    }

    #[test]
    fn a_type_that_needs_a_draft_file_is_not_offered_without_one() {
        /*
            Measured 2026-08-31: the `Qwen3.6-35B-A3B` GGUF on this machine carries no MTP
            tensors, so `draft-mtp` there is a configuration that cannot run rather than one that
            is slow. A row that always says the same thing is not a measurement.
        */
        let offered: Vec<String> = ["none", "ngram-simple", "draft-mtp", "ngram-cache"]
            .iter()
            .map(|it| (*it).to_owned())
            .collect();

        let without = candidates(&offered, None, false);
        assert!(
            without
                .iter()
                .all(|it| !it.speculation.kind.starts_with("draft-")),
            "{:?}",
            without.iter().map(label).collect::<Vec<_>>()
        );

        let with = candidates(&offered, Some(std::path::Path::new("draft.gguf")), false);
        assert!(
            with.iter().any(|it| it.speculation.kind == "draft-mtp"),
            "and it is offered once there is a file for it",
        );
    }

    #[test]
    fn every_kind_includes_the_first_one_and_at_length_deliberately_does_not() {
        /*
            The defect this pair exists to keep apart. The optimizer asked for *one length per
            kind* and got *every kind except the first* — so the search of an MTP artefact ran ten
            configurations without ever trying `draft-mtp`, which sorts first among the kinds that
            model can use.
        */
        let offered: Vec<String> = ["none", "draft-mtp", "ngram-simple", "ngram-cache"]
            .iter()
            .map(|it| (*it).to_owned())
            .collect();

        let all = every_kind(&offered, None, true, 2);
        assert_eq!(all.len(), 3);
        assert!(
            all.iter().any(|it| it.speculation.kind == "draft-mtp"),
            "the first kind is the point: {:?}",
            all.iter().map(label).collect::<Vec<_>>(),
        );

        let second_stage = at_length(&offered, None, true, 2);
        assert_eq!(second_stage.len(), 2, "the swept one is not re-measured");
        assert!(second_stage
            .iter()
            .all(|it| it.speculation.kind != "draft-mtp"));
    }

    #[test]
    fn the_sweep_is_lengths_plus_types_and_not_lengths_times_types() {
        // Eleven types times six lengths is an evening for an answer nobody waits for.
        let offered: Vec<String> = ["none", "ngram-simple", "ngram-cache", "ngram-mod"]
            .iter()
            .map(|it| (*it).to_owned())
            .collect();
        let every = candidates(&offered, None, false);
        assert_eq!(
            every.len(),
            LENGTHS.len() + 2,
            "five lengths, then two more types"
        );
    }

    #[test]
    fn nothing_beating_the_baseline_is_an_answer_and_not_a_missing_winner() {
        /*
            Off costs no memory and has nothing to go wrong. Promoting a 1% win over it would be
            a coin toss with a decimal point on it — two runs of one configuration on this
            machine differ by about that much on their own.
        */
        let sweep = Sweep {
            model: "m".to_owned(),
            tried: vec![
                tried("off", 35.4, true, None),
                tried("ngram-simple n=1", 35.6, true, Some(true)),
                tried("ngram-simple n=2", 34.0, true, Some(true)),
            ],
            best: None,
        };
        assert!(sweep.pick().is_none(), "0.6% is not a result");
    }

    #[test]
    fn the_winner_is_the_fastest_that_was_stable_and_wrote_the_same_text() {
        let sweep = Sweep {
            model: "m".to_owned(),
            tried: vec![
                tried("off", 35.4, true, None),
                // Fastest of all, and it crashed a run.
                tried("ngram-mod n=8", 90.0, false, Some(true)),
                // Faster still on paper, and it is not the same model any more.
                tried("ngram-cache n=8", 80.0, true, Some(false)),
                tried("ngram-simple n=3", 52.1, true, Some(true)),
                tried("ngram-simple n=2", 44.0, true, Some(true)),
            ],
            best: None,
        };
        let best = sweep.pick().expect("one qualified");
        assert_eq!(best.label, "ngram-simple n=3");
    }

    #[test]
    fn a_configuration_whose_answers_could_not_be_compared_is_unproven_not_wrong() {
        // `None` is *nobody checked*. Disqualifying on it would be reading silence as failure,
        // which is the inversion this codebase keeps paying for.
        let sweep = Sweep {
            model: "m".to_owned(),
            tried: vec![
                tried("off", 35.4, true, None),
                tried("ngram-simple n=3", 52.1, true, None),
            ],
            best: None,
        };
        assert_eq!(
            sweep.pick().map(|it| it.label.as_str()),
            Some("ngram-simple n=3")
        );
    }

    #[test]
    fn a_repeating_run_is_compared_against_a_repeating_baseline() {
        /*
            Measured 2026-08-31: `ngram-mod` repeating a paragraph it had just written ran at
            182.5 t/s against a 47.0 cold baseline. Dividing those two would credit speculation
            with the prompt cache as well as with its own drafting — and the prompt cache is there
            with speculation off. Each column has its own reference.
        */
        let mut baseline = tried("off", 47.0, true, None);
        baseline.repeated = ran(52.0, &["a", "a", "a"]);
        let mut fast = tried("ngram-mod n=3", 41.3, true, Some(true));
        fast.repeated = ran(182.5, &["a", "a", "a"]);

        let against = baseline.repeated.generation().expect("measured");
        let got = fast.repeated.generation().expect("measured") / against;
        assert!(
            (got - 3.51).abs() < 0.01,
            "against the repeating baseline, not the cold one: {got}",
        );
        assert!(
            got < 182.5 / 47.0,
            "and it is smaller than the flattering number",
        );
    }

    #[test]
    fn answers_are_only_compared_across_the_same_number_of_runs() {
        assert_eq!(same(&[], &[]), None, "nothing to compare");
        assert_eq!(
            same(&["a".to_owned()], &["a".to_owned(), "a".to_owned()]),
            None,
            "two runs against one is not a comparison",
        );
        assert_eq!(same(&["a".to_owned()], &["a".to_owned()]), Some(true));
        assert_eq!(same(&["a".to_owned()], &["b".to_owned()]), Some(false));
    }

    #[test]
    fn accepted_per_draft_and_the_acceptance_rate_are_different_questions() {
        /*
            A rate of 0.5 at a draft length of 8 keeps four tokens a pass; the same rate at 2
            keeps one. Reporting only the rate would make those look like the same result.
        */
        let mut one = tried("ngram-simple n=8", 50.0, true, Some(true));
        one.measured = Measured {
            runs: vec![
                Run {
                    drafted: Some(80),
                    accepted: Some(40),
                    ..Run::default()
                },
                Run {
                    drafted: Some(80),
                    accepted: Some(40),
                    ..Run::default()
                },
            ],
        };
        assert_eq!(one.measured.acceptance(), Some(0.5));
        assert_eq!(one.accepted_per_draft(), Some(40.0));
    }

    #[test]
    fn off_is_called_off_and_a_length_nobody_sent_is_not_reported_as_three() {
        // llama.cpp's default is 3, and saying so would be Epoch reporting a number it did not
        // send.
        assert_eq!(label(&Tuning::default()), "off");
        let mut unset = speculating("ngram-simple", None, None);
        assert_eq!(label(&unset), "ngram-simple n=default");
        unset.speculation.n_max = Some(5);
        assert_eq!(label(&unset), "ngram-simple n=5");
    }

    #[test]
    fn every_type_the_build_lists_is_one_this_crate_knows() {
        // The guard against a build that grew a twelfth: `known` is what validates a value before
        // it reaches a preset, and it is the program's own list.
        assert!(known("draft-mtp") && known("ngram-cache"));
        assert!(!known("draft-imaginary"));
    }
}
