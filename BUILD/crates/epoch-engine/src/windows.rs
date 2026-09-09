//! How a model's capability changes as the window grows.
//!
//! ## Three benchmarks, three questions, and they never share a number
//!
//! | | asks |
//! |---|---|
//! | Standard performance | how fast and how stable, at one window every model gets |
//! | Quality (`suite2`) | how capable the model is |
//! | **This** | how that capability changes when the window grows |
//!
//! They cross afterwards and they never collapse. A model that is quick at 32K and forgets the
//! middle of its own context at 128K is two facts, and one number would hide whichever half the
//! reader needed.
//!
//! ## Four different things people call "context"
//!
//! Saying a model supports 262,144 tokens answers almost nothing. What is worth knowing:
//!
//! | | |
//! |---|---|
//! | **native** | what the model's own header declares |
//! | **loadable** | the largest that actually loaded and ran here |
//! | **effective** | the largest where it still *uses* what is in the window |
//! | **recommended** | the best measured balance of that against speed and memory |
//!
//! A plausible answer looks like `native 262K · loadable 262K · effective 128K · recommended 64K`,
//! and every one of those four is a different decision for the person reading it.
//!
//! ## Effective is measured by use, not by loading
//!
//! Loading without an out-of-memory error proves the allocator worked. What this asks instead is
//! whether a fact placed 100,000 tokens ago is still reachable, whether two distant facts can be
//! combined, whether an instruction given early still governs the answer, and whether a later
//! correction beats an earlier statement.
//!
//! **Not only needle-in-a-haystack.** Finding one sentence is the easiest thing a long window is
//! asked to do, and a model can be perfect at it while being unable to use anything it found. So
//! retrieval and reasoning are scored **separately** and reported separately.
//!
//! ## Versioned, like the others
//!
//! The prompts, where the facts sit, the validators, the budgets and how the filler is composed
//! are all part of the measurement. Changing any of them is a new version, because a score from
//! one composition is not comparable with a score from another.

use serde::{Deserialize, Serialize};

use crate::trials::Trial;

/// Which composition a context result was measured against.
pub const VERSION: u32 = 1;

/// How many tokens a context answer may take.
///
/// Short on purpose: every question here has a short answer, and the expensive part of the trial
/// is the prompt rather than the reply. A model that needs more than this to say a name is not
/// being measured on its window.
pub const ANSWER_TOKENS: u32 = 1_024;

/// Roughly how many characters make a token.
///
/// **An estimate, and it is used as one.** Four is the usual figure for English prose and every
/// tokeniser disagrees with it a little. What matters is that the composed prompt lands *near* the
/// window being tested; the real length is read back from the server's own `prompt_n`, which is
/// the number recorded. Composing to a character count and reporting it as a token count would be
/// the invented gauge.
pub const CHARS_PER_TOKEN: usize = 4;

/// Which of the two things a long window is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Dimension {
    /// Can it find what is in there.
    Retrieval,
    /// Can it *use* what is in there — combine, resolve, obey.
    Reasoning,
}

/// Where in the window a fact is placed.
///
/// **The middle is the interesting one.** A model that reads the start and the end of its context
/// and skims what is between them scores perfectly on a needle placed at either edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Place {
    Start,
    Quarter,
    Middle,
    ThreeQuarters,
    End,
}

impl Place {
    /// How far through the filler it sits, as a fraction.
    fn at(self) -> f64 {
        match self {
            Place::Start => 0.02,
            Place::Quarter => 0.25,
            Place::Middle => 0.5,
            Place::ThreeQuarters => 0.75,
            Place::End => 0.97,
        }
    }
}

/// One thing to plant in the window, and what it is for.
struct Fact {
    place: Place,
    text: &'static str,
}

/// One question asked of a composed window.
struct Probe {
    id: &'static str,
    dimension: Dimension,
    facts: &'static [Fact],
    question: &'static str,
    answers: &'static [&'static str],
}

/// The filler the facts are planted in.
///
/*
    **Deterministic, and deliberately dull.** It has to be long, it has to be the same every time,
    and it must not accidentally contain anything that looks like an answer — a haystack with a
    second needle in it measures nothing.

    Written as a rotating set of neutral sentences about logistics, with a running counter so no
    two paragraphs are identical. Identical paragraphs would let a model skip the body entirely and
    still know its shape, which is a different skill from reading it.

    **And it contains no digits at all**, which is not tidiness. The first version numbered
    its lines with a running counter, the counter reached 322, and 322 is the answer to the
    trial that multiplies 14 by 23. A haystack with a second needle in it measures nothing,
    and the test that checks every answer against the filler is what caught it. With no
    digits anywhere, a numeric answer cannot collide by arithmetic accident.
*/
fn filler(chars: usize) -> String {
    const LINES: [&str; 6] = [
        "Consignment {a} left the {b} depot and was logged by the duty clerk without any incident.",
        "The loading bay recorded pallets from {a} moving through the {b} gate on the late shift.",
        "A routine inspection of aisle {a} found the {b} shelving in tolerance, nothing to report.",
        "Vehicle {a} completed its {b} circuit and returned with the manifest countersigned again.",
        "Stock count {a} matched the {b} ledger and required no adjustment from the supervising team.",
        "The night crew filed the {a} report, noting {b} conditions and no exceptions of any kind.",
    ];
    // Different lengths, and neither divides the other, so the combination takes a long time to
    // repeat. Nothing here is a number, and nothing here is a word a probe uses.
    const A: [&str; 11] = [
        "harbour", "meadow", "willow", "quarry", "thistle", "beacon", "ferry", "orchard", "cobble",
        "lantern", "bramble",
    ];
    const B: [&str; 7] = [
        "outer", "lower", "second", "shaded", "quiet", "far", "inner",
    ];
    let mut out = String::with_capacity(chars + 128);
    let mut n = 0usize;
    while out.len() < chars {
        out.push_str(
            &LINES[n % LINES.len()]
                .replace("{a}", A[n % A.len()])
                .replace("{b}", B[n % B.len()]),
        );
        out.push('\n');
        n += 1;
    }
    out.truncate(chars);
    out
}

/// Plant the facts in filler of the right size.
///
/// Positions are by character, which is why they are approximate — see [`CHARS_PER_TOKEN`]. What
/// matters is that a fact placed in the middle is genuinely in the middle, and it is.
fn compose(facts: &[Fact], tokens: u32) -> String {
    let chars = tokens as usize * CHARS_PER_TOKEN;
    let mut body = filler(chars);

    // Furthest first, so inserting one does not move the ones that come after it.
    let mut ordered: Vec<&Fact> = facts.iter().collect();
    ordered.sort_by(|a, b| b.place.at().partial_cmp(&a.place.at()).unwrap());
    for fact in ordered {
        let at = ((body.len() as f64) * fact.place.at()) as usize;
        // Land on a line boundary, so a fact is never spliced into the middle of a sentence.
        let at = body[..at.min(body.len())]
            .rfind('\n')
            .map(|it| it + 1)
            .unwrap_or(0);
        body.insert_str(at, &format!("{}\n", fact.text));
    }
    body
}

/// Every probe, in order.
fn probes() -> &'static [Probe] {
    &[
        // ---- retrieval: one fact, at each of the three positions that matter ----
        Probe {
            id: "x-start",
            dimension: Dimension::Retrieval,
            facts: &[Fact {
                place: Place::Start,
                text: "NOTE: the override key for the eastern depot is HARROW-6182.",
            }],
            question: "What is the override key for the eastern depot? Answer with just the key.",
            answers: &["HARROW-6182", "harrow 6182"],
        },
        Probe {
            id: "x-middle",
            dimension: Dimension::Retrieval,
            facts: &[Fact {
                place: Place::Middle,
                text: "NOTE: the override key for the eastern depot is TALLOW-4457.",
            }],
            question: "What is the override key for the eastern depot? Answer with just the key.",
            answers: &["TALLOW-4457", "tallow 4457"],
        },
        Probe {
            id: "x-end",
            dimension: Dimension::Retrieval,
            facts: &[Fact {
                place: Place::End,
                text: "NOTE: the override key for the eastern depot is MERLIN-9034.",
            }],
            question: "What is the override key for the eastern depot? Answer with just the key.",
            answers: &["MERLIN-9034", "merlin 9034"],
        },
        // ---- retrieval with distractors: three keys, and only one is the eastern depot ----
        Probe {
            id: "x-distractors",
            dimension: Dimension::Retrieval,
            facts: &[
                Fact {
                    place: Place::Quarter,
                    text: "NOTE: the override key for the northern depot is BRAMBLE-1120.",
                },
                Fact {
                    place: Place::Middle,
                    text: "NOTE: the override key for the eastern depot is CINDER-7741.",
                },
                Fact {
                    place: Place::ThreeQuarters,
                    text: "NOTE: the override key for the southern depot is PELICAN-3308.",
                },
            ],
            question: "What is the override key for the eastern depot? Answer with just the key.",
            answers: &["CINDER-7741", "cinder 7741"],
        },
        // ---- reasoning: two distant facts that must be combined ----
        Probe {
            id: "x-combine",
            dimension: Dimension::Reasoning,
            facts: &[
                Fact {
                    place: Place::Start,
                    text: "NOTE: each sealed crate weighs 14 kilograms.",
                },
                Fact {
                    place: Place::End,
                    text: "NOTE: vehicle 77 is carrying 23 sealed crates and nothing else.",
                },
            ],
            question: "How many kilograms is vehicle 77 carrying? Answer with just the number.",
            answers: &["322"],
        },
        // ---- reasoning: a later correction beats an earlier statement ----
        Probe {
            id: "x-contradiction",
            dimension: Dimension::Reasoning,
            facts: &[
                Fact {
                    place: Place::Quarter,
                    text: "NOTE: the cut-off time for the eastern depot is 16:00.",
                },
                Fact {
                    place: Place::ThreeQuarters,
                    text: "CORRECTION: the earlier cut-off time for the eastern depot was wrong. \
                           From now on it is 18:30.",
                },
            ],
            question: "What is the cut-off time for the eastern depot? Answer with just the time.",
            answers: &["18:30", "1830", "6:30 pm", "18 30"],
        },
        // ---- reasoning: an instruction given early must still govern the answer ----
        Probe {
            id: "x-standing-order",
            dimension: Dimension::Reasoning,
            facts: &[
                Fact {
                    place: Place::Start,
                    text:
                        "STANDING ORDER: whenever you are asked for a depot's status, reply with \
                           the single word AMBER and nothing else, whatever the status actually is.",
                },
                Fact {
                    place: Place::ThreeQuarters,
                    text: "NOTE: the eastern depot status is currently GREEN.",
                },
            ],
            question: "What is the status of the eastern depot?",
            answers: &["AMBER"],
        },
        // ---- reasoning: a cross-reference, where the answer names a second fact ----
        Probe {
            id: "x-cross-reference",
            dimension: Dimension::Reasoning,
            facts: &[
                Fact {
                    place: Place::Quarter,
                    text: "NOTE: route B is handled by the driver named Okonkwo.",
                },
                Fact {
                    place: Place::Middle,
                    text: "NOTE: the consignment marked URGENT is on route B.",
                },
                Fact {
                    place: Place::ThreeQuarters,
                    text: "NOTE: route C is handled by the driver named Varga.",
                },
            ],
            question: "Who is handling the consignment marked URGENT? Answer with just the name.",
            answers: &["Okonkwo"],
        },
    ]
}

/// Every trial, composed for one window size.
///
/// The same questions at every size, so a difference between two rungs is about the window and
/// nothing else.
pub fn at(tokens: u32) -> Vec<Trial> {
    probes()
        .iter()
        .map(|probe| {
            let body = compose(probe.facts, tokens.saturating_sub(ANSWER_TOKENS + 256));
            Trial::Reasoning {
                id: probe.id.to_owned(),
                prompt: format!(
                    "Read the log below and then answer the question at the end.\n\n\
                     ----- LOG BEGINS -----\n{body}\n----- LOG ENDS -----\n\n{}",
                    probe.question
                ),
                answers: probe.answers.iter().map(|it| (*it).to_owned()).collect(),
            }
        })
        .collect()
}

/// Which dimension a probe belongs to.
pub fn dimension_of(id: &str) -> Option<Dimension> {
    probes()
        .iter()
        .find(|it| it.id == id)
        .map(|it| it.dimension)
}

/// The window sizes worth trying, in order.
///
/// **Doubling, and it stops at three things**: what the model declares, what the runtime allows,
/// and what the hardware can hold. Refining between two rungs is a second pass and only where the
/// gap matters — the owner's rule, and it is what keeps this from being a hundred loads.
pub fn ladder(native: u32) -> Vec<u32> {
    let mut every = Vec::new();
    let mut at = 8_192u32;
    while at <= native {
        every.push(at);
        let Some(next) = at.checked_mul(2) else { break };
        at = next;
    }
    // The declared maximum itself where it is not a power of two — a model trained for 262,144
    // stops at 131,072 otherwise, and the last half is the interesting part.
    if every.last().is_none_or(|last| *last < native) && native > 0 {
        every.push(native);
    }
    every
}

/// A midpoint between two rungs, for refining once the ladder has found the edge.
///
/// `None` where the two are already adjacent enough that a third measurement would be noise —
/// under a quarter apart, which on this ladder means they are not two rungs at all.
pub fn between(low: u32, high: u32) -> Option<u32> {
    if high <= low || (high - low) * 4 < high {
        return None;
    }
    Some(low + (high - low) / 2)
}

/// What one window size did.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rung {
    pub context: u32,
    /// What the server said the prompt really was, in tokens. **The number recorded**, because the
    /// composition is measured in characters and the two never agree exactly.
    pub prompt_tokens: Option<u64>,
    pub generation: Option<f64>,
    pub prompt: Option<f64>,
    pub first_token_ms: Option<f64>,
    /// How long the prompt took to process, which is what a long window actually costs.
    pub prefill_seconds: Option<f64>,
    pub vram_used: Option<u64>,
    pub ram_used: Option<u64>,
    pub shared_used: Option<u64>,
    /// The KV cache this rung ran with.
    pub cache: Option<String>,
    pub kv_bytes: Option<u64>,
    pub stable: bool,
    /// What share of the retrieval probes it got right. `None` where none ran.
    pub retrieval: Option<f64>,
    /// And of the long-context reasoning ones. **Kept apart**: finding a sentence and using it are
    /// different skills, and a model can be perfect at the first while failing the second.
    pub reasoning: Option<f64>,
    /// Why it stopped, where it did.
    pub note: Option<String>,
}

impl Rung {
    /// Whether this rung is still doing the job. Both dimensions, and neither excuses the other.
    pub fn holds_up(&self) -> bool {
        self.stable
            && self.retrieval.is_none_or(|it| it >= KEEPS_UP)
            && self.reasoning.is_none_or(|it| it >= KEEPS_UP)
    }
}

/// How much of its capability a window has to keep to count as effective.
///
/// **Nine tenths, and it applies to both dimensions separately.** A model answering 89% of the
/// retrieval probes at 256K is not usable at 256K for anything that depends on what is in there,
/// however cleanly it loaded.
const KEEPS_UP: f64 = 0.90;

/// The four answers, and they are four different decisions.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capability {
    pub artifact: String,
    pub gpu: String,
    pub build: String,
    pub runtime: String,
    /// Which composition this was measured against.
    pub suite: u32,
    /// What the model's own header declares.
    pub native: Option<u32>,
    pub rungs: Vec<Rung>,
    pub at: u64,
}

impl Capability {
    /// The largest window that loaded and ran at all.
    ///
    /// **Loading is not using**, which is why this is reported beside `effective` rather than as
    /// the headline. A model that loads at 262K and cannot find anything in it supports 262K in
    /// the sense that matters to an allocator.
    pub fn loadable(&self) -> Option<u32> {
        self.rungs
            .iter()
            .filter(|it| it.stable)
            .map(|it| it.context)
            .max()
    }

    /// The largest window where it still uses what is in there.
    pub fn effective(&self) -> Option<u32> {
        self.rungs
            .iter()
            .filter(|it| it.holds_up())
            .map(|it| it.context)
            .max()
    }

    /// The best measured balance.
    ///
    /*
        **A rule, not a weighting.** Among the windows that hold up, the largest one that still
        runs at four fifths of the fastest rung's speed. Two things are being traded and both are
        measured, so the trade is stated rather than tuned: somebody who wants the other end of it
        has `effective` and `loadable` right beside this.
    */
    pub fn recommended(&self) -> Option<u32> {
        let fastest = self
            .rungs
            .iter()
            .filter(|it| it.holds_up())
            .filter_map(|it| it.generation)
            .fold(f64::MIN, f64::max);
        if fastest <= 0.0 {
            return self.effective();
        }
        self.rungs
            .iter()
            .filter(|it| it.holds_up())
            .filter(|it| {
                it.generation
                    .is_some_and(|rate| rate >= fastest * KEEPS_SPEED)
            })
            .map(|it| it.context)
            .max()
    }

    /// One line per rung, for a panel that has not been drawn yet.
    pub fn table(&self) -> Vec<String> {
        self.rungs
            .iter()
            .map(|it| {
                let rate = it
                    .generation
                    .map(|rate| format!("{rate:>5.1} t/s"))
                    .unwrap_or_else(|| "     —   ".to_owned());
                let share = |of: Option<f64>| {
                    of.map(|it| format!("{:.0}%", it * 100.0))
                        .unwrap_or_else(|| "—".to_owned())
                };
                format!(
                    "{:>5}K  {rate}  retrieval {:>4}  reasoning {:>4}{}",
                    it.context / 1024,
                    share(it.retrieval),
                    share(it.reasoning),
                    if it.stable { "" } else { "  UNSTABLE" },
                )
            })
            .collect()
    }
}

/// How much of the fastest rung's speed a recommended window must keep.
const KEEPS_SPEED: f64 = 0.80;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ladder_doubles_and_stops_where_the_model_does() {
        assert_eq!(ladder(32_768), vec![8_192, 16_384, 32_768]);
        let long = ladder(262_144);
        assert_eq!(long.first(), Some(&8_192));
        assert_eq!(long.last(), Some(&262_144));
        assert!(
            long.len() <= 7,
            "a handful of loads, not a hundred: {long:?}"
        );
        // A model trained below the first rung still gets one.
        assert_eq!(ladder(4_096), vec![4_096]);
    }

    #[test]
    fn refining_between_two_rungs_stops_where_it_would_be_noise() {
        assert_eq!(between(65_536, 131_072), Some(98_304));
        // Already close: a third measurement between them measures the spread.
        assert_eq!(between(120_000, 131_072), None);
        assert_eq!(between(131_072, 65_536), None);
    }

    #[test]
    fn a_fact_placed_in_the_middle_really_is_in_the_middle() {
        /*
            **The middle is the whole point.** A model that reads the start and the end of its
            context and skims between them scores perfectly on a needle at either edge, and a
            composer that quietly put everything near the front would never catch it.
        */
        let facts = [Fact {
            place: Place::Middle,
            text: "NOTE: the marker is HERE.",
        }];
        let body = compose(&facts, 8_000);
        let at = body.find("HERE").expect("planted");
        let share = at as f64 / body.len() as f64;
        assert!(
            (0.45..0.55).contains(&share),
            "planted at {share:.2} of the way through",
        );
    }

    #[test]
    fn several_facts_land_where_they_were_asked_to() {
        // Inserting one must not move the others — hence furthest-first.
        let facts = [
            Fact {
                place: Place::Start,
                text: "NOTE: first is ALPHA.",
            },
            Fact {
                place: Place::Middle,
                text: "NOTE: second is BETA.",
            },
            Fact {
                place: Place::End,
                text: "NOTE: third is GAMMA.",
            },
        ];
        let body = compose(&facts, 12_000);
        let a = body.find("ALPHA").expect("first");
        let b = body.find("BETA").expect("second");
        let c = body.find("GAMMA").expect("third");
        assert!(a < b && b < c, "{a} {b} {c}");
        assert!((b as f64 / body.len() as f64) > 0.4);
        assert!((c as f64 / body.len() as f64) > 0.9);
    }

    #[test]
    fn the_filler_never_contains_an_answer() {
        /*
            A haystack with a second needle in it measures nothing. Every answer string is checked
            against filler long enough to be the largest window on the ladder.
        */
        let hay = filler(400_000).to_lowercase();
        for probe in probes() {
            for answer in probe.answers {
                assert!(
                    !hay.contains(&answer.to_lowercase()),
                    "{} appears in the filler",
                    answer,
                );
            }
        }
    }

    #[test]
    fn the_filler_is_the_same_every_time_and_never_two_identical_paragraphs() {
        // Deterministic, so two rungs differ by their window and nothing else. And varied, because
        // identical paragraphs would let a model skip the body and still know its shape.
        assert_eq!(filler(5_000), filler(5_000));
        let body = filler(5_000);
        let lines: Vec<&str> = body.lines().take(20).collect();
        let unique: std::collections::BTreeSet<&&str> = lines.iter().collect();
        assert_eq!(unique.len(), lines.len(), "a repeated line");
    }

    #[test]
    fn a_composed_prompt_lands_near_the_window_it_was_asked_for() {
        // Near, not exactly — the composition counts characters and the server counts tokens.
        // Which is why `prompt_tokens` is what gets recorded.
        for want in [8_192u32, 32_768, 131_072] {
            let trials = at(want);
            let Trial::Reasoning { prompt, .. } = &trials[0] else {
                panic!("a context trial is a reasoning trial");
            };
            let guessed = prompt.len() / CHARS_PER_TOKEN;
            let ratio = guessed as f64 / want as f64;
            assert!((0.7..1.1).contains(&ratio), "{want}: landed at {ratio:.2}");
        }
    }

    #[test]
    fn retrieval_and_reasoning_are_never_folded_together() {
        /*
            Finding a sentence and using it are different skills, and a model can be perfect at the
            first while failing the second. One number would hide whichever half the reader needed.
        */
        let of_kind = |want: Dimension| probes().iter().filter(|it| it.dimension == want).count();
        assert!(of_kind(Dimension::Retrieval) >= 3);
        assert!(of_kind(Dimension::Reasoning) >= 4);
        assert_eq!(dimension_of("x-middle"), Some(Dimension::Retrieval));
        assert_eq!(dimension_of("x-combine"), Some(Dimension::Reasoning));
        assert_eq!(dimension_of("nothing"), None);
    }

    fn rung(context: u32, rate: f64, retrieval: f64, reasoning: f64) -> Rung {
        Rung {
            context,
            generation: Some(rate),
            retrieval: Some(retrieval),
            reasoning: Some(reasoning),
            stable: true,
            ..Rung::default()
        }
    }

    #[test]
    fn the_four_answers_are_four_different_numbers() {
        /*
            The owner's example: native 262K, loadable 262K, effective 128K, recommended 64K.
            Reporting only the first is what makes a specification sheet useless.
        */
        let it = Capability {
            native: Some(262_144),
            rungs: vec![
                rung(8_192, 48.0, 1.0, 1.0),
                rung(16_384, 47.0, 1.0, 1.0),
                rung(32_768, 46.0, 1.0, 0.98),
                rung(65_536, 44.0, 1.0, 0.95),
                rung(131_072, 40.0, 0.96, 0.92),
                rung(262_144, 31.0, 0.78, 0.67),
            ],
            ..Capability::default()
        };
        assert_eq!(it.native, Some(262_144));
        assert_eq!(it.loadable(), Some(262_144));
        assert_eq!(
            it.effective(),
            Some(131_072),
            "256K stops using what is in it"
        );
        assert_eq!(
            it.recommended(),
            Some(131_072),
            "and 40 of 48 is within the trade"
        );
    }

    #[test]
    fn a_window_that_loads_and_forgets_is_never_effective() {
        // Loading without an out-of-memory error proves the allocator worked.
        let it = Capability {
            rungs: vec![
                rung(32_768, 46.0, 1.0, 1.0),
                rung(262_144, 30.0, 0.55, 0.40),
            ],
            ..Capability::default()
        };
        assert_eq!(it.loadable(), Some(262_144));
        assert_eq!(it.effective(), Some(32_768));
    }

    #[test]
    fn either_dimension_failing_is_enough_to_disqualify_a_window() {
        // Perfect retrieval and broken reasoning is a model that can find a sentence and not use
        // it, which is exactly the case one number would hide.
        let finds_but_cannot_use = rung(131_072, 40.0, 1.0, 0.60);
        assert!(!finds_but_cannot_use.holds_up());
        let uses_but_cannot_find = rung(131_072, 40.0, 0.60, 1.0);
        assert!(!uses_but_cannot_find.holds_up());
    }

    #[test]
    fn an_unstable_rung_is_never_recommended_however_well_it_scored() {
        let mut broken = rung(131_072, 60.0, 1.0, 1.0);
        broken.stable = false;
        let it = Capability {
            rungs: vec![rung(32_768, 46.0, 1.0, 1.0), broken],
            ..Capability::default()
        };
        assert_eq!(it.loadable(), Some(32_768));
        assert_eq!(it.recommended(), Some(32_768));
    }

    #[test]
    fn recommended_gives_up_a_window_only_for_a_real_slowdown() {
        // 40 of 48 is 83% and stays; 31 of 48 is 65% and does not.
        let it = Capability {
            rungs: vec![
                rung(32_768, 48.0, 1.0, 1.0),
                rung(65_536, 40.0, 1.0, 1.0),
                rung(131_072, 31.0, 1.0, 1.0),
            ],
            ..Capability::default()
        };
        assert_eq!(it.recommended(), Some(65_536));
        assert_eq!(
            it.effective(),
            Some(131_072),
            "and the bigger one is still effective"
        );
    }

    #[test]
    fn nothing_measured_is_nothing_rather_than_nought() {
        let empty = Capability::default();
        assert_eq!(empty.loadable(), None);
        assert_eq!(empty.effective(), None);
        assert_eq!(empty.recommended(), None);
        assert!(empty.table().is_empty());
    }
}
