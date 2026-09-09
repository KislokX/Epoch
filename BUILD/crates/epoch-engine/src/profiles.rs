//! What a model should be run with here, chosen from what was actually run here.
//!
//! ## The rule this whole file exists to keep
//!
//! **A profile is a measured row, never a preset.** `BALANCED`, `FAST`, `MAX QUALITY` and
//! `LONG CONTEXT` are *intents* — questions somebody can ask without knowing what a KV cache is —
//! and each one resolves to the fastest configuration this machine actually ran that satisfies it.
//! None of them is a set of flags somebody typed into a table.
//!
//! So a model nobody has optimised has **no profiles at all**, and the card says so. It does not
//! show four greyed-out ones, and it does not show four plausible ones with no measurement
//! underneath. `CONTENT_PHILOSOPHY`'s Styles arrived at the same answer for the same reason: a
//! catalogue with nothing behind it is not a cold instrument, it is an invented one.
//!
//! ## Why intents rather than settings on the row
//!
//! The owner's brief, 2026-08-31:
//!
//! > MODELS deja de ser *"configurá llama.cpp"* y pasa a ser *"estos son tus modelos, así
//! > funcionan en tu máquina, y esta es la mejor configuración que Epoch encontró para cada uno."*
//!
//! Flash attention, speculative decoding, cache type, GPU layers and draft lengths all still
//! exist and are all still reachable. What changed is who has to understand them: they are now
//! **Epoch's instruments for finding an answer**, not a form somebody fills in before they can
//! use a model.
//!
//! ## Selecting, not scoring
//!
//! Each intent is a filter and then a maximum, and both halves are written out rather than folded
//! into a weighted score. A score would need weights, the weights would be invented, and the
//! first question anybody asked would be *why is that one recommended* — which a filter can
//! answer in a sentence and a weighted sum cannot.
//!
//! ## AUTO is architecture, and it says so
//!
//! [`Intent::Auto`] resolves to whatever `BALANCED` resolves to today. It exists because the
//! interesting version — picking per task, so a one-line question runs `FAST` and a large
//! repository runs `LONG CONTEXT` — needs somewhere to live that is not a rewrite. It is a
//! selection over the same measured rows with a different filter, and the filter would come from
//! the turn rather than from the user. Nothing here forecloses that; nothing here pretends to do
//! it yet.

use serde::{Deserialize, Serialize};

use crate::models::loadout::{Cache, Loadout};
use crate::models::tuning::Tuning;

/// What somebody wants, before knowing what the machine can give them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Intent {
    /// Epoch's pick. Today: whatever `Balanced` picks — see the module docs.
    Auto,
    /// The fastest configuration that still holds the standard context and changed nothing.
    Balanced,
    /// The fastest, whatever it costs in context.
    Fast,
    /// Nothing approximated: full-precision cache, no speculation that altered the text.
    MaxQuality,
    /// The largest context that stayed stable.
    LongContext,
    /// Whatever the user saved.
    Custom,
}

impl Intent {
    /// In the order a person reads them, `Auto` first.
    pub const ALL: [Intent; 5] = [
        Intent::Auto,
        Intent::Balanced,
        Intent::Fast,
        Intent::MaxQuality,
        Intent::LongContext,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Intent::Auto => "AUTO",
            Intent::Balanced => "BALANCED",
            Intent::Fast => "FAST",
            Intent::MaxQuality => "MAX QUALITY",
            Intent::LongContext => "LONG CONTEXT",
            Intent::Custom => "CUSTOM",
        }
    }

    /// What it is for, in one sentence somebody can act on.
    pub fn about(self) -> &'static str {
        match self {
            Intent::Auto => "Epoch picks. Today that is the balanced configuration.",
            Intent::Balanced => "Best balance of quality, speed and context.",
            Intent::Fast => "The fastest measured, at whatever context it needed to give up.",
            Intent::MaxQuality => "Nothing approximated: full-precision cache, unaltered answers.",
            Intent::LongContext => "The most context that stayed stable here.",
            Intent::Custom => "Yours.",
        }
    }
}

/// Whether a control held, on one side of a candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Held {
    /// It reproduced the session's control.
    Reproduced,
    /// It ran and did not reproduce.
    Failed,
    /// It never ran. **Unrecorded, which is not the same as failed** — and not the same as
    /// passed either, which is the whole reason it is a third value.
    Missing,
}

impl Held {
    pub fn ok(self) -> bool {
        self == Held::Reproduced
    }
}

/// The two controls that make a candidate's measurement mean something.
///
/// ## Measurement is not eligibility
///
/// A candidate can produce a perfectly valid reading — 52.1 tok/s, five runs, small spread — and
/// still not be a thing to recommend, because nothing yet shows the machine was in the same state
/// afterwards as it was before. That is what a bracket demonstrates:
///
/// ```text
/// control O-019  ✓ reproduced
///     candidate A
/// control O-021  ✓ reproduced      → A is eligible
///     candidate B
/// control O-023  ✗ below reference → B is contaminated, and the search stops
/// ```
///
/// **A is untouched by B's failure.** It was closed by a control that held, and a later collapse
/// says nothing about a measurement that was already bracketed. What it does say is that nothing
/// after it may be trusted.
///
/// **Not a boolean.** `bracketed: true` records that somebody decided, and loses which controls
/// decided it. This keeps the ids, so History can answer *what made this configuration eligible*
/// rather than *who set the flag* — and the chain of controls reads straight through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Bracket {
    /// The observation id of the control that ran before this candidate.
    pub before: Option<String>,
    pub before_held: Held,
    /// The control that ran after it. **The closing control of one candidate is the opening
    /// control of the next**, so this id appears twice in a session and that is the chain.
    pub after: Option<String>,
    pub after_held: Held,
}

impl Bracket {
    /// Whether this candidate may be recommended.
    pub fn valid(&self) -> bool {
        self.before_held.ok() && self.after_held.ok()
    }

    /// What to call it, for a person reading the row.
    pub fn label(&self) -> &'static str {
        if self.valid() {
            "VALID"
        } else if self.after_held == Held::Missing {
            // The search was stopped, or it is still running. Not a claim about the candidate.
            "INCOMPLETE"
        } else {
            "CONTAMINATED"
        }
    }

    /// One line naming the controls, so the chain can be followed.
    pub fn describe(&self) -> String {
        let side = |id: &Option<String>, held: Held| {
            format!(
                "{} \u{00b7} {}",
                id.clone().unwrap_or_else(|| "\u{2014}".to_owned()),
                match held {
                    Held::Reproduced => "reproduced",
                    Held::Failed => "did not reproduce",
                    Held::Missing => "never ran",
                },
            )
        };
        format!(
            "before {} | after {}",
            side(&self.before, self.before_held),
            side(&self.after, self.after_held),
        )
    }
}

/// One configuration that was really run on this machine, and what it did.
///
/// **Everything needed to run it again is in here**, because a profile the user picks has to be
/// writable back out as flags without anybody re-deriving anything. That is the difference
/// between a measurement and a recommendation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Configuration {
    /// Context and cache type.
    pub loadout: Loadout,
    /// Flash attention and speculative decoding.
    pub tuning: Tuning,
    /// `--override-tensor`, where llama.cpp's own `llama-fit-params` produced one.
    ///
    /// **Opaque on purpose.** It is a regular expression naming tensors, generated by the program
    /// that will consume it, and nothing here parses or reasons about it. Epoch's job is to have
    /// asked and to pass on the answer.
    #[serde(default)]
    pub offload: Option<String>,
    /// How many layers went on the card, where the fit told us.
    #[serde(default)]
    pub gpu_layers: Option<i32>,

    /// Median generation rate over the runs that finished.
    pub generation: f64,
    /// Median prompt-processing rate.
    #[serde(default)]
    pub prompt: Option<f64>,
    #[serde(default)]
    pub first_token_ms: Option<f64>,
    /// Peak video memory in use while it ran, across the whole card.
    #[serde(default)]
    pub vram_used: Option<u64>,
    #[serde(default)]
    pub ram_used: Option<u64>,
    /// What the card was doing while it ran, peak and mean.
    ///
    /// **The pair that answers *does not fit* against *badly distributed*.** A model running
    /// mostly off the card shows low GPU utilisation and high CPU while it generates, which is
    /// invisible from outside — same answer, same tokens, a third of the speed. Measured
    /// 2026-08-31: `gemma4:12b` at 59% GPU and 17% CPU against `Qwen3.6-35B-A3B` at 39% and 52%.
    #[serde(default)]
    pub gpu_peak: Option<f64>,
    #[serde(default)]
    pub gpu_mean: Option<f64>,
    #[serde(default)]
    pub cpu_peak: Option<f64>,
    #[serde(default)]
    pub cpu_mean: Option<f64>,
    /// How much of the model llama.cpp put on the card, and how much stayed in host memory —
    /// its own arithmetic, from `llama-fit-params`, in bytes.
    ///
    /// `None` where nobody asked. **Never a difference computed from the two totals**, which
    /// would charge the model for the desktop.
    #[serde(default)]
    pub model_on_gpu: Option<u64>,
    #[serde(default)]
    pub model_on_host: Option<u64>,
    /// Drafted and kept, where speculation was on. Absent means *nothing was drafted*, which is
    /// not the same as *nothing was accepted*.
    #[serde(default)]
    pub drafted: Option<u64>,
    #[serde(default)]
    pub accepted: Option<u64>,
    /// Every run finished and it produced a rate.
    ///
    /// **Kept, and no longer the thing that decides.** It answers *did the requests come back*,
    /// which is a different question from *is this configuration one configuration* — see
    /// `verdict`.
    pub stable: bool,
    /// What the runs of this configuration amounted to: median, spread, and how many had to be
    /// thrown away.
    ///
    /*
        **The measurement that replaced peak throughput.** Measured 2026-08-31, the same
        configuration answered at 48.3, 44.2 and 25.0 across three searches, because a model
        instance can be evicted partway through and never recover. A peak is the best moment of
        whichever instance happened to survive; a median with a spread beside it is the
        configuration.

        The owner's rule, and it is not a tie-break:

        | | peak | median | range | collapses |
        |---|---|---|---|---|
        | A | 48.1 | 44.0 | 25–48 | 2 |
        | B | 45.9 | 45.5 | 45.1–45.9 | 0 |

        B, without argument.
    */
    #[serde(default)]
    pub verdict: crate::models::health::Verdict,
    /// What the machine looked like around the run — the readings the collapse detector uses.
    #[serde(default)]
    pub around: crate::models::health::Around,
    /// The controls on either side of it. `None` on anything measured before brackets existed.
    ///
    /// **Unrecorded is not proof**, so an old row without one is not recommendable — its
    /// measurement is kept and its eligibility is not asserted. Nothing is deleted; what is
    /// withheld is the claim that the machine was in the same state afterwards.
    #[serde(default)]
    pub bracket: Option<Bracket>,
    /// Whether it wrote the same text as the unconfigured baseline.
    ///
    /// `None` is *nobody checked* and does not disqualify anything; `Some(false)` means this
    /// configuration is a different model rather than a faster one, and `MaxQuality` refuses it.
    #[serde(default)]
    pub same_answers: Option<bool>,
    /// How many prompt tokens were actually in the window while this was measured.
    ///
    /// **The workload this row belongs to, and two of them may never be compared.** A short
    /// question in a 64K window and 62K of prompt in the same window are two different
    /// measurements of two different things — 48.5 tok/s and 41.9 on this card, measured
    /// 2026-09-01 — and a curve that mixed them would report a cliff or a bargain that is
    /// neither.
    ///
    /// `None` is the short workload: a question, which is what every reading before that day
    /// was. `Some(n)` is the server's own `prompt_n` for the runs, not the number Epoch asked
    /// for.
    #[serde(default)]
    pub filled: Option<u64>,
    pub at: u64,
}

impl Configuration {
    pub fn context(&self) -> u32 {
        self.loadout.context
    }

    /// Whether anything about this configuration trades exactness for speed.
    ///
    /// A compressed cache approximates the attention keys and values; speculation that altered
    /// the text is not the same model. Speculation that did *not* alter the text is exact by
    /// construction — that is the whole guarantee of the technique — so it does not count against
    /// quality, and saying otherwise would cost a free win for a reason nobody measured.
    /// What share of the drafted tokens were kept. `None` where nothing drafted.
    ///
    /// **Recorded beside the speed-up and never used to compute it.** Three of four accepted is
    /// not 4x, and the first live run of this machinery proved it: 100% acceptance, 2.47x.
    pub fn acceptance(&self) -> Option<f64> {
        let drafted = self.drafted?;
        (drafted > 0).then(|| self.accepted.unwrap_or(0) as f64 / drafted as f64)
    }

    pub fn approximates(&self) -> bool {
        self.loadout.cache == Cache::Q8_0 || self.same_answers == Some(false)
    }

    /// How much this configuration is doing to the runtime, in things somebody would have to
    /// understand to explain it.
    ///
    /// **Not a quality judgement and not a cost.** It is only the tie-break: where two
    /// configurations measure the same, the one that changes less is the one to run for hours.
    /// Nothing set is 0; every flag on top of that is 1.
    pub fn moving_parts(&self) -> u32 {
        let mut parts = 0;
        if self.loadout.cache != Cache::F16 {
            parts += 1;
        }
        if self.tuning.flash_attn.is_some() {
            parts += 1;
        }
        if self.tuning.placement.override_tensor.is_some()
            || self.tuning.placement.gpu_layers.is_some()
        {
            parts += 1;
        }
        if !self.tuning.speculation.kind.is_empty() && self.tuning.speculation.kind != "none" {
            parts += 1;
        }
        parts
    }
}

/// Everything measured for one model, on one machine, and the profiles that fall out of it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Optimized {
    pub model: String,
    /// The card, the runtime and the build these were taken on. A configuration measured on
    /// another machine is not a fact about this one.
    pub gpu: String,
    pub build: String,
    pub runtime: String,
    /// The unconfigured run, so an improvement is against something that was measured rather than
    /// against something remembered.
    pub baseline: Option<Configuration>,
    /// Every configuration tried, in the order it ran.
    pub tried: Vec<Configuration>,
    /// What the user saved by hand, if anything.
    #[serde(default)]
    pub custom: Option<Configuration>,
    /// Why this search produced no context curve, where it tried and could not.
    ///
    /// **`None` means it ran or was never reached, and the rows say which.** A refusal used to be
    /// emitted as a progress event, which arrives after the panel has moved on — so a stage that
    /// silently did not happen looked exactly like one nobody asked for. Measured 2026-09-02:
    /// eight rows, session complete, zero rungs, and nothing anywhere saying why.
    #[serde(default)]
    pub ladder: Option<String>,
    /// Which intent the user chose. `None` until they choose one — Epoch measures and the user
    /// picks (ADR-0033), so nothing here is applied by having been measured.
    #[serde(default)]
    pub chosen: Option<Intent>,
    /// The search this came from, and whether the machine stayed the same machine throughout.
    #[serde(default)]
    pub session: Option<crate::session::Session>,
    /// Whether these rows may become profiles.
    ///
    /// **A degraded session produces none.** Its rows are kept because they are the diagnosis —
    /// the candidate that broke the environment is named — and nothing measured after the control
    /// stopped reproducing may become something somebody runs for hours.
    #[serde(default = "yes")]
    pub usable: bool,
}

/// Results recorded before sessions existed were taken without a control, and treating them as
/// unusable would throw away every measurement on the machine.
fn yes() -> bool {
    true
}

/// Why a measured configuration is not among the ones Epoch offers by itself.
///
/// **It exists so the reason can be shown rather than implied by an absence.** Measured
/// 2026-09-02, three searches each set a configuration aside and no surface said so — on one
/// of them it was the model's own prediction head, the reason its owner had chosen that build.
/// A row that silently disappears reads as *this did nothing*, which is the opposite of what was
/// measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Aside {
    /// The runs disagreed too much, and the range overlaps something steadier.
    Varied,
    /// The instance broke partway through. Not offered at all — see [`Optimized::set_aside`].
    Collapsed,
    /// It changed what the model answered.
    ChangedAnswers,
    /// No control, or one that did not reproduce, so nothing says the machine was the same.
    Unwitnessed,
    /// The whole session ended on a machine that stopped reproducing.
    SessionDegraded,
}

/// One measured configuration Epoch does not offer, and the reason.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasAside {
    pub configuration: Configuration,
    pub why: Aside,
}

/// An optimisation as a surface sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    #[serde(flatten)]
    pub found: Optimized,
    /// Measured, not offered, and named rather than absent.
    pub aside: Vec<WasAside>,
    /// What each intent resolves to — **resolved here, never again downstream.**
    ///
    /// The panel used to carry its own copy of `resolve`, defended on the grounds that the
    /// alternative was a command per intent per row on every render. That was true when the only
    /// way to ask was a round trip; it stopped being true the moment an optimisation travelled as
    /// a view. The copy then did what a copy does: [`Verdict::dominates`] was added to the Engine
    /// on 2026-09-02 and the deck went on showing the row the Engine had stopped choosing.
    ///
    /// [`Verdict::dominates`]: crate::models::health::Verdict::dominates
    pub profiles: Vec<Profile>,
}

impl Optimized {
    /// The fastest thing that is eligible without argument — what a wide spread is measured
    /// against.
    fn best_beyond_doubt(&self) -> Option<&Configuration> {
        self.plainly_usable()
            .max_by(|a, b| crate::models::health::better(&a.verdict, &b.verdict))
    }

    /// Rows that qualify on their own terms.
    fn plainly_usable(&self) -> impl Iterator<Item = &Configuration> {
        use crate::models::health::State;
        let session_held = self.usable;
        self.tried.iter().filter(move |it| {
            session_held
                &&
            it.stable
                && it.same_answers != Some(false)
                // **A configuration that collapsed is never offered**, however fast its surviving
                // runs were. The instance that produced them is one reload away from the state
                // that was thrown out, and a profile is something somebody runs for hours.
                && it.verdict.collapses == 0
                && !matches!(it.verdict.state(), State::Unstable | State::Invalid)
                // **Measurement is not eligibility.** A reading can be perfect and still not be a
                // thing to recommend until a control shows the machine was the same afterwards.
                && it.bracket.as_ref().is_some_and(Bracket::valid)
        })
    }

    /// Only the rows worth offering.
    ///
    /// **A wide spread is judged against what it would replace, never on its own.** A
    /// configuration whose slowest run beat the fastest run of the best alternative cannot leave
    /// anybody worse off, whatever its label — the reasoning is in
    /// [`crate::models::health::Verdict::dominates`], and it was found by measuring a model whose
    /// own prediction head was discarded for running 51.8–56.4 against a steady 48.6.
    ///
    /// Everything else a row must satisfy is unchanged: it finished, it did not collapse, it did
    /// not rewrite the model's answers, and a control witnessed the machine on both sides.
    fn usable(&self) -> impl Iterator<Item = &Configuration> {
        use crate::models::health::State;
        let bar = self.best_beyond_doubt().map(|it| it.verdict.clone());
        let session_held = self.usable;
        self.tried.iter().filter(move |it| {
            let sound = session_held
                && it.stable
                && it.same_answers != Some(false)
                && it.verdict.collapses == 0
                && it.bracket.as_ref().is_some_and(Bracket::valid);
            if !sound {
                return false;
            }
            if !matches!(it.verdict.state(), State::Unstable | State::Invalid) {
                return true;
            }
            // Invalid means no usable reading at all, so there is no range to compare.
            it.verdict.state() == State::Unstable
                && bar.as_ref().is_some_and(|best| it.verdict.dominates(best))
        })
    }

    /// The record, plus what it set aside and why — the shape every surface receives.
    ///
    /// **The rule lives here and is not repeated anywhere else.** A frontend that recomputed
    /// which rows were held back would be a second place deciding, and two places deciding is how
    /// they come to disagree the first time one of them learns something. Carried on the same
    /// payload rather than fetched separately, so a panel never paints *nothing was set aside* on
    /// its way to the truth.
    pub fn as_shown(&self) -> Shown {
        Shown {
            profiles: self.profiles(crate::suite::STANDARD_CONTEXT),
            aside: self
                .set_aside()
                .into_iter()
                .map(|(it, why)| WasAside {
                    configuration: it.clone(),
                    why,
                })
                .collect(),
            found: self.clone(),
        }
    }

    /// What was measured, is not offered, and is worth saying so about.
    ///
    /// **Not everything excluded belongs here.** A collapsed instance is reported as collapsed
    /// and never handed over: `Collapsed` exists so the row can be named, not so it can be
    /// chosen. The surface decides what it lets somebody press; this decides what it lets
    /// somebody know.
    pub fn set_aside(&self) -> Vec<(&Configuration, Aside)> {
        use crate::models::health::State;
        let offered: Vec<*const Configuration> = self.usable().map(|it| it as *const _).collect();
        self.tried
            .iter()
            .filter(|it| !offered.contains(&(*it as *const _)))
            .filter_map(|it| {
                let why = if !self.usable {
                    Aside::SessionDegraded
                } else if it.verdict.collapses > 0 {
                    Aside::Collapsed
                } else if it.same_answers == Some(false) {
                    Aside::ChangedAnswers
                } else if !it.bracket.as_ref().is_some_and(Bracket::valid) {
                    Aside::Unwitnessed
                } else if it.verdict.state() == State::Unstable {
                    Aside::Varied
                } else {
                    // `Invalid`, or a row that produced no reading. Nothing to offer and nothing
                    // useful to say beyond that it did not work, which the absence already says.
                    return None;
                };
                Some((it, why))
            })
            .collect()
    }

    /// Whether anything here was measured with the window actually full.
    ///
    /// Once a ladder has run, its rows are the ones that answer *how much conversation* — and the
    /// short-prompt rows beside them answer a different question and must not be ranked against
    /// them.
    pub fn has_a_filled_curve(&self) -> bool {
        self.usable().any(|it| it.filled.is_some())
    }

    /// The configuration one intent resolves to, or `None` where nothing measured satisfies it.
    ///
    /// **`None` is a real answer.** A profile that always exists is one that eventually names a
    /// configuration nobody ran.
    pub fn resolve(&self, intent: Intent, standard_context: u32) -> Option<&Configuration> {
        /// **The best median, not the best reading.** `generation` is one number from one
        /// sitting; `verdict.median` is what the configuration does.
        ///
        /// The fallback to `generation` is a safety net rather than a feature, and the
        /// comment here used to say otherwise — *"where a verdict was never taken the
        /// reading stands in, so an older card is still comparable"*. It cannot: `usable`
        /// calls an unrecorded verdict `Invalid` and excludes the row before this runs. That
        /// is the right behaviour and the sentence was describing a fallback nothing can
        /// reach; a test now holds the real answer.
        fn fastest(rows: Vec<&Configuration>) -> Option<&Configuration> {
            let rate = |it: &Configuration| it.verdict.median.unwrap_or(it.generation);
            let best = rows
                .iter()
                .map(|it| rate(it))
                .fold(f64::NEG_INFINITY, f64::max);
            /*
                **A practical tie goes to the configuration with less in it.**

                Measured 2026-09-01, one search produced 52.93, 53.37 and 53.4 across three ngram
                kinds — a 0.9% spread, inside the run-to-run variation of the machine that
                produced them. Picking the top of that is picking noise, and what it buys is a
                flag somebody now has to keep working. A profile is something run for hours.

                One percent, because it is roughly the between-process reproducibility this
                machine actually shows (1.2% across three fresh calibrations) — under that, the
                two numbers are not distinguishable by the instrument that produced them.
                Above it, speed wins outright and nothing here softens that.
            */
            const TIE: f64 = 0.01;
            rows.into_iter()
                .filter(|it| best <= 0.0 || rate(it) >= best * (1.0 - TIE))
                .min_by(|a, b| {
                    a.moving_parts().cmp(&b.moving_parts()).then_with(|| {
                        rate(b)
                            .partial_cmp(&rate(a))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                })
        }
        /*
            **One workload at a time.** A short question in a 64K window and 62K of prompt in the
            same window measured 48.5 and 41.9 tok/s on this card. Ranking them together would
            report a cliff between two rungs that is really the difference between two questions.

            The filled rows win where any exist: they are the ones that answer *how much
            conversation can this hold*, which is what every intent below is about.
        */
        let filled = self.has_a_filled_curve();
        let usable = || {
            self.usable()
                .filter(move |it| it.filled.is_some() == filled)
        };

        match intent {
            Intent::Custom => self.custom.as_ref(),
            // Stated once, here, rather than duplicated: Auto *is* Balanced today, and the day it
            // stops being it will be because a filter arrived, not because this was rewritten.
            Intent::Auto | Intent::Balanced => fastest(
                usable()
                    .filter(|it| it.context() >= standard_context)
                    .collect(),
            )
            // A machine that cannot hold the standard context at all still deserves a balanced
            // answer, and the honest one is the fastest thing that ran.
            .or_else(|| fastest(usable().collect())),
            Intent::Fast => fastest(usable().collect()),
            Intent::MaxQuality => fastest(usable().filter(|it| !it.approximates()).collect()),
            Intent::LongContext => {
                /*
                    **Only where a context was actually varied.**

                    A Standard search holds the window still on purpose — that is what stops a
                    configuration winning by giving half of it away — so every row shares one
                    context and this would resolve to whatever `FAST` resolved to, offered under
                    a name that promises something nobody measured.

                    Derived rather than declared: the day Context Scaling measures a ladder, this
                    starts answering with no change here.
                */
                let mut every: Vec<u32> = usable().map(Configuration::context).collect();
                every.sort_unstable();
                every.dedup();
                let most = *every.last()?;
                (every.len() > 1)
                    .then(|| fastest(usable().filter(|it| it.context() == most).collect()))
                    .flatten()
            }
        }
    }

    /// Every profile that has something behind it.
    pub fn profiles(&self, standard_context: u32) -> Vec<Profile> {
        let mut seen: Vec<Profile> = Vec::new();
        for intent in Intent::ALL {
            let Some(one) = self.resolve(intent, standard_context) else {
                continue;
            };
            seen.push(Profile {
                intent,
                name: intent.name().to_owned(),
                about: intent.about().to_owned(),
                // Two intents landing on one configuration is ordinary and honest — on a machine
                // where the fastest run is also the longest, FAST and LONG CONTEXT are the same
                // row. Hiding one would be tidier and would be a lie about what was measured.
                same_as: seen
                    .iter()
                    .find(|other| other.configuration == *one)
                    .map(|other| other.intent),
                configuration: one.clone(),
            });
        }
        seen
    }

    /// The one Epoch marks. `None` where nothing has been measured.
    pub fn recommended(&self, standard_context: u32) -> Option<&Configuration> {
        self.resolve(Intent::Balanced, standard_context)
    }

    /// How much better the recommended configuration is than the unconfigured one.
    ///
    /// **Both halves measured, or nothing.** An improvement quoted against a remembered number is
    /// a comparison of two afternoons.
    pub fn improvement(&self, standard_context: u32) -> Option<f64> {
        let before = self.baseline.as_ref()?.generation;
        let after = self.recommended(standard_context)?.generation;
        (before > 0.0).then(|| after / before - 1.0)
    }

    /// Whether this was measured on the machine asking.
    ///
    /// A card, a build and a runtime — the same three that make a benchmark comparable. A result
    /// from a different card is not wrong, it is about somewhere else, and offering it as a
    /// profile here would be the most convincing kind of invented gauge.
    pub fn taken_here(&self, gpu: &str, build: &str, runtime: &str) -> bool {
        self.gpu == gpu && self.build == build && self.runtime == runtime
    }
}

/// One intent, resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub intent: Intent,
    pub name: String,
    pub about: String,
    /// Where this resolved to the same configuration as an earlier intent.
    #[serde(default)]
    pub same_as: Option<Intent>,
    pub configuration: Configuration,
}

/// Every model's measured configurations, as they are written down.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Every {
    #[serde(default)]
    kept: std::collections::BTreeMap<String, Optimized>,
}

pub fn path(library: &std::path::Path) -> std::path::PathBuf {
    library.join("optimized.json")
}

impl Every {
    /// Never fails: an unreadable file is an empty memory, so a fresh optimisation is never lost
    /// to a stale one that will not parse.
    pub fn load(library: &std::path::Path) -> Self {
        std::fs::read_to_string(path(library))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, library: &std::path::Path) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(self).map_err(|why| why.to_string())?;
        if let Some(home) = path(library).parent() {
            std::fs::create_dir_all(home).map_err(|why| why.to_string())?;
        }
        std::fs::write(path(library), raw).map_err(|why| why.to_string())
    }

    /// Every model something has been measured for, by the name it was measured under.
    ///
    /// Exists so a caller holding a *different* spelling of one model can find the one this
    /// store knows — the character panel holds the name llama.cpp serves, `gemma4-12b`, and
    /// this file is keyed `gemma4:12b`. Reading Epoch's own small JSON costs nothing; walking
    /// the shelves to answer the same question costs eleven seconds of GGUF headers.
    pub fn models(&self) -> impl Iterator<Item = &str> {
        self.kept.keys().map(String::as_str)
    }

    pub fn of(&self, model: &str) -> Option<&Optimized> {
        self.kept.get(model)
    }

    pub fn set(&mut self, model: &str, one: Optimized) {
        self.kept.insert(model.to_owned(), one);
    }

    pub fn forget(&mut self, model: &str) {
        self.kept.remove(model);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A row measured with the window full, for the tests about mixing workloads.
    fn full(context: u32, rate: f64) -> Configuration {
        let mut it = one(context, Cache::F16, rate);
        // About 90% of the window, which is what the ladder aims at. The number a row keeps is
        // the server's own count; this fixture stands in for it.
        it.filled = Some(u64::from(context) * 9 / 10);
        it
    }

    #[test]
    fn a_short_prompt_and_a_full_window_are_never_ranked_against_each_other() {
        /*
            Measured 2026-09-01 on `gemma4-12b`, one 64K window: a short question answers at 48.5
            tok/s and 62K of prompt at 41.9. Both true. A curve holding both would report a cliff
            between two rungs that is really the difference between two questions.

            The filled rows win where any exist: they are the ones that answer *how much
            conversation can this hold*, which is what every intent is about.
        */
        let mixed = measured(vec![
            one(65_536, Cache::F16, 48.5),
            full(65_536, 41.9),
            full(32_768, 44.8),
        ]);
        assert!(mixed.has_a_filled_curve());
        assert_eq!(
            mixed.recommended(32_768).map(|it| it.generation),
            Some(44.8),
            "the 48.5 is about a different question and may not win",
        );
        assert!(mixed
            .profiles(32_768)
            .iter()
            .all(|it| it.configuration.filled.is_some()));
    }

    #[test]
    fn a_model_with_no_ladder_still_answers_from_the_short_rows() {
        // Every reading before the ladder existed is a short one, and nothing about them changed.
        let short = measured(vec![one(32_768, Cache::F16, 51.7)]);
        assert!(!short.has_a_filled_curve());
        assert_eq!(
            short.recommended(32_768).map(|it| it.generation),
            Some(51.7)
        );
    }

    #[test]
    fn a_ladder_is_what_makes_fast_and_long_context_differ() {
        /*
            The reason the ladder is a separate button rather than a nicety. With one context
            measured, FAST, BALANCED and LONG CONTEXT are one row wearing three names; with a
            curve they are three answers.
        */
        let curve = measured(vec![
            full(16_384, 47.0),
            full(32_768, 44.8),
            full(65_536, 41.9),
        ]);
        assert_eq!(
            curve
                .resolve(Intent::Fast, 32_768)
                .map(Configuration::context),
            Some(16_384),
        );
        assert_eq!(
            curve
                .resolve(Intent::Balanced, 32_768)
                .map(Configuration::context),
            Some(32_768),
            "balanced holds the standard window",
        );
        assert_eq!(
            curve
                .resolve(Intent::LongContext, 32_768)
                .map(Configuration::context),
            Some(65_536),
        );
        assert!(!oneish(&curve), "three different answers, so three cards");
    }

    /// Whether every intent landed on one configuration — the Engine's half of what the deck
    /// collapses.
    fn oneish(found: &Optimized) -> bool {
        let shown = found.profiles(32_768);
        shown.len() > 1 && shown.iter().skip(1).all(|it| it.same_as.is_some())
    }

    /// Speculation of one kind, so a row can differ in how much is switched on.
    fn drafting(kind: &str, rate: f64) -> Configuration {
        let mut it = one(32_768, Cache::F16, rate);
        it.tuning.speculation.kind = kind.to_owned();
        it.verdict = crate::models::health::judge(&[rate], 0, 0);
        it
    }

    #[test]
    fn a_practical_tie_goes_to_the_simpler_configuration() {
        /*
            Measured 2026-09-01: one search produced 52.93, 53.37 and 53.40 across three ngram
            kinds — 0.9% apart, inside the run-to-run variation of the machine that produced them.
            Picking the top of that is picking noise, and what it buys is a flag somebody has to
            keep working for as long as the profile is in force.
        */
        let plain = one(32_768, Cache::F16, 53.10);
        let fancy = drafting("ngram-mod", 53.40);
        assert_eq!(plain.moving_parts(), 0);
        assert_eq!(fancy.moving_parts(), 1);

        let found = measured(vec![fancy.clone(), plain.clone()]);
        let picked = found.recommended(32_768).expect("something was measured");
        assert_eq!(
            picked.generation, 53.10,
            "0.6% is not a difference this machine can see"
        );
    }

    #[test]
    fn a_real_difference_still_wins_outright() {
        // Nothing here softens speed. The tie-break only runs inside the band where the two
        // numbers are indistinguishable.
        let plain = one(32_768, Cache::F16, 46.8);
        let fancy = drafting("ngram-mod", 53.4);
        let found = measured(vec![plain, fancy]);
        assert_eq!(
            found.recommended(32_768).map(|it| it.generation),
            Some(53.4)
        );
    }

    #[test]
    fn among_equally_simple_rows_the_fastest_still_wins() {
        let slower = drafting("ngram-cache", 53.10);
        let faster = drafting("ngram-mod", 53.40);
        let found = measured(vec![slower, faster]);
        assert_eq!(
            found
                .recommended(32_768)
                .map(|it| it.tuning.speculation.kind.clone()),
            Some("ngram-mod".to_owned()),
            "same moving parts, so the measurement decides",
        );
    }

    /// The measurement that produced the rule, replayed through the thing that acts on it.
    mod what_is_offered_and_what_is_only_named {
        use super::*;

        /// One configuration's runs, as they were actually recorded.
        fn ran(rate: f64, runs: &[f64], spec: Option<&str>) -> Configuration {
            let mut it = one(32_768, Cache::F16, rate);
            it.verdict = crate::models::health::judge(runs, 0, 0);
            it.tuning.speculation.kind = spec.unwrap_or_default().to_owned();
            it
        }

        #[test]
        fn a_range_entirely_above_the_steady_one_is_offered() {
            /*
                `Qwen3.6-35B-A3B-UD-IQ4_XS`, 2026-09-02. Before this rule the search recommended
                48.6 and said nothing about the 52.4 it had measured with the model's own
                prediction head -- the reason that build was on the machine at all.
            */
            let mtp = ran(52.44, &[51.76, 52.44, 56.37], Some("draft-mtp"));
            let plain = ran(48.62, &[48.42, 48.62, 48.84], None);
            assert_eq!(mtp.verdict.state(), crate::models::health::State::Unstable);

            let found = measured(vec![plain, mtp]);
            let best = found
                .resolve(Intent::Fast, 32_768)
                .expect("something is offered");
            assert_eq!(
                best.tuning.speculation.kind, "draft-mtp",
                "the configuration whose slowest run beat the other's fastest",
            );
            assert!(
                found.set_aside().is_empty(),
                "nothing was set aside, so there is nothing to explain",
            );
        }

        #[test]
        fn a_range_that_overlaps_is_named_rather_than_offered() {
            // gemma4:12b's `ngram-mod`, same afternoon: 48.8-72.2 against a steady 48.96. It
            // might be half as fast again and it might be slightly slower, and nothing measured
            // says which.
            let wild = ran(48.85, &[48.80, 48.85, 72.20], Some("ngram-mod"));
            let steady = ran(48.96, &[48.90, 48.96, 49.02], Some("ngram-simple"));

            let found = measured(vec![steady, wild]);
            let best = found
                .resolve(Intent::Fast, 32_768)
                .expect("something is offered");
            assert_eq!(best.tuning.speculation.kind, "ngram-simple");

            let aside = found.set_aside();
            assert_eq!(aside.len(), 1, "the one that was not offered");
            assert_eq!(aside[0].0.tuning.speculation.kind, "ngram-mod");
            assert_eq!(aside[0].1, Aside::Varied);
        }

        #[test]
        fn every_reason_a_row_is_held_back_can_be_named() {
            /*
                **The point of the type.** Each of these was already excluded and each vanished
                without a word, so a person reading the deck could not tell a configuration that
                broke the machine from one that was simply never witnessed.
            */
            let mut collapsed = ran(60.0, &[59.0, 60.0, 61.0], Some("a"));
            collapsed.verdict.collapses = 1;
            let mut rewrote = ran(50.0, &[49.9, 50.0, 50.1], Some("b"));
            rewrote.same_answers = Some(false);
            let mut unwitnessed = ran(50.0, &[49.9, 50.0, 50.1], Some("c"));
            unwitnessed.bracket = None;
            let steady = ran(48.0, &[47.9, 48.0, 48.1], None);

            let found = measured(vec![steady, collapsed, rewrote, unwitnessed]);
            let mut said: Vec<Aside> = found.set_aside().into_iter().map(|it| it.1).collect();
            said.sort_by_key(|it| format!("{it:?}"));
            assert_eq!(
                said,
                vec![Aside::ChangedAnswers, Aside::Collapsed, Aside::Unwitnessed],
            );
        }

        #[test]
        fn a_collapse_is_never_promoted_by_being_fast() {
            // Dominance is about a spread, and a collapsed set has no honest spread. This row is
            // the fastest thing on the list and must stay off the offer.
            let mut collapsed = ran(90.0, &[89.0, 90.0, 91.0], Some("a"));
            collapsed.verdict.collapses = 2;
            let steady = ran(48.0, &[47.9, 48.0, 48.1], None);

            let found = measured(vec![steady, collapsed]);
            let best = found.resolve(Intent::Fast, 32_768).expect("the steady one");
            assert!(best.tuning.speculation.kind.is_empty());
        }
    }

    /// Which number a profile is chosen on.
    ///
    /// **Moved here from the deck**, which used to hold a copy of `resolve` and therefore a copy
    /// of these. There is one implementation now, so there is one place to assert about it.
    mod the_number_a_profile_is_chosen_on {
        use super::*;

        #[test]
        fn the_median_decides_rather_than_the_single_reading() {
            /*
                A configuration whose one recorded `generation` is higher but whose median is
                lower is not the faster one — it is the one that was measured on a better
                afternoon. Measured 2026-08-31: the same configuration answered 48.3, 44.2 and
                25.0 across three searches.
            */
            let mut lucky = one(32_768, Cache::F16, 48.0);
            lucky.verdict = crate::models::health::judge(&[40.0, 42.0, 48.0], 0, 0);
            let mut honest = one(32_768, Cache::F16, 45.0);
            honest.verdict = crate::models::health::judge(&[44.8, 45.0, 45.2], 0, 0);

            let found = measured(vec![lucky, honest]);
            let best = found
                .resolve(Intent::Balanced, 32_768)
                .expect("something is offered");
            assert_eq!(best.generation, 45.0);
        }

        #[test]
        fn a_row_with_no_verdict_is_not_offered_however_fast_it_read() {
            /*
                **Found by moving this test out of the deck, 2026-09-02.** The deck’s copy of
                `resolve` admitted a row with no verdict — its filter asked
                `stateOf !== "unstable"`, and `undefined` is not `"unstable"` — so the panel
                could mark a configuration that `use_profile` would then refuse to write. The
                Engine has always called an unrecorded verdict `Invalid`, which is the right
                answer: measurement is not eligibility, and a reading nobody characterised is not
                one to run a model on for hours.

                51.0 is the faster number here and it is not the answer.
            */
            let mut older = one(32_768, Cache::F16, 51.0);
            older.verdict = crate::models::health::Verdict::default();
            let mut newer = one(32_768, Cache::F16, 40.0);
            newer.verdict = crate::models::health::judge(&[39.8, 40.0, 40.2], 0, 0);

            let found = measured(vec![older, newer]);
            let best = found
                .resolve(Intent::Balanced, 32_768)
                .expect("something is offered");
            assert_eq!(best.generation, 40.0);
        }
    }

    fn one(context: u32, cache: Cache, rate: f64) -> Configuration {
        Configuration {
            loadout: Loadout { context, cache },
            tuning: Tuning::default(),
            offload: None,
            gpu_layers: None,
            generation: rate,
            prompt: None,
            first_token_ms: None,
            vram_used: None,
            ram_used: None,
            gpu_peak: None,
            gpu_mean: None,
            cpu_peak: None,
            cpu_mean: None,
            model_on_gpu: None,
            model_on_host: None,
            drafted: None,
            accepted: None,
            stable: true,
            verdict: crate::models::health::judge(&[rate], 0, 0),
            around: crate::models::health::Around::default(),
            same_answers: Some(true),
            filled: None,
            // Closed on both sides, because that is the ordinary case a fixture should
            // represent. The tests below take it away deliberately.
            bracket: Some(Bracket {
                before: Some("O-001".into()),
                before_held: Held::Reproduced,
                after: Some("O-002".into()),
                after_held: Held::Reproduced,
            }),
            at: 1,
        }
    }

    fn measured(rows: Vec<Configuration>) -> Optimized {
        Optimized {
            model: "m".to_owned(),
            gpu: "a card".to_owned(),
            build: "b1".to_owned(),
            runtime: "llama_cpp".to_owned(),
            baseline: Some(one(32_768, Cache::F16, 41.0)),
            tried: rows,
            custom: None,
            ladder: None,
            chosen: None,
            session: None,
            usable: true,
        }
    }

    #[test]
    fn a_model_nobody_measured_has_no_profiles_at_all() {
        /*
            Not four greyed-out ones and not four plausible ones. A catalogue with nothing behind
            it is not a cold instrument, it is an invented one — the same answer Styles arrived at
            for the same reason.
        */
        let empty = Optimized {
            baseline: None,
            ..measured(Vec::new())
        };
        assert!(empty.profiles(32_768).is_empty());
        assert_eq!(empty.recommended(32_768), None);
        assert_eq!(empty.improvement(32_768), None);
    }

    #[test]
    fn balanced_is_the_fastest_that_still_holds_the_standard_context() {
        // Faster at 16K is `FAST`; the recommendation must not quietly give away half the window.
        let it = measured(vec![
            one(16_384, Cache::Q8_0, 61.3),
            one(32_768, Cache::Q8_0, 52.8),
            one(32_768, Cache::F16, 43.1),
        ]);
        let balanced = it.recommended(32_768).expect("one qualifies");
        assert_eq!(balanced.generation, 52.8);
        assert_eq!(balanced.context(), 32_768);

        let fast = it.resolve(Intent::Fast, 32_768).expect("one qualifies");
        assert_eq!(fast.generation, 61.3);
    }

    #[test]
    fn max_quality_refuses_what_approximates_even_when_it_is_faster() {
        /*
            A compressed cache approximates the attention keys and values, and speculation that
            altered the text is a different model. Both are fine trades and neither is what
            somebody asking for maximum quality asked for.
        */
        let it = measured(vec![
            one(32_768, Cache::Q8_0, 52.8),
            one(32_768, Cache::F16, 43.1),
        ]);
        let best = it
            .resolve(Intent::MaxQuality, 32_768)
            .expect("one qualifies");
        assert_eq!(best.generation, 43.1);
        assert_eq!(best.loadout.cache, Cache::F16);
    }

    #[test]
    fn speculation_that_changed_nothing_does_not_count_against_quality() {
        /*
            Speculative decoding's whole claim is that the verifier only keeps a token it would
            have produced anyway. A configuration measured as writing identical text is exact, and
            charging it for quality would give away a free win for a reason nobody measured.
        */
        let mut fast = one(32_768, Cache::F16, 58.0);
        fast.tuning.speculation.kind = "draft-mtp".to_owned();
        fast.same_answers = Some(true);
        assert!(!fast.approximates());

        let mut different = fast.clone();
        different.same_answers = Some(false);
        assert!(different.approximates());

        let it = measured(vec![fast, one(32_768, Cache::F16, 43.1)]);
        assert_eq!(
            it.resolve(Intent::MaxQuality, 32_768)
                .map(|it| it.generation),
            Some(58.0),
        );
    }

    #[test]
    fn long_context_is_the_most_that_was_stable_and_then_the_fastest_of_those() {
        let it = measured(vec![
            one(32_768, Cache::F16, 52.8),
            one(131_072, Cache::Q8_0, 38.4),
            one(131_072, Cache::F16, 31.0),
        ]);
        let long = it
            .resolve(Intent::LongContext, 32_768)
            .expect("one qualifies");
        assert_eq!(long.context(), 131_072);
        assert_eq!(long.generation, 38.4, "the fastest of the longest");
    }

    #[test]
    fn a_candidate_is_only_recommendable_once_a_control_closed_it() {
        /*
            **Measurement is not eligibility.** A reading can be perfect — five runs, small spread,
            same answers — and still not be a thing to recommend, because nothing yet shows the
            machine was in the same state afterwards as it was before.

            The defect this prevents: the search pushed a candidate into its results and *then*
            took the closing control, so a candidate whose control failed was already eligible.
            Nothing downstream could see it — `usable` filters on the verdict, on stability and on
            whether the answers changed, and none of those knows what happened afterwards.
        */
        let good = one(32_768, Cache::F16, 52.1);
        // Four, not five: LONG CONTEXT needs a context to have been varied, and this is one row.
        assert_eq!(measured(vec![good.clone()]).profiles(32_768).len(), 4);

        // The same measurement, with a closing control that did not reproduce.
        let contaminated = Configuration {
            bracket: Some(Bracket {
                before: Some("O-021".into()),
                before_held: Held::Reproduced,
                after: Some("O-023".into()),
                after_held: Held::Failed,
            }),
            ..good.clone()
        };
        assert!(
            measured(vec![contaminated.clone()])
                .profiles(32_768)
                .is_empty(),
            "52.1 tok/s is a real reading and not a recommendation",
        );
        assert_eq!(
            contaminated.bracket.as_ref().unwrap().label(),
            "CONTAMINATED"
        );

        // Stopped or cancelled before the closing control ran: incomplete, and it does not claim
        // the candidate was slow. Nobody knows that.
        let incomplete = Configuration {
            bracket: Some(Bracket {
                before: Some("O-021".into()),
                before_held: Held::Reproduced,
                after: None,
                after_held: Held::Missing,
            }),
            ..good.clone()
        };
        assert!(measured(vec![incomplete.clone()])
            .profiles(32_768)
            .is_empty());
        assert_eq!(incomplete.bracket.as_ref().unwrap().label(), "INCOMPLETE");

        // And anything measured before brackets existed: unrecorded is not proof.
        let older = Configuration {
            bracket: None,
            ..good
        };
        assert!(measured(vec![older]).profiles(32_768).is_empty());
    }

    #[test]
    fn a_broken_control_contaminates_one_candidate_and_not_the_ones_already_closed() {
        /*
            The chain, and why the closing control of one candidate is the opening control of the
            next:

            ```text
            O-019 ✓ → candidate A → O-021 ✓ → candidate B → O-023 ✗ → stop
            ```

            A was closed by a control that held. A later collapse says nothing about a measurement
            that was already bracketed — what it says is that nothing after it may be trusted.
        */
        let a = Configuration {
            bracket: Some(Bracket {
                before: Some("O-019".into()),
                before_held: Held::Reproduced,
                after: Some("O-021".into()),
                after_held: Held::Reproduced,
            }),
            ..one(32_768, Cache::F16, 48.0)
        };
        let b = Configuration {
            bracket: Some(Bracket {
                before: Some("O-021".into()),
                before_held: Held::Reproduced,
                after: Some("O-023".into()),
                after_held: Held::Failed,
            }),
            ..one(32_768, Cache::Q8_0, 61.0)
        };
        let both = measured(vec![a.clone(), b.clone()]);

        // B is the faster row and it is not offered. A is.
        let fastest = both
            .resolve(Intent::Fast, 32_768)
            .expect("something is offered");
        assert!(
            (fastest.generation - 48.0).abs() < 0.01,
            "{}",
            fastest.generation
        );
        assert!(
            both.tried
                .iter()
                .any(|it| (it.generation - 61.0).abs() < 0.01),
            "B is kept as evidence, it is only not recommendable"
        );

        // The chain reads through: B's opening control is A's closing control.
        assert_eq!(
            a.bracket.as_ref().unwrap().after,
            b.bracket.as_ref().unwrap().before
        );
        assert!(a.bracket.as_ref().unwrap().describe().contains("O-019"));
        assert!(b
            .bracket
            .as_ref()
            .unwrap()
            .describe()
            .contains("did not reproduce"));
    }

    #[test]
    fn a_configuration_that_did_not_finish_is_never_offered() {
        // Fastest on paper and it crashed a run. A profile is something somebody will press.
        let mut broken = one(32_768, Cache::F16, 90.0);
        broken.stable = false;
        let it = measured(vec![broken, one(32_768, Cache::F16, 43.1)]);
        assert_eq!(it.recommended(32_768).map(|it| it.generation), Some(43.1));
    }

    #[test]
    fn two_intents_landing_on_one_row_say_so_rather_than_hiding_one() {
        /*
            On a machine where the fastest run is also the longest, FAST and LONG CONTEXT are the
            same configuration. Hiding one would be tidier and would misdescribe the measurement.
        */
        let it = measured(vec![
            one(32_768, Cache::F16, 52.8),
            one(16_384, Cache::F16, 51.0),
        ]);
        let seen = it.profiles(32_768);
        assert_eq!(seen.len(), Intent::ALL.len(), "every intent resolved");
        assert_eq!(seen[0].intent, Intent::Auto);
        assert_eq!(seen[1].same_as, Some(Intent::Auto), "Balanced is Auto here");
        assert!(seen.iter().skip(1).all(|it| it.same_as.is_some()));
    }

    #[test]
    fn long_context_is_not_offered_where_no_context_was_varied() {
        /*
            A Standard search holds the window still on purpose. Offering LONG CONTEXT over rows
            that all share one context would promise a measurement nobody took — and it would
            resolve to whatever FAST resolved to, wearing a different name.
        */
        let held = measured(vec![
            one(32_768, Cache::F16, 52.8),
            one(32_768, Cache::Q8_0, 51.0),
        ]);
        assert_eq!(held.resolve(Intent::LongContext, 32_768), None);
        assert!(!held
            .profiles(32_768)
            .iter()
            .any(|it| it.intent == Intent::LongContext));

        // And it starts answering the moment a ladder is measured, with no change here.
        let laddered = measured(vec![
            one(32_768, Cache::F16, 52.8),
            one(65_536, Cache::F16, 48.0),
        ]);
        assert_eq!(
            laddered
                .resolve(Intent::LongContext, 32_768)
                .map(Configuration::context),
            Some(65_536),
        );
    }

    #[test]
    fn an_improvement_needs_both_halves_measured() {
        // Quoting one against a remembered number compares two afternoons.
        let it = measured(vec![one(32_768, Cache::Q8_0, 52.8)]);
        let better = it.improvement(32_768).expect("both halves");
        assert!((better - 0.2878).abs() < 0.001, "{better}");

        let no_baseline = Optimized {
            baseline: None,
            ..it
        };
        assert_eq!(no_baseline.improvement(32_768), None);
    }

    #[test]
    fn a_machine_that_cannot_reach_the_standard_context_still_gets_an_answer() {
        // Otherwise a small card would have a FAST profile and no recommendation, which reads as
        // *Epoch has nothing to suggest* about a model it measured perfectly well.
        let it = measured(vec![one(8_192, Cache::Q8_0, 22.0)]);
        assert_eq!(it.recommended(32_768).map(|it| it.context()), Some(8_192));
    }

    #[test]
    fn a_result_from_another_card_is_not_a_fact_about_this_one() {
        let it = measured(Vec::new());
        assert!(it.taken_here("a card", "b1", "llama_cpp"));
        assert!(!it.taken_here("another card", "b1", "llama_cpp"));
        assert!(
            !it.taken_here("a card", "b2", "llama_cpp"),
            "a build changes kernels"
        );
    }

    #[test]
    fn what_was_written_reads_back() {
        let here = std::env::temp_dir().join(format!("epoch-profiles-{}", std::process::id()));
        let mut every = Every::default();
        every.set("m", measured(vec![one(32_768, Cache::F16, 52.8)]));
        every.save(&here).expect("saved");
        let read = Every::load(&here);
        assert_eq!(read.of("m").map(|it| it.tried.len()), Some(1));
        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    fn an_unreadable_file_is_an_empty_memory_and_not_a_refusal() {
        let here = std::env::temp_dir().join(format!("epoch-profiles-bad-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&here);
        std::fs::write(path(&here), "{ not json").expect("written");
        assert!(Every::load(&here).of("m").is_none());
        let _ = std::fs::remove_dir_all(&here);
    }
}
