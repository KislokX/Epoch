//! Finding out how to run one model well on this machine, by running it.
//!
//! ## What this replaces
//!
//! A row of MODELS used to carry flash attention, a speculation kind, a draft file, a cache type
//! and a context — five controls somebody had to understand before they could use a model. They
//! all still exist and they are all still reachable. What changed is **whose job it is to
//! understand them**: they are Epoch's instruments now, and what the user sees is the answer.
//!
//! > *Simple arriba. Poder absoluto abajo.* — the owner, 2026-08-31.
//!
//! ## Two phases, and the first one never touches the context
//!
//! **Standard** holds the window at [`crate::suite::STANDARD_CONTEXT`] and finds the best
//! configuration at it. **Profile** then looks for the other intents, and only there may the
//! context move.
//!
//! The owner's rule, 2026-08-31, and it is what keeps the model championship honest:
//!
//! > No quiero que el optimizer *gane* bajando contexto de 32K a 4K o 8K.
//!
//! A search allowed to shrink the window would report a bigger number for every model and would
//! be comparing seven different benchmarks. Discovering that 16K is faster is worth knowing and
//! belongs in `FAST`, where somebody has asked for it.
//!
//! ## Baseline first, speculation last
//!
//! Also the owner's, and it is a priority rather than an ordering detail. The MTP measurement
//! found speculation worth **+3.9%** and Epoch's own stale preset costing about **10%** — so a
//! search that reached for speculation early would be using it to compensate for a base
//! configuration nobody had fixed, and would report the compensation as a win.
//!
//! Each stage keeps the best configuration found so far and varies **one** thing against it:
//!
//! 1. **Baseline** — nothing set. What an improvement is measured against, measured rather than
//!    remembered.
//! 2. **Cache** — full precision against compressed. Measured **separately** from the context,
//!    because the two were changed together in the stale preset and *both cost 10%* is not a
//!    finding, it is two unmeasured halves.
//! 3. **Flash attention** — on, off, and llama.cpp's own `auto`, which is a real third state.
//! 4. **Offload** — llama.cpp's own `llama-fit-params`, where it has something to say. Probably
//!    the most important step here: `Qwen3.6-35B-A3B` leaves the card three fifths idle while its
//!    experts sit in host memory.
//! 5. **Speculation** — every kind this build offers that can run here, on the winner of 1–4.
//!
//! That is roughly twenty configurations rather than several hundred, and the assumption it rests
//! on is written down as one: **that the axes are close enough to independent that a greedy walk
//! finds the same answer as the grid.** Nobody has measured that here. What makes it defensible
//! is that every row is kept, so a later grid would be adding rows to this table rather than
//! replacing it.
//!
//! ## Nothing is applied by having been measured
//!
//! The search ends by putting back exactly what the model had. A measurement is Epoch's and the
//! choice is the user's (ADR-0033), and a search that quietly left the last thing it tried in
//! place would be choosing by the order of a loop.

use serde::{Deserialize, Serialize};

use crate::models::loadout::{Cache, Loadout};
use crate::models::tuning::Tuning;
use crate::profiles::Configuration;

/// One thing to try, and how it reads while it is being tried.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    /// What a person sees: `Speculative Decoding · MTP n=2`.
    pub about: String,
    /// The flags, for whoever wants them. Behind *show technical details*.
    pub technical: String,
    pub loadout: Loadout,
    pub tuning: Tuning,
    #[serde(default)]
    pub offload: Option<String>,
    #[serde(default)]
    pub gpu_layers: Option<i32>,
}

/// The four things a benchmark does, in the order it does them.
///
/// **Named for what the user sees, not for the code that runs.** `Establishing baseline` covers
/// a calibration series, a clean-state check, or nothing at all when a reference already governs
/// — three quite different amounts of work that answer one question a person actually has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Stage {
    /// What card, what runtime, what backend, what is holding the GPU.
    #[default]
    CheckingHardware,
    /// Establishing or reproducing the reference every later number is judged against.
    EstablishingBaseline,
    /// The candidates.
    TestingConfigurations,
    /// The closing control, and whether anything measured may become a profile.
    ValidatingStability,
}

impl Stage {
    /// In order, for a checklist that shows what is done and what is coming.
    pub const ALL: [Stage; 4] = [
        Stage::CheckingHardware,
        Stage::EstablishingBaseline,
        Stage::TestingConfigurations,
        Stage::ValidatingStability,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Stage::CheckingHardware => "Checking hardware",
            Stage::EstablishingBaseline => "Establishing baseline",
            Stage::TestingConfigurations => "Testing configurations",
            Stage::ValidatingStability => "Validating stability",
        }
    }

    /// Whether this stage comes before another. What lets a checklist tick the earlier ones.
    pub fn before(self, other: Stage) -> bool {
        let at = |one: Stage| Stage::ALL.iter().position(|it| *it == one).unwrap_or(0);
        at(self) < at(other)
    }
}

/// How a search is going, as it goes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// **Which of the four things Epoch is doing**, so a person is told the work rather than the
    /// instrument.
    ///
    /// Nobody pressing BENCHMARK & OPTIMIZE needs to know what a sentinel is, which reference
    /// governs, or that `O-004` exists. They need to know that the hardware was checked, that a
    /// baseline is being established, that configurations are being tried, and roughly how much
    /// is left. `about` still carries the sentence; this carries which stage it belongs to.
    #[serde(default)]
    pub stage: Stage,
    pub model: String,
    /// The card, in its own words, so the panel can say what it is measuring on.
    pub machine: String,
    pub done: usize,
    pub total: usize,
    /// What is being tried right now, in words.
    pub about: String,
    /// The flags behind those words.
    pub technical: String,
    /// The best configuration so far. `None` until something finishes.
    pub best: Option<Configuration>,
}

impl Progress {
    /// Nought to one hundred. `0` before anything finished rather than a division by zero.
    pub fn percent(&self) -> u32 {
        if self.total == 0 {
            return 0;
        }
        ((self.done as f64 / self.total as f64) * 100.0).round() as u32
    }
}

/// Every context worth trying, from the standard one outwards.
///
/// **The standard one first**, so a search that is stopped early has already answered the
/// question everything else is compared against. Powers of two because that is how a context
/// window is spoken about, and never past what the model was trained for — asking for more buys
/// nothing and costs a load to find out.
pub fn contexts(standard: u32, trained: u32) -> Vec<u32> {
    let mut every = vec![standard.min(trained.max(1))];
    for at in [8_192u32, 16_384, 32_768, 65_536, 131_072] {
        if at <= trained && !every.contains(&at) {
            every.push(at);
        }
    }
    every
}

/// Every window worth asking about, up to what the model was trained for.
///
/// **The rungs people speak in**, and never one above `trained`: asking a model for more context
/// than it was trained to hold buys nothing and costs a load to find out.
///
/// This is a fact about the *model* and it is deliberately not the same list as what this machine
/// can hold. Gemma 4 declares 262,144; whether 256K fits on a 12 GB card is a different question
/// with a different answer, and collapsing the two would lose the one a person actually asks —
/// *how much was this model built for?*
pub fn ladder(trained: u32) -> Vec<u32> {
    [
        16_384u32, 32_768, 65_536, 131_072, 262_144, 524_288, 1_048_576,
    ]
    .into_iter()
    .filter(|it| *it <= trained)
    .collect()
}

/// One rung of the ladder, and whether this machine can hold it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rung {
    pub context: u32,
    /// How the model sits at this window, asked of the fitter before anything is measured.
    pub fit: crate::models::runtimes::FitClass,
}

impl Rung {
    /// Whether it is worth spending a measurement on.
    ///
    /// **`Unknown` is worth spending on.** A machine whose fitter could not be asked is not a
    /// machine that cannot hold the window; refusing there would turn a missing sibling binary
    /// into a shorter ladder, with nothing saying why.
    pub fn worth_measuring(&self) -> bool {
        self.fit != crate::models::runtimes::FitClass::Unloadable
    }
}

/// The ladder for one model on this machine, each rung carrying what the fitter said about it.
///
/// **Asked before measuring, not discovered by failing.** A rung the fitter refuses costs a
/// model load to skip and several minutes to find out the other way — and the answer *256K does
/// not fit here* is worth as much on the screen as a number would be.
///
/// One fit call per rung, about four seconds each on this machine.
pub fn feasible(model: &std::path::Path, trained: u32) -> Vec<Rung> {
    ladder(trained)
        .into_iter()
        .map(|context| Rung {
            context,
            fit: crate::models::runtimes::fitting(model, context).class,
        })
        .collect()
}

/// The name of the control every Standard-phase measurement is taken under.
///
/// **A reference governs the experiment it was taken for and no other.** The owner's rule,
/// 2026-08-31: a figure established here must not later find itself gating a 64K run, an MTP
/// artefact, an IQ2 quantisation, another build or another backend. Each of those already differs
/// on a fingerprint field, so this is belt and braces — and it is the half that survives somebody
/// adding a field later and forgetting to require it.
pub const STANDARD_CONTROL: &str = "STANDARD PERFORMANCE CONTROL";

/// The whole plan, in the order it will run.
///
/// `speculations` is what [`crate::spec::candidates`] offered for this model — already filtered to
/// what can actually run here, so a `draft-*` kind with no draft file never reaches the plan.
/// Which question a search is answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// One window for every model, so two numbers can be put side by side. **The context never
    /// moves here**, which is what stops a search winning by giving half of it away.
    Standard,
    /// The other intents. This is where a smaller window is allowed to be faster, because
    /// somebody asked for `FAST` and knows what they traded.
    Profile,
}

/// Which of the things Epoch knows how to measure **this build can actually run**.
///
/// The KV cache candidates, in the order they are tried. `F16` is always first because it is what
/// the program does with no flag at all, so it needs no permission from anybody; every other one
/// is an argument, and an argument is only offered where the build published it.
///
/// **Epoch offers what it can both run and judge.** `--cache-type-k` publishes `q4_0`, `q4_1` and
/// `iq4_nl` here, and a speed search that recommended a 4-bit KV cache would be trading quality
/// it has no way to measure. That is the Quality Suite's question; until it can answer, the trade
/// is not offered, and this says so out loud rather than looking like an oversight.
pub fn caches_here(accepts: &crate::models::accepts::Accepts) -> Vec<Cache> {
    let mut every = vec![Cache::F16];
    if accepts.takes("--cache-type-k", "q8_0") && accepts.takes("--cache-type-v", "q8_0") {
        every.push(Cache::Q8_0);
    }
    every
}

/// What this build cannot do, in the words a person would use.
///
/// **A shorter search must say why it is shorter.** Dropping a candidate the build cannot run is
/// right; dropping it silently turns *10 configurations* into *8* with nothing to read, which is
/// the absent reading the cold-instrument rule is about. Empty when everything Epoch measures is
/// available — and empty for an unasked build too, because *nobody asked* is not a list of
/// missing features.
pub fn unavailable(accepts: &crate::models::accepts::Accepts) -> Vec<String> {
    let mut missing = Vec::new();
    if !accepts.anything() {
        return missing;
    }
    if !(accepts.takes("--cache-type-k", "q8_0") && accepts.takes("--cache-type-v", "q8_0")) {
        missing.push("KV cache compression — this build does not offer q8_0".to_owned());
    }
    if !accepts.has("--flash-attn") {
        missing.push("Flash attention — this build has no --flash-attn".to_owned());
    }
    if !accepts.has("--override-tensor") {
        missing.push("GPU offload — this build has no --override-tensor".to_owned());
    }
    missing
}

/// **Eight, and they stay eight until after the release.**
///
/// Clippy is right that this is wide, and the fix is a parameter struct rather than fewer
/// facts: every one of these is something the search genuinely needs and none can be derived
/// from the others. Doing it now would restructure the optimisation path in the days before
/// publishing, which is the one moment where a refactor with no correctness consequence is at
/// its most expensive. Deferred deliberately, and written down so it is deferred rather than
/// forgotten.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    phase: Phase,
    standard: u32,
    trained: u32,
    caches: &[Cache],
    speculations: &[Tuning],
    fitted: Option<(String, i32)>,
    accepts: &crate::models::accepts::Accepts,
    fit: crate::models::runtimes::FitClass,
) -> Vec<Step> {
    let mut every = Vec::new();
    let base = Loadout {
        context: standard.min(trained.max(1)),
        cache: caches.first().copied().unwrap_or(Cache::F16),
    };

    every.push(Step {
        about: "Baseline · nothing set".to_owned(),
        technical: format!("-c {}", base.context),
        loadout: base,
        tuning: Tuning::default(),
        offload: None,
        gpu_layers: None,
    });

    /*
        **Where the placement candidate goes depends on how the model sits here**, and that is
        read from the fitter rather than from the file's size.

        `Hybrid` means part of the weights are in host memory — measured on the 35B at 32K, 22 of
        41 layers overflowing and 8218 MiB on the host. There, *is this slow because it does not
        fit or because it is badly distributed* is the largest question in the search, and a run
        stopped by drift should already have answered it. So it goes first, ahead of flash
        attention.

        `Comfortable` means the fitter arranged nothing, and `fitted` is `None` anyway — a
        candidate identical to the baseline is a configuration measured twice.
    */
    let fitted = fitted.filter(|_| !accepts.anything() || accepts.has("--override-tensor"));
    let placement_first = fit == crate::models::runtimes::FitClass::Hybrid;
    let mut placement: Option<Step> = fitted.map(|(offload, layers)| Step {
        about: "GPU offload · llama.cpp's own fit".to_owned(),
        technical: format!("-ngl {layers} -ot \"{offload}\""),
        loadout: base,
        tuning: Tuning {
            placement: crate::models::tuning::Placement {
                gpu_layers: Some(layers),
                override_tensor: Some(offload.clone()),
            },
            ..Tuning::default()
        },
        offload: Some(offload),
        gpu_layers: Some(layers),
    });
    if placement_first {
        every.extend(placement.take());
    }

    /*
        Stage 2 — flash attention, at the standard context and on the plain cache.

        **`on`, and not `off`.** Three states exist and the baseline already measured one of them:
        with no flag written, llama.cpp uses `auto` — its own answer, and the state somebody who
        never opens this screen is already running. So the question worth two minutes of GPU time
        is *does forcing it on beat what the runtime chose*, measured 2026-09-01 at 46.8 → 50.5
        tok/s.

        `off` was measured too, and it is the one candidate on this machine that reliably produces
        nothing at all — no answer, system memory 28 → 33.7 GB, in every run of every search. It
        is also the direction `auto` would already have taken if it were the right one. A
        candidate whose best case is *the same as the baseline* and whose measured case is a
        failure is not low-risk and high-value; it is the search paying for a row nobody would
        apply.

        Only where the build has the flag: a candidate written for a program that will reject it
        costs the same two minutes and produces a failure that reads like a finding.
    */
    if !accepts.anything() || accepts.has("--flash-attn") {
        every.push(Step {
            about: "Flash attention · on".to_owned(),
            technical: "--flash-attn on".to_owned(),
            loadout: base,
            tuning: Tuning {
                flash_attn: Some(true),
                ..Tuning::default()
            },
            offload: None,
            gpu_layers: None,
        });
    }

    /*
        Stage 3 — the other cache, at the standard context.

        After flash attention rather than before it, because a search that stops early should
        already have answered the question with the largest measured effect. The order is what a
        drift-stopped run keeps.
    */
    for cache in caches.iter().skip(1) {
        every.push(Step {
            about: match cache {
                Cache::Q8_0 => "KV cache · compressed".to_owned(),
                Cache::F16 => "KV cache · full precision".to_owned(),
            },
            technical: format!("-c {} {}", base.context, cache.flags())
                .trim_end()
                .to_owned(),
            loadout: Loadout {
                cache: *cache,
                ..base
            },
            tuning: Tuning::default(),
            offload: None,
            gpu_layers: None,
        });
    }

    /*
        Stage 3b — the other contexts, and **only in the profile phase**.

        A Standard search that could shrink the window would report a bigger number for every
        model and would be comparing seven different benchmarks.
    */
    if phase == Phase::Profile {
        for context in contexts(standard, trained).into_iter().skip(1) {
            every.push(Step {
                about: format!("Context · {}K", context / 1024),
                technical: format!("-c {context}"),
                loadout: Loadout { context, ..base },
                tuning: Tuning::default(),
                offload: None,
                gpu_layers: None,
            });
        }
    }

    /*
        Stage 4 — llama.cpp's own answer to what fits.

        **This is the step that answers the interesting question**: is the model slow because it
        does not fit, or because it is badly distributed? Measured 2026-08-31, the runtime left to
        itself spills whole layers; asked, it recommends keeping attention on the card and moving
        only the MoE experts. Those are different machines to run on, and only one of them had
        ever been tried.

        The pattern is passed through untouched — Epoch asks the program that will consume it and
        does not compose one (`tuning::Placement`).
    */
    if !placement_first {
        every.extend(placement.take());
    }

    // Stage 5 — speculation.
    for tuning in speculations {
        every.push(Step {
            about: format!("Speculative decoding · {}", crate::spec::label(tuning)),
            technical: tuning.lines().replace('\n', " ").trim().to_owned(),
            loadout: base,
            tuning: tuning.clone(),
            offload: None,
            gpu_layers: None,
        });
    }

    every
}

/// What one step measured, folded into a `Configuration`.
///
/// Its own function so the search's bookkeeping and the measuring are separable, and so a step
/// that failed becomes a row that says it failed rather than a gap in the table.
/// Nine, for the same reason and with the same deferral as `plan` above.
#[allow(clippy::too_many_arguments)]
pub fn recorded(
    step: &Step,
    measured: &crate::bench::Measured,
    load: &crate::models::load::Load,
    // Model bytes on the card and in host memory, from llama.cpp's own breakdown.
    placed: Option<(u64, u64)>,
    // How many runs of this configuration were thrown away, and how many of those were collapses
    // rather than plain failures. They change the verdict even when every kept run looks perfect.
    discarded: usize,
    collapses: usize,
    same_answers: Option<bool>,
    // Whether this was measured with the window deliberately filled.
    //
    // **The workload, not the reading.** `filled` was taken from whatever the runs reported, and
    // a short question is twenty-one tokens — so every short-workload row came back
    // `Some(21)` and nothing could tell the two workloads apart. Measured 2026-09-02: eight
    // Stage-1 rows carried `filled: 21`, a ladder skipped its 32K rung because one of them
    // looked like a filled reading at that window, and `resolve` was about to rank a question
    // against a full window.
    //
    // The count kept is still the server's own; what this decides is whether there is one.
    filling: bool,
    at: u64,
) -> Configuration {
    let (finished, asked) = measured.stability();
    Configuration {
        loadout: step.loadout,
        tuning: step.tuning.clone(),
        offload: step.offload.clone(),
        gpu_layers: step.gpu_layers,
        generation: measured.generation().unwrap_or(0.0),
        prompt: measured.prompt(),
        first_token_ms: measured.first_token_ms(),
        vram_used: load.vram_used.map(|it| it.peak as u64),
        ram_used: load.ram_used.map(|it| it.peak as u64),
        gpu_peak: load.gpu_percent.map(|it| it.peak),
        gpu_mean: load.gpu_percent.map(|it| it.mean),
        cpu_peak: load.cpu_percent.map(|it| it.peak),
        cpu_mean: load.cpu_percent.map(|it| it.mean),
        // What llama.cpp said it would place where, for the step that asked it. `None` for every
        // other step, which is *nobody asked* rather than *all of it on the card*.
        model_on_gpu: placed.map(|(gpu, _)| gpu),
        model_on_host: placed.map(|(_, host)| host),
        drafted: totalled(measured, |r| r.drafted),
        accepted: totalled(measured, |r| r.accepted),
        // What was actually in the window, from the server's own count — and only where filling
        // was the point. `None` on a short workload is what separates the two curves.
        filled: filling
            .then(|| measured.middle(|r| r.prompt_tokens.map(|it| it as f64)))
            .flatten()
            .map(|it| it as u64),
        stable: finished == asked && measured.generation().is_some(),
        // Every run of this configuration that came back, judged together.
        verdict: crate::models::health::judge(
            &measured
                .runs
                .iter()
                .filter(|r| r.ok())
                .filter_map(|r| r.generation)
                .collect::<Vec<f64>>(),
            discarded,
            collapses,
        ),
        around: crate::models::health::Around {
            gpu_mean: load.gpu_percent.map(|it| it.mean),
            shared_before: load.shared_before,
            shared_after: load.shared_after,
            vram_before: load.vram_before,
            vram_peak: load.vram_used.map(|it| it.peak as u64),
        },
        // Filled in by the search once the closing control has run: a candidate is not
        // eligible until something shows the machine was the same afterwards.
        bracket: None,
        same_answers,
        at,
    }
}

/// A total across the runs that finished, or `None` where none of them reported it.
///
/// **`None` rather than zero**, because a configuration that drafted nothing and one that was
/// never asked to draft are different facts, and only one of them is about the configuration.
fn totalled(
    measured: &crate::bench::Measured,
    of: impl Fn(&crate::bench::Run) -> Option<u64>,
) -> Option<u64> {
    let got: Vec<u64> = measured
        .runs
        .iter()
        .filter(|r| r.ok())
        .filter_map(of)
        .collect();
    (!got.is_empty()).then(|| got.iter().sum())
}

/// Which configuration a search should carry forward into the next stage.
///
/// **The best so far, and never a failed one.** A greedy walk that carried a crash forward would
/// spend the rest of the search measuring variations of something broken.
pub fn carry<'a>(found: &'a [Configuration], from: &'a Configuration) -> &'a Configuration {
    found
        .iter()
        .filter(|it| it.stable)
        // **By verdict, not by reading.** Carrying the best *moment* forward would spend the rest
        // of the search measuring variations of a configuration that only looked good once.
        .max_by(|a, b| crate::models::health::better(&a.verdict, &b.verdict))
        .unwrap_or(from)
}

/// A short reading of the current configuration: is it healthy, and how fast.
///
/// **This is QUICK TEST**, and it is deliberately not the search. It answers *how does the thing
/// I have right now behave* in under a minute; the search answers *what should I have* in
/// twenty. Conflating them would make the cheap question cost the expensive one's time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quick {
    pub model: String,
    pub generation: Option<f64>,
    pub prompt: Option<f64>,
    pub first_token_ms: Option<f64>,
    pub context: u32,
    pub vram_used: Option<u64>,
    pub ram_used: Option<u64>,
    pub gpu_percent: Option<f64>,
    pub cpu_percent: Option<f64>,
    /// Every run finished.
    pub healthy: bool,
    /// What went wrong, in the server's own words.
    pub failures: Vec<String>,
}

impl Quick {
    /// One word for the state of it.
    pub fn verdict(&self) -> &'static str {
        if !self.healthy {
            return "unstable";
        }
        match self.generation {
            None => "no answer",
            Some(_) => "healthy",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A build that withholds nothing, for the tests that are about ordering rather than about
    /// what a program offers.
    fn everything() -> crate::models::accepts::Accepts {
        crate::models::accepts::Accepts::from_help(
            "-ctk,  --cache-type-k TYPE              KV cache data type for K
                                                     allowed values: f16, q8_0
             -ctv,  --cache-type-v TYPE              KV cache data type for V
                                                     allowed values: f16, q8_0
             -fa,   --flash-attn [on|off|auto]       set Flash Attention use
             -ot,   --override-tensor <pattern>      override tensor buffer type
",
        )
    }

    /// What a search spends GPU time on, and where the list comes from.
    /// The windows a model is asked about, and which of them this machine can hold.
    mod rungs {
        use super::*;
        use crate::models::runtimes::FitClass;

        #[test]
        fn the_ladder_stops_at_what_the_model_was_trained_for() {
            /*
                Gemma 4 declares 262,144, so 256K is a rung and 512K is not. Asking a model for
                more than it was trained to hold buys nothing and costs a load to find out.
            */
            assert_eq!(ladder(262_144), [16_384, 32_768, 65_536, 131_072, 262_144],);
            // And a model that really does declare a million gets one.
            assert_eq!(ladder(1_048_576).last(), Some(&1_048_576));
            // A small window is a short ladder rather than an empty one.
            assert_eq!(ladder(32_768), [16_384, 32_768]);
            assert!(ladder(4_096).is_empty(), "nothing here is worth a load");
        }

        #[test]
        fn what_the_model_declares_and_what_the_machine_holds_are_two_facts() {
            /*
                Collapsing them loses the one a person actually asks. *This model was built for
                256K and this card holds 128K of it* is two readings and both are true; replacing
                the first with the second answers a question about the machine when somebody
                asked about the model.
            */
            let rungs: Vec<Rung> = ladder(262_144)
                .into_iter()
                .map(|context| Rung {
                    context,
                    fit: if context > 131_072 {
                        FitClass::Unloadable
                    } else {
                        FitClass::Comfortable
                    },
                })
                .collect();
            assert_eq!(rungs.len(), 5, "the model still declares five");
            assert_eq!(
                rungs.iter().filter(|it| it.worth_measuring()).count(),
                4,
                "and this machine can be asked about four",
            );
        }

        #[test]
        fn a_rung_nobody_could_ask_about_is_still_measured() {
            /*
                A machine whose fitter could not be run is not a machine that cannot hold the
                window. Refusing there would turn a missing sibling binary into a shorter ladder
                with nothing saying why — silence read as no, wearing a rung.
            */
            let unasked = Rung {
                context: 131_072,
                fit: FitClass::Unknown,
            };
            assert!(unasked.worth_measuring());
            let refused = Rung {
                context: 262_144,
                fit: FitClass::Unloadable,
            };
            assert!(!refused.worth_measuring());
        }
    }

    mod planned {
        use super::*;
        use crate::models::accepts::Accepts;

        use crate::models::runtimes::FitClass;

        fn steps_for(accepts: &Accepts, fit: FitClass) -> Vec<String> {
            plan(
                Phase::Standard,
                32_768,
                262_144,
                &caches_here(accepts),
                &[],
                Some(("exps=CPU".to_owned(), 41)),
                accepts,
                fit,
            )
            .into_iter()
            .map(|it| it.about)
            .collect()
        }

        #[test]
        fn a_model_that_does_not_fit_is_asked_about_placement_first() {
            /*
                Measured 2026-09-01: `Qwen3.6-35B-A3B-UD-IQ4_XS` at 32K fits by moving 22 of 41
                layers' experts to the host — 8218 MiB of model in system memory, the GPU idle
                three fifths of the time. There, *is this slow because it does not fit or because
                it is badly distributed* is the largest question in the search, and a run stopped
                by drift should already have answered it.

                Flash attention was worth 3.2% on the model that fits. This one is worth the
                difference between running on a card and running across a bus.
            */
            let full = build("f32, f16, q8_0", true, true);
            let about = steps_for(&full, FitClass::Hybrid);
            assert_eq!(about[0], "Baseline · nothing set");
            assert_eq!(about[1], "GPU offload · llama.cpp's own fit");
            assert_eq!(about[2], "Flash attention · on");
        }

        #[test]
        fn a_model_that_fits_is_asked_about_it_last() {
            // Everything on the card already: the arrangement is the least interesting question,
            // and on a truly comfortable model the fitter offers no arguments at all.
            let full = build("f32, f16, q8_0", true, true);
            let about = steps_for(&full, FitClass::Comfortable);
            assert_eq!(
                about.last().map(String::as_str),
                Some("GPU offload · llama.cpp's own fit")
            );
            assert_eq!(about[1], "Flash attention · on");
        }

        #[test]
        fn every_class_plans_the_same_candidates_in_a_different_order() {
            // The branch decides *what to ask first*, never *what to leave out*: a search that
            // measured fewer things on one machine than another would not be comparable with it.
            let full = build("f32, f16, q8_0", true, true);
            let mut hybrid = steps_for(&full, FitClass::Hybrid);
            let mut comfortable = steps_for(&full, FitClass::Comfortable);
            let mut unknown = steps_for(&full, FitClass::Unknown);
            hybrid.sort();
            comfortable.sort();
            unknown.sort();
            assert_eq!(hybrid, comfortable);
            assert_eq!(comfortable, unknown);
        }

        /// The shape llama.cpp actually writes, trimmed to the options the planner asks about.
        fn build(cache: &str, flash: bool, offload: bool) -> Accepts {
            let mut help = String::from("----- common params -----\n");
            help.push_str(&format!(
                "-ctk,  --cache-type-k TYPE              KV cache data type for K\n\
                 \x20                                       allowed values: {cache}\n\
                 -ctv,  --cache-type-v TYPE              KV cache data type for V\n\
                 \x20                                       allowed values: {cache}\n"
            ));
            if flash {
                help.push_str("-fa,   --flash-attn [on|off|auto]       set Flash Attention use\n");
            }
            if offload {
                help.push_str(
                    "-ot,   --override-tensor <pattern>      override tensor buffer type\n",
                );
            }
            Accepts::from_help(&help)
        }

        fn steps(accepts: &Accepts) -> Vec<String> {
            plan(
                Phase::Standard,
                32_768,
                262_144,
                &caches_here(accepts),
                &[],
                Some(("exps=CPU".to_owned(), 99)),
                accepts,
                crate::models::runtimes::FitClass::Unknown,
            )
            .into_iter()
            .map(|it| it.about)
            .collect()
        }

        #[test]
        fn everything_published_is_offered() {
            let full = build("f32, f16, bf16, q8_0, q4_0", true, true);
            let about = steps(&full);
            assert_eq!(
                about,
                [
                    // **The order is the strategy.** A search stopped by drift keeps the rows it
                    // already took, so the largest measured effect comes first.
                    "Baseline · nothing set",
                    "Flash attention · on",
                    "KV cache · compressed",
                    "GPU offload · llama.cpp's own fit",
                ],
            );
            assert!(unavailable(&full).is_empty());
        }

        #[test]
        fn a_flag_this_build_lacks_is_not_planned_and_is_named() {
            /*
                A candidate written for a program that will reject it costs two minutes of GPU
                time and produces a failure that reads like a finding. Dropping it silently turns
                *10 configurations* into *8* with nothing to read, which is the absent reading the
                cold-instrument rule is about — so it is dropped **and** named.
            */
            let plain = build("f32, f16", false, false);
            let about = steps(&plain);
            assert_eq!(about, ["Baseline · nothing set"]);

            let said = unavailable(&plain).join(" | ");
            assert!(said.contains("q8_0"), "{said}");
            assert!(said.contains("--flash-attn"), "{said}");
            assert!(said.contains("--override-tensor"), "{said}");
        }

        #[test]
        fn a_build_nobody_could_ask_is_offered_everything() {
            /*
                `unasked` is not *this build has no options*. Reading silence as a missing flag
                would take the whole search away from a machine where `--help` merely failed to
                run — the same inversion as reading it as *no*, wearing a planner.
            */
            let nothing = Accepts::unasked();
            assert_eq!(
                caches_here(&nothing),
                [Cache::F16],
                "and no cache flag is guessed either"
            );
            assert_eq!(
                steps(&nothing),
                [
                    "Baseline · nothing set",
                    "Flash attention · on",
                    "GPU offload · llama.cpp's own fit",
                ],
            );
            assert!(
                unavailable(&nothing).is_empty(),
                "nobody asked is not a list of absences"
            );
        }

        #[test]
        fn the_cache_that_needs_no_flag_is_always_offered() {
            // `f16` is what the program does with no argument at all, so it needs permission from
            // nobody — including from a build that publishes nothing.
            for accepts in [
                build("q4_0", false, false),
                Accepts::unasked(),
                build("f16", true, true),
            ] {
                assert_eq!(caches_here(&accepts).first(), Some(&Cache::F16));
            }
        }

        #[test]
        fn a_value_epoch_will_not_recommend_is_not_offered_even_when_published() {
            /*
                `--cache-type-k` publishes `q4_0` on this machine. A speed search that recommended
                a 4-bit KV cache would be trading quality it cannot measure — the Quality Suite's
                question. Being able to run something is not a reason to recommend it.
            */
            let wide = build(
                "f32, f16, bf16, q8_0, q4_0, q4_1, iq4_nl, q5_0, q5_1",
                true,
                true,
            );
            assert_eq!(caches_here(&wide), [Cache::F16, Cache::Q8_0]);
        }

        #[test]
        fn one_side_of_the_cache_is_not_enough() {
            // K published and V not is a build that cannot run the configuration Epoch would
            // write, which passes `-ctk` and `-ctv` together.
            let half = Accepts::from_help(
                "-ctk,  --cache-type-k TYPE              KV cache data type for K\n\
                 \x20                                       allowed values: f16, q8_0\n\
                 -ctv,  --cache-type-v TYPE              KV cache data type for V\n\
                 \x20                                       allowed values: f16\n",
            );
            assert_eq!(caches_here(&half), [Cache::F16]);
        }
    }

    fn spec(kind: &str, n: u32) -> Tuning {
        Tuning {
            speculation: crate::models::tuning::Speculation {
                kind: kind.to_owned(),
                n_max: Some(n),
                ..crate::models::tuning::Speculation::default()
            },
            ..Tuning::default()
        }
    }

    #[test]
    fn the_plan_begins_with_the_baseline_and_the_standard_context() {
        /*
            A search that is stopped halfway must already have measured the thing everything else
            is compared against. An improvement quoted against a baseline that never ran is a
            comparison of two afternoons.
        */
        let every = plan(
            Phase::Standard,
            32_768,
            262_144,
            &[Cache::F16, Cache::Q8_0],
            &[],
            None,
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        assert!(every[0].about.contains("Baseline"));
        assert_eq!(every[0].loadout.context, 32_768);
        assert_eq!(every[0].tuning, Tuning::default());
    }

    #[test]
    fn a_plan_is_about_twenty_steps_and_not_several_hundred() {
        // Every axis times every other axis is an evening for an answer nobody waits for.
        let every = plan(
            Phase::Standard,
            32_768,
            262_144,
            &[Cache::F16, Cache::Q8_0],
            &[
                spec("ngram-mod", 1),
                spec("ngram-mod", 2),
                spec("ngram-cache", 3),
            ],
            Some(("blk.*=CPU".to_owned(), 41)),
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        assert!(every.len() <= 24, "{} steps", every.len());
        assert!(every.len() >= 6, "{} steps", every.len());
    }

    #[test]
    fn the_ladder_never_asks_for_more_than_the_model_was_trained_for() {
        // It buys nothing and costs a load to find out.
        assert_eq!(contexts(32_768, 32_768), vec![32_768, 8_192, 16_384]);
        let small = contexts(32_768, 8_192);
        assert!(small.iter().all(|it| *it <= 8_192), "{small:?}");
    }

    #[test]
    fn a_model_trained_below_the_standard_context_still_gets_a_plan() {
        // The first version would have asked a 4K model for 32K and called the refusal a result.
        let every = plan(
            Phase::Profile,
            32_768,
            4_096,
            &[Cache::F16],
            &[],
            None,
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        assert!(
            every.iter().all(|it| it.loadout.context <= 4_096),
            "{every:?}"
        );
        assert!(!every.is_empty());
    }

    #[test]
    fn a_standard_search_never_touches_the_context() {
        /*
            The owner's rule, and it is what keeps the model championship honest: a search allowed
            to shrink the window reports a bigger number for every model and is comparing seven
            different benchmarks. Discovering that 16K is faster belongs in `FAST`, where somebody
            asked for it.
        */
        let standard = plan(
            Phase::Standard,
            32_768,
            262_144,
            &[Cache::F16, Cache::Q8_0],
            &[],
            None,
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        assert!(
            standard.iter().all(|it| it.loadout.context == 32_768),
            "{:?}",
            standard
                .iter()
                .map(|it| (it.about.clone(), it.loadout.context))
                .collect::<Vec<_>>(),
        );

        let profile = plan(
            Phase::Profile,
            32_768,
            262_144,
            &[Cache::F16, Cache::Q8_0],
            &[],
            None,
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        assert!(
            profile.iter().any(|it| it.loadout.context != 32_768),
            "and the profile phase is where the window may move",
        );
    }

    #[test]
    fn the_cache_and_the_context_are_never_changed_in_one_step() {
        /*
            Epoch's stale preset had `ctx-size = 16384` **and** `cache-type-k = q8_0`, and cost
            about 10%. *Both of them cost 10%* is not a finding, it is two unmeasured halves — so
            no step here moves more than one of them at a time.
        */
        let every = plan(
            Phase::Profile,
            32_768,
            262_144,
            &[Cache::F16, Cache::Q8_0],
            &[],
            None,
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        let base = every[0].loadout;
        for step in &every {
            let moved_cache = step.loadout.cache != base.cache;
            let moved_context = step.loadout.context != base.context;
            assert!(
                !(moved_cache && moved_context),
                "{} moves both at once",
                step.about,
            );
        }
    }

    #[test]
    fn speculation_comes_after_everything_that_fixes_the_baseline() {
        /*
            Measured 2026-08-31: speculation is worth +3.9% and a stale preset was costing about
            10%. A search that reached for speculation early would be using it to compensate for a
            base configuration nobody had fixed, and would report the compensation as a win.
        */
        let every = plan(
            Phase::Standard,
            32_768,
            262_144,
            &[Cache::F16, Cache::Q8_0],
            &[spec("draft-mtp", 1)],
            Some(("blk.*=CPU".to_owned(), 41)),
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        let first_speculation = every
            .iter()
            .position(|it| it.about.starts_with("Speculative"))
            .expect("one was offered");
        let last_base = every
            .iter()
            .rposition(|it| !it.about.starts_with("Speculative"))
            .expect("there are base steps");
        assert!(
            first_speculation > last_base,
            "speculation at {first_speculation}, base ends at {last_base}",
        );
    }

    #[test]
    fn a_failed_step_is_never_carried_forward() {
        /*
            A greedy walk that carried a crash forward would spend the rest of the search
            measuring variations of something broken.
        */
        let base = Configuration {
            loadout: Loadout {
                context: 32_768,
                cache: Cache::F16,
            },
            tuning: Tuning::default(),
            offload: None,
            gpu_layers: None,
            generation: 41.0,
            prompt: None,
            first_token_ms: None,
            vram_used: None,
            ram_used: None,
            verdict: crate::models::health::judge(&[41.0], 0, 0),
            around: crate::models::health::Around::default(),
            gpu_peak: None,
            gpu_mean: None,
            cpu_peak: None,
            cpu_mean: None,
            model_on_gpu: None,
            model_on_host: None,
            drafted: None,
            accepted: None,
            stable: true,
            same_answers: None,
            filled: None,
            bracket: None,
            at: 1,
        };
        let broken = Configuration {
            generation: 99.0,
            stable: false,
            ..base.clone()
        };
        let better = Configuration {
            generation: 52.8,
            ..base.clone()
        };
        assert_eq!(carry(std::slice::from_ref(&broken), &base).generation, 41.0);
        assert_eq!(carry(&[broken, better], &base).generation, 52.8);
    }

    #[test]
    fn nothing_measured_yet_is_nought_percent_and_not_a_division_by_zero() {
        assert_eq!(Progress::default().percent(), 0);
        let half = Progress {
            done: 12,
            total: 24,
            ..Progress::default()
        };
        assert_eq!(half.percent(), 50);
    }

    #[test]
    fn the_offload_step_actually_asks_for_the_offload() {
        /*
            The first version put the fitted pattern on the `Step` for the record and left the
            `Tuning` default — so the preset carried nothing, the server ran exactly as it had,
            and the row would have reported *llama.cpp's own fit makes no difference* about a
            configuration that was never applied. A step that measures the baseline twice under
            two names is worse than a missing step.
        */
        let every = plan(
            Phase::Standard,
            32_768,
            262_144,
            &[Cache::F16],
            &[],
            Some(("blk\\.19\\.ffn_.*=CPU".to_owned(), 41)),
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        let offload = every
            .iter()
            .find(|it| it.about.starts_with("GPU offload"))
            .expect("the fit produced a step");
        assert_eq!(offload.tuning.placement.gpu_layers, Some(41));
        assert_eq!(
            offload.tuning.placement.override_tensor.as_deref(),
            Some("blk\\.19\\.ffn_.*=CPU"),
        );
        assert!(
            offload.tuning.lines().contains("override-tensor ="),
            "{}",
            offload.tuning.lines()
        );
    }

    #[test]
    fn a_step_says_what_it_is_doing_in_words_and_keeps_the_flags_for_whoever_wants_them() {
        // The panel shows the words. `Show technical details` shows the rest. Neither is invented
        // at render time, because a sentence composed in the frontend is a second place to keep
        // in step with what was actually run.
        let every = plan(
            Phase::Standard,
            32_768,
            262_144,
            &[Cache::F16],
            &[spec("draft-mtp", 2)],
            None,
            &everything(),
            crate::models::runtimes::FitClass::Unknown,
        );
        let last = every.last().expect("one speculation");
        assert_eq!(last.about, "Speculative decoding · draft-mtp n=2");
        assert!(
            last.technical.contains("spec-type = draft-mtp"),
            "{}",
            last.technical
        );
        assert!(
            last.technical.contains("spec-draft-n-max = 2"),
            "{}",
            last.technical
        );
    }

    #[test]
    fn a_quick_test_that_did_not_finish_says_unstable_rather_than_a_rate() {
        let broken = Quick {
            model: "m".to_owned(),
            generation: Some(50.0),
            prompt: None,
            first_token_ms: None,
            context: 32_768,
            vram_used: None,
            ram_used: None,
            gpu_percent: None,
            cpu_percent: None,
            healthy: false,
            failures: vec!["500: out of memory".to_owned()],
        };
        assert_eq!(broken.verdict(), "unstable");
        assert_eq!(
            Quick {
                healthy: true,
                ..broken.clone()
            }
            .verdict(),
            "healthy",
        );
        assert_eq!(
            Quick {
                healthy: true,
                generation: None,
                ..broken
            }
            .verdict(),
            "no answer",
        );
    }
}
