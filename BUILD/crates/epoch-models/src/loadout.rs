//! The best way to load one model on one machine, found by trying.
//!
//! ## Why this is a search and not a formula
//!
//! Two knobs decide almost everything about how fast a local model answers: how much context it
//! is loaded with, and whether its KV cache is kept at `f16` or quantised. Measured on an
//! RTX 4070 SUPER, same prompt, warm:
//!
//! ```text
//!                       f16 KV    q8_0 KV
//! gemma4-12b   (7.4 GB)   46.5       45.0
//! Qwen3.8-27B (10.6 GB)   20.0       33.7
//!
//!                       16k ctx   32k ctx   64k ctx
//! gemma4-12b              46.1      46.6      46.8
//! Qwen3.8-27B             33.8      18.0      does not fit
//! ```
//!
//! **The right answer is opposite for the two models on the same card.** One global setting has
//! to pick a loser: Epoch's did, and it was costing gemma four times its context and the 27B
//! two thirds of its speed, in the same command.
//!
//! A formula was the obvious alternative and it does not survive contact. The KV cache size is
//! computable from a GGUF header for ordinary attention, and `gemma4` uses a sliding window on
//! five layers out of six — computed naively it wants 12.9 GB where it really takes about two.
//! The next architecture will break the next formula. `whichllm` models this from a table of
//! card bandwidths and is 19% low here for a model that fits and three times out for one that
//! does not.
//!
//! So Epoch does what it already does for pictures ([`crate::recipes`]) and for speed
//! ([`crate::speeds`]): **it measures, and it remembers.** Eight loads, once, in about four
//! minutes — measured end to end at 230 s for gemma and 285 s for the 27B.
//!
//! ## And Epoch does not choose from it
//!
//! It maps the curve, marks the balanced row, and the user picks. Context against speed is a
//! **preference**, not a fact: the 27B's 24k row costs a third of its speed and is exactly what
//! somebody writing long documents wants. An earlier version pruned — it stopped climbing at the
//! first loss — which is right when Epoch decides and throws away the rows a choice is made
//! from.
//!
//! Rows that cannot hold a real turn are measured and **marked**, never hidden: 8k is genuinely
//! the fastest and genuinely stops mid-sentence, and saying both is the Studio Panel's rule
//! (greyed, explained, and still selectable — ADR-0033).
//!
//! ## It belongs to the model *and* the machine
//!
//! ADR-0026's test — *does this survive changing the engine?* — says no twice over. A loadout
//! names a context that fits **this card**, so a recipe found on a 12 GB card is a lie on a
//! 24 GB one. The key is therefore the model's hash **and** the card, and a new card means the
//! question is unanswered rather than answered badly. That matters most exactly when somebody
//! upgrades, which is when a stale recipe would quietly cap their new hardware.
//!
//! ## What it never does
//!
//! It does not block. A model is usable the moment it downloads, on the conservative loadout;
//! the search runs afterwards and replaces it. Six minutes of an unusable model is worse than a
//! model nobody optimised.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// How the KV cache is kept.
///
/// Only these two. `q4` cache is where quantising is known to be felt, and this search exists to
/// win speed rather than to trade away answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cache {
    F16,
    Q8_0,
}

impl Cache {
    /// What `llama-server` is told, or nothing at all for its own default.
    pub fn flags(self) -> &'static str {
        match self {
            Cache::F16 => "",
            Cache::Q8_0 => "--cache-type-k q8_0 --cache-type-v q8_0",
        }
    }

    /// What a record calls it. Stable across releases, because a fingerprint written today has
    /// to still mean the same thing when it is read back next month.
    pub fn id(self) -> &'static str {
        match self {
            Cache::F16 => "f16",
            Cache::Q8_0 => "q8_0",
        }
    }

    pub fn plainly(self) -> &'static str {
        match self {
            Cache::F16 => "full-precision cache",
            Cache::Q8_0 => "compressed cache",
        }
    }
}

/// One way of loading a model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Loadout {
    pub context: u32,
    pub cache: Cache,
}

/// What happened when one loadout was run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Run {
    pub tokens_per_second: f64,
}

/// Something that can load a model a given way and time one answer.
///
/// A trait so the search is testable without a graphics card: the decisions this makes are the
/// part worth holding still, and they are decisions about numbers.
pub trait Probe {
    /// `None` when the model did not load at all — too big, or an architecture this build does
    /// not know. Either way there is nothing to time and nothing to compare.
    fn run(&mut self, one: Loadout) -> Option<Run>;
}

/// Whether the cache type is one of the things a curve can vary here.
///
/// **Measured per runtime, 2026-08-30.** llama.cpp takes `--cache-type-k/v` on the child it
/// spawns, so both can be run and compared. Ollama reads `OLLAMA_KV_CACHE_TYPE` once when the
/// *server* starts and ignores a per-request `cache_type_k` silently — 8.39 GB with it and 8.39
/// GB without, byte for byte — so a curve there maps context only, against whatever the running
/// server was started with. LM Studio cannot be told at all: no flag on `lms load`, no load route
/// in its API.
///
/// Running eight rows on a runtime that can be told four of them would be four invented
/// readings, and a curve is only worth having if every row is a thing that happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caches {
    /// Both, and the curve decides which.
    Either,
    /// One, because this runtime cannot be told per run. The curve says which it ran under.
    Only(Cache),
}

/// One loadout that was tried, kept so the choice can be explained rather than asserted.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tried {
    #[serde(flatten)]
    pub loadout: Loadout,
    /// `None` when it did not load.
    pub tokens_per_second: Option<f64>,
}

/// What this machine settled on for one model, and everything it tried on the way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Best {
    /// Which file this is, on this machine.
    ///
    /// **Not a hash, deliberately, and the reason is the read path.** `recipes` keys on sha256
    /// because a picture's provenance is a claim about *what a model is*, which is ADR-0024's
    /// question and which a filename may never answer. This is a different question — *have I
    /// measured this exact file here* — and the answer only has to survive on one disk.
    ///
    /// Hashing costs 3.4 s for a 6.2 GB model on this machine, and the MODELS deck lists six of
    /// them: twenty seconds every time somebody opens it, for a key that is no more correct in
    /// practice. ADR-0032's amendment already draws this line — hash after a render that cost
    /// tens of seconds, never on a path somebody is waiting on.
    ///
    /// So it is the size and the name together. Two different GGUFs on one machine sharing both
    /// is not a thing that happens; a file that changed size, or moved, or was renamed, simply
    /// reads as unmeasured, which costs a search and never a wrong answer.
    pub model: String,
    /// What to call it on screen.
    pub name: String,
    /// The card this was measured on. A different one means unanswered, not answered.
    pub card: String,
    /// The llama.cpp build the readings were taken with.
    ///
    /// The card is not the whole machine. A new build changes kernels, and a release that made
    /// something faster would go unused until somebody re-measured by hand — the system ageing
    /// quietly, which is the one failure mode a remembered measurement really has.
    #[serde(default)]
    pub build: String,
    /// Which runtime the readings were taken through.
    ///
    /// **A curve is about a model *on a runtime*, not about a model.** The same GGUF answers at
    /// different rates through llama.cpp, Ollama and LM Studio on one card — 8.5, 10.5 and 19.8
    /// seconds for one short answer, measured — and only llama.cpp can be told a cache type at
    /// all. Keeping them apart is what stops the second measurement quietly replacing the first
    /// with an answer about a different program.
    ///
    /// Absent in files written before this existed, and those were all llama.cpp.
    #[serde(default = "was_llama_cpp")]
    pub runtime: String,
    pub chose: Loadout,
    /// The largest context that actually **loaded** during the search.
    ///
    /// **Not the recommended one, and the difference matters.** `chose` is the best trade of
    /// speed against room; this is the wall. Above it the model did not load at all here —
    /// measured, not inferred — which is the one thing worth telling a turn that is sizing its
    /// own window, because asking for more than this does not get a slower answer, it gets none.
    ///
    /// Zero in files written before this existed, and zero means *unmeasured*: nothing is capped.
    #[serde(default)]
    pub most: u32,
    pub tokens_per_second: f64,
    #[serde(default)]
    pub tried: Vec<Tried>,
    pub at: u64,
}

/// Everything this installation has worked out.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Loadouts {
    #[serde(default)]
    kept: Vec<Best>,
}

pub fn path(library: &Path) -> PathBuf {
    library.join("loadouts.json")
}

/// What names one file on this machine, cheaply enough to be asked on a read path.
///
/// `None` when the file is not there — which is not an identity, and reads as one.
/// Every loadout written before a curve could run anywhere else was taken through llama.cpp.
fn was_llama_cpp() -> String {
    "llama_cpp".to_owned()
}

pub fn names(file: &Path) -> Option<String> {
    let bytes = std::fs::metadata(file).ok()?.len();
    let stem = file.file_stem()?.to_string_lossy();
    Some(format!("{bytes}:{stem}"))
}

impl Loadouts {
    /// Never fails: an unreadable file is an empty memory, and nobody's model should refuse to
    /// start over a stray brace in a file of advice.
    pub fn load(library: &Path) -> Self {
        std::fs::read_to_string(path(library))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, library: &Path) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(self).map_err(|why| why.to_string())?;
        if let Some(dir) = path(library).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path(library), body).map_err(|why| why.to_string())
    }

    /// What was found for this model **on this card**.
    ///
    /// The card is part of the question, not a detail of the answer. A loadout found on a 12 GB
    /// card describes a context that fitted there, and returning it for a 24 GB card would cap
    /// the better machine at the worse one's limit — silently, and precisely on the day somebody
    /// upgraded.
    pub fn about<'a>(&'a self, model: &str, card: &str, build: &str) -> Option<&'a Best> {
        self.kept
            .iter()
            .find(|one| one.model == model && one.card == card && one.build == build)
    }

    /// Keep one, replacing any earlier answer for the same model on the same card.
    pub fn remember(&mut self, best: Best) {
        self.kept.retain(|one| {
            !(one.model == best.model
                && one.card == best.card
                && one.build == best.build
                && one.runtime == best.runtime)
        });
        self.kept.push(best);
    }

    pub fn all(&self) -> &[Best] {
        &self.kept
    }

    /// The largest context measured to load for this model through this runtime, on this
    /// installation.
    ///
    /// **No card in the question**, unlike [`Loadouts::ceiling_for`], because this is read on
    /// every turn and asking which card this is costs a `nvidia-smi`. The file belongs to one
    /// installation, so a curve from a different card means the card was swapped — and the worst
    /// that does is cap a turn lower than the new one needs.
    pub fn ceiling_here(&self, name: &str, runtime: &str) -> Option<u32> {
        self.kept
            .iter()
            .filter(|one| one.name == name && one.runtime == runtime && one.most > 0)
            .map(|one| one.most)
            .max()
    }

    /// The largest context measured to load for this model, on this card, through this runtime.
    ///
    /// **All three have to match, and `None` is the ordinary answer.** A curve taken on another
    /// card, or through another program, is a measurement of something else — and an unmeasured
    /// model is not capped at all, because a cap invented from nothing is the gauge with nothing
    /// behind it, applied to somebody's turn.
    pub fn ceiling_for(&self, name: &str, card: &str, runtime: &str) -> Option<u32> {
        self.kept
            .iter()
            .find(|one| {
                one.name == name && one.card == card && one.runtime == runtime && one.most > 0
            })
            .map(|one| one.most)
    }

    /// Forget everything measured on one card.
    ///
    /// For the day a card is replaced: the answers are not wrong so much as about a machine that
    /// no longer exists, and leaving them would answer a question nobody asked again.
    pub fn forget_card(&mut self, card: &str) {
        self.kept.retain(|one| one.card != card);
    }
}
/// How close to the fastest a bigger context must stay to be worth taking.
///
/// The two cases are nowhere near each other, which is why the exact number does not matter:
/// gemma gained context for nothing (46.50 → 46.55 → 46.52 across 16k, 32k, 64k, which is noise)
/// while the 27B paid a third for the first step (33.68 → 22.21). Anything from two to twenty
/// percent separates them; five is the middle of that.
const WORTH_IT: f64 = 0.95;

/// How much faster the compressed cache must be to be worth choosing.
///
/// It is not free — `q8_0` cost gemma 3% (46.5 against 44.6, measured by the search itself) for
/// a cache it never needed — so `f16` wins ties and near-ties. On the model that needs it there
/// is no argument to have: 33.7 against 20.2.
const CACHE_WORTH_IT: f64 = 1.05;

/// The largest context worth exploring to.
///
/// Not a limit on what a model can do — a limit on what Epoch will spend a machine's memory on
/// unasked. Past it the memory is better spent on the model, and a user who wants more can say
/// so.
pub const CEILING: u32 = 65_536;

/// The context sizes tried, smallest first.
///
/// Steps of 8192 up to 32k and then coarser, because that is where the interesting part is: the
/// 27B lost a third between 16k and 24k and another fifth by 32k, while gemma was flat across
/// the whole range. Fine where models differ, coarse where they do not.
/// Public since EpochServices gained a search of its own: a second consumer needs the top rung
/// to size a model with no trained context in its header, and writing that number twice is how
/// the two would eventually disagree.
pub const LADDER: [u32; 6] = [8_192, 16_384, 24_576, 32_768, 49_152, 65_536];

/// How big a real turn is, in tokens.
///
/// **Measured, not assumed, and it has already moved once.** `ROUTER_CONTEXT` was set to 16384
/// because a turn was measured at 8235 tokens. Re-measured 2026-08-26 through the model's own
/// tokenizer, against the turns Epoch actually recorded:
///
/// ```text
/// vault/last-turn.json          11789 tokens
/// vault/last-bridge-turn.json   13209 tokens
/// ```
///
/// Turns grew — two agent accounts and two MCP servers' worth of tool declarations — and nothing
/// re-took the measurement. A constant that describes the product as it was a month ago is the
/// stale-gauge failure in a different costume, so this one says when it was taken.
pub const A_REAL_TURN: u32 = 13_209;

/// Room for the model's answer on top of the turn that prompted it.
///
/// A loadout that holds the prompt exactly is one that stops mid-sentence, which reads as the
/// model being broken rather than as the context being tight.
pub const ROOM_TO_ANSWER: u32 = 2_048;

// **A guard on the constant, not on a run.** These were `assert!` inside tests, which clippy
// reads correctly as assertions that can never fail: both sides are known at compile time. That
// is not a reason to delete them -- they exist so that changing the number breaks something --
// it is a reason to make them what they were trying to be. A `const` assertion fails the
// **build**, which is stricter than failing a test and arrives at whoever changed the number
// rather than at whoever ran the suite next.

// The floor is a measurement of the product and it says when it was taken: smaller than
// `A_REAL_TURN` is smaller than the turn measured through the model's own tokenizer, and a
// prompt that fits exactly stops mid-sentence.
const _: () = assert!(A_REAL_TURN >= 13_209);
const _: () = assert!(ROOM_TO_ANSWER > 0);

/// One loadout, what it did, and whether it can actually be used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    pub loadout: Loadout,
    /// `None` when it did not load at all.
    pub tokens_per_second: Option<f64>,
    /// Whether it can hold a real turn and still leave the model room to answer.
    ///
    /// A `false` here is **not** a reason to hide the row. It is a reason to say what is wrong
    /// with it, exactly as the Studio Panel greys an incompatible part and still offers
    /// `USE IT ANYWAY` — Epoch measured and said; the card is the user's.
    pub holds_a_turn: bool,
}

/// Everything one model did on this card, and what Epoch would pick.
#[derive(Debug, Clone, PartialEq)]
pub struct Map {
    pub readings: Vec<Reading>,
}

impl Map {
    /// The largest context that loaded. `0` when nothing did.
    pub fn most_that_loaded(&self) -> u32 {
        self.readings
            .iter()
            .filter(|r| r.tokens_per_second.is_some())
            .map(|r| r.loadout.context)
            .max()
            .unwrap_or(0)
    }

    /// Whether anything loaded at all.
    pub fn loaded(&self) -> bool {
        self.readings.iter().any(|r| r.tokens_per_second.is_some())
    }

    /// The one Epoch marks: **the largest context that costs no real speed.**
    ///
    /// ## Not simply the fastest, and this was found by writing it that way first
    ///
    /// *Fastest that holds a turn* sounds right and is not. gemma's curve is flat — 46.50 at
    /// 16k, 46.55 at 32k, 46.52 at 64k — so the fastest row wins by **0.03 tok/s**, and that is
    /// measurement noise choosing between 32k and 64k of conversation. Whichever way the noise
    /// falls, one of them is thrown away for nothing.
    ///
    /// So speed comes first and ties go to context: find the best rate, then take the biggest
    /// context still within [`WORTH_IT`] of it. gemma collects the whole ceiling; the 27B stays
    /// at 16k, because 24k really does cost a third and is nowhere near the tolerance.
    ///
    /// A recommendation rather than a decision — every other reading stays selectable. But there
    /// is always one, because a table of eight rows with nothing marked is the failure
    /// `CLAUDE.md` names as the hardest to see: correct, present, and unusable.
    ///
    /// `None` only when nothing loaded, or when nothing that loaded can hold a turn — which is a
    /// true and useful thing to say about a model on a card that cannot really run it.
    pub fn recommended(&self) -> Option<Loadout> {
        let usable: Vec<(Loadout, f64)> = self
            .readings
            .iter()
            .filter(|r| r.holds_a_turn)
            .filter_map(|r| r.tokens_per_second.map(|rate| (r.loadout, rate)))
            .collect();
        let fastest = usable
            .iter()
            .map(|(_, rate)| *rate)
            .fold(f64::NEG_INFINITY, f64::max);
        usable
            .into_iter()
            .filter(|(_, rate)| *rate >= fastest * WORTH_IT)
            // Context first, then speed. Both caches reach the top rung, so without the second
            // key the tie is broken by whichever was measured last — and gemma got the
            // compressed cache it does not need, at 45.0 against 46.5.
            .max_by(|a, b| {
                a.0.context
                    .cmp(&b.0.context)
                    .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            })
            .map(|(one, _)| one)
    }

    /// What the recommended loadout answered at.
    pub fn recommended_rate(&self) -> Option<f64> {
        let one = self.recommended()?;
        self.readings
            .iter()
            .find(|r| r.loadout == one)?
            .tokens_per_second
    }
}

/// Map what one model can do on this card, so somebody can choose.
///
/// ## Why this explores rather than searches
///
/// An earlier version stopped climbing the moment a bigger context cost speed, and returned one
/// answer. That is the right shape when Epoch decides — and Epoch does not decide this. Context
/// against speed is a **preference**: somebody writing long documents wants the 32k row that
/// costs a fifth, and somebody having quick conversations wants the fastest one. A search that
/// prunes has thrown away the rows the choice is made from.
///
/// So it maps, Epoch marks one, and the user picks.
///
/// ## What it does not measure, and why that is not a missing option
///
/// The cache question is asked once, at the base of the ladder, and the winner carries upward.
/// The loser is re-tried at the top of what loaded — so the trade is visible at both ends — and
/// nowhere in between. Memory pressure only grows with context, so a compressed cache that was
/// not worth it at 16k cannot become worth it at 24k and then stop being worth it again at 32k.
/// Probing the middle of that would cost minutes to re-measure a line with no bend in it.
///
/// `ceiling` is the model's own trained context when that is smaller than [`CEILING`]: asking a
/// model for more than it was trained for buys nothing.
/// The most settings a search can try, given a ceiling.
///
/// **A bound, not a plan.** `explore` walks the ladder adaptively and may stop early, so this is
/// the largest honest number to draw a bar against: every rung the ceiling allows, plus the
/// second cache tried at the base. A bar that fills faster than expected has told the truth the
/// whole way; one drawn against a guessed total has not.
pub fn how_many(ceiling: u32, caches: Caches) -> usize {
    let ceiling = ceiling.max(LADDER[0]);
    let rungs = LADDER.iter().filter(|rung| **rung <= ceiling).count();
    match caches {
        // The ladder, plus the two that decide the cache and the one that shows the trade at
        // the top. One of the deciding pair is a rung, so it is counted once.
        Caches::Either => rungs + 1,
        // Just the ladder. Nothing to compare.
        Caches::Only(_) => rungs,
    }
}

pub fn explore(ceiling: u32, caches: Caches, probe: &mut dyn Probe) -> Map {
    let ceiling = ceiling.max(LADDER[0]);
    let needs = A_REAL_TURN.saturating_add(ROOM_TO_ANSWER);
    let mut readings: Vec<Reading> = Vec::new();

    let take = |probe: &mut dyn Probe, readings: &mut Vec<Reading>, one: Loadout| -> Option<f64> {
        if readings.iter().any(|r| r.loadout == one) {
            return readings
                .iter()
                .find(|r| r.loadout == one)
                .and_then(|r| r.tokens_per_second);
        }
        let got = probe.run(one).map(|r| r.tokens_per_second);
        readings.push(Reading {
            loadout: one,
            tokens_per_second: got,
            holds_a_turn: one.context >= needs,
        });
        got
    };

    // **A runtime that cannot be told is not asked twice.** With one cache there is nothing to
    // decide, so the whole comparison is skipped and the ladder runs under the cache the server
    // is actually holding — which the caller knows and this cannot.
    if let Caches::Only(cache) = caches {
        for rung in LADDER.iter().copied().filter(|rung| *rung <= ceiling) {
            take(
                probe,
                &mut readings,
                Loadout {
                    context: rung,
                    cache,
                },
            );
        }
        return Map { readings };
    }

    // The base is the smallest rung that can hold a turn, so the cache is decided on a loadout
    // somebody would really use rather than on one that is too small to be an option.
    let base = LADDER
        .iter()
        .copied()
        .find(|rung| *rung >= needs && *rung <= ceiling)
        .unwrap_or(ceiling);

    let full = take(
        probe,
        &mut readings,
        Loadout {
            context: base,
            cache: Cache::F16,
        },
    );
    let squeezed = take(
        probe,
        &mut readings,
        Loadout {
            context: base,
            cache: Cache::Q8_0,
        },
    );

    let winner = match (full, squeezed) {
        (Some(f), Some(q)) if q > f * CACHE_WORTH_IT => Cache::Q8_0,
        (Some(_), _) => Cache::F16,
        (None, Some(_)) => Cache::Q8_0,
        (None, None) => Cache::Q8_0,
    };
    let loser = match winner {
        Cache::F16 => Cache::Q8_0,
        Cache::Q8_0 => Cache::F16,
    };

    // Every rung, up and down, so the whole curve is there to choose from. Rungs below the base
    // are measured too: they are faster, they cannot hold a turn, and saying both is more useful
    // than pretending they do not exist.
    for rung in LADDER.iter().copied().filter(|rung| *rung <= ceiling) {
        take(
            probe,
            &mut readings,
            Loadout {
                context: rung,
                cache: winner,
            },
        );
    }

    // The other cache at the top of what actually loaded, so the trade is visible where it is
    // largest as well as where it was decided.
    if let Some(top) = readings
        .iter()
        .filter(|r| r.tokens_per_second.is_some() && r.loadout.cache == winner)
        .map(|r| r.loadout.context)
        .max()
    {
        if top != base {
            take(
                probe,
                &mut readings,
                Loadout {
                    context: top,
                    cache: loser,
                },
            );
        }
    }

    readings.sort_by_key(|r| (r.loadout.context, r.loadout.cache == Cache::Q8_0));
    Map { readings }
}

/// What to load with before anything has been measured.
///
/// **Deliberately the safe half of every trade**, because this is what somebody gets in the
/// minutes between a download finishing and a search running. The compressed cache costs a model
/// that did not need it three percent; the full one costs a model that did need it two thirds.
/// The floor rather than a guess at more, for the same reason.
pub fn conservative(floor: u32) -> Loadout {
    Loadout {
        context: floor,
        cache: Cache::Q8_0,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_bar_is_drawn_against_a_bound_the_search_cannot_exceed() {
        // Every rung the ceiling allows, plus the second cache at the base. A search that stops
        // early fills it faster; nothing can overflow it.
        assert_eq!(
            super::how_many(super::CEILING, super::Caches::Either),
            super::LADDER.len() + 1
        );
        assert_eq!(super::how_many(16_384, super::Caches::Either), 3);
        // Below the first rung is still one rung: `explore` clamps up, and so does this.
        assert_eq!(super::how_many(1, super::Caches::Either), 2);
    }

    use super::*;

    /// A machine that answers from a table, so the *decisions* can be held still without a card.
    struct Fake {
        /// `(context, cache) -> tok/s`, and absent means it did not load.
        answers: Vec<(u32, Cache, f64)>,
        asked: Vec<Loadout>,
    }

    impl Probe for Fake {
        fn run(&mut self, one: Loadout) -> Option<Run> {
            self.asked.push(one);
            self.answers
                .iter()
                .find(|(ctx, cache, _)| *ctx == one.context && *cache == one.cache)
                .map(|(_, _, rate)| Run {
                    tokens_per_second: *rate,
                })
        }
    }

    /// gemma4-12b's real numbers, taken by the search itself on 2026-08-26.
    fn gemma() -> Fake {
        Fake {
            answers: vec![
                (8_192, Cache::F16, 46.6),
                (16_384, Cache::F16, 46.50),
                (16_384, Cache::Q8_0, 44.56),
                (24_576, Cache::F16, 46.5),
                (32_768, Cache::F16, 46.55),
                (49_152, Cache::F16, 46.5),
                (65_536, Cache::F16, 46.52),
                (65_536, Cache::Q8_0, 45.0),
            ],
            asked: Vec::new(),
        }
    }

    /// Qwen3.8-27B's real numbers: the card is full, so the trade runs the other way.
    fn big() -> Fake {
        Fake {
            answers: vec![
                (8_192, Cache::Q8_0, 34.0),
                (16_384, Cache::F16, 20.20),
                (16_384, Cache::Q8_0, 33.68),
                (24_576, Cache::Q8_0, 22.21),
                (32_768, Cache::Q8_0, 17.95),
                (32_768, Cache::F16, 12.0),
            ],
            asked: Vec::new(),
        }
    }

    /// A model with room keeps its precision and is recommended the whole ceiling.
    #[test]
    fn a_model_with_room_is_recommended_its_largest_context() {
        let map = explore(65_536, Caches::Either, &mut gemma());
        let best = map.recommended().expect("something holds a turn");
        assert_eq!(best.cache, Cache::F16, "3% is not worth a trade");
        assert_eq!(best.context, 65_536);
    }

    /// A model that fills the card compresses, and stays where it is fastest.
    #[test]
    fn a_model_that_fills_the_card_is_recommended_the_fastest_that_still_works() {
        let map = explore(65_536, Caches::Either, &mut big());
        let best = map.recommended().expect("something holds a turn");
        assert_eq!(best.cache, Cache::Q8_0);
        assert_eq!(best.context, 16_384, "24k cost a third of the speed");
        assert!((map.recommended_rate().unwrap() - 33.68).abs() < 0.01);
    }

    /// **The whole curve survives, because the user chooses from it.**
    ///
    /// The earlier version stopped climbing at the first loss and would have returned three rows
    /// for the 27B. Context against speed is a preference, and a pruned search has thrown away
    /// the rows the preference is expressed with.
    #[test]
    fn every_rung_is_measured_even_the_ones_that_lose() {
        let map = explore(65_536, Caches::Either, &mut big());
        for rung in [8_192u32, 16_384, 24_576, 32_768] {
            assert!(
                map.readings.iter().any(|r| r.loadout.context == rung),
                "{rung} missing from {:?}",
                map.readings
            );
        }
        // Both caches at the base, so the trade that was decided is visible.
        assert_eq!(
            map.readings
                .iter()
                .filter(|r| r.loadout.context == 16_384)
                .count(),
            2
        );
    }

    /// A rung too small to hold a turn is measured and marked, never hidden.
    ///
    /// It is genuinely the fastest row, and it genuinely stops mid-sentence. Saying both is the
    /// Studio Panel's rule - greyed, explained, and `USE IT ANYWAY` still offered.
    #[test]
    fn a_context_that_cannot_hold_a_turn_is_shown_and_never_recommended() {
        let map = explore(65_536, Caches::Either, &mut big());
        let small = map
            .readings
            .iter()
            .find(|r| r.loadout.context == 8_192)
            .expect("it is measured");
        assert!(!small.holds_a_turn, "8192 < {A_REAL_TURN} + room to answer");
        assert!(small.tokens_per_second.unwrap() > map.recommended_rate().unwrap());
        assert_ne!(map.recommended().unwrap().context, 8_192);
    }

    /// `unknown model architecture: 'gptoss'` is a real answer, and it is not a slow one.
    #[test]
    fn a_model_that_never_loads_says_so_rather_than_recommending_something() {
        let mut nothing = Fake {
            answers: Vec::new(),
            asked: Vec::new(),
        };
        let map = explore(65_536, Caches::Either, &mut nothing);
        assert!(!map.loaded());
        assert!(map.recommended().is_none());
    }

    /// A model trained for less than the ceiling is not asked for more than it knows.
    #[test]
    fn the_ladder_stops_at_what_the_model_was_trained_for() {
        let mut fake = Fake {
            answers: vec![
                (16_384, Cache::F16, 40.0),
                (24_576, Cache::F16, 40.0),
                (32_768, Cache::F16, 40.0),
                (65_536, Cache::F16, 40.0),
            ],
            asked: Vec::new(),
        };
        // qwen3-14b reports 40960 and nothing should offer it 65536.
        let map = explore(40_960, Caches::Either, &mut fake);
        assert!(fake.asked.iter().all(|one| one.context <= 40_960));
        assert_eq!(map.recommended().unwrap().context, 32_768);
    }

    /// Nothing is loaded twice. The probes are the whole cost of this.
    #[test]
    fn no_loadout_is_measured_more_than_once() {
        let mut fake = gemma();
        let _ = explore(65_536, Caches::Either, &mut fake);
        let mut seen = fake.asked.clone();
        seen.sort_by_key(|one| (one.context, one.cache == Cache::Q8_0));
        seen.dedup();
        assert_eq!(seen.len(), fake.asked.len(), "{:?}", fake.asked);
        assert!(
            fake.asked.len() <= 9,
            "{} probes is too many",
            fake.asked.len()
        );
    }

    /// A runtime that cannot be told a cache type is not asked about one.
    #[test]
    fn one_cache_maps_the_ladder_and_nothing_else() {
        // Ollama ignores a per-request `cache_type_k` silently, so a curve there varies context
        // only. Running the comparison anyway would put two readings in the map that differ in
        // a setting the server never received - two rows describing one run.
        // Answers at every rung under one cache, which is what a running server does.
        let mut fake = Fake {
            answers: LADDER
                .iter()
                .map(|rung| (*rung, Cache::Q8_0, 20.0))
                .collect(),
            asked: Vec::new(),
        };
        let map = explore(super::CEILING, Caches::Only(Cache::Q8_0), &mut fake);
        assert!(
            fake.asked.iter().all(|one| one.cache == Cache::Q8_0),
            "the other cache was never even asked for"
        );
        assert!(
            map.readings.iter().all(|r| r.loadout.cache == Cache::Q8_0),
            "every row ran under the cache the server was started with"
        );
        let mut sizes: Vec<u32> = map.readings.iter().map(|r| r.loadout.context).collect();
        sizes.sort_unstable();
        sizes.dedup();
        assert_eq!(sizes.len(), map.readings.len(), "no size measured twice");
        assert_eq!(
            map.readings.len(),
            how_many(super::CEILING, Caches::Only(Cache::Q8_0))
        );
    }

    /// The wall is not the recommendation, and a turn needs the wall.
    #[test]
    fn the_largest_that_loaded_is_kept_apart_from_the_best() {
        // A card that runs out: everything up to 24k loads and nothing above it does.
        let mut fails_high = Fake {
            answers: vec![
                (8_192, Cache::F16, 30.0),
                (16_384, Cache::F16, 28.0),
                (24_576, Cache::F16, 25.0),
                (8_192, Cache::Q8_0, 29.0),
                (16_384, Cache::Q8_0, 27.5),
                (24_576, Cache::Q8_0, 24.0),
            ],
            asked: Vec::new(),
        };
        let map = explore(super::CEILING, Caches::Either, &mut fails_high);
        assert_eq!(map.most_that_loaded(), 24_576);
        let most = map.most_that_loaded();
        assert!(most > 0, "something loaded");
        assert!(
            map.readings
                .iter()
                .filter(|r| r.tokens_per_second.is_none())
                .all(|r| r.loadout.context > most),
            "nothing above the wall loaded, and nothing below it failed"
        );
    }

    /// A model measured on one runtime says nothing about it on another.
    #[test]
    fn a_ceiling_belongs_to_the_runtime_it_was_measured_on() {
        let mut held = Loadouts::default();
        held.remember(Best {
            model: "9300000000:qwen3-14b".to_owned(),
            name: "qwen3:14b".to_owned(),
            card: "a card".to_owned(),
            build: "Ollama".to_owned(),
            runtime: "ollama".to_owned(),
            most: 24_576,
            chose: Loadout {
                context: 16_384,
                cache: Cache::F16,
            },
            tokens_per_second: 20.0,
            tried: Vec::new(),
            at: 1,
        });
        assert_eq!(held.ceiling_here("qwen3:14b", "ollama"), Some(24_576));
        // The same model through another program is a different measurement, and an unmeasured
        // one is never capped - a ceiling invented from nothing would be the gauge with nothing
        // behind it, applied to somebody's conversation.
        assert_eq!(held.ceiling_here("qwen3:14b", "llama_cpp"), None);
        assert_eq!(held.ceiling_here("something else", "ollama"), None);
    }

    /// An answer measured on one card, or one build, is not an answer about another.
    #[test]
    fn a_new_card_or_a_new_build_makes_the_question_unanswered() {
        let mut held = Loadouts::default();
        held.remember(Best {
            model: "9300000000:qwen3-14b".to_owned(),
            name: "a model".to_owned(),
            card: "NVIDIA GeForce RTX 4070 SUPER".to_owned(),
            build: "10622".to_owned(),
            runtime: "llama_cpp".to_owned(),
            most: 32_768,
            chose: Loadout {
                context: 16_384,
                cache: Cache::Q8_0,
            },
            tokens_per_second: 33.6,
            tried: Vec::new(),
            at: 1,
        });
        let here = "9300000000:qwen3-14b";
        assert!(held
            .about(here, "NVIDIA GeForce RTX 4070 SUPER", "10622")
            .is_some());
        assert!(
            held.about(here, "Intel Arc B60", "10622").is_none(),
            "a 12 GB answer must not cap a 24 GB card"
        );
        assert!(
            held.about(here, "NVIDIA GeForce RTX 4070 SUPER", "10700")
                .is_none(),
            "a new build changes kernels; ageing quietly is the failure to avoid"
        );
        assert!(
            held.about(
                "7400000000:qwen3-14b",
                "NVIDIA GeForce RTX 4070 SUPER",
                "10622"
            )
            .is_none(),
            "a file that changed size is a file nobody measured"
        );
    }

    /// The safe half of every trade, because it is what somebody uses while the search runs.
    #[test]
    fn nothing_measured_yet_costs_three_percent_rather_than_two_thirds() {
        assert_eq!(conservative(16_384).cache, Cache::Q8_0);
        assert_eq!(conservative(16_384).context, 16_384);
    }
}

#[cfg(test)]
mod on_this_machine {
    use super::*;

    /// The whole search, against a real card, on whatever this machine has.
    ///
    /// Ignored by default: it loads a model three or four times and takes minutes. Run with
    /// `cargo test -p epoch-models real_search -- --ignored --nocapture` and read the table.
    #[test]
    #[ignore = "needs EPOCH_BENCH_MODEL and a real card; it loads a model three or four times"]
    fn a_real_search_on_a_real_model() {
        let Some(shelf) = std::env::var_os("EPOCH_BENCH_MODEL") else {
            eprintln!("set EPOCH_BENCH_MODEL to a .gguf to run this");
            return;
        };
        let path = std::path::PathBuf::from(shelf);
        let ceiling = crate::gguf::read(&path)
            .ok()
            .and_then(|h| h.trained_context())
            .and_then(|n| u32::try_from(n).ok())
            .map(|n| n.min(CEILING))
            .unwrap_or(CEILING);
        eprintln!("{} — ceiling {ceiling}", path.display());

        let mut bench = crate::runtimes::Bench::new(&path).expect("llama-server");
        let began = std::time::Instant::now();
        let map = explore(ceiling, Caches::Either, &mut bench);
        eprintln!("took {:.0}s", began.elapsed().as_secs_f64());
        let best = map.recommended();
        for one in &map.readings {
            eprintln!(
                "  {:>6} ctx  {:<22} {:>16}{}{}",
                one.loadout.context,
                one.loadout.cache.plainly(),
                one.tokens_per_second
                    .map_or("did not load".to_owned(), |r| format!("{r:.2} tok/s")),
                if one.holds_a_turn {
                    ""
                } else {
                    "   too small for a turn"
                },
                if Some(one.loadout) == best {
                    "   <- recommended"
                } else {
                    ""
                },
            );
        }
        assert!(map.loaded(), "nothing loaded at all");
    }
}

#[cfg(test)]
mod one_probe {
    use super::*;

    /// One probe, through the same `Bench` a search uses, printing what it got.
    ///
    /// Exists because a search read 25.4 tok/s for a loadout that measures 34.9 from a
    /// standalone `llama-server` with the same flags, and the difference had to be found rather
    /// than reasoned about.
    #[test]
    #[ignore = "needs EPOCH_BENCH_MODEL and a llama-server; one probe, printed"]
    fn what_one_probe_really_measures() {
        let Some(path) = std::env::var_os("EPOCH_BENCH_MODEL") else {
            return;
        };
        let path = std::path::PathBuf::from(path);
        let mut bench = crate::runtimes::Bench::new(&path).expect("llama-server");
        for cache in [Cache::Q8_0, Cache::Q8_0] {
            let one = Loadout {
                context: 16_384,
                cache,
            };
            let began = std::time::Instant::now();
            let got = <crate::runtimes::Bench as Probe>::run(&mut bench, one);
            eprintln!(
                "{:?} -> {:?} in {:.0}s",
                one,
                got.map(|r| r.tokens_per_second),
                began.elapsed().as_secs_f64()
            );
        }
    }
}
