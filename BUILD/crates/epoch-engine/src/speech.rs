//! The mouth — the program on this machine that turns text into sound (Phase 15).
//!
//! ## Why this is neither a Runtime nor a Studio
//!
//! `runtimes.rs` answers *what can think here*, and every entry there becomes a Provider a
//! character can be assigned to. `studio.rs` answers *what can draw here*. Both are servers with
//! a port that answers or does not.
//!
//! Piper is neither. It **has no server and no port**: it is an executable handed a sentence and
//! a voice, which writes a `.wav` and exits. A row with a `SERVING` lamp would be an instrument
//! that can never move, which is worse than no instrument.
//!
//! What it shares with both is the only thing a deck row is about: **installed, where, and how
//! to fix it if not**. Those three are drawn alike because they answer alike — and, as in
//! `studio.rs`, the struct is deliberately *not* shared, because sharing the struct is the first
//! step to sharing the list, and a text-to-speech binary in the Brain dropdown is a brain nobody
//! can hold a conversation with.
//!
//! ## Why it lives in the Engine and not in `epoch-models`
//!
//! The standing question, asked before it was built: **does EpochServices need this too?**
//! (ADR-0029). It does not — a lent machine answers turns, and nothing there speaks. Runtimes and
//! studios live in `epoch-models` because *both* surfaces draw them. This one has one reader.
//!
//! ## Epoch has to fetch this one itself, and that is measured
//!
//! Every other program on the deck has a package manager command Epoch prints and runs in the
//! user's own terminal. Piper has none, on either platform — asked on 2026-09-04:
//!
//! | asked | answered |
//! |---|---|
//! | `winget search piper` | `npiperelay`, `PhotoPiper`. Nothing. |
//! | `formulae.brew.sh` for `piper`, `piper-tts`, `rhasspy-piper` | **404** as formula and as cask |
//!
//! So the row either fetches the release archive or it is a link and an instruction, which is a
//! row that does nothing. It fetches — 22.5 MB, into **Epoch's own folder**, which is exactly
//! what ADR-0032 permits: Epoch never writes into somebody else's tree, and here there is no
//! other tree to write into.
//!
//! ## Which Piper, and the cost of that choice
//!
//! `rhasspy/piper` `2023.11.14-2`, the standalone build. **Frozen since 2023** and chosen anyway,
//! because the alternative — `OHF-Voice/piper1-gpl`, actively maintained — is a Python wheel:
//! `.py` files around one `.pyd`, with inference running in Python on onnxruntime.
//!
//! What makes frozen acceptable is measured rather than hoped: `es_ES-davefx-medium`, published
//! long after that release, runs on it unchanged. **The engine is stable and the voices are what
//! evolve.** If the standalone line ever fails, the fallback is the wheel, and it brings Python.
//!
//! ## Measured on this machine, 2026-09-04
//!
//! | | |
//! |---|---|
//! | 5 seconds of Spanish, `x_low` | **377 ms** warm, 1 204 ms cold |
//! | the same, `medium` | **406 ms** |
//! | 44.7 seconds of Mage's real answer | **2.1 s** — about 21× faster than real time |
//!
//! That last number is what decides step 4 of this phase: speaking sentence by sentence as tokens
//! arrive is comfortably fast enough, and nothing has to wait for a whole answer.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// A program that turns text into sound.
///
/// One variant today and an enum anyway, for the reason `Studio` is one: the second family is
/// already named (Kokoro, ADR-0030's own note), and what separates them is a binary and an
/// argument list — exactly the shape an enum holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Voicebox {
    Piper,
}

impl Voicebox {
    pub const ALL: [Voicebox; 1] = [Voicebox::Piper];

    pub const fn id(self) -> &'static str {
        match self {
            Voicebox::Piper => "piper",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Voicebox::Piper => "Piper",
        }
    }

    /// The executable's name on this platform.
    const fn binary(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Voicebox::Piper, true) => "piper.exe",
            (Voicebox::Piper, false) => "piper",
        }
    }

    /// The release archive for this platform, and what it weighs.
    ///
    /// **`None` where nothing has been measured**, which is the honest answer for a platform
    /// nobody has run this on: the row then says the archive exists and Epoch cannot fetch it
    /// here, rather than offering a download that fails.
    pub fn archive(self) -> Option<Archive> {
        let base = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2";
        match self {
            Voicebox::Piper => {
                let file = if cfg!(windows) {
                    Some(("piper_windows_amd64.zip", 22_500_000_u64))
                } else if cfg!(target_os = "macos") {
                    Some((
                        if cfg!(target_arch = "aarch64") {
                            "piper_macos_aarch64.tar.gz"
                        } else {
                            "piper_macos_x64.tar.gz"
                        },
                        19_100_000,
                    ))
                } else if cfg!(target_arch = "aarch64") {
                    Some(("piper_linux_aarch64.tar.gz", 26_000_000))
                } else {
                    Some(("piper_linux_x86_64.tar.gz", 26_500_000))
                };
                file.map(|(name, bytes)| Archive {
                    url: format!("{base}/{name}"),
                    name: name.to_owned(),
                    bytes,
                })
            }
        }
    }

    /// What somebody should know before pressing INSTALL.
    pub const fn installing(self) -> &'static str {
        match self {
            Voicebox::Piper => {
                "Epoch downloads Piper's own release archive and unpacks it into Epoch's folder. \
                 Nothing is installed system-wide and nothing else on this machine is touched; \
                 deleting the folder undoes it completely."
            }
        }
    }

    /// Its licence, said on the row rather than discovered later.
    ///
    /// **GPL-3, and it matters only at distribution.** Epoch downloads it the way the user would
    /// have; it does not bundle it. The voices are MIT and are a separate question (ADR-0016
    /// requires the licence to travel with the asset, and here there are two).
    pub const fn licence(self) -> &'static str {
        match self {
            Voicebox::Piper => "GPL-3.0 · the voices it reads are separately licensed",
        }
    }
}

/// One release archive Epoch would fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Archive {
    pub url: String,
    pub name: String,
    /// What the release publishes. Approximate on purpose — it is a size to warn with, and the
    /// download reports the real one as it arrives.
    pub bytes: u64,
}

/// Where Epoch keeps the programs it fetched itself.
///
/// Beside the Generative Library rather than inside it: a shelf holds **assets**, and a binary
/// is not one. Filing `piper.exe` under `voices` would put a program in a list of things a
/// character can be given.
pub fn tools() -> PathBuf {
    if let Some(said) = std::env::var_os("EPOCH_TOOLS") {
        return PathBuf::from(said);
    }
    epoch_models::generative::application_data()
        .join("Epoch")
        .join("tools")
}

/// What one voice engine is on this machine, right now.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Mouth {
    pub id: &'static str,
    pub name: &'static str,
    /// The binary was found. `false` is *not here*, and the fix is the download.
    pub installed: bool,
    /// Exactly which file, so a person can see which of two installs answered.
    pub at: Option<String>,
    /// Whether Epoch fetched this one, as opposed to finding one the user already had.
    ///
    /// Worth distinguishing: Epoch may delete what it fetched and may not delete what it found.
    pub ours: bool,
    pub archive: Option<Archive>,
    pub installing: &'static str,
    pub licence: &'static str,
}

/// Find the voice engine, wherever it is.
///
/// **Epoch's own folder first, then the PATH.** Somebody who already had Piper keeps using
/// theirs — the same order the runtimes deck uses, and the same reason: a program the user
/// installed deliberately outranks one an application fetched for them.
pub fn look_for(which: Voicebox) -> Mouth {
    let mine = tools().join(which.id());
    let found = find_program(&mine, which.binary())
        .map(|path| (path, true))
        .or_else(|| epoch_engine_path(which.binary()).map(|path| (path, false)));

    Mouth {
        id: which.id(),
        name: which.name(),
        installed: found.is_some(),
        at: found.as_ref().map(|(path, _)| path.display().to_string()),
        ours: found.as_ref().map(|(_, ours)| *ours).unwrap_or(false),
        archive: which.archive(),
        installing: which.installing(),
        licence: which.licence(),
    }
}

/// Every voice engine, in the order the deck draws them.
pub fn survey() -> Vec<Mouth> {
    Voicebox::ALL.into_iter().map(look_for).collect()
}

/// The PATH, through the Engine's own helper so there is one answer to *is this on the PATH*.
fn epoch_engine_path(name: &str) -> Option<PathBuf> {
    crate::paths::on_path(name)
}

/// Look for a binary in a folder, one level deep.
///
/// One level because that is what the archives actually contain — measured: the Windows zip
/// unpacks to `piper/piper.exe` beside its DLLs and `espeak-ng-data`. A deeper walk would be
/// the directory scan ADR-0032 refuses, running forwards.
pub(crate) fn find_program(root: &Path, binary: &str) -> Option<PathBuf> {
    let here = root.join(binary);
    if here.is_file() {
        return Some(here);
    }
    let entries = std::fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            let inside = entry.path().join(binary);
            if inside.is_file() {
                return Some(inside);
            }
        }
    }
    None
}

/// Fetch the voice engine and unpack it into Epoch's own folder.
///
/// **Nothing is installed system-wide and no other program's tree is touched.** ADR-0032's rule
/// holds unchanged: the only reason Epoch writes at all here is that there is no package manager
/// to ask, measured on both platforms.
pub fn install(which: Voicebox, watching: &dyn Fn(u64, Option<u64>)) -> Result<String, String> {
    let archive = which
        .archive()
        .ok_or("Epoch has no measured download of that for this platform")?;
    let into = tools().join(which.id());
    std::fs::create_dir_all(&into).map_err(|why| format!("could not make {into:?}: {why}"))?;

    install_archive(&archive, &into, watching)?;

    let mouth = look_for(which);
    match mouth.at {
        Some(at) => Ok(format!("{} is here: {at}", which.name())),
        // Unpacked, and the binary is not where it was expected. Said plainly rather than
        // reported as success: the next thing to fail would be somebody asking for speech.
        None => Err(format!(
            "{} unpacked into {into:?}, and {} was not inside it.",
            which.name(),
            which.binary()
        )),
    }
}

/// Fetch an archive and unpack it, leaving nothing behind.
///
/// Shared with the ear (`hearing.rs`) rather than copied: whisper.cpp and Piper are the same
/// shape — a release archive, no package to ask a package manager for, and nowhere else to put
/// it. Two copies of this would be two places to fix the day a platform's archive changes.
pub(crate) fn install_archive(
    archive: &Archive,
    into: &Path,
    watching: &dyn Fn(u64, Option<u64>),
) -> Result<(), String> {
    let landed = fetch(&archive.url, &archive.name, into, watching)?;
    unpack(&landed, into)?;
    // The archive has done its job. Keeping it would double what the folder weighs for no
    // question it can answer afterwards.
    let _ = std::fs::remove_file(&landed);
    Ok(())
}

/// Fetch one file and keep it under its own name — a model, rather than an archive.
pub(crate) fn install_file(
    url: &str,
    name: &str,
    into: &Path,
    watching: &dyn Fn(u64, Option<u64>),
) -> Result<PathBuf, String> {
    fetch(url, name, into, watching)
}

/// Download to a file, reporting progress the way the asset shelf does.
pub(crate) fn fetch(
    url: &str,
    name: &str,
    into: &Path,
    watching: &dyn Fn(u64, Option<u64>),
) -> Result<PathBuf, String> {
    let answer = ureq::get(url)
        .timeout(std::time::Duration::from_secs(60 * 30))
        .call()
        .map_err(|why| format!("{name} could not be downloaded: {why}"))?;
    let total = answer
        .header("Content-Length")
        .and_then(|it| it.parse::<u64>().ok());

    let path = into.join(name);
    let mut file =
        std::fs::File::create(&path).map_err(|why| format!("could not write {path:?}: {why}"))?;
    let mut body = answer.into_reader();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut done = 0_u64;
    loop {
        let read = std::io::Read::read(&mut body, &mut buffer)
            .map_err(|why| format!("the download stopped: {why}"))?;
        if read == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buffer[..read])
            .map_err(|why| format!("could not write {path:?}: {why}"))?;
        done += read as u64;
        watching(done, total);
    }
    Ok(path)
}

/// Unpack a `.zip` or a `.tar.gz`, whichever this platform's release is.
///
/// **`tar` rather than two more crates on Unix.** It is in POSIX and ships on macOS and Linux,
/// and Epoch already starts other programs; adding `flate2` and `tar` to link one archive
/// format would be complexity that has not earned itself.
pub(crate) fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    if archive.extension().and_then(|it| it.to_str()) == Some("zip") {
        let file = std::fs::File::open(archive)
            .map_err(|why| format!("could not open {archive:?}: {why}"))?;
        let mut zip = zip::ZipArchive::new(file)
            .map_err(|why| format!("{archive:?} is not a readable archive: {why}"))?;
        zip.extract(into)
            .map_err(|why| format!("could not unpack {archive:?}: {why}"))?;
        return Ok(());
    }

    let done = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .output()
        .map_err(|why| format!("this machine has no `tar` to unpack it with: {why}"))?;
    if done.status.success() {
        Ok(())
    } else {
        Err(format!(
            "tar could not unpack it: {}",
            String::from_utf8_lossy(&done.stderr).trim()
        ))
    }
}

/// Say something, and write it to a `.wav`.
///
/// The text goes in on **stdin** rather than as an argument, which is how Piper is driven and
/// also what keeps a sentence with quotes in it from becoming a shell problem.
///
/// Returns how long the engine took. The caller decides what that means: for a person waiting on
/// one sentence it is a latency, and for the deck's own check it is the measurement.
pub fn speak(
    exe: &Path,
    voice: &Path,
    text: &str,
    into: &Path,
) -> Result<std::time::Duration, String> {
    if !voice.is_file() {
        return Err(format!("that voice is not on the shelf: {voice:?}"));
    }
    // A voice is two files and Piper opens the second one itself. Saying so here means the
    // failure is named where somebody can act on it, rather than arriving as Piper's own error.
    let mut sidecar = voice.as_os_str().to_owned();
    sidecar.push(".json");
    if !Path::new(&sidecar).is_file() {
        return Err(format!(
            "{voice:?} has no `.onnx.json` beside it, and Piper reads that for its phonemes."
        ));
    }

    use epoch_models::quiet::Quiet;

    let started = std::time::Instant::now();
    // **No console window.** A voice that flashes a black rectangle every sentence is the
    // immersion leak ComfyUI is still remembered for, and `quiet` is the machinery that already
    // exists for it.
    let mut child = std::process::Command::new(exe)
        .arg("-m")
        .arg(voice)
        .arg("-f")
        .arg(into)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .quiet()
        .spawn()
        .map_err(|why| format!("could not start {exe:?}: {why}"))?;
    {
        let mut stdin = child.stdin.take().ok_or("could not write to it")?;
        std::io::Write::write_all(&mut stdin, text.as_bytes())
            .map_err(|why| format!("could not hand it the words: {why}"))?;
    }
    let done = child
        .wait_with_output()
        .map_err(|why| format!("it did not finish: {why}"))?;

    if !into.is_file() || std::fs::metadata(into).map(|it| it.len()).unwrap_or(0) <= 44 {
        // **The side effect, not the exit code.** A success code is a claim about the process;
        // an empty `.wav` is 44 bytes of header and no sound, and this codebase has been caught
        // by that shape before.
        return Err(format!(
            "it produced no sound: {}",
            String::from_utf8_lossy(&done.stderr).trim()
        ));
    }
    Ok(started.elapsed())
}

/// One voice installed on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Voice {
    /// What a character's `speaks_with` holds — `es_ES-davefx-medium`.
    pub name: String,
    /// What it is, read from the sidecar: *Español (Spain) · medium · 22050 Hz*. Empty where the
    /// sidecar did not say, which reads as unmeasured rather than as a blank field.
    pub what: String,
    /// Which language, for grouping a list of 175 into something a person can scan.
    pub language: Option<String>,
}

/// Every voice on the shelf, in the order a list should show them.
///
/// ## The name is the file's stem, and that is a fact about Piper rather than a rule
///
/// A character holds a **voice name** and never a path (ADR-0016). For Piper those happen to
/// coincide: it publishes one file per voice and names the file after the voice. The day an
/// engine arrives that holds fifty-four voices in one file — Kokoro does — this function keeps
/// its signature and stops being a directory listing, which is exactly why the *name* is what
/// travels in the character's file.
pub fn voices_here() -> Vec<Voice> {
    let shelf =
        epoch_models::generative::Library::here().shelf(epoch_models::generative::Shelf::Voices);
    let Ok(entries) = std::fs::read_dir(&shelf) else {
        return Vec::new();
    };

    let mut found: Vec<Voice> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|it| it.to_str()) == Some("onnx"))
        // **A Piper voice is two files**, and this is what says so. Anything else that is an
        // `.onnx` -- an RVC timbre dropped in here by hand, say -- would otherwise be listed as
        // a voice somebody could speak with, and Piper fails on it at the moment they talk.
        // Timbres have their own shelf; this is the half of the answer that does not depend on
        // everybody putting files in the right folder.
        .filter(|path| {
            let mut sidecar = path.as_os_str().to_owned();
            sidecar.push(".json");
            Path::new(&sidecar).is_file()
        })
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let spoken = epoch_models::voices::Spoken::beside(&path);
            Some(Voice {
                name,
                what: spoken.as_ref().map(|it| it.plainly()).unwrap_or_default(),
                language: spoken.and_then(|it| it.english.or(it.native)),
            })
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Where a voice's model is, by the name a character holds.
///
/// `None` is **not installed here**, which is a state the surface must render rather than treat
/// as an error: a character keeps a preference the machine cannot honour yet, so that installing
/// the voice makes them sound right again instead of making the user choose twice.
pub fn find_voice(name: &str) -> Option<PathBuf> {
    // Matched exactly against the stem, never by resemblance. A near-miss is how somebody's
    // Argentinian voice quietly becomes a Spanish one — the same refusal the Style vocabulary
    // makes (ADR-0030's second amendment).
    voices_here()
        .into_iter()
        .find(|voice| voice.name == name)
        .map(|voice| {
            epoch_models::generative::Library::here()
                .shelf(epoch_models::generative::Shelf::Voices)
                .join(format!("{}.onnx", voice.name))
        })
}

/// One converted RVC voice on the shelf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Timbre {
    /// What a character's `sounds_like.voice` holds.
    pub name: String,
    /// What it is, from the sidecar the conversion wrote: *v2 · 40 kHz · 109 speakers*. Empty
    /// where it could not be read, which reads as unmeasured rather than as a blank field.
    pub what: String,
    /// How many voices are inside it, so a surface knows whether to offer a choice at all.
    pub speakers: i64,
}

/// Every converted RVC voice, in the order a list should show them.
///
/// **Derived from what is beside the file, not from its name.** A timbre is an `.onnx` with a
/// `<name>.json` next to it; a Piper voice is an `.onnx` with a `<name>.onnx.json`. Reading the
/// sidecar is also what makes an unconverted `.pth` invisible here, which is correct: it is on
/// the shelf and it cannot be spoken yet.
pub fn timbres_here() -> Vec<Timbre> {
    let shelf =
        epoch_models::generative::Library::here().shelf(epoch_models::generative::Shelf::Timbres);
    let Ok(entries) = std::fs::read_dir(&shelf) else {
        return Vec::new();
    };

    let mut found: Vec<Timbre> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|it| it.to_str()) == Some("onnx"))
        .filter_map(|path| {
            let name = path.file_stem()?.to_string_lossy().into_owned();
            let made = crate::rvc::timbre::made(&path).ok()?;
            Some(Timbre {
                what: format!(
                    "{} kHz · {} speaker{}",
                    made.rate / 1000,
                    made.speakers,
                    if made.speakers == 1 { "" } else { "s" }
                ),
                speakers: made.speakers,
                name,
            })
        })
        .collect();
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Where a converted timbre is, by the name a character holds.
///
/// `None` is **not converted here**. Same rule as `find_voice`: exact against the stem, never by
/// resemblance, and a preference the machine cannot honour yet is kept rather than cleared.
pub fn find_timbre(name: &str) -> Option<PathBuf> {
    let shelf =
        epoch_models::generative::Library::here().shelf(epoch_models::generative::Shelf::Timbres);
    timbres_here()
        .into_iter()
        .find(|it| it.name == name)
        .map(|it| shelf.join(format!("{}.onnx", it.name)))
}

/// Say something, and colour it into somebody else's voice if the character has one.
///
/// **One function, because a caller must not be able to forget the second half.** The timbre is
/// part of who a character is (ADR-0026), so a path that spoke without applying it would be a
/// path that answers as the wrong person -- and it would sound perfectly fine, which is what
/// makes it worth removing rather than documenting.
///
/// A timbre that is named and not installed **speaks anyway**, in the Piper voice, and says why.
/// Refusing would silence somebody over an optional last step.
pub fn speak_as(
    exe: &Path,
    voice: &Path,
    timbre: Option<(&str, f32, i64)>,
    text: &str,
    into: &Path,
) -> Result<(std::time::Duration, Option<String>), String> {
    let took = speak(exe, voice, text, into)?;
    let Some((name, semitones, speaker)) = timbre else {
        return Ok((took, None));
    };
    let Some(model) = find_timbre(name) else {
        return Ok((
            took,
            Some(format!(
                "'{name}' is not converted on this machine, so this was said in the plain voice."
            )),
        ));
    };

    let started = std::time::Instant::now();
    let wav = std::fs::read(into)
        .map_err(|why| format!("it spoke and the sound could not be read back: {why}"))?;
    let heard = crate::rvc::signal::read_wav(&wav)?;
    match crate::rvc::timbre::recolour_wav(&model, &wav, semitones, speaker, heard.rate) {
        Ok(coloured) => {
            std::fs::write(into, coloured)
                .map_err(|why| format!("the coloured sound could not be written: {why}"))?;
            Ok((took + started.elapsed(), None))
        }
        // The answer is already on screen and the voice is the optional part, so this is
        // reported and not raised.
        Err(why) => Ok((took, Some(why))),
    }
}

/// Where spoken lines are kept, so the window can ask for one by name.
///
/// **Its own folder, deliberately not `media/`.** That folder is the Chronicle's record — what a
/// capability made and what a person shared — and a sentence read out loud is neither. Filing
/// speech there would put a `.wav` beside every picture in a conversation and make evidence of
/// something that is only a rendering of text already on screen.
pub fn spoken_dir(vault: &Path) -> PathBuf {
    vault.join("spoken")
}

/// Keep one spoken line, named from its own bytes, and answer with the name.
///
/// ## Why a name and not the bytes
///
/// The first version handed the window a `data:` URI, and **it never made a sound**: this build's
/// `media-src` is `epoch: http://epoch.localhost`, so the browser refused every one of them —
/// silently, because a blocked source and a malformed file raise the same `NotSupportedError`.
///
/// It was reported as working because the measurement was a spy on `new Audio(...)`, which
/// proves the element was *created*. **Creating a player is not playing**, and the governor that
/// decides is `media-src`, which nothing in that measurement touched.
///
/// So this takes the route pictures already take (ADR-0024 §2b): the page is handed a **name**,
/// the Engine decides what it resolves to, and no path and no `fs` capability go anywhere near
/// the webview. Named by content, so the same sentence twice is one file and the browser can
/// cache it forever.
pub fn keep_spoken(vault: &Path, bytes: &[u8]) -> std::io::Result<String> {
    let dir = spoken_dir(vault);
    std::fs::create_dir_all(&dir)?;
    // FNV-1a, the same naming `import::keep_bytes` uses — one way of naming bytes in this
    // codebase, so nothing has to learn a second.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let name = format!("{hash:016x}.wav");
    let file = dir.join(&name);
    if !file.is_file() {
        std::fs::write(&file, bytes)?;
    }
    Ok(name)
}

/// A `.wav` as the window can play it.
///
/// **A `data:` URI and never a path** (ADR-0024): the presentation layer is handed bytes, and it
/// has no filesystem to hand a path to. This is the same route a World's own sounds already take.
///
/// Its own function rather than a `format!` at the call site because the Engine is where the
/// bytes are, and a surface that encoded its own would need `base64` and a reason to have it.
pub fn sound_uri(wav: &[u8]) -> String {
    use base64::Engine as _;
    format!(
        "data:audio/wav;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(wav)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of its own, so a test never looks at somebody's real install.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("epoch-speech-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    #[test]
    fn a_binary_is_found_at_the_root_and_one_level_down() {
        let dir = scratch("depth");
        assert_eq!(
            find_program(&dir, "piper.exe"),
            None,
            "an empty folder holds none"
        );

        let nested = dir.join("piper");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("piper.exe"), b"").unwrap();
        assert_eq!(
            find_program(&dir, "piper.exe"),
            Some(nested.join("piper.exe")),
            "the archive unpacks one level down, which is where it must be found"
        );

        // And two levels down is deliberately not searched: a walk that keeps going is the
        // directory scan this codebase refuses.
        let deeper = dir.join("a").join("b");
        std::fs::create_dir_all(&deeper).unwrap();
        std::fs::write(deeper.join("kokoro.exe"), b"").unwrap();
        assert_eq!(find_program(&dir, "kokoro.exe"), None);

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_row_says_what_it_is_before_anything_is_installed() {
        let dir = scratch("cold");
        std::env::set_var("EPOCH_TOOLS", &dir);
        let mouth = look_for(Voicebox::Piper);
        std::env::remove_var("EPOCH_TOOLS");

        assert_eq!(mouth.id, "piper");
        // Cold, the row keeps its frame: it still says what it would fetch, what that weighs,
        // what installing means and under what licence. A row with nothing on it is a row
        // nobody can act on.
        let archive = mouth.archive.expect("this platform has a measured archive");
        assert!(archive
            .url
            .starts_with("https://github.com/rhasspy/piper/releases/"));
        assert!(archive.bytes > 10_000_000, "a real size, not a placeholder");
        assert!(mouth.licence.contains("GPL-3"));
        assert!(!mouth.installing.is_empty());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The whole chain, through the code the deck calls: fetch, unpack, find, speak.
    ///
    /// **It ends at a `.wav` with sound in it**, not at an exit code — a success code is a claim
    /// about the process, and this codebase has been caught by that shape before. And it goes
    /// through `install` and `look_for` rather than pointing at a binary somebody put there,
    /// because a harness that prepares its own input measures a path the product does not take.
    #[test]
    #[ignore = "network"]
    fn epoch_can_fetch_a_voice_engine_and_make_it_speak() {
        let dir = scratch("live");
        std::env::set_var("EPOCH_TOOLS", &dir);

        let said = install(Voicebox::Piper, &|_, _| {}).expect("Piper installs");
        assert!(said.contains("piper"), "{said}");

        let mouth = look_for(Voicebox::Piper);
        std::env::remove_var("EPOCH_TOOLS");
        assert!(mouth.installed && mouth.ours, "{mouth:?}");

        // A voice off the real shelf. Skipped rather than failed when there is none: this test
        // is about the engine, and the Voices Workshop has its own.
        let shelf = epoch_models::generative::Library::here()
            .shelf(epoch_models::generative::Shelf::Voices);
        let voice = std::fs::read_dir(&shelf).ok().and_then(|entries| {
            entries
                .flatten()
                .map(|it| it.path())
                .find(|path| path.extension().and_then(|it| it.to_str()) == Some("onnx"))
        });
        let Some(voice) = voice else {
            eprintln!("no voice installed in {shelf:?} — install one first");
            std::fs::remove_dir_all(&dir).ok();
            return;
        };

        let wav = dir.join("said.wav");
        let took = speak(
            Path::new(&mouth.at.unwrap()),
            &voice,
            "Hola, soy Mage. Esta prueba salio del motor.",
            &wav,
        )
        .expect("it speaks");
        let bytes = std::fs::metadata(&wav).unwrap().len();
        eprintln!(
            "spoke {} in {:?} -> {bytes} bytes",
            voice.file_name().unwrap().to_string_lossy(),
            took
        );
        assert!(bytes > 44, "a header with no sound after it is not speech");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A voice's name and its file, joined by the one function that knows how.
    ///
    /// **Found by talking to Mage.** He answered, nothing was spoken, and the reason was a
    /// second place deciding where a voice lives: the deck's check built `<shelf>/<name>` while
    /// the file is `<name>.onnx`, so every voice was reported as *not on the shelf*. The
    /// extension was invisible in the message because it named a path instead of a voice.
    #[test]
    fn a_name_finds_its_file_and_a_stranger_finds_nothing() {
        let shelf = epoch_models::generative::Library::here()
            .shelf(epoch_models::generative::Shelf::Voices);
        let Ok(entries) = std::fs::read_dir(&shelf) else {
            eprintln!("no voices shelf here yet — install one first");
            return;
        };
        let installed: Vec<String> = entries
            .flatten()
            .map(|it| it.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("onnx"))
            .filter_map(|p| Some(p.file_stem()?.to_string_lossy().into_owned()))
            .collect();
        let Some(name) = installed.first() else {
            eprintln!("the shelf is empty — install a voice first");
            return;
        };

        let found = find_voice(name).expect("an installed voice resolves to a file");
        assert!(found.is_file(), "{found:?} is not a file that exists");
        assert_eq!(found.extension().and_then(|it| it.to_str()), Some("onnx"));

        // Exactly, never by resemblance: a near-miss is how an Argentinian voice quietly
        // becomes a Spanish one.
        assert_eq!(find_voice(&format!("{name}-x")), None);
        assert_eq!(find_voice(""), None);
    }

    #[test]
    fn epochs_own_copy_is_preferred_and_says_so() {
        let dir = scratch("ours");
        let nested = dir.join("piper");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join(Voicebox::Piper.binary()), b"").unwrap();

        std::env::set_var("EPOCH_TOOLS", &dir);
        let mouth = look_for(Voicebox::Piper);
        std::env::remove_var("EPOCH_TOOLS");

        assert!(mouth.installed);
        // `ours` is not decoration: Epoch may delete what it fetched and may not delete what it
        // found. A row that could not tell them apart would offer to remove somebody's own
        // install.
        assert!(mouth.ours);
        assert!(mouth
            .at
            .as_deref()
            .unwrap()
            .ends_with(Voicebox::Piper.binary()));

        std::fs::remove_dir_all(&dir).ok();
    }
}
