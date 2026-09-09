//! How a standard control is measured, named and versioned.
//!
//! ## Why a boolean was not enough
//!
//! The first fix for *the reference was taken with an instrument the control does not have* was
//! going to be a field called `instrumented: bool`. The owner refused it, 2026-09-01, and was
//! right: two different instruments are both `instrumented = true`, and the hidden variable
//! comes straight back under a name that looks like it was handled.
//!
//! So the fingerprint carries a **protocol**, versioned, describing every knob that decides what
//! a number means.
//!
//! ## What the measurements forced into it
//!
//! Three figures on one machine, one afternoon, one nominal configuration:
//!
//! | | tok/s | instrument | router |
//! |---|---|---|---|
//! | RETRY control | 46.7 | none | freshly started |
//! | Calibration B | 41.9 | `load::Watch` | freshly started |
//! | Calibration A | 39.7 | `load::Watch` | had already served a control |
//!
//! **That is not a factorial experiment and it proves no cause.** What it proves is that
//! *process lifecycle* is a live experimental variable that nothing was normalising — A and B
//! ran the identical protocol and their three-run sets do not overlap (A max 40.50, B min 41.44)
//! while placement, shared memory, VRAM, RAM, prompt throughput and TTFT stayed put.
//!
//! Hence [`Lifecycle`], chosen and then always obeyed. `A first load is not a latency` becomes:
//!
//! > **A control is only comparable if its lifecycle and warmup protocol are comparable.**

use serde::{Deserialize, Serialize};

/// How the serving process is treated around a measurement.
///
/// **Pick one and keep it.** Mixing them is what produced two non-overlapping run sets under one
/// protocol name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Lifecycle {
    /// Spawn the router, load the model, warm up, measure, let the model go.
    ///
    /// The standard, because it is the one that can be reproduced from a description. A reused
    /// process carries however many requests it happened to have served, and that number is not
    /// in anybody's notes.
    Fresh,
    /// Measure on whatever was already serving.
    ///
    /// Honest, cheaper, and **not comparable to `Fresh`** — which is the entire reason it is a
    /// recorded value rather than an implementation detail.
    Reused,
}

impl Lifecycle {
    pub fn id(self) -> &'static str {
        match self {
            Lifecycle::Fresh => "fresh",
            Lifecycle::Reused => "reused",
        }
    }
}

/// Everything about *how* a control is taken, as one versioned value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Protocol {
    /// `STANDARD_CONTROL_V2`. The name that travels on a fingerprint.
    pub name: &'static str,
    /// The process lifecycle every run of this protocol obeys.
    pub lifecycle: Lifecycle,
    /// Generations run and **discarded** before anything is timed.
    ///
    /// Deterministic on purpose. An adaptive rule — *warm until two agree* — is the interesting
    /// version and it is also a rule whose stopping point depends on the noise it is measuring.
    /// One number first; adaptivity when there is something to calibrate it against.
    pub warmup_runs: u32,
    /// Timed runs. Their median is the answer and their spread is the evidence for it.
    pub measured_runs: u32,
    /// Whether the load sampler runs during the measurement, and how often it looks.
    ///
    /// **Named as an implementation, not as a boolean.** A second sampler would also be
    /// "instrumented".
    pub instrumentation: &'static str,
    /// Milliseconds between card readings, and between machine readings.
    pub card_every_ms: u64,
    pub machine_every_ms: u64,
    /// What decides the tokens: llama.cpp's own defaults, unless Epoch starts setting them.
    pub sampler: &'static str,
    /// The workload's version, from `bench`.
    pub workload_version: u32,
    /// The shape of the readings this protocol produces. Bumped when a metric's meaning changes.
    pub metrics_version: u32,
}

/// **The standard control.** One definition, obeyed by calibration, clean-state checks, opening
/// controls and every sentinel in a search.
///
/// V1 is not defined here because V1 was not a definition — it was three call sites that happened
/// to do similar things. Anything measured before this exists is recorded as protocol `unknown`,
/// which compares as *unrecorded* and therefore never silently matches V2.
pub const STANDARD_CONTROL_V2: Protocol = Protocol {
    name: "STANDARD_CONTROL_V2",
    lifecycle: Lifecycle::Fresh,
    // One generation, discarded. Chosen rather than measured, and said so: the alternative was
    // an adaptive rule whose stopping point depends on the very noise under investigation.
    warmup_runs: 1,
    // Five, not three. A and B each held their three runs inside ~3% and still landed 5.6%
    // apart — three is enough to describe a load and not enough to pin a machine.
    measured_runs: 5,
    instrumentation: "load::Watch v1",
    card_every_ms: 500,
    machine_every_ms: 5_000,
    sampler: "server default",
    workload_version: crate::bench::VERSION,
    metrics_version: 1,
};

impl Protocol {
    /// The single string a fingerprint carries, and the thing two records are compared on.
    ///
    /// Every knob in the name, so a protocol that is edited without its version being bumped
    /// still cannot pass as the old one. **The name alone would be a promise; this is a
    /// description.**
    pub fn id(&self) -> String {
        format!(
            "{}/lifecycle={};warmup={};runs={};instr={};card={}ms;machine={}ms;sampler={};\
             workload=v{};metrics=v{}",
            self.name,
            self.lifecycle.id(),
            self.warmup_runs,
            self.measured_runs,
            self.instrumentation,
            self.card_every_ms,
            self.machine_every_ms,
            self.sampler,
            self.workload_version,
            self.metrics_version,
        )
    }
}

/// What a serving process had done before the measured runs started.
///
/// Recorded on every observation. It does not gate on its own — the protocol's `lifecycle` is
/// what a comparison keys on — and it is what makes *did these two follow the same sequence?*
/// answerable after the fact rather than by recollection.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sequence {
    /// What was asked for against what ran. **The protocol id is a claim until this agrees.**
    #[serde(default)]
    pub execution: Execution,
    /// `fresh` or `reused`, as it actually happened. May disagree with the protocol's intent,
    /// and if it does the observation says so rather than the protocol lying about it.
    pub lifecycle: Option<String>,
    /// When the serving process started, epoch milliseconds. `None` where it could not be read.
    pub router_started_at: Option<u64>,
    /// How long the process had been alive when the measured runs began.
    pub router_age_ms: Option<u64>,
    /// Requests this process had served before the first *measured* run — warmups included.
    pub requests_before_measurement: Option<u32>,
    pub warmup_runs: Option<u32>,
    /// What the warmups themselves read, kept as evidence rather than averaged in. A warmup that
    /// came back far from the measured runs is the interesting case.
    #[serde(default)]
    pub warmup_rates: Vec<f64>,
}

/// What the protocol asked for against what actually ran.
///
/// ## Asking for V2 is not the same as V2 having happened
///
/// Measured 2026-09-01, by running the first calibration. `STANDARD_CONTROL_V2` declared
/// `warmup=1;runs=5`, that id travelled on the fingerprint as a description of the run, and what
/// actually happened was **1 + 3 and then 1 + 3** — `bench::measure` had a private run count and
/// a warm-up of its own, so the caller's numbers were silently replaced.
///
/// The protocol id was a **claim**, and nothing checked it. A record whose description does not
/// match its content is worse than no record: it is evidence that will be trusted.
///
/// So the protocol declares what should happen, the observation records what did, and this
/// compares them. A mismatch can never become a reference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Execution {
    pub warmups_expected: u32,
    pub warmups_observed: u32,
    pub runs_expected: u32,
    pub runs_observed: u32,
}

impl Execution {
    /// Whether what ran is what was declared.
    ///
    /// **Exact, in both directions.** Fewer runs than asked for is a truncated measurement; more
    /// is an instrument doing something nobody described. Neither is the protocol.
    pub fn matched(&self) -> bool {
        self.warmups_expected == self.warmups_observed && self.runs_expected == self.runs_observed
    }

    pub fn label(&self) -> &'static str {
        if self.matched() {
            "MATCH"
        } else {
            "MISMATCH"
        }
    }

    /// The two lines a person checks.
    pub fn describe(&self) -> Vec<String> {
        vec![
            format!(
                "Declared: {} warmup + {} measured",
                self.warmups_expected, self.runs_expected
            ),
            format!(
                "Observed: {} warmup + {} measured",
                self.warmups_observed, self.runs_observed
            ),
            format!("Protocol execution: {}", self.label()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_protocol_id_describes_every_knob_rather_than_promising_with_a_name() {
        /*
            A name alone is a promise: edit the protocol, forget the version, and old records
            still compare equal to new ones. The id carries the knobs, so an unversioned edit
            makes the comparison fail loudly instead of passing quietly.
        */
        let id = STANDARD_CONTROL_V2.id();
        assert!(id.starts_with("STANDARD_CONTROL_V2/"));
        for part in [
            "lifecycle=fresh",
            "warmup=1",
            "runs=5",
            "instr=load::Watch v1",
            "card=500ms",
            "machine=5000ms",
            "sampler=server default",
            "metrics=v1",
        ] {
            assert!(id.contains(part), "{part} missing from {id}");
        }

        // An edited knob is a different protocol even under the same name.
        let edited = Protocol {
            measured_runs: 3,
            ..STANDARD_CONTROL_V2
        };
        assert_ne!(edited.id(), STANDARD_CONTROL_V2.id());
    }

    #[test]
    fn a_boolean_would_not_have_been_enough() {
        /*
            The owner's refusal, 2026-09-01. Two different instruments are both
            `instrumented = true`, and the hidden variable returns under a name that looks like it
            was handled.
        */
        let other = Protocol {
            instrumentation: "load::Watch v2",
            ..STANDARD_CONTROL_V2
        };
        assert_ne!(other.id(), STANDARD_CONTROL_V2.id());

        // As would a sampler that looks ten times as often — the thing that may be costing 11%.
        let busy = Protocol {
            card_every_ms: 50,
            ..STANDARD_CONTROL_V2
        };
        assert_ne!(busy.id(), STANDARD_CONTROL_V2.id());
    }

    #[test]
    fn lifecycle_is_part_of_the_identity_because_the_measurements_made_it_one() {
        /*
            A and B ran the identical protocol and their run sets do not overlap — A max 40.50
            against B min 41.44 — while placement, shared memory, VRAM, RAM, prompt throughput
            and TTFT stayed put. The named difference was that B's router was freshly started and
            A's had already served a control. That proves no cause; it proves the variable is
            live and was not being normalised.
        */
        let reused = Protocol {
            lifecycle: Lifecycle::Reused,
            ..STANDARD_CONTROL_V2
        };
        assert_ne!(reused.id(), STANDARD_CONTROL_V2.id());
        assert_eq!(STANDARD_CONTROL_V2.lifecycle, Lifecycle::Fresh);
    }

    #[test]
    fn the_standard_takes_five_runs_and_one_discarded_warmup() {
        // Three runs each held inside ~3% and still landed 5.6% apart between loads: enough to
        // describe a load, not enough to pin a machine.
        assert_eq!(STANDARD_CONTROL_V2.measured_runs, 5);
        assert_eq!(STANDARD_CONTROL_V2.warmup_runs, 1);
    }
}
