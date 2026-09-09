//! Speaking one voice in somebody else's timbre.
//!
//! ## The chain, and why it is two models and not one
//!
//! ```text
//! Piper .wav ──▶ 16 kHz ──▶ encoder ──▶ features ──▶┐
//!                    └────▶ pitch ────────────────▶ generator ──▶ .wav
//! ```
//!
//! The **encoder** is ContentVec: it hears speech and answers what was *said*, stripped of who
//! said it. The **generator** is the voice somebody downloaded, and it can only answer that
//! question — it has never heard audio. So one model per RVC voice, and one encoder shared by
//! every one of them, which is why the encoder is converted with the toolchain rather than
//! beside each voice.
//!
//! ## What is measured and what is inherited
//!
//! The frame rates are not free parameters: the encoder subsamples 16 kHz audio by 320, so it
//! answers at 50 frames a second, and the generator was trained against 100. RVC repeats each
//! feature twice to bridge them, and this does the same — **because that is what the weights
//! were trained against**, not because doubling is a reasonable idea.
//!
//! Everything else about the model is read from the sidecar the conversion wrote, not assumed:
//! its sample rate, its embedding width and how many speakers it holds.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;

use super::signal::{self, Sound};

/// What the conversion wrote down beside the model.
#[derive(Debug, Clone)]
pub struct Made {
    /// What the model speaks at. RVC writes `40k` or `48k`.
    pub rate: u32,
    /// 768 for v2, 256 for v1 — measured from the tensor, not read off a filename.
    pub embedding: usize,
    /// How many voices are inside it. Most are one; some are a collection.
    pub speakers: i64,
    /// Whether it was trained with a pitch track. **Everything Epoch converts is**, and a model
    /// that was not would need a different graph — so it is refused by name rather than fed
    /// arrays it has no inputs for.
    pub pitched: bool,
    /// How many frames the graph is actually correct at.
    ///
    /// ## The one number the graph lies about
    ///
    /// The export declares its frame axis dynamic and it is not. RVC's relative attention pads
    /// with `int(length) - 1` and slices its position embeddings with Python arithmetic, so the
    /// tracer freezes both into constants and any other length answers
    ///
    /// ```text
    /// Reshape … Input shape:{1,2,3591299}, requested shape:{1,2,1341,2679}
    /// ```
    ///
    /// **So it is written down by the process that chose it**, rather than read back off the
    /// model — asking the graph returns the declared axis, which is the value that is wrong.
    /// Models converted before this was known say nothing and are read as 100, which is exactly
    /// what they were traced at.
    pub window: usize,
}

/// Read the sidecar.
pub fn made(model: &Path) -> Result<Made, String> {
    let beside = model.with_extension("json");
    let raw = std::fs::read_to_string(&beside).map_err(|why| {
        format!(
            "{beside:?} is not beside that model, and it is what says the rate the voice speaks \
             at: {why}"
        )
    })?;
    let read: serde_json::Value =
        serde_json::from_str(&raw).map_err(|why| format!("{beside:?} is not readable: {why}"))?;

    // `40k` -> 40000. Written by the checkpoint's author, so a rate Epoch cannot read is refused
    // rather than defaulted: a voice played at the wrong rate is not a broken voice, it is a
    // working voice at the wrong speed, which sounds like a bad model.
    let said = read
        .get("sr")
        .and_then(|it| it.as_str())
        .unwrap_or_default();
    let rate = match said.trim_end_matches('k').parse::<u32>() {
        Ok(thousands) if said.ends_with('k') => thousands * 1000,
        _ => match said.parse::<u32>() {
            Ok(exact) if exact >= 8000 => exact,
            _ => {
                return Err(format!(
                    "That model does not say what rate it speaks at (`sr` was {said:?}), and \
                     Epoch will not guess — a wrong rate plays a working voice at the wrong speed."
                ))
            }
        },
    };

    Ok(Made {
        rate,
        embedding: read
            .get("embedding")
            .and_then(|it| it.as_u64())
            .unwrap_or(768) as usize,
        speakers: read
            .get("speakers")
            .and_then(|it| it.as_i64())
            .unwrap_or(1)
            .max(1),
        // `1` is the ordinary value; anything else is a model this graph does not fit.
        pitched: read.get("f0").and_then(|it| it.as_i64()).unwrap_or(1) == 1,
        window: read
            .get("window")
            .and_then(|it| it.as_u64())
            .unwrap_or(100)
            .max(8) as usize,
    })
}

/// What the encoder was trained to listen at, and what the generator was trained to be fed.
const LISTENING: u32 = 16_000;
/// The encoder's subsampling: 320 samples of 16 kHz audio per feature frame, so 50 a second.
const PER_FEATURE: usize = 320;

/// Sessions, kept for as long as the process lives.
///
/// **Built once and held.** A 110 MB graph takes real time to open, and a character answering a
/// question would otherwise pay it on every sentence. `Session::run` wants `&mut self`, so this
/// is a `Mutex` rather than a read-heavy map — which costs nothing, because the World runs one
/// turn at a time by design and only one voice is ever speaking.
static OPEN: Mutex<Option<HashMap<PathBuf, Session>>> = Mutex::new(None);

/// Forget every open model.
///
/// Two large graphs is a few hundred megabytes, and a machine that also draws has better uses
/// for it. The same reasoning that releases a language model when somebody stops talking.
pub fn release() {
    if let Ok(mut open) = OPEN.lock() {
        *open = None;
    }
}

/// Run one model, opening it if this is the first time.
fn with_session<T>(
    model: &Path,
    run: impl FnOnce(&mut Session) -> Result<T, String>,
) -> Result<T, String> {
    super::runtime::arm()?;
    let mut open = OPEN
        .lock()
        .map_err(|_| "the voices are in a bad state".to_owned())?;
    let held = open.get_or_insert_with(HashMap::new);
    if !held.contains_key(model) {
        let session = Session::builder()
            .and_then(|mut it| it.commit_from_file(model))
            .map_err(|why| format!("{model:?} could not be opened: {why}"))?;
        held.insert(model.to_owned(), session);
    }
    let session = held.get_mut(model).expect("it was just inserted");
    run(session)
}

/// Speak `sound` in the timbre of `model`, shifted by `semitones`.
///
/// `speaker` picks a voice inside a model that holds several; a model with one ignores it.
///
/// **Returns audio at the model's own rate**, not at the input's. Resampling it back is the
/// caller's decision, because whoever plays it knows what it is being played into.
pub fn recolour(
    model: &Path,
    sound: &Sound,
    semitones: f32,
    speaker: i64,
) -> Result<Sound, String> {
    let made = made(model)?;
    if !made.pitched {
        return Err(
            "That voice was trained without a pitch track, which needs a different graph than \
             the one Epoch builds. It converted correctly and cannot be spoken here."
                .to_owned(),
        );
    }

    let encoder = super::encoder();
    if !encoder.is_file() {
        return Err(
            "The encoder that listens to a voice is not on this machine yet, so nothing can be \
             recoloured. Epoch can fetch and convert it."
                .to_owned(),
        );
    }

    let heard = signal::resample(sound, LISTENING);
    if heard.samples.len() < PER_FEATURE * 4 {
        return Err("that is too short to recolour".to_owned());
    }

    // ── what was said ────────────────────────────────────────────────────────────────────────
    let samples = heard.samples.clone();
    let (features, width) = with_session(&encoder, |session| {
        let input = Tensor::from_array(([1_i64, samples.len() as i64], samples))
            .map_err(|why| format!("the audio could not be handed to the encoder: {why}"))?;
        let answered = session
            .run(ort::inputs!["input_values" => input])
            .map_err(|why| format!("the encoder would not run: {why}"))?;
        let (shape, values) = answered[0]
            .try_extract_tensor::<f32>()
            .map_err(|why| format!("the encoder answered something unreadable: {why}"))?;
        let width = *shape.last().unwrap_or(&0) as usize;
        Ok((values.to_vec(), width))
    })?;

    if width != made.embedding {
        return Err(format!(
            "The encoder answers {width} numbers a frame and that voice was trained against \
             {}. They are not interchangeable.",
            made.embedding
        ));
    }
    let frames = features.len().checked_div(width).unwrap_or(0);

    // ── 50 frames a second becomes 100 ───────────────────────────────────────────────────────
    let mut doubled = Vec::with_capacity(features.len() * 2);
    for frame in 0..frames {
        let row = &features[frame * width..(frame + 1) * width];
        doubled.extend_from_slice(row);
        doubled.extend_from_slice(row);
    }

    // The generator is fed whichever is shorter: the features it was given, or the audio those
    // features came from. They can differ by a frame at the end, and a pitch array one longer
    // than the feature array is an out-of-bounds read inside the model.
    let length = (frames * 2).min(heard.samples.len() / (LISTENING as usize / 100));
    if length < 2 {
        return Err("that is too short to recolour".to_owned());
    }
    doubled.truncate(length * width);

    // ── the melody ───────────────────────────────────────────────────────────────────────────
    let pitch = signal::pitch(&heard, length, semitones);

    // ── somebody else's voice, a window at a time ────────────────────────────────────────────
    let speaker = speaker.clamp(0, made.speakers.saturating_sub(1));
    let window = made.window;
    let overlap = (window / 5).clamp(2, 50);
    let hop = window.saturating_sub(overlap).max(1);
    // One feature frame is 10 ms, so this is how many samples of the model's own audio each
    // frame becomes. Exact, which is what lets a crossfade land on a sample boundary.
    let per_frame = made.rate as usize / signal::FRAMES_PER_SECOND as usize;

    let mut out = vec![0.0_f32; length * per_frame];
    let mut at = 0_usize;
    while at < length {
        // Every chunk is exactly `window` frames, padded with silence at the tail. That is not
        // tidiness: the graph is only correct at the length it was traced at.
        let mut feats = vec![0.0_f32; window * width];
        let mut coarse = vec![0_i64; window];
        let mut hertz = vec![0.0_f32; window];
        let taken = window.min(length - at);
        feats[..taken * width].copy_from_slice(&doubled[at * width..(at + taken) * width]);
        coarse[..taken].copy_from_slice(&pitch.coarse[at..at + taken]);
        hertz[..taken].copy_from_slice(&pitch.hertz[at..at + taken]);

        let made_rate = made.rate;
        let piece = with_session(model, move |session| {
            let feats = Tensor::from_array(([1_i64, window as i64, width as i64], feats))
                .map_err(|why| format!("the features could not be handed over: {why}"))?;
            let p_len = Tensor::from_array(([1_i64], vec![window as i64]))
                .map_err(|why| format!("the length could not be handed over: {why}"))?;
            let coarse = Tensor::from_array(([1_i64, window as i64], coarse))
                .map_err(|why| format!("the pitch could not be handed over: {why}"))?;
            let hertz = Tensor::from_array(([1_i64, window as i64], hertz))
                .map_err(|why| format!("the pitch could not be handed over: {why}"))?;
            let sid = Tensor::from_array(([1_i64], vec![speaker]))
                .map_err(|why| format!("the speaker could not be handed over: {why}"))?;

            let answered = session
                .run(ort::inputs![
                    "feats" => feats,
                    "p_len" => p_len,
                    "pitch" => coarse,
                    "pitchf" => hertz,
                    "sid" => sid,
                ])
                .map_err(|why| format!("that voice would not run: {why}"))?;
            let (_, values) = answered[0]
                .try_extract_tensor::<f32>()
                .map_err(|why| format!("that voice answered something unreadable: {why}"))?;
            Ok(Sound {
                samples: values.to_vec(),
                rate: made_rate,
            })
        })?;

        // Lay it down, fading across whatever the previous chunk already wrote. A hard join
        // clicks — the two chunks are each internally consistent and are not continuous at
        // their seam, which is audible on every one of them.
        let start = at * per_frame;
        let fade = if at == 0 { 0 } else { overlap * per_frame };
        for (n, sample) in piece.samples.iter().enumerate() {
            let target = start + n;
            if target >= out.len() {
                break;
            }
            out[target] = if n < fade {
                let across = (n as f32 + 1.0) / (fade as f32 + 1.0);
                out[target] * (1.0 - across) + sample * across
            } else {
                *sample
            };
        }

        if at + window >= length {
            break;
        }
        at += hop;
    }
    let spoken = Sound {
        samples: out,
        rate: made.rate,
    };

    // **The side effect, not the exit code**, one subsystem along: a graph that ran and answered
    // silence is a voice somebody cannot hear, and it would arrive as a working file.
    if spoken.samples.iter().all(|it| it.abs() < 1e-6) {
        return Err(
            "That voice ran and produced silence. The model converted, so this is about the \
             audio it was given rather than the file."
                .to_owned(),
        );
    }
    Ok(spoken)
}

/// The whole errand, from one `.wav` to another.
///
/// Kept separate from `recolour` because this is where the *output* rate is decided, and that is
/// a question about whatever is going to play it rather than about the voice.
pub fn recolour_wav(
    model: &Path,
    wav: &[u8],
    semitones: f32,
    speaker: i64,
    into: u32,
) -> Result<Vec<u8>, String> {
    let sound = signal::read_wav(wav)?;
    let spoken = recolour(model, &sound, semitones, speaker)?;
    let out = if into == 0 || into == spoken.rate {
        spoken
    } else {
        signal::resample(&spoken, into)
    };
    Ok(signal::write_wav(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A model that does not exist, with a sidecar that does.
    ///
    /// **Named by the caller, not derived from the body.** It was `body.len()` for one run, and
    /// two different sidecars of the same length then shared a file -- so whichever test the
    /// runner reached second read the other one's model and failed with a true assertion about
    /// the wrong file. A name that is unique by coincidence is a name that collides.
    fn sidecar(called: &str, body: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("epoch-timbre-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temp folder");
        let model = dir.join(format!("{called}.onnx"));
        std::fs::write(model.with_extension("json"), body).expect("the sidecar");
        model
    }

    #[test]
    fn a_model_says_its_own_rate_and_width() {
        let model = sidecar(
            "says-its-own-rate",
            r#"{"version":"v2","sr":"40k","f0":1,"info":"160epoch","embedding":768,"speakers":109}"#,
        );
        let read = made(&model).expect("the sidecar is readable");
        assert_eq!(read.rate, 40_000);
        assert_eq!(read.embedding, 768);
        assert_eq!(read.speakers, 109);
        assert!(read.pitched);
    }

    #[test]
    fn a_rate_epoch_cannot_read_is_refused_rather_than_defaulted() {
        // The interesting failure, because it does not look like one: a voice played at 40 kHz
        // when it was trained at 48 is not silent or broken, it is the right voice at the wrong
        // speed -- which everybody hears as a bad model.
        let model = sidecar("no-rate", r#"{"version":"v2","embedding":768}"#);
        let refused = made(&model).expect_err("a missing rate must be refused");
        assert!(refused.contains("wrong speed"), "{refused}");
    }

    #[test]
    fn a_model_without_a_pitch_track_is_named_rather_than_fed_the_wrong_graph() {
        let model = sidecar(
            "no-pitch",
            r#"{"sr":"48k","f0":0,"embedding":768,"speakers":1}"#,
        );
        assert!(!made(&model).expect("readable").pitched);
        let refused = recolour(
            &model,
            &Sound {
                samples: vec![0.0; 32_000],
                rate: 16_000,
            },
            0.0,
            0,
        )
        .expect_err("a model with no pitch input must be refused");
        assert!(refused.contains("different graph"), "{refused}");
    }

    /// The whole chain, from a `.wav` Piper wrote to a `.wav` in somebody else's voice.
    ///
    /// **It ends at audio on disk that somebody can listen to.** Every part of this passed its
    /// own test while the product could not speak, which is the only reason this exists: a
    /// measurement of the pieces is not a measurement of the chain.
    #[test]
    #[ignore = "needs a converted voice, the encoder and the runtime"]
    fn a_piper_sentence_comes_back_in_somebody_else_s_voice() {
        let model = std::env::var("EPOCH_TEST_ONNX").unwrap_or_default();
        let spoken = std::env::var("EPOCH_TEST_WAV").unwrap_or_default();
        if model.is_empty() || spoken.is_empty() {
            eprintln!("set EPOCH_TEST_ONNX and EPOCH_TEST_WAV");
            return;
        }
        let semitones: f32 = std::env::var("EPOCH_TEST_SEMITONES")
            .ok()
            .and_then(|it| it.parse().ok())
            .unwrap_or(0.0);

        let wav = std::fs::read(&spoken).expect("the spoken line");
        let heard = signal::read_wav(&wav).expect("Piper writes a readable WAV");
        eprintln!("in : {:.2}s at {} Hz", heard.seconds(), heard.rate);

        let started = std::time::Instant::now();
        let out = recolour_wav(Path::new(&model), &wav, semitones, 0, heard.rate)
            .expect("the chain runs");
        let took = started.elapsed();

        let into = std::env::temp_dir().join("epoch-recoloured.wav");
        std::fs::write(&into, &out).expect("it can be written");
        let read = signal::read_wav(&out).expect("Epoch's own WAV is readable");
        eprintln!(
            "out: {:.2}s at {} Hz in {took:?} ({:.1}x real time) -> {}",
            read.seconds(),
            read.rate,
            read.seconds() / took.as_secs_f32(),
            into.display()
        );

        // Same length, because nothing here stretches time -- a chain that lost or gained
        // seconds would still play and would be out of step with everything measuring it.
        assert!(
            (read.seconds() - heard.seconds()).abs() < 0.25,
            "{:.2}s went in and {:.2}s came out",
            heard.seconds(),
            read.seconds()
        );
    }

    #[test]
    fn a_machine_with_no_runtime_says_so_instead_of_failing_later() {
        // Nothing here is a claim about *this* machine: whichever way it answers, the sentence
        // must name the thing that is missing rather than arriving as a `dlopen` error.
        if super::super::runtime::ready() && super::super::encoder().is_file() {
            return;
        }
        let model = sidecar(
            "nothing-installed",
            r#"{"sr":"40k","f0":1,"embedding":768,"speakers":1}"#,
        );
        let refused = recolour(
            &model,
            &Sound {
                samples: vec![0.1; 32_000],
                rate: 16_000,
            },
            0.0,
            0,
        )
        .expect_err("nothing is installed, so this cannot work");
        assert!(
            refused.contains("encoder") || refused.contains("ONNX Runtime"),
            "{refused}"
        );
    }
}
