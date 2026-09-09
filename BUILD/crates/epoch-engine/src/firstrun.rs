//! What a machine already has, and what it can be given.
//!
//! ## Why this exists
//!
//! The installer puts Epoch on the disk and nothing else — the executable and one World,
//! `archipelago`. Everything a World actually *thinks* with is somebody
//! else's program, and until now a new machine opened a World with nothing behind it and left the
//! person to find five decks.
//!
//! ## The rule this obeys, which is older than this file
//!
//! **Offer, never impose.** Nothing here is checked by default, every size that is known is shown,
//! and declining all of it produces a working Epoch that says what is missing. A first run that
//! installs six programs because it can is a first run that decided for somebody.
//!
//! **And Epoch never holds a key.** Nothing here signs in. An agent's login is the agent's own,
//! started in its own window, and this file does not have a field to put a credential in.
//!
//! ## Every command here was measured on 2026-08-30, not remembered
//!
//! `winget show <id>` answered for each of the five packages, and `npm ls -g` on the machine this
//! was built on reports `@anthropic-ai/claude-code` and `@google/gemini-cli` — which is how those
//! two are named here rather than from memory. `OpenAI.Codex` is in winget at 0.146.1, so Codex is
//! offered as a native program rather than through npm, which is how it is actually installed
//! here.
//!
//! Somebody else's package id is a measurement and it can stop being true. When one of these
//! stops resolving, the honest failure is the package manager's own sentence, kept whole.

use serde::Serialize;

/// How a program is added to this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "id")]
pub enum How {
    /// Windows' own package manager.
    Winget(String),
    /// macOS, through Homebrew. `--cask` where the package is an application.
    Brew(String),
    /// A global npm package, which needs Node.js first.
    Npm(String),
    /// **Epoch fetches it itself**, because no package manager has it.
    ///
    /// Measured rather than assumed, and the measurements are in the modules that do the
    /// fetching: `winget search piper` answers `npiperelay` and `PhotoPiper`, Homebrew answers
    /// 404 as formula and as cask, and whisper.cpp ships releases rather than packages. So these
    /// four are not a different *kind* of offer — they are the same offer with the only route
    /// that exists.
    Epoch(Fetch),
}

/// One thing Epoch downloads itself, named rather than described.
///
/// **A name, never a URL or a command.** Every address, size and licence already lives in the
/// module that owns the thing, and writing one here would be the same fact in two places — the
/// mistake `package()` exists two hundred lines below to avoid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "what")]
pub enum Fetch {
    /// whisper.cpp and one model, in that order. **One decision, two downloads** — the same
    /// grouping `install_ear` already makes, and for the same reason: a program that hears
    /// nothing is not an ear, and two rows pressed in the wrong order teach the order by failing.
    Ear,
    /// Piper's release archive.
    Mouth,
    /// A voice for it. Its own box because it is its own *no*, and it needs Piper first.
    Voice,
    /// The Python environment that turns a downloaded RVC model into one Epoch can run.
    Forge,
}

impl How {
    /// The command, as it would be typed.
    pub fn command(&self) -> String {
        match self {
            // The two `--accept-*` flags are what stops winget stopping to ask: this runs with no
            // terminal for somebody to answer in, and a prompt nobody can see is a hang.
            How::Winget(id) => format!(
                "winget install --exact --id {id} --accept-package-agreements --accept-source-agreements"
            ),
            How::Brew(id) => format!("brew install {id}"),
            How::Npm(id) => format!("npm install -g {id}"),
            // Not a command, because there is no command — said as what actually happens, in
            // the same place the other three say what they would type.
            How::Epoch(what) => match what {
                Fetch::Ear => "Epoch downloads whisper.cpp and its base model".to_owned(),
                Fetch::Mouth => "Epoch downloads Piper's release archive".to_owned(),
                Fetch::Voice => "Epoch downloads one Piper voice".to_owned(),
                Fetch::Forge => {
                    "Epoch builds a Python environment and installs torch into it".to_owned()
                }
            },
        }
    }

    /// The program and its arguments, for running it directly rather than through a shell.
    pub fn parts(&self) -> Vec<String> {
        self.command()
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    }
}

/// One thing First Run can find, or add.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    pub id: String,
    pub name: String,
    /// One line: what this gives the machine. Never a feature list.
    pub what: String,
    /// Where it was found. `None` is *not here*, and it is the only thing that makes it offerable.
    pub found: Option<String>,
    /// How Epoch would add it. `None` where Epoch cannot, which is said rather than hidden.
    pub how: Option<How>,
    /// Why Epoch cannot add it, when it cannot. Never empty while `how` is `None`.
    pub why_not: Option<String>,
    /// What it needs first, by id. Resolved into steps of its own.
    pub needs: Vec<String>,
    /// What it weighs, where that is **measured**.
    ///
    /// `None` for everything a package manager installs, and that is the honest answer rather
    /// than a gap: Epoch would have to ask winget or npm to find out, which is a network call
    /// per row in a survey that has to answer instantly and offline. A row with no size says
    /// nothing about its size; it does not claim to be small.
    ///
    /// It matters most where the numbers are furthest apart. 8.4 MB against 870 MB is not a
    /// difference somebody should discover after pressing INSTALL.
    pub bytes: Option<u64>,
}

impl Offer {
    pub fn here(&self) -> bool {
        self.found.is_some()
    }
}

/// Everything First Run knows how to look for, in the order it is worth reading.
///
/// **Runtimes first, because they are what makes a World think**, then the studio, then the
/// agents. A machine with none of these still runs Epoch and says so.
pub fn survey() -> Vec<Offer> {
    let windows = cfg!(target_os = "windows");
    let mut all = Vec::new();

    // --- the package manager, because nothing below can be added without one --------------
    let manager = if windows { "winget" } else { "brew" };
    all.push(Offer {
        id: "package-manager".to_owned(),
        name: if windows { "winget" } else { "Homebrew" }.to_owned(),
        what: "Installs the programs below. Without it, each one has to be downloaded by hand."
            .to_owned(),
        found: crate::models::runtimes::found_in(manager, None, None)
            .map(|at| at.display().to_string())
            .or_else(|| on_the_path(manager)),
        // **Epoch does not install a package manager.** winget arrives with Windows and Homebrew
        // is a command somebody runs knowing what it does; installing either from here would be
        // Epoch changing how the machine gets its software.
        how: None,
        why_not: Some(if windows {
            "winget comes with Windows. If it is missing, it is in the Microsoft Store as App Installer.".to_owned()
        } else {
            "Homebrew is installed from brew.sh. Epoch does not change how this machine gets its software.".to_owned()
        }),
        needs: Vec::new(),
        bytes: None,
    });

    // --- the three local runtimes ---------------------------------------------------------
    for runtime in crate::models::runtimes::Runtime::ALL {
        let seen = crate::models::runtimes::look_for(runtime);
        all.push(Offer {
            id: runtime.id().to_owned(),
            name: runtime.name().to_owned(),
            what: match runtime {
                crate::models::runtimes::Runtime::Ollama => {
                    "Runs models on this machine. The easiest one to start with."
                }
                crate::models::runtimes::Runtime::LlamaCpp => {
                    "Runs models on this machine, and the only one Epoch can measure a full curve on."
                }
                crate::models::runtimes::Runtime::LmStudio => {
                    "Runs models on this machine, with its own window for browsing them."
                }
            }
            .to_owned(),
            found: seen.found_at.clone(),
            how: package(runtime.install_command()),
            why_not: None,
            needs: vec!["package-manager".to_owned()],
            bytes: None,
        });
    }

    // --- the studio -----------------------------------------------------------------------
    let studio = crate::models::studio::Studio::ComfyUi;
    let seen = crate::models::studio::look_for(studio);
    all.push(Offer {
        id: "comfyui".to_owned(),
        name: "ComfyUI".to_owned(),
        what: "Draws pictures, video, sound and 3D. Nothing in Epoch makes an image without it."
            .to_owned(),
        found: seen.found_at.clone(),
        how: package(studio.install_command()),
        why_not: None,
        needs: vec!["package-manager".to_owned()],
        bytes: None,
    });

    // --- the voice chain, which no package manager has ------------------------------------
    //
    // **Four boxes, because they are four separate answers.** Somebody may want the crew to
    // speak and never want to talk back; somebody else wants only the ear. Declining all four
    // still produces a working Epoch — it simply does not talk, and the Character editor says
    // so on the control rather than showing an empty dropdown.
    //
    // Everything here is fetched by Epoch itself, and every address, size and licence is read
    // from the module that owns the thing. Nothing about Piper is written twice.

    // The ear. Its model rides with it: `install_ear` already treats the two as one decision,
    // and a whisper with no model is a program that hears nothing.
    let ear = crate::hearing::look_for(crate::hearing::Ear::WhisperCpp);
    let smallest = crate::hearing::Hearing::Base;
    all.push(Offer {
        id: "whisper".to_owned(),
        name: "whisper.cpp".to_owned(),
        what: "The ear. Speak to the crew instead of typing. Runs on the processor, so it never                competes with a model or a picture."
            .to_owned(),
        // **Installed is the binary *and* a model.** Reporting the program alone as found would
        // hide the state this module already calls out on its own type: an ear with no model
        // hears nothing, and the box would disappear leaving somebody with half a chain.
        found: (ear.installed && ear.models.iter().any(|it| it.here))
            .then(|| ear.at.clone().unwrap_or_else(|| "this machine".to_owned())),
        // **Offered only where there is something to fetch.** whisper.cpp publishes an
        // xcframework for macOS rather than a runnable binary, so `archive()` answers `None`
        // there — and a box that offers a download nothing can perform is the invented gauge
        // wearing an install button. It keeps its frame, loses its light, and says which of the
        // two absences it is.
        //
        // Found on the Mac. On Windows this branch cannot be reached, which is why nothing here
        // had ever noticed that the box was live with no bytes behind it.
        how: ear
            .archive
            .as_ref()
            .map(|_| How::Epoch(Fetch::Ear)),
        why_not: ear.archive.as_ref().is_none().then(|| {
            "whisper.cpp publishes no runnable build for this machine yet — macOS gets an              xcframework rather than a program. Everything else here still works; the crew              simply cannot be spoken to."
                .to_owned()
        }),
        needs: Vec::new(),
        bytes: ear
            .archive
            .as_ref()
            .map(|it| it.bytes + smallest.bytes()),
    });

    // The mouth.
    let mouth = crate::speech::look_for(crate::speech::Voicebox::Piper);
    all.push(Offer {
        id: "piper".to_owned(),
        name: "Piper".to_owned(),
        what: "The mouth. Characters say their answers out loud. No server and no port — it is                handed one sentence and writes a sound."
            .to_owned(),
        found: mouth.at.clone(),
        how: Some(How::Epoch(Fetch::Mouth)),
        why_not: None,
        needs: Vec::new(),
        bytes: crate::speech::Voicebox::Piper.archive().map(|it| it.bytes),
    });

    // And somebody for it to sound like.
    //
    // **Its own box and not part of Piper’s**, because which voice is a choice and a person may
    // already have one. It needs Piper, which the plan resolves into a step of its own rather
    // than into an order somebody has to know.
    all.push(Offer {
        id: "voice".to_owned(),
        name: "A starter voice".to_owned(),
        what: "One voice for Piper, in the language this machine is set to where one exists.                A mouth with no voice installs nothing you can hear."
            .to_owned(),
        found: crate::speech::voices_here()
            .first()
            .map(|it| it.name.clone()),
        how: Some(How::Epoch(Fetch::Voice)),
        why_not: None,
        needs: vec!["piper".to_owned()],
        // **Unknown here on purpose.** Which voice is chosen at fetch time from what the
        // repository actually holds, and its size is that file’s. A number written here would
        // be about a different voice than the one that arrives.
        bytes: None,
    });

    // The forge: what turns a downloaded RVC model into one Epoch can run.
    let forge = crate::rvc::look();
    all.push(Offer {
        id: "voice-forge".to_owned(),
        name: "PyTorch (processor build)".to_owned(),
        what: "Turns a downloaded voice into one Epoch can use. Needed once to convert and never                to speak — it can be removed afterwards without breaking a voice already made."
            .to_owned(),
        found: forge.torch.then(|| "this machine".to_owned()),
        how: Some(How::Epoch(Fetch::Forge)),
        why_not: None,
        needs: vec!["python".to_owned()],
        // Measured 2026-09-04: torch CPU alone is 544 MB and the whole toolchain a conversion
        // needs is 867. It was ~2.5 GB from memory first, which is what torch weighs with CUDA.
        bytes: Some(867_000_000),
    });

    // Python, because the forge cannot be built without one and Epoch will not borrow another
    // application’s. Offered rather than assumed: `rvc::look` already refuses to treat a Python
    // on the PATH as its own.
    all.push(Offer {
        id: "python".to_owned(),
        name: "Python".to_owned(),
        what: "Needed only to convert a voice. Nothing else in Epoch uses it.".to_owned(),
        found: forge.python.clone(),
        how: package(forge.how_to_get_python),
        why_not: (package(forge.how_to_get_python).is_none())
            .then(|| format!("Install it with {}.", forge.how_to_get_python)),
        needs: vec!["package-manager".to_owned()],
        bytes: None,
    });

    // --- Node.js, which two of the agents need -------------------------------------------
    all.push(Offer {
        id: "node".to_owned(),
        name: "Node.js".to_owned(),
        what: "Needed by Claude Code and Gemini CLI. Nothing else here uses it.".to_owned(),
        found: crate::models::runtimes::found_in("node", None, None)
            .map(|at| at.display().to_string())
            .or_else(|| on_the_path("node")),
        how: package(if windows {
            "winget install OpenJS.NodeJS"
        } else {
            "brew install node"
        }),
        why_not: None,
        needs: vec!["package-manager".to_owned()],
        bytes: None,
    });

    // --- the agents -----------------------------------------------------------------------
    //
    // **Found by asking the programs**, which is what `agents::installed` already does — the same
    // probe the Launcher's CREW LINKS draws, so the two can never disagree about what is here.
    let known = crate::agents::installed();
    // **The programs' own ids, never a string typed here.** Written as `"gemini-cli"` first,
    // from the npm package name, and the program calls itself `gemini` — so the row read *not
    // installed* about an agent the Launcher was showing as READY two panels away. Two places
    // answering one question disagree the first time somebody writes one of them from memory.
    for (id, name, what, how, needs) in [
        (
            crate::agents::claude::ID,
            "Claude Code",
            "Anthropic's coding agent. Epoch gives it your crew, your Quest and this World.",
            Some(How::Npm("@anthropic-ai/claude-code".to_owned())),
            vec!["node".to_owned()],
        ),
        (
            crate::agents::codex::ID,
            "Codex",
            "OpenAI's coding agent. Installed as its own program rather than through npm.",
            package("winget install OpenAI.Codex"),
            vec!["package-manager".to_owned()],
        ),
        (
            crate::agents::gemini::ID,
            "Gemini CLI",
            "Google's agent. Signs in with an API key rather than an account.",
            Some(How::Npm("@google/gemini-cli".to_owned())),
            vec!["node".to_owned()],
        ),
    ] {
        all.push(Offer {
            id: id.to_owned(),
            name: name.to_owned(),
            what: what.to_owned(),
            // Keyed on `kind` rather than `id`: an id identifies a *sign-in* and a kind names the
            // program, and this is a question about the program. A machine with two Claude Code
            // accounts must not read as two Claude Codes to install.
            found: known
                .survey()
                .iter()
                .find(|one| one.kind == id && one.installed)
                .map(|one| {
                    one.looked_in
                        .clone()
                        .unwrap_or_else(|| "this machine".to_owned())
                }),
            why_not: how.is_none().then(|| {
                "Epoch has no measured way to install this here. It is downloaded from its own site."
                    .to_owned()
            }),
            how,
            needs,
            bytes: None,
        });
    }

    all
}

/// A `How` from a command Epoch already knows, or nothing on a platform it was not written for.
///
/// The runtimes and the studio have carried their install commands since long before this file,
/// and re-typing them here would be one fact written twice — the two would disagree the first time
/// one of them changed.
fn package(command: &str) -> Option<How> {
    let mut words = command.split_whitespace();
    match (words.next(), words.next()) {
        (Some("winget"), Some("install")) => {
            // `winget install <id>` and `winget install --id <id>` both appear in the wild; the
            // last word is the package either way.
            command
                .split_whitespace()
                .last()
                .map(|id| How::Winget(id.to_owned()))
        }
        (Some("brew"), Some("install")) => {
            let rest: Vec<&str> = words.filter(|it| !it.is_empty()).collect();
            (!rest.is_empty()).then(|| How::Brew(rest.join(" ")))
        }
        _ => None,
    }
}

/// Whether a program answers on the PATH, for the ones that are not installed into a known folder.
fn on_the_path(program: &str) -> Option<String> {
    let asking = if cfg!(target_os = "windows") {
        ("where", program)
    } else {
        ("which", program)
    };
    let said = {
        use crate::models::quiet::Quiet;
        std::process::Command::new(asking.0)
            .arg(asking.1)
            .quiet()
            .output()
            .ok()?
    };
    if !said.status.success() {
        return None;
    }
    String::from_utf8_lossy(&said.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|it| !it.is_empty())
        .map(str::to_owned)
}

/// The steps that adding these would take, in the order they have to happen.
///
/// **What something needs comes first, and appears as a step of its own.** A person who asks for
/// Claude Code on a machine with no Node.js is asking for two installs, and a list that hid one of
/// them would be a progress bar that lies about how long it is.
///
/// Anything already here is not a step. Anything Epoch cannot install is not a step either — it is
/// said on the row instead, which is where somebody can act on it.
pub fn plan(all: &[Offer], wanted: &[String]) -> Vec<Step> {
    let mut steps: Vec<Step> = Vec::new();
    let mut add = |offer: &Offer, because: Option<&str>| {
        if offer.here() || steps.iter().any(|it| it.id == offer.id) {
            return;
        }
        let Some(how) = offer.how.clone() else {
            return;
        };
        steps.push(Step {
            id: offer.id.clone(),
            name: offer.name.clone(),
            because: because.map(str::to_owned),
            command: how.command(),
            how,
        });
    };

    for id in wanted {
        let Some(offer) = all.iter().find(|it| &it.id == id) else {
            continue;
        };
        for needed in &offer.needs {
            if let Some(first) = all.iter().find(|it| &it.id == needed) {
                add(first, Some(&offer.name));
            }
        }
        add(offer, None);
    }
    steps
}

/// One install, as the screen draws it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Step {
    pub id: String,
    pub name: String,
    /// The thing that needed this one, when it is not what was asked for.
    pub because: Option<String>,
    pub command: String,
    pub how: How,
}

/// Run one step, reporting every line the program writes as it writes it.
///
/// ## Through a shell, and the reason is measured rather than stylistic
///
/// `Command::new("npm")` fails on Windows: npm is `npm.cmd`, and Rust's process spawn appends
/// `.exe` and nothing else. winget is a real executable and would work either way, so a runner
/// that only ever tried winget would look correct until the first agent install. One shell for
/// both — `cmd /C` on Windows, `sh -c` elsewhere — is one path that works for every step, and the
/// commands here are Epoch's own words with nothing of the user's in them.
///
/// ## And what it says back is kept whole
///
/// Every line goes to the caller as it arrives, which is what makes `Show details` a record of
/// what actually happened rather than a summary of it. A failure carries the program's own last
/// words: it knows why far better than a sentence written here, and this codebase has paid for
/// rewriting somebody else's refusal before.
pub fn run(step: &Step, say: &dyn Fn(&str)) -> Result<f64, String> {
    use std::io::BufRead;

    let began = std::time::Instant::now();

    // **What Epoch fetches itself does not go through a shell**, and the reason is not style: no
    // package manager has any of these, which is the measurement that made `How::Epoch` exist.
    // Every one of them calls the *same function the Workshop's own button calls* — two places
    // installing Piper is two places that disagree the first time the release URL moves.
    if let How::Epoch(what) = &step.how {
        return fetch(what, say).map(|_| began.elapsed().as_secs_f64());
    }

    let mut child = shell(&step.command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|why| format!("{} could not be started: {why}", step.command))?;

    // stderr first and on its own thread: a package manager writes progress to one and trouble to
    // the other, and reading them in sequence would hold the second until the first was done.
    let complaints = child.stderr.take().map(|it| {
        std::thread::spawn(move || {
            std::io::BufReader::new(it)
                .lines()
                .map_while(Result::ok)
                .collect::<Vec<String>>()
        })
    });
    if let Some(out) = child.stdout.take() {
        for line in std::io::BufReader::new(out).lines().map_while(Result::ok) {
            let line = line.trim_end();
            if !line.is_empty() {
                say(line);
            }
        }
    }
    let said = child
        .wait()
        .map_err(|why| format!("{} did not finish: {why}", step.name))?;
    let stderr = complaints.and_then(|it| it.join().ok()).unwrap_or_default();
    for line in &stderr {
        if !line.trim().is_empty() {
            say(line);
        }
    }

    if said.success() {
        return Ok(began.elapsed().as_secs_f64());
    }
    // The program's own last words, or its exit code where it said nothing at all — which is
    // itself a fact, and a better one than a sentence claiming to know why.
    let last = stderr
        .iter()
        .rev()
        .find(|it| !it.trim().is_empty())
        .map(|it| it.trim().to_owned());
    Err(match last {
        Some(words) => format!("{} would not install: {words}", step.name),
        None => format!(
            "{} would not install, and said nothing. It stopped with {}.",
            step.name, said
        ),
    })
}

/// The one shell that can run every step on this platform.
fn shell(command: &str) -> std::process::Command {
    let mut asking = if cfg!(target_os = "windows") {
        let mut it = std::process::Command::new("cmd");
        it.arg("/C").arg(command);
        it
    } else {
        let mut it = std::process::Command::new("sh");
        it.arg("-c").arg(command);
        it
    };
    // No console window: this runs behind a screen that is already reporting it, and a terminal
    // flashing open per step is the immersion leak version of a progress bar.
    {
        use crate::models::quiet::Quiet;
        asking.quiet();
    }
    asking
}

/// Fetch one of the things no package manager has.
///
/// ## Progress, reported the way this screen already reports it
///
/// The installers hand back bytes-so-far and a total; this screen takes lines. So the bytes are
/// turned into a line and **only when the number has moved a whole percent** — a callback that
/// fires per chunk would put ten thousand lines into a log somebody is meant to read.
///
/// ## The one that is not a download
///
/// `Fetch::Forge` builds a Python environment and runs pip inside it, so it reports *steps*
/// rather than bytes. Both arrive here as the same lines, which is what the caller can draw.
fn fetch(what: &Fetch, say: &dyn Fn(&str)) -> Result<String, String> {
    let percent = std::cell::Cell::new(u64::MAX);
    let watching = |done: u64, total: Option<u64>| {
        let Some(total) = total.filter(|it| *it > 0) else {
            return;
        };
        let now = done.saturating_mul(100) / total;
        if now != percent.get() {
            percent.set(now);
            say(&format!("{now}% — {} of {}", size(done), size(total)));
        }
    };

    match what {
        Fetch::Ear => {
            use crate::hearing::{Ear, Hearing};
            // The program, then a model. **In that order and both**, because either alone is a
            // chain with a hole in it — and the second is the one that takes the time, so a
            // person watching a finished download would otherwise think it was done.
            let said = crate::hearing::install(Ear::WhisperCpp, &watching)?;
            say(&said);
            percent.set(u64::MAX);
            crate::hearing::install_model(Hearing::Base, &watching)
        }
        Fetch::Mouth => crate::speech::install(crate::speech::Voicebox::Piper, &watching),
        Fetch::Voice => voice(say, &watching),
        Fetch::Forge => crate::rvc::prepare(&|step| say(step)),
    }
}

/// One voice, chosen for the machine it is landing on.
///
/// **The locale decides and the repository confirms**, in that order. Which voices exist is a
/// question only Hugging Face can answer, so it is asked here rather than in `survey()` — a
/// first-run survey has to answer instantly and on a machine with no network, and a list of 175
/// voices is neither.
///
/// A machine whose language Piper has no voice for gets English rather than nothing: a mouth
/// that speaks the wrong language can be changed in one dropdown, and a mouth with no voice is
/// a control that does nothing.
fn voice(say: &dyn Fn(&str), watching: &dyn Fn(u64, Option<u64>)) -> Result<String, String> {
    let wanted = spoken_here();
    say(&format!("Looking for a {wanted} voice…"));

    let found = epoch_models::voices::voices_in("rhasspy/piper-voices")
        .map_err(|why| format!("Hugging Face could not be asked for a voice: {why}"))?;

    // `es_ES-davefx-medium` — the language is the part before the underscore, which is how the
    // repository itself is laid out (`es/es_ES/davefx/medium/…`).
    let mine = |it: &epoch_models::voices::Found| it.name().split('_').next() == Some(&wanted);
    let pick = found
        .iter()
        .find(|it| mine(it) && it.name().ends_with("-medium"))
        .or_else(|| found.iter().find(|it| mine(it)))
        .or_else(|| found.iter().find(|it| it.name().starts_with("en_US")))
        .or_else(|| found.first())
        .ok_or("That repository published no voices Epoch could read.")?;

    say(&format!("{} — {}", pick.name(), size(pick.bytes)));

    // **The model and its sidecar, both, in the order the catalogue lists them.** A voice whose
    // sidecar failed is a file that refuses at the moment somebody tries to speak — minutes
    // later and somewhere else — so neither part is best-effort.
    //
    // This is the narrow version of what the Workshop's own INSTALL does, and not a call to it:
    // that lives in `epoch-tauri`, which the Engine cannot reach and should not. The parts it
    // has and this does not are the parts this case does not need — no API key, because this
    // one repository is public and states so; no identification, because a voice is described
    // by the sidecar arriving beside it rather than by reading its tensors.
    use epoch_models::catalogue::Catalogue;
    let asset = epoch_models::voices::Voices::new()
        .asset(&pick.id())
        .map_err(|why| why.to_string())?
        .ok_or("that voice is no longer in the repository")?;

    let shelf =
        epoch_models::generative::Library::here().shelf(epoch_models::generative::Shelf::Voices);
    let mut landed = String::new();
    for (at, part) in asset.parts().into_iter().enumerate() {
        // Only the first reports progress: the sidecar is four kilobytes, and a second bar that
        // fills instantly is noise on a screen that is already saying what is happening.
        let quiet = |_: u64, _: Option<u64>| {};
        let got = epoch_models::catalogue::fetch(
            part,
            &shelf,
            None,
            if at == 0 { watching } else { &quiet },
        )
        .map_err(|why| why.to_string())?;
        if at == 0 {
            landed = got
                .file_name()
                .map(|it| it.to_string_lossy().into_owned())
                .unwrap_or_default();
        }
    }
    Ok(format!("{landed} is on the shelf."))
}

/// The language this machine is set to, as a two-letter code.
///
/// **Read from the system, never from a setting Epoch owns** — there is no Epoch yet when this
/// runs. `en` where it cannot be read, which is a fallback rather than a claim.
fn spoken_here() -> String {
    let raw = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .ok()
        .or_else(windows_language)
        .unwrap_or_default();
    let code: String = raw
        .chars()
        .take_while(|it| it.is_ascii_alphabetic())
        .collect();
    if code.len() == 2 {
        code.to_ascii_lowercase()
    } else {
        "en".to_owned()
    }
}

#[cfg(windows)]
fn windows_language() -> Option<String> {
    use crate::models::quiet::Quiet;
    let said = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "(Get-Culture).Name"])
        .quiet()
        .output()
        .ok()?;
    let name = String::from_utf8_lossy(&said.stdout).trim().to_owned();
    (!name.is_empty()).then_some(name)
}

#[cfg(not(windows))]
fn windows_language() -> Option<String> {
    None
}

/// A size somebody can read, at the scale the thing actually is.
fn size(bytes: u64) -> String {
    let mb = bytes as f64 / 1_000_000.0;
    if mb >= 1000.0 {
        format!("{:.1} GB", mb / 1000.0)
    } else if mb >= 10.0 {
        format!("{mb:.0} MB")
    } else {
        format!("{mb:.1} MB")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(id: &str, here: bool, how: Option<How>, needs: &[&str]) -> Offer {
        Offer {
            id: id.to_owned(),
            name: id.to_owned(),
            what: String::new(),
            found: here.then(|| "somewhere".to_owned()),
            how,
            why_not: None,
            needs: needs.iter().map(|it| (*it).to_owned()).collect(),
            bytes: None,
        }
    }

    /// The four things no package manager has are offered, and offered as fetches.
    ///
    /// **Not a list of names.** A test that asserted four ids would pass with four rows Epoch
    /// cannot install. What makes them real is that each has a route, and that the route is the
    /// only one that exists for it — which is exactly the claim `How::Epoch` was added to make.
    ///
    /// **Except where there is nothing published to fetch.** whisper.cpp ships an xcframework
    /// on macOS rather than a program, so its box is dark there and carries a reason instead of
    /// a route. That is a fifth state rather than an exception to the rule: *offered with a way
    /// in*, or *offered, dark, and explained*. What must never happen is a live button with
    /// nothing behind it, which is what this asserts on the other side of the branch.
    #[test]
    fn the_voice_chain_is_offered_and_epoch_is_the_one_who_fetches_it() {
        let all = survey();
        for id in ["whisper", "piper", "voice", "voice-forge"] {
            let one = all
                .iter()
                .find(|it| it.id == id)
                .unwrap_or_else(|| panic!("{id} is not offered at all"));
            match &one.how {
                Some(How::Epoch(_)) => {}
                Some(other) => {
                    panic!("{id} would be installed by {other:?}, and no package manager has it")
                }
                None => {
                    let why = one
                        .why_not
                        .as_deref()
                        .unwrap_or_else(|| panic!("{id} has no way in and does not say why"));
                    assert!(!why.trim().is_empty(), "{id}'s reason is blank");
                    assert!(one.bytes.is_none(), "{id} has no route and a download size");
                }
            }
        }
    }

    /// A mouth is installed before the voice it reads.
    ///
    /// The dependency machinery already existed; this holds that the voice actually uses it,
    /// because the alternative — a voice landing on a shelf no program can read — fails
    /// silently and much later.
    #[test]
    fn a_voice_waits_for_the_mouth_that_reads_it() {
        let all = vec![
            offer("piper", false, Some(How::Epoch(Fetch::Mouth)), &[]),
            offer("voice", false, Some(How::Epoch(Fetch::Voice)), &["piper"]),
        ];
        let steps = plan(&all, &["voice".to_owned()]);
        let order: Vec<&str> = steps.iter().map(|it| it.id.as_str()).collect();
        assert_eq!(order, ["piper", "voice"]);
        assert_eq!(steps[0].because.as_deref(), Some("voice"));
    }

    /// The ear’s size is the program **and** a model.
    ///
    /// Both are downloaded under one box, so a number that named only the 8.4 MB binary would
    /// understate what pressing it actually costs by a factor of about eighteen. Read from the
    /// two modules that own the figures rather than restated here.
    #[test]
    fn the_ears_size_counts_what_it_actually_downloads() {
        let Some(ear) = survey().into_iter().find(|it| it.id == "whisper") else {
            panic!("the ear is not offered");
        };

        // **Where there is nothing to fetch, there is nothing to size.** whisper.cpp ships an
        // xcframework on macOS rather than a program, so the box is dark and says so — which is
        // a state to assert rather than a case to skip.
        if crate::hearing::Ear::WhisperCpp.archive().is_none() {
            assert!(ear.bytes.is_none(), "no download, no number");
            assert!(ear.how.is_none(), "and no button that could only fail");
            let why = ear
                .why_not
                .expect("a dark box has to say which absence it is");
            assert!(why.contains("whisper.cpp"), "{why}");
            return;
        }

        let Some(said) = ear.bytes else {
            panic!("the ear is offered with no size, and it is the box with two downloads in it");
        };
        let archive = crate::hearing::Ear::WhisperCpp
            .archive()
            .map(|it| it.bytes)
            .unwrap_or_default();
        let model = crate::hearing::Hearing::Base.bytes();
        assert_eq!(said, archive + model);
        assert!(
            said > model,
            "a size that is not at least the model is wrong"
        );
    }

    /// A size somebody reads is at the scale of the thing.
    #[test]
    fn a_size_is_written_at_the_scale_it_is() {
        assert_eq!(size(8_400_000), "8.4 MB");
        assert_eq!(size(22_500_000), "22 MB");
        assert_eq!(size(867_000_000), "867 MB");
        assert_eq!(size(2_500_000_000), "2.5 GB");
    }

    /// The language is two letters, or it is `en` — never half a locale.
    ///
    /// `Get-Culture` answers `es-AR` and `LANG` answers `es_MX.UTF-8`; both are the same voice
    /// question, and a code taken without trimming would ask the repository for `es-AR`, which
    /// it does not have.
    #[test]
    fn a_locale_is_cut_to_the_language_it_names() {
        let it = spoken_here();
        assert_eq!(it.len(), 2, "{it:?} is not a language code");
        assert!(it.chars().all(|c| c.is_ascii_lowercase()), "{it:?}");
    }

    #[test]
    fn what_is_already_here_is_not_installed_again() {
        // The cold-instrument rule on a checkbox: a program that is here is a reading, not an
        // offer, and a step for it would be a progress bar counting something that never runs.
        let all = vec![offer(
            "ollama",
            true,
            Some(How::Winget("Ollama.Ollama".into())),
            &[],
        )];
        assert!(plan(&all, &["ollama".to_owned()]).is_empty());
    }

    #[test]
    fn what_something_needs_comes_first_and_is_a_step_of_its_own() {
        // Asking for Claude Code on a machine with no Node.js is asking for two installs, and a
        // list that hid one would lie about how long it is.
        let all = vec![
            offer(
                "node",
                false,
                Some(How::Winget("OpenJS.NodeJS".into())),
                &[],
            ),
            offer(
                "claude-code",
                false,
                Some(How::Npm("@anthropic-ai/claude-code".into())),
                &["node"],
            ),
        ];
        let steps = plan(&all, &["claude-code".to_owned()]);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].id, "node");
        assert_eq!(
            steps[0].because.as_deref(),
            Some("claude-code"),
            "and it says whose it is"
        );
        assert_eq!(steps[1].id, "claude-code");
        assert_eq!(steps[1].because, None);
    }

    #[test]
    fn something_needed_twice_is_one_step() {
        let all = vec![
            offer(
                "node",
                false,
                Some(How::Winget("OpenJS.NodeJS".into())),
                &[],
            ),
            offer("claude-code", false, Some(How::Npm("a".into())), &["node"]),
            offer("gemini-cli", false, Some(How::Npm("b".into())), &["node"]),
        ];
        let steps = plan(&all, &["claude-code".to_owned(), "gemini-cli".to_owned()]);
        assert_eq!(steps.len(), 3, "{steps:?}");
        assert_eq!(steps.iter().filter(|it| it.id == "node").count(), 1);
    }

    #[test]
    fn what_epoch_cannot_install_is_never_a_step() {
        // It is said on the row instead, where somebody can act on it. A step that cannot run is
        // the dead control this codebase has already deleted once.
        let all = vec![offer("package-manager", false, None, &[])];
        assert!(plan(&all, &["package-manager".to_owned()]).is_empty());
    }

    #[test]
    fn every_agent_this_screen_offers_is_one_the_engine_knows() {
        // Written as `"gemini-cli"` from the npm package name, while the program calls itself
        // `gemini` - so the row read *not installed* about an agent the Launcher was showing as
        // READY two panels away. This is the assertion that would have caught it.
        let known: Vec<String> = crate::agents::installed()
            .survey()
            .iter()
            .map(|one| one.kind.clone())
            .collect();
        for id in ["claude-code", "codex", "gemini"] {
            assert!(
                known.iter().any(|it| it == id),
                "{id} is not a program the Engine knows: {known:?}"
            );
        }
        for offered in survey()
            .iter()
            .filter(|it| it.needs.contains(&"node".to_owned()))
        {
            assert!(
                known.contains(&offered.id),
                "{} is offered and the Engine has never heard of it",
                offered.id
            );
        }
    }

    #[test]
    fn a_command_answers_no_prompt_nobody_can_see() {
        // This runs with no terminal for somebody to answer in, so winget must not stop to ask.
        let said = How::Winget("Ollama.Ollama".into()).command();
        assert!(said.contains("--accept-package-agreements"), "{said}");
        assert!(said.contains("--accept-source-agreements"), "{said}");
        assert!(
            said.contains("--exact"),
            "an id is exact or it is a search: {said}"
        );
    }

    #[test]
    fn a_step_reports_every_line_and_the_time_it_took() {
        // Through a real shell, because that is the path every step takes and a runner tested
        // against a fake one proves the wiring and nothing else (11.18).
        let seen = std::sync::Mutex::new(Vec::<String>::new());
        let step = Step {
            id: "echo".to_owned(),
            name: "a step".to_owned(),
            because: None,
            command: "echo one && echo two".to_owned(),
            how: How::Winget("nothing".to_owned()),
        };
        let took = run(&step, &|line| {
            if let Ok(mut held) = seen.lock() {
                held.push(line.to_owned());
            }
        });
        assert!(took.is_ok(), "{took:?}");
        let seen = seen.into_inner().unwrap_or_default();
        assert!(
            seen.iter().any(|it| it.contains("one")) && seen.iter().any(|it| it.contains("two")),
            "both lines arrived: {seen:?}"
        );
    }

    #[test]
    fn a_failure_carries_the_programs_own_words() {
        // It knows why far better than a sentence written here, and rewriting somebody else's
        // refusal is a mistake this codebase has already paid for.
        let step = Step {
            id: "nope".to_owned(),
            name: "a step".to_owned(),
            because: None,
            // A program that does not exist: the shell's own complaint is the only true account.
            command: "epoch-a-program-that-does-not-exist --please".to_owned(),
            how: How::Winget("nothing".to_owned()),
        };
        let said = run(&step, &|_| {}).expect_err("it cannot have worked");
        assert!(said.contains("a step"), "{said}");
        assert!(
            said.len() > "a step would not install: ".len(),
            "and it says something beyond the name: {said}"
        );
    }

    #[test]
    fn an_install_command_is_read_from_where_it_already_lives() {
        // One fact written twice disagrees the first time one of them changes, so the runtimes'
        // own commands are parsed rather than re-typed here.
        assert_eq!(
            package("winget install Ollama.Ollama"),
            Some(How::Winget("Ollama.Ollama".to_owned()))
        );
        assert_eq!(
            package("brew install --cask lm-studio"),
            Some(How::Brew("--cask lm-studio".to_owned()))
        );
        // A platform this was not written for says nothing rather than guessing a command.
        assert_eq!(package("apt-get install ollama"), None);
    }
}
