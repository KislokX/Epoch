//! The signal work between Piper's `.wav` and an RVC model: reading, resampling, pitch.
//!
//! ## Why this is Rust and not another Python script
//!
//! Converting a voice happens **once**; speaking one happens every time somebody talks. The
//! conversion is allowed to be a heavy Python errand precisely because this half is not — a
//! character answering a question must not start an interpreter.
//!
//! ## Pitch, and what is honestly claimed about it
//!
//! RVC is very sensitive to its pitch track, and the reference implementation offers several
//! estimators — `pm`, `harvest`, `crepe`, `rmvpe` — of which the last two are neural and would
//! each be another model to fetch, convert and keep in step.
//!
//! What is here is **YIN**: autocorrelation with the cumulative mean normalised difference,
//! parabolic interpolation and an absolute threshold. That is the same family as `pm`, which
//! RVC shipped as its default for a long time, and it is named as such rather than described as
//! *the* pitch. If a neural estimator earns its way in later it will be because somebody
//! measured this one falling short, not because it is newer.

/// Sound, as everything here passes it around: mono, `f32` in roughly `[-1, 1]`, and a rate.
#[derive(Debug, Clone)]
pub struct Sound {
    pub samples: Vec<f32>,
    pub rate: u32,
}

impl Sound {
    pub fn seconds(&self) -> f32 {
        if self.rate == 0 {
            0.0
        } else {
            self.samples.len() as f32 / self.rate as f32
        }
    }
}

/// Read a PCM `.wav` — which is what Piper writes and what whisper is handed.
///
/// **Deliberately narrow.** 16-bit and 32-bit float PCM, any channel count, mixed to mono. A
/// compressed WAV is refused by name rather than misread: `format 2` decoded as PCM is noise,
/// and noise arriving three subsystems later is the failure that takes an afternoon.
pub fn read_wav(bytes: &[u8]) -> Result<Sound, String> {
    if bytes.len() < 12 || !bytes.starts_with(b"RIFF") || &bytes[8..12] != b"WAVE" {
        return Err("that is not a WAV".to_owned());
    }

    let mut at = 12;
    let mut format = 0_u16;
    let mut channels = 0_u16;
    let mut rate = 0_u32;
    let mut bits = 0_u16;
    let mut data: Option<&[u8]> = None;

    while at + 8 <= bytes.len() {
        let id = &bytes[at..at + 4];
        let size = u32::from_le_bytes([bytes[at + 4], bytes[at + 5], bytes[at + 6], bytes[at + 7]])
            as usize;
        let body = at + 8;
        let end = body.saturating_add(size).min(bytes.len());
        match id {
            b"fmt " if end - body >= 16 => {
                let f = &bytes[body..end];
                format = u16::from_le_bytes([f[0], f[1]]);
                channels = u16::from_le_bytes([f[2], f[3]]);
                rate = u32::from_le_bytes([f[4], f[5], f[6], f[7]]);
                bits = u16::from_le_bytes([f[14], f[15]]);
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        // Chunks are word aligned, and a file that omits the pad byte would otherwise walk off
        // by one and read the next chunk's id out of its own body.
        at = body + size + (size & 1);
    }

    let data = data.ok_or("that WAV has no audio in it")?;
    if channels == 0 || rate == 0 {
        return Err("that WAV declares no format".to_owned());
    }

    // 1 is integer PCM, 3 is IEEE float, 0xFFFE is "extensible" and carries the real one in a
    // sub-format Epoch does not read — so it is refused rather than assumed.
    let mono: Vec<f32> = match (format, bits) {
        (1, 16) => mix(
            data.chunks_exact(2)
                .map(|it| i16::from_le_bytes([it[0], it[1]]) as f32 / 32768.0),
            channels as usize,
        ),
        (1, 8) => mix(
            data.iter().map(|it| (*it as f32 - 128.0) / 128.0),
            channels as usize,
        ),
        (3, 32) => mix(
            data.chunks_exact(4)
                .map(|it| f32::from_le_bytes([it[0], it[1], it[2], it[3]])),
            channels as usize,
        ),
        _ => {
            return Err(format!(
                "Epoch reads 8- and 16-bit PCM and 32-bit float WAVs; this one is format \
                 {format} at {bits} bits."
            ))
        }
    };

    Ok(Sound {
        samples: mono,
        rate,
    })
}

/// Average the channels together.
fn mix(samples: impl Iterator<Item = f32>, channels: usize) -> Vec<f32> {
    if channels <= 1 {
        return samples.collect();
    }
    let all: Vec<f32> = samples.collect();
    all.chunks(channels)
        .map(|frame| frame.iter().sum::<f32>() / frame.len() as f32)
        .collect()
}

/// Write mono 16-bit PCM — what Piper writes, so what everything downstream already reads.
pub fn write_wav(sound: &Sound) -> Vec<u8> {
    let bytes = sound.samples.len() * 2;
    let mut out = Vec::with_capacity(44 + bytes);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((36 + bytes) as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1_u16.to_le_bytes()); // mono
    out.extend_from_slice(&sound.rate.to_le_bytes());
    out.extend_from_slice(&(sound.rate * 2).to_le_bytes()); // bytes per second
    out.extend_from_slice(&2_u16.to_le_bytes()); // block align
    out.extend_from_slice(&16_u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(bytes as u32).to_le_bytes());
    for sample in &sound.samples {
        let clipped = (sample * 32767.0).clamp(-32768.0, 32767.0);
        out.extend_from_slice(&(clipped as i16).to_le_bytes());
    }
    out
}

/// How many taps either side of a resampled point are read.
///
/// Sixteen is a compromise nobody should have to guess at: the kernel is a windowed sinc, so
/// this is the trade between stop-band rejection and arithmetic, and the arithmetic is small
/// enough here that going below it would buy nothing anybody could hear.
const TAPS: isize = 16;

/// Resample, band-limited.
///
/// **Not linear interpolation.** Piper speaks at 22 050 and the encoder wants 16 000, which is a
/// *downsample* — and linear interpolation folds everything above the new Nyquist back into the
/// audible band as a metallic ring. The kernel's cutoff moves with the ratio, so the same code
/// is correct in both directions.
pub fn resample(sound: &Sound, to: u32) -> Sound {
    if to == 0 || sound.rate == 0 || to == sound.rate || sound.samples.is_empty() {
        return Sound {
            samples: sound.samples.clone(),
            rate: if to == 0 { sound.rate } else { to },
        };
    }

    let ratio = to as f64 / sound.rate as f64;
    // Going down, the kernel must cut at the *new* Nyquist; going up, at the old one. `min(1)`
    // is that whole rule, and leaving it out is what makes a downsample alias.
    let cutoff = ratio.min(1.0) * 0.95;
    let width = (TAPS as f64 / cutoff).ceil() as isize;
    let length = ((sound.samples.len() as f64) * ratio).round() as usize;
    let mut out = Vec::with_capacity(length);

    for n in 0..length {
        let centre = n as f64 / ratio;
        let base = centre.floor() as isize;
        let mut sum = 0.0_f64;
        let mut weight = 0.0_f64;
        for tap in (base - width)..=(base + width) {
            if tap < 0 || tap as usize >= sound.samples.len() {
                continue;
            }
            let distance = centre - tap as f64;
            let w = sinc(distance * cutoff) * blackman(distance / width as f64);
            sum += sound.samples[tap as usize] as f64 * w;
            weight += w;
        }
        // Normalising by the weights actually used keeps the edges from fading, which a fixed
        // gain does not — the first and last few samples read fewer taps than the middle.
        out.push(if weight.abs() > 1e-9 {
            (sum / weight) as f32
        } else {
            0.0
        });
    }

    Sound {
        samples: out,
        rate: to,
    }
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-9 {
        1.0
    } else {
        let pi_x = std::f64::consts::PI * x;
        pi_x.sin() / pi_x
    }
}

fn blackman(x: f64) -> f64 {
    if x.abs() >= 1.0 {
        return 0.0;
    }
    let t = std::f64::consts::PI * (x + 1.0);
    0.42 - 0.5 * t.cos() + 0.08 * (2.0 * t).cos()
}

/// The lowest and highest pitch looked for, in hertz.
///
/// RVC's own range, kept because the coarse scale below is defined against exactly these two
/// numbers: changing one here and not the other would silently move every note.
pub const LOWEST: f32 = 50.0;
pub const HIGHEST: f32 = 1100.0;

/// One pitch reading per 10 ms, which is the rate the model was trained at.
pub const FRAMES_PER_SECOND: u32 = 100;

/// A pitch track, in the two forms the model is handed.
pub struct Pitch {
    /// Hertz, `0.0` where nothing was voiced.
    pub hertz: Vec<f32>,
    /// The same track on RVC's 1–255 mel scale, `0` where nothing was voiced.
    pub coarse: Vec<i64>,
}

/// Estimate pitch, and shift it by a number of semitones.
///
/// The shift belongs here rather than in the caller because both outputs have to move together:
/// `coarse` is derived from `hertz` *after* the shift, and deriving it before would hand the
/// model a note and a description of a different note.
pub fn pitch(sound: &Sound, frames: usize, semitones: f32) -> Pitch {
    let hop = (sound.rate / FRAMES_PER_SECOND).max(1) as usize;
    // The window has to hold two of the longest period looked for, or the lowest notes cannot
    // be seen at all.
    let window = ((sound.rate as f32 / LOWEST).ceil() as usize * 2).next_power_of_two();
    let shift = 2.0_f32.powf(semitones / 12.0);

    let mut hertz = Vec::with_capacity(frames);
    for frame in 0..frames {
        let start = frame * hop;
        let found = if start + window <= sound.samples.len() {
            yin(&sound.samples[start..start + window], sound.rate)
        } else {
            0.0
        };
        hertz.push(if found > 0.0 {
            (found * shift).clamp(LOWEST, HIGHEST)
        } else {
            0.0
        });
    }

    let coarse = hertz.iter().map(|it| coarse_of(*it)).collect();
    Pitch { hertz, coarse }
}

/// RVC's mel-scaled 1–255 bucket. Zero stays zero: it means *nothing was voiced here*, which is
/// information the model uses, not a very low note.
fn coarse_of(hertz: f32) -> i64 {
    if hertz <= 0.0 {
        return 0;
    }
    let mel = |f: f32| 1127.0 * (1.0 + f / 700.0).ln();
    let low = mel(LOWEST);
    let high = mel(HIGHEST);
    let scaled = (mel(hertz) - low) * 254.0 / (high - low) + 1.0;
    scaled.round().clamp(1.0, 255.0) as i64
}

/// Below this the reading is called voiced. YIN's own paper suggests 0.1–0.15 for the search;
/// this second, looser gate is what decides *silence*, and it is deliberately generous — a
/// consonant read as unvoiced is a gap in the melody, which RVC handles, while noise read as a
/// note is a squeak, which it does not.
const VOICED: f32 = 0.45;

/// One frame of YIN.
fn yin(window: &[f32], rate: u32) -> f32 {
    let shortest = (rate as f32 / HIGHEST).floor().max(2.0) as usize;
    let longest = ((rate as f32 / LOWEST).ceil() as usize).min(window.len() / 2);
    if longest <= shortest {
        return 0.0;
    }

    // The squared difference, then the cumulative mean that makes the threshold meaningful.
    let mut difference = vec![0.0_f32; longest + 1];
    let half = window.len() / 2;
    for tau in 1..=longest {
        let mut sum = 0.0_f32;
        for j in 0..half {
            let d = window[j] - window[j + tau];
            sum += d * d;
        }
        difference[tau] = sum;
    }

    let mut normalised = vec![1.0_f32; longest + 1];
    let mut running = 0.0_f32;
    for tau in 1..=longest {
        running += difference[tau];
        normalised[tau] = if running > 0.0 {
            difference[tau] * tau as f32 / running
        } else {
            1.0
        };
    }

    // The *first* dip below the threshold, not the deepest — the deepest is usually an octave
    // down, and an octave error is the one pitch mistake a listener always hears.
    let mut best = 0_usize;
    for tau in shortest..longest {
        if normalised[tau] < 0.15 && normalised[tau + 1] >= normalised[tau] {
            best = tau;
            break;
        }
    }
    if best == 0 {
        let (tau, value) = (shortest..=longest)
            .map(|tau| (tau, normalised[tau]))
            .fold((0, f32::MAX), |a, b| if b.1 < a.1 { b } else { a });
        if value > VOICED {
            return 0.0;
        }
        best = tau;
    }
    if best == 0 {
        return 0.0;
    }

    // Parabolic interpolation between the three points around the dip. Without it the pitch can
    // only land on whole samples, which at 16 kHz is a quarter-tone of error up at 800 Hz.
    let refined = if best > 0 && best < longest {
        let (a, b, c) = (normalised[best - 1], normalised[best], normalised[best + 1]);
        let bottom = 2.0 * (2.0 * b - a - c);
        if bottom.abs() > 1e-9 {
            best as f32 + (c - a) / bottom
        } else {
            best as f32
        }
    } else {
        best as f32
    };

    if refined <= 0.0 {
        0.0
    } else {
        rate as f32 / refined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hertz: f32, rate: u32, seconds: f32) -> Sound {
        let count = (rate as f32 * seconds) as usize;
        let samples = (0..count)
            .map(|n| {
                let t = n as f32 / rate as f32;
                // Two harmonics, because a pure sine is the one waveform every estimator gets
                // right and voice is never one.
                0.6 * (std::f32::consts::TAU * hertz * t).sin()
                    + 0.3 * (std::f32::consts::TAU * 2.0 * hertz * t).sin()
            })
            .collect();
        Sound { samples, rate }
    }

    #[test]
    fn a_wav_survives_being_written_and_read() {
        let sound = tone(220.0, 16_000, 0.05);
        let read = read_wav(&write_wav(&sound)).expect("Epoch's own WAV should be readable");
        assert_eq!(read.rate, 16_000);
        assert_eq!(read.samples.len(), sound.samples.len());
        // 16-bit, so exactness is not the claim — that nothing was mangled is.
        for (a, b) in read.samples.iter().zip(&sound.samples) {
            assert!((a - b).abs() < 1e-3, "{a} against {b}");
        }
    }

    #[test]
    fn something_that_is_not_a_wav_is_refused_rather_than_decoded() {
        assert!(read_wav(b"not audio at all").is_err());
        // A compressed WAV is the interesting one: the header parses and the body is not PCM.
        let mut fake = write_wav(&tone(220.0, 16_000, 0.01));
        fake[20] = 2; // format 2, ADPCM
        let refused = read_wav(&fake).expect_err("a format Epoch cannot read must be refused");
        assert!(refused.contains("format 2"), "{refused}");
    }

    #[test]
    fn resampling_keeps_the_note_and_the_length() {
        let sound = tone(220.0, 22_050, 0.3);
        let down = resample(&sound, 16_000);
        assert_eq!(down.rate, 16_000);
        // Within a sample of the exact ratio.
        let expected = (sound.samples.len() as f64 * 16_000.0 / 22_050.0).round() as usize;
        assert!(down.samples.len().abs_diff(expected) <= 1);

        let heard = pitch(&down, 20, 0.0);
        let voiced: Vec<f32> = heard.hertz.iter().copied().filter(|it| *it > 0.0).collect();
        assert!(
            !voiced.is_empty(),
            "a resampled tone should still have a pitch"
        );
        let average = voiced.iter().sum::<f32>() / voiced.len() as f32;
        assert!((average - 220.0).abs() < 8.0, "heard {average} Hz, not 220");
    }

    #[test]
    fn a_known_note_is_heard_as_that_note() {
        for wanted in [110.0_f32, 220.0, 440.0] {
            let sound = tone(wanted, 16_000, 0.3);
            let heard = pitch(&sound, 20, 0.0);
            let voiced: Vec<f32> = heard.hertz.iter().copied().filter(|it| *it > 0.0).collect();
            assert!(voiced.len() > 10, "{wanted} Hz was mostly heard as silence");
            let average = voiced.iter().sum::<f32>() / voiced.len() as f32;
            // 2% — an octave error would be 100%, which is the mistake this is guarding.
            assert!(
                (average / wanted - 1.0).abs() < 0.02,
                "asked for {wanted} Hz and heard {average}"
            );
        }
    }

    #[test]
    fn silence_is_unvoiced_rather_than_a_very_low_note() {
        let sound = Sound {
            samples: vec![0.0; 16_000],
            rate: 16_000,
        };
        let heard = pitch(&sound, 50, 0.0);
        assert!(
            heard.hertz.iter().all(|it| *it == 0.0),
            "silence was given a pitch"
        );
        assert!(heard.coarse.iter().all(|it| *it == 0));
    }

    #[test]
    fn twelve_semitones_is_an_octave_in_both_tracks() {
        let sound = tone(220.0, 16_000, 0.3);
        let plain = pitch(&sound, 20, 0.0);
        let up = pitch(&sound, 20, 12.0);
        // The coarse track is derived *after* the shift. If it were derived before, the model
        // would be handed a note and a description of a different one -- which is exactly the
        // bug that puts a voice half an octave out and looks like a bad model.
        for (before, after) in plain.hertz.iter().zip(&up.hertz) {
            if *before > 0.0 && *after > 0.0 && *after < HIGHEST - 1.0 {
                assert!((after / before - 2.0).abs() < 0.05, "{before} -> {after}");
            }
        }
        for (frame, (h, c)) in up.hertz.iter().zip(&up.coarse).enumerate() {
            assert_eq!(
                *c,
                coarse_of(*h),
                "frame {frame} disagrees with its own hertz"
            );
        }
    }

    #[test]
    fn the_coarse_scale_stays_inside_the_model_s_range() {
        assert_eq!(coarse_of(0.0), 0);
        assert_eq!(coarse_of(LOWEST), 1);
        assert_eq!(coarse_of(HIGHEST), 255);
        // Anything outside is clamped rather than wrapped: an out-of-range bucket is an index
        // the model does not have.
        assert_eq!(coarse_of(10.0), 1);
        assert_eq!(coarse_of(4000.0), 255);
    }
}
