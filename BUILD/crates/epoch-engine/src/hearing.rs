//! The ear — the program on this machine that turns sound into text (Phase 15, step 5).
//!
//! ## The same shape as the mouth, and that is not an accident
//!
//! `speech.rs` says why a voice engine is neither a Runtime nor a Studio: no server, no port, an
//! executable handed one utterance. **whisper.cpp is the same**, and this reuses that module's
//! `tools()`, its download and its unpacking rather than growing a second copy of them.
//!
//! ## A command per press, and it is measured rather than assumed
//!
//! The plan said *start `whisper-server` headless*. Measured on this machine, that server is not
//! needed: `whisper-cli` **including the model load** answers in
//!
//! | clip | `base` | `small` |
//! |---|---|---|
//! | 6.1 s | 0.92 s | — |
//! | 5 s of Spanish | 1.03 s | 3.12 s |
//! | 19.7 s | 1.34 s | — |
//! | 36.7 s | 2.48 s | — |
//!
//! **The cost is per 30-second window, not per second of speech**: six seconds and twenty
//! seconds cost the same because both fit in one window. So anything said in one breath is
//! transcribed in about a second, from a cold start, with nothing running in between.
//!
//! A server would buy back the load time and cost a process, a port, a lifecycle and a console
//! window nobody asked for. **Nothing running is the simpler thing and the measurement says it
//! is fast enough** — the same arrangement Piper has, arrived at from the other direction.
//!
//! ## `small` is not simply better, and the roster is why
//!
//! Same 5-second Spanish clip: `base` heard *"soy **Image**"* and `small` heard *"**Cola** soy
//! **imagen**"* — three times the cost for a different error. What fixed it was telling whisper
//! **who lives in the World**, which Epoch already knows and was not saying:
//!
//! ```text
//! base  + roster → still "Image"
//! small + roster → "Hola, soy Mage. Trabajo en la torre y convierto objetivos…"
//! ```
//!
//! So the crew's names travel with every request. Not a nicety: it is what makes the larger
//! model worth its three seconds, and it is the same shape as every other defect in `CLAUDE.md`
//! — a fact Epoch held and did not pass on.
//!
//! **Stated as measured and not further:** those word errors are on *synthesised* speech at
//! `x_low` quality. The timings are the product's; the accuracy is a robot voice's, and a real
//! microphone has to be measured on its own.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// A program that turns sound into text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Ear {
    WhisperCpp,
}

/// Which model the ear listens with.
///
/// Two, because they are a real choice and not a quality ladder: `small` is three times the cost
/// and traded one error for another until the roster was passed. Both run **entirely on the
/// CPU** — the card never enters the question, which is what lets pressing the microphone start
/// something without competing with the brain or the easel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Hearing {
    Base,
    Small,
}

impl Hearing {
    pub const ALL: [Hearing; 2] = [Hearing::Base, Hearing::Small];

    pub const fn id(self) -> &'static str {
        match self {
            Hearing::Base => "base",
            Hearing::Small => "small",
        }
    }

    pub const fn file(self) -> &'static str {
        match self {
            Hearing::Base => "ggml-base.bin",
            Hearing::Small => "ggml-small.bin",
        }
    }

    /// What it weighs, as the publisher states it. Approximate on purpose: it is a number to
    /// warn with, and the download reports the real one as it arrives.
    pub const fn bytes(self) -> u64 {
        match self {
            Hearing::Base => 147_951_465,
            Hearing::Small => 487_601_967,
        }
    }

    /// What choosing this one actually costs, measured here rather than described.
    pub const fn about(self) -> &'static str {
        match self {
            Hearing::Base => {
                "About a second for anything said in one breath. Gets ordinary words right and \
                 stumbles on names."
            }
            Hearing::Small => {
                "About three seconds. Worth it with the crew's names passed along, which Epoch \
                 does — that is what it takes to hear `Mage` as a name rather than a word."
            }
        }
    }

    pub fn url(self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.file()
        )
    }
}

impl Ear {
    pub const ALL: [Ear; 1] = [Ear::WhisperCpp];

    pub const fn id(self) -> &'static str {
        match self {
            Ear::WhisperCpp => "whisper",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Ear::WhisperCpp => "whisper.cpp",
        }
    }

    pub const fn binary(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Ear::WhisperCpp, true) => "whisper-cli.exe",
            (Ear::WhisperCpp, false) => "whisper-cli",
        }
    }

    /// The release archive for this platform.
    ///
    /// **The CPU build, deliberately.** The CUDA one is 671 MB against 8.4, and the measurement
    /// says the card is not needed: a breath transcribes in about a second on the CPU. Paying
    /// eighty times the download for a resource the brain and the easel are competing over
    /// would be a worse machine, not a faster one.
    pub fn archive(self) -> Option<crate::speech::Archive> {
        // Measured 2026-09-04 against release `b4938` (2026-08-20).
        let base = "https://github.com/ggml-org/whisper.cpp/releases/download/b4938";
        match self {
            Ear::WhisperCpp => {
                let file = if cfg!(windows) {
                    Some(("whisper-bin-x64.zip", 8_361_840_u64))
                } else if cfg!(target_os = "macos") {
                    // Published as an xcframework rather than a runnable binary, so nothing is
                    // offered here yet: a download that cannot be run is worse than none.
                    None
                } else if cfg!(target_arch = "aarch64") {
                    Some(("whisper-bin-ubuntu-arm64.tar.gz", 4_600_000))
                } else {
                    Some(("whisper-bin-ubuntu-x64.tar.gz", 9_500_000))
                };
                file.map(|(name, bytes)| crate::speech::Archive {
                    url: format!("{base}/{name}"),
                    name: name.to_owned(),
                    bytes,
                })
            }
        }
    }

    pub const fn installing(self) -> &'static str {
        match self {
            Ear::WhisperCpp => {
                "Epoch downloads whisper.cpp's own release archive and a model, into Epoch's \
                 folder. Everything runs on the CPU: the graphics card is never asked for, so \
                 listening never competes with a character thinking or a picture drawing."
            }
        }
    }

    pub const fn licence(self) -> &'static str {
        match self {
            Ear::WhisperCpp => "MIT",
        }
    }
}

/// What the ear is on this machine, right now.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listening {
    pub id: &'static str,
    pub name: &'static str,
    /// The binary was found.
    pub installed: bool,
    pub at: Option<String>,
    /// Epoch fetched it, as opposed to finding one that was already here.
    pub ours: bool,
    /// Which models are on this machine. **Empty is the state that matters**: an ear with no
    /// model hears nothing, exactly as a mouth with no voice says nothing.
    pub models: Vec<Model>,
    pub archive: Option<crate::speech::Archive>,
    pub installing: &'static str,
    pub licence: &'static str,
}

/// One model, offered or present.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    pub id: &'static str,
    pub file: &'static str,
    pub bytes: u64,
    pub about: &'static str,
    pub here: bool,
}

/// Where the ear and its models live.
fn home() -> PathBuf {
    crate::speech::tools().join(Ear::WhisperCpp.id())
}

/// Find the ear, and say which models it can listen with.
///
/// Epoch's own copy first, then the PATH — the same order the mouth uses, and the same reason: a
/// program somebody installed deliberately outranks one an application fetched for them.
pub fn look_for(which: Ear) -> Listening {
    let mine = home();
    let found = crate::speech::find_program(&mine, which.binary())
        .map(|path| (path, true))
        .or_else(|| crate::paths::on_path(which.binary()).map(|path| (path, false)));

    Listening {
        id: which.id(),
        name: which.name(),
        installed: found.is_some(),
        at: found.as_ref().map(|(path, _)| path.display().to_string()),
        ours: found.as_ref().map(|(_, ours)| *ours).unwrap_or(false),
        models: Hearing::ALL
            .into_iter()
            .map(|model| Model {
                id: model.id(),
                file: model.file(),
                bytes: model.bytes(),
                about: model.about(),
                here: model_path(model).is_some(),
            })
            .collect(),
        archive: which.archive(),
        installing: which.installing(),
        licence: which.licence(),
    }
}

pub fn survey() -> Vec<Listening> {
    Ear::ALL.into_iter().map(look_for).collect()
}

/// Where a model is, if it is here.
pub fn model_path(which: Hearing) -> Option<PathBuf> {
    let file = home().join(which.file());
    file.is_file().then_some(file)
}

/// The best model on this machine, or `None` when there is none.
///
/// **Larger first**, because `small` with the roster is the one that hears a name — and a
/// machine that has both was told to download the bigger one on purpose.
pub fn best_model() -> Option<PathBuf> {
    model_path(Hearing::Small).or_else(|| model_path(Hearing::Base))
}

/// Fetch the ear's program.
pub fn install(which: Ear, watching: &dyn Fn(u64, Option<u64>)) -> Result<String, String> {
    let archive = which
        .archive()
        .ok_or("Epoch has no measured download of that for this platform")?;
    let into = home();
    std::fs::create_dir_all(&into).map_err(|why| format!("could not make {into:?}: {why}"))?;
    crate::speech::install_archive(&archive, &into, watching)?;

    match look_for(which).at {
        Some(at) => Ok(format!("{} is here: {at}", which.name())),
        None => Err(format!(
            "{} unpacked into {into:?}, and {} was not inside it.",
            which.name(),
            which.binary()
        )),
    }
}

/// Fetch one model.
pub fn install_model(
    which: Hearing,
    watching: &dyn Fn(u64, Option<u64>),
) -> Result<String, String> {
    let into = home();
    std::fs::create_dir_all(&into).map_err(|why| format!("could not make {into:?}: {why}"))?;
    crate::speech::install_file(&which.url(), which.file(), &into, watching)?;
    Ok(format!("{} is here.", which.file()))
}

/// Keep what was said, and drop what whisper said *about* the audio.
///
/// **Found by pressing the button in a silent room.** Four seconds of nothing came back as
/// `[Pause]`, and it landed in the composer as though somebody had typed it. whisper marks
/// non-speech in brackets or parentheses — `[Pause]`, `[BLANK_AUDIO]`, `(música)`, `[SOUND]` —
/// and those are a description of the recording, not words anybody spoke.
///
/// The same shape as the tool call that used to reach a model as the sentence
/// `[I called see_image(...)]`: **a description arriving where a message belongs**. And the
/// honest outcome of a silent recording is *nothing was heard*, which the caller already knows
/// how to say — not a box quietly filled with a stage direction.
///
/// Deliberately only whole markers. A sentence that genuinely contains a bracket keeps it: this
/// removes what stands alone, never what somebody wrapped around their own words.
fn words_only(said: &str) -> String {
    let mut kept = String::new();
    let mut inside: Option<char> = None;
    for letter in said.chars() {
        match (inside, letter) {
            (None, '[') => inside = Some(']'),
            (None, '(') => inside = Some(')'),
            (Some(closing), letter) if letter == closing => inside = None,
            (Some(_), _) => {}
            (None, letter) => kept.push(letter),
        }
    }
    kept.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// What somebody said, transcribed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Heard {
    pub text: String,
    /// How long it took, on this machine. A measurement and never a promise about another one.
    pub millis: u64,
    /// Which language whisper **decided** it was hearing, when nobody told it.
    ///
    /// ## Why a transcription has to say this
    ///
    /// `-l auto` on a short phrase is a guess, and it guesses wrong. The owner said *"hazme una
    /// tabla con esos datos"* three times and read back
    /// `Αυτοί, πρέπει να τα βλακουμε τα τάτια.` — **Greek**, confidently, with nothing on screen
    /// saying so. A wrong transcription looks like a bad ear; a wrong *language* looks like
    /// nothing at all.
    ///
    /// `None` when a language was named: there is no decision to report, and printing one would
    /// be an instrument that never moves.
    pub heard_as: Option<String>,
    /// How sure whisper was, `0.0`–`1.0`, from its own line. `None` alongside `heard_as`.
    ///
    /// Carried because *what* it decided and *how sure it was* are two different facts, and a
    /// surface that shows the first without the second cannot tell a confident answer from a
    /// coin toss — `es (p = 0.993)` and a 0.3 are not the same reading.
    pub sure: Option<f32>,
}

/// What whisper said it decided, out of its own stderr.
///
/// Its own function so a test can hold the shape: `whisper_full_with_state: auto-detected
/// language: es (p = 0.993047)`. Read rather than remembered — the line was copied off this
/// machine, not from documentation.
pub(crate) fn detected_in(said: &str) -> (Option<String>, Option<f32>) {
    let Some(at) = said.find("auto-detected language: ") else {
        return (None, None);
    };
    let rest = &said[at + "auto-detected language: ".len()..];
    let code = rest
        .split(|c: char| c.is_whitespace() || c == '(')
        .next()
        .unwrap_or_default()
        .trim();
    // **`auto` is whisper saying it did not decide, and that is not a language.** Measured in the
    // window: `Intl.DisplayNames.of("auto")` throws `RangeError: invalid_argument`, and so does
    // the empty string — which is how a display name for a non-decision took a whole
    // transcription down. Refused here as well as caught there: a value that is not a reading
    // should not leave the function that reads.
    if code.is_empty() || code.eq_ignore_ascii_case("auto") {
        return (None, None);
    }
    // **The probability is optional and the language is not.** An older build that prints the
    // code without one still says which language it chose, and that is the half that matters.
    let sure = rest
        .split_once("p = ")
        .and_then(|(_, after)| after.split(')').next())
        .and_then(|it| it.trim().parse::<f32>().ok());
    (Some(code.to_owned()), sure)
}

/// Which language to tell whisper it is hearing.
///
/// Its own function so it can be asserted on: the whole defect was an argument that was not
/// passed, and nothing in a `Command` builder chain is visible to a test.
pub(crate) fn spoken_language(asked: Option<&str>) -> &str {
    // **Never the empty string, and never absent.** `-l ""` is not `auto`, and leaving the flag
    // off is whisper's own `en` -- which is the reading Epoch has no measurement for.
    match asked.map(str::trim) {
        Some(said) if !said.is_empty() => said,
        _ => "auto",
    }
}

/// Listen to a `.wav` and answer with the words.
///
/// ## Whose names are passed, and why that is not a nicety
///
/// `--prompt` takes an initial prompt, and Epoch knows who lives in the World. Measured: without
/// it `small` heard *"Cola soy imagen"*; with it, *"Hola, soy Mage"*. **That is what makes the
/// larger model worth its three seconds**, and withholding a fact Epoch holds is the shape of
/// every defect in `CLAUDE.md`'s *What a turn owes the model*.
///
/// ## Sixteen kilohertz, and the caller does not have to know
///
/// whisper wants 16 kHz mono. A browser records at whatever its device runs at, so the caller
/// hands over what it has and this refuses clearly rather than transcribing noise.
pub fn transcribe(
    exe: &Path,
    model: &Path,
    wav: &Path,
    language: Option<&str>,
    who_lives_here: &[String],
) -> Result<Heard, String> {
    use epoch_models::quiet::Quiet;

    if !model.is_file() {
        return Err("no model is installed for listening yet".into());
    }
    let started = std::time::Instant::now();
    let mut running = std::process::Command::new(exe);
    running
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(wav)
        // No timestamps: what a composer wants is the sentence.
        .arg("-nt")
        /*
            **`-np` is gone, and it was hiding the one reading that matters.**
            *No prints* suppresses whisper's own `auto-detected language: es (p = 0.993047)` —
            the line that turns *"why did it write Greek"* into an answer. Measured both ways:
            with `-np` the stderr holds nothing; without it, stdout is **still only the
            sentence** (the noise is on stderr, which is where this reads it from anyway).
            So it cost nothing and was hiding something. Second time `-np` has been the reason a
            fact was not there to read.
        */
        // **Always said, and `auto` when nobody said it.**
        //
        // `whisper-cli --help` states its own default: `-l LANG [en]`. So a caller that says
        // nothing is not asking whisper to detect the language -- it is asking for English, and
        // Epoch has never measured what language anybody speaks.
        //
        // The owner met it as *"cuando hablo en spanish no sirve bien"*. Measured on his own
        // Spanish, through the crew prompt Epoch actually sends:
        //
        // ```text
        // (no -l)    Hello, I'm Mage, welcome to the World of Poets.
        // -l auto    Hola, soy Mage, bienvenido al mundo de Potso.
        // ```
        //
        // Not a mistranscription -- a **translation**, confidently, into a language he was not
        // speaking. And it is invisible without the prompt: the same clip with no `--prompt`
        // stayed in Spanish either way, so a measurement that skipped it would have found
        // nothing. Measuring the wire is not measuring the product.
        //
        // Every model Epoch installs is a multilingual one (`ggml-base.bin`, never
        // `ggml-base.en.bin`), so `auto` is always a question this model can answer.
        .arg("-l")
        .arg(spoken_language(language));
    if !who_lives_here.is_empty() {
        // The crew, as a sentence — which is what an initial prompt is: the words whisper should
        // expect to hear, not a list it looks things up in.
        running
            .arg("--prompt")
            .arg(format!("Aquí viven: {}.", who_lives_here.join(", ")));
    }
    let done = running
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        // No console window. A black rectangle flashing every time somebody speaks is the
        // immersion leak ComfyUI is still remembered for.
        .quiet()
        .output()
        .map_err(|why| format!("could not start {exe:?}: {why}"))?;

    let text = words_only(&String::from_utf8_lossy(&done.stdout));
    if text.is_empty() {
        // **The side effect, not the exit code.** whisper answers 0 having heard nothing at all,
        // and a composer filled with an empty string is a press that did nothing and said so
        // nowhere.
        // **The side effect, not the exit code.** whisper answers 0 having heard nothing at
        // all, and a composer filled with an empty string — or with `[Pause]` — is a press that
        // did nothing and said so nowhere.
        return Err("Nothing was heard.".to_owned());
    }
    // **Only when nobody named one.** A language that was chosen has no decision to report, and
    // an instrument that never moves is worse than none.
    let (heard_as, sure) = match language {
        Some(said) if !said.trim().is_empty() => (None, None),
        _ => detected_in(&String::from_utf8_lossy(&done.stderr)),
    };

    Ok(Heard {
        text,
        millis: started.elapsed().as_millis() as u64,
        heard_as,
        sure,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`EPOCH_TOOLS` is process-wide and these tests run in parallel.**
    ///
    /// Each passed alone and they failed together, which is the shape a shared environment
    /// variable always takes: one test set it while another was reading it. A lock rather than a
    /// rewrite, because pointing at a folder is exactly what the surface does and a test that
    /// avoided it would stop measuring the thing.
    static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// What this machine answers, printed rather than asserted.
    ///
    /// A diagnostic: `Nothing here can listen yet` is a sentence about `look_for`, and the only
    /// way to tell a wrong gauge from a missing file is to ask the same function the product asks.
    #[test]
    #[ignore = "prints what this machine holds"]
    fn what_this_machine_can_hear_with() {
        eprintln!("tools()      = {:?}", crate::speech::tools());
        eprintln!("home()       = {:?}", home());
        eprintln!("EPOCH_TOOLS  = {:?}", std::env::var("EPOCH_TOOLS"));
        eprintln!("look_for     = {:#?}", look_for(Ear::WhisperCpp));
        eprintln!("best_model   = {:?}", best_model());
    }

    #[test]
    fn a_transcription_says_which_language_it_decided_on() {
        // The line, copied off this machine rather than remembered.
        let (code, sure) = detected_in(
            "whisper_full_with_state: auto-detected language: es (p = 0.993047)
",
        );
        assert_eq!(code.as_deref(), Some("es"));
        assert!(
            sure.is_some_and(|p| (p - 0.993_047).abs() < 1e-6),
            "{sure:?}"
        );

        // The owner's actual failure: three tries, and the first came back Greek.
        assert_eq!(
            detected_in("auto-detected language: el (p = 0.41)")
                .0
                .as_deref(),
            Some("el")
        );

        // **The language is the half that matters and the probability is optional.** A build
        // that prints one without the other still says what it chose.
        assert_eq!(
            detected_in(
                "auto-detected language: fr
"
            )
            .0
            .as_deref(),
            Some("fr")
        );
        assert_eq!(detected_in("nothing of the sort"), (None, None));

        // **Not a language, so not a reading.** Both of these throw in `Intl.DisplayNames`,
        // measured in this WebView2 — and the throw took down the transcription that carried
        // them.
        assert_eq!(
            detected_in("auto-detected language: auto (p = 0.10)"),
            (None, None)
        );
        assert_eq!(
            detected_in("auto-detected language:  (p = 0.10)"),
            (None, None)
        );
    }

    #[test]
    fn a_language_is_always_named_and_nobody_is_assumed_to_speak_english() {
        // `whisper-cli --help` says `-l LANG [en]`. Leaving the flag off is therefore not
        // "let it detect" -- it is a claim about the user that Epoch never measured, and on the
        // owner's own Spanish it produced *"Hello, I'm Mage, welcome to the World of Poets."*
        assert_eq!(spoken_language(None), "auto");
        assert_eq!(spoken_language(Some("")), "auto");
        assert_eq!(spoken_language(Some("   ")), "auto");
        // What somebody does say is passed through untouched.
        assert_eq!(spoken_language(Some("es")), "es");
        assert_eq!(spoken_language(Some(" es ")), "es");
    }

    #[test]
    fn a_cold_row_says_what_it_would_fetch_and_what_it_costs() {
        let _held = ONE_AT_A_TIME.lock().unwrap_or_else(|it| it.into_inner());
        let dir = std::env::temp_dir().join(format!("epoch-ear-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("EPOCH_TOOLS", &dir);
        let ear = look_for(Ear::WhisperCpp);
        std::env::remove_var("EPOCH_TOOLS");

        assert_eq!(ear.id, "whisper");
        assert!(!ear.installed);
        assert_eq!(ear.licence, "MIT");
        // Both models are offered, neither is here, and each says what it costs — a choice
        // between two real trade-offs rather than a quality ladder.
        assert_eq!(ear.models.len(), 2);
        assert!(ear.models.iter().all(|m| !m.here));
        assert!(ear.models.iter().all(|m| m.bytes > 100_000_000));
        assert!(ear.models.iter().all(|m| !m.about.is_empty()));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_larger_model_is_preferred_when_both_are_here() {
        let _held = ONE_AT_A_TIME.lock().unwrap_or_else(|it| it.into_inner());
        let dir = std::env::temp_dir().join(format!("epoch-ear2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let inside = dir.join("whisper");
        std::fs::create_dir_all(&inside).unwrap();
        std::env::set_var("EPOCH_TOOLS", &dir);

        std::fs::write(inside.join(Hearing::Base.file()), b"x").unwrap();
        assert_eq!(best_model(), Some(inside.join(Hearing::Base.file())));

        // `small` with the roster is the one that hears a name, and a machine holding both was
        // told to fetch the larger one deliberately.
        std::fs::write(inside.join(Hearing::Small.file()), b"x").unwrap();
        assert_eq!(best_model(), Some(inside.join(Hearing::Small.file())));

        std::env::remove_var("EPOCH_TOOLS");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// What whisper says *about* the audio is not what somebody said.
    ///
    /// Found by pressing the microphone in a silent room: `[Pause]` arrived in the composer as
    /// though it had been typed.
    #[test]
    fn a_stage_direction_is_not_a_message() {
        assert_eq!(words_only("[Pause]"), "");
        assert_eq!(words_only(" [BLANK_AUDIO] "), "");
        assert_eq!(words_only("(música de fondo)"), "");
        assert_eq!(words_only("[SOUND] Hola, soy Mage."), "Hola, soy Mage.");
        assert_eq!(words_only("Hola [Pause] soy Mage"), "Hola soy Mage");

        // And what somebody actually said survives untouched, including the punctuation that
        // makes it a sentence.
        assert_eq!(words_only("  Hola, ¿cómo estás?  "), "Hola, ¿cómo estás?");
        // A stray opening bracket takes the rest, which is the honest reading of an unbalanced
        // marker and is what whisper produces when it is cut off.
        assert_eq!(words_only("Hola [Pau"), "Hola");
    }

    /// The whole ear, through the code the surface calls: fetch, find, and hear real words.
    #[test]
    #[ignore = "network"]
    fn epoch_can_fetch_an_ear_and_hear_something_with_it() {
        let _held = ONE_AT_A_TIME.lock().unwrap_or_else(|it| it.into_inner());
        let dir = std::env::temp_dir().join(format!("epoch-ear-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("EPOCH_TOOLS", &dir);

        install(Ear::WhisperCpp, &|_, _| {}).expect("whisper.cpp installs");
        install_model(Hearing::Base, &|_, _| {}).expect("a model installs");
        let ear = look_for(Ear::WhisperCpp);
        let model = best_model().expect("a model is here");
        std::env::remove_var("EPOCH_TOOLS");

        assert!(ear.installed && ear.ours, "{ear:?}");

        // A real recording, made by the mouth: the two halves of this phase, meeting.
        let voice = epoch_models::generative::Library::here()
            .shelf(epoch_models::generative::Shelf::Voices);
        let onnx = std::fs::read_dir(&voice).ok().and_then(|entries| {
            entries
                .flatten()
                .map(|it| it.path())
                .find(|p| p.extension().and_then(|e| e.to_str()) == Some("onnx"))
        });
        let (Some(onnx), Some(piper)) = (
            onnx,
            crate::speech::look_for(crate::speech::Voicebox::Piper).at,
        ) else {
            eprintln!("no voice or no Piper — install both first");
            std::fs::remove_dir_all(&dir).ok();
            return;
        };
        let said = dir.join("said.wav");
        crate::speech::speak(
            Path::new(&piper),
            &onnx,
            "Hola, soy Mage. Esto es una prueba del oido.",
            &said,
        )
        .expect("it speaks");

        let heard = transcribe(
            Path::new(&ear.at.unwrap()),
            &model,
            &said,
            // **`None`, deliberately.** This asked for `Some("es")` and therefore measured a
            // path the product does not take: the composer passes nothing, and nothing meant
            // English. A harness that supplies the missing argument cannot find a missing
            // argument.
            None,
            &["Mage".to_owned(), "Paladin".to_owned()],
        )
        .expect("it hears");
        eprintln!("heard in {}ms: {}", heard.millis, heard.text);
        assert!(heard.text.to_lowercase().contains("hola"), "{}", heard.text);

        std::fs::remove_dir_all(&dir).ok();
    }
}
