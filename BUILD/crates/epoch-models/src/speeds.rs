//! How fast this machine actually answers, and what that says about a model it has never run.
//!
//! ## Why not a table of graphics cards
//!
//! The obvious way to estimate tokens per second is the one `whichllm` takes: a curated table of
//! GPU memory bandwidths, times a per-quantisation efficiency factor. It works, and it needs a
//! shipped list that is wrong for every card nobody put in it.
//!
//! This machine can be asked instead. One measured answer gives
//!
//! ```text
//! throughput = tokens per second x bytes of weights
//! ```
//!
//! which is what the card, the driver, the runtime's kernels and the quantisation add up to —
//! measured together rather than modelled apart. Every other model that **fits** is then that
//! number divided by its size.
//!
//! Contrasted against `whichllm`'s model on the machine this was written on (RTX 4070 SUPER,
//! 504 GB/s): its formula says 37.5 tok/s for a 7.4 GB Q4 model and the real answer is 46.3, so
//! its table is 19% low here. The measured throughput is exact by construction, because it is
//! this machine's own answer.
//!
//! ## Only for models that fit
//!
//! The same contrast is what says where to stop. For a model too big for the card, `whichllm`
//! estimates 7.0 tok/s and the measured answer is 2.4 — three times out, because what happens
//! then is not throughput at all but weights crossing PCIe. So a model that does not fit gets no
//! number: it gets the sentence *it will not fit, and it will be slow*, which is the honest thing
//! and the only thing that was true in the measurement.
//!
//! ## Measured beats estimated, and the two never blur
//!
//! A row says which it is. An estimate carries `~`; a measurement carries the runtime it was
//! taken on and no tilde. Nothing here averages the two.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One real answer, timed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    /// What the runtime calls it.
    pub model: String,
    /// Which runtime answered: `ollama`, `llama_cpp`, `lm_studio`.
    pub runtime: String,
    /// The weights, in bytes — what the throughput is divided by.
    pub bytes: u64,
    pub tokens_per_second: f64,
    /// Whether it fitted on the card. A measurement of a model that spilled says nothing about
    /// this machine's throughput, so it is kept and never used to calibrate.
    pub fitted: bool,
    pub at: u64,
}

/// Everything this machine has actually run, timed.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Speeds {
    #[serde(default)]
    kept: Vec<Measured>,
}

pub fn path(library: &Path) -> PathBuf {
    library.join("speeds.json")
}

impl Speeds {
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

    /// Keep one, replacing an older answer for the same model on the same runtime.
    ///
    /// Replaced rather than averaged: a card with a picture on it and a card with nothing on it
    /// give different answers, and the newer one describes the machine as it is now.
    pub fn remember(&mut self, one: Measured) {
        self.kept
            .retain(|kept| !(kept.model == one.model && kept.runtime == one.runtime));
        self.kept.push(one);
    }

    /// What was measured for one model, newest first.
    pub fn about<'a>(&'a self, model: &str) -> Vec<&'a Measured> {
        let mut found: Vec<&Measured> = self.kept.iter().filter(|it| it.model == model).collect();
        found.sort_by_key(|one| std::cmp::Reverse(one.at));
        found
    }

    /// This machine's effective throughput, in bytes per second of weights read.
    ///
    /// `None` until something has been measured that actually fitted — and that is a real answer
    /// rather than a missing one: with nothing measured, Epoch has nothing true to say about how
    /// fast a model it has never run would be, and says so.
    ///
    /// The **best** of the fitting measurements rather than the average. A slow answer can come
    /// from a card that was busy drawing; a fast one cannot come from a card that was not.
    pub fn throughput(&self) -> Option<f64> {
        self.kept
            .iter()
            .filter(|one| one.fitted && one.bytes > 0 && one.tokens_per_second > 0.0)
            .map(|one| one.tokens_per_second * one.bytes as f64)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    /// What a model of this size would answer at, on this machine.
    ///
    /// `None` when nothing has been measured, and `None` for a model that does not fit — where
    /// the number would be three times out, measured.
    pub fn estimate(&self, bytes: u64, fits: bool) -> Option<f64> {
        if !fits || bytes == 0 {
            return None;
        }
        Some(self.throughput()? / bytes as f64)
    }
}

/// Ask one runtime for a short answer and time it.
///
/// ## Why a second call
///
/// The first one loads the model off disk — measured on this machine at 10.4 s for a 7.4 GB model
/// against 3.5 s of actual answering. Timing that would report the disk rather than the card, and
/// it is a number that happens once. So the first answer is thrown away and the second is timed:
/// what somebody experiences from the second message onwards.
///
/// ## Why the runtime's own count
///
/// Tokens are not words and are not characters, and a count taken here would be Epoch's guess at
/// somebody else's tokeniser. Both shapes report it: OpenAI-compatible servers under
/// `usage.completion_tokens`, Ollama under `eval_count`. An answer that carries no count is
/// reported as unmeasurable rather than divided by an estimate.
pub fn time_it(at: &str, model: &str, ollama: bool) -> Result<f64, String> {
    let at = &at
        .trim_end_matches('/')
        .replace("://localhost:", "://127.0.0.1:")
        .replace("://[::1]:", "://127.0.0.1:");
    let ask = |most: u32| -> Result<(f64, u64), String> {
        let (url, body) = if ollama {
            (
                format!("{at}/api/chat"),
                serde_json::json!({
                    "model": model,
                    "messages": [{ "role": "user", "content": WORDS }],
                    "stream": false,
                    "options": { "num_predict": most }
                }),
            )
        } else {
            (
                format!("{at}/v1/chat/completions"),
                serde_json::json!({
                    "model": model,
                    "messages": [{ "role": "user", "content": WORDS }],
                    "stream": false,
                    "max_tokens": most
                }),
            )
        };

        let began = std::time::Instant::now();
        let said: serde_json::Value = ureq::post(&url)
            .timeout(std::time::Duration::from_secs(900))
            .send_json(body)
            .map_err(|why| format!("{model} did not answer: {why}"))?
            .into_json()
            .map_err(|why| format!("{model} answered something unreadable: {why}"))?;
        let took = began.elapsed().as_secs_f64();

        // The runtime's own count, in whichever place its shape puts it.
        let made = said["usage"]["completion_tokens"]
            .as_u64()
            .or_else(|| said["eval_count"].as_u64())
            .ok_or_else(|| format!("{model} answered without saying how many tokens it wrote"))?;
        Ok((took, made))
    };

    // Loads it. Timed by nobody.
    ask(8)?;
    let (took, made) = ask(160)?;
    if took <= 0.0 || made == 0 {
        return Err(format!("{model} answered nothing to time"));
    }
    Ok(made as f64 / took)
}

/// Time one model at a stated context, through Ollama's own API.
///
/// ## Why this is Ollama's shape and not a general one
///
/// A curve varies the context, and where that can be said differs per runtime — measured
/// 2026-08-30. Ollama takes `options.num_ctx` **per request**, so a curve is eight requests and
/// nothing has to be restarted. LM Studio takes it only at *load*, through `lms load -c`, which
/// is a different mechanism and a different cost; writing one function that pretended both were
/// the same request would be the flattening ADR-0026 refuses.
///
/// `None` where it did not answer at all — too big for the card at that size, or refused. That
/// is a row of the curve like any other: it did not load, and the map says so.
pub fn at_context(at: &str, model: &str, context: u32) -> Option<f64> {
    let at = at
        .trim_end_matches('/')
        .replace("://localhost:", "://127.0.0.1:")
        .replace("://[::1]:", "://127.0.0.1:");
    let ask = |most: u32| -> Option<(f64, u64)> {
        let began = std::time::Instant::now();
        let said: serde_json::Value = ureq::post(&format!("{at}/api/chat"))
            .timeout(std::time::Duration::from_secs(900))
            .send_json(serde_json::json!({
                "model": model,
                "messages": [{ "role": "user", "content": WORDS }],
                "stream": false,
                // **Both, together.** Ollama reloads the model whenever `num_ctx` changes, so
                // every rung of the ladder is a real load at that size — which is the thing being
                // measured, and the reason a curve here takes minutes rather than seconds.
                "options": { "num_ctx": context, "num_predict": most }
            }))
            .ok()?
            .into_json()
            .ok()?;
        let took = began.elapsed().as_secs_f64();
        let made = said["eval_count"]
            .as_u64()
            .or_else(|| said["usage"]["completion_tokens"].as_u64())?;
        Some((took, made))
    };

    // Loads it at this size. Timed by nobody, exactly as `time_it` does — the load is what
    // changes between rungs, and timing it would measure the disk.
    ask(8)?;
    let (took, made) = ask(160)?;
    (took > 0.0 && made > 0).then_some(made as f64 / took)
}

/// What it is asked. Ordinary prose, in the language this was measured in, and long enough to
/// need a paragraph rather than a word.
const WORDS: &str = "Explica en un parrafo que es un faro y para que sirve.";

#[cfg(test)]
mod tests {
    use super::*;

    fn one(model: &str, runtime: &str, gb: f64, rate: f64, fitted: bool) -> Measured {
        Measured {
            model: model.into(),
            runtime: runtime.into(),
            bytes: (gb * 1e9) as u64,
            tokens_per_second: rate,
            fitted,
            at: 1,
        }
    }

    #[test]
    fn nothing_measured_is_nothing_said() {
        // The cold instrument: with no measurement, Epoch has nothing true to say about a model
        // it has never run, and an invented number would be exactly the gauge nobody can explain.
        let speeds = Speeds::default();
        assert_eq!(speeds.throughput(), None);
        assert_eq!(speeds.estimate(7_400_000_000, true), None);
    }

    #[test]
    fn one_real_answer_estimates_the_rest() {
        // Measured on the machine this was written on: gemma4:12b, 7.4 GB, 46.3 tok/s.
        let mut speeds = Speeds::default();
        speeds.remember(one("gemma4-12b", "llama_cpp", 7.4, 46.3, true));

        // A smaller model reads fewer bytes per token, so it answers faster.
        let smaller = speeds.estimate(4_000_000_000, true).unwrap();
        assert!(smaller > 46.3, "{smaller}");
        // And a bigger one slower.
        let bigger = speeds.estimate(9_500_000_000, true).unwrap();
        assert!(bigger < 46.3, "{bigger}");
    }

    #[test]
    fn a_model_that_does_not_fit_gets_no_number() {
        // Measured: for a model too big for the card the bandwidth model is three times out,
        // because what happens then is not throughput but weights crossing PCIe.
        let mut speeds = Speeds::default();
        speeds.remember(one("gemma4-12b", "llama_cpp", 7.4, 46.3, true));
        assert_eq!(speeds.estimate(17_700_000_000, false), None);
    }

    #[test]
    fn a_measurement_that_spilled_never_calibrates_anything() {
        // It says what that model did on this machine, and nothing about the machine's throughput.
        let mut speeds = Speeds::default();
        speeds.remember(one("qwen3.8", "llama_cpp", 17.7, 2.4, false));
        assert_eq!(speeds.throughput(), None);
        assert_eq!(
            speeds.about("qwen3.8").len(),
            1,
            "and it is still remembered"
        );
    }

    #[test]
    fn measuring_again_replaces_rather_than_averages() {
        // A card busy drawing answers slower. The newer answer describes the machine as it is.
        let mut speeds = Speeds::default();
        speeds.remember(one("gemma4-12b", "llama_cpp", 7.4, 12.0, true));
        speeds.remember(Measured {
            at: 2,
            ..one("gemma4-12b", "llama_cpp", 7.4, 46.3, true)
        });
        assert_eq!(speeds.about("gemma4-12b").len(), 1);
        assert_eq!(speeds.about("gemma4-12b")[0].tokens_per_second, 46.3);

        // The same model on another runtime is another answer, not a replacement.
        speeds.remember(one("gemma4-12b", "ollama", 7.4, 30.0, true));
        assert_eq!(speeds.about("gemma4-12b").len(), 2);
    }
}
