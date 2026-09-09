//! Whether a model instance is still the instance that was measured a minute ago.
//!
//! ## The thing this exists for
//!
//! Measured 2026-08-31, one model loaded once and left resident for nine cells:
//!
//! ```text
//! cells 1-4    45.0  45.0  46.1  45.8 tok/s    GPU  26-32%    shared  217-229 MiB
//! cells 5-9    26.2  26.9  27.1  26.3  28.3    GPU  53-57%    shared  280-295 MiB
//! ```
//!
//! Same process, same weights, same prompts. Something started while the card was full, the driver
//! evicted about 70 MiB of the model into shared memory, and it never migrated back. **Throughput
//! halved while the card got busier** — it was moving memory rather than computing.
//!
//! That is not a slow run. It is a **degraded instance**, and the difference matters because a
//! slow run can be repeated and a degraded instance cannot: every measurement taken against it
//! afterwards is wrong in the same direction.
//!
//! ## Never an absolute number
//!
//! Nothing here knows that 26 tok/s is bad. It knows that *this configuration* answered at 45.8,
//! 45.2 and 46.1, and then answered at 27.0 — which is a state change and not the spread. On
//! another card the healthy figure is different and the shape is the same.
//!
//! ## Never one signal
//!
//! A rate can fall because the machine got busy, because a prompt was longer, or because somebody
//! opened a browser. What makes this identifiable is the **combination**: the rate collapses *and*
//! the card reports it is working harder, or *and* the shared pool grew. One signal on its own
//! discards runs that were fine.
//!
//! ## The instrument stays small
//!
//! Shared GPU memory is the clearest signal and the most expensive to read — the better part of a
//! second through `Get-Counter`. So it is read **twice per run**, before and after, never sampled.
//! Everything else comes from readings [`crate::load`] is already taking. The rule this file was
//! written after: an instrument that costs a measurable share of what it measures is not an
//! instrument.

use serde::{Deserialize, Serialize};

/// What a configuration turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// Every run finished, none collapsed, and the spread is small.
    Stable,
    /// It finished, and the instance degraded partway through. The number is real and it is a
    /// number about a broken instance.
    Degraded,
    /// It kept collapsing, or the spread is too wide to call it one configuration.
    Unstable,
    /// It did not produce a usable reading at all.
    Invalid,
}

impl State {
    /// Whether a profile may be built on this.
    pub fn trustworthy(self) -> bool {
        self == State::Stable
    }
}

/// What was seen around one run, beside its rate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Around {
    /// Card utilisation, mean across the run.
    pub gpu_mean: Option<f64>,
    /// Bytes in the GPU's shared pool, read once before and once after. **Not sampled** — it is
    /// the most expensive reading on the machine.
    pub shared_before: Option<u64>,
    pub shared_after: Option<u64>,
    /// Dedicated video memory in use, before and at the peak.
    pub vram_before: Option<u64>,
    pub vram_peak: Option<u64>,
}

impl Around {
    /// How much moved into the shared pool while this ran.
    pub fn shared_grew(&self) -> Option<i64> {
        let before = self.shared_before? as i64;
        let after = self.shared_after? as i64;
        Some(after - before)
    }
}

/// How much of its healthy rate a run has to keep.
///
/// **Below four fifths is a different state, not the spread.** Measured on the run this was
/// written for: healthy was 45.3 to 46.1 across eight rounds — under 2% — and degraded was 27.0,
/// which is 59%. Twenty percent is far outside anything a healthy configuration did here and far
/// inside the collapse.
const KEEPS: f64 = 0.80;

/// How much the shared pool has to grow to count as corroboration, in bytes.
///
/// Measured: the eviction moved about 70 MiB and the healthy rounds moved ±7 MiB. Thirty is above
/// the noise and well below the event.
const SHARED_GREW: i64 = 30 * 1024 * 1024;

/// How much busier the card has to look while producing less.
///
/// Measured: 26–32% healthy against 53–57% degraded. A quarter more is outside the healthy spread
/// and inside the event.
const BUSIER: f64 = 1.25;

/// Whether this run came from an instance that is no longer the one that was healthy.
///
/// `healthy` is what **this same configuration** answered while it was known good. `None` — no
/// healthy history yet — can never be a collapse: the first run of anything is the thing every
/// later run is judged against, and calling it degraded would be judging it against nothing.
pub fn collapsed(
    rate: Option<f64>,
    around: &Around,
    healthy: Option<f64>,
    healthy_gpu: Option<f64>,
) -> bool {
    let (Some(rate), Some(healthy)) = (rate, healthy) else {
        return false;
    };
    if healthy <= 0.0 || rate >= healthy * KEEPS {
        return false;
    }

    // The rate fell. That alone is a slow run — a longer prompt, a busy moment, a browser opening.
    // What makes it a *state change* is the machine saying it did more work for less output.
    let card_busier = around
        .gpu_mean
        .zip(healthy_gpu)
        .is_some_and(|(now, was)| was > 0.0 && now >= was * BUSIER);
    let spilled = around.shared_grew().is_some_and(|it| it >= SHARED_GREW);

    card_busier || spilled
}

/// The middle value.
pub fn median(values: &[f64]) -> Option<f64> {
    let mut got: Vec<f64> = values.iter().copied().filter(|it| it.is_finite()).collect();
    if got.is_empty() {
        return None;
    }
    got.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    /*
        **The conventional median, including for an even count.**

        This was `got[len / 2]` — the *upper* median — which is a different estimator wearing the
        general name. Caught by the owner reading a report, 2026-09-01: two calibration medians of
        51.60 and 51.81 were summarised as `median of medians 51.60`... and the number printed was
        51.81, because upper-median of two is the larger one. The conventional answer is 51.705.
        Neither is wrong as an estimator; only one of them is what `median` means.

        It mattered nowhere in the series that found it — five runs each, three calibrations, both
        odd — and it would have mattered the first time anything had four observations. **A
        general primitive with unexpected semantics is a defect waiting for an even number.**
    */
    let middle = got.len() / 2;
    Some(if got.len().is_multiple_of(2) {
        (got[middle - 1] + got[middle]) / 2.0
    } else {
        got[middle]
    })
}

/// Median absolute deviation — how far a typical reading sits from the middle.
///
/// **Robust on purpose.** A standard deviation computed across a healthy run and a collapsed one
/// is enormous, and the enormity is what would then be accepted as normal; the MAD of the same
/// set barely moves. This is dispersion for deciding *whether something changed*, and a measure
/// that a change can inflate cannot answer that.
pub fn mad(values: &[f64]) -> Option<f64> {
    let middle = median(values)?;
    let spread: Vec<f64> = values.iter().map(|it| (it - middle).abs()).collect();
    median(&spread)
}

/// What counts as *the same machine* as the one the baseline was measured on.
///
/// ## Derived from the baseline's own behaviour, never a constant
///
/// The owner's rule: no hardcoded ±1 tok/s. Measured on this card, eight healthy rounds of one
/// configuration gave 45.3 to 46.1 — a median of 45.8 and a MAD of about 0.25 — and the degraded
/// state gave 27.0. Six MADs is 1.5, which is comfortably outside the healthy spread and nowhere
/// near the collapse.
///
/// ## And a floor, because three runs can agree by luck
///
/// A sentinel is three readings, not eight, and three readings can land on the same number and
/// produce a MAD of zero — which would make *any* later change a failure. The floor is a fraction
/// of the median, so it scales with the machine instead of assuming this one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tolerance {
    pub middle: f64,
    pub mad: f64,
    /// How far a reading may sit from `middle` and still be the same machine.
    pub allowed: f64,
}

/// How many MADs away is still the same machine.
const MADS: f64 = 6.0;

/// The least tolerance, as a share of the median. Three readings can agree exactly.
pub const FLOOR: f64 = 0.03;

impl Tolerance {
    /// From a set of readings believed healthy. `None` where there is nothing to derive from.
    pub fn of(healthy: &[f64]) -> Option<Self> {
        let middle = median(healthy)?;
        if middle <= 0.0 {
            return None;
        }
        let mad = mad(healthy).unwrap_or(0.0);
        Some(Self {
            middle,
            mad,
            allowed: (MADS * mad).max(FLOOR * middle),
        })
    }

    /// From a single remembered rate, where the spread behind it was not kept.
    ///
    /// **The floor, and only the floor.** A real tolerance is six MADs of the readings that
    /// produced the number; those readings are gone, so the honest stand-in is the narrowest
    /// band this file ever allows. Narrow is the safe direction for a gate whose job is to
    /// refuse: it can only ever refuse more than the true band would, never less.
    /// The same **width**, around a different centre.
    ///
    /// ## Why a band travels and a centre does not
    ///
    /// A reference measures two things and only one of them keeps. Its *width* — how much this
    /// experiment varies when nothing has changed — is a property of the model, the card and the
    /// runtime, and three fresh processes measure it far better than one control can. Its
    /// *centre* is what this machine was doing that afternoon.
    ///
    /// Judging today against that centre is what turns a reference into a gate that eventually
    /// never opens. Measured 2026-09-01: `gemma4:12b` reproduced its reference five times inside
    /// twenty minutes and then read 46.5 and 49.1 against a centre of 51.35, so a search refused
    /// twice and the owner still had no profile. The owner named the reason — a desktop is never
    /// in one state, and waiting for the state a reference was minted in is waiting for something
    /// that does not come back.
    ///
    /// What a search actually needs is not a fast machine but **the same machine across the
    /// candidates it is comparing**. So the centre comes from this run and the width comes from
    /// whatever measured one properly.
    pub fn at(self, middle: f64) -> Option<Self> {
        if self.middle <= 0.0 || middle <= 0.0 {
            return None;
        }
        let share = self.allowed / self.middle;
        Some(Self {
            middle,
            // Scaled with it, so the recorded spread still describes the same relative variation.
            mad: self.mad / self.middle * middle,
            allowed: share * middle,
        })
    }

    pub fn around(rate: f64) -> Option<Self> {
        (rate > 0.0).then_some(Self {
            middle: rate,
            mad: 0.0,
            allowed: FLOOR * rate,
        })
    }

    /// Whether a later reading is the same machine answering.
    ///
    /// **One-sided on purpose.** A sentinel that came back *faster* than the baseline is not a
    /// degraded environment — it is the machine having settled further, and refusing it would
    /// abort a search for getting better.
    pub fn reproduces(&self, rate: f64) -> bool {
        rate >= self.middle - self.allowed
    }

    /// How far below the middle a reading fell, as a share of it. Negative where it was faster.
    pub fn short_by(&self, rate: f64) -> f64 {
        (self.middle - rate) / self.middle
    }
}

/// What a set of runs of one configuration amounts to.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verdict {
    pub state_of: Option<State>,
    /// The middle rate across the runs that were kept.
    pub median: Option<f64>,
    pub fastest: Option<f64>,
    pub slowest: Option<f64>,
    /// Fastest minus slowest, across what was kept.
    pub spread: Option<f64>,
    pub kept: usize,
    /// Runs thrown away for any reason.
    pub discarded: usize,
    /// How many of those were collapses rather than failures.
    pub collapses: usize,
}

impl Verdict {
    pub fn state(&self) -> State {
        self.state_of.unwrap_or(State::Invalid)
    }

    /// Whether **every run of this beat every run of that** — the slowest here above the
    /// fastest there.
    ///
    /// ## Why a wide spread is not, on its own, a reason to refuse
    ///
    /// Measured 2026-09-02 on `Qwen3.6-35B-A3B-UD-IQ4_XS`, a model kept in its MTP build
    /// specifically so llama.cpp could draft with the model's own prediction head:
    ///
    /// | configuration | runs | state |
    /// |---|---|---|
    /// | `draft-mtp` | 51.8 – 56.4 | Unstable (8.8% spread) |
    /// | plain | 48.4 – 48.8 | Stable |
    ///
    /// The search discarded the first and recommended 48.6. Every number in that decision was
    /// correct and the decision was wrong: **the worst run of the discarded configuration beat
    /// the best run of the one that replaced it.** Speculative decoding varies because it varies
    /// with how often the draft is accepted, which varies with the text — so its spread is a
    /// property of the technique rather than evidence that the machine was unwell.
    ///
    /// Judging spread on its own asks *is this reading trustworthy?* The question a
    /// recommendation actually turns on is *could choosing this leave me worse off than the
    /// alternative?*, and where one range sits entirely above another, every run observed says
    /// no.
    ///
    /// **No threshold, and none possible.** This is a comparison between two measured ranges, so
    /// there is no band to pick and nothing to tune — which is the whole reason it is
    /// trustworthy where a widened tolerance would not be. A band admitted to fit today's
    /// evidence is a gate adjusted until it passes.
    ///
    /// **A collapse is still disqualifying on either side.** A collapsed set is not a wide
    /// spread; it is two configurations wearing one name, and `slowest` across it describes
    /// neither.
    pub fn dominates(&self, other: &Verdict) -> bool {
        if self.collapses > 0 || other.collapses > 0 {
            return false;
        }
        match (self.slowest, other.fastest) {
            (Some(mine), Some(theirs)) => mine > theirs,
            // Silence on either side is not evidence of dominance. A range that was never
            // recorded cannot be shown to sit above anything.
            _ => false,
        }
    }
}

/// What a set of runs actually did, with every quantity named for what it is.
///
/// ## Four numbers that were being called one word
///
/// A calibration reported *"spread ±5.5%"* and the 5.5% was `Tolerance::allowed` — the **gate
/// band**, six MADs wide. The runs themselves spanned 2.97%. Reading the second as the first led
/// to *"this exceeds the 5% one-configuration threshold"* about a set of runs that
/// [`judge`] correctly calls `Stable`.
///
/// Nothing in the code was wrong; the *report* conflated a description of the data with a
/// decision about it. The fix is that they can no longer share a name.
///
/// | | |
/// |---|---|
/// | `observed_range` | fastest − slowest. What the runs did. |
/// | `max_deviation` | furthest run from the median. |
/// | `mad` | median absolute deviation — the robust width. |
/// | *tolerance* | six MADs, floored. What a **later** reading is allowed to differ by. |
///
/// The first three describe this measurement. The fourth is a rule about the next one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spread {
    pub median: f64,
    pub observed_range_tps: f64,
    pub observed_range_pct: f64,
    pub max_deviation_tps: f64,
    pub max_deviation_pct: f64,
    pub mad_tps: f64,
    pub mad_pct: f64,
    pub runs: usize,
}

impl Spread {
    /// `None` where there is nothing to describe.
    pub fn of(rates: &[f64]) -> Option<Self> {
        let mut got: Vec<f64> = rates.iter().copied().filter(|it| *it > 0.0).collect();
        if got.is_empty() {
            return None;
        }
        got.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let median = median(&got)?;
        if median <= 0.0 {
            return None;
        }
        let range = got[got.len() - 1] - got[0];
        let furthest = got
            .iter()
            .map(|it| (it - median).abs())
            .fold(0.0_f64, f64::max);
        let mad = mad(&got).unwrap_or(0.0);
        Some(Self {
            median,
            observed_range_tps: range,
            observed_range_pct: 100.0 * range / median,
            max_deviation_tps: furthest,
            max_deviation_pct: 100.0 * furthest / median,
            mad_tps: mad,
            mad_pct: 100.0 * mad / median,
            runs: got.len(),
        })
    }

    /// Whether these runs are one configuration, by the same threshold [`judge`] uses.
    ///
    /// **On the observed range, which is the quantity the threshold was written about.**
    pub fn one_configuration(&self) -> bool {
        self.observed_range_pct / 100.0 <= ONE_CONFIGURATION
    }

    /// The three descriptive figures, spelled out so none of them can be read as the gate.
    pub fn describe(&self) -> String {
        format!(
            "observed range {:.2}% · max deviation {:.2}% · MAD {:.2}% over {} runs",
            self.observed_range_pct, self.max_deviation_pct, self.mad_pct, self.runs,
        )
    }
}

/// How wide a spread is still one configuration, as a fraction of the median.
///
/// **Five percent.** The healthy eight rounds spanned 1.7% of their median; a configuration whose
/// runs disagree by more than five is not being measured, it is being sampled from two states.
const ONE_CONFIGURATION: f64 = 0.05;

/// Judge a configuration from its runs.
///
/// `rates` are the runs that were kept. `discarded` and `collapses` are what happened to the rest,
/// and they change the verdict even when every kept run looks perfect: a configuration that had to
/// be retried twice to produce three good runs is **not** the same finding as one that produced
/// three first time.
pub fn judge(rates: &[f64], discarded: usize, collapses: usize) -> Verdict {
    let mut got: Vec<f64> = rates.iter().copied().filter(|it| *it > 0.0).collect();
    got.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    if got.is_empty() {
        return Verdict {
            state_of: Some(State::Invalid),
            discarded,
            collapses,
            ..Verdict::default()
        };
    }

    // The same estimator as `median`, and it has to be: a verdict computed one way and reported
    // another is two spellings of one number.
    let median = median(&got).unwrap_or(got[got.len() / 2]);
    let slowest = got[0];
    let fastest = got[got.len() - 1];
    let spread = fastest - slowest;

    let state = if collapses > 0 {
        // **A collapse is disqualifying even when the surviving runs agree.** The instance that
        // produced them is one reload away from the state that was thrown out, and a profile is
        // something somebody will run for hours.
        State::Unstable
    } else if median > 0.0 && spread / median > ONE_CONFIGURATION {
        State::Unstable
    } else if discarded > 0 {
        // It got there, and it needed help. Worth knowing and not worth refusing.
        State::Degraded
    } else {
        State::Stable
    };

    Verdict {
        state_of: Some(state),
        median: Some(median),
        fastest: Some(fastest),
        slowest: Some(slowest),
        spread: Some(spread),
        kept: got.len(),
        discarded,
        collapses,
    }
}

/// Which of two configurations to prefer.
///
/// **Median first, and a collapse is not paid for by speed.** The owner's rule, 2026-08-31:
///
/// | | peak | median | range | collapses |
/// |---|---|---|---|---|
/// | A | 48.1 | 44.0 | 25–48 | 2 |
/// | B | 45.9 | 45.5 | 45.1–45.9 | 0 |
///
/// B, without argument. A is not a faster configuration, it is two configurations wearing one
/// name — and the fast one is the one that has not collapsed yet.
pub fn better(a: &Verdict, b: &Verdict) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    // Anything that collapsed loses to anything that did not, whatever the numbers say.
    match (a.collapses == 0, b.collapses == 0) {
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    /*
        **A range entirely above another wins, whatever either is called.** The owner's
        correction, 2026-09-02, on a measurement that had just been made: an `Unstable`
        configuration running 51.8–56.4 was ranked below a `Stable` one running 48.4–48.8,
        because the label was consulted before the numbers. See [`Verdict::dominates`].
    */
    if a.dominates(b) {
        return Ordering::Greater;
    }
    if b.dominates(a) {
        return Ordering::Less;
    }
    // Where the ranges overlap the label decides, and a wide spread genuinely is a reason to
    // prefer the reading somebody can rely on.
    match (a.state().trustworthy(), b.state().trustworthy()) {
        (true, false) => return Ordering::Greater,
        (false, true) => return Ordering::Less,
        _ => {}
    }
    a.median
        .unwrap_or(0.0)
        .partial_cmp(&b.median.unwrap_or(0.0))
        .unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A range entirely above another, and the four cases that produced the rule.
    mod dominance {
        use super::*;

        /// Every number here was measured on this machine on 2026-09-02, three models, one
        /// search each.
        fn ran(slowest: f64, median: f64, fastest: f64, state: State) -> Verdict {
            Verdict {
                state_of: Some(state),
                median: Some(median),
                fastest: Some(fastest),
                slowest: Some(slowest),
                spread: Some(fastest - slowest),
                kept: 3,
                discarded: 0,
                collapses: 0,
            }
        }

        #[test]
        fn the_models_own_prediction_head_beats_the_steady_row_it_was_discarded_for() {
            /*
                `Qwen3.6-35B-A3B-UD-IQ4_XS`, kept in its MTP build so llama.cpp could draft with
                the model's own head. The search called `draft-mtp` Unstable at 8.8% and
                recommended the plain row — whose *best* run is slower than the discarded
                configuration's *worst* one.
            */
            let mtp = ran(51.76, 52.44, 56.37, State::Unstable);
            let plain = ran(48.42, 48.62, 48.84, State::Stable);
            assert!(mtp.dominates(&plain));
            assert!(!plain.dominates(&mtp));
            assert_eq!(better(&mtp, &plain), std::cmp::Ordering::Greater);
        }

        #[test]
        fn a_range_that_overlaps_loses_to_the_one_somebody_can_rely_on() {
            /*
                The other three exclusions of the same afternoon, and the rule must keep every
                one of them excluded — each could land below the steady alternative:

                | model | configuration | runs | steady alternative |
                |---|---|---|---|
                | IQ4_XS | `ngram-mod` | 44.9–52.3 | 49.05 |
                | IQ2_M | `ngram-mod` | 82.0–103.0 | 92.06 |
                | gemma4:12b | `ngram-mod` | 48.8–72.2 | 48.96 |
            */
            for (slow, mid, fast, bar) in [
                (44.90, 45.46, 52.30, 49.05),
                (82.00, 83.09, 103.00, 92.06),
                (48.80, 48.85, 72.20, 48.96),
            ] {
                let wild = ran(slow, mid, fast, State::Unstable);
                let steady = ran(bar - 0.2, bar, bar + 0.2, State::Stable);
                assert!(!wild.dominates(&steady), "{mid} must not dominate {bar}");
                assert_eq!(better(&wild, &steady), std::cmp::Ordering::Less);
            }
        }

        #[test]
        fn a_collapse_is_not_a_wide_spread_and_never_dominates() {
            // A collapsed set is two configurations wearing one name; `slowest` across it
            // describes neither, so the comparison must not be attempted.
            let mut broken = ran(51.76, 52.44, 56.37, State::Unstable);
            broken.collapses = 1;
            let plain = ran(48.42, 48.62, 48.84, State::Stable);
            assert!(!broken.dominates(&plain));
            assert_eq!(better(&broken, &plain), std::cmp::Ordering::Less);
        }

        #[test]
        fn a_range_nobody_recorded_dominates_nothing() {
            // Silence is not evidence of dominance, the same way it is not evidence of a refusal.
            let unmeasured = Verdict {
                state_of: Some(State::Unstable),
                median: Some(99.0),
                ..Verdict::default()
            };
            let plain = ran(48.42, 48.62, 48.84, State::Stable);
            assert!(!unmeasured.dominates(&plain));
        }

        #[test]
        fn touching_ranges_do_not_dominate() {
            // Strictly above, not above-or-equal: two ranges that meet at a point have a reading
            // in common, and one of them is not better than the other at that reading.
            let a = ran(48.84, 50.0, 52.0, State::Unstable);
            let b = ran(48.42, 48.62, 48.84, State::Stable);
            assert!(!a.dominates(&b));
        }
    }

    #[test]
    fn the_median_of_an_even_count_is_the_average_of_the_two_middle_values() {
        /*
            **Caught by the owner reading a report, 2026-09-01.** Two calibration medians of 51.60
            and 51.81 were summarised as `median of medians`, and the number printed was 51.81 —
            because this was `got[len / 2]`, the *upper* median, which is a different estimator
            wearing the general name. The conventional answer is 51.705.

            It mattered nowhere in the series that found it: five runs per calibration and three
            calibrations, both odd. It would have mattered the first time anything had four
            observations, silently, in whatever was reading it then.

            **A general primitive with unexpected semantics is a defect waiting for an even
            number.** If a domain ever wants the upper median, it gets its own name.
        */
        assert_eq!(median(&[51.60, 51.81]), Some(51.705));
        assert_eq!(
            median(&[51.81, 51.60]),
            Some(51.705),
            "order does not matter"
        );
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), Some(2.5));

        // Odd is unchanged, which is why the three-calibration series was unaffected.
        assert_eq!(median(&[51.19, 51.81, 51.60]), Some(51.60));
        assert_eq!(median(&[5.0]), Some(5.0));

        // Empty keeps the semantics the rest of this file relies on: nothing measured is `None`,
        // never a zero that would compare as a real reading.
        assert_eq!(median(&[]), None);
        assert_eq!(
            median(&[f64::NAN]),
            None,
            "an unreadable value is not a value"
        );
    }

    #[test]
    fn the_mad_uses_that_same_median() {
        /*
            A verdict computed one way and reported another is two spellings of one number. With
            the pair above: median 51.705, deviations 0.105 and 0.105, so the MAD is 0.105 — where
            the upper-median version gave 0.214, twice as wide, and that figure was feeding a
            tolerance.
        */
        let it = mad(&[51.60, 51.81]).expect("a pair has a MAD");
        assert!((it - 0.105).abs() < 1e-9, "{it}");

        // Four values, both levels even.
        let four = mad(&[1.0, 2.0, 3.0, 4.0]).expect("four");
        assert!((four - 1.0).abs() < 1e-9, "{four}");

        // And the three calibration medians, which is what actually gets used.
        let three = mad(&[51.81, 51.60, 51.19]).expect("three");
        assert!((three - 0.21).abs() < 0.01, "{three}");

        assert_eq!(mad(&[]), None);
    }

    #[test]
    fn judge_reports_the_same_middle_it_decided_with() {
        // `judge` had its own inline `got[len / 2]`, so a four-run configuration would have been
        // judged on one number and had another printed beside it.
        let it = judge(&[51.60, 51.81, 51.19, 51.90], 0, 0);
        assert_eq!(it.median, median(&[51.60, 51.81, 51.19, 51.90]));
    }

    #[test]
    fn the_runs_that_started_all_this_are_described_correctly() {
        /*
            **Calibration A, verbatim.** It was reported as *"spread ±5.5%"* — and 5.5% was
            `Tolerance::allowed`, six MADs wide, which is a rule about the *next* reading. The
            runs themselves span 2.97%, and `judge` correctly calls them Stable.

            Nothing in the code was wrong. The report conflated a description of the data with a
            decision about it, and the fix is that they can no longer share a name.
        */
        let a = Spread::of(&[39.68, 39.32, 40.50]).expect("three runs");
        assert!((a.median - 39.68).abs() < 0.01);
        assert!(
            (a.observed_range_pct - 2.97).abs() < 0.02,
            "{}",
            a.observed_range_pct
        );
        assert!(
            (a.max_deviation_pct - 2.05).abs() < 0.02,
            "{}",
            a.max_deviation_pct
        );
        assert!((a.mad_pct - 0.92).abs() < 0.02, "{}", a.mad_pct);
        assert_eq!(a.runs, 3);
        assert!(a.one_configuration(), "2.97% is inside the 5% threshold");

        // And the gate band is a different number about a different question.
        let tolerance = Tolerance::of(&[39.68, 39.32, 40.50]).expect("a tolerance");
        assert!(
            (100.0 * tolerance.allowed / tolerance.middle - 5.5).abs() < 0.1,
            "{}",
            100.0 * tolerance.allowed / tolerance.middle,
        );
        assert!(
            tolerance.allowed / tolerance.middle > a.observed_range_pct / 100.0,
            "the rule about the next reading is wider than what these runs did, which is the \
             whole reason confusing them was possible",
        );

        // Calibration B, for the record: tighter runs, and a *wider* gate band, because its MAD
        // happened to be larger. Two quantities that do not even move together.
        let b = Spread::of(&[41.44, 41.90, 42.30]).expect("three runs");
        assert!(b.observed_range_pct < a.observed_range_pct);
        assert!(b.mad_pct > a.mad_pct);
    }

    #[test]
    fn a_description_says_nothing_that_could_be_read_as_a_verdict() {
        let it = Spread::of(&[39.68, 39.32, 40.50]).expect("three runs");
        let said = it.describe();
        for word in ["observed range", "max deviation", "MAD"] {
            assert!(said.contains(word), "{said}");
        }
        assert!(
            !said.contains("spread"),
            "the word that meant two things: {said}"
        );
        assert!(!said.contains("tolerance"), "{said}");
    }

    #[test]
    fn a_run_set_that_really_is_two_states_says_so() {
        // Three of Calibration A and three of Calibration B, pooled: 7.5% apart, and not one
        // configuration by the threshold that was written about this quantity.
        let both = Spread::of(&[39.32, 39.68, 40.50, 41.44, 41.90, 42.30]).expect("six runs");
        assert!(!both.one_configuration(), "{}", both.observed_range_pct);
        assert_eq!(Spread::of(&[]), None);
    }

    fn around(gpu: f64, shared_before: u64, shared_after: u64) -> Around {
        Around {
            gpu_mean: Some(gpu),
            shared_before: Some(shared_before),
            shared_after: Some(shared_after),
            vram_before: None,
            vram_peak: None,
        }
    }

    const MIB: u64 = 1024 * 1024;

    #[test]
    fn the_measured_collapse_is_recognised() {
        /*
            The run this file was written from. Healthy: 45.8 tok/s at 29% card utilisation with
            the shared pool at 222 MiB. Then 27.0 at 56%, shared 295.
        */
        assert!(collapsed(
            Some(27.0),
            &around(56.0, 222 * MIB, 295 * MIB),
            Some(45.8),
            Some(29.0),
        ));
    }

    #[test]
    fn the_healthy_spread_is_never_a_collapse() {
        // Eight rounds spanning 45.3 to 46.1 — 1.7% — and nothing else moved.
        for rate in [45.3, 45.7, 46.1] {
            assert!(
                !collapsed(
                    Some(rate),
                    &around(31.0, 223 * MIB, 223 * MIB),
                    Some(45.8),
                    Some(29.0)
                ),
                "{rate} is the spread, not a state change",
            );
        }
    }

    #[test]
    fn a_slow_run_with_nothing_else_wrong_is_a_slow_run() {
        /*
            A longer prompt, a busy moment, a browser opening. The rate alone cannot tell those
            from an eviction, and throwing every one of them away would discard runs that were
            fine — so corroboration is required.
        */
        assert!(!collapsed(
            Some(27.0),
            &around(29.0, 222 * MIB, 222 * MIB),
            Some(45.8),
            Some(29.0),
        ));
    }

    #[test]
    fn either_corroboration_is_enough_on_its_own() {
        // The card working harder for less output, or the shared pool growing. Both were present
        // in the measured event; requiring both would miss a machine that reports only one.
        assert!(collapsed(
            Some(27.0),
            &around(56.0, 222 * MIB, 222 * MIB),
            Some(45.8),
            Some(29.0)
        ));
        assert!(collapsed(
            Some(27.0),
            &around(29.0, 222 * MIB, 295 * MIB),
            Some(45.8),
            Some(29.0)
        ));
    }

    #[test]
    fn nothing_is_judged_against_a_history_it_does_not_have() {
        // The first run of anything is what every later run is judged against. Calling it degraded
        // would be judging it against nothing.
        assert!(!collapsed(
            Some(27.0),
            &around(56.0, 222 * MIB, 400 * MIB),
            None,
            None
        ));
    }

    #[test]
    fn a_collapse_is_disqualifying_even_when_the_surviving_runs_agree() {
        /*
            The instance that produced them is one reload away from the state that was thrown out,
            and a profile is something somebody runs for hours.
        */
        let clean = judge(&[45.5, 45.6, 45.9], 0, 0);
        assert_eq!(clean.state(), State::Stable);

        let recovered = judge(&[45.5, 45.6, 45.9], 2, 2);
        assert_eq!(recovered.state(), State::Unstable);
        assert_eq!(recovered.collapses, 2);
    }

    #[test]
    fn a_configuration_whose_runs_disagree_is_not_one_configuration() {
        // Eight healthy rounds spanned 1.7% of their median. Runs disagreeing by a quarter are
        // being sampled from two states rather than measured.
        assert_eq!(judge(&[25.0, 44.0, 48.0], 0, 0).state(), State::Unstable);
    }

    #[test]
    fn needing_a_retry_is_worth_knowing_and_not_worth_refusing() {
        let helped = judge(&[45.5, 45.6, 45.9], 1, 0);
        assert_eq!(helped.state(), State::Degraded);
        assert_eq!(helped.discarded, 1);
    }

    #[test]
    fn nothing_usable_is_invalid_rather_than_nought() {
        let nothing = judge(&[], 3, 1);
        assert_eq!(nothing.state(), State::Invalid);
        assert_eq!(nothing.median, None, "never a zero");
    }

    #[test]
    fn the_tolerance_comes_from_the_baseline_and_not_from_a_constant() {
        /*
            Measured: eight healthy rounds of one configuration, 45.3 to 46.1. The degraded state
            of that same configuration was 27.0.
        */
        let healthy = [45.3, 45.6, 45.7, 45.9, 45.7, 46.1, 45.9, 46.1];
        let it = Tolerance::of(&healthy).expect("eight readings");
        assert!((it.middle - 45.9).abs() < 0.2, "{}", it.middle);
        assert!(
            it.allowed > 1.0 && it.allowed < 3.0,
            "allowed {}",
            it.allowed
        );

        for rate in healthy {
            assert!(it.reproduces(rate), "{rate} is the healthy spread");
        }
        assert!(!it.reproduces(27.0), "and the collapse is not");
    }

    #[test]
    fn three_readings_that_agree_exactly_do_not_make_everything_a_failure() {
        // A sentinel is three runs, and three runs can land on one number. Without a floor the
        // MAD would be nought and the next reading would fail by a thousandth.
        let it = Tolerance::of(&[45.0, 45.0, 45.0]).expect("three readings");
        assert!(it.mad == 0.0);
        assert!(
            it.allowed >= 45.0 * 0.03,
            "a floor, scaled to the machine: {}",
            it.allowed
        );
        assert!(it.reproduces(44.2), "ordinary variation still reproduces");
        assert!(!it.reproduces(27.0));
    }

    #[test]
    fn a_sentinel_that_came_back_faster_is_not_a_degraded_machine() {
        // It is the machine having settled further. Refusing it would abort a search for getting
        // better, which is the wrong direction to be strict in.
        let it = Tolerance::of(&[45.0, 45.2, 45.1]).expect("three readings");
        assert!(it.reproduces(48.0));
        assert!(it.short_by(48.0) < 0.0);
    }

    #[test]
    fn dispersion_is_robust_so_a_collapse_cannot_widen_what_counts_as_normal() {
        /*
            A standard deviation across one healthy set and one collapsed reading is enormous, and
            the enormity would then be accepted as normal. The MAD barely moves.
        */
        let clean = [45.3, 45.9, 46.1];
        let with_a_collapse = [45.3, 45.9, 46.1, 27.0];
        let a = mad(&clean).expect("three");
        let b = mad(&with_a_collapse).expect("four");
        assert!(b < a * 4.0, "MAD {a} -> {b} rather than exploding");
    }

    #[test]
    fn the_owners_example_picks_the_slower_one() {
        /*
            | | peak | median | range | collapses |
            | A | 48.1 | 44.0 | 25-48 | 2 |
            | B | 45.9 | 45.5 | 45.1-45.9 | 0 |
        */
        let a = judge(&[25.0, 44.0, 48.1], 2, 2);
        let b = judge(&[45.1, 45.5, 45.9], 0, 0);
        assert_eq!(better(&a, &b), std::cmp::Ordering::Less, "B wins");
        assert!(a.fastest > b.fastest, "even though A was faster once");
    }

    #[test]
    fn between_two_clean_configurations_the_median_decides() {
        let slower = judge(&[42.0, 42.5, 43.0], 0, 0);
        let faster = judge(&[45.1, 45.5, 45.9], 0, 0);
        assert_eq!(better(&faster, &slower), std::cmp::Ordering::Greater);
    }
}
