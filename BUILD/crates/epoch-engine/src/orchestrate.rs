//! What Epoch decides for itself when somebody presses BENCHMARK & OPTIMIZE.
//!
//! ## The button is the whole interface
//!
//! A user should not have to know what a sentinel is, which reference governs, whether this
//! machine has been calibrated, or that `O-004` exists. They press one thing and Epoch works out
//! what it needs to measure. Everything in this file is that decision, made explicit so it can be
//! tested without a graphics card.
//!
//! ## The rule this file was written after
//!
//! Six controls of one experiment, 2026-09-01, on one machine in three hours:
//!
//! ```text
//! 51.81 → 51.60 → 51.19 → 48.04 → 42.75 → 38.58
//! ```
//!
//! Same binary, same artefact, same fingerprint, fresh process each time. −25.5%, monotonic, and
//! ten minutes of idle did not interrupt it.
//!
//! A search run across that slope would have produced twenty candidate rows where the last is a
//! quarter slower than the first for reasons that have nothing to do with any of them — **and
//! every one of those rows would have looked like a finding.** So:
//!
//! > **A search that loses reproducibility stops and classifies the state. It does not keep
//! > measuring candidates and attribute the loss to them.**
//!
//! Which is more valuable than knowing what caused it.

use serde::{Deserialize, Serialize};

use crate::reference::PerformanceReference;

/// Where a benchmark is, as one value.
///
/// **The states are what a person could be told**, not implementation phases: each answers *what
/// is happening and what would come next*. Transitions are enumerated in [`Phase::may_become`]
/// because a benchmark that could move from `Complete` back to `Calibrating` would be a benchmark
/// nobody could reason about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Nothing has ever been measured for this model here.
    #[default]
    Unbenchmarked,
    /// Nothing comparable exists to judge a control against. **Not a claim about the machine** —
    /// a claim about the record, which is why it is not `RecoveryRequired`.
    ReferenceRequired,
    /// Taking the calibrations a reference is built from.
    Calibrating,
    /// A reference governs and the last control reproduced it. Candidates may run.
    Ready,
    /// A search is running.
    Benchmarking,
    /// The control no longer reproduces. Everything measured from here is about a different
    /// machine.
    Degraded,
    /// Recovery was attempted and the control still does not reproduce. **Epoch says what it
    /// needs and does not act** — no reboot, no driver reset, nothing that could take down a
    /// remote session.
    RecoveryRequired,
    /// Every candidate ran and the closing control reproduced.
    Complete,
    /// It never established a control at all.
    Invalid,
    /// Somebody stopped it. Whatever was validly measured is kept; none of it becomes a profile.
    Cancelled,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Unbenchmarked => "UNBENCHMARKED",
            Phase::ReferenceRequired => "REFERENCE REQUIRED",
            Phase::Calibrating => "CALIBRATING",
            Phase::Ready => "READY",
            Phase::Benchmarking => "BENCHMARKING",
            Phase::Degraded => "DEGRADED",
            Phase::RecoveryRequired => "RECOVERY REQUIRED",
            Phase::Complete => "COMPLETE",
            Phase::Invalid => "INVALID",
            Phase::Cancelled => "CANCELLED",
        }
    }

    /// Whether measurements taken in this phase may become profiles.
    pub fn results_are_usable(self) -> bool {
        matches!(self, Phase::Benchmarking | Phase::Complete)
    }

    /// Whether a transition is one this machine allows.
    ///
    /// **Enumerated rather than derived.** A rule like *anything may become Degraded* reads well
    /// and would let a `Complete` benchmark silently reopen; the list is longer and it is the
    /// thing somebody can check against the diagram they were shown.
    pub fn may_become(self, next: Phase) -> bool {
        use Phase::*;
        if self == next {
            return true;
        }
        match self {
            Unbenchmarked => matches!(next, ReferenceRequired | Calibrating | Invalid | Cancelled),
            ReferenceRequired => matches!(next, Calibrating | Invalid | Cancelled),
            // A calibration establishes a reference, or fails to.
            Calibrating => matches!(next, Ready | ReferenceRequired | Invalid | Cancelled),
            Ready => matches!(
                next,
                Benchmarking | Degraded | ReferenceRequired | Cancelled
            ),
            Benchmarking => matches!(next, Complete | Degraded | Cancelled),
            Degraded => matches!(next, Ready | RecoveryRequired | Cancelled),
            // **Nothing leaves recovery on its own.** The way out is a control that reproduces,
            // which is `Ready`, or somebody giving up.
            RecoveryRequired => matches!(next, Ready | Cancelled),
            // Terminal. A new benchmark is a new session, never a reopened one — the rows before
            // and after would have been measured on two different machines.
            Complete | Invalid | Cancelled => false,
        }
    }
}

/// What Epoch needs before it may measure candidates, worked out rather than asked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Needs {
    /// A reference exists and governs. Take one control and compare.
    OneControl { reference: String },
    /// Nothing comparable exists. Calibrate first — **this is allowed here and comparative
    /// profiles are not.**
    Calibration { because: String },
}

/// Decide what this model needs, from what is on file.
///
/// **The whole point of the button.** A user does not choose between calibrating and checking;
/// the answer is a property of the record and the machine, and Epoch reads it.
pub fn what_is_needed(
    governing: Option<&PerformanceReference>,
    against: &crate::reference::Fingerprint,
) -> Needs {
    match governing {
        Some(one) if one.is_safety_reference() => {
            let how = one.fingerprint.comparable_to(against);
            if how.yes() {
                Needs::OneControl {
                    reference: one.id.clone(),
                }
            } else {
                Needs::Calibration {
                    because: format!(
                        "the reference on file is about something else — {}",
                        how.why()
                    ),
                }
            }
        }
        Some(one) => Needs::Calibration {
            because: format!(
                "the reference on file may not govern: {}",
                one.standing.label()
            ),
        },
        None => Needs::Calibration {
            because: "nothing has been measured for this model on this card and build".to_owned(),
        },
    }
}

/// How far a session's controls have moved since it opened.
///
/// ## Never one signal
///
/// The same discipline `health::collapsed` uses, for a slower phenomenon. A control below the
/// band can be a busy moment; controls that fall **and keep falling** are a state leaving.
/// Requiring both is what stops a single noisy sentinel aborting a twenty-minute search, and what
/// stops a slow slide being read as noise.
///
/// Measured, and the reason this exists: `51.60 → 51.19 → 48.04 → 42.75 → 38.58` inside three
/// hours, every step below the last, on an unchanging configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Drift {
    /// Not enough controls to say anything.
    TooFew,
    /// The controls are where they were.
    Steady,
    /// They are falling, step after step, and the latest is outside what the reference allows.
    Detected {
        opening: f64,
        latest: f64,
        fallen_pct: f64,
        steps: usize,
    },
}

impl Drift {
    /// Whether a search must stop.
    pub fn stops_the_search(&self) -> bool {
        matches!(self, Drift::Detected { .. })
    }

    /// What to tell somebody, with the readings that produced it.
    pub fn say(&self, controls: &[f64]) -> Vec<String> {
        match self {
            Drift::TooFew => vec!["Not enough controls to judge drift.".to_owned()],
            Drift::Steady => vec!["Controls are steady.".to_owned()],
            Drift::Detected {
                opening,
                latest,
                fallen_pct,
                steps,
            } => {
                let mut said = vec!["PERFORMANCE DRIFT DETECTED".to_owned()];
                for (n, one) in controls.iter().enumerate() {
                    said.push(match n {
                        0 => format!("Opening control: {one:.2}"),
                        _ => format!("Sentinel {n}: {one:.2}"),
                    });
                }
                said.push(format!(
                    "Fallen {fallen_pct:.1}% over {steps} steps \u{2014} {opening:.2} to {latest:.2}."
                ));
                said.push("Search paused.".to_owned());
                said.push(
                    "Candidate results measured after reproducibility was lost are invalid."
                        .to_owned(),
                );
                said
            }
        }
    }
}

/// Look at a session's controls, oldest first, and say whether the state is leaving.
///
/// `allowed` is how far the same state is known to vary, as a share — measured by a reference
/// where there is one. Without it drift cannot be called: a falling sequence alone is a shape
/// rather than a verdict, because nothing says how wide the same state is.
///
/// **Both signals are about this run.** They used to be *falling* and *below the reference*, and
/// the second is a different question: on a day the machine runs 9% slower than the afternoon a
/// reference was minted, every control is below it from the first one, so three readings that
/// merely wobble downwards read as a collapse. What invalidates a comparison is the machine
/// changing *between the candidates being compared*, so the fall is measured from this run's own
/// opening control.
pub fn drift(controls: &[f64], allowed: Option<f64>) -> Drift {
    if controls.len() < 3 {
        return Drift::TooFew;
    }
    let (Some(opening), Some(latest)) = (controls.first().copied(), controls.last().copied())
    else {
        return Drift::TooFew;
    };

    // **Signal one: it is still falling.** Each reading below the one before it. A single dip
    // between two good readings is noise and does not qualify.
    let falling = controls.windows(2).all(|pair| pair[1] < pair[0]);

    // **Signal two: it has fallen further than the same state is known to vary**, from where this
    // run opened. Without a measured width this is not a judgement Epoch may make.
    let outside = allowed.is_some_and(|share| opening > 0.0 && latest < opening * (1.0 - share));

    if falling && outside && opening > 0.0 {
        return Drift::Detected {
            opening,
            latest,
            fallen_pct: 100.0 * (opening - latest) / opening,
            steps: controls.len() - 1,
        };
    }
    Drift::Steady
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::health::Tolerance;
    use crate::reference::{Confidence, Fingerprint, Observed, Standing};

    fn shape() -> Fingerprint {
        Fingerprint {
            artifact: Some("Qwen3.6-35B-A3B-UD-IQ4_XS".into()),
            artifact_bytes: Some(17_730_509_792),
            runtime: Some("llama_cpp".into()),
            build: Some("0.3.0-dev (build 10622, commit 3737e4137)".into()),
            backend: Some("CUDA".into()),
            gpu: Some("NVIDIA GeForce RTX 4070 SUPER".into()),
            context: Some(32_768),
            cache: Some("f16".into()),
            flash_attention: Some("auto".into()),
            control: Some(crate::optimize::STANDARD_CONTROL.into()),
            measurement_protocol: Some(crate::protocol::STANDARD_CONTROL_V2.id()),
            workload: Some("3 prompts x 160 tokens".into()),
            ..Default::default()
        }
    }

    /// R-001 as it was actually minted: centre 51.60, band 50.05–53.15.
    fn r001() -> PerformanceReference {
        PerformanceReference {
            id: "R-001".into(),
            fingerprint_id: String::new(),
            fingerprint: shape(),
            median: 51.60,
            dispersion: Some(Tolerance {
                middle: 51.60,
                mad: 0.214,
                allowed: 1.548,
            }),
            runs: Some(15),
            rates: vec![],
            observed: Observed::default(),
            sources: vec!["O-001".into(), "O-002".into(), "O-003".into()],
            policy: Some(crate::reference::REPRODUCTION_POLICY_V1.name.to_owned()),
            between: None,
            at: 1,
            legacy: false,
            standing: Standing::Current,
            superseded_by: None,
            short_term: Confidence::High,
            temporal: Confidence::Unknown,
        }
    }

    #[test]
    fn a_search_that_loses_reproducibility_stops_instead_of_blaming_the_candidates() {
        /*
            **The real sequence, 2026-09-01.** One experiment, one machine, three hours, nothing
            changed between the readings. A search across that slope produces twenty candidate
            rows where the last is a quarter slower than the first for reasons that have nothing
            to do with any of them — and every one of them looks like a finding.
        */
        let controls = [51.60, 51.19, 48.04, 42.75, 38.58];
        let got = drift(&controls, Some(0.03));
        assert!(got.stops_the_search(), "{got:?}");
        let Drift::Detected {
            fallen_pct, steps, ..
        } = got
        else {
            panic!("{got:?}");
        };
        assert_eq!(steps, 4);
        assert!((fallen_pct - 25.2).abs() < 0.5, "{fallen_pct}");

        let said = drift(&controls, Some(0.03)).say(&controls);
        let all = said.join("\n");
        assert!(all.contains("PERFORMANCE DRIFT DETECTED"));
        assert!(all.contains("Opening control: 51.60"));
        assert!(all.contains("Sentinel 4: 38.58"));
        assert!(all.contains("Search paused"));
        assert!(all.contains("invalid"), "{all}");
    }

    #[test]
    fn never_one_signal() {
        /*
            The same discipline `health::collapsed` uses, for a slower phenomenon. One dip between
            two good readings is a busy moment and must not abort a twenty-minute search; a
            monotonic slide that stays inside the band is not yet a verdict either.
        */
        // Falling, and the latest is still inside 50.05–53.15: a shape, not a state leaving.
        let inside = [51.60, 51.40, 51.20, 50.90];
        assert_eq!(drift(&inside, Some(0.03)), Drift::Steady);

        // Outside the band, and not falling step after step: one bad sentinel.
        let dip = [51.60, 42.00, 51.40];
        assert_eq!(drift(&dip, Some(0.03)), Drift::Steady);

        // **And without a reference there is no band to be outside of.** A slope alone is not a
        // judgement Epoch may make.
        assert_eq!(drift(&[51.60, 48.0, 42.0], None), Drift::Steady);
    }

    #[test]
    fn two_controls_are_not_a_trend() {
        assert_eq!(drift(&[51.60, 42.0], Some(0.03)), Drift::TooFew);
        assert_eq!(drift(&[], Some(0.03)), Drift::TooFew);
    }

    #[test]
    fn the_button_works_out_what_it_needs_rather_than_asking() {
        // A governing, comparable reference: one control and a comparison.
        assert_eq!(
            what_is_needed(Some(&r001()), &shape()),
            Needs::OneControl {
                reference: "R-001".into()
            },
        );

        // The reference this machine actually has now: correct about its sources, and marked
        // unrepresentative. Calibration is what that permits.
        let marked = PerformanceReference {
            standing: Standing::TemporalCoverageInsufficient,
            superseded_by: None,
            ..r001()
        };
        let got = what_is_needed(Some(&marked), &shape());
        assert!(matches!(got, Needs::Calibration { .. }), "{got:?}");
        let Needs::Calibration { because } = got else {
            unreachable!()
        };
        assert!(because.contains("UNREPRESENTATIVE"), "{because}");

        // A reference about a different build is not about this experiment.
        let elsewhere = Fingerprint {
            build: Some("0.1.2-dev (build 10507, commit 95c409c13)".into()),
            ..shape()
        };
        let got = what_is_needed(Some(&r001()), &elsewhere);
        assert!(matches!(got, Needs::Calibration { .. }), "{got:?}");

        // And a model nothing has been measured for.
        let got = what_is_needed(None, &shape());
        let Needs::Calibration { because } = got else {
            panic!()
        };
        assert!(because.contains("nothing has been measured"), "{because}");
    }

    #[test]
    fn a_finished_benchmark_never_reopens() {
        /*
            The rows before a break and the rows after it were measured on two different machines.
            A new benchmark is a new session — which is the rule `sessions.rs` already keeps for
            pauses, enumerated here so the whole shape is checkable against the diagram.
        */
        for terminal in [Phase::Complete, Phase::Invalid, Phase::Cancelled] {
            for next in [
                Phase::Unbenchmarked,
                Phase::ReferenceRequired,
                Phase::Calibrating,
                Phase::Ready,
                Phase::Benchmarking,
                Phase::Degraded,
                Phase::RecoveryRequired,
            ] {
                assert!(!terminal.may_become(next), "{terminal:?} -> {next:?}");
            }
            assert!(
                terminal.may_become(terminal),
                "staying put is not a transition"
            );
        }
    }

    #[test]
    fn nothing_leaves_recovery_except_a_control_that_reproduces() {
        // Epoch says what it needs and does not act: no reboot, no driver reset, nothing that
        // could take down a remote session. So the only way out is the machine being itself.
        assert!(Phase::RecoveryRequired.may_become(Phase::Ready));
        assert!(Phase::RecoveryRequired.may_become(Phase::Cancelled));
        for next in [
            Phase::Benchmarking,
            Phase::Calibrating,
            Phase::Complete,
            Phase::Degraded,
        ] {
            assert!(!Phase::RecoveryRequired.may_become(next), "{next:?}");
        }
    }

    #[test]
    fn the_agreed_path_through_the_machine_is_legal_and_the_absurd_ones_are_not() {
        let path = [
            Phase::Unbenchmarked,
            Phase::ReferenceRequired,
            Phase::Calibrating,
            Phase::Ready,
            Phase::Benchmarking,
            Phase::Complete,
        ];
        for pair in path.windows(2) {
            assert!(
                pair[0].may_become(pair[1]),
                "{:?} -> {:?}",
                pair[0],
                pair[1]
            );
        }
        // A search collapsing mid-way, and the recovery road.
        assert!(Phase::Benchmarking.may_become(Phase::Degraded));
        assert!(Phase::Degraded.may_become(Phase::RecoveryRequired));
        assert!(Phase::Degraded.may_become(Phase::Ready));

        // Absurd ones.
        assert!(
            !Phase::Unbenchmarked.may_become(Phase::Benchmarking),
            "no reference yet"
        );
        assert!(
            !Phase::ReferenceRequired.may_become(Phase::Ready),
            "calibrate first"
        );
        assert!(
            !Phase::Benchmarking.may_become(Phase::Calibrating),
            "mid-search"
        );
    }

    #[test]
    fn only_a_search_that_stayed_reproducible_produces_profiles() {
        assert!(Phase::Benchmarking.results_are_usable());
        assert!(Phase::Complete.results_are_usable());
        for one in [
            Phase::Degraded,
            Phase::RecoveryRequired,
            Phase::Cancelled,
            Phase::Invalid,
            Phase::ReferenceRequired,
            Phase::Calibrating,
        ] {
            assert!(!one.results_are_usable(), "{one:?}");
        }
    }
}
