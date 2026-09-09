//! Turning an RVC voice into something Epoch can use (Phase 15).
//!
//! ## What this is for
//!
//! A character speaks with Piper and may optionally be *coloured* by an RVC voice — measured
//! 2026-09-04 and confirmed by ear: Mage through a Pato Donald model at +12 semitones is
//! unmistakably Donald Duck. `text → Piper → RVC → audio`.
//!
//! RVC ships as a `.pth`, which is a PyTorch pickle. Epoch reads it without executing it
//! (`pickle.rs`) and cannot *run* it: inference needs the model as ONNX, and getting there needs
//! PyTorch exactly once, per voice, and never again.
//!
//! ## Epoch's own Python, and the measurement that decided it
//!
//! The obvious shortcut is to use whatever `python` is on the PATH. Asked on this machine,
//! 2026-09-04:
//!
//! ```text
//! python   C:\Users\…\AppData\Local\hermes\hermes-agent\venv\Scripts\python.exe
//! python3  C:\Users\…\WindowsApps\python3.exe        (the Store stub)
//! py       C:\WINDOWS\py.exe                          (the launcher)
//! ```
//!
//! **The first is another application's virtual environment**, and it has no torch. Installing
//! into it would be writing into somebody else's tree, which ADR-0032 forbids for exactly the
//! reason it forbids copying a checkpoint into ComfyUI's folder.
//!
//! > **A Python on the PATH is not Epoch's Python.** It belongs to whatever put it there, and
//! > the fact that it answers `python` is not consent.
//!
//! So Epoch makes its own, under `tools/rvc/venv`, the way it keeps its own Piper and its own
//! whisper. Deleting that folder undoes it completely, and nothing else on the machine changes.
//!
//! ## What is offered and what is refused
//!
//! Everything here is **offered, never imposed**. A machine with no Python has RVC visibly
//! dormant, with the one command that would change that on screen — and every Piper voice keeps
//! working, because RVC only ever colours a voice that already speaks.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub mod runtime;
pub mod signal;
pub mod timbre;

/// Where the converter and its Python live.
pub fn home() -> PathBuf {
    crate::speech::tools().join("rvc")
}

fn venv_python() -> PathBuf {
    let venv = home().join("venv");
    if cfg!(windows) {
        venv.join("Scripts").join("python.exe")
    } else {
        venv.join("bin").join("python")
    }
}

/// What stands between a `.pth` on disk and a voice a character can use.
///
/// Four separate facts because they have four separate fixes, and collapsing them into
/// *"conversion unavailable"* is the sentence that sends somebody to fix the wrong thing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Forge {
    /// A Python **Epoch could build its own environment from**. `None` means the machine has
    /// none at all, which on Windows is the ordinary state.
    ///
    /// Never a Python on the PATH that belongs to something else: measured on this machine,
    /// `python` was another application's venv.
    pub python: Option<String>,
    /// How somebody gets one, in their own package manager's words. Shown, never run.
    pub how_to_get_python: &'static str,
    /// Epoch's own environment exists.
    pub environment: bool,
    /// …and PyTorch is inside it.
    pub torch: bool,
    /// The model definitions a conversion needs are here.
    pub definitions: bool,
    /// The encoder that listens to Piper is converted and on disk.
    ///
    /// Separate from `definitions` because it has a separate fix and a separate cost: 378 MB
    /// fetched and converted once, and it is needed to **speak** a voice rather than to make
    /// one.
    pub encoder: bool,
    /// The ONNX Runtime is here.
    ///
    /// Also about speaking rather than converting, and it needs no Python at all — which is why
    /// it is its own fact. A machine that can convert and cannot speak is a real state, and
    /// collapsing the two is the sentence that sends somebody to install PyTorch again.
    pub runtime: bool,
    /// What preparing it would cost, said before it is pressed.
    pub cost: &'static str,
}

/// Roughly what the whole toolchain weighs, measured on 2026-09-04 rather than estimated: torch
/// CPU alone is 544 MB and everything a conversion needs is 867 MB. It was written down as
/// *~2.5 GB* from memory first, which is what PyTorch weighs **with CUDA** and three times wrong
/// on the one number that decides whether somebody accepts.
const COST: &str = "About 870 MB, once. It is needed to convert a voice and never to use one, so \
                    it can be deleted afterwards without breaking anything already converted.";

const GET_PYTHON: &str = if cfg!(windows) {
    "winget install Python.Python.3.12"
} else if cfg!(target_os = "macos") {
    "brew install python@3.12"
} else {
    "your distribution's python3 package"
};

/// What the machine can do about RVC right now.
pub fn look() -> Forge {
    let python = find_python();
    let venv = venv_python();
    let environment = venv.is_file();
    Forge {
        python: python.map(|it| it.display().to_string()),
        how_to_get_python: GET_PYTHON,
        environment,
        torch: environment && has_torch(&venv),
        definitions: home().join("scripts").is_dir(),
        encoder: encoder_is_whole(),
        runtime: runtime::ready(),
        cost: COST,
    }
}

/// The encoder that turns speech into the features an RVC model was trained against.
pub fn encoder() -> PathBuf {
    home().join("encoder.onnx")
}

/// Whether the encoder is **whole**.
///
/// **It is two files.** torch writes anything past its inline limit as external data, so what
/// landed here was a 1.2 MB graph beside a 377 MB `encoder.onnx.data` — and `is_file()` on the
/// first was a real check of the wrong quantity. A graph with no weights beside it opens
/// perfectly and fails at the moment somebody speaks, which is the worst place to find out.
fn encoder_is_whole() -> bool {
    let graph = encoder();
    if !graph.is_file() {
        return false;
    }
    let mut beside = graph.clone().into_os_string();
    beside.push(".data");
    // A model small enough to be written inline has no sidecar at all, and that is complete.
    let external = Path::new(&beside);
    !external.exists() || external.metadata().map(|it| it.len() > 0).unwrap_or(false)
}

impl Forge {
    /// Whether a conversion could run right now.
    pub fn ready(&self) -> bool {
        self.environment && self.torch && self.definitions
    }

    /// Whether a voice that has **already** been converted could be spoken right now.
    ///
    /// Not the same question as `ready`, and deliberately so: speaking needs no Python at all.
    /// A machine whose PyTorch has been deleted — which `cost` explicitly invites — keeps every
    /// voice it already made.
    pub fn can_speak(&self) -> bool {
        self.encoder && self.runtime
    }

    /// What to do next, in one sentence — **the single next step and never a list**.
    ///
    /// A panel that showed four unticked boxes would be asking somebody to work out an order.
    /// The order is fixed, so this says the one thing that is actually in the way.
    pub fn next_step(&self) -> Option<String> {
        if self.python.is_none() {
            return Some(format!(
                "This machine has no Python that Epoch can build its own environment from. \
                 `{GET_PYTHON}` installs one; Epoch never uses a Python that belongs to \
                 something else."
            ));
        }
        if !self.environment || !self.torch {
            return Some(format!(
                "Epoch needs its own Python environment first. {COST}"
            ));
        }
        if !self.definitions {
            return Some(
                "The model definitions a conversion reads are not here yet. They are 46 KB and \
                 Epoch fetches them with the rest."
                    .to_owned(),
            );
        }
        if !self.encoder {
            return Some(
                "The encoder that listens to a voice is not converted yet. It is 378 MB fetched \
                 once, and Epoch converts it with the same environment it just built."
                    .to_owned(),
            );
        }
        if !self.runtime {
            return Some(
                runtime::cost()
                    .map(|it| format!("The ONNX Runtime is not here yet. {it}"))
                    .unwrap_or_else(|| {
                        "Epoch has no ONNX Runtime build for this platform, so RVC voices cannot \
                         be spoken here. Piper voices are unaffected."
                            .to_owned()
                    }),
            );
        }
        None
    }
}

/// A Python Epoch may build an environment **from**, never one it would install into.
///
/// The launcher first on Windows — `py -3` is the supported way to reach a real installation and
/// is not itself an environment. Then interpreters that are plainly not somebody else's venv.
fn find_python() -> Option<PathBuf> {
    if cfg!(windows) {
        if let Some(launcher) = crate::paths::on_path("py.exe") {
            return Some(launcher);
        }
    }
    for name in ["python3", "python"] {
        let candidate = crate::paths::on_path(name)
            .or_else(|| crate::paths::on_path(&format!("{name}.exe")))?;
        // **Refused if it is an environment.** A `pyvenv.cfg` beside it means somebody else's
        // virtual environment, and the Windows Store stub answers nothing useful either.
        let inside_a_venv = candidate
            .parent()
            .and_then(|it| it.parent())
            .map(|it| it.join("pyvenv.cfg").is_file())
            .unwrap_or(false);
        let is_store_stub = candidate
            .to_string_lossy()
            .to_lowercase()
            .contains("windowsapps");
        if !inside_a_venv && !is_store_stub {
            return Some(candidate);
        }
    }
    None
}

fn has_torch(python: &Path) -> bool {
    use epoch_models::quiet::Quiet;
    std::process::Command::new(python)
        .args(["-c", "import torch"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .quiet()
        .status()
        .map(|it| it.success())
        .unwrap_or(false)
}

/// Epoch's own conversion script, shipped in the repository and written out when it is needed.
///
/// **Epoch owns the call it makes.** The architecture is a third party's (MIT, fetched beside
/// it); the export — its opset, its dynamic axes and `dynamo=False` — is this project's decision
/// and must be changeable without patching somebody else's file.
const CONVERT: &str = include_str!("rvc/convert.py");

/// Where the model definitions come from.
///
/// `SUC-DriverOld/rvc.onnx`, MIT, 46 KB — **read before it was ever run**: no network, no
/// `subprocess`, no shell, only torch model definitions. A named set, and it grows the day
/// somebody measures another one.
const DEFINITIONS: &str =
    "https://codeload.github.com/SUC-DriverOld/rvc.onnx/zip/refs/heads/master";

/// Build Epoch's own Python environment and fetch what a conversion reads.
///
/// Long — most of it is PyTorch — and it happens once. Everything lands under `tools/rvc`, so
/// deleting that folder undoes the whole thing and touches nothing else on the machine.
pub fn prepare(watching: &dyn Fn(&str)) -> Result<String, String> {
    use epoch_models::quiet::Quiet;

    let python = find_python().ok_or_else(|| {
        format!("This machine has no Python Epoch can build from. `{GET_PYTHON}` installs one.")
    })?;
    let root = home();
    std::fs::create_dir_all(&root).map_err(|why| format!("could not make {root:?}: {why}"))?;

    if !venv_python().is_file() {
        watching("Making Epoch's own Python environment…");
        // `py -3` on Windows is the launcher; elsewhere the interpreter is called directly.
        let mut making = std::process::Command::new(&python);
        if python.file_name().and_then(|it| it.to_str()) == Some("py.exe") {
            making.arg("-3");
        }
        let done = making
            .arg("-m")
            .arg("venv")
            .arg(root.join("venv"))
            .quiet()
            .output()
            .map_err(|why| format!("could not start {python:?}: {why}"))?;
        if !done.status.success() {
            return Err(format!(
                "Epoch's Python environment could not be made: {}",
                String::from_utf8_lossy(&done.stderr).trim()
            ));
        }
    }

    let venv = venv_python();
    if !has_torch(&venv) {
        watching("Installing PyTorch (CPU). About 870 MB, once…");
        let done = std::process::Command::new(&venv)
            .args([
                "-m",
                "pip",
                "install",
                "--quiet",
                "torch",
                "--index-url",
                "https://download.pytorch.org/whl/cpu",
            ])
            .quiet()
            .output()
            .map_err(|why| format!("could not run pip: {why}"))?;
        if !done.status.success() {
            return Err(format!(
                "PyTorch would not install: {}",
                String::from_utf8_lossy(&done.stderr).trim()
            ));
        }
        // (the rest is installed below, outside this guard)
    }

    if !has_module(&venv, "scipy") {
        watching("Installing what the model definitions read…");
        // What the export needs beyond torch, **read out of the definitions rather than
        // guessed**: `grep` over what they import answers `numpy`, `scipy`, `transformers`, and
        // the export itself needs `onnx` and `onnxscript`. Guessing produced one missing module
        // per attempt, which is a game somebody plays for an hour before asking the question.
        let done = std::process::Command::new(&venv)
            .args([
                "-m",
                "pip",
                "install",
                "--quiet",
                "onnx",
                "onnxscript",
                "numpy",
                "scipy",
                "transformers",
            ])
            .quiet()
            .output()
            .map_err(|why| format!("could not run pip: {why}"))?;
        if !done.status.success() {
            return Err(format!(
                "The rest of the toolchain would not install: {}",
                String::from_utf8_lossy(&done.stderr).trim()
            ));
        }
    }

    if !root.join("scripts").is_dir() {
        watching("Fetching the model definitions (46 KB)…");
        fetch_definitions(&root)?;
    }

    // Written before the encoder, because the encoder's conversion reads the definitions beside
    // it and a half-written folder should still be a folder that can convert.
    std::fs::write(root.join("convert.py"), CONVERT)
        .map_err(|why| format!("could not write the conversion script: {why}"))?;

    if !encoder().is_file() {
        prepare_encoder(&root, watching)?;
    }

    if !runtime::ready() {
        let named = runtime::cost().unwrap_or_default();
        watching(&format!("Fetching the ONNX Runtime. {named}"));
        runtime::install(&|_, _| {})?;
    }

    Ok("Epoch can convert and speak RVC voices on this machine.".to_owned())
}

/// Where the encoder comes from, and what it is.
///
/// ContentVec (`lengyue233/content-vec-best`, **MIT**) — the encoder every RVC v2 model was
/// trained against. It is a PyTorch pickle, so it goes through the same reader a voice does
/// before torch is allowed near it.
///
/// **Not a ready-made ONNX off the internet.** Several were looked for on 2026-09-06 and every
/// one either did not exist or was behind a login; the two that answered at all were `401`. A
/// blob whose provenance cannot be checked is the last thing to hand a subsystem whose whole
/// guarantee is that it reads a file before running it — and Epoch already owns a converter.
const ENCODER: &str = "https://huggingface.co/lengyue233/content-vec-best/resolve/main";

/// Fetch ContentVec and export it, using the environment that was just built.
fn prepare_encoder(root: &Path, watching: &dyn Fn(&str)) -> Result<(), String> {
    use epoch_models::quiet::Quiet;

    let source = root.join("content-vec");
    std::fs::create_dir_all(&source).map_err(|why| format!("could not make {source:?}: {why}"))?;

    let weights = source.join("pytorch_model.bin");
    if !weights.is_file() {
        watching("Fetching the encoder (378 MB, once)…");
        crate::speech::fetch(
            &format!("{ENCODER}/config.json"),
            "config.json",
            &source,
            &|_, _| {},
        )?;
        crate::speech::fetch(
            &format!("{ENCODER}/pytorch_model.bin"),
            "pytorch_model.bin",
            &source,
            &|_, _| {},
        )?;
    }

    // **The same gate a voice goes through.** `from_pretrained` unpickles, so a file Epoch would
    // refuse must not reach it — and the encoder is downloaded from a third party exactly like a
    // voice is. Reading it costs a fraction of a second against 378 MB already spent.
    crate::pickle::read(&weights)
        .map_err(|why| format!("Epoch will not convert that encoder: {why}"))?;

    watching("Converting the encoder…");
    let done = std::process::Command::new(venv_python())
        .arg(root.join("scripts").join("convert_hubert_to_onnx.py"))
        .arg("--model-dir")
        .arg(&source)
        .arg("--output")
        .arg(encoder())
        // **Windows pipes are not UTF-8 by default.** `transformers` prints a tick, Python
        // encodes stdout as cp1252 when it is a pipe rather than a console, and the whole
        // conversion died with `'charmap' codec can't encode character '✅'` -- a model
        // that converted perfectly, lost to a decoration in a log line.
        .env("PYTHONIOENCODING", "utf-8")
        // The script reads the definitions as a package. **`current_dir` is not enough**:
        // Python puts the *script's* folder on the path, not the working directory, so from
        // `scripts/` the `infer` package next door is invisible and it answers
        // `No module named 'infer'`. Epoch's own script inserts its folder itself; this one is
        // somebody else's, so it is told rather than patched.
        .env("PYTHONPATH", root)
        .current_dir(root)
        .quiet()
        .output()
        .map_err(|why| format!("the encoder converter could not be started: {why}"))?;

    // The side effect, not the exit code -- and **the whole side effect**: torch writes the
    // weights beside the graph when they are large, so checking only the `.onnx` would call a
    // 1.2 MB skeleton a converted encoder.
    if !encoder_is_whole() {
        return Err(format!(
            "The encoder did not convert: {}",
            String::from_utf8_lossy(&done.stderr)
                .lines()
                .last()
                .unwrap_or("")
                .trim()
        ));
    }

    // 378 MB that has now been consumed. Keeping it would mean this feature costs twice what it
    // needs to for the life of the installation, and it is Epoch's own scratch under Epoch's own
    // folder -- fetching it again is one download, which the conversion already does by itself.
    let _ = std::fs::remove_file(source.join("pytorch_model.bin"));
    Ok(())
}

/// Whether one module is importable, without asking about the rest.
///
/// **Its own question, outside the torch guard.** The extras were installed inside
/// `if !has_torch`, so a machine that already had torch never got them and the conversion failed
/// on `scipy` with everything looking prepared. The same shape this project already records: a
/// guard that exists to skip expensive work makes everything below it accidental.
fn has_module(python: &Path, module: &str) -> bool {
    use epoch_models::quiet::Quiet;
    std::process::Command::new(python)
        .args(["-c", &format!("import {module}")])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .quiet()
        .status()
        .map(|it| it.success())
        .unwrap_or(false)
}

/// Fetch the definitions and flatten the one directory GitHub wraps them in.
fn fetch_definitions(root: &Path) -> Result<(), String> {
    let answer = ureq::get(DEFINITIONS)
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|why| format!("the model definitions could not be downloaded: {why}"))?;
    let mut bytes = Vec::new();
    {
        // Capped, because this is somebody else's server answering: the definitions are 46 KB
        // and anything claiming to be eight megabytes of them is not them.
        use std::io::Read as _;
        std::io::copy(&mut answer.into_reader().take(8 << 20), &mut bytes)
            .map_err(|why| format!("the download stopped: {why}"))?;
    }

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|why| format!("that is not a readable archive: {why}"))?;
    for at in 0..zip.len() {
        let mut entry = zip
            .by_index(at)
            .map_err(|why| format!("could not read the archive: {why}"))?;
        let Some(inside) = entry.enclosed_name().map(|it| it.to_owned()) else {
            // A name that escapes its own folder is refused rather than sanitised: `zip` answers
            // `None` for exactly that, and nothing here needs to guess what was meant.
            continue;
        };
        // GitHub wraps everything in `rvc.onnx-master/`. Only the two directories a conversion
        // reads are kept — the GUI and the inference scripts are not Epoch's to run.
        let mut parts = inside.components();
        parts.next();
        let kept: PathBuf = parts.collect();
        let wanted = kept.starts_with("infer") || kept.starts_with("scripts");
        if !wanted || entry.is_dir() {
            continue;
        }
        let out = root.join(&kept);
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|why| format!("could not make {parent:?}: {why}"))?;
        }
        let mut file =
            std::fs::File::create(&out).map_err(|why| format!("could not write {out:?}: {why}"))?;
        std::io::copy(&mut entry, &mut file)
            .map_err(|why| format!("could not write {out:?}: {why}"))?;
    }
    Ok(())
}

/// Convert one checkpoint, and answer with where the ONNX landed.
///
/// **The pickle is read first**, so a file Epoch would refuse never reaches torch. That order is
/// the whole guarantee: `weights_only=True` in the script is a second lock on the same door, and
/// neither is asked to hold alone.
pub fn convert(checkpoint: &Path, into: &Path) -> Result<PathBuf, String> {
    use epoch_models::quiet::Quiet;

    let read = crate::pickle::read(checkpoint).map_err(|why| why.to_string())?;
    if read.generation().is_none() {
        return Err(format!(
            "That checkpoint's embedding is {:?}, which is neither v1 nor v2. Epoch will not \
             guess at a model it cannot recognise.",
            read.embedding
        ));
    }

    let forge = look();
    if !forge.ready() {
        return Err(forge
            .next_step()
            .unwrap_or_else(|| "Epoch cannot convert voices on this machine yet.".to_owned()));
    }

    let name = checkpoint
        .file_stem()
        .map(|it| it.to_string_lossy().into_owned())
        .ok_or("that is not a checkpoint's name")?;
    let output = into.join(format!("{name}.onnx"));
    std::fs::create_dir_all(into).map_err(|why| format!("could not make {into:?}: {why}"))?;

    let done = std::process::Command::new(venv_python())
        // See `prepare_encoder`: a pipe on Windows is cp1252 unless it is told otherwise, and
        // one non-ASCII character in a library's log line takes the whole conversion down.
        .env("PYTHONIOENCODING", "utf-8")
        .arg(home().join("convert.py"))
        .arg("--checkpoint")
        .arg(checkpoint)
        .arg("--output")
        .arg(&output)
        // The width Epoch measured out of the tensor, handed over rather than guessed at again.
        .arg("--embedding")
        .arg(read.embedding.unwrap_or_default().to_string())
        .quiet()
        .output()
        .map_err(|why| format!("the converter could not be started: {why}"))?;

    // **The side effect, not the exit code.** A converter that answered zero and wrote nothing
    // would leave somebody with a voice that fails at the moment they try to speak.
    if !output.is_file() {
        return Err(format!(
            "The conversion produced no model: {}",
            String::from_utf8_lossy(&done.stderr)
                .lines()
                .last()
                .unwrap_or("")
                .trim()
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cold_forge_names_one_next_step_and_not_a_list() {
        let nothing = Forge {
            python: None,
            how_to_get_python: GET_PYTHON,
            environment: false,
            torch: false,
            definitions: false,
            encoder: false,
            runtime: false,
            cost: COST,
        };
        assert!(!nothing.ready());
        // The order is fixed, so a panel that showed four unticked boxes would be asking
        // somebody to work out which one comes first.
        let said = nothing.next_step().expect("something is in the way");
        assert!(said.contains(GET_PYTHON), "{said}");
        assert!(!said.contains("definitions"), "one step, not all of them");
    }

    #[test]
    fn each_missing_piece_says_its_own_sentence() {
        let mut forge = Forge {
            python: Some("py.exe".into()),
            how_to_get_python: GET_PYTHON,
            environment: false,
            torch: false,
            definitions: false,
            encoder: false,
            runtime: false,
            cost: COST,
        };
        assert!(forge
            .next_step()
            .unwrap()
            .contains("own Python environment"));

        forge.environment = true;
        forge.torch = true;
        assert!(forge.next_step().unwrap().contains("model definitions"));

        forge.definitions = true;
        // Converting is possible here and speaking is not, which is a real state and the reason
        // the two questions are asked separately.
        assert!(forge.ready());
        assert!(!forge.can_speak());
        assert!(forge.next_step().unwrap().contains("encoder"));

        forge.encoder = true;
        assert!(forge.next_step().unwrap().contains("ONNX Runtime"));

        forge.runtime = true;
        assert_eq!(forge.next_step(), None);
        assert!(forge.can_speak());
    }

    /// Build everything, and say what each piece cost.
    ///
    /// Separate from the conversion below because it needs no `.pth`: a machine can be made
    /// ready to speak before anybody has downloaded a voice, and that is the ordinary order.
    #[test]
    #[ignore = "network, and about 1.3 GB"]
    fn the_forge_prepares_itself() {
        let started = std::time::Instant::now();
        let said =
            prepare(&|step| eprintln!("  [{:>6.1}s] {step}", started.elapsed().as_secs_f32()))
                .expect("the forge prepares");
        eprintln!("{said} (in {:?})", started.elapsed());

        let forge = look();
        eprintln!("{forge:#?}");
        assert!(forge.ready(), "it cannot convert");
        assert!(forge.can_speak(), "it cannot speak");
        for (what, path) in [
            ("encoder", encoder()),
            ("runtime", runtime::library().unwrap_or_default()),
        ] {
            let bytes = std::fs::metadata(&path).map(|it| it.len()).unwrap_or(0);
            eprintln!(
                "{what:>8}: {:.1} MB  {}",
                bytes as f64 / 1e6,
                path.display()
            );
            assert!(bytes > 0, "{what} is not on disk");
        }
    }

    /// The whole forge: build the environment, fetch the definitions, convert a real voice.
    ///
    /// **It ends at an ONNX on disk**, not at an exit code — a converter that answered zero and
    /// wrote nothing would leave somebody with a voice that fails when they try to speak.
    #[test]
    #[ignore = "network, and about 870 MB"]
    fn epoch_can_prepare_a_forge_and_convert_a_voice() {
        let checkpoint = std::env::var("EPOCH_TEST_PTH").unwrap_or_default();
        if checkpoint.is_empty() {
            eprintln!("set EPOCH_TEST_PTH to a real RVC .pth");
            return;
        }
        let said = prepare(&|step| eprintln!("  {step}")).expect("the forge prepares");
        eprintln!("{said}");

        let forge = look();
        assert!(forge.ready(), "{forge:?}");

        let into = std::env::temp_dir().join("epoch-rvc-out");
        let started = std::time::Instant::now();
        let made = convert(Path::new(&checkpoint), &into).expect("it converts");
        let bytes = std::fs::metadata(&made).map(|it| it.len()).unwrap_or(0);
        eprintln!(
            "converted in {:?} -> {} ({:.1} MB)",
            started.elapsed(),
            made.display(),
            bytes as f64 / 1e6
        );
        assert!(
            bytes > 10_000_000,
            "an ONNX of that model is tens of megabytes"
        );

        // What Epoch knows about it, written beside it so it survives the process that read it.
        let beside = made.with_extension("json");
        assert!(beside.is_file(), "the metadata is written beside the model");
    }

    /// What this machine actually answers.
    #[test]
    fn this_machine_is_read_rather_than_assumed() {
        let forge = look();
        eprintln!("{forge:?}");
        // The measurement that decided the design: `python` here is another application's venv,
        // so whatever is found must not be one.
        if let Some(found) = &forge.python {
            assert!(
                !found.to_lowercase().contains("hermes"),
                "a Python on the PATH is not Epoch's Python: {found}"
            );
        }
    }
}
