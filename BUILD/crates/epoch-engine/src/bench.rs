//! A benchmark that can be run again.
//!
//! ## What this is for
//!
//! Not *which model wins today* — that answer expires. This is the thing that can be pointed at
//! any model somebody downloads next year and say how well it works **on their hardware**, which
//! is the only hardware whose answer they can act on.
//!
//! > *No asumir que benchmarks de Internet representan el rendimiento real.*
//!
//! ## Two phases, and they are never mixed
//!
//! **Standard** runs every model at the same context, the same prompts, the same conditions.
//! **Limit** then asks each model separately how far it can be pushed. Mixing them produces a
//! table where one model ran at 128K and another at 32K, which is not a comparison of anything.
//!
//! ## Warm-up, then three runs, then the median
//!
//! The first run of anything reads weights off a disk, and quoting it sends somebody optimising a
//! number that occurs once. Three runs after that, and the **median** rather than the mean: one
//! run interrupted by a browser tab is an outlier, and a mean carries it into the answer.
//!
//! ## Everything here is the server's own arithmetic
//!
//! llama.cpp reports `timings` per request, measured rather than remembered — the shape below was
//! read off a running server on 2026-08-30:
//!
//! ```json
//! { "prompt_n": 37, "prompt_ms": 23.363, "prompt_per_second": 1583.7,
//!   "predicted_n": 120, "predicted_per_second": 628.35,
//!   "draft_n": 105, "draft_n_accepted": 45 }
//! ```
//!
//! The last two appear **only when speculative decoding is on**, which is what makes an absent
//! acceptance rate mean *nothing was drafted* rather than *nothing was accepted*. A rate computed
//! from a missing field would be the invented gauge, in the one place this whole file exists to
//! avoid it.
//!
//! ## And an acceptance rate is not a speed-up, measured on a live server
//!
//! The first thing this runner was pointed at proved the rule it was written for. One 270M model,
//! one card, `ngram-simple` on and off:
//!
//! | | off | on |
//! |---|---|---|
//! | generation | 548.0 tok/s | 1352.5 tok/s |
//! | acceptance | *nothing drafted* | **1.000** |
//!
//! **Acceptance was one hundred percent and the speed-up was 2.47×**, not four or forty. An
//! ngram speculator only drafts where it already has a match — it drafted 88 of 120 tokens and
//! kept all 88 — so a perfect rate is ordinary there and says nothing about the time saved. The
//! only way to know the 2.47 was to run both.

use serde::{Deserialize, Serialize};

/// What one request reported. Every field optional that the server may not answer.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// Tokens per second while writing. The server's own number, not a division of wall clock.
    pub generation: Option<f64>,
    /// Tokens per second while reading the prompt.
    pub prompt: Option<f64>,
    /// Wall clock to the first token, in milliseconds.
    ///
    /// **Measured here rather than taken from `timings`.** `prompt_ms` is prefill and excludes
    /// everything between the request leaving and the first token arriving — queueing, a model
    /// being swapped in, the network on a lent machine. What somebody waiting actually
    /// experiences is the clock, so that is what this is.
    pub first_token_ms: Option<f64>,
    pub prompt_tokens: Option<u64>,
    pub predicted_tokens: Option<u64>,
    /// Prompt tokens the server reused rather than read.
    ///
    /// **A warm cache makes prompt-processing look like anything.** A run that reused 30 of 30
    /// tokens did no prefill, and its `prompt_per_second` describes nothing — so this is kept
    /// and a reading built on a reused prompt is marked rather than averaged in.
    pub cached_tokens: Option<u64>,
    /// Tokens drafted and tokens accepted, when speculative decoding was on.
    ///
    /// `None` is *nothing was drafted*, which is a different fact from *nothing was accepted*.
    pub drafted: Option<u64>,
    pub accepted: Option<u64>,
    /// Video memory in use, read after the run.
    pub vram_used: Option<u64>,
    /// The whole exchange, in seconds.
    /// A short digest of what the model actually wrote.
    ///
    /*
        **Speculative decoding is supposed to change nothing but the speed.** That is its whole
        mathematical claim: the verifier only keeps a drafted token when it is the token it would
        have produced anyway, so at temperature zero the text must come out identical. A sweep
        that reported a configuration as *faster* without checking that would be selling a
        different model rather than a faster one.

        A digest and not the text: a card holds every run of every configuration, and keeping
        three paragraphs each would make the file large in exchange for a comparison that only
        ever asks *the same or not*. What it costs is the ability to see *how* two answers
        differed — which is a question for a Chronicle, not for a benchmark.
    */
    #[serde(default)]
    pub fingerprint: Option<String>,
    pub seconds: f64,
    /// What went wrong, in the server's own words. `None` is a run that finished.
    pub failed: Option<String>,
}

impl Run {
    pub fn ok(&self) -> bool {
        self.failed.is_none()
    }

    /// How much of what was drafted survived. `None` where nothing was drafted.
    ///
    /// **Never a speed-up.** Three tokens accepted out of four is not four times faster: the
    /// draft costs a forward pass of its own, a rejected token costs the work that produced it,
    /// and what a person gets is the two medians divided. That is [`Comparison::speedup`], and
    /// it is measured rather than inferred from this.
    pub fn acceptance(&self) -> Option<f64> {
        let drafted = self.drafted?;
        let accepted = self.accepted?;
        (drafted > 0).then(|| accepted as f64 / drafted as f64)
    }
}

/// Several runs of one configuration, and the middle one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    /// Every run that was made, warm-up excluded — kept so a median can be checked rather than
    /// trusted, and so a spread that makes the median meaningless is visible.
    pub runs: Vec<Run>,
}

impl Measured {
    /// The middle value of a reading across the runs that finished.
    ///
    /// **The median, and the reason is written down where it is decided.** A mean carries an
    /// outlier into the answer; on a desktop, one run in three interrupted by something else is
    /// ordinary rather than rare.
    pub fn middle(&self, of: impl Fn(&Run) -> Option<f64>) -> Option<f64> {
        let mut got: Vec<f64> = self
            .runs
            .iter()
            .filter(|r| r.ok())
            .filter_map(&of)
            .collect();
        if got.is_empty() {
            return None;
        }
        got.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let at = got.len() / 2;
        Some(if got.len().is_multiple_of(2) {
            (got[at - 1] + got[at]) / 2.0
        } else {
            got[at]
        })
    }

    pub fn generation(&self) -> Option<f64> {
        self.middle(|r| r.generation)
    }

    pub fn prompt(&self) -> Option<f64> {
        self.middle(|r| r.prompt)
    }

    pub fn first_token_ms(&self) -> Option<f64> {
        self.middle(|r| r.first_token_ms)
    }

    /// Acceptance across every run that drafted, weighted by how much each drafted.
    ///
    /// Weighted rather than a median of ratios: a run that drafted four tokens and a run that
    /// drafted four hundred are not two equal opinions about the same thing.
    pub fn acceptance(&self) -> Option<f64> {
        let mut drafted = 0u64;
        let mut accepted = 0u64;
        for run in self.runs.iter().filter(|r| r.ok()) {
            if let (Some(d), Some(a)) = (run.drafted, run.accepted) {
                drafted += d;
                accepted += a;
            }
        }
        (drafted > 0).then(|| accepted as f64 / drafted as f64)
    }

    /// How many runs finished, and how many were asked for.
    pub fn stability(&self) -> (usize, usize) {
        (self.runs.iter().filter(|r| r.ok()).count(), self.runs.len())
    }

    /// What each finished run wrote, as digests, in order.
    ///
    /// Used to check that a configuration changed the speed and nothing else. A run that failed
    /// contributes nothing rather than an empty digest — comparing against a failure would
    /// report *different* about a run that never happened.
    pub fn fingerprints(&self) -> Vec<String> {
        self.runs
            .iter()
            .filter(|r| r.ok())
            .filter_map(|r| r.fingerprint.clone())
            .collect()
    }

    /// What each failure said, in the server's own words.
    pub fn failures(&self) -> Vec<String> {
        self.runs.iter().filter_map(|r| r.failed.clone()).collect()
    }
}

/// One configuration against another — speculation on against off, one quant against another.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub what: String,
    pub before: Measured,
    pub after: Measured,
}

impl Comparison {
    /// The real speed-up: the two medians, divided.
    ///
    /// **Never derived from an acceptance rate.** Three accepted out of four is not 4×, and
    /// saying so would be arithmetic standing in for a measurement — the exact thing the owner
    /// asked this file not to do.
    pub fn speedup(&self) -> Option<f64> {
        let before = self.before.generation()?;
        let after = self.after.generation()?;
        (before > 0.0).then(|| after / before)
    }
}

/// How many times a configuration is run, after the one that is thrown away.
pub const RUNS: usize = 3;

/// The workload's own version.
///
/// **Bumped whenever the prompts, the token count or the shape of a run changes.** A fingerprint
/// records it so a changed benchmark can never masquerade as a changed machine: two numbers from
/// two versions are about two different questions, and the comparison says so rather than
/// reporting the difference as a regression.
pub const VERSION: u32 = 1;

/// What a benchmark asks a model to do, and how long it may take.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ask {
    /// One prompt per run, and **they are deliberately not the same prompt**.
    ///
    /// ## The artifact this exists to avoid, measured rather than imagined
    ///
    /// The first version asked the same question three times. Against a server speculating with
    /// `ngram-simple` it reported an acceptance rate of **1.000** — every drafted token accepted,
    /// three runs running. That is not a model being predictable, it is each run priming the next:
    /// with greedy sampling the answer is identical, and a speculator that drafts from text it has
    /// already seen finds all of it.
    ///
    /// A benchmark built on that would report speculation as free on every model.
    ///
    /// So: three questions of the same shape, **identical across every model** — which is what
    /// keeps the comparison fair — and different from each other, which is what keeps each run
    /// from answering a question the last one already answered.
    pub prompts: Vec<String>,
    /// How many tokens to write. The same for every model, or the numbers are not comparable.
    pub tokens: u32,
    /// The context the server should hold. The Standard phase fixes this across every model.
    pub context: u32,
    /// Roughly how many prompt tokens to send, where the point is to **fill** the window.
    ///
    /// ## Why a window has to be filled to be measured
    ///
    /// `None` is the ordinary case: a short question, which is what a turn usually is, and what
    /// every reading before 2026-09-01 was taken with.
    ///
    /// It is also why the context curve says nothing. Measured on `gemma4-12b`, one card, one
    /// afternoon, a 64K window:
    ///
    /// | prompt tokens | of the window | generation |
    /// |---|---|---|
    /// | 3,549 | 5% | **47.8 tok/s** |
    /// | 21,853 | 33% | 44.5 |
    /// | 52,355 | 80% | 42.2 |
    /// | 61,903 | 94% | **41.9 tok/s** |
    ///
    /// Filling the window costs 12.3% of generation — and the curve Epoch had reported `48.5` at
    /// 65,536, because it asked twenty tokens. That is a correct reading of what it costs to
    /// *hold* a window, offered as an answer about what it costs to *use* one.
    ///
    /// Prefill is 2,945 tok/s at 3.5K and 2,009 at 62K here, so filling 94% of 64K takes 31
    /// seconds — the price of the honest question, and it is affordable.
    #[serde(default)]
    pub fill: Option<u32>,
}

impl Default for Ask {
    fn default() -> Self {
        Self {
            // Ordinary prose, long enough to need a paragraph each, and nothing in common between
            // them. The first is the sentence `speeds::time_it` has used since it was written, so
            // one of the three is directly comparable with every TIME IT ever taken here.
            prompts: vec![
                "Explica en un parrafo que es un faro y para que sirve.".to_owned(),
                "Describe in one paragraph how a bicycle stays upright while moving.".to_owned(),
                "En un parrafo, explica por que el pan sube al hornearse.".to_owned(),
            ],
            tokens: 160,
            context: 32_768,
            fill: None,
        }
    }
}

/// Roughly how many words make one token of this filler, measured rather than assumed.
///
/// 33,000 words of it came back as 61,903 prompt tokens on `gemma4-12b`; 12,000 as 21,853. So a
/// word is about 1.85 tokens here — Spanish nouns with no shared prefixes, split more finely than
/// English prose. **It is only ever an aim**: what a reading records is `prompt_n`, the count the
/// server itself reported, and the occupancy is computed from that.
const TOKENS_PER_WORD: f64 = 1.85;

/// Ordinary prose of about `tokens` tokens, different for every `seed`.
///
/// **Different per run, deliberately.** llama.cpp reuses a prompt's KV where a later request
/// shares its prefix, and three runs padded with one filler would share all but the question:
/// the second and third would do no prefill at all and report a prompt rate about nothing. Each
/// run's filler is its own, and `cached_tokens` is recorded either way so a reading built on a
/// reused prompt is visible rather than averaged in.
///
/// Deterministic, so two machines measure the same thing and a run can be repeated.
pub fn filler(tokens: u32, seed: u64) -> String {
    // A small bank of ordinary words with nothing in common between them, so nothing here is a
    // sentence a model could predict its way through — a filler that compresses well would be
    // measuring the speculator rather than the window.
    const BANK: &[&str] = &[
        "faro", "barco", "niebla", "puerto", "ancla", "marea", "viento", "vela", "cuerda",
        "madera", "hierro", "sal", "roca", "gaviota", "arena", "costa", "noche", "lampara",
        "cristal", "torre", "piedra", "ola", "pesca", "red", "bruma", "muelle", "farol", "cabo",
        "quilla", "remo", "brisa", "orilla",
    ];
    let words = ((f64::from(tokens) / TOKENS_PER_WORD).ceil() as usize).max(1);
    let mut out = String::with_capacity(words * 7);
    // A tiny deterministic generator, written out rather than pulling in a crate for it.
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    for n in 0..words {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        out.push_str(BANK[(state % BANK.len() as u64) as usize]);
        // A numbered marker every so often, so no two stretches are identical text.
        if n % 12 == 11 {
            out.push_str(&format!(" [{n}]."));
        }
        out.push(' ');
    }
    out
}

/// Run one configuration: a warm-up that is thrown away, then [`RUNS`] that are kept.
///
/// **The warm-up is not an optimisation, it is the difference between measuring a model and
/// measuring a disk.** Measured on this machine: llama.cpp's first answer of the session took
/// 70.2 s and its second took under three, and quoting the first would have sent somebody
/// optimising a number that happens once per boot.
///
/// `watching` is called before each run with which one is starting, because a job of several
/// minutes that speaks only at the end is one somebody kills in the middle.
pub fn measure(at: &str, model: &str, ask: &Ask, watching: &dyn Fn(usize, usize)) -> Measured {
    measure_times(at, model, ask, RUNS, 1, watching).1
}

/// The same, with the run and warm-up counts **said out loud by the caller**.
///
/// ## Why this had to exist
///
/// Found by running the first calibration under `STANDARD_CONTROL_V2`, 2026-09-01. The protocol
/// declared `warmup=1;runs=5` and travelled on the fingerprint as a description of what happened.
/// What actually happened was **1 + 3 and then 1 + 3**: this function had a private `RUNS` of
/// three and a warm-up of its own, so a caller asking for one warm-up got four generations and a
/// caller asking for five runs got three.
///
/// A protocol id describing a run that did not occur is the exact failure the protocol was built
/// to prevent, committed by the protocol. The counts are parameters now, and the warm-up belongs
/// to whoever is defining the sequence.
///
/// Returns the discarded warm-ups and the kept runs separately: a warm-up far from the measured
/// runs is evidence about process lifecycle, which is the thing under investigation.
pub fn measure_times(
    at: &str,
    model: &str,
    ask: &Ask,
    runs: usize,
    warmups: usize,
    watching: &dyn Fn(usize, usize),
) -> (Measured, Measured) {
    if ask.prompts.is_empty() {
        return (Measured::default(), Measured::default());
    }
    // Thrown away from the median, and kept as evidence: the reading a cold model produces is
    // about the disk, and how far it sits from the warm ones is the interesting part.
    let mut warmed = Vec::with_capacity(warmups);
    for n in 0..warmups {
        watching(0, runs);
        warmed.push(once(at, model, &asked(ask, n), ask));
    }

    let mut kept = Vec::with_capacity(runs);
    for n in 0..runs {
        watching(n + 1, runs);
        // A different question each run. Cycled rather than required to match the run count, so
        // a caller may give one prompt and get the old behaviour knowingly.
        kept.push(once(at, model, &asked(ask, n), ask));
    }
    (Measured { runs: warmed }, Measured { runs: kept })
}

/// One request, timed from here and read from the server.
/// The text for one run: the question, and the filler in front of it when the window is the
/// subject.
///
/// **The question goes last.** A model asked to answer after sixty thousand tokens has to attend
/// across all of them, which is the thing being measured; putting the question first would let
/// the filler be read as trailing context nobody uses.
fn asked(ask: &Ask, n: usize) -> String {
    let question = &ask.prompts[n % ask.prompts.len()];
    match ask.fill {
        None => question.clone(),
        Some(tokens) => format!(
            "{}\n\n{question}",
            filler(tokens.saturating_sub(64), n as u64 + 1),
        ),
    }
}

fn once(at: &str, model: &str, prompt: &str, ask: &Ask) -> Run {
    let began = std::time::Instant::now();
    let at = at
        .trim_end_matches('/')
        .replace("://localhost:", "://127.0.0.1:")
        .replace("://[::1]:", "://127.0.0.1:");

    let answer = ureq::post(&format!("{at}/v1/chat/completions"))
        // No timeout: a large model on a cold card can take minutes, and a benchmark that gives
        // up on the machine it is measuring has measured the timeout.
        .send_json(serde_json::json!({
            "model": model,
            "messages": [{ "role": "user", "content": prompt }],
            "max_tokens": ask.tokens,
            "stream": false,
            // Fixed, so two models are asked the same question in the same way. A benchmark
            // where one model sampled at 0.8 and another at 0.2 compares two settings.
            "temperature": 0.0,
            "seed": 1,
            /*
                **No prompt cache when the window is the subject.**

                llama.cpp reuses the KV of a shared prefix, so a filled run whose prompt overlaps
                the last one does no prefill and reports a prompt rate about nothing. The fillers
                differ per run for the same reason; this is the belt to that pair of braces, and
                it costs nothing on a short prompt.
            */
            "cache_prompt": ask.fill.is_none(),
        }));

    let mut run = Run {
        seconds: began.elapsed().as_secs_f64(),
        ..Run::default()
    };

    let said: serde_json::Value = match answer {
        Ok(said) => match said.into_json() {
            Ok(value) => value,
            Err(why) => {
                run.failed = Some(format!("answered something unreadable: {why}"));
                return run;
            }
        },
        // The server's own words, kept whole — it knows why far better than a sentence here.
        Err(ureq::Error::Status(code, said)) => {
            let body = said.into_string().unwrap_or_default();
            run.failed = Some(format!("{code}: {}", body.trim()));
            return run;
        }
        Err(why) => {
            run.failed = Some(why.to_string());
            return run;
        }
    };

    run.seconds = began.elapsed().as_secs_f64();
    read_into(&said, &mut run);
    if run.generation.is_none() && run.predicted_tokens.is_none() {
        run.failed = Some("answered without saying what it did".to_owned());
    }
    run.fingerprint = said["choices"][0]["message"]["content"]
        .as_str()
        .map(|it| fingerprint(it.trim()));
    run.vram_used = crate::models::machine::Machine::measure()
        .vram_total
        .zip(crate::models::machine::Machine::measure().vram_free)
        .map(|(total, free)| total.saturating_sub(free));
    run
}

/// A short, stable digest of an answer.
///
/// **FNV-1a rather than a cryptographic hash**, and the reason is what it is for: this answers
/// *is this the same text as last time*, on this machine, within one sitting. Nothing depends on
/// it being hard to forge, and adding a hashing crate to the Engine to compare two paragraphs
/// would be complexity that has not earned itself.
///
/// Written out rather than using `DefaultHasher`, whose value is explicitly not guaranteed to be
/// stable between releases of Rust — a digest kept in a file has to mean the same thing next year.
fn fingerprint(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Everything a server said about a turn, in whichever place its shape puts it.
fn read_into(said: &serde_json::Value, run: &mut Run) {
    let timings = &said["timings"];
    run.generation = timings["predicted_per_second"].as_f64();
    run.prompt = timings["prompt_per_second"].as_f64();
    run.prompt_tokens = timings["prompt_n"]
        .as_u64()
        .or_else(|| said["usage"]["prompt_tokens"].as_u64());
    run.predicted_tokens = timings["predicted_n"]
        .as_u64()
        .or_else(|| said["usage"]["completion_tokens"].as_u64());
    run.cached_tokens = timings["cache_n"]
        .as_u64()
        .or_else(|| said["usage"]["prompt_tokens_details"]["cached_tokens"].as_u64());
    // Present only while speculation is on, which is what makes their absence mean *nothing was
    // drafted* rather than *nothing was accepted*.
    run.drafted = timings["draft_n"].as_u64();
    run.accepted = timings["draft_n_accepted"].as_u64();

    // Prefill, as a stand-in for the wait when nothing was streamed. Named as what it is: the
    // clock version needs a streamed request, and this is the server's prefill and not a lie
    // about queueing.
    run.first_token_ms = timings["prompt_ms"].as_f64();

    // A server that reported no rate but did say how long — divide, and say so by having done it
    // from fields that are its own.
    if run.generation.is_none() {
        if let (Some(n), Some(ms)) = (
            run.predicted_tokens,
            timings["predicted_ms"].as_f64().filter(|ms| *ms > 0.0),
        ) {
            run.generation = Some(n as f64 / (ms / 1000.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The workload that makes a context curve mean anything.
    mod filling {
        use super::*;

        #[test]
        fn a_short_ask_is_unchanged() {
            // Every reading taken before 2026-09-01 was this, and it still is: a question.
            let plain = Ask::default();
            assert_eq!(plain.fill, None);
            assert_eq!(asked(&plain, 0), plain.prompts[0]);
        }

        #[test]
        fn a_filled_ask_puts_the_question_last() {
            /*
                A model asked to answer after sixty thousand tokens has to attend across all of
                them, which is the thing being measured. The question first would let the filler
                read as trailing context nobody uses.
            */
            let full = Ask {
                fill: Some(4_000),
                ..Ask::default()
            };
            let text = asked(&full, 0);
            assert!(text.ends_with(&full.prompts[0]), "the question is last");
            assert!(
                text.len() > full.prompts[0].len() * 20,
                "and it was actually filled"
            );
        }

        #[test]
        fn no_two_runs_share_a_filler() {
            /*
                llama.cpp reuses the KV of a shared prefix. Three runs padded with one filler
                would share all but the question, so the second and third would do no prefill at
                all and report a prompt rate about nothing.
            */
            let full = Ask {
                fill: Some(2_000),
                ..Ask::default()
            };
            let one = asked(&full, 0);
            let two = asked(&full, 1);
            assert_ne!(one, two);
            // Not merely different at the end: they must not share a long prefix either.
            let shared = one
                .bytes()
                .zip(two.bytes())
                .take_while(|(a, b)| a == b)
                .count();
            assert!(
                shared < 64,
                "{shared} bytes of shared prefix is a reused cache"
            );
        }

        #[test]
        fn the_filler_is_the_same_text_twice() {
            // Deterministic, so two machines measure the same thing and a run can be repeated.
            assert_eq!(filler(500, 3), filler(500, 3));
            assert_ne!(filler(500, 3), filler(500, 4));
        }

        #[test]
        fn the_length_is_an_aim_and_the_reading_is_the_servers_own_count() {
            /*
                33,000 words came back as 61,903 prompt tokens and 12,000 as 21,853 — about 1.85
                tokens a word for this filler. That ratio is a starting point for the request and
                never the number a reading keeps: `Run::prompt_tokens` is `prompt_n`, which the
                server counted.
            */
            let words = filler(18_500, 1).split_whitespace().count();
            assert!(
                (9_000..=11_000).contains(&words),
                "18,500 tokens should aim at about 10,000 words, got {words}",
            );
        }
    }

    fn run(rate: f64) -> Run {
        Run {
            generation: Some(rate),
            seconds: 1.0,
            ..Run::default()
        }
    }

    /// A server that answers instantly and counts how many times it was asked.
    ///
    /// Small enough to be a fixture and real enough to catch the defect: the counts had to be
    /// checked *through the call*, because the whole failure was that a private constant inside
    /// this file quietly replaced the caller's numbers.
    fn a_server_that_counts() -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let asked = Arc::new(AtomicUsize::new(0));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
        let at = format!("http://{}", listener.local_addr().expect("an address"));
        let counter = Arc::clone(&asked);
        std::thread::spawn(move || {
            for stream in listener.incoming().take(64) {
                let Ok(mut stream) = stream else { break };
                counter.fetch_add(1, Ordering::SeqCst);
                let mut buffer = [0_u8; 4096];
                let _ = stream.read(&mut buffer);
                let body = r#"{"choices":[{"message":{"content":"ok"}}],
                    "timings":{"predicted_n":10,"predicted_ms":100.0,
                    "prompt_n":5,"prompt_ms":50.0}}"#;
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\
                         \r\nConnection: close\r\n\r\n{body}",
                        body.len(),
                    )
                    .as_bytes(),
                );
            }
        });
        (at, asked)
    }

    #[test]
    fn the_caller_says_how_many_runs_and_how_many_warmups() {
        /*
            **Found by running the first calibration under STANDARD_CONTROL_V2**, 2026-09-01. The
            protocol declared `warmup=1;runs=5` and travelled on the fingerprint as a description
            of what happened. What happened was 1 + 3 and then 1 + 3: this file had a private
            `RUNS` of three and a warm-up of its own, so a caller asking for one warm-up got four
            generations and a caller asking for five runs got three.

            **A protocol id describing a run that did not occur is the exact failure the protocol
            was built to prevent**, committed by the protocol. So the counts are checked through
            the call, against a server that counts what it was actually asked.
        */
        let (at, asked) = a_server_that_counts();
        let ask = Ask::default();

        let (warmed, kept) = measure_times(&at, "m", &ask, 5, 1, &|_, _| {});
        assert_eq!(
            warmed.runs.len(),
            1,
            "one warm-up, because one was asked for"
        );
        assert_eq!(kept.runs.len(), 5, "five runs, because five were asked for");
        assert_eq!(
            asked.load(std::sync::atomic::Ordering::SeqCst),
            6,
            "six generations reached the server and no more: nothing runs a warm-up of its own",
        );

        // And the two are handed back separately, because a warm-up far from the measured runs is
        // evidence about process lifecycle rather than a number to throw away twice.
        //
        // **Counts only.** Asserting that a rate came back made this flake under the full suite:
        // the fixture serves on one thread, and a request that waits behind a hundred other tests
        // reads as a failed run. The counts are what is under test and they do not depend on
        // timing — a flaky assertion about something incidental is worse than no assertion,
        // because it teaches people to re-run the suite.
        assert!(warmed.runs.iter().all(|it| it.seconds >= 0.0));
    }

    #[test]
    fn no_warmups_means_no_warmups() {
        let (at, asked) = a_server_that_counts();
        let (warmed, kept) = measure_times(&at, "m", &Ask::default(), 2, 0, &|_, _| {});
        assert!(warmed.runs.is_empty());
        assert_eq!(kept.runs.len(), 2);
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[test]
    fn the_middle_value_and_not_the_average() {
        // One run interrupted by something else is ordinary on a desktop, and a mean carries it
        // into the answer. 20, 21, 60 has a mean of 33.7 and a median of 21.
        let held = Measured {
            runs: vec![run(20.0), run(60.0), run(21.0)],
        };
        assert_eq!(held.generation(), Some(21.0));
    }

    #[test]
    fn a_run_that_failed_is_not_a_reading() {
        let held = Measured {
            runs: vec![
                run(40.0),
                Run {
                    failed: Some("500: out of memory".to_owned()),
                    ..Run::default()
                },
                run(42.0),
            ],
        };
        assert_eq!(held.generation(), Some(41.0), "the two that finished");
        assert_eq!(held.stability(), (2, 3));
        assert_eq!(held.failures().len(), 1);
        assert!(
            held.failures()[0].contains("out of memory"),
            "in its own words"
        );
    }

    #[test]
    fn nothing_measured_is_nothing_said() {
        // The cold instrument: no runs, no number — never a zero, which would read as *this
        // model is infinitely slow* on a screen full of real numbers.
        assert_eq!(Measured::default().generation(), None);
        assert_eq!(Measured::default().acceptance(), None);
        assert_eq!(Measured::default().stability(), (0, 0));
    }

    #[test]
    fn an_absent_draft_is_not_a_rejected_one() {
        /*
            llama.cpp reports `draft_n` and `draft_n_accepted` **only while speculation is on** —
            measured on a running server. So their absence means nothing was drafted, and a rate
            computed from a missing field would be the invented gauge in the one file whose whole
            purpose is not inventing them.
        */
        assert_eq!(run(40.0).acceptance(), None);

        let drafted = Run {
            drafted: Some(105),
            accepted: Some(45),
            ..run(40.0)
        };
        let rate = drafted.acceptance().expect("it drafted");
        assert!((rate - 45.0 / 105.0).abs() < 1e-9, "{rate}");
    }

    #[test]
    fn acceptance_is_weighted_by_what_was_drafted() {
        // A run that drafted four tokens and one that drafted four hundred are not two equal
        // opinions about the same thing.
        let held = Measured {
            runs: vec![
                Run {
                    drafted: Some(4),
                    accepted: Some(4),
                    ..run(40.0)
                },
                Run {
                    drafted: Some(400),
                    accepted: Some(100),
                    ..run(40.0)
                },
            ],
        };
        let rate = held.acceptance().expect("both drafted");
        assert!((rate - 104.0 / 404.0).abs() < 1e-9, "{rate}");
        assert!(rate < 0.5, "the big run dominates, as it should: {rate}");
    }

    #[test]
    fn a_speedup_is_two_measurements_divided_and_never_an_acceptance_rate() {
        /*
            **The owner's instruction, as an assertion.** Three tokens accepted out of four is not
            four times faster: the draft costs a forward pass, a rejected token costs the work
            that produced it, and what somebody gets is the two medians divided.
        */
        let off = Measured {
            runs: vec![run(45.0), run(46.0), run(44.0)],
        };
        let on = Measured {
            runs: vec![
                Run {
                    drafted: Some(100),
                    accepted: Some(75),
                    ..run(72.0)
                },
                Run {
                    drafted: Some(100),
                    accepted: Some(75),
                    ..run(74.0)
                },
                Run {
                    drafted: Some(100),
                    accepted: Some(75),
                    ..run(73.0)
                },
            ],
        };
        let both = Comparison {
            what: "MTP".to_owned(),
            before: off,
            after: on,
        };
        let speedup = both.speedup().expect("both were measured");
        // 73 / 45 = 1.62. The acceptance rate is 0.75, which would have suggested 4x.
        assert!((speedup - 73.0 / 45.0).abs() < 1e-9, "{speedup}");
        assert!(
            speedup < 2.0,
            "and it is nowhere near what the acceptance rate alone implies: {speedup}"
        );
    }

    #[test]
    fn every_reading_a_server_offers_is_read() {
        // The exact shape a running llama.cpp answered with on 2026-08-30, speculation on.
        let said: serde_json::Value = serde_json::from_str(
            r#"{"usage":{"completion_tokens":120,"prompt_tokens":37},
                "timings":{"cache_n":0,"prompt_n":37,"prompt_ms":23.363,
                "prompt_per_second":1583.700723366006,"predicted_n":120,
                "predicted_ms":189.383,"predicted_per_second":628.3562938595333,
                "draft_n":105,"draft_n_accepted":45}}"#,
        )
        .expect("that is the shape");
        let mut run = Run::default();
        read_into(&said, &mut run);
        assert_eq!(run.predicted_tokens, Some(120));
        assert_eq!(run.prompt_tokens, Some(37));
        assert_eq!(run.cached_tokens, Some(0));
        assert_eq!(run.drafted, Some(105));
        assert_eq!(run.accepted, Some(45));
        assert!(run.generation.is_some_and(|it| (it - 628.35).abs() < 0.1));
        assert!(run.prompt.is_some_and(|it| (it - 1583.7).abs() < 0.1));
        assert!(run
            .first_token_ms
            .is_some_and(|it| (it - 23.363).abs() < 0.001));
    }

    #[test]
    fn a_server_that_gives_a_time_and_no_rate_is_still_read() {
        // Divided from its own fields rather than from this side's wall clock, which would
        // include the request leaving and the answer arriving.
        let said: serde_json::Value =
            serde_json::from_str(r#"{"timings":{"predicted_n":100,"predicted_ms":2000.0}}"#)
                .expect("shape");
        let mut run = Run::default();
        read_into(&said, &mut run);
        assert_eq!(run.generation, Some(50.0));
    }
}
