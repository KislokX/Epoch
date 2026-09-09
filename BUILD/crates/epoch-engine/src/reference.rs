//! What a performance number is a number *about*.
//!
//! ## Why this file exists
//!
//! Measured 2026-08-31. A benchmark was resumed after a reboot and Epoch answered *"the control
//! reproduces: 40.1 tok/s. This machine is itself again"*. It had compared nothing: the clean
//! reference lived on the paused record as a bare `f64` — `46.0` — and the gate read a different
//! list, which was empty. One fact written in two places, and the gate was reading the vacant one.
//!
//! Fixing the lookup was necessary and was not enough, because the number it would then have used
//! is **46.0 and a date**. Nothing on it says which artefact, which build, which context, which
//! cache, or what workload produced it. Comparing a fresh control against that is not science; it
//! is a memory with a decimal point.
//!
//! > **A performance figure without provenance is an anecdote.** It may be true and it may be
//! > about something else, and there is no way to tell which from the number.
//!
//! ## What comparability means here
//!
//! A reference is comparable to a control when everything that can change throughput is **known
//! on both sides and equal**. Three verdicts, and the third is the one this repository keeps
//! getting wrong in other subsystems:
//!
//! | | |
//! |---|---|
//! | [`Comparison::Comparable`] | every field that matters is known on both sides and agrees |
//! | [`Comparison::Differs`] | something that matters is known on both sides and disagrees |
//! | [`Comparison::Incomplete`] | something that matters is unrecorded on one side or the other |
//!
//! **`Incomplete` is not `Comparable`, and it is not `Differs` either.** Silence about a field is
//! not evidence that the field matched. That is the same discipline as `Shown`'s every-field-
//! optional rule and `Declared::uses_tools` — `None` means *unasked*, never *no* — arriving in the
//! one place where reading it wrongly decides whether hours of measurement may be trusted.
//!
//! ## And a legacy figure is kept, not deleted and not promoted
//!
//! The historical `46.0` stays, labelled for what it is: an observation whose provenance did not
//! survive. It may not act as a safety reference — `is_safety_reference()` says so — and it is
//! still worth showing a person, because *we once saw 46 here* is real information about the
//! machine even when it cannot be a gate.

use serde::{Deserialize, Serialize};

use crate::models::health::Tolerance;

/// Everything about a measurement that can change the number it produced.
///
/// **Every field is `Option`, and that is the design rather than laziness.** A record written
/// before a field existed genuinely does not know it, and inventing a value to fill the gap would
/// make an old anecdote look like a modern measurement. `None` reads as *unrecorded* everywhere
/// below and never as *the same as yours*.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fingerprint {
    /* ---- what was loaded ---------------------------------------------------------------- */
    /// The model as a person names it: `Qwen3.6-35B-A3B`. Two quantisations share this.
    pub logical_model: Option<String>,
    /// The exact artefact, by Epoch's own identity (`artifact::Identity::id`) — which already
    /// distinguishes two files that share a filename stem.
    pub artifact: Option<String>,
    /// Size in bytes. The cheap half of *is this the same file* (`loadout::names`' reasoning).
    pub artifact_bytes: Option<u64>,
    pub quant: Option<String>,
    /// Whether this artefact carries its own MTP head. Read from the file, never from its name.
    pub mtp: Option<bool>,

    /* ---- what ran it -------------------------------------------------------------------- */
    pub runtime: Option<String>,
    pub build: Option<String>,
    /// CUDA · Vulkan · ROCm · Metal. **One build carries one GPU backend**, and the winget
    /// llama.cpp is Vulkan-only while `~/.llama/bin` is CUDA — measured at 4.5x apart on one card.
    pub backend: Option<String>,

    /* ---- what it ran on ----------------------------------------------------------------- */
    pub gpu: Option<String>,
    pub driver: Option<String>,
    pub os: Option<String>,

    /* ---- how it was configured ---------------------------------------------------------- */
    pub context: Option<u32>,
    /// `f16` or `q8_0`.
    pub cache: Option<String>,
    pub flash_attention: Option<String>,
    pub speculation: Option<String>,
    /// `--override-tensor`, opaque and compared as a string.
    pub offload: Option<String>,
    pub gpu_layers: Option<i32>,
    pub batch: Option<u32>,
    pub ubatch: Option<u32>,

    /* ---- what was asked of it ----------------------------------------------------------- */
    /// **Which experiment this reference governs**, named rather than inferred.
    ///
    /// The owner's requirement, 2026-08-31: a figure established for the 32K standard control
    /// must not later find itself gating a 64K run, an MTP artefact, an IQ2 quantisation or
    /// another build.
    ///
    /// Every one of those already differs on a field above, so this is belt *and* braces — and
    /// the braces are the half that survives somebody adding a field later and forgetting to
    /// require it. A named purpose cannot be accidentally satisfied; a set of coincidentally
    /// equal fields can.
    pub control: Option<String>,
    /// **How it was measured**, from `protocol::Protocol::id`.
    ///
    /// The owner refused an `instrumented: bool` for this, 2026-09-01, and was right: two
    /// different instruments are both `true`, and the hidden variable returns under a name that
    /// looks like it was handled. The protocol id carries the lifecycle, the warmup, the run
    /// count, the sampler's intervals and the metrics version, so an unversioned edit makes a
    /// comparison fail loudly rather than pass quietly.
    pub measurement_protocol: Option<String>,
    /// The workload, by name — which prompt, how many tokens.
    pub workload: Option<String>,
    /// The benchmark's own version, so a changed workload cannot masquerade as a changed machine.
    pub benchmark_version: Option<u32>,
    pub sampler: Option<String>,
}

/// The fields that decide whether two numbers are about the same thing.
///
/// **Deliberately not every field.** `driver` and `os` are recorded and are compared *when both
/// sides know them*, but a reference taken before Epoch read the driver is not thereby useless —
/// it is a reference with one fewer guarantee, and saying so is more useful than refusing it. The
/// list below is the set where absence alone is enough to make the comparison unprovable.
const MUST_BE_KNOWN: &[&str] = &[
    "artifact",
    // Two files published under one filename stem differ by 479 MB and are two different
    // models. The size is the cheap half of *is this the same file*.
    "artifactBytes",
    "runtime",
    "build",
    // The 4.5x. Same GGUF, same card, CUDA against Vulkan.
    "backend",
    "gpu",
    "context",
    "cache",
    "flashAttention",
    "control",
    // **The instrument.** A reference taken with a load sampler and a control taken without one
    // are two experiments; the day that was not recorded, they were 11% apart and about to be
    // compared.
    "measurementProtocol",
    "workload",
];

impl Fingerprint {
    /// Field by field, as `(name, mine, theirs)` — the shape both checks below read.
    fn against<'a>(&'a self, other: &'a Fingerprint) -> Vec<(&'static str, String, String)> {
        fn s(one: &Option<impl ToString>) -> String {
            one.as_ref().map(ToString::to_string).unwrap_or_default()
        }
        vec![
            (
                "logicalModel",
                s(&self.logical_model),
                s(&other.logical_model),
            ),
            ("artifact", s(&self.artifact), s(&other.artifact)),
            (
                "artifactBytes",
                s(&self.artifact_bytes),
                s(&other.artifact_bytes),
            ),
            ("quant", s(&self.quant), s(&other.quant)),
            ("mtp", s(&self.mtp), s(&other.mtp)),
            ("runtime", s(&self.runtime), s(&other.runtime)),
            ("build", s(&self.build), s(&other.build)),
            ("backend", s(&self.backend), s(&other.backend)),
            ("gpu", s(&self.gpu), s(&other.gpu)),
            ("driver", s(&self.driver), s(&other.driver)),
            ("os", s(&self.os), s(&other.os)),
            ("context", s(&self.context), s(&other.context)),
            ("cache", s(&self.cache), s(&other.cache)),
            (
                "flashAttention",
                s(&self.flash_attention),
                s(&other.flash_attention),
            ),
            ("speculation", s(&self.speculation), s(&other.speculation)),
            ("offload", s(&self.offload), s(&other.offload)),
            ("gpuLayers", s(&self.gpu_layers), s(&other.gpu_layers)),
            ("batch", s(&self.batch), s(&other.batch)),
            ("ubatch", s(&self.ubatch), s(&other.ubatch)),
            ("control", s(&self.control), s(&other.control)),
            (
                "measurementProtocol",
                s(&self.measurement_protocol),
                s(&other.measurement_protocol),
            ),
            ("workload", s(&self.workload), s(&other.workload)),
            (
                "benchmarkVersion",
                s(&self.benchmark_version),
                s(&other.benchmark_version),
            ),
            ("sampler", s(&self.sampler), s(&other.sampler)),
        ]
    }

    /// A deterministic token for *this kind of experiment*.
    ///
    /// **This is the fingerprint's identity, and it is not a measurement's identity.** The
    /// fingerprint answers *what kind of experiment is this and what may it be compared with*;
    /// a measurement answers *what happened this time*. Keying the store on the fingerprint made
    /// the second calibration overwrite the first — two real observations of the same experiment,
    /// and only the newer one survived, which is precisely the drift a history exists to show.
    ///
    /// Built from every field, so two fingerprints that differ anywhere get different keys.
    pub fn key(&self) -> String {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for (name, mine, _) in self.against(&Fingerprint::default()) {
            for byte in name.bytes().chain(b"=".iter().copied()).chain(mine.bytes()) {
                hash ^= byte as u64;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        format!("{hash:016x}")
    }

    /// The fields this fingerprint cannot answer that a comparison needs.
    pub fn unrecorded(&self) -> Vec<&'static str> {
        self.against(&Fingerprint::default())
            .into_iter()
            .filter(|(name, mine, _)| mine.is_empty() && MUST_BE_KNOWN.contains(name))
            .map(|(name, _, _)| name)
            .collect()
    }

    /// Whether this fingerprint records everything a comparison needs.
    pub fn complete(&self) -> bool {
        self.unrecorded().is_empty()
    }

    /// Whether a measurement taken with `other` may be compared against one taken with this.
    pub fn comparable_to(&self, other: &Fingerprint) -> Comparison {
        let pairs = self.against(other);

        // **Different beats unrecorded.** A build that is known to differ is a definite answer
        // and deserves to be reported as one, even where a third field is also unrecorded.
        let differs: Vec<String> = pairs
            .iter()
            .filter(|(_, mine, theirs)| !mine.is_empty() && !theirs.is_empty() && mine != theirs)
            .map(|(name, mine, theirs)| format!("{name}: {mine} \u{2192} {theirs}"))
            .collect();
        if !differs.is_empty() {
            return Comparison::Differs(differs);
        }

        let missing: Vec<String> = pairs
            .iter()
            .filter(|(name, mine, theirs)| {
                MUST_BE_KNOWN.contains(name) && (mine.is_empty() || theirs.is_empty())
            })
            .map(|(name, _, _)| (*name).to_owned())
            .collect();
        if !missing.is_empty() {
            return Comparison::Incomplete(missing);
        }

        Comparison::Comparable
    }
}

/// Whether two measurements are about the same thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Comparison {
    Comparable,
    /// Something that changes throughput is known on both sides and disagrees.
    Differs(Vec<String>),
    /// Something that changes throughput is unrecorded on one side or the other.
    Incomplete(Vec<String>),
}

impl Comparison {
    pub fn yes(&self) -> bool {
        *self == Comparison::Comparable
    }

    /// What to tell somebody, in one sentence.
    pub fn why(&self) -> String {
        match self {
            Comparison::Comparable => {
                "the same artefact, build, card, context, cache and workload".to_owned()
            }
            Comparison::Differs(what) => format!("these differ \u{2014} {}", what.join("; ")),
            Comparison::Incomplete(what) => {
                format!("these were never recorded \u{2014} {}", what.join(", "))
            }
        }
    }
}

/// What a calibration run saw beside the number it produced.
///
/// **None of this decides comparability.** A reference governs experiments whose *fingerprint*
/// matches; how much system memory was free that afternoon is a fact about the afternoon. It is
/// kept because a person coming back to a reference wants to know the machine was not visibly
/// struggling when it was taken, and because an unexplained later reading is easier to explain
/// beside a picture of the healthy one.
///
/// Every field `Option`, and every one of them a measurement or nothing.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observed {
    /// Prompt-processing rate, median across the runs.
    pub prompt: Option<f64>,
    /// Time to the first token, milliseconds.
    pub first_token_ms: Option<f64>,
    /// Dedicated video memory in use at the peak, in bytes.
    pub vram_used: Option<u64>,
    /// The GPU's shared pool — read twice, never sampled: it is the most expensive reading on
    /// the machine. **The number that named the original fault.**
    pub shared_before: Option<u64>,
    pub shared_after: Option<u64>,
    pub ram_used: Option<u64>,
    pub gpu_percent: Option<f64>,
    pub cpu_percent: Option<f64>,
    /// What the sampler itself cost, so a reading is not quietly a measurement of the
    /// instrument. Measured once at 13% → 55% CPU before the interval was widened.
    pub sampling_overhead: Option<f64>,
}

/// One measured, healthy figure, and everything needed to know what it is a figure about.
///
/// **The single entity.** A paused session points at one of these by `id`; it does not keep its
/// own copy of the number. Two independent spellings of one fact is precisely the defect this
/// file was written after.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReference {
    /// **This measurement**, not this kind of measurement. `R-001`, `R-002`, monotonic.
    ///
    /// The store used to key on the fingerprint, so a second calibration of the same experiment
    /// replaced the first. Two real observations, one survivor — and the difference between them
    /// (39.68 against 41.90, non-overlapping run sets) is exactly what a history is for.
    pub id: String,
    /// Which *kind* of experiment this is: `Fingerprint::key`. Several references share it, and
    /// that is the point.
    #[serde(default)]
    pub fingerprint_id: String,
    pub fingerprint: Fingerprint,
    /// Median generation rate over the runs that were kept.
    pub median: f64,
    /// The spread those runs had, where it was kept. **`None` is why a legacy figure gets the
    /// conservative fallback band rather than a measured one.**
    pub dispersion: Option<Tolerance>,
    pub runs: Option<usize>,
    /// Every rate that was kept, so the median and the spread can be checked rather than
    /// believed. Three numbers cost nothing to store and are the whole evidence for the fourth.
    #[serde(default)]
    pub rates: Vec<f64>,
    /// What else the calibration saw. **Recorded, and no part of comparability** — a reference
    /// governs experiments whose *fingerprint* matches, and how much RAM was free that afternoon
    /// is a fact about the afternoon.
    #[serde(default)]
    pub observed: Observed,
    pub at: u64,
    /// Which observations this was built from. Empty for a reference minted from a single run.
    ///
    /// **The evidence, addressable.** A number whose sources are named can be re-derived; one
    /// whose sources are gone is back to being an anecdote with a decimal point.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Which policy built the band, by name. `None` for anything older than policies.
    #[serde(default)]
    pub policy: Option<String>,
    /// The between-process figures the band came from.
    #[serde(default)]
    pub between: Option<crate::models::health::Spread>,
    /// Whether this still governs. **Never a reason to edit the numbers above.**
    #[serde(default)]
    pub standing: Standing,
    /// Which reference replaced this one, when one did.
    ///
    /// **So that retiring a replacement does not reinstate what it replaced.** Standing alone
    /// cannot tell the two apart: a reference marked aside for its own reasons and one that was
    /// explicitly succeeded both read `Superseded`, and the difference decides whether the
    /// authority falls back to the older row or to nothing at all.
    ///
    /// Set at the one point a replacement actually arrives. Clearing it is reinstatement, and it
    /// is deliberate — which is the whole reason it is a field rather than an inference.
    #[serde(default)]
    pub superseded_by: Option<String>,
    /// How well its sources agreed with each other, over the minutes they were taken.
    #[serde(default)]
    pub short_term: Confidence,
    /// Whether it still holds hours later. **`Unknown` until something measures it**, which is
    /// what a later comparable control does whether or not anybody asked it to.
    #[serde(default)]
    pub temporal: Confidence,
    /// Recovered from an older record whose provenance did not survive.
    ///
    /// Kept and labelled rather than deleted: *we once saw 46 here* is real information about
    /// this machine. It simply cannot decide whether a later number is a regression.
    #[serde(default)]
    pub legacy: bool,
}

impl PerformanceReference {
    /// Fill `fingerprint_id` in from the fingerprint. **One fact, derived, never typed.**
    pub fn keyed(mut self) -> Self {
        self.fingerprint_id = self.fingerprint.key();
        self
    }

    /// Whether this may decide that a machine is or is not itself.
    ///
    /// **A legacy figure may not**, and neither may one whose fingerprint is missing something a
    /// comparison needs. Both are still shown; neither is a gate.
    pub fn is_safety_reference(&self) -> bool {
        !self.legacy
            && self.fingerprint.complete()
            && self.standing.governs()
            // A reference that was explicitly replaced does not come back when its replacement
            // is retired. Standing would allow it the moment somebody restood this one, and
            // *what replaced it* is the fact that decides — so it is read, not inferred.
            && self.superseded_by.is_none()
    }

    /// The band a later reading has to land in, and where that band came from.
    ///
    /// Measured where the original runs were kept; the 3% floor otherwise, **flagged as a
    /// fallback** so nobody reads a provisional guard as a measured distribution.
    pub fn band(&self) -> Option<(Tolerance, Band)> {
        match self.dispersion {
            Some(measured) => Some((measured, Band::Measured)),
            None => Tolerance::around(self.median).map(|it| (it, Band::Fallback)),
        }
    }

    /// One line for a person: the number, and what it is a number about.
    pub fn describe(&self) -> String {
        let mut said = format!("{:.1} tok/s", self.median);
        if let Some(runs) = self.runs {
            said.push_str(&format!(" over {runs} runs"));
        }
        let f = &self.fingerprint;
        let mut about: Vec<String> = Vec::new();
        if let Some(one) = &f.artifact {
            about.push(one.clone());
        }
        if let Some(one) = f.context {
            about.push(format!("{}K", one / 1024));
        }
        if let Some(one) = &f.cache {
            about.push(format!("cache {one}"));
        }
        if let Some(one) = &f.flash_attention {
            about.push(format!("flash attention {one}"));
        }
        if let Some(one) = &f.build {
            about.push(one.clone());
        }
        if !about.is_empty() {
            said.push_str(&format!(" \u{00b7} {}", about.join(" \u{00b7} ")));
        }
        if self.legacy {
            said.push_str(" \u{00b7} LEGACY, provenance incomplete");
        }
        said
    }
}

/// Whether a control reproduced a reference, and if not, which way it missed.
///
/// ## Reproduction is equivalence, not "at least as fast"
///
/// The owner's correction, 2026-09-01. `Tolerance::reproduces` is deliberately one-sided, and it
/// is right for the question it was written about: *is this machine worse than it was?* A reading
/// that came back faster is the machine having settled, and refusing it would abort a search for
/// getting better.
///
/// But a clean-state check is not asking that. It asks **does this reproduce the reference**, and
/// a control 18% above its reference has not reproduced it — it has produced a different state,
/// which is exactly as interesting and exactly as disqualifying for a comparison.
///
/// Live on this machine: a reference of 39.7 against a control of 46.7. One-sided, that is a
/// pass and the pause gets cleared. Two-sided, it is [`Reproduction::AboveReference`] — *the
/// experiment changed, or the reference is stale*, and either way nothing may be compared until
/// somebody looks.
///
/// So there are two gates and they are not the same gate:
///
/// | | |
/// |---|---|
/// | **degradation** | `current >= lower` — one-sided, asks *are we worse* |
/// | **reproducibility** | `lower <= current <= upper` — two-sided, asks *is this the same state* |
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Reproduction {
    /// Inside the band, both sides.
    Reproduced,
    /// Slower than the band allows. The machine is worse than the reference.
    BelowReference,
    /// Faster than the band allows. **Not a pass**: the experiment changed or the reference is
    /// stale, and a comparison against it would be a comparison of two different states.
    AboveReference,
    /// The reference is about something else.
    Incomparable(Comparison),
    /// Nothing usable exists to compare against.
    Incomplete,
}

impl Reproduction {
    /// Whether comparative work may proceed. **Only `Reproduced`.**
    pub fn yes(&self) -> bool {
        *self == Reproduction::Reproduced
    }

    /// Whether this says the machine is *worse*, which is a narrower claim than *not the same*.
    ///
    /// The distinction `RecoveryRequired` needs: only `BelowReference` is evidence of damage.
    /// `AboveReference` is evidence that something changed, and pointing somebody at a hardware
    /// fault over it would be an invention.
    pub fn degraded(&self) -> bool {
        *self == Reproduction::BelowReference
    }

    pub fn label(&self) -> &'static str {
        match self {
            Reproduction::Reproduced => "REPRODUCED",
            Reproduction::BelowReference => "NOT REPRODUCED \u{2014} BELOW REFERENCE",
            Reproduction::AboveReference => "NOT REPRODUCED \u{2014} ABOVE REFERENCE",
            Reproduction::Incomparable(_) => "NOT COMPARABLE",
            Reproduction::Incomplete => "NO REFERENCE",
        }
    }
}

impl PerformanceReference {
    /// Two-sided: does this control reproduce the state this reference was taken in?
    pub fn reproduced_by(&self, control: f64, against: &Fingerprint) -> Reproduction {
        if !self.is_safety_reference() {
            return Reproduction::Incomplete;
        }
        let comparison = self.fingerprint.comparable_to(against);
        if !comparison.yes() {
            return Reproduction::Incomparable(comparison);
        }
        let Some((band, _)) = self.band() else {
            return Reproduction::Incomplete;
        };
        if control < band.middle - band.allowed {
            return Reproduction::BelowReference;
        }
        if control > band.middle + band.allowed {
            return Reproduction::AboveReference;
        }
        Reproduction::Reproduced
    }
}

/// One measurement that happened, before anybody decides whether it may govern anything.
///
/// ## Observation, validation, reference
///
/// The owner's separation, 2026-09-01: **a calibration should not become an authority merely by
/// having run.** A run that produced two usable rates instead of five, or that was taken while
/// something else had the card, is still evidence worth keeping — and it is not a thing to judge
/// hours of later work against.
///
/// So this is what a run produced, and [`Observation::validate`] is the step that decides whether
/// it may become a [`PerformanceReference`]. A refusal keeps the observation; it only withholds
/// the authority.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Observation {
    /// **This run**: `O-001`, `O-002`, monotonic. An observation is a thing that happened and it
    /// keeps its own name whether or not anything is ever built on it.
    #[serde(default)]
    pub id: String,
    /// Which kind of experiment, from `Fingerprint::key`. Several observations share it.
    #[serde(default)]
    pub fingerprint_id: String,
    pub fingerprint: Fingerprint,
    /// The protocol id this run followed, from `protocol::Protocol::id`.
    pub protocol: String,
    /// **Which errand took this measurement.** `calibration` · `retry` · `openingControl` ·
    /// `sentinel` · `finalControl`.
    ///
    /// ## Every valid standard control produces an observation
    ///
    /// The rule the owner set, 2026-09-01, after a control read 48.04 tok/s — five runs, protocol
    /// MATCH, fingerprint comparable to the reference it was being judged against — and existed
    /// only as a sentence on a screen. `clean_state_check` measured it, compared it, and threw the
    /// rates away, so the one reading that showed `R-001` was unrepresentative could not be
    /// reconstructed afterwards without inventing five numbers.
    ///
    /// **The route by which a measurement was taken must not decide whether it survives.** A
    /// retry is not a special source of truth; it is `measure_standard_control` followed by a
    /// comparison. So is a sentinel. So is an opening control. This field records which, and
    /// nothing keys on it — it is provenance, not behaviour.
    #[serde(default)]
    pub source: String,
    /// What the serving process had done before the measured runs began.
    pub sequence: crate::protocol::Sequence,
    /// Every kept rate, in the order they were run.
    pub rates: Vec<f64>,
    pub observed: Observed,
    pub at: u64,
}

/// Whether an observation is a true record of the protocol it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ObservationStatus {
    Valid,
    /// Kept as diagnostic evidence and excluded from everything else: references, tolerances,
    /// gates, recommendations, historical medians and the optimizer.
    InvalidProtocolExecution,
}

impl ObservationStatus {
    pub fn label(self) -> &'static str {
        match self {
            ObservationStatus::Valid => "VALID",
            ObservationStatus::InvalidProtocolExecution => "INVALID PROTOCOL EXECUTION",
        }
    }

    pub fn usable(self) -> bool {
        self == ObservationStatus::Valid
    }
}

/// Why an observation may not become a reference.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotAReference {
    /// **What ran is not what the protocol declared.**
    ///
    /// The failure that produced this, 2026-09-01: the protocol said `warmup=1;runs=5`, the run
    /// was 1+3 and then 1+3, and the id travelled on the fingerprint describing a run that did not
    /// happen. Asking for V2 is not the same as V2 having occurred, and until this is checked the
    /// protocol id is a claim rather than a record.
    InvalidProtocolExecution(crate::protocol::Execution),
    /// Nothing came back.
    NoReading,
    /// Fewer usable runs than the protocol asked for.
    TooFewRuns { got: usize, wanted: usize },
    /// The runs disagree by more than one configuration's worth.
    NotOneConfiguration { observed_range_pct: f64 },
    /// The fingerprint could not record something a comparison needs.
    Unrecorded(Vec<String>),
}

impl NotAReference {
    pub fn why(&self) -> String {
        match self {
            NotAReference::InvalidProtocolExecution(one) => format!(
                "what ran is not what the protocol declared — {} warmup + {} measured were \
                 asked for and {} + {} happened",
                one.warmups_expected, one.runs_expected, one.warmups_observed, one.runs_observed,
            ),
            NotAReference::NoReading => "it produced no reading at all".to_owned(),
            NotAReference::TooFewRuns { got, wanted } => {
                format!("only {got} of {wanted} runs finished")
            }
            NotAReference::NotOneConfiguration { observed_range_pct } => format!(
                "the runs span {observed_range_pct:.2}%, which is more than one configuration"
            ),
            NotAReference::Unrecorded(what) => {
                format!("it could not record {}", what.join(", "))
            }
        }
    }
}

impl Observation {
    /// Whether this observation is what it says it is.
    ///
    /// **Separate from `validate`.** Validation asks *may this govern later work*, which has
    /// several answers and most of them still leave usable evidence. This asks *is the record
    /// true*, and there is only one answer that keeps it usable at all: a run whose description
    /// does not match its content is worse than no run, because it is evidence that will be
    /// trusted.
    pub fn status(&self) -> ObservationStatus {
        if self.sequence.execution.matched() {
            ObservationStatus::Valid
        } else {
            ObservationStatus::InvalidProtocolExecution
        }
    }

    /// Name it, and derive its fingerprint key. **One fact, derived, never typed.**
    pub fn named(mut self, id: &str) -> Self {
        self.id = id.to_owned();
        self.fingerprint_id = self.fingerprint.key();
        self
    }

    /// The descriptive figures. Never a verdict.
    pub fn spread(&self) -> Option<crate::models::health::Spread> {
        crate::models::health::Spread::of(&self.rates)
    }

    /// Whether this may govern later comparisons, and a reference if it may.
    ///
    /// **Every refusal keeps the observation.** The caller stores it either way; what this
    /// decides is whether it is also an authority.
    pub fn validate(
        &self,
        id: &str,
        wanted_runs: usize,
    ) -> Result<PerformanceReference, NotAReference> {
        // **First**, because everything below it describes a run whose description may be false.
        if !self.sequence.execution.matched() {
            return Err(NotAReference::InvalidProtocolExecution(
                self.sequence.execution,
            ));
        }
        let Some(spread) = self.spread() else {
            return Err(NotAReference::NoReading);
        };
        if spread.runs < wanted_runs {
            return Err(NotAReference::TooFewRuns {
                got: spread.runs,
                wanted: wanted_runs,
            });
        }
        if !spread.one_configuration() {
            return Err(NotAReference::NotOneConfiguration {
                observed_range_pct: spread.observed_range_pct,
            });
        }
        let missing = self.fingerprint.unrecorded();
        if !missing.is_empty() {
            return Err(NotAReference::Unrecorded(
                missing.into_iter().map(str::to_owned).collect(),
            ));
        }
        Ok(PerformanceReference {
            id: id.to_owned(),
            fingerprint_id: String::new(),
            fingerprint: self.fingerprint.clone(),
            median: spread.median,
            dispersion: Tolerance::of(&self.rates),
            runs: Some(spread.runs),
            rates: self.rates.clone(),
            observed: self.observed.clone(),
            sources: Vec::new(),
            policy: None,
            between: None,
            at: self.at,
            legacy: false,
            standing: Standing::Current,
            superseded_by: None,
            /*
                **Established, because it was measured.** Three fresh processes agreed under a
                named policy; that is short-term reproducibility and recording it as `Unknown`
                would be a cold instrument pointed at a reading that exists.
            */
            short_term: Confidence::High,
            /*
                **`Unknown` means nobody asked**, not *assume the worst*. Whether this still holds
                in an hour is a different question and no calibration series can answer it —
                R-001's three calibrations agreed to 1.2% over twenty minutes and the same
                experiment read 25% lower two hours later.

                It is learned during the benchmark, from sentinels and later sessions, rather than
                by making somebody wait an hour before a search may start.
            */
            temporal: Confidence::Unknown,
        }
        .keyed())
    }
}

/// Several calibrations of one experiment, with the two kinds of variability kept apart.
///
/// ## Why the fifteen runs may not simply be pooled
///
/// The owner's rule, 2026-09-01, and it is a statement about the experiment rather than about
/// statistics: **five runs inside one process are not five runs from five processes.** They share
/// a model load, a memory layout, whatever the driver did that minute — they are clustered, and
/// pooling them reports the tight within-process agreement as if it described the machine.
///
/// Measured, before this existed: two calibrations each held their runs inside ~3% and landed
/// 5.6% apart. Pooled, that is one wide distribution and a shrug. Kept apart, it is *each load is
/// tight and the loads disagree*, which is a different finding and the actionable one.
///
/// | | what it answers |
/// |---|---|
/// | **within** | is this instance steady while it runs? |
/// | **between** | does a fresh process reproduce the same state? |
///
/// So the hierarchy is preserved — calibration, then runs — and both levels are reported. No
/// tolerance is derived here: how to build a band out of the two is a decision to make **after**
/// looking at the distribution, not before.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Series {
    pub fingerprint_id: String,
    pub protocol: String,
    /// One entry per calibration, oldest first.
    pub calibrations: Vec<Calibrated>,
    /// The medians, treated as the sample. **Between-process variability.**
    pub between: Option<crate::models::health::Spread>,
}

/// One calibration's own numbers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Calibrated {
    pub id: String,
    pub at: u64,
    pub rates: Vec<f64>,
    /// **Within-process variability.** `None` where nothing came back.
    pub within: Option<crate::models::health::Spread>,
    pub warmup_rates: Vec<f64>,
    pub observed: Observed,
}

impl Series {
    /// Build the two levels from a set of observations of one experiment.
    pub fn of(observations: &[Observation]) -> Self {
        let mut sorted: Vec<&Observation> = observations.iter().collect();
        sorted.sort_by_key(|it| it.at);
        let calibrations: Vec<Calibrated> = sorted
            .iter()
            .map(|one| Calibrated {
                id: one.id.clone(),
                at: one.at,
                rates: one.rates.clone(),
                within: one.spread(),
                warmup_rates: one.sequence.warmup_rates.clone(),
                observed: one.observed.clone(),
            })
            .collect();
        // The medians, and only the medians. One number per process, which is what makes this
        // the between-process sample rather than a second view of the same fifteen readings.
        let medians: Vec<f64> = calibrations
            .iter()
            .filter_map(|it| it.within.as_ref().map(|it| it.median))
            .collect();
        Series {
            fingerprint_id: sorted
                .first()
                .map(|it| it.fingerprint_id.clone())
                .unwrap_or_default(),
            protocol: sorted
                .first()
                .map(|it| it.protocol.clone())
                .unwrap_or_default(),
            between: crate::models::health::Spread::of(&medians),
            calibrations,
        }
    }

    /// Every calibration held together internally, by the same threshold `judge` uses.
    pub fn each_is_one_configuration(&self) -> bool {
        !self.calibrations.is_empty()
            && self
                .calibrations
                .iter()
                .all(|it| it.within.is_some_and(|it| it.one_configuration()))
    }

    /// What this series can say **without a threshold nobody has chosen yet**.
    ///
    /// ## The threshold that is not being reused
    ///
    /// `ONE_CONFIGURATION` is 5% and it was derived for **within-run** spread: eight healthy runs
    /// of one instance spanned 1.7%, so runs disagreeing by more than five are being sampled from
    /// two states. That is a fact about one loaded model answering repeatedly.
    ///
    /// Between-process variability is a different quantity and nothing has measured what is
    /// normal for it. Reusing 5% would have called a 4.79% gap between two calibrations
    /// *agreement* — a verdict resting entirely on a number borrowed from another question. The
    /// owner's instruction was explicit: **look at the distribution before inventing the
    /// formula.**
    ///
    /// So this reports two things it can stand behind and withholds the third:
    ///
    /// | | |
    /// |---|---|
    /// | `TooFew` | fewer than two calibrations |
    /// | `UnstableWithin` | a single process could not hold still — its own threshold, its own quantity |
    /// | `Measured` | every process was steady; the between-process figures are reported and **not judged** |
    ///
    /// `may_mint` is `false` for all three. A reference is minted when a person looks at the
    /// series and says so, and the algorithm for its band is chosen then.
    pub fn reading(&self) -> SeriesReading {
        if self.calibrations.len() < 2 {
            return SeriesReading::TooFew;
        }
        if !self.each_is_one_configuration() {
            return SeriesReading::UnstableWithin;
        }
        SeriesReading::Measured
    }

    /// The two levels, spelled out. **Descriptive throughout; no line here is a verdict.**
    pub fn describe(&self) -> Vec<String> {
        let mut said = vec![format!("Series: {}", self.reading().label())];
        for one in &self.calibrations {
            let rates = one
                .rates
                .iter()
                .map(|it| format!("{it:.2}"))
                .collect::<Vec<_>>()
                .join(", ");
            match one.within {
                Some(w) => said.push(format!(
                    "  {} · median {:.2} · observed range {:.2}% · max deviation {:.2}% \
                     · MAD {:.2}% · [{rates}]",
                    one.id, w.median, w.observed_range_pct, w.max_deviation_pct, w.mad_pct,
                )),
                None => said.push(format!("  {} · no reading", one.id)),
            }
            if !one.warmup_rates.is_empty() {
                said.push(format!(
                    "      warmup (discarded, kept as evidence): {}",
                    one.warmup_rates
                        .iter()
                        .map(|it| format!("{it:.2}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                ));
            }
        }
        match self.between {
            Some(b) => {
                said.push(format!(
                    "Between processes ({} medians): median of medians {:.2} · observed range \
                     {:.2} tok/s ({:.2}%) · max deviation {:.2}% · MAD {:.3} ({:.2}%)",
                    b.runs,
                    b.median,
                    b.observed_range_tps,
                    b.observed_range_pct,
                    b.max_deviation_pct,
                    b.mad_tps,
                    b.mad_pct,
                ));
                said.push(
                    "No threshold is applied to the between-process figures: the 5% rule is about \
                     within-run spread, and what is normal between fresh loads has not been \
                     measured."
                        .to_owned(),
                );
            }
            None => said.push("Between processes: nothing to compare".to_owned()),
        }
        said
    }
}

/// What a series of calibrations says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SeriesReading {
    /// Fewer than two calibrations: nothing to compare.
    TooFew,
    /// A single process could not hold still. The problem is below process lifecycle, and
    /// investigating what differs *between* loads would be looking in the wrong place.
    UnstableWithin,
    /// Every process was steady. The between-process figures are reported and **not judged** —
    /// see `Series::reading` for why no threshold is applied to them yet.
    Measured,
}

impl SeriesReading {
    pub fn label(self) -> &'static str {
        match self {
            SeriesReading::TooFew => "TOO FEW CALIBRATIONS",
            SeriesReading::UnstableWithin => "UNSTABLE WITHIN A SINGLE PROCESS",
            SeriesReading::Measured => {
                "EACH PROCESS STEADY — BETWEEN-PROCESS FIGURES REPORTED, NOT JUDGED"
            }
        }
    }

    /// Whether a series is in a shape a policy could build a reference from.
    ///
    /// ## This was `false` for a day, deliberately
    ///
    /// It returned `false` for every reading, because the algorithm for a band had to be chosen
    /// **from** a distribution rather than before one existed. That was right while nothing was
    /// named; `REPRODUCTION_POLICY_V1` now exists, is versioned, and states its own floor and its
    /// own width, so minting is no longer an invention.
    ///
    /// **What it still refuses is a series that cannot support one.** A single calibration is not
    /// a series, and a process that could not hold still is a problem below the level a band
    /// describes — neither is fixed by a formula.
    pub fn may_mint(self) -> bool {
        self == SeriesReading::Measured
    }
}

/// How a reproducibility band is built from a series of calibrations.
///
/// ## Versioned, because 3% is not a law
///
/// The owner's instruction, 2026-09-01, and it is the same discipline the protocol id already
/// follows: a constant that decides whether hours of work may be trusted must be **named and
/// dated**, not embedded. This one was chosen from three calibrations. When Epoch has dozens, the
/// band should come from that evidence instead, and the record of every reference will say which
/// policy built it.
///
/// ## What it does
///
/// | | |
/// |---|---|
/// | **center** | median of the calibration medians — one number per process, never the pooled runs |
/// | **tolerance** | `max(3% of center, 6 × between-process MAD)` |
///
/// Six MADs is the same robust width `Tolerance` uses elsewhere. The 3% floor is what stops a
/// suspiciously tidy series from minting a band so narrow that ordinary variation reads as a
/// fault — and on the series that produced it, the floor is what wins: 3% of 51.60 is 1.548
/// against 6 × 0.214 = 1.284.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReproductionPolicy {
    pub name: &'static str,
    /// The least tolerance, as a share of the centre.
    pub floor: f64,
    /// How many between-process MADs still count as the same state.
    pub mads: f64,
    /// How many fresh calibrations a reference is built from.
    ///
    /// **Three, because one cannot answer the question it is asking.** Whether a fresh process
    /// reproduces a state is a property of a *series*; a single calibration describes one load.
    /// Three is what was run by hand to validate the instrument, and it is the smallest number
    /// where "these agree" means anything.
    pub calibrations: usize,
}

/// The first policy. Chosen from three calibrations and expected to be replaced by evidence.
pub const REPRODUCTION_POLICY_V1: ReproductionPolicy = ReproductionPolicy {
    name: "REPRODUCTION_POLICY_V1",
    floor: 0.03,
    mads: 6.0,
    calibrations: 3,
};

impl ReproductionPolicy {
    /// The band, and which half of the `max` decided it.
    pub fn band(&self, center: f64, between_mad: f64) -> Option<(Tolerance, &'static str)> {
        if center <= 0.0 {
            return None;
        }
        let from_mad = self.mads * between_mad;
        let from_floor = self.floor * center;
        let (allowed, whence) = if from_mad > from_floor {
            (from_mad, "six between-process MADs")
        } else {
            (from_floor, "the policy floor")
        };
        Some((
            Tolerance {
                middle: center,
                mad: between_mad,
                allowed,
            },
            whence,
        ))
    }
}

impl Series {
    /// Build a reference from this series, or say why not.
    ///
    /// **The authority is the series, never the newest calibration.** A band derived from one run
    /// describes that run; a band derived from three fresh processes describes what a fresh
    /// process does — which is the question a later control is actually asking.
    ///
    /// Every source observation has to be valid and every fingerprint has to agree: a series is
    /// only a series if its members are the same experiment.
    pub fn mint(
        &self,
        id: &str,
        observations: &[Observation],
        policy: &ReproductionPolicy,
        wanted: usize,
    ) -> Result<PerformanceReference, NotAReference> {
        if observations.len() < wanted {
            return Err(NotAReference::TooFewRuns {
                got: observations.len(),
                wanted,
            });
        }
        for one in observations {
            // Each has to be a true record of the protocol it names, and to hold together.
            let _ = one.validate("probe", one.rates.len())?;
        }
        let Some(between) = self.between else {
            return Err(NotAReference::NoReading);
        };
        let first = &observations[0];
        let Some((band, _)) = policy.band(between.median, between.mad_tps) else {
            return Err(NotAReference::NoReading);
        };
        Ok(PerformanceReference {
            id: id.to_owned(),
            fingerprint_id: String::new(),
            fingerprint: first.fingerprint.clone(),
            median: between.median,
            dispersion: Some(band),
            runs: Some(observations.iter().map(|it| it.rates.len()).sum()),
            // Every rate from every calibration, in order. The evidence, not the summary.
            rates: observations
                .iter()
                .flat_map(|it| it.rates.clone())
                .collect(),
            observed: first.observed.clone(),
            sources: observations.iter().map(|it| it.id.clone()).collect(),
            policy: Some(policy.name.to_owned()),
            between: Some(between),
            at: crate::now_ms(),
            legacy: false,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
        }
        .keyed())
    }
}

/// Whether a reference still describes this machine.
///
/// ## Correct about its sources and no longer adequate as a gate
///
/// The distinction the measurements of 2026-09-01 forced. `R-001` was built from three
/// calibrations taken over twenty minutes: 51.81, 51.60, 51.19, agreeing to within 1.2%, with a
/// band of 50.05–53.15. Everything about it was right.
///
/// Two hours later, controls of the identical experiment read **48.04** and then **42.75** — a
/// 17.5% fall across five measurements, monotonic, with no other reading changing the way a
/// degraded instance changes them. The gate did its job perfectly each time.
///
/// **The reference did not become wrong; it became unrepresentative.** It is a true summary of
/// what those three processes did and a poor description of what this machine does over hours.
/// Those are different failures and only one of them is fixed by re-measuring.
///
/// The temptation is to widen the band until the new readings fit. That would destroy the only
/// instrument that detected the thing worth knowing:
///
/// > **Never widen a band to admit the evidence that contradicts it.** A gate that is adjusted
/// > until it passes is not a gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Standing {
    /// It governs.
    #[default]
    Current,
    /// Valid for its sources, and later comparable controls have fallen outside it.
    ///
    /// **Its numbers are never edited.** The record of how it was built stays exactly as it was;
    /// what changes is whether it may decide anything.
    TemporalCoverageInsufficient,
    /// Something in the fingerprint changed under it — a driver, a build, a backend.
    Superseded,
}

impl Standing {
    pub fn label(self) -> &'static str {
        match self {
            Standing::Current => "CURRENT",
            Standing::TemporalCoverageInsufficient => {
                "UNREPRESENTATIVE \u{2014} TEMPORAL COVERAGE INSUFFICIENT"
            }
            Standing::Superseded => "SUPERSEDED",
        }
    }

    /// Whether a reference in this standing may act as a gate.
    pub fn governs(self) -> bool {
        self == Standing::Current
    }
}

/// The three properties a benchmark can have, and they are not the same property.
///
/// Measured on one machine in one afternoon, which is why they had to be separated:
///
/// | | what it asks | what was seen |
/// |---|---|---|
/// | **within-process** | is this instance steady while it runs? | 1.0–4.4% range, steady |
/// | **between-process** | does a fresh load reproduce the last one? | 1.2% across three, in twenty minutes |
/// | **temporal** | does it still, an hour later? | 17.6% across five, over two hours |
///
/// A machine can be excellent at the first two and fail the third, and a reference built only
/// from the second will be confidently wrong about the machine.
///
/// **Nothing here computes a verdict yet.** The domain is being kept from mixing them; the
/// policy for temporal stability comes from evidence Epoch has not yet accumulated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    /// Nobody has measured this property.
    #[default]
    Unknown,
    Low,
    High,
}

/// Where a tolerance came from, so a provisional guard is never read as a measured distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Band {
    /// Six MADs of the runs that produced the reference.
    Measured,
    /// The 3% floor, because the runs behind the reference were not kept.
    Fallback,
}

impl Band {
    pub fn about(self) -> &'static str {
        match self {
            Band::Measured => "derived from the reference's own runs",
            Band::Fallback => {
                "the conservative 3% fallback \u{2014} the original spread was not kept"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> Fingerprint {
        Fingerprint {
            logical_model: Some("Qwen3.6-35B-A3B".into()),
            artifact: Some("Qwen3.6-35B-A3B-UD-IQ4_XS".into()),
            artifact_bytes: Some(17_730_509_792),
            quant: Some("IQ4_XS".into()),
            mtp: Some(false),
            runtime: Some("llama_cpp".into()),
            build: Some("0.3.0-dev (build 10622, commit 3737e4137)".into()),
            backend: Some("CUDA".into()),
            gpu: Some("NVIDIA GeForce RTX 4070 SUPER".into()),
            driver: Some("581.29".into()),
            os: Some("Windows 11".into()),
            context: Some(32_768),
            cache: Some("f16".into()),
            flash_attention: Some("auto".into()),
            speculation: Some("off".into()),
            offload: None,
            gpu_layers: None,
            batch: None,
            ubatch: None,
            control: Some("STANDARD PERFORMANCE CONTROL".into()),
            measurement_protocol: Some(crate::protocol::STANDARD_CONTROL_V2.id()),
            workload: Some("3 prompts × 256 tokens".into()),
            benchmark_version: Some(1),
            sampler: Some("greedy".into()),
        }
    }

    #[test]
    fn the_same_measurement_twice_is_comparable() {
        assert_eq!(full().comparable_to(&full()), Comparison::Comparable);
        assert!(full().complete());
    }

    #[test]
    fn a_different_build_is_a_different_measurement_and_says_which_field() {
        /*
            One build carries one GPU backend: the winget llama.cpp is Vulkan-only and the one in
            `~/.llama/bin` is CUDA, measured 4.5x apart on this card. Comparing across them and
            calling the difference a degraded machine would send somebody hunting a fault that is
            a package.
        */
        let theirs = Fingerprint {
            build: Some("0.1.2-dev (build 10507, commit 95c409c13)".into()),
            ..full()
        };
        let got = full().comparable_to(&theirs);
        assert!(matches!(got, Comparison::Differs(_)), "{got:?}");
        assert!(got.why().contains("build"), "{}", got.why());
    }

    #[test]
    fn a_different_context_or_cache_is_not_the_same_number() {
        for changed in [
            Fingerprint {
                context: Some(16_384),
                ..full()
            },
            Fingerprint {
                cache: Some("q8_0".into()),
                ..full()
            },
            Fingerprint {
                flash_attention: Some("off".into()),
                ..full()
            },
        ] {
            assert!(
                matches!(full().comparable_to(&changed), Comparison::Differs(_)),
                "{changed:?}",
            );
        }
    }

    #[test]
    fn silence_about_a_field_is_never_evidence_that_it_matched() {
        /*
            **The whole reason this file exists.** The historical figure on this machine is `46.0`
            and a timestamp — no artefact, no build, no context, no cache, no workload. Treating
            that as *the same as whatever we are running now* is the invented gauge wearing a
            verdict, at the moment somebody decides whether to trust hours of measurement.
        */
        let anecdote = Fingerprint::default();
        let got = anecdote.comparable_to(&full());
        assert!(matches!(got, Comparison::Incomplete(_)), "{got:?}");
        assert!(!got.yes());
        for named in ["artifact", "build", "context", "cache", "workload"] {
            assert!(got.why().contains(named), "{} lacks {named}", got.why());
        }
    }

    #[test]
    fn a_definite_difference_is_reported_ahead_of_a_missing_field() {
        // Two answers are available and only one of them is certain. "The build changed" is
        // actionable; "and also nobody recorded the sampler" is noise beside it.
        let half = Fingerprint {
            build: Some("another build".into()),
            workload: None,
            ..full()
        };
        assert!(matches!(
            full().comparable_to(&half),
            Comparison::Differs(_)
        ));
    }

    #[test]
    fn an_unrecorded_field_outside_the_required_set_does_not_block_a_comparison() {
        // A reference taken before Epoch read the driver is not useless. It is a reference with
        // one fewer guarantee, and saying so is worth more than refusing it.
        let older = Fingerprint {
            driver: None,
            os: None,
            sampler: None,
            ..full()
        };
        assert_eq!(older.comparable_to(&full()), Comparison::Comparable);
    }

    #[test]
    fn a_reference_governs_the_experiment_it_was_taken_for_and_no_other() {
        /*
            **The owner's requirement, 2026-08-31.** A figure established for the 32K standard
            control must not later find itself gating a 64K run, an MTP artefact, an IQ2
            quantisation, another build, another backend or another driver.

            Each of those already differs on a field, so `control` is belt *and* braces — and the
            braces are the half that survives somebody adding a field later and forgetting to
            require it. A named purpose cannot be accidentally satisfied; a set of coincidentally
            equal fields can.
        */
        let standard = full();
        for (what, other) in [
            (
                "64K",
                Fingerprint {
                    context: Some(65_536),
                    ..full()
                },
            ),
            (
                "MTP",
                Fingerprint {
                    artifact: Some("Qwen3.6-35B-A3B-UD-IQ4_XS · MTP".into()),
                    artifact_bytes: Some(18_209_036_576),
                    mtp: Some(true),
                    ..full()
                },
            ),
            (
                "IQ2",
                Fingerprint {
                    artifact: Some("Qwen3.6-35B-A3B-UD-IQ2_M".into()),
                    artifact_bytes: Some(11_900_000_000),
                    quant: Some("IQ2_M".into()),
                    ..full()
                },
            ),
            (
                "another build",
                Fingerprint {
                    build: Some("b2".into()),
                    ..full()
                },
            ),
            (
                "another backend",
                Fingerprint {
                    backend: Some("Vulkan".into()),
                    ..full()
                },
            ),
            (
                "another driver",
                Fingerprint {
                    driver: Some("999.99".into()),
                    ..full()
                },
            ),
            (
                "compressed cache",
                Fingerprint {
                    cache: Some("q8_0".into()),
                    ..full()
                },
            ),
            (
                "a different experiment entirely",
                Fingerprint {
                    control: Some("CONTEXT CAPABILITY LADDER".into()),
                    ..full()
                },
            ),
        ] {
            let got = standard.comparable_to(&other);
            assert!(
                matches!(got, Comparison::Differs(_)),
                "{what} was allowed to reuse the standard reference: {got:?}",
            );
        }

        // And the one it does govern.
        assert_eq!(standard.comparable_to(&full()), Comparison::Comparable);
    }

    #[test]
    fn a_legacy_figure_is_kept_and_may_not_be_a_gate() {
        let legacy = PerformanceReference {
            id: "legacy-46".into(),
            fingerprint_id: String::new(),
            fingerprint: Fingerprint::default(),
            median: 46.0,
            dispersion: None,
            runs: None,
            rates: Vec::new(),
            observed: Default::default(),
            at: 1,
            legacy: true,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
            sources: Vec::new(),
            policy: None,
            between: None,
        };
        assert!(!legacy.is_safety_reference());
        assert!(legacy.describe().contains("46.0"));
        assert!(
            legacy.describe().contains("LEGACY"),
            "{}",
            legacy.describe()
        );

        // A complete, measured one is.
        let proper = PerformanceReference {
            id: "now".into(),
            fingerprint_id: String::new(),
            fingerprint: full(),
            legacy: false,
            ..legacy
        };
        assert!(proper.is_safety_reference());
    }

    #[test]
    fn reproduction_is_equivalence_and_faster_is_not_a_pass() {
        /*
            **Live on this machine, 2026-09-01.** Reference 39.7, control 46.7. One-sided that is
            a pass and the pause gets cleared; two-sided it is a different state, and comparing
            twenty configurations against it would be comparing them to something that is not
            happening any more.

            Both gates are correct about their own question. `Tolerance::reproduces` asks *are we
            worse*, which is right for a sentinel mid-search. A clean-state check asks *is this
            the same state*, and only the two-sided answer answers it.
        */
        let it = PerformanceReference {
            id: "r".into(),
            fingerprint_id: String::new(),
            fingerprint: full(),
            median: 39.7,
            dispersion: Tolerance::of(&[39.68, 39.32, 40.50]),
            runs: Some(3),
            rates: vec![39.68, 39.32, 40.50],
            observed: Observed::default(),
            at: 1,
            legacy: false,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
            sources: Vec::new(),
            policy: None,
            between: None,
        };
        let (band, _) = it.band().expect("a band");

        // The one-sided gate says yes, and it is not wrong about its own question.
        assert!(band.reproduces(46.7));
        // The two-sided one says what actually happened.
        let got = it.reproduced_by(46.7, &full());
        assert_eq!(got, Reproduction::AboveReference);
        assert!(!got.yes());
        assert!(
            !got.degraded(),
            "faster is not evidence of damage, and calling it that sends somebody hunting a \
             fault nobody has shown exists",
        );
        assert!(got.label().contains("ABOVE"));

        // Inside the band, both sides.
        assert_eq!(it.reproduced_by(39.7, &full()), Reproduction::Reproduced);
        assert_eq!(it.reproduced_by(41.0, &full()), Reproduction::Reproduced);
        // And the case the whole subsystem was built for.
        let low = it.reproduced_by(28.0, &full());
        assert_eq!(low, Reproduction::BelowReference);
        assert!(low.degraded());
    }

    #[test]
    fn a_reference_about_something_else_never_becomes_a_reproduction_verdict() {
        let it = PerformanceReference {
            id: "r".into(),
            fingerprint_id: String::new(),
            fingerprint: full(),
            median: 39.7,
            dispersion: Tolerance::of(&[39.68, 39.32, 40.50]),
            runs: Some(3),
            rates: vec![],
            observed: Observed::default(),
            at: 1,
            legacy: false,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
            sources: Vec::new(),
            policy: None,
            between: None,
        };
        let elsewhere = Fingerprint {
            build: Some("another build".into()),
            ..full()
        };
        let got = it.reproduced_by(39.7, &elsewhere);
        assert!(matches!(got, Reproduction::Incomparable(_)), "{got:?}");
        assert!(!got.yes() && !got.degraded());

        // And a legacy figure decides nothing, in either direction.
        let anecdote = PerformanceReference {
            fingerprint: Fingerprint::default(),
            legacy: true,
            ..it
        };
        assert_eq!(
            anecdote.reproduced_by(28.0, &full()),
            Reproduction::Incomplete
        );
    }

    fn seen(rates: Vec<f64>) -> Observation {
        Observation {
            id: String::new(),
            fingerprint_id: String::new(),
            fingerprint: full(),
            protocol: crate::protocol::STANDARD_CONTROL_V2.id(),
            source: "calibration".into(),
            sequence: crate::protocol::Sequence::default(),
            rates,
            observed: Observed::default(),
            at: 1,
        }
    }

    fn calibration(id: &str, at: u64, rates: Vec<f64>) -> Observation {
        Observation {
            id: String::new(),
            fingerprint_id: String::new(),
            fingerprint: full(),
            protocol: crate::protocol::STANDARD_CONTROL_V2.id(),
            source: "calibration".into(),
            sequence: crate::protocol::Sequence::default(),
            rates,
            observed: Observed::default(),
            at,
        }
        .named(id)
    }

    #[test]
    fn fifteen_runs_from_three_processes_are_not_fifteen_runs() {
        /*
            **The owner's rule, 2026-09-01, and it is about the experiment rather than about
            statistics.** Five runs inside one process share a model load, a memory layout and
            whatever the driver did that minute. Pooling them reports the tight within-process
            agreement as if it described the machine.

            These are Calibration A and B as they actually happened, padded to five: each holds
            inside ~3%, and they land 5.6% apart. Pooled that is one wide shrug. Kept apart it is
            *each load is tight and the loads disagree*, which is the actionable finding.
        */
        let a = calibration("O-001", 100, vec![39.68, 39.32, 40.50, 39.90, 40.10]);
        let b = calibration("O-002", 200, vec![41.44, 41.90, 42.30, 41.70, 42.00]);
        let series = Series::of(&[b.clone(), a.clone()]);

        assert_eq!(series.calibrations.len(), 2);
        assert_eq!(series.calibrations[0].id, "O-001", "oldest first");

        // Within: each process is steady.
        assert!(series.each_is_one_configuration());
        for one in &series.calibrations {
            assert!(one.within.expect("within").observed_range_pct < 5.0);
        }

        // Between: the medians are the sample, and they disagree.
        let between = series.between.expect("between");
        assert_eq!(between.runs, 2, "one number per process, not fifteen");
        assert!(
            between.observed_range_pct > 4.0,
            "{}",
            between.observed_range_pct
        );

        /*
            **And no verdict is offered about that 4.79%.** The 5% threshold was derived for
            within-run spread — eight healthy runs of one instance spanning 1.7% — and what is
            normal between fresh loads has never been measured here. Reusing the number would
            have called these two calibrations "agreement" on the strength of a constant borrowed
            from a different question.
        */
        assert_eq!(series.reading(), SeriesReading::Measured);
        /*
            **A shape a policy can work with, which is not the same as a good machine.** These
            two calibrations disagree by 4.79% and `REPRODUCTION_POLICY_V1` would happily build a
            band around them — the policy describes how wide the same state is, not whether the
            state is worth having. What refuses a series is a process that could not hold still,
            or too few of them.
        */
        assert!(series.reading().may_mint());
        let said = series.describe().join("\n");
        assert!(said.contains("has not been measured"), "{said}");
        assert!(said.contains("O-001") && said.contains("O-002"));
        assert!(said.contains("observed range"), "{said}");
    }

    #[test]
    fn three_agreeing_calibrations_are_a_baseline_and_one_is_not_a_series() {
        let series = Series::of(&[
            calibration("O-001", 100, vec![46.0, 46.1, 45.9, 46.2, 45.8]),
            calibration("O-002", 200, vec![45.8, 45.9, 46.0, 45.7, 46.1]),
            calibration("O-003", 300, vec![46.2, 46.0, 46.3, 46.1, 45.9]),
        ]);
        assert_eq!(series.reading(), SeriesReading::Measured);
        assert!(series.reading().may_mint());
        assert_eq!(series.between.expect("between").runs, 3);

        // One calibration is not a series, however tidy it is. Nothing to compare it against.
        let alone = Series::of(&[calibration(
            "O-001",
            100,
            vec![46.0, 46.1, 45.9, 46.2, 45.8],
        )]);
        assert_eq!(alone.reading(), SeriesReading::TooFew);
        assert!(
            !alone.reading().may_mint(),
            "one calibration is not a series"
        );
    }

    #[test]
    fn a_process_that_cannot_hold_still_is_a_different_problem_from_two_that_disagree() {
        // The problem is below process lifecycle, so *investigate the load* is the wrong next
        // step and the reading says so rather than reporting a wide band.
        let series = Series::of(&[
            calibration("O-001", 100, vec![39.0, 46.0, 40.0, 45.0, 41.0]),
            calibration("O-002", 200, vec![39.5, 45.5, 40.5, 44.5, 41.5]),
        ]);
        assert_eq!(series.reading(), SeriesReading::UnstableWithin);
        // And the medians happen to sit close together, which is exactly why the two levels are
        // separate: agreement between processes means nothing when a single process is not one
        // state, and a series that only reported the between figures would look healthy.
        let between = series.between.expect("between");
        assert!(
            between.observed_range_pct < 5.0,
            "{}",
            between.observed_range_pct
        );
    }

    #[test]
    fn asking_for_a_protocol_is_not_the_same_as_the_protocol_having_happened() {
        /*
            **Measured 2026-09-01, by running it.** `STANDARD_CONTROL_V2` declared
            `warmup=1;runs=5`, that id travelled on the fingerprint as a description of the run,
            and what happened was 1+3 and then 1+3 — `bench::measure` had a private run count and
            a warm-up of its own, so the caller's numbers were silently replaced.

            A record whose description does not match its content is worse than no record: it is
            evidence that will be trusted. So the protocol declares, the observation counts, and
            validation compares — and a mismatch can never become a reference.
        */
        let lying = Observation {
            sequence: crate::protocol::Sequence {
                execution: crate::protocol::Execution {
                    warmups_expected: 1,
                    warmups_observed: 3,
                    runs_expected: 5,
                    runs_observed: 3,
                },
                ..Default::default()
            },
            // Five perfectly agreeable rates. Every other check below would have passed it.
            ..seen(vec![52.04, 51.29, 52.10, 51.90, 52.00])
        };
        assert_eq!(lying.status(), ObservationStatus::InvalidProtocolExecution);
        assert!(!lying.status().usable());

        let got = lying.validate("R-001", 5);
        assert!(
            matches!(got, Err(NotAReference::InvalidProtocolExecution(_))),
            "the run that produced this had five usable rates and still may not govern: {got:?}",
        );
        let why = got.unwrap_err().why();
        assert!(why.contains("1 warmup + 5 measured were"), "{why}");
        assert!(why.contains("3 + 3"), "{why}");

        // And the honest version of the same run.
        let honest = Observation {
            sequence: crate::protocol::Sequence {
                execution: crate::protocol::Execution {
                    warmups_expected: 1,
                    warmups_observed: 1,
                    runs_expected: 5,
                    runs_observed: 5,
                },
                ..Default::default()
            },
            ..seen(vec![52.04, 51.29, 52.10, 51.90, 52.00])
        };
        assert_eq!(honest.status(), ObservationStatus::Valid);
        assert!(honest.validate("R-001", 5).is_ok());
        assert!(honest
            .sequence
            .execution
            .describe()
            .join(" ")
            .contains("MATCH"));
    }

    #[test]
    fn more_runs_than_asked_for_is_also_not_the_protocol() {
        // Fewer is a truncated measurement; more is an instrument doing something nobody
        // described. Neither is what the id claims.
        let extra = crate::protocol::Execution {
            warmups_expected: 1,
            warmups_observed: 1,
            runs_expected: 5,
            runs_observed: 6,
        };
        assert!(!extra.matched());
        assert_eq!(extra.label(), "MISMATCH");
    }

    #[test]
    fn a_reference_is_built_from_the_series_and_not_from_the_newest_run() {
        /*
            **The three real calibrations, 2026-09-01.** C1 51.81, C2 51.60, C3 51.19 — three
            fresh processes, three distinct router PIDs, three protocol executions MATCH, five
            measured runs each.

            The authority is the series. A band derived from one run describes that run; a band
            derived from three fresh processes describes what a fresh process does, which is the
            question a later control is actually asking.
        */
        let ones = vec![
            calibration("O-001", 100, vec![51.86, 51.37, 51.88, 51.81, 51.78]),
            calibration("O-002", 200, vec![51.80, 51.60, 51.08, 51.83, 51.24]),
            calibration("O-003", 300, vec![51.19, 50.51, 50.61, 51.21, 51.45]),
        ];
        let series = Series::of(&ones);
        let one = series
            .mint("R-001", &ones, &REPRODUCTION_POLICY_V1, 3)
            .expect("three valid calibrations");

        assert_eq!(one.id, "R-001");
        assert_eq!(one.sources, vec!["O-001", "O-002", "O-003"]);
        assert_eq!(one.policy.as_deref(), Some("REPRODUCTION_POLICY_V1"));
        assert_eq!(
            one.runs,
            Some(15),
            "every rate is kept, not just the summary"
        );
        assert_eq!(one.rates.len(), 15);

        // Centre: the median of the medians. One number per process, never the pooled runs.
        assert!((one.median - 51.60).abs() < 0.01, "{}", one.median);

        // Band: the floor wins here — 3% of 51.60 is 1.548 against 6 x 0.214 = 1.284.
        let (band, whence) = one.band().expect("a band");
        assert!((band.allowed - 1.548).abs() < 0.02, "{}", band.allowed);
        assert_eq!(whence, Band::Measured, "derived, not the fallback");

        // Two-sided, and faster is still not a pass.
        for (rate, expected) in [
            (51.81, Reproduction::Reproduced),
            (52.40, Reproduction::Reproduced),
            (50.10, Reproduction::Reproduced),
            (46.70, Reproduction::BelowReference),
            (54.00, Reproduction::AboveReference),
        ] {
            assert_eq!(
                one.reproduced_by(rate, &full()),
                expected,
                "{rate} against 51.60 +/- 1.55",
            );
        }
    }

    #[test]
    fn a_policy_says_which_half_of_the_max_decided_the_band() {
        // 3% is not a law. It is `REPRODUCTION_POLICY_V1`, chosen from three calibrations and
        // expected to be replaced by evidence — so which half won is part of the record.
        let (_, whence) = REPRODUCTION_POLICY_V1.band(51.60, 0.214).expect("a band");
        assert_eq!(whence, "the policy floor");

        // A noisier series, where the measured dispersion is what decides.
        let (band, whence) = REPRODUCTION_POLICY_V1.band(51.60, 1.0).expect("a band");
        assert_eq!(whence, "six between-process MADs");
        assert!((band.allowed - 6.0).abs() < 1e-9);

        assert_eq!(REPRODUCTION_POLICY_V1.name, "REPRODUCTION_POLICY_V1");
    }

    #[test]
    fn a_series_with_one_invalid_member_mints_nothing() {
        // A series is only a series if its members are the same experiment, honestly recorded.
        let mut ones = vec![
            calibration("O-001", 100, vec![51.86, 51.37, 51.88, 51.81, 51.78]),
            calibration("O-002", 200, vec![51.80, 51.60, 51.08, 51.83, 51.24]),
        ];
        ones.push(Observation {
            sequence: crate::protocol::Sequence {
                execution: crate::protocol::Execution {
                    warmups_expected: 1,
                    warmups_observed: 3,
                    runs_expected: 5,
                    runs_observed: 3,
                },
                ..Default::default()
            },
            ..calibration("O-003", 300, vec![51.19, 50.51, 50.61])
        });
        let series = Series::of(&ones);
        let got = series.mint("R-001", &ones, &REPRODUCTION_POLICY_V1, 3);
        assert!(
            matches!(got, Err(NotAReference::InvalidProtocolExecution(_))),
            "{got:?}",
        );

        // And two calibrations are not three.
        let two: Vec<_> = ones[..2].to_vec();
        let got = Series::of(&two).mint("R-001", &two, &REPRODUCTION_POLICY_V1, 3);
        assert!(matches!(
            got,
            Err(NotAReference::TooFewRuns { got: 2, wanted: 3 })
        ));
    }

    #[test]
    fn a_reference_can_be_right_about_its_sources_and_stop_describing_the_machine() {
        /*
            **Measured 2026-09-01.** `R-001` was built from three calibrations taken over twenty
            minutes: 51.81, 51.60, 51.19, agreeing to within 1.2%, band 50.05–53.15. Everything
            about it was correct.

            Over the next two hours, controls of the identical experiment read 48.04, then 42.75,
            then 38.58 — a 25% fall, monotonic, and ten minutes of idle did not interrupt it. The
            gate reported `BelowReference` every time and was right every time.

            The reference did not become wrong. It became **unrepresentative**, which is a
            different failure and is not fixed by widening the band — widening it would have
            destroyed the only instrument that detected the thing worth knowing.
        */
        let mut it = PerformanceReference {
            id: "R-001".into(),
            fingerprint_id: String::new(),
            fingerprint: full(),
            median: 51.60,
            dispersion: Tolerance::of(&[51.81, 51.60, 51.19]),
            runs: Some(15),
            rates: vec![],
            observed: Observed::default(),
            sources: vec!["O-001".into(), "O-002".into(), "O-003".into()],
            policy: Some(REPRODUCTION_POLICY_V1.name.to_owned()),
            between: None,
            at: 1,
            legacy: false,
            standing: Standing::Current,
            superseded_by: None,
            short_term: Confidence::High,
            temporal: Confidence::Unknown,
        };
        assert!(
            it.is_safety_reference(),
            "it governed, correctly, for a while"
        );
        let before = it.median;

        it.standing = Standing::TemporalCoverageInsufficient;

        // **The numbers are untouched.** What changed is whether it may decide anything.
        assert_eq!(it.median, before);
        assert_eq!(it.sources.len(), 3);
        assert_eq!(it.dispersion, Tolerance::of(&[51.81, 51.60, 51.19]));
        assert!(!it.is_safety_reference(), "and it no longer gates");
        assert_eq!(it.reproduced_by(38.58, &full()), Reproduction::Incomplete);
        assert!(it.standing.label().contains("UNREPRESENTATIVE"));
    }

    #[test]
    fn the_three_stabilities_are_three_properties() {
        /*
            One machine, one afternoon: within-process 1.0–5.2%, between-process 1.2% across three
            calibrations in twenty minutes, and 25% across six over two hours. A machine can be
            excellent at the first two and fail the third, and a reference built only from the
            second is confidently wrong about the machine.
        */
        assert_eq!(Confidence::default(), Confidence::Unknown);
        // Nothing here computes a verdict: the policy for temporal stability comes from evidence
        // Epoch has not accumulated yet. The domain only has to keep them from being one field.
        let it = PerformanceReference {
            short_term: Confidence::High,
            temporal: Confidence::Unknown,
            ..PerformanceReference {
                id: "R".into(),
                fingerprint_id: String::new(),
                fingerprint: full(),
                median: 51.60,
                dispersion: None,
                runs: None,
                rates: vec![],
                observed: Observed::default(),
                sources: vec![],
                policy: None,
                between: None,
                at: 1,
                legacy: false,
                standing: Standing::Current,
                superseded_by: None,
                short_term: Confidence::Unknown,
                temporal: Confidence::Unknown,
            }
        };
        assert_ne!(it.short_term, it.temporal);
    }

    #[test]
    fn a_run_does_not_become_an_authority_merely_by_having_happened() {
        /*
            The owner's separation, 2026-09-01. A calibration that produced two usable rates
            instead of five is still evidence worth keeping, and it is not a thing to judge hours
            of later work against. Every refusal below keeps the observation; it withholds the
            authority.
        */
        assert_eq!(
            seen(vec![]).validate("R-001", 5),
            Err(NotAReference::NoReading)
        );

        let short = seen(vec![39.7, 39.3]).validate("R-001", 5);
        assert_eq!(short, Err(NotAReference::TooFewRuns { got: 2, wanted: 5 }));
        assert!(short.unwrap_err().why().contains("only 2 of 5"));

        // Calibration A and B pooled: two states, not one configuration.
        let both = seen(vec![39.32, 39.68, 40.50, 41.44, 41.90, 42.30]).validate("R-001", 5);
        assert!(
            matches!(both, Err(NotAReference::NotOneConfiguration { .. })),
            "{both:?}"
        );

        // And a run whose fingerprint could not record what a comparison needs.
        let blind = Observation {
            fingerprint: Fingerprint {
                build: None,
                ..full()
            },
            ..seen(vec![39.7, 39.3, 40.5, 39.9, 40.1])
        };
        let got = blind.validate("R-001", 5);
        assert!(matches!(got, Err(NotAReference::Unrecorded(_))), "{got:?}");
        assert!(got.unwrap_err().why().contains("build"));
    }

    #[test]
    fn a_good_run_becomes_a_reference_that_carries_its_own_evidence() {
        let rates = vec![39.68, 39.32, 40.50, 39.90, 40.10];
        let one = seen(rates.clone())
            .validate("R-001", 5)
            .expect("a reference");
        assert_eq!(one.id, "R-001");
        assert_eq!(one.rates, rates, "the runs behind the median are kept");
        assert_eq!(one.runs, Some(5));
        assert!(one.is_safety_reference());
        // Derived, never typed: the fingerprint's key travels with it.
        assert_eq!(one.fingerprint_id, full().key());
        assert!(!one.fingerprint_id.is_empty());
    }

    #[test]
    fn two_measurements_of_one_experiment_share_a_key_and_keep_their_own_ids() {
        /*
            **The overwrite this exists to prevent.** Calibration A and Calibration B are two real
            observations of one experiment. Keyed on the fingerprint, the second replaced the
            first and the only interesting thing about the pair — that they do not overlap —
            disappeared with it.
        */
        let a = seen(vec![39.68, 39.32, 40.50, 39.90, 40.10])
            .validate("R-001", 5)
            .expect("A");
        let b = seen(vec![41.44, 41.90, 42.30, 41.70, 42.00])
            .validate("R-002", 5)
            .expect("B");

        assert_eq!(a.fingerprint_id, b.fingerprint_id, "one kind of experiment");
        assert_ne!(a.id, b.id, "two measurements");
        assert!((a.median - 39.9).abs() < 0.3 && (b.median - 41.9).abs() < 0.3);
    }

    #[test]
    fn a_band_says_whether_it_was_measured_or_fallen_back_on() {
        /*
            A provisional 3% guard and a distribution of real runs are both usable and they are
            not the same claim. Somebody reading *within tolerance* deserves to know which.
        */
        let bare = PerformanceReference {
            id: "x".into(),
            fingerprint_id: String::new(),
            fingerprint: full(),
            median: 46.0,
            dispersion: None,
            runs: None,
            rates: Vec::new(),
            observed: Default::default(),
            at: 1,
            legacy: false,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
            sources: Vec::new(),
            policy: None,
            between: None,
        };
        let (band, whence) = bare.band().expect("a band");
        assert_eq!(whence, Band::Fallback);
        assert!(whence.about().contains("fallback"));
        assert!(
            (band.allowed - 1.38).abs() < 0.01,
            "3% of 46 is {}",
            band.allowed
        );
        assert!(!band.reproduces(40.1), "13% short is not the same machine");

        let measured = PerformanceReference {
            dispersion: Tolerance::of(&[46.0, 46.2, 45.9, 46.1]),
            runs: Some(4),
            rates: Vec::new(),
            observed: Default::default(),
            ..bare
        };
        assert_eq!(measured.band().expect("a band").1, Band::Measured);
        assert!(measured.describe().contains("over 4 runs"));
    }
}
