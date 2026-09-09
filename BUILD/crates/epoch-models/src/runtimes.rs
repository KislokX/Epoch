//! The other things that run a model on a machine: llama.cpp and LM Studio.
//!
//! ## Why they need no Provider of their own
//!
//! Both serve an **OpenAI-compatible API**, and Epoch has spoken that since Phase 8 — the whole
//! reason `Kind::OpenAi` exists is that adding a backend should be a form rather than a pull
//! request. So this file measures; nothing here talks to a model.
//!
//! ## Availability is three facts, not one
//!
//! **Installed** · **serving** · **where**. They have three different answers — install it,
//! start it, or nothing at all — and the same discipline the agents' readiness follows keeps
//! them apart (ADR-0027). A machine with LM Studio installed and its server switched off is not
//! a machine without LM Studio, and telling somebody to install what they already have is the
//! failure this shape exists to prevent.
//!
//! ## Serving is measured by asking, never by finding a binary
//!
//! A runtime is usable when its API answers. That is one HTTP request against
//! `/v1/models`, it is true regardless of how the thing was installed, and it cannot go stale
//! the way a version string can. Finding the binary answers the *other* question — whether
//! there is something to start.
//!
//! ## Every command here was measured, on 2026-08-20
//!
//! ```text
//! winget search "LM Studio" --source winget  → ElementLabs.LMStudio  0.4.21+2
//! winget search "llama.cpp" --source winget  → ggml.llamacpp         b10507
//! brew info --cask lm-studio                 → lm-studio             0.4.21,2
//! brew info llama.cpp                        → llama.cpp             10360
//! ```
//!
//! Nothing is guessed. `winget search --id llama.cpp` finds nothing at all — the id is
//! `ggml.llamacpp` — and a plausible-looking command that does not exist is worse than no
//! command, because somebody pastes it and blames themselves.

use serde::Serialize;

/// A runtime this machine could be asked to think with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    /// The one Epoch has spoken since the first day. Here because *starting* it is the same
    /// question as starting the other two, and a panel that could start two of three would be
    /// answering it in two places.
    Ollama,
    /// `llama-server`, and the `llama` CLI that wraps it. Takes a GGUF from disk, or fetches one
    /// itself with `-hf`.
    LlamaCpp,
    /// A desktop application with a server inside it, driven by the `lms` CLI.
    LmStudio,
}

impl Runtime {
    pub const ALL: [Runtime; 3] = [Runtime::Ollama, Runtime::LlamaCpp, Runtime::LmStudio];

    pub const fn id(self) -> &'static str {
        match self {
            Runtime::Ollama => "ollama",
            Runtime::LlamaCpp => "llama_cpp",
            Runtime::LmStudio => "lm_studio",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Runtime::Ollama => "Ollama",
            Runtime::LlamaCpp => "llama.cpp",
            Runtime::LmStudio => "LM Studio",
        }
    }

    /// Where its server listens when nobody has moved it.
    ///
    /// LM Studio's is fixed by the application. llama.cpp's is `llama-server`'s own default, and
    /// a person who chose another port adds a backend by hand — which is what backends are for.
    pub const fn usual_port(self) -> u16 {
        match self {
            Runtime::Ollama => 11434,
            Runtime::LlamaCpp => 8080,
            Runtime::LmStudio => 1234,
        }
    }

    /// What counts as this runtime being here, in the order worth trying.
    ///
    /// **Measured after installing both, because looking for a CLI on the PATH found neither.**
    ///
    /// `winget install ggml.llamacpp` puts `llama.exe` and `llama-server.exe` inside
    /// `%LOCALAPPDATA%\Microsoft\WinGet\Packages\ggml.llamacpp_…\`, which is not on the PATH
    /// and is not shimmed into WinGet's `Links` folder either — only four unrelated programs
    /// were. So the package directories are searched by name.
    ///
    /// And LM Studio's `lms` **does not exist after installing**: the application creates
    /// `~/.lmstudio/bin` the first time it is run. Installed is a fact about the application, so
    /// the application is what is looked for; the CLI is a bonus that arrives later.
    const fn binaries(self) -> &'static [&'static str] {
        match self {
            // `llama` is the wrapper; `llama-server` is the server itself, and a build from
            // source leaves only that.
            Runtime::Ollama => &["ollama"],
            Runtime::LlamaCpp => &["llama", "llama-server", "llama-cli"],
            Runtime::LmStudio => &["lms", "LM Studio"],
        }
    }

    /// The folder an installer that behaves puts it in, under `Programs` or `/Applications`.
    ///
    /// LM Studio is the only runtime that installs that way; the other two are a service and a
    /// pile of executables in a package directory.
    pub(crate) const fn program_folder(self) -> Option<&'static str> {
        match self {
            Runtime::Ollama | Runtime::LlamaCpp => None,
            Runtime::LmStudio => Some("LM Studio"),
        }
    }

    /// The winget package whose directory holds it, when one does.
    pub(crate) const fn winget_package(self) -> Option<&'static str> {
        match self {
            Runtime::Ollama => None,
            Runtime::LlamaCpp => Some("ggml.llamacpp"),
            // LM Studio installs itself properly, into Programs. There is no package directory
            // to rummage in.
            Runtime::LmStudio => None,
        }
    }

    /// How somebody installs it on this platform, in their own terminal.
    ///
    /// Measured against the package managers themselves rather than remembered: `winget search
    /// --id llama.cpp` finds nothing, because the id is `ggml.llamacpp`. A plausible command
    /// that does not exist is worse than none — somebody pastes it and blames themselves.
    pub fn install_command(self) -> &'static str {
        match (self, cfg!(windows)) {
            (Runtime::Ollama, true) => "winget install Ollama.Ollama",
            (Runtime::Ollama, false) => "brew install --cask ollama",
            (Runtime::LlamaCpp, true) => "winget install ggml.llamacpp",
            (Runtime::LmStudio, true) => "winget install ElementLabs.LMStudio",
            (Runtime::LlamaCpp, false) => "brew install llama.cpp",
            (Runtime::LmStudio, false) => "brew install --cask lm-studio",
        }
    }
}

/// What one runtime is on this machine, right now.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Available {
    pub id: &'static str,
    pub name: &'static str,
    /// A binary was found. `false` is *not here*, and the fix is [`Available::install`].
    pub installed: bool,
    /// Where it was found, so "not installed" is a fact somebody can go and check.
    pub found_at: Option<String>,
    /// Its API answered. **The fact that actually matters** — a runtime is usable when it
    /// answers, however it got there.
    pub serving: bool,
    /// The address that answered, or the one that was tried.
    pub endpoint: String,
    /// Models it **offers**. Empty is honest: LM Studio serves with nothing loaded.
    ///
    /// Offered, not held. This field was shown on the deck as *"holding …"*, which is a reading
    /// nothing measured — `/v1/models` lists a shelf, and a shelf of five models was reported as
    /// five models in memory. [`Available::resident`] is the one that answers that.
    pub models: Vec<String>,
    /// Models actually **in memory right now**, measured.
    ///
    /// ## Why this is a separate question
    ///
    /// A user watching a 7.5 GB `llama-server.exe` in Task Manager and a deck reading *"holding
    /// gemma4-12b, gemma4-26b, gpt-oss-20b, qwen3-14b, qwen3.8"* has been told nothing true
    /// about their graphics card. This is the gauge that was missing, and every runtime answers
    /// it — none of them was ever asked.
    ///
    /// Each spells it its own way, and all three were asked rather than remembered:
    /// - Ollama: `/api/ps`, a list of exactly what is resident.
    /// - llama.cpp: `/v1/models`, `status.value == "loaded"`. **`sleeping` is not holding** —
    ///   measured across an idle release, RAM 8812 MB -> 126 MB and VRAM 10736 MiB -> 1937 MiB.
    /// - LM Studio: its own `/api/v0/models`, `state == "loaded"`.
    ///
    /// Empty means nothing is loaded, which is the ordinary state of a server between turns.
    pub resident: Vec<String>,
    /// The command that would install it here.
    pub install: &'static str,
    /// The command that would start its server, when the program is on this machine.
    ///
    /// `None` means there is nothing here to start — which is a different sentence from *it is
    /// not running*, and the surface says the right one because it has both facts.
    pub start: Option<String>,
    /// What this runtime says it can put a model on — **its own words**, once.
    ///
    /// Empty means it was never asked, which is every runtime but llama.cpp: `--list-devices` is
    /// a question llama.cpp answers and the other two have no equivalent for. A guess in this
    /// field would be worse than the blank.
    pub devices: Vec<String>,
    /// The one thing this machine could be doing faster, when the program's own answer says so.
    ///
    /// `None` is the ordinary state, and it is not padding — see [`slower_than_it_could_be`] for
    /// the measurement that made this a field rather than a comment.
    pub handicap: Option<String>,
    /// Models this runtime keeps in **its own** cache, whether or not it is serving.
    ///
    /// llama.cpp only: `--cache-list` is a question it answers and the other two have no
    /// equivalent for. Empty is *none cached* for llama.cpp and *never asked* for the others,
    /// which is why nothing here reads it as a count of what a machine has.
    pub cached: Vec<String>,
}

/// The last survey, and when it was taken.
///
/// **Because a survey is not free and was being paid for like it was.** Measured 2026-08-21:
/// `survey()` costs **624 ms** — a walk of the installer directories plus a TCP probe each — and
/// `Backends::load` calls it, and `load` is called whenever anything asks what this machine
/// offers. Opening a deck paid it several times over.
///
/// A runtime does not start or stop inside a second, so a reading from a second ago is the same
/// reading. Anything that wants the truth *now* — the ASK AGAIN button — calls [`survey`], which
/// always measures.
static LAST: std::sync::Mutex<Option<(std::time::Instant, Vec<Available>)>> =
    std::sync::Mutex::new(None);

/// How long a survey stays worth reusing.
///
/// Short enough that pressing a button and looking at the panel never disagree; long enough that
/// one screen opening does not measure the same three programs four times.
const FRESH_FOR: std::time::Duration = std::time::Duration::from_secs(3);

/// Ask this machine about its runtimes, reusing a very recent answer.
///
/// For everything on a path somebody is waiting on. [`survey`] is for the button that means
/// *ask again*, and it is the only thing that should ever bypass this.
pub fn survey_cached() -> Vec<Available> {
    if let Ok(held) = LAST.lock() {
        if let Some((taken, found)) = held.as_ref() {
            if taken.elapsed() < FRESH_FOR {
                return found.clone();
            }
        }
    }
    survey()
}

/// Ask this machine about both runtimes.
///
/// Cheap enough to call when a screen opens: two loopback requests with a short timeout, and a
/// PATH walk. Nothing is started and nothing is downloaded.
pub fn survey() -> Vec<Available> {
    let found: Vec<Available> = Runtime::ALL.into_iter().map(look_for).collect();
    // Remembered for whoever asks next, so a measurement taken for one panel is not taken again
    // for the one beside it.
    if let Ok(mut held) = LAST.lock() {
        *held = Some((std::time::Instant::now(), found.clone()));
    }
    found
}

/// One runtime, measured.
pub fn look_for(runtime: Runtime) -> Available {
    let endpoint = format!("http://127.0.0.1:{}", runtime.usual_port());
    let found = runtime
        .binaries()
        .iter()
        .find_map(|name| found_in(name, runtime.winget_package(), runtime.program_folder()));
    let models = models_at(&endpoint);

    Available {
        id: runtime.id(),
        name: runtime.name(),
        installed: found.is_some(),
        found_at: found.map(|p| p.display().to_string()),
        serving: models.is_some(),
        // Asked only of a server that answered. A second probe against a closed port would
        // undo the TCP-connect deadline `models_at` exists to protect.
        resident: if models.is_some() {
            resident_at(&endpoint, runtime)
        } else {
            Vec::new()
        },
        endpoint,
        models: models.unwrap_or_default(),
        install: runtime.install_command(),
        start: start_command(runtime),
        devices: devices_of(runtime),
        cached: if runtime == Runtime::LlamaCpp {
            cached_by_llama_cpp()
        } else {
            Vec::new()
        },
        handicap: slower_than_it_could_be(&devices_of(runtime)),
    }
}

/// What a runtime says it can offload to, asked once per run of Epoch.
///
/// **Asked, not sniffed.** The obvious version of this reads the DLLs beside the binary —
/// `ggml-cuda.dll` present, therefore CUDA — and that is a guess about somebody else's packaging.
/// `llama-server --list-devices` is the program answering for itself:
///
/// ```text
/// Available devices:
///   Vulkan0: NVIDIA GeForce RTX 4070 SUPER (11997 MiB, 3555 MiB free)
/// ```
///
/// Only llama.cpp offers it. Ollama and LM Studio have no equivalent, so they answer nothing —
/// unasked rather than "no devices", which would read as a machine with no graphics card.
///
/// Cached for the lifetime of the process because it costs starting the program, and because a
/// different build of llama.cpp arrives by installing one, not while a window is open.
/// What llama.cpp has in **its own** model cache.
///
/// ## Why Epoch has to ask
///
/// Reported from the AMD machine: a model downloaded, `llama serve` in a terminal reporting
/// `Available models (1)`, and Epoch's own deck reading `ON THIS MACHINE — nothing yet`. The
/// model was real and Epoch could not see it, because `everything_here` looks at Ollama's store
/// and Epoch's own shelf and nothing else — and `llama download -hf` files it in llama.cpp's
/// cache, which is neither.
///
/// **Listed, never adopted** (ADR-0032). Nothing here moves, copies or renames a file: it asks a
/// program what it holds, exactly as the Ollama shelf already does.
///
/// The format is measured rather than remembered — one model was fetched on the machine this was
/// written on purely to read it:
///
/// ```text
/// number of models in cache: 1
///    1. ggml-org/gemma-3-270m-GGUF:Q8_0
/// ```
///
/// Asked once: it spawns a process, and a deck that re-reads it on every paint would pay for a
/// process launch to answer a question whose answer changes when somebody downloads something.
pub fn cached_by_llama_cpp() -> Vec<String> {
    static ASKED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ASKED
        .get_or_init(|| {
            let Some(program) = found_in("llama-server", Runtime::LlamaCpp.winget_package(), None)
            else {
                return Vec::new();
            };
            use crate::quiet::Quiet;
            let asked = std::process::Command::new(program)
                .arg("--cache-list")
                .quiet()
                .output();
            let Ok(said) = asked else {
                return Vec::new();
            };
            let mut all = String::from_utf8_lossy(&said.stdout).into_owned();
            all.push_str(&String::from_utf8_lossy(&said.stderr));
            all.lines()
                .map(str::trim)
                // `   1. ggml-org/gemma-3-270m-GGUF:Q8_0` — numbered, and the count line above
                // it has no dot after a number.
                .filter_map(|line| {
                    let (number, rest) = line.split_once('.')?;
                    number.trim().parse::<u32>().ok()?;
                    let name = rest.trim();
                    (!name.is_empty()).then(|| name.to_owned())
                })
                .collect()
        })
        .clone()
}

/// Which GPU backend a build carries, in the program's own words.
///
/// ## Why it belongs on a fingerprint
///
/// **One build carries one GPU backend** and there is nothing to switch at runtime: the CUDA
/// archive holds `ggml-cuda.dll` and no Vulkan, the winget archive holds `ggml-vulkan.dll` and no
/// CUDA. On this machine those two answered **4.5× apart with the same GGUF on the same card** —
/// so *the same model on the same hardware* can honestly produce two numbers that look exactly
/// like a degraded machine to anything comparing them.
///
/// Read from `--list-devices`, which prints `Vulkan0: NVIDIA GeForce RTX 4070 SUPER` or
/// `CUDA0: …` — the prefix before the index is the answer. A device line Epoch cannot parse
/// gives `None`, which reads as *unrecorded*: a backend guessed from a build string would be a
/// name invented for a fact that decides whether two benchmarks are comparable.
pub fn backend_of(devices: &[String]) -> Option<String> {
    let first = devices.first()?;
    let name = first.split(':').next()?.trim();
    let cut = name.trim_end_matches(|c: char| c.is_ascii_digit());
    (!cut.is_empty() && cut != name).then(|| cut.to_owned())
}

/// The backend llama.cpp is built with here. Asked once, like the devices it reads from.
pub fn llama_backend() -> Option<String> {
    backend_of(&devices_of(Runtime::LlamaCpp))
}

pub fn devices_of(runtime: Runtime) -> Vec<String> {
    if runtime != Runtime::LlamaCpp {
        return Vec::new();
    }
    static ASKED: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    ASKED
        .get_or_init(|| {
            let Some(program) = found_in("llama-server", runtime.winget_package(), None) else {
                return Vec::new();
            };
            let Ok(said) = std::process::Command::new(program)
                .arg("--list-devices")
                .output()
            else {
                return Vec::new();
            };
            let mut all = String::from_utf8_lossy(&said.stdout).into_owned();
            all.push_str(&String::from_utf8_lossy(&said.stderr));
            all.lines()
                .map(str::trim)
                .filter(|line| line.contains(':') && !line.starts_with("Available"))
                .map(str::to_owned)
                .collect()
        })
        .clone()
}

#[cfg(test)]
mod which_backend {
    #[test]
    fn a_build_says_which_backend_it_carries_and_epoch_never_guesses() {
        /*
            The 4.5x on this machine: the same GGUF, the same card, two archives. Anything
            comparing across them without knowing which is which reports a package difference as
            a degraded machine.
        */
        use super::backend_of;
        assert_eq!(
            backend_of(&["Vulkan0: NVIDIA GeForce RTX 4070 SUPER (12282 MiB)".to_owned()]),
            Some("Vulkan".to_owned()),
        );
        assert_eq!(
            backend_of(&["CUDA0: NVIDIA GeForce RTX 4070 SUPER (12282 MiB)".to_owned()]),
            Some("CUDA".to_owned()),
        );
        assert_eq!(
            backend_of(&["ROCm0: AMD Radeon RX 7900 XTX".to_owned()]),
            Some("ROCm".to_owned()),
        );
        // Nothing asked, and a line with no index to strip: unrecorded, never a guess.
        assert_eq!(backend_of(&[]), None);
        assert_eq!(backend_of(&["something: else".to_owned()]), None);
    }
}

#[cfg(test)]
mod consoles_epoch_opened {
    /// **The marker has to be inside the shell, or nothing afterwards can find the window.**
    ///
    /// `start "Epoch - ComfyUI"` names the console and exits; what survives is the inner `cmd`,
    /// and its command line is the only thing a later `Get-CimInstance` can match on. So the
    /// title is set again by the shell itself, which puts [`super::OURS`] where it can be read.
    #[test]
    fn the_title_reaches_the_shell_that_survives() {
        let title = format!("{}ComfyUI", super::OURS);
        let command = "\"C:\\a\\python.exe\" main.py --port 8188";
        let line = format!("/c start \"{title}\" cmd /k \"title {title}&&{command}\"");

        // What `start` names, and what the surviving shell will carry.
        assert!(line.contains(&format!("start \"{title}\"")));
        let inner = line.split("cmd /k ").nth(1).expect("an inner shell");
        assert!(
            inner.contains(super::OURS),
            "the console Epoch opened must be findable afterwards: {inner}"
        );
        assert!(
            inner.contains("main.py"),
            "and it still runs the command: {inner}"
        );
    }
}

#[cfg(test)]
mod which_backend_this_build_uses {
    use super::slower_than_it_could_be;

    /// Every vendor gets *its own* faster path, and one that has none is left alone.
    ///
    /// The first version of this function knew only "Vulkan on NVIDIA" and would have told a
    /// Radeon owner to install CUDA — advice for hardware that cannot run it. That is the
    /// invented gauge arriving as a recommendation, which is worse than the blank it replaced.
    #[test]
    fn the_faster_build_named_is_the_one_that_exists_for_that_card() {
        let of = |device: &str| slower_than_it_could_be(&[device.to_owned()]);

        assert!(of("Vulkan0: NVIDIA GeForce RTX 4070 SUPER (11997 MiB)")
            .expect("nvidia")
            .contains("CUDA"));
        assert!(of("Vulkan0: AMD Radeon RX 9600 XT (16384 MiB)")
            .expect("amd")
            .contains("ROCm"));
        assert!(of("Vulkan0: Intel(R) Arc(TM) B60 Graphics (12288 MiB)")
            .expect("intel")
            .contains("SYCL"));

        // **Never CUDA for somebody else's silicon.** This is the exact row a settings panel
        // offering a menu of backends would have produced.
        for card in [
            "Vulkan0: AMD Radeon RX 9600 XT",
            "Vulkan0: Intel(R) Arc(TM) B60",
        ] {
            assert!(!of(card).expect("a card").contains("CUDA"), "{card}");
        }
    }

    /// A card already on its native backend has nothing to be told.
    #[test]
    fn a_build_already_using_the_card_properly_says_nothing() {
        assert_eq!(
            slower_than_it_could_be(&["CUDA0: NVIDIA GeForce RTX 4070 SUPER (12281 MiB)".into()]),
            None
        );
        assert_eq!(
            slower_than_it_could_be(&["ROCm0: AMD Radeon RX 9600 XT".into()]),
            None
        );
        // And a machine nothing was asked about stays blank rather than guessing.
        assert_eq!(slower_than_it_could_be(&[]), None);
    }

    /// A vendor Epoch cannot name gets no advice, because it has no archive to point at.
    #[test]
    fn an_unrecognised_card_is_left_alone() {
        assert_eq!(
            slower_than_it_could_be(&["Vulkan0: Some Accelerator Nobody Has Heard Of".into()]),
            None
        );
    }
}

/// The one sentence worth saying about a runtime that is working, and working slowly.
///
/// ## Measured 2026-08-25, on this machine, and it is a 3× difference
///
/// The same GGUF, the same card, the same engine — and 200 tokens took **36.7 s through Epoch's
/// llama.cpp (5.4 tok/s)** against **11.3 s through LM Studio (17.7 tok/s)**. LM Studio *is*
/// llama.cpp underneath, so the model, the quantisation and the hardware were all identical.
///
/// The difference is the backend, and both programs say which one they use when asked:
/// `llama-server --list-devices` reports `Vulkan0: NVIDIA GeForce RTX 4070 SUPER`, and
/// `lms runtime ls` reports `llama.cpp-win-x86_64-nvidia-cuda12-avx2` selected. The winget
/// package the deck installs (`ggml.llamacpp`) ships CPU and Vulkan backends and no CUDA one —
/// checked: winget has exactly one `llama.cpp` package, so there is no better command to offer.
///
/// **So this is not a bug Epoch can fix, and that is exactly why it has to be said.** A user who
/// installed llama.cpp from the deck is getting a third of their card with nothing on screen to
/// explain it, and will conclude the slow one is Epoch. A true fact the product declines to
/// mention becomes the user's bug to explain.
///
/// Confirmed on this machine 2026-08-25: the CUDA build took the same 200 tokens in **8.1 s**
/// against Vulkan's 36.7 s — 4.5×, and faster than LM Studio.
///
/// ## Vulkan is not the wrong answer; it is the wrong answer *for this card*
///
/// The first version of this only knew "Vulkan on an NVIDIA card". That would have told somebody
/// with a Radeon to install CUDA, which does not exist for their hardware — the invented gauge,
/// arriving as advice. So the rule is stated the way it is actually true: **a build whose backend
/// is not the card vendor's native one is leaving speed on the table**, and which one is native
/// depends on who made the card.
///
/// Measured from the release itself rather than remembered — `llama.cpp` publishes seven Windows
/// backends and these are their exact names:
///
/// ```text
/// cpu · cuda · opencl · openvino · rocm · sycl · vulkan
/// ```
///
/// Deliberately no number in the sentence: 4.5× was measured on one card, and a figure quoted
/// for somebody else's machine is the invented gauge again. The fact and the fix, nothing more.
pub fn slower_than_it_could_be(devices: &[String]) -> Option<String> {
    // Vulkan runs on everything, which is why it is what a general build ships and why it is
    // the only backend that can be "the slow one" for a card that has a native path.
    let generic = devices.iter().find(|d| d.starts_with("Vulkan"))?;
    let said = generic.to_lowercase();

    // Whose card it is, in the words the program printed. Anything unrecognised gets nothing:
    // a vendor Epoch cannot name is one it has no faster build to recommend, and guessing would
    // send somebody to download the wrong archive.
    let (vendor, native) = if said.contains("nvidia") || said.contains("geforce") {
        ("NVIDIA", "CUDA")
    } else if said.contains("amd") || said.contains("radeon") {
        ("AMD", "ROCm")
    } else if said.contains("intel") || said.contains("arc") {
        ("Intel", "SYCL")
    } else {
        return None;
    };

    Some(format!(
        "This build runs your {vendor} card through Vulkan. A {native} build of llama.cpp is \
         faster on the same card — it is not on winget, so it comes from llama.cpp's own \
         releases. Unpack it into ~/.llama/bin and Epoch will prefer it."
    ))
}

/// What an OpenAI-compatible server says it is holding.
///
/// `None` means it did not answer at all, which is a different fact from answering with nothing:
/// LM Studio serves happily with no model loaded, and reporting that as "offline" would send
/// somebody to start something that is already running.
fn models_at(endpoint: &str) -> Option<Vec<String>> {
    /*
        **The door before the question.**

        Measured: `ureq`'s `timeout` is the *read* timeout, and a closed port on Windows is not
        always refused — a filtered one lets the SYN retries run their course. One probe against
        a port with nothing behind it took **21 seconds**, so a survey of two runtimes cost
        forty, on a screen that opens.

        A TCP connect with its own short deadline answers the same question honestly and
        instantly. Nothing listening is the overwhelmingly common case for a machine that runs
        models through Ollama alone, and it must cost nothing to find out.
    */
    let address: std::net::SocketAddr = endpoint.trim_start_matches("http://").parse().ok()?;
    std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(300)).ok()?;

    #[derive(serde::Deserialize)]
    struct Listed {
        id: String,
    }
    #[derive(serde::Deserialize)]
    struct Models {
        #[serde(default)]
        data: Vec<Listed>,
    }

    let said: Models = ureq::builder()
        .timeout_connect(std::time::Duration::from_millis(300))
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .get(&format!("{endpoint}/v1/models"))
        .call()
        .ok()?
        .into_json()
        .ok()?;
    Some(said.data.into_iter().map(|m| m.id).collect())
}

/// What this server is holding in memory **right now**.
///
/// Three servers, three spellings, all measured on 2026-08-25 against the running programs:
///
/// ```text
/// Ollama     GET /api/ps          -> {"models":[{"name":"gemma4:12b", ...}]}
/// llama.cpp  GET /v1/models       -> data[].status.value  in loaded | sleeping | unloaded
/// LM Studio  GET /api/v0/models   -> data[].state         in loaded | not-loaded
/// ```
///
/// **`sleeping` is not resident.** llama.cpp's `--sleep-idle-seconds` puts a model to sleep and
/// gives the memory back: across one such release the process went from 8812 MB to 126 MB and
/// the card from 10736 MiB to 1937 MiB. Counting it would report a card as busy when it is free.
///
/// Best-effort throughout: a server that will not answer this question is reported as holding
/// nothing rather than as an error, because the row it feeds is an instrument and an instrument
/// that cannot read says nothing.
fn llama_cpp_loaded(body: &serde_json::Value) -> Vec<String> {
    // Whether *this* server states residency at all. Asked of the whole list rather than of each
    // entry: a build either publishes the field or it does not, and one entry missing it inside a
    // build that has it would be a shape nobody has seen.
    let states = body["data"]
        .as_array()
        .is_some_and(|all| all.iter().any(|m| !m["status"]["value"].is_null()));
    body["data"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter(|m| {
                    if states {
                        m["status"]["value"].as_str() == Some("loaded")
                    } else {
                        true
                    }
                })
                .filter_map(|m| m["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

fn resident_at(endpoint: &str, runtime: Runtime) -> Vec<String> {
    let ask = |path: &str| -> Option<serde_json::Value> {
        ureq::builder()
            .timeout_connect(std::time::Duration::from_millis(300))
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .get(&format!("{endpoint}{path}"))
            .call()
            .ok()?
            .into_json()
            .ok()
    };
    let named = |body: &serde_json::Value,
                 list: &str,
                 name: &str,
                 keep: &dyn Fn(&serde_json::Value) -> bool| {
        body[list]
            .as_array()
            .map(|models| {
                models
                    .iter()
                    .filter(|m| keep(m))
                    .filter_map(|m| m[name].as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };

    match runtime {
        // Ollama answers this and only this: everything in `/api/ps` is loaded.
        Runtime::Ollama => ask("/api/ps")
            .map(|body| named(&body, "models", "name", &|_| true))
            .unwrap_or_default(),
        // **llama.cpp is two programs wearing one name, and only one of them says `status`.**
        //
        // Measured on 2026-09-08, both live in the same minute:
        //
        // ```text
        // Windows, the router (`llama-server` with a model directory)
        //   data[] -> {"id": "Ministral-…", "status": {"value": "unloaded", …}}
        // macOS, one model (`llama-server -m file.gguf`, Homebrew)
        //   data[] -> {"id": "gemma4-12b", …, "meta": {…}}   and no `status` at all
        // ```
        //
        // The rule was written against the first and applied to the second, where an absent
        // field is not `loaded` — so every model on a single-model server read as **not loaded**
        // while it was answering turns. The KEEP lamp said *not loaded* about a model that had
        // just replied in 51 s off a warm cache, which is the one thing a lamp may not do.
        //
        // So the shape of the answer decides which question can be asked. A server that states
        // residency is believed, including when it says `sleeping` — that is memory genuinely
        // given back, measured across one idle release at 8812 MB -> 126 MB. A server that
        // states nothing has one model, loaded before it answers anything at all; listing it is
        // reading what the server said rather than guessing past it.
        Runtime::LlamaCpp => ask("/v1/models")
            .map(|body| llama_cpp_loaded(&body))
            .unwrap_or_default(),
        Runtime::LmStudio => ask("/api/v0/models")
            .map(|body| {
                named(&body, "data", "id", &|m| {
                    m["state"].as_str() == Some("loaded")
                })
            })
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod room {
    /// **The window it was given, never the one it could take.** Both numbers sit in the same
    /// object and only one of them is what a turn has to fit inside; taking the other is what
    /// sent an 8.6k turn into an 8192 window and brought a 502 back across the Bridge.
    #[test]
    fn a_held_window_is_read_and_a_ceiling_is_not() {
        let listing = serde_json::json!({"data": [
            {"id": "gemma4-12b", "loaded_context_length": 8_192, "max_context_length": 262_144}
        ]});
        assert_eq!(super::held_window(&listing, "gemma4-12b"), Some(8_192));

        // Listed but not loaded: unknown, never the ceiling.
        let cold = serde_json::json!({"data": [
            {"id": "gemma4-12b", "state": "not-loaded", "max_context_length": 262_144}
        ]});
        assert_eq!(super::held_window(&cold, "gemma4-12b"), None);
        assert_eq!(super::held_window(&listing, "not-here"), None);
    }

    /// A load runs a program on this computer, so it may only ever be aimed at this computer.
    #[test]
    fn only_this_machine_is_ever_loaded_into() {
        assert!(super::is_this_machine("http://127.0.0.1:1234"));
        assert!(super::is_this_machine("http://localhost:1234/"));
        assert!(!super::is_this_machine("http://192.168.1.20:1234"));
        assert!(!super::is_this_machine("https://api.example.com"));
    }
}

/// Make sure an OpenAI-compatible server is holding a window this turn fits inside.
///
/// ## Why a request cannot do it
///
/// **Measured, four spellings.** `nothing`, `context_length`, `num_ctx` and `max_context_length`
/// in the chat body all answered `200` and all left LM Studio loaded at its own default of 8192
/// — that server's habit of answering 200 to a field it ignores. `lms load <model> -c <n>` was
/// then verified to give exactly what it was asked for. So it is a load-time decision, like
/// llama.cpp's `-c` at router start, and the only door to it is that server's own CLI.
///
/// ## What it prevents
///
/// An 8.6k-token turn into an 8192-token window: the same `gemma4-12b` that answered correctly
/// on two other backends had one reply cut mid-sentence and answered a greeting to a question
/// about files. Across a Bridge it is louder — the server refuses outright and the turn comes
/// back **502**.
///
/// ## Why it lives here
///
/// Both surfaces need it and only one of them can ever run it: `lms` is on the machine holding
/// the weights, so a Host reaching across a network cannot load anything. The Bridge is the
/// program standing on that machine. One implementation, called from whichever side is local.
///
/// **LM Studio only, and derived rather than declared**: `/api/v0/models` is that server's own
/// route and llama.cpp does not serve it, so a listing that answers is the one program this can
/// act on. llama.cpp is told at router start and needs nothing here.
///
/// ## What it answers
///
/// `Ok(())` when the window is big enough, whether this had to do anything or not. `Err` carries
/// **the CLI's own words** about why it could not be — measured, that sentence is worth relaying:
/// *"this model requires approximately 9.61 GB of memory, and continuing to load it would likely
/// overload your system"* is something a person can act on, and the 502 the turn came back with
/// afterwards is not.
///
/// The caller decides what to do with a failure. On the Host a smaller window merely degrades an
/// answer, so the turn goes ahead; across a Bridge the server refuses outright, so the Bridge
/// says this instead of letting a 502 stand in for it.
pub fn make_room(endpoint: &str, model: &str, needed: u32) -> Result<(), String> {
    if !is_this_machine(endpoint) {
        return Ok(());
    }
    let listing: serde_json::Value = match ureq::builder()
        .timeout_connect(std::time::Duration::from_millis(400))
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .get(&format!("{endpoint}/api/v0/models"))
        .call()
        .ok()
        .and_then(|answer| answer.into_json().ok())
    {
        Some(listing) => listing,
        // Not LM Studio, or not answering. Nothing to do and nothing to report: llama.cpp is
        // told at router start and every other server is somebody else's.
        None => return Ok(()),
    };
    if held_window(&listing, model).is_some_and(|held| held >= needed) {
        return Ok(());
    }
    let Some(lms) = found_in("lms", None, Some("LM Studio")) else {
        return Err(
            "LM Studio is holding a window too small for this turn, and its `lms` command is \
             not on this machine to open a bigger one."
                .to_owned(),
        );
    };
    let ran = std::process::Command::new(lms)
        .args([
            "load",
            model,
            "-c",
            &needed.to_string(),
            // Unattended: this is inside a turn, and a CLI stopping to ask which model it meant
            // would hang the caller rather than fail.
            "-y",
        ])
        .output()
        .map_err(|why| format!("`lms load` would not run: {why}"))?;

    // **Read the state back, never the exit code.** LM Studio answers 200 to routes it does not
    // have and this codebase keeps finding the same shape; a load that printed an error and
    // exited 0 would otherwise read as success.
    let now = ureq::builder()
        .timeout_connect(std::time::Duration::from_millis(400))
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .get(&format!("{endpoint}/api/v0/models"))
        .call()
        .ok()
        .and_then(|answer| answer.into_json::<serde_json::Value>().ok());
    if now
        .as_ref()
        .and_then(|listing| held_window(listing, model))
        .is_some_and(|held| held >= needed)
    {
        return Ok(());
    }

    // Its own words. Measured, they are the useful half: *"this model requires approximately
    // 9.61 GB of memory, and continuing to load it would likely overload your system"*.
    let said = String::from_utf8_lossy(&ran.stderr);
    let said = if said.trim().is_empty() {
        String::from_utf8_lossy(&ran.stdout).into_owned()
    } else {
        said.into_owned()
    };
    let said = said
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("it did not say why")
        .to_owned();
    Err(format!(
        "LM Studio would not load {model} with {needed} tokens of context: {said}"
    ))
}

/// The window one model **was given**, out of an LM Studio listing.
///
/// Not `max_context_length`, which is what it *could* take. The two sit side by side in the same
/// object and only one of them is what a turn has to fit inside.
fn held_window(listing: &serde_json::Value, model: &str) -> Option<u32> {
    listing["data"]
        .as_array()?
        .iter()
        .find(|one| one["id"].as_str() == Some(model))?
        .pointer("/loaded_context_length")?
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0)
}

/// Whether an address is this machine, so a local-only action stays local.
fn is_this_machine(endpoint: &str) -> bool {
    let host = endpoint
        .split("//")
        .nth(1)
        .unwrap_or(endpoint)
        .split(['/', ':'])
        .next()
        .unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

/// Where a binary is, if it is anywhere this machine looks.
///
/// **The installer's own directories before the PATH**, for the same reason `hf` needs it: an
/// application launched from a desktop does not inherit the PATH a shell would have, and
/// looking only there reports "not installed" about a machine that has it.
pub fn found_in(
    name: &str,
    winget_package: Option<&str>,
    program_folder: Option<&str>,
) -> Option<std::path::PathBuf> {
    let exact = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };

    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Some(home) = home() {
        roots.push(home.join(".local").join("bin"));
        // LM Studio's CLI, once the application has been run once and created it.
        roots.push(home.join(".lmstudio").join("bin"));
        roots.push(home.join(".llama").join("bin"));

        if cfg!(windows) {
            let local = home.join("AppData").join("Local");
            // Where an installer that behaves puts an application. Named by the caller rather
            // than listed here: this file used to hardcode `LM Studio`, and the second program
            // that installed the same way (ComfyUI, into `Programs\Comfy Desktop`) would have
            // meant a second hardcoded line in a function that has no business knowing either.
            roots.extend(program_folder.map(|folder| local.join("Programs").join(folder)));
            // WinGet's shims. Measured: llama.cpp is *not* shimmed here — only four unrelated
            // programs were — but something else might be, and it costs one `is_file`.
            roots.push(local.join("Microsoft").join("WinGet").join("Links"));
            roots.extend(winget_package.into_iter().flat_map(|package| {
                winget_homes(
                    &local.join("Microsoft").join("WinGet").join("Packages"),
                    package,
                )
            }));
        }
    }
    if cfg!(target_os = "macos") {
        // An application bundle keeps its binary inside itself.
        roots.extend(program_folder.map(|folder| {
            std::path::PathBuf::from(format!("/Applications/{folder}.app/Contents/MacOS"))
        }));
    }
    if !cfg!(windows) {
        roots.push(std::path::PathBuf::from("/opt/homebrew/bin"));
        roots.push(std::path::PathBuf::from("/usr/local/bin"));
    }
    if let Some(path) = std::env::var_os("PATH") {
        roots.extend(std::env::split_paths(&path));
    }

    roots
        .into_iter()
        .map(|dir| dir.join(&exact))
        .find(|candidate| candidate.is_file())
}

/// WinGet's directories for one package.
///
/// It appends a source suffix to the id — `ggml.llamacpp_Microsoft.Winget.Source_8wekyb3d8bbwe`
/// — which is not something to hardcode: it is WinGet's business and it varies by source. The
/// prefix is the part that is the package's own name.
#[cfg(windows)]
fn winget_homes(packages: &std::path::Path, package: &str) -> Vec<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(packages) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .to_lowercase()
                .starts_with(&package.to_lowercase())
        })
        .map(|entry| entry.path())
        .collect()
}

#[cfg(not(windows))]
fn winget_homes(_packages: &std::path::Path, _package: &str) -> Vec<std::path::PathBuf> {
    Vec::new()
}

fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
}

#[cfg(test)]
mod tests {

    /// Nothing told, which is the ordinary state: llama.cpp deciding for itself.
    fn untuned(_: &Weights) -> crate::tuning::Tuning {
        crate::tuning::Tuning::default()
    }

    use super::*;

    /// How a model sits on this card, read from what the fitter said about its own decision.
    ///
    /// Every string below is copied from `llama-fit-params -v` on this machine, 2026-09-01.
    mod fits {
        use super::*;

        /// The 35B at 32K: five placements tried, the last one accepted.
        const HYBRID: &str = "\
0.01.984.034 I common_memory_breakdown_print: |   - CUDA0 (RTX 4070 SUPER) | 12281 = 11005 + (17579 = 16383 +     702 +     493) +      -16302 |
0.01.984.035 I common_memory_breakdown_print: |   - Host                   |                    555 =   515 +       0 +      40                |
0.03.514.348 I common_memory_breakdown_print: |   - CUDA0 (RTX 4070 SUPER) | 12281 = 11005 + (9879 =  8680 +     702 +     497) +       -8603 |
0.03.514.348 I common_memory_breakdown_print: |   - Host                   |                  8258 =  8218 +       0 +      40                |
0.03.556.233 I common_params_fit_impl:   - CUDA0 (NVIDIA GeForce RTX 4070 SUPER): 41 layers (22 overflowing),   9879 MiB used,   1125 MiB free
0.03.556.248 I common_fit_params: successfully fit params to free device memory
";
        const HYBRID_ARGS: &str = r#"-c 32768 -ngl 41 -ot "blk\.19\.ffn_(gate|up)_exps=CPU,blk\.20\.ffn_(up|down)_exps=CPU""#;

        #[test]
        fn the_accepted_placement_is_the_last_one_tried_not_the_first() {
            /*
                The defect this closes. `find` read the first table — `17579 = 16383 + 702 + 493`
                with `-16302` unaccounted, the all-on-card arrangement about to be rejected —
                and the accepted one is five tables later.
            */
            let (model, context, compute) = breakdown(HYBRID, "CUDA0").expect("a device row");
            assert_eq!(
                model,
                8680 * 1024 * 1024,
                "the accepted fit, not the rejected one"
            );
            assert_eq!(context, 702 * 1024 * 1024);
            assert_eq!(compute, 497 * 1024 * 1024);
        }

        #[test]
        fn eight_gigabytes_on_the_host_do_not_read_as_zero() {
            /*
                `host_model` split the whole line on non-digits, and every line begins
                `0.03.514.348 I` — so the second number it found was the `00` of `0.03`. It
                answered `Some(0)` for a model with 8218 MiB of weights in host memory, and a
                planner filtering on `host > 0` then dropped the placement candidate for the one
                model that needed it.
            */
            assert_eq!(host_model(HYBRID), Some(8218 * 1024 * 1024));
        }

        #[test]
        fn a_model_that_needed_arranging_is_hybrid() {
            let got = read_fitting(HYBRID_ARGS, HYBRID);
            assert_eq!(got.class, FitClass::Hybrid);
            assert_eq!(got.overflowing, Some((22, 41)));
            assert_eq!(got.on_host, Some(8218 * 1024 * 1024));
            let args = got.args.expect("arguments to give llama.cpp");
            assert_eq!(args.gpu_layers, 41);
            assert!(args.override_tensor.contains("=CPU"));
            assert_eq!(args.model_on_host, Some(8218 * 1024 * 1024));
        }

        #[test]
        fn a_model_that_needed_nothing_is_comfortable_and_offers_no_candidate() {
            /*
                `gemma4-12b` at 32K answers `-c 32768 -ngl -1` and writes no `-ot`, and prints no
                overflow line at all. Nothing was arranged, so there is nothing to measure: a
                placement candidate here would be the baseline run a second time.
            */
            let got = read_fitting(
                "-c 32768 -ngl -1",
                "0.00.738 I common_fit_params: successfully fit params to free device memory",
            );
            assert_eq!(got.class, FitClass::Comfortable);
            assert_eq!(got.args, None);
            assert_eq!(got.overflowing, None);
        }

        /// The same two models, against the program that is actually installed.
        ///
        /// It loads a model and arranges nothing — no generation, no benchmark — and it is the
        /// only way to know the parse still matches a build nobody here chose.
        #[test]
        #[ignore = "runs llama-fit-params against the models on this machine"]
        fn the_installed_fitter_still_says_what_this_reads() {
            let shelf = std::path::Path::new(r"C:\Users\someone\Epoch\BUILD\vault\shelf");
            let hybrid = shelf.join("Qwen3.6-35B-A3B-UD-IQ4-XS.gguf");
            if hybrid.is_file() {
                let got = fitting(&hybrid, 32_768);
                assert_eq!(got.class, FitClass::Hybrid, "17.7 GB on a 12 GB card");
                assert!(got.on_host.is_some_and(|it| it > 1_000_000_000));
                assert!(got
                    .overflowing
                    .is_some_and(|(over, all)| over > 0 && all > over));
            }
            let comfortable = shelf.join("gemma4-12b.gguf");
            if comfortable.is_file() {
                let got = fitting(&comfortable, 32_768);
                assert_eq!(got.class, FitClass::Comfortable, "6.9 GB on a 12 GB card");
                assert_eq!(got.args, None, "nothing to arrange is nothing to measure");
            }
        }

        #[test]
        fn a_refusal_is_a_sentence_and_silence_is_not_one() {
            // What the program prints when there is no arrangement.
            let refused = read_fitting(
                "",
                "W common_fit_params: failed to fit params to free device memory",
            );
            assert_eq!(refused.class, FitClass::Unloadable);

            /*
                And a run nobody could read is `Unknown`, never `Unloadable`. A planner reading
                silence as *it does not fit* would refuse a search on a machine that is merely
                missing a sibling binary.
            */
            assert_eq!(read_fitting("", "").class, FitClass::Unknown);
            assert_eq!(
                read_fitting("some other output", "").class,
                FitClass::Unknown
            );
        }
    }

    #[test]
    fn a_served_name_and_a_shelf_name_can_be_compared_by_shelving_both() {
        /*
            **The property that makes one comparison safe for two spellings of one model.**

            A character on llama.cpp carries the name the router serves it under, `gemma4-12b`;
            the Models deck and the profile store both key on the shelf's own name, `gemma4:12b`.
            Every model whose name has no colon in it matches either way — which is how a
            character panel came to read `Not configured in MODELS` for a model with a 64K
            loadout on the deck, and, worse, would never have found a profile applied to its own
            Brain.

            Shelving *both* sides fixes it only because shelving is idempotent: a name that has
            already been through it must survive unchanged, or the served side would be mangled
            a second time and miss all over again.
        */
        for name in [
            "gemma4:12b",
            "gemma4-12b",
            "ggml-org/gemma-3-270m-GGUF:Q8_0",
            "Qwen3.6-35B-A3B-UD-IQ4_XS · MTP",
            "plain",
        ] {
            let once = shelf_name(name);
            assert_eq!(shelf_name(&once), once, "shelving {name} twice changed it");
        }
        // And the two spellings of the one model really do meet.
        assert_eq!(shelf_name("gemma4:12b"), shelf_name("gemma4-12b"));
        // Without changing what two different models mean.
        assert_ne!(shelf_name("gemma4:12b"), shelf_name("gemma4:27b"));
    }

    #[test]
    fn the_model_layer_is_found_by_what_the_manifest_says_it_is() {
        // **A walk of `blobs/` could not do this.** Templates, licences and parameter files sit
        // beside the weights, all named by digest and indistinguishable. The manifest is what
        // says which blob is the model, so it is what is read — the same rule the deletion work
        // follows: an enumerated list, never a directory walk.
        //
        // The manifest below is the real shape of `qwen3:14b` on the machine this was written
        // against, trimmed. The layer order is deliberately not model-first: reading `layers[0]`
        // would have worked on every model tried and broken on the first one that differed.
        let raw = serde_json::json!({
            "schemaVersion": 2,
            "layers": [
                {"mediaType": "application/vnd.ollama.image.template", "digest": "sha256:ae37", "size": 1723},
                {"mediaType": "application/vnd.ollama.image.model", "digest": "sha256:a8cc", "size": 9_276_184_896u64},
                {"mediaType": "application/vnd.ollama.image.license", "digest": "sha256:d18a", "size": 11338},
            ],
        });

        let model = raw["layers"]
            .as_array()
            .unwrap()
            .iter()
            .find(|layer| layer["mediaType"] == "application/vnd.ollama.image.model")
            .expect("the manifest names its model layer");
        assert_eq!(model["digest"], "sha256:a8cc");
        // The digest becomes a filename by swapping the colon, which is how Ollama writes it.
        assert_eq!(
            model["digest"].as_str().unwrap().replace(':', "-"),
            "sha256-a8cc"
        );
    }

    #[test]
    fn a_shared_name_is_a_filename_on_every_platform() {
        // `qwen3:14b` is not one on Windows, and the colon is in every Ollama model's name.
        assert_eq!(shelf_name("qwen3:14b"), "qwen3-14b");
        assert_eq!(
            shelf_name("hf.co/unsloth/Qwen3-GGUF:IQ3_XXS"),
            "hf.co-unsloth-Qwen3-GGUF-IQ3-XXS"
        );
        // The dot survives, because it is a filename character and part of `hf.co`.
        assert!(shelf_name("gemma4:12b").ends_with("12b"));
        // Nothing that could climb out of the directory it is put in.
        for name in ["../../etc", "a/b", "c\\d", "e:f"] {
            let clean = shelf_name(name);
            assert!(!clean.contains('/'), "{clean}");
            assert!(!clean.contains('\\'), "{clean}");
            assert!(!clean.contains(':'), "{clean}");
        }
    }

    #[test]
    fn a_saved_file_is_found_where_the_workshop_puts_it() {
        // `SAVE THE FILE` produced a GGUF nothing was looking at: not Ollama, which knows only
        // its own store, and not the other two, which were only ever offered Ollama's blobs. A
        // download that lands somewhere nothing can reach is a download that did not happen.
        let dir = std::env::temp_dir().join(format!("epoch-gguf-{}", std::process::id()));
        let repo = dir.join("unsloth-Qwen3-GGUF");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::write(repo.join("Qwen3-UD-IQ3_XXS.gguf"), b"GGUF").unwrap();
        // Things that are not models, beside it — `hf download` brings a README and a config.
        std::fs::write(repo.join("README.md"), b"hello").unwrap();
        std::fs::write(repo.join("config.json"), b"{}").unwrap();

        let found = gguf_in(&dir);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "Qwen3-UD-IQ3_XXS");
        assert_eq!(found[0].from, "Saved here");
        assert_eq!(found[0].bytes, 4);

        // A directory that is not there is an empty answer, not a failure: a vault with nothing
        // saved yet is the ordinary first run.
        assert!(gguf_in(&dir.join("nothing")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn importing_writes_one_line_and_names_the_model_safely() {
        // The recipe is one line on purpose. A template or a parameter here would be Epoch
        // deciding how somebody else's model behaves, which belongs to the Character
        // (ADR-0026) and not to a file on disk.
        let dir = std::env::temp_dir().join(format!("epoch-import-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let gguf = dir.join("Qwen3-UD-IQ3_XXS.gguf");
        std::fs::write(&gguf, b"GGUF").unwrap();

        let out = ollama_import_command("unsloth/Qwen3:IQ3_XXS", &gguf.display().to_string());
        // Ollama may not be on the machine running the tests, and that is a real answer rather
        // than a failure — but the recipe is written either way, because writing it is what
        // this function is for.
        let recipe = dir.join("unsloth-Qwen3-IQ3-XXS.Modelfile");
        assert!(recipe.is_file(), "{recipe:?}");
        assert_eq!(
            std::fs::read_to_string(&recipe).unwrap().trim(),
            format!("FROM {}", gguf.display())
        );

        if let Ok(command) = out {
            // Named safely: `unsloth/Qwen3:IQ3_XXS` is not a model name Ollama takes, and the
            // slash would read as a namespace it does not have.
            assert!(
                command.contains("create unsloth-Qwen3-IQ3-XXS"),
                "{command}"
            );
            assert!(command.contains("-f "), "{command}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_shelf_is_stocked_named_and_pruned() {
        // **Why a shelf rather than a choice.** llama.cpp was started *holding one model*,
        // picked from a dropdown — a question nobody wants to answer before they know what they
        // are going to ask. `llama-server --models-dir` is a router: measured 2026-08-21, it
        // reported both models at `/v1/models`, loaded one only when a request named it
        // ("Blue", 21.2s), and left the other merely available.
        let shelf = std::env::temp_dir().join(format!("epoch-shelf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&shelf);
        let source = shelf.join("source");
        std::fs::create_dir_all(&source).unwrap();
        let blob = source.join("sha256-a8cc");
        std::fs::write(&blob, b"GGUF").unwrap();

        let held = vec![Weights {
            name: "qwen3:14b".into(),
            from: "Ollama".into(),
            path: blob.display().to_string(),
            bytes: 4,
            sees_with: None,
        }];
        // Whatever a model has not been measured for. The shelf's job is the links and the
        // file; which loadout each model gets is `loadout`'s question and is tested there.
        let plain = |_: &Weights| crate::loadout::conservative(16_384);
        let stocked = stock_shelf(&shelf, &held, &plain, &untuned);
        assert_eq!(stocked.models, 1, "{:?}", stocked.problems);

        // **The extension is the whole point.** llama.cpp's router looks for `.gguf` and so does
        // LM Studio, while Ollama's blobs are named by digest and have none — a link named after
        // the digest is created successfully and then never appears.
        let link = shelf.join("qwen3-14b.gguf");
        assert!(link.is_file(), "named for a person, and ending .gguf");
        assert_eq!(std::fs::read(&link).unwrap(), b"GGUF");

        // Stocking twice is not two links. It is the same shelf, checked.
        assert_eq!(stock_shelf(&shelf, &held, &plain, &untuned).models, 1);

        // And a model that went away stops being offered: a shelf that only ever grew would
        // hand out links pointing at nothing.
        let empty = stock_shelf(&shelf, &[], &plain, &untuned);
        assert_eq!(empty.models, 0);
        assert!(!link.exists(), "pruned");

        // Only `.gguf`, and only here. This deletes things.
        std::fs::write(shelf.join("notes.txt"), b"mine").unwrap();
        stock_shelf(&shelf, &held, &plain, &untuned);
        assert!(shelf.join("notes.txt").is_file(), "nothing else is touched");

        let _ = std::fs::remove_dir_all(&shelf);
    }

    /// A vision model is two files, and lending one of them lends a model that cannot see.
    ///
    /// Measured 2026-08-21 against the real LM Studio: with the weights alone it answered
    /// `gemma4-12b does not support image inputs` — about a model that had been describing
    /// pictures through Ollama all week, because Ollama keeps the projector as a second layer
    /// and pairs them itself. With `mmproj-gemma4-12b.gguf` linked beside it, the same server
    /// described the photograph: *"A man wearing a black leather jacket and tie reaches out his
    /// hand toward the camera."*
    #[test]
    fn a_model_that_can_see_goes_on_the_shelf_with_its_eyes() {
        let shelf = std::env::temp_dir().join(format!("epoch-eyes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&shelf);
        let source = shelf.join("source");
        std::fs::create_dir_all(&source).unwrap();
        let weights = source.join("sha256-1278");
        let projector = source.join("sha256-675a");
        std::fs::write(&weights, b"GGUF").unwrap();
        std::fs::write(&projector, b"GGUFmm").unwrap();

        let held = vec![Weights {
            name: "gemma4:12b".into(),
            from: "Ollama".into(),
            path: weights.display().to_string(),
            bytes: 4,
            sees_with: Some(projector.display().to_string()),
        }];

        // A model measured as wanting a large context on the compressed cache, so the preset
        // can be checked for carrying it: every model gets a section now, and what is *in* the
        // section is what makes one worth writing.
        let measured = |_: &Weights| crate::loadout::Loadout {
            context: 32_768,
            cache: crate::loadout::Cache::Q8_0,
        };
        let stocked = stock_shelf(&shelf, &held, &measured, &untuned);
        assert!(stocked.problems.is_empty(), "{:?}", stocked.problems);

        let eyes = shelf.join("mmproj-gemma4-12b.gguf");
        assert!(eyes.is_file(), "the projector is beside the weights");
        assert_eq!(std::fs::read(&eyes).unwrap(), b"GGUFmm");

        // **And the prune does not take them.** The projector is a `.gguf` in a directory that
        // deletes unrecognised `.gguf` files, so a shelf that counted only models would link
        // the eyes and then remove them on the next press.
        let again = stock_shelf(&shelf, &held, &measured, &untuned);
        assert!(eyes.is_file(), "still there after stocking twice");
        assert_eq!(again.models, 2, "the model and its eyes are both wanted");

        // **And llama.cpp is told, because it does not pair them by name.** LM Studio does;
        // llama.cpp answered `image input is not supported` with the projector sitting right
        // beside the weights. `key = value`, not CLI flags — flags gave `failed to parse server
        // config file`, and `key = value` gave `Loaded 1 custom model presets` and a correct
        // description of the photograph.
        let ini = std::fs::read_to_string(presets_file(&shelf)).expect("a preset was written");
        assert!(ini.contains("[gemma4-12b]"), "{ini}");
        assert!(ini.contains("model = "), "{ini}");
        assert!(ini.contains("mmproj = "), "{ini}");
        assert!(!ini.contains("-m "), "flags do not parse: {ini}");
        // It names this shelf's links, never Ollama's blobs: a preset pointing into the store
        // would be wrong the moment the model is unpulled.
        assert!(ini.contains("mmproj-gemma4-12b.gguf"), "{ini}");

        // **And how to load it, which is why every model gets a section now.** Measured: a
        // `ctx-size` in the preset is honoured only when the parent passes no `-c`, so this and
        // `router_command`'s missing `-c` are one fix in two places.
        assert!(ini.contains("ctx-size = 32768"), "{ini}");
        assert!(ini.contains("cache-type-k = q8_0"), "{ini}");
        assert!(ini.contains("cache-type-v = q8_0"), "{ini}");

        // A model that goes away takes its eyes with it.
        stock_shelf(&shelf, &[], &measured, &untuned);
        assert!(!eyes.exists(), "pruned with the model it belongs to");
        // And so does the file that named them. A preset outliving the links it describes is a
        // server that fails to start over a model nobody has any more.
        assert!(!presets_file(&shelf).exists(), "the preset went with them");

        let _ = std::fs::remove_dir_all(&shelf);
    }

    /// Removing a model takes away every link Epoch made, not only the ones on its own shelf.
    #[test]
    fn a_removal_cleans_up_after_the_lending_too() {
        // Measured on the owner's machine: after deleting four models, `.lmstudio/models/ollama`
        // still held 58.8 GB with a link count of 1 on every file — Epoch's own link was the
        // last thing keeping those bytes alive, and the deck said they were gone.
        // **Unique per run, not per process.** The first version keyed on the pid and the same
        // test ran twice concurrently against one directory, so it deleted the other run's
        // fixture and failed on a file it had just written.
        let root = std::env::temp_dir().join(format!(
            "epoch-remove-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|it| it.as_nanos())
                .unwrap_or_default()
        ));
        let shelf = root.join("shelf");
        let lent = root
            .join("lmstudio")
            .join("models")
            .join("ollama")
            .join("a-model");
        std::fs::create_dir_all(&shelf).unwrap();
        std::fs::create_dir_all(&lent).unwrap();
        for at in [shelf.join("a-model.gguf"), lent.join("a-model.gguf")] {
            std::fs::write(&at, b"weights").unwrap();
        }
        // Both are files Epoch put there, and both must go.
        assert!(shelf.join("a-model.gguf").is_file());
        assert!(lent.join("a-model.gguf").is_file());

        // `remove` reaches LM Studio through `home()`, which a test cannot move — so the shape
        // that matters is asserted directly: the name it builds, and that the directory is only
        // removed when nothing else is in it.
        let clean = shelf_name("a model");
        assert_eq!(
            clean, "a-model",
            "the folder is named the way lending names it"
        );
        std::fs::write(lent.join("something-else.txt"), b"not Epoch's").unwrap();
        std::fs::remove_file(lent.join("a-model.gguf")).unwrap();
        assert!(
            std::fs::remove_dir(&lent).is_err(),
            "a directory holding something that is not Epoch's is left alone"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// A removal that freed nothing says nothing.
    #[test]
    fn only_a_real_number_is_reported() {
        // The cold-instrument rule on a sentence: a model whose links were shared with something
        // else frees no bytes, and "0.0 GB went with it" would be a reading of nothing.
        assert_eq!(also_freed(0), "");
        assert!(also_freed(17_987_569_344).contains("18.0 GB"));
        assert!(also_freed(17_987_569_344).contains("LM Studio"));
    }

    /// The two spellings of one model, and the one Epoch wrote itself.
    ///
    /// Measured 2026-08-30 with the router serving this machine's shelf: Ollama publishes
    /// `gemma4:12b` and llama.cpp publishes `gemma4-12b`, because `shelf_name` made the link it
    /// loads and a colon is not a filename character. Compared by equality, a deck row could
    /// never be timed on llama.cpp — and the request would have carried a name that server has
    /// never heard of even if it had matched.
    #[test]
    fn a_model_is_found_under_the_name_each_server_uses() {
        let ollama = vec!["gemma4:12b".to_owned(), "qwen3:14b".to_owned()];
        let llama = vec!["gemma4-12b".to_owned(), "qwen3-14b".to_owned()];

        assert_eq!(
            offered_as("gemma4:12b", &ollama),
            Some("gemma4:12b".to_owned()),
            "its own name where the server uses it"
        );
        assert_eq!(
            offered_as("gemma4:12b", &llama),
            Some("gemma4-12b".to_owned()),
            "and the shelf's name where that is what the server calls it"
        );
        assert_eq!(
            offered_as("gemma4:12b", &["something-else".to_owned()]),
            None,
            "and nothing where it is genuinely not offered"
        );
        // A saved GGUF is already filename-shaped, so both spellings are the same one.
        assert_eq!(
            offered_as(
                "Qwen3.8-27B-Uncensored-IQ2-M",
                &["Qwen3.8-27B-Uncensored-IQ2-M".to_owned()]
            ),
            Some("Qwen3.8-27B-Uncensored-IQ2-M".to_owned())
        );
    }

    #[test]
    fn where_a_server_offers_both_spellings_the_real_name_wins() {
        /*
            Measured 2026-08-31: llama.cpp's router served `gemma-3-270m` under its own name
            *and* under the shelf's, and a benchmark of it answered
            `model name=ggml-org-gemma-3-270m-GGUF-Q8-0 failed to load` — because one `find` over
            both spellings returns whichever the server listed first, and the server lists
            alphabetically, and `ggml-org-…` sorts before `ggml-org/…`.

            A fallback that can win a race against the fact it falls back from is not a fallback.
        */
        let both = vec![
            "ggml-org-gemma-3-270m-GGUF-Q8-0".to_owned(),
            "ggml-org/gemma-3-270m-GGUF:Q8_0".to_owned(),
        ];
        assert_eq!(
            offered_as("ggml-org/gemma-3-270m-GGUF:Q8_0", &both),
            Some("ggml-org/gemma-3-270m-GGUF:Q8_0".to_owned()),
        );
        // Reversed, so the answer is not the list order agreeing with us by luck.
        let reversed: Vec<String> = both.iter().rev().cloned().collect();
        assert_eq!(
            offered_as("ggml-org/gemma-3-270m-GGUF:Q8_0", &reversed),
            Some("ggml-org/gemma-3-270m-GGUF:Q8_0".to_owned()),
        );
    }

    #[test]
    fn a_shelf_link_that_no_longer_matches_its_source_is_made_again() {
        /*
            A model in the Hugging Face cache is reached through a symlink, and hard-linking a
            symlink on Windows links the reparse point — 76 bytes of pointer that llama.cpp
            reports as `failed to load`. `!link.is_file()` was the whole guard, and a broken link
            *is* a file, so once one existed nothing would ever replace it.
        */
        let here = std::env::temp_dir().join(format!("epoch-relink-{}", std::process::id()));
        let shelf = here.join("shelf");
        let _ = std::fs::create_dir_all(&shelf);
        let source = here.join("real.gguf");
        std::fs::write(&source, vec![7u8; 4096]).expect("written");

        let held = vec![Weights {
            name: "real".to_owned(),
            from: "test".into(),
            path: source.display().to_string(),
            bytes: 4096,
            sees_with: None,
        }];
        let plain = |_: &Weights| crate::loadout::conservative(16_384);
        let untuned = |_: &Weights| crate::tuning::Tuning::default();

        // Somebody's broken link is already sitting there.
        let link = shelf.join("real.gguf");
        std::fs::write(&link, b"not the model").expect("written");
        stock_shelf(&shelf, &held, &plain, &untuned);

        assert_eq!(
            std::fs::metadata(&link).expect("still there").len(),
            4096,
            "the link was replaced with one that matches its source",
        );
        let _ = std::fs::remove_dir_all(&here);
    }

    /// **It never ran.** The function lost its `#[test]` at some point and clippy found it as
    /// dead code, which is the only reason anybody looked: a test with no attribute compiles,
    /// is never executed, and reads exactly like one that passes.
    #[test]
    fn a_command_with_two_quoted_paths_is_wrapped_so_cmd_cannot_eat_them() {
        // Measured 2026-08-21. `cmd /k <string>` strips the *first and last* quote when there
        // are more than two, so one quoted path survives — which is why `ollama serve` worked
        // for weeks — and two do not:
        //
        //   cmd /c "<exe>" -m "<model>" --version   -> "The filename, directory name, or
        //                                              volume label syntax is incorrect."
        //   cmd /c ""<exe>" -m "<model>" --version" -> version: 0.1.2-dev (build 10507)
        //
        // The assertion is on the shape rather than on a run, because running it opens a window.
        let shelf = std::path::Path::new(r"C:\Users\a b\shelf");
        if let Some(command) = router_command(shelf) {
            assert!(
                command.matches('"').count() >= 4,
                "two quoted paths: {command}"
            );
            assert!(command.contains("--models-dir"), "{command}");
            // One at a time: a router allowed several would hold two large models on a consumer
            // card, which is the machine swapping.
            assert!(command.contains("--models-max 1"), "{command}");

            // **And it says nothing about context or cache.** Both were here, both were right
            // about the machine they were measured on, and both are per-model answers: gemma
            // wants 64k on the full-precision cache and the 27B wants 16k compressed, on the
            // same card. They live in the preset now.
            //
            // Asserted as an absence, because the absence is the mechanism. Measured
            // 2026-08-26: a `ctx-size` in a preset is honoured only when the parent passes no
            // `-c` — with `-c 16384` on the parent, a preset asking for 8192 spawned a child
            // with `--ctx-size 16384`. So a `-c` here would silently flatten every model back
            // to one setting while the presets went on saying otherwise.
            assert!(
                !command.contains(" -c "),
                "context is per-model now: {command}"
            );
            assert!(
                !command.contains("--cache-type"),
                "the cache is per-model now: {command}"
            );

            // **And it does not answer the question llama.cpp measures.** `--fit` adjusts
            // *unset* arguments to device memory and is on by default; `-ngl` was the only
            // thing switching it off. A model too big for the card went from 0.92 tok/s to
            // 6.2 once the fitter was allowed to split it instead of being told to force
            // every layer on. Asserted as an absence, because that is what the fix is.
            assert!(
                !command.contains("-ngl"),
                "the fitter decides layers now: {command}"
            );
            // Only where there is a card to hold memory back from. On unified memory the flag
            // is destructive, not merely wrong — see `fit_margin`.
            assert_eq!(
                command.contains("-fitt"),
                !crate::machine::Machine::measure().unified,
                "{command}"
            );
            assert!(
                !command.contains("-fitt") || command.contains(&format!("-fitt {FIT_MARGIN_MIB}")),
                "{command}"
            );
            // A lifeboat for a turn that never got to release: the exact unload happens in
            // `turn::drive`, and this is what covers Epoch being killed mid-turn.
            assert!(command.contains("--sleep-idle-seconds"), "{command}");
        }
    }

    #[test]
    fn a_shared_model_is_named_and_weighed_the_way_a_person_reads_it() {
        // Whatever this machine actually has. An empty list is honest — Ollama is not here, or
        // has pulled nothing — so the assertions are about shape rather than about contents.
        for held in ollama_weights() {
            assert!(held.name.contains(':'), "a name and a tag: {}", held.name);
            assert!(!held.name.starts_with("library/"), "{}", held.name);
            assert!(held.bytes > 0, "{}", held.name);
            assert!(
                std::path::Path::new(&held.path).is_file(),
                "the file was checked before it was offered: {}",
                held.path
            );
        }
    }

    #[test]
    fn a_command_that_does_not_exist_is_worse_than_none() {
        // Measured against the package managers on 2026-08-20. `winget search --id llama.cpp`
        // finds nothing — the id is `ggml.llamacpp` — and the id nobody checked is exactly the
        // kind somebody pastes and then blames themselves for.
        for runtime in Runtime::ALL {
            let command = runtime.install_command();
            assert!(!command.is_empty());
            if cfg!(windows) {
                assert!(command.starts_with("winget install "), "{command}");
            } else {
                assert!(command.starts_with("brew install"), "{command}");
            }
        }
    }

    #[test]
    fn only_a_runtime_that_can_be_told_to_serve_offers_a_command() {
        // Read from Hugging Face's own "How to use from llama.cpp" panel rather than
        // remembered: `llama serve -hf <repo>:<QUANT>` fetches and serves in one step.
        // Only when the program is here: the command names the binary that was actually found,
        // so it can never be a plausible-looking line that does nothing on this machine.
        let said = serve_command(
            Runtime::LlamaCpp,
            "hf.co/unsloth/Qwen3.8-27B-GGUF:UD-IQ3_XXS",
        );
        match said {
            None => assert!(
                found_in("llama", Runtime::LlamaCpp.winget_package(), None).is_none()
                    && found_in("llama-server", Runtime::LlamaCpp.winget_package(), None).is_none(),
                "no command offered, so neither binary should be here"
            ),
            Some(line) => {
                assert!(
                    line.contains("-hf unsloth/Qwen3.8-27B-GGUF:UD-IQ3_XXS"),
                    "{line}"
                );
                // The full path, quoted. These are usually *not* on the PATH — winget leaves
                // them inside its own package directory — so a bare name in a terminal that
                // cannot resolve it is the same failure as a command that does not exist.
                assert!(line.contains("llama"), "{line}");
                assert!(line.starts_with('"'), "{line}");
            }
        }

        // An Ollama registry name is not a repository, so there is nothing to hand it.
        assert_eq!(serve_command(Runtime::LlamaCpp, "gemma4:12b"), None);

        // And LM Studio's server is started from the application. A command guessed at here
        // would be a control that does nothing on most machines.
        assert_eq!(
            serve_command(Runtime::LmStudio, "hf.co/unsloth/x-GGUF:Q4_K_M"),
            None
        );
    }

    #[test]
    fn no_two_runtimes_share_a_port() {
        // They are separate answers on one machine, so a survey that put two at one address
        // would report whichever answered as both.
        let mut ports: Vec<u16> = Runtime::ALL.into_iter().map(Runtime::usual_port).collect();
        ports.sort_unstable();
        let before = ports.len();
        ports.dedup();
        assert_eq!(ports.len(), before, "two runtimes claim one port");
    }

    #[test]
    fn nothing_here_offers_a_command_to_start_it() {
        // `None` means there is nothing to start, which is a different sentence from *it is not
        // running* — and a button offering to start a program that is not installed is the
        // control-that-cannot-do-what-it-says this module opens by naming.
        for runtime in Runtime::ALL {
            let seen = look_for(runtime);
            assert_eq!(
                seen.start.is_some(),
                seen.installed,
                "{}: installed and startable must agree",
                seen.name
            );
            if let Some(line) = &seen.start {
                // Quoted full path: none of these is reliably on the PATH.
                assert!(line.starts_with('"'), "{line}");
            }
        }
    }

    #[test]
    fn nothing_installed_is_a_reading_rather_than_a_fault() {
        // A machine that runs models through Ollama alone is an ordinary machine. The survey
        // answers rather than failing, and every field says what it measured.
        for one in survey() {
            assert!(!one.name.is_empty());
            assert!(one.endpoint.starts_with("http://127.0.0.1:"));
            // Serving is measured by asking. It can never be true while the list is unset.
            if !one.serving {
                assert!(one.models.is_empty(), "{one:?}");
            }
        }
    }

    #[test]
    #[ignore = "reads this machine"]
    fn what_is_actually_here() {
        for one in survey() {
            println!("{one:?}");
        }
    }
}

/// The command that starts this runtime's server, when the program is here.
///
/// **All three were measured on 2026-08-20, on a machine that had them installed.** None of
/// this is remembered:
///
/// ```text
/// ollama serve
/// lms server start          (`lms --help` → server start · stop · status)
/// llama-server              → "Loaded 0 cached model presets · Available models (0)"
/// ```
///
/// The last one is the surprise worth writing down: `llama-server` **does** start with no model
/// at all and serves an empty `/v1/models`, exactly as LM Studio does. A panel that refused to
/// start it without one would have been wrong about something it never tried.
///
/// The full path is used, quoted, because none of these are reliably on the PATH — winget
/// leaves llama.cpp inside its own package directory, and `lms` lives under `~/.lmstudio/bin`.
/// `None` when the program is not here: there is nothing to start.
/// What Epoch will tell a runtime when it starts it.
///
/// **Only what the program can be told at start**, which is a shorter list than it looks. A
/// context length is per request or per load and never belongs here; llama.cpp's cache is chosen
/// per model by the curve that measured it (`loadout`), so it is not here either.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preference {
    /// Which installed GPU backend to use. `None` is the program's own detection, which is what
    /// it does with the variable unset — an absence rather than a value.
    #[serde(default)]
    pub engine: Option<String>,
    /// Whether to hold the attention cache compressed rather than at full precision.
    ///
    /// **A decision, never a default.** Measured on `gemma4:12b` at 65,536 tokens: full precision
    /// reports 9.11 GB of which only 3.53 GB is on the card, the rest spilled across PCIe;
    /// compressed reports 7.95 GB entirely resident. It also costs some accuracy, and llama.cpp's
    /// own curve measures which side of that trade wins for a given model — inheriting that
    /// answer for a different runtime would be measuring the wrong thing.
    ///
    /// **And it can fail outright**, which is why nothing keeps it without proving it first: a
    /// quantized V cache requires flash attention, flash attention is a property of the
    /// *backend*, and without it the model does not load at all. See [`prove_the_cache`].
    #[serde(default)]
    pub compressed_cache: bool,
}

impl Preference {
    /// The environment a runtime is started with, as pairs.
    ///
    /// Empty for everything except Ollama, and that is not an omission: LM Studio's engine is
    /// chosen through its own CLI (`engines::choose`) and keeps that choice itself, and
    /// llama.cpp carries one backend per install with nothing to switch (11.20).
    pub fn told_to(&self, runtime: Runtime) -> Vec<(&'static str, String)> {
        if runtime != Runtime::Ollama {
            return Vec::new();
        }
        let mut said = Vec::new();
        if let Some(engine) = self.engine.as_deref().filter(|it| !it.trim().is_empty()) {
            // Measured, not read from the help: started with `vulkan`, Ollama's log says
            // `library=Vulkan` and `using device Vulkan`.
            said.push(("OLLAMA_LLM_LIBRARY", engine.to_owned()));
        }
        if self.compressed_cache {
            // Ollama ignores a per-request `cache_type_k` silently — 8.39 GB with it and 8.39 GB
            // without, byte for byte. This variable is the only thing that moves the number.
            said.push(("OLLAMA_KV_CACHE_TYPE", "q8_0".to_owned()));
        }
        said
    }
}

/// What the router says it would run, read back as rows.
#[cfg(test)]
mod what_llama_cpp_is_serving {
    use super::*;

    /// A real `/v1/models` answer, recorded off the running llama.cpp on 2026-09-08.
    ///
    /// Trimmed to two entries and kept otherwise verbatim — argument order included, because the
    /// flags are read by name and a fixture that tidied them would stop testing that.
    fn a_real_answer(model: &str, mmproj: Option<&str>) -> serde_json::Value {
        let mut args = vec![
            serde_json::json!("C:\\Users\\kislok\\.llama\\bin\\llama-server.exe"),
            serde_json::json!("--host"),
            serde_json::json!("127.0.0.1"),
            serde_json::json!("--port"),
            serde_json::json!("0"),
            serde_json::json!("--sleep-idle-seconds"),
            serde_json::json!("86400"),
            serde_json::json!("--alias"),
            serde_json::json!("Qwen3.5-9B-Q6-K"),
            serde_json::json!("--ctx-size"),
            serde_json::json!("131072"),
            serde_json::json!("--fit-target"),
            serde_json::json!("128"),
            serde_json::json!("--model"),
            serde_json::json!(model),
        ];
        if let Some(eyes) = mmproj {
            args.push(serde_json::json!("--mmproj"));
            args.push(serde_json::json!(eyes));
        }
        serde_json::json!({
            "object": "list",
            "data": [{
                "id": "Qwen3.5-9B-Q6-K",
                "object": "model",
                "owned_by": "llamacpp",
                "status": { "value": "unloaded", "args": args },
            }],
        })
    }

    fn a_file(name: &str, bytes: usize) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("epoch-served-{n}"));
        let _ = std::fs::create_dir_all(&dir);
        let at = dir.join(name);
        std::fs::write(&at, vec![0u8; bytes]).expect("a file to point at");
        at
    }

    /// **The path comes from `--model`, and the eyes from `--mmproj`.**
    ///
    /// Both by flag. The projector especially: a flat shelf holds one `mmproj` per vision model,
    /// so "the first one in the folder" — which is right for a Hugging Face snapshot — would
    /// hand the same eyes to every model on it. The router was told which; it is asked.
    #[test]
    fn a_served_model_is_the_file_the_router_was_pointed_at() {
        let model = a_file("Qwen3.5-9B-Q6-K.gguf", 2048);
        let eyes = model.with_file_name("mmproj-Qwen3.5-9B.gguf");
        std::fs::write(&eyes, [0u8; 16]).unwrap();

        let found = weights_in(&a_real_answer(
            &model.display().to_string(),
            Some(&eyes.display().to_string()),
        ));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "Qwen3.5-9B-Q6-K");
        assert_eq!(found[0].from, "llama.cpp");
        assert_eq!(found[0].path, model.display().to_string());
        assert_eq!(found[0].bytes, 2048);
        assert_eq!(found[0].sees_with, Some(eyes.display().to_string()));

        // No projector on the line is no projector, never the neighbour that happens to be there.
        let alone = a_file("Qwen3.5-9B-Q6-K.gguf", 2048);
        std::fs::write(alone.with_file_name("mmproj-somebody-else.gguf"), [0u8; 16]).unwrap();
        let found = weights_in(&a_real_answer(&alone.display().to_string(), None));
        assert_eq!(found[0].sees_with, None, "eyes are told, not found");
    }

    /// **A row for bytes that are not there is worse than a missing row.**
    ///
    /// A router keeps offering a model whose file somebody deleted, and every reading downstream
    /// — how big it is, whether it fits, whether it can be measured — would be about nothing.
    #[test]
    fn a_model_whose_file_is_gone_is_not_offered() {
        let answer = a_real_answer(r"C:\nowhere\at\all\ghost.gguf", None);
        assert!(weights_in(&answer).is_empty());
    }

    /// **Against the router that is actually running.** Opt-in, and it starts nothing.
    ///
    /// The defect this closes was two panels of one window disagreeing about one machine, so the
    /// assertion is the agreement: every model the router offers whose file is still here shows
    /// up as a row.
    #[test]
    #[ignore = "needs a running llama.cpp router"]
    fn what_the_router_serves_is_what_this_machine_reports() {
        let served = served_weights();
        assert!(!served.is_empty(), "nothing served, or nothing listening");
        for one in &served {
            println!("{} · {} MB · {}", one.name, one.bytes / 1_048_576, one.path);
            assert!(std::path::Path::new(&one.path).exists());
        }
        let everything = everything_here(&[]);
        for one in &served {
            assert!(
                everything.iter().any(|it| it.path == one.path),
                "{} is served and must be listed",
                one.name
            );
        }
    }

    /// An answer without the shape this reads says nothing, rather than half of it.
    #[test]
    fn an_answer_of_another_shape_contributes_nothing() {
        assert!(weights_in(&serde_json::json!({})).is_empty());
        assert!(weights_in(&serde_json::json!({ "data": [] })).is_empty());
        // An older build that publishes no `status` at all — the one `llama_cpp_loaded` beside
        // this already has to cope with.
        assert!(
            weights_in(&serde_json::json!({ "data": [{ "id": "x", "object": "model" }] }))
                .is_empty()
        );
    }
}

#[cfg(test)]
mod what_llama_cpp_says_it_is_holding {
    use super::llama_cpp_loaded;

    /// **The router states residency, and is believed — including when it says no.**
    ///
    /// Measured live on Windows, 2026-09-08: a `llama-server` given a model directory answers
    /// `status: {"value": "unloaded"}` for a model it is offering and not holding.
    #[test]
    fn a_server_that_states_it_is_taken_at_its_word() {
        let said = serde_json::json!({"data": [
            {"id": "Ministral-3-14B", "status": {"value": "unloaded"}},
            {"id": "gemma4-12b",      "status": {"value": "loaded"}},
            {"id": "qwen3-14b",       "status": {"value": "sleeping"}},
        ]});
        // `sleeping` is not holding: across one idle release the process went 8812 MB -> 126 MB
        // and the card 10736 MiB -> 1937 MiB. Counting it would call a free card busy.
        assert_eq!(llama_cpp_loaded(&said), vec!["gemma4-12b".to_owned()]);
    }

    /// **And a server that states nothing is not saying no.**
    ///
    /// Measured live on the crew's MacBook in the same minute: Homebrew's `llama-server -m
    /// file.gguf` publishes no `status` anywhere in `/v1/models`. The rule had been written
    /// against the router and applied here, so an absent field read as *unloaded* and the KEEP
    /// lamp said *not loaded* about a model that had just answered a turn in 51 s off a warm
    /// cache. A single-model server has one model and loads it before it answers anything.
    #[test]
    fn a_server_with_one_model_and_no_status_is_holding_it() {
        let said = serde_json::json!({"data": [
            {"id": "gemma4-12b", "meta": {"n_ctx": 24576, "n_params": 11907350576i64}},
        ]});
        assert_eq!(llama_cpp_loaded(&said), vec!["gemma4-12b".to_owned()]);
    }

    /// Nothing listed is nothing held, whichever build it is.
    #[test]
    fn an_empty_list_holds_nothing() {
        assert!(llama_cpp_loaded(&serde_json::json!({"data": []})).is_empty());
        assert!(llama_cpp_loaded(&serde_json::json!({"error": "no"})).is_empty());
    }
}

#[cfg(test)]
mod what_a_runtime_is_told {
    use super::*;

    #[test]
    fn nothing_chosen_says_nothing() {
        // The absence is the program's own detection, and an empty variable is not that.
        assert!(Preference::default().told_to(Runtime::Ollama).is_empty());
    }

    #[test]
    fn only_ollama_is_told_at_the_start() {
        // LM Studio keeps its own choice through `lms runtime select`, and llama.cpp carries one
        // backend per install (11.20). Setting Ollama's variables for either would be a control
        // that changes nothing.
        let both = Preference {
            engine: Some("vulkan".into()),
            compressed_cache: true,
        };
        assert_eq!(both.told_to(Runtime::Ollama).len(), 2);
        assert!(both.told_to(Runtime::LmStudio).is_empty());
        assert!(both.told_to(Runtime::LlamaCpp).is_empty());
    }

    #[test]
    fn the_variables_are_the_ones_that_were_measured() {
        let told = Preference {
            engine: Some("cuda_v13".into()),
            compressed_cache: true,
        }
        .told_to(Runtime::Ollama);
        assert!(told.contains(&("OLLAMA_LLM_LIBRARY", "cuda_v13".to_owned())));
        // `q8_0` and not `q8`: Ollama passes the value straight to llama.cpp, which names it
        // that way, and a near-miss here is a server that refuses after everything else worked.
        assert!(told.contains(&("OLLAMA_KV_CACHE_TYPE", "q8_0".to_owned())));
    }

    #[test]
    fn an_engine_of_only_spaces_is_not_a_choice() {
        assert!(Preference {
            engine: Some("   ".into()),
            compressed_cache: false,
        }
        .told_to(Runtime::Ollama)
        .is_empty());
    }

    #[test]
    fn a_refusal_is_kept_in_the_programs_own_words() {
        // Measured: Ollama answers 500 with this in the body when the backend has no flash
        // attention. The sentence is the only true account of why, so it survives whole.
        let body = r#"{"error":"llama-server process has terminated: exit status 1: llama_init_from_model: quantized V cache requires flash_attn to be enabled"}"#;
        assert!(read_complaint(body).contains("requires flash_attn to be enabled"));
        // And a body that is not the shape Epoch expected is shown rather than swallowed.
        assert_eq!(read_complaint("  plain trouble  "), "plain trouble");
    }
}

/// A curve run through a server that is already answering, rather than through a child Epoch
/// spawns.
///
/// **Only where the context can be said per request**, which is Ollama (measured 2026-08-30).
/// `Bench` starts its own `llama-server` at each setting and can therefore vary the cache too;
/// this one asks a running server, so it varies the one axis that server accepts and leaves the
/// cache to whatever it was started with.
pub struct Served {
    pub endpoint: String,
    pub model: String,
}

impl crate::loadout::Probe for Served {
    fn run(&mut self, one: crate::loadout::Loadout) -> Option<crate::loadout::Run> {
        crate::speeds::at_context(&self.endpoint, &self.model, one.context).map(|rate| {
            crate::loadout::Run {
                tokens_per_second: rate,
            }
        })
    }
}

/// What cache a running Ollama is holding, from what Epoch started it with.
///
/// **Read from the preference, and honest that it is a claim about the start.** Ollama does not
/// publish the value, and a server the user started themselves in a terminal may be holding
/// something else entirely. This is what Epoch asked for; a curve says so in those words rather
/// than asserting what the server is doing.
pub fn cache_asked_for(pref: &Preference) -> crate::loadout::Cache {
    if pref.compressed_cache {
        crate::loadout::Cache::Q8_0
    } else {
        crate::loadout::Cache::F16
    }
}

/// The command that starts a runtime, carrying what the user chose.
///
/// **Two spellings, and they are not correct for the same reader.** The command is handed to a
/// terminal — `cmd /k` on Windows, `do script` in Terminal.app on a Mac — so the environment is
/// set the way that shell sets it. This codebase has already paid once for a line that was
/// correct `cmd` and a parse error in `zsh` (11.22).
pub fn start_command_with(runtime: Runtime, pref: &Preference) -> Option<String> {
    let command = start_command(runtime)?;
    let told = pref.told_to(runtime);
    if told.is_empty() {
        return Some(command);
    }
    Some(if cfg!(windows) {
        // `set "VAR=value"` quotes the pair rather than the value, which is how `cmd` avoids
        // taking a trailing space into the variable.
        let sets: Vec<String> = told
            .iter()
            .map(|(name, value)| format!("set \"{name}={value}\""))
            .collect();
        format!("{}&&{command}", sets.join("&&"))
    } else {
        let sets: Vec<String> = told
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        format!("{} {command}", sets.join(" "))
    })
}

/// Load one model, and answer with the program's own words if it will not.
///
/// **This is what earns the compressed cache the right to be kept.** A quantized V cache needs
/// flash attention; flash attention belongs to the backend; and on a backend without it nothing
/// loads — measured, `llama_init_from_model: quantized V cache requires flash_attn to be
/// enabled`, and the refusal reaches the HTTP client as a 500 with that sentence in it. So the
/// setting is never declared to work: it is switched on, proved, and turned back off if the
/// proof fails.
///
/// A tiny window on purpose. This is asking *does it load*, and a load is what costs the time.
pub fn prove_the_cache(endpoint: &str, model: &str) -> Result<(), String> {
    let asked = ureq::builder()
        .timeout_connect(std::time::Duration::from_millis(400))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .post(&format!("{endpoint}/api/generate"))
        .send_json(serde_json::json!({
            "model": model,
            "prompt": "hi",
            "stream": false,
            "options": { "num_ctx": 1024 },
        }));
    match asked {
        Ok(_) => Ok(()),
        // The server's own sentence, kept whole. It is the only true account of why, and a
        // rewritten one would be Epoch guessing about somebody else's refusal.
        Err(ureq::Error::Status(_, said)) => Err(said
            .into_string()
            .map(|body| read_complaint(&body))
            .unwrap_or_else(|_| "it would not load, and said nothing.".to_owned())),
        Err(why) => Err(format!("{}", why)),
    }
}

/// The message out of a runtime's error body, or the body itself.
fn read_complaint(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|it| it.get("error")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| body.trim().to_owned())
}

pub fn start_command(runtime: Runtime) -> Option<String> {
    let at = runtime
        .binaries()
        .iter()
        .find_map(|name| found_in(name, runtime.winget_package(), runtime.program_folder()))?;
    let program = format!("\"{}\"", at.display());

    Some(match runtime {
        Runtime::Ollama => format!("{program} serve"),
        // `lms server start`, from its own `--help`. Starting the application instead would
        // leave the server switched off, which is the state this button exists to change.
        Runtime::LmStudio if at.file_stem().is_some_and(|s| s == "lms") => {
            format!("{program} server start")
        }
        // Only the application is here — its CLI arrives the first time it is run, so running
        // it is exactly the useful thing to do.
        Runtime::LmStudio => program,
        // `llama serve` is the wrapper and needs a model; `llama-server` is the server and does
        // not. Preferring the server here is why this is not `serve_command`.
        Runtime::LlamaCpp => match found_in("llama-server", runtime.winget_package(), None) {
            Some(server) => format!("\"{}\"", server.display()),
            None => format!("{program} serve"),
        },
    })
}

/// The command that would serve one model, for a runtime that can be told to.
///
/// **llama.cpp only, and that is not an omission.** `llama serve -hf <repo>:<QUANT>` fetches the
/// GGUF and starts an OpenAI-compatible server in one step — the form Hugging Face's own *"How
/// to use from llama.cpp"* panel gives, which is where this was read rather than remembered.
///
/// LM Studio has no equivalent worth inventing: its server is started from the application, and
/// a command guessed at here would be a control that does nothing on most machines. `None` says
/// so, and the surface offers what it can instead.
pub fn serve_command(runtime: Runtime, pull: &str) -> Option<String> {
    if runtime != Runtime::LlamaCpp {
        return None;
    }
    // `hf.co/` is Ollama's spelling of a repository; llama.cpp wants the bare id, with the
    // quantisation after a colon exactly as it is published.
    let bare = pull
        .trim()
        .strip_prefix("hf.co/")
        .or_else(|| pull.trim().strip_prefix("huggingface.co/"))?;

    /*
        **Named after the binary that is actually here.**

        `llama serve` is the wrapper the llama.app installer puts down, and it is the form
        Hugging Face's own panel gives — the one this was read from rather than remembered. A
        build from source or from a package manager may leave only `llama-server`, which takes
        the same `-hf` but is its own program.

        Deriving it from what was found means the command names something that exists on *this*
        machine. Offering `llama serve` on a machine that has only `llama-server` would be the
        plausible-looking command that does nothing, which is the failure this file opens by
        naming.
    */
    // The full path, because these are usually not on the PATH: winget leaves them inside its
    // own package directory. A bare name in a terminal that cannot resolve it is the same
    // failure as a command that does not exist.
    let package = Runtime::LlamaCpp.winget_package();
    let program = found_in("llama", package, None)
        .map(|at| format!("\"{}\" serve", at.display()))
        .or_else(|| {
            found_in("llama-server", package, None).map(|at| format!("\"{}\"", at.display()))
        })?;
    Some(format!("{program} -hf {bare}"))
}

/// One model Ollama already has on disk, as a file another runtime can open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Weights {
    /// What a person recognises it by — `qwen3:14b`, or the file's own name.
    pub name: String,
    /// Which shelf it is on, in words: `Ollama` or `Saved here`.
    ///
    /// **Two shelves, and they behave differently**, which is why this is on screen rather than
    /// implied. A model pulled by Ollama is usable by Ollama the moment it lands; a GGUF saved
    /// by the Workshop is a file, and until something opens it, it is a file nobody is using.
    pub from: String,
    /// The file itself.
    pub path: String,
    /// How big it is, so a machine can be asked whether it fits before it is loaded.
    pub bytes: u64,
    /// The vision projector beside it, when the model has one.
    ///
    /// **A vision model is two files, and lending one of them lends a model that cannot see.**
    /// Measured 2026-08-21: `gemma4:12b` is a 7381 MB `image.model` layer *and* a 175 MB
    /// `image.projector` layer. The shelf linked only the first, and LM Studio answered
    /// `gemma4-12b does not support image inputs` — about a model that had been describing
    /// pictures through Ollama all week.
    ///
    /// With the projector linked beside it under `mmproj-<name>.gguf`, the same request through
    /// the same server answered *"A man wearing a black leather jacket and tie reaches out his
    /// hand toward the camera"*, which is what the picture is.
    ///
    /// `None` for a text-only model and for a GGUF found on disk: a file has no manifest to say
    /// it has a second half.
    pub sees_with: Option<String>,
}

/// Every model Ollama has pulled, as GGUF files.
///
/// ## Why this is possible at all
///
/// **Ollama stores unmodified GGUF.** Measured rather than assumed: the layer a manifest marks
/// `application/vnd.ollama.image.model` is a blob whose first four bytes are `GGUF`, and
/// `llama-server -m <blob>` loaded it — 14.7B parameters, `Q4_K - Medium`, `n_ctx_train` 40960,
/// and it answered a chat completion in 2.4 s on the card. Nothing is converted, copied or
/// re-downloaded.
///
/// That matters because the alternative was pulling the same 9 GB twice to try a model on a
/// second runtime, on a machine that has to hold both copies.
///
/// ## Read from the manifests, never from the directory
///
/// `blobs/` holds templates, licences and parameter files beside the weights, all named by
/// digest and indistinguishable from each other. The manifest is what says which blob is the
/// model, so it is what is read — the same rule the deletion work follows: an enumerated list,
/// never a walk.
///
/// An empty list is honest: Ollama is not installed here, or has pulled nothing.
pub fn ollama_weights() -> Vec<Weights> {
    let Some(home) = home() else {
        return Vec::new();
    };
    let models = home.join(".ollama").join("models");
    let mut found = Vec::new();
    // `manifests/<registry>/<namespace>/<name>/<tag>` — four levels, and the last is a file.
    walk_manifests(
        &models.join("manifests"),
        &mut Vec::new(),
        &mut |parts, file| {
            let Ok(raw) = std::fs::read_to_string(file) else {
                return;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
                return;
            };
            let Some(layers) = value.get("layers").and_then(|l| l.as_array()) else {
                return;
            };
            let Some(model) = layers.iter().find(|layer| {
                layer.get("mediaType").and_then(|m| m.as_str())
                    == Some("application/vnd.ollama.image.model")
            }) else {
                return;
            };
            let Some(digest) = model.get("digest").and_then(|d| d.as_str()) else {
                return;
            };
            let blob = models.join("blobs").join(digest.replace(':', "-"));
            if !blob.is_file() {
                return;
            }

            // `library/qwen3` is what Ollama shows as `qwen3`; anything else keeps its namespace,
            // because `unsloth/x` and `library/x` are two different models.
            let name = match parts {
                [_registry, namespace, rest @ ..] if namespace == "library" => rest.join("/"),
                [_registry, rest @ ..] => rest.join("/"),
                _ => return,
            };
            // The other half of a vision model, when there is one. Same manifest, same enumeration —
            // never a guess from a filename, because a blob's name is its digest.
            let sees_with = layers
                .iter()
                .find(|layer| {
                    layer.get("mediaType").and_then(|m| m.as_str())
                        == Some("application/vnd.ollama.image.projector")
                })
                .and_then(|layer| layer.get("digest").and_then(|d| d.as_str()))
                .map(|digest| models.join("blobs").join(digest.replace(':', "-")))
                .filter(|blob| blob.is_file())
                .map(|blob| blob.display().to_string());

            let tag = file.file_name().map(|n| n.to_string_lossy().into_owned());
            found.push(Weights {
                sees_with,
                name: match tag {
                    Some(tag) => format!("{name}:{tag}"),
                    None => name,
                },
                from: "Ollama".to_owned(),
                bytes: model.get("size").and_then(|s| s.as_u64()).unwrap_or(0),
                path: blob.display().to_string(),
            });
        },
    );
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Walk the manifest tree, calling back with the path parts and the file.
///
/// Depth is not assumed: a registry, a namespace and a name today, and Ollama has changed that
/// layout before. What is assumed is that a *file* at the bottom is a manifest, which the
/// caller verifies by reading it.
fn walk_manifests(
    dir: &std::path::Path,
    parts: &mut Vec<String>,
    found: &mut impl FnMut(&[String], &std::path::Path),
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            parts.push(entry.file_name().to_string_lossy().into_owned());
            walk_manifests(&path, parts, found);
            parts.pop();
        } else if path.is_file() {
            found(parts, &path);
        }
    }
}

/// Every GGUF sitting in one directory, as models another runtime can open.
///
/// ## Why this exists
///
/// The Workshop has two ways to fetch a model and they do not land in the same place. `PULL`
/// hands it to Ollama, which files it and can use it immediately. `SAVE THE FILE` runs
/// `hf download` into the vault — and that produced a GGUF **nothing was looking at**: not
/// Ollama, which only knows its own store, and not llama.cpp or LM Studio, which were only ever
/// offered Ollama's blobs.
///
/// A download that lands somewhere nothing can reach is a download that did not happen, and it
/// is the same failure as a Provider nobody can be assigned to.
///
/// Shallow by one level: `hf download` writes `<repo>/<file>.gguf`, and a full recursive walk
/// of somebody's vault is work this has no reason to do.
pub fn gguf_in(dir: &std::path::Path) -> Vec<Weights> {
    let mut found = gguf_files(dir);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten().filter(|e| e.path().is_dir()) {
            found.extend(gguf_files(&entry.path()));
        }
    }
    found = pair_up(found);
    found.sort_by(|a, b| a.name.cmp(&b.name));
    found
}

/// Take the projectors out of the list and give them to the models they belong to.
///
/// ## Why this could not be done before, and can be now
///
/// `gguf_files` used to set `sees_with: None` for everything and say so: *guessing at
/// `mmproj-*.gguf` here would be inventing a pairing nobody stated*. That was right — a filename
/// is a claim (ADR-0024) — and it meant a saved vision model arrived blind. The repository that
/// forced the issue ships `Qwen3.8-27B-Uncensored-IQ2_M.gguf` beside
/// `Qwen3.8-27B-Uncensored-vision-f16.gguf`, and nothing in either name is a fact.
///
/// A GGUF header is. Measured on this machine:
///
/// ```text
/// Qwen3.8-27B-Uncensored-vision-f16   architecture = clip, has_vision_encoder = true,
///                                     vision.projection_dim = 5120
/// Qwen3.8-27B-Uncensored-IQ2_M        architecture = qwen35, embedding_length = 5120
/// qwen3-14b                           architecture = qwen3,  embedding_length = 5120
/// ```
///
/// ## Two facts, because one of them has ties
///
/// The width is what a projector hands its model and it **filters** — a projector at 5120 cannot
/// belong to a model at 3840. It cannot **identify**: `qwen3-14b` is also 5120 and has no eyes
/// at all. *A default is only safe where the measurement is a comparison*, and this measurement
/// has ties.
///
/// So the width has to agree **and** the two files have to have been published together, which
/// is what a shared filename stem says. The filename is doing the job it can honestly do — *these
/// arrived as a set* — while the header decides whether that set makes sense. Neither alone
/// would be enough, and a pairing that fails either test is simply not made: the model stays
/// blind, which is what it was.
fn pair_up(found: Vec<Weights>) -> Vec<Weights> {
    let read = |one: &Weights| crate::gguf::read(std::path::Path::new(&one.path)).ok();

    let eyes: Vec<(String, Option<i128>, String)> = found
        .iter()
        .filter_map(|one| {
            let header = read(one)?;
            header
                .is_projector()
                .then(|| (one.path.clone(), header.width(), one.name.clone()))
        })
        .collect();
    if eyes.is_empty() {
        return found;
    }

    found
        .into_iter()
        .filter(|one| !eyes.iter().any(|(path, _, _)| *path == one.path))
        .map(|mut one| {
            let Some(header) = read(&one) else { return one };
            if header.is_projector() {
                return one;
            }
            let width = header.width();
            one.sees_with = eyes
                .iter()
                .find(|(_, theirs, name)| {
                    // Both, and in this order: the cheap comparison first, the honest one second.
                    theirs.is_some() && *theirs == width && arrived_together(&one.name, name)
                })
                .map(|(path, _, _)| path.clone());
            one
        })
        .collect()
}

/// Whether two files were published as a set, judged by the stem they share.
///
/// Deliberately a **claim** and used only as one — it narrows a set the header has already
/// approved, and never widens it. `Qwen3.8-27B-Uncensored-IQ2_M` and
/// `Qwen3.8-27B-Uncensored-vision-f16` share twenty-three characters; `qwen3-14b` and that
/// projector share five.
///
/// The threshold is a real length rather than a proportion, because a proportion lets two short
/// names agree by accident: `q4` and `q8` are 50% alike.
pub(crate) fn arrived_together(model: &str, projector: &str) -> bool {
    const ENOUGH: usize = 8;
    let shared = model
        .chars()
        .zip(projector.chars())
        .take_while(|(a, b)| a.eq_ignore_ascii_case(b))
        .count();
    shared >= ENOUGH
}

/// The `.gguf` files directly inside one directory.
fn gguf_files(dir: &std::path::Path) -> Vec<Weights> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gguf"))
        })
        .map(|entry| Weights {
            /*
                **The file's identity, which is its stem plus whatever its header says
                distinguishes it** — never the stem alone, and never a path.

                Measured 2026-08-31: `unsloth/Qwen3.6-35B-A3B-GGUF` and
                `unsloth/Qwen3.6-35B-A3B-MTP-GGUF` publish `Qwen3.6-35B-A3B-UD-IQ4_XS.gguf` and
                the two files are 17,730,509,792 and 18,209,036,576 bytes. Named by stem they
                were one deck row, one tuning entry, one loadout, one set of benchmark results
                and one shelf link — two different models sharing everything.

                A file with nothing distinguishing keeps exactly the stem it had, so nothing
                measured before this existed was orphaned by it (`artifact`).
            */
            name: {
                let id = crate::artifact::Identity::read(&entry.path()).id();
                if id.is_empty() {
                    "a model".to_owned()
                } else {
                    id
                }
            },
            from: "Saved here".to_owned(),
            bytes: entry.metadata().map(|m| m.len()).unwrap_or(0),
            path: entry.path().display().to_string(),
            // A file has no manifest to say it has a second half. A saved vision model whose
            // projector was downloaded beside it is a case for the day somebody hits it —
            // guessing at `mmproj-*.gguf` here would be inventing a pairing nobody stated.
            sees_with: None,
        })
        .collect()
}

/// What stocking the shelf did.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stocked {
    /// How many models the shelf now offers.
    pub models: usize,
    /// What could not be linked, and why. Never fatal: a shelf with most of the models on it is
    /// better than no shelf.
    pub problems: Vec<String>,
    /// Where it is, so a command can name it and a person can look at it.
    pub at: String,
}

/// Put every model this machine has on one shelf, under names a person reads.
///
/// ## Why a shelf rather than a choice
///
/// llama.cpp was being started *holding one model*, picked from a dropdown — which is a
/// question nobody wants to answer before they know what they are going to ask. Measured
/// 2026-08-21: `llama-server --models-dir <shelf>` is a **router**. It reports every model in
/// the directory at `/v1/models`, loads one only when a request names it, and leaves the rest
/// merely available:
///
/// ```text
/// GET /v1/models   ->  gemma4-12b, qwen3-14b
/// POST /v1/chat/completions {"model":"gemma4-12b"}  ->  "Blue", 21.2s (loaded on demand)
/// GET /v1/models   ->  gemma4-12b, qwen3-14b        (the other is still just available)
/// ```
///
/// ## Hard links, so the shelf costs nothing
///
/// Every entry is a second name for a file that already exists — Ollama's blob, or a GGUF the
/// Workshop saved. The two names share the same blocks, so a shelf of five models adds bytes
/// for none of them, and deleting either name leaves the other working.
///
/// **The extension is the whole point**, twice over: llama.cpp's router looks for `.gguf` and so
/// does LM Studio, while Ollama's blobs are named by digest and have no extension at all. A link
/// named after the digest is created successfully and then never appears.
///
/// ## Enumerated, and it prunes
///
/// A model removed from Ollama leaves a link pointing at nothing, and a shelf that only ever
/// grew would offer models that are gone. So links Epoch made and no longer recognises are
/// removed — and **only `.gguf` files are ever touched**, in a directory Epoch created, because
/// this deletes things.
pub fn stock_shelf(
    shelf: &std::path::Path,
    models: &[Weights],
    how: &dyn Fn(&Weights) -> crate::loadout::Loadout,
    told: &dyn Fn(&Weights) -> crate::tuning::Tuning,
) -> Stocked {
    let mut problems = Vec::new();
    if let Err(err) = std::fs::create_dir_all(shelf) {
        return Stocked {
            models: 0,
            problems: vec![format!(
                "cannot make the shelf at {}: {err}",
                shelf.display()
            )],
            at: shelf.display().to_string(),
        };
    }

    let mut wanted = std::collections::BTreeSet::new();
    for model in models {
        let clean = shelf_name(&model.name);
        let file = format!("{clean}.gguf");
        let link = shelf.join(&file);
        wanted.insert(file);
        /*
            **Link what the path leads to, never the path.**

            A model in the Hugging Face cache is reached through a symlink — `hub/models--…/
            snapshots/…/file.gguf` points at `blobs/<sha>`. Hard-linking that on Windows links the
            *reparse point*, and what lands on the shelf is a 76-byte file that llama.cpp
            faithfully reports as `failed to load`. Measured: exactly that happened to
            `gemma-3-270m`, and it was invisible until a benchmark asked the router for the shelf
            copy by name.

            The size check is what makes it self-repairing. `!link.is_file()` was the whole guard,
            and a broken link *is* a file — so once one existed nothing would ever replace it.
            Comparing against the source is the cheap way to notice: two hard links to one file
            cannot disagree about its length, so a mismatch means this is not that.
        */
        let real = std::fs::canonicalize(&model.path)
            .unwrap_or_else(|_| std::path::PathBuf::from(&model.path));
        let want = std::fs::metadata(&real).map(|it| it.len()).ok();
        let have = std::fs::metadata(&link).ok().map(|it| it.len());
        if have.is_none() || (want.is_some() && have != want) {
            if have.is_some() {
                let _ = std::fs::remove_file(&link);
            }
            if let Err(err) = std::fs::hard_link(&real, &link) {
                problems.push(format!(
                    "{} could not go on the shelf ({err}). A hard link needs both places on one drive.",
                    model.name
                ));
            }
        }

        // **Both halves, or the model arrives blind.** A vision model is weights plus a
        // projector, and linking only the weights produced a `gemma4-12b` that llama.cpp and LM
        // Studio load happily and then refuse a picture to — while the same file had been
        // answering questions about pictures through Ollama all week.
        //
        // `mmproj-<name>.gguf` is the name both look for beside a model.
        if let Some(projector) = &model.sees_with {
            let file = format!("mmproj-{clean}.gguf");
            let link = shelf.join(&file);
            wanted.insert(file);
            // The same two corrections as the weights above: follow the symlink, and notice a
            // link that no longer matches what it was made from.
            let real = std::fs::canonicalize(projector)
                .unwrap_or_else(|_| std::path::PathBuf::from(projector));
            let want = std::fs::metadata(&real).map(|it| it.len()).ok();
            let have = std::fs::metadata(&link).ok().map(|it| it.len());
            if have.is_none() || (want.is_some() && have != want) {
                if have.is_some() {
                    let _ = std::fs::remove_file(&link);
                }
                if let Err(err) = std::fs::hard_link(&real, &link) {
                    problems.push(format!(
                        "{} is on the shelf without its eyes ({err}). It will answer questions and refuse pictures.",
                        model.name
                    ));
                }
            }
        }
    }

    // Prune. Only `.gguf`, only here, and only what is not wanted any more — a model removed
    // from Ollama would otherwise stay on offer as a link pointing at nothing.
    if let Ok(entries) = std::fs::read_dir(shelf) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.to_lowercase().ends_with(".gguf") && !wanted.contains(&name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    if let Err(why) = write_presets(shelf, models, how, told) {
        problems.push(why);
    }

    Stocked {
        models: wanted.len(),
        problems,
        at: shelf.display().to_string(),
    }
}

/// What a shelf calls its presets file.
///
/// Beside the models rather than in a config directory: the shelf is one thing Epoch made and
/// can rebuild, and a preset that outlived the shelf it describes would point at links that are
/// gone.
pub fn presets_file(shelf: &std::path::Path) -> std::path::PathBuf {
    shelf.join("presets.ini")
}

/// Tell llama.cpp's router which models have eyes.
///
/// ## Why linking the projector was not enough
///
/// Measured 2026-08-21, all four of these in one sitting:
///
/// ```text
/// shelf without mmproj        -> "image input is not supported - hint: ... provide the mmproj"
/// shelf with mmproj-<name>.gguf beside it, no preset
///                             -> the same refusal; the router does not pair by filename
///                                (it also did not list it as a fifth model, which is correct)
/// preset written as CLI flags -> "failed to parse server config file"
/// preset written as key = value
///                             -> "Loaded 1 custom model presets", and the same picture came
///                                back as "A man in a leather jacket and a tie reaches out his
///                                hand toward the viewer."
/// ```
///
/// LM Studio pairs them by name and llama.cpp does not, so the projector is linked *and* named
/// here. Two runtimes, two conventions, one shelf.
///
/// ## Now every model gets a section, and that is the point
///
/// It used to be only the ones with eyes, on the reasoning that a preset for every model merely
/// restates what `--models-dir` already found. That was true while every model was loaded the
/// same way. It is not any more: how much context a model gets and whether its KV cache is
/// compressed are **per-model** answers, opposite for two models on the same card
/// ([`crate::loadout`]), and a preset is the only place llama.cpp will take them.
///
/// Measured 2026-08-26, and it is the whole reason this works:
///
/// ```text
/// parent passes -c 16384, preset says ctx-size = 8192   -> child spawned with --ctx-size 16384
/// parent passes no -c,     preset says ctx-size = 8192   -> child spawned with --ctx-size 8192
/// ```
///
/// So `router_command` stops passing `-c` and `--cache-type-*` at all. What the parent still
/// passes is what belongs to the machine rather than to a model: the port, the shelf, one at a
/// time, and the fitter's margin.
fn write_presets(
    shelf: &std::path::Path,
    models: &[Weights],
    how: &dyn Fn(&Weights) -> crate::loadout::Loadout,
    // What each model is *told*, as opposed to what was measured about it: flash attention and
    // speculative decoding. Separate from `how` because one is a choice and the other is a
    // reading, and a preset carries both without confusing them (`tuning.rs`).
    told: &dyn Fn(&Weights) -> crate::tuning::Tuning,
) -> Result<(), String> {
    let mut ini = String::new();
    for model in models {
        let clean = shelf_name(&model.name);
        let one = how(model);
        // The shelf's own links, never the blob: the preset describes this shelf, and a path
        // into Ollama's store would make the file wrong the moment the model is unpulled.
        ini.push_str(&format!(
            "[{clean}]\nmodel = {}\n",
            shelf.join(format!("{clean}.gguf")).display(),
        ));
        if model.sees_with.is_some() {
            ini.push_str(&format!(
                "mmproj = {}\n",
                shelf.join(format!("mmproj-{clean}.gguf")).display(),
            ));
        }
        ini.push_str(&format!("ctx-size = {}\n", one.context));
        // Whatever else this model was told, in llama.cpp's own key names. Nothing is
        // written for a value nobody chose, so the program's own defaults stay reachable.
        ini.push_str(&told(model).lines());
        // The cache keys are written only when something is being asked for, so a model on the
        // full-precision cache carries llama.cpp's own default rather than a spelling of it.
        if one.cache == crate::loadout::Cache::Q8_0 {
            ini.push_str("cache-type-k = q8_0\ncache-type-v = q8_0\n");
        }
        ini.push('\n');
    }

    let path = presets_file(shelf);
    if ini.is_empty() {
        // An empty shelf. Remove a file left by one that used to have models on it, rather than
        // leaving it naming links that were pruned.
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    std::fs::write(&path, ini).map_err(|err| {
        format!("the shelf is stocked, but llama.cpp was not told how to load from it ({err})")
    })
}

/// The command that starts llama.cpp knowing about **every** model on the shelf.
///
/// `--models-max 1` deliberately: the router will hold several at once if allowed, and on a
/// consumer card two large models at once is the machine swapping. One at a time, swapped on
/// demand, is what somebody moving between models actually wants.
pub fn router_command(shelf: &std::path::Path) -> Option<String> {
    let program = found_in("llama-server", Runtime::LlamaCpp.winget_package(), None)?;
    // `--models-preset` only when there is one, so a shelf of text models starts the server it
    // always started rather than failing on a file that is not there.
    let presets = presets_file(shelf);
    let eyes = if presets.is_file() {
        format!(" --models-preset \"{}\"", presets.display())
    } else {
        String::new()
    };
    // **No `-c` and no `--cache-type-*`.** Both are per-model answers now and live in the
    // preset; measured, anything the parent passes overrides what the preset asks for, so
    // passing them here would silently flatten every model back to one setting.
    Some(format!(
        "\"{}\" --models-dir \"{}\"{eyes} --port {} --models-max 1{} --sleep-idle-seconds {}",
        program.display(),
        shelf.display(),
        Runtime::LlamaCpp.usual_port(),
        fit_margin(),
        IDLE_SLEEP_SECONDS,
    ))
}

/// How much memory to hold back from llama.cpp's fitter, and **whether to say anything at all**.
///
/// ## A card's answer, given to a machine with no card
///
/// On CUDA this is decisive: the default of 1024 MiB held back cost half the speed on a nearly
/// full card, 16.6 tok/s against 33.7, so Epoch says 128.
///
/// On Apple Silicon the same number **breaks the server**. Measured on an M2 with `gemma4-12b`
/// and nothing else resident: with `-fitt 128` the model loads and the first decode dies with
///
/// ```text
/// error: Insufficient Memory (00000008:kIOGPUCommandBufferCallbackErrorOutOfMemory)
/// srv send_error: task id = 0, error: Compute error.
/// ```
///
/// and the request comes back `{"code":500,"message":"Compute error."}` in 40 ms. With the flag
/// left off, the same model on the same machine answers at **10.69 tok/s**. Reproduced twice.
///
/// The reason is what `Machine::unified` exists to say: there is no separate device memory here.
/// A margin is *how much of the card to leave for everything else*, and on a machine where the
/// GPU and the system share one pool, 128 MiB is not a cautious margin — it is an instruction to
/// fill memory the OS is also using.
///
/// So the honest answer on a unified machine is **silence**, and llama.cpp's own default stands.
/// The same shape as the vendor advice this file already gives: a value derived from what was
/// measured, and nothing at all where Epoch cannot measure a better one.
fn fit_margin() -> String {
    if crate::machine::Machine::measure().unified {
        String::new()
    } else {
        format!(" -fitt {FIT_MARGIN_MIB}")
    }
}

/// How much of the card to leave alone, in MiB — and why `-ngl` is no longer here.
///
/// ## Epoch says what the turn needs; llama.cpp decides how to fit it
///
/// `-ngl 99` was Epoch answering a question it cannot measure: *how many layers fit on this
/// machine's card*. llama.cpp measures exactly that, and says so when it is overruled:
///
/// ```text
/// W common_fit_params: failed to fit params to free device memory:
///   n_gpu_layers already set by user to 99, abort
/// ```
///
/// Asking the program (`--help`) rather than remembering: **`--fit` adjusts *unset* arguments to
/// fit device memory, and it is on by default.** Setting `-ngl` was the only thing switching it
/// off. So the flag is gone and the fitter does its job.
///
/// ## The context stays Epoch's to state, and this is measured
///
/// `-fitc` looked like the elegant answer — a *floor* the fitter may raise. It is a floor and the
/// fitter maximises against it, so with room to spare it spends the whole card on KV cache:
/// `gemma4-12b` fell from 45 tok/s to **9.3**. More context is not free. Epoch knows what a turn
/// needs (`ROUTER_CONTEXT`) and llama.cpp does not, so Epoch keeps stating it with `-c`.
///
/// ## Why the margin is 128 and not the default 1024
///
/// The fitter reserves `--fit-target` MiB per device and offloads layers to the CPU to keep it.
/// A gigabyte of a twelve-gigabyte card is a lot to hold back. Measured on the 27B, same prompt,
/// warm:
///
/// ```text
/// -fitt 1024 (default)   16.6 tok/s    1079 MiB left unused
/// -fitt  384             24.7 tok/s     384 MiB
/// -fitt  256             28.6 tok/s     332 MiB
/// -fitt  128             33.7 tok/s     199 MiB
/// ```
///
/// ## What the whole change costs and buys, measured across the shelf
///
/// ```text
///                        weights   -ngl 99 (before)   fitter + margin 128
/// gemma4-12b              7.4 GB        44.9                45.1
/// qwen3-14b               9.3 GB        40.9                40.9
/// Qwen3.8-27B-IQ2_M      10.6 GB        33.8                33.6
/// qwen3.8 (Q4)           16.8 GB         0.92                6.2      x6.7
/// ```
///
/// Nothing changes for a model that fits, and **a model too big for the card stops collapsing
/// and starts degrading** — 6.7x, because the fitter splits it deliberately instead of being
/// told to force every layer onto memory that cannot hold them. That is the part which makes
/// this adapt to a card Epoch has never seen: the decision is taken by the thing that measured
/// the card, on the card it measured.
///
/// **Not measured on Apple.** Metal has no separate device memory to hold back, so the right
/// margin there is a question this machine cannot answer. Parked in
/// `docs/Build/Minecraft.md` rather than guessed at.
const FIT_MARGIN_MIB: u32 = 128;

// **A guard on the constant, not on a run.** These were `assert!` inside tests, which clippy
// reads correctly as assertions that can never fail: both sides are known at compile time. That
// is not a reason to delete them -- they exist so that changing the number breaks something --
// it is a reason to make them what they were trying to be. A `const` assertion fails the
// **build**, which is stricter than failing a test and arrives at whoever changed the number
// rather than at whoever ran the suite next.

// The default of 1024 costs half the speed on a nearly full card: 1024 MiB held back read
// 16.6 tok/s where 128 gave 33.7.
const _: () = assert!(FIT_MARGIN_MIB <= 256);

// `KV_CACHE` was here - `--cache-type-k q8_0 --cache-type-v q8_0` on the router, for every model
// on the shelf. It was a real fix (`qwen3-14b` answered at 5.3 tok/s without it and 40.9 with) and
// it was the right answer for one model and the wrong one for another on the same card: gemma
// pays 3% for a cache it never needed. It is a per-model preset key now, and the measurements
// that justified it live in `loadout`, where the choice is made.

// `ROUTER_CONTEXT` was here - one context size for every model, sized to a turn measured at 8235
// tokens. Both halves of that went stale: turns are 11789 and 13209 tokens now (re-measured
// through the model's own tokenizer), and one size cannot serve gemma at 64k and the 27B at 16k.
// `loadout::A_REAL_TURN` carries the measurement and says when it was taken; the size is a
// per-model preset key.

/// A lifeboat, not the way models are released.
///
/// `turn::drive` unloads through `POST /models/unload` the moment a turn ends, which is exact.
/// This covers the case that call never happens — Epoch crashing, or being killed mid-turn —
/// so llama.cpp behaves like the other two instead of holding a card until the machine reboots.
///
/// Measured: `--sleep-idle-seconds 30` took the process from 8812 MB to 126 MB and the card from
/// 10736 MiB to 1937 MiB, and the router passes the flag down to every child it starts.
///
/// ## Why it is a day and not five minutes
///
/// It was 300, which is a sensible idle policy and a **second policy competing with Epoch's**.
/// Once somebody can hold a brain on the card from the conversation, the router's own timer
/// contradicts them: measured, KEEP was on, the lamp went green, and llama.cpp put the model to
/// sleep five minutes later anyway — the next turn paid 15.7 s of prompt evaluation for 8603
/// tokens where a warm one paid 906 ms.
///
/// The router offers no other lever: `--help` has this flag and nothing per request, and the
/// router serves no load or keep route (asked — `/models/load`, `/models/keep`, both 404; only
/// `/models/unload` exists, and Epoch already calls it). So the flag has to be longer than any
/// hold Epoch will ask for, and a hold has a ceiling of a day (`settings::HELD_HOURS`).
///
/// Nothing is given up by making it long, because it was never the release: `turn::drive`
/// unloads explicitly the moment an unheld turn ends. This is only the lifeboat, and a lifeboat
/// that launches while the ship is still sailing is worse than a slow one.
const IDLE_SLEEP_SECONDS: u32 = 24 * 60 * 60;

// At 300 the router put a held model to sleep five minutes after the user asked to keep it -- a
// second policy quietly outvoting the one with a button on screen. It must outlast any hold
// Epoch will ask for.
const _: () = assert!(IDLE_SLEEP_SECONDS >= 24 * 60 * 60);

/// Make one of Ollama's models visible to LM Studio, without a second copy of it.
///
/// ## Measured, and one of the measurements is a trap
///
/// `lms import <file>` exists and takes `--copy`, `--hard-link` and `--symbolic-link` — **and
/// its default is to _move_ the file.** Moving an Ollama blob takes it out of Ollama's store,
/// so the obvious command is the one that quietly breaks the runtime the file came from. That
/// is why nothing here shells out to it: the link is made directly, and a flag nobody passed
/// cannot destroy anything.
///
/// The second measurement is the one that made it work at all. A hard link named after the
/// digest was created successfully and `lms ls` did not list it — LM Studio scans for `.gguf`,
/// and Ollama's blobs have no extension. Renaming the same link to `qwen3-14b.gguf` made it
/// appear immediately: `qwen3-14b · 14B · qwen3 · 9.28 GB · Local`, and it loaded in 13.9 s.
///
/// ## A hard link, not a copy and not a symlink
///
/// A hard link costs nothing — the two names share the same blocks — and needs no elevation,
/// which a symbolic link does on Windows. It requires both paths on one volume; if they are
/// not, that is reported rather than silently copied nine gigabytes.
///
/// Deleting either name leaves the other working, which is what makes this safe to offer: LM
/// Studio removing "its" model does not touch Ollama's.
pub fn share_with_lm_studio(
    name: &str,
    file: &str,
    sees_with: Option<&str>,
) -> Result<String, String> {
    let home = home().ok_or("this machine has no home directory")?;
    let clean = shelf_name(name);
    // `ollama/` as the publisher, so the shelf says where it came from and a person can tell
    // these apart from what they downloaded through LM Studio itself.
    let into = home
        .join(".lmstudio")
        .join("models")
        .join("ollama")
        .join(&clean);
    std::fs::create_dir_all(&into).map_err(|err| format!("cannot create {into:?}: {err}"))?;

    // **The extension is the whole point.** LM Studio scans for `.gguf`; a link named after the
    // digest is created successfully and then never appears, which is the worst kind of success.
    let link = into.join(format!("{clean}.gguf"));
    if !link.is_file() {
        std::fs::hard_link(file, &link).map_err(|err| {
            format!(
                "could not link it into LM Studio ({err}). A hard link needs both places on one drive; if they are not, LM Studio's own Import can copy it instead."
            )
        })?;
    }

    // **Its eyes, when it has any.** Measured 2026-08-21: with the weights alone, LM Studio
    // answered `gemma4-12b does not support image inputs` — about a model that describes
    // pictures perfectly well through Ollama, because Ollama keeps the projector as a second
    // layer and pairs the two itself. With `mmproj-<name>.gguf` beside it, the same server
    // described the photograph correctly.
    //
    // A projector that will not link is **not** fatal: a model that answers questions and
    // refuses pictures is worth more than no model, and `stock_shelf` says so where it happens.
    if let Some(projector) = sees_with {
        let eyes = into.join(format!("mmproj-{clean}.gguf"));
        if !eyes.is_file() {
            let _ = std::fs::hard_link(projector, &eyes);
        }
    }

    Ok(link.display().to_string())
}

/// What a shelf calls one of Ollama's models.
///
/// `qwen3:14b` is not a filename on any platform, and the colon is in every Ollama model's name.
/// Its own function so the rule can be held still by a test that touches nothing: a test that
/// checked this by making a link would be a test writing into somebody's real LM Studio.
/// Whether a server that offers `offered` is offering **this** model, and what it calls it.
///
/// ## Two spellings of one model, and Epoch wrote the second one
///
/// A deck row is named the way a person recognises it — `gemma4:12b`, or a file's own stem.
/// Ollama publishes exactly that. llama.cpp's router publishes `gemma4-12b`, because
/// [`shelf_name`] made the link it loads and a colon is not a filename character.
///
/// Measured 2026-08-30 with the router serving this machine's shelf: `gemma4-12b`,
/// `qwen3-14b`, `qwen3.8-latest`, `Qwen3.8-27B-Uncensored-IQ2-M`. Compared by equality, a deck
/// row could never be timed on llama.cpp at all — and the request would have carried the wrong
/// name even if it had matched.
///
/// **Derived, not guessed.** The second spelling is the one Epoch itself wrote when it stocked
/// the shelf, so this asks the same function rather than pattern-matching a name.
/// **The real name first, and the shelf name only if the real one is not there.**
///
/// One `find` over both spellings looks the same and is not: it returns whichever comes first in
/// *the server's* list, which is alphabetical and has nothing to do with which one is right.
/// Measured 2026-08-31 — a router serving a model under both names answered
/// `model name=ggml-org-gemma-3-270m-GGUF-Q8-0 failed to load` for a benchmark, because
/// `ggml-org-…` sorts before `ggml-org/…` and the shelf copy of that particular model was broken.
/// The name the deck knows is a fact; the sanitised one is a fallback, and a fallback that can
/// win a race against the fact is not a fallback.
pub fn offered_as(model: &str, offered: &[String]) -> Option<String> {
    if let Some(exact) = offered.iter().find(|had| had.as_str() == model) {
        return Some(exact.clone());
    }
    let shelved = shelf_name(model);
    offered.iter().find(|had| **had == shelved).cloned()
}

/// A draft or MTP file for one model, if there is one beside it on the shelf.
///
/// **`draft-<name>.gguf`, the same convention `mmproj-<name>.gguf` already uses.** A vision model
/// is weights plus a projector and a speculating model is weights plus a draft; both are a second
/// file that belongs to the first, and one convention for both is one thing to know.
///
/// Looked for rather than declared. Whether a model can use `draft-mtp` is a fact about the files
/// on this machine, and a manifest saying so is one that will eventually lie — the rule ADR-0030
/// arrived at for what a workflow can draw, one shelf over.
///
/// Measured 2026-08-31 and worth recording: the `Qwen3.6-35B-A3B` GGUF on this machine carries no
/// MTP tensors at all, so there is nothing for this to find and the `draft-*` types are simply not
/// offered for it. The MTP head is a separate download.
pub fn draft_beside(shelf: &std::path::Path, model: &str) -> Option<std::path::PathBuf> {
    let clean = shelf_name(model);
    ["draft", "mtp"]
        .into_iter()
        .map(|prefix| shelf.join(format!("{prefix}-{clean}.gguf")))
        .find(|it| it.is_file())
}

pub fn shelf_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

// `serve_file_command` was here — `llama-server -m <one file>`. The router replaced it
// (`router_command`): measured 2026-08-21, `--models-dir` reports every model in a directory and
// loads one only when a request names it, so starting llama.cpp *holding* something was a
// question nobody needed to answer. A second way to start the same program, worse, is not worth
// keeping for the day somebody wants exactly one model — that day is `--models-dir` with a
// shelf of one.

/// Give Ollama a GGUF it did not download, by writing the Modelfile that names it.
///
/// ## Measured on 2026-08-21, and every number here came from the run
///
/// ```text
/// ollama create epoch-test-import -f Modelfile     3m30s
///   copying file sha256:a8ccâ€¦
///   parsing GGUF
///   verifying conversion
///   using existing layer sha256:a8ccâ€¦      <- the important line
///   writing manifest
///   success
///
/// blobs before   46363 MB
/// blobs after    46363 MB
/// the model then answered "Blue." in 18s
/// ```
///
/// **It costs no disk _only_ for a file Ollama already holds**, which is what the run above
/// was. `using existing layer` is that case happening. For a GGUF the Workshop saved it costs
/// **the whole file again** — measured on this machine: the vault's 10,624,771,968 bytes and
/// Ollama's blob both present, 21.2 GB for one model.
///
/// ## And Epoch cannot place that blob itself
///
/// The obvious saving is to hard-link the file into Ollama's store under the name it will look
/// for. Measured, and it does not work: a blob's name **is** the sha256 of its content (both
/// hashed to check), but that content is *not the file Epoch has*. The vault GGUF hashes to
/// `28e0f88e…` and Ollama's copy to `7cb7cedc…` at **exactly the same length**, so
/// `ollama create` rewrites bytes inside the file rather than storing what it was given.
///
/// Writing the manifest by hand instead is worse. It names a `config` blob carrying `renderer`,
/// `parser`, `requires`, `model_family` and `file_type` — all derived by Ollama from the GGUF,
/// and `renderer` decides how the chat template is applied. Epoch guessing those would be
/// inventing how somebody else's model behaves, silently, in a file it does not own.
///
/// So the cost stays, and the honest thing is to **say it before the button is pressed** —
/// which is what [`ollama_import_cost`] is for.
///
/// **It costs three and a half minutes**, because the whole file is hashed to find that out.
/// That is why this returns a command for the user's own terminal rather than blocking a
/// window: the same door as installing and starting, for the same reason â€” the work outlives
/// the button and its progress is somewhere a person can read it.
///
/// The Modelfile is written beside the model, named after it. It is one line, it records what
/// was done, and a second import finds it already there.
/// What importing this file into Ollama will cost, in bytes on the disk.
///
/// The file's own size, because that is exactly what Ollama's copy of it weighs. Said on the
/// control rather than discovered afterwards: it is the difference between somebody choosing to
/// spend ten gigabytes and somebody finding out they did.
pub fn ollama_import_cost(bytes: u64) -> String {
    format!(
        "Ollama keeps its own copy — this adds {:.1} GB and takes a few minutes. The model \
         already works with llama.cpp and LM Studio from where it is.",
        bytes as f64 / 1e9
    )
}

pub fn ollama_import_command(name: &str, file: &str) -> Result<String, String> {
    let path = std::path::Path::new(file);
    let beside = path
        .parent()
        .ok_or("that model is not in a directory this can write to")?;
    let recipe = beside.join(format!("{}.Modelfile", shelf_name(name)));

    // `FROM <absolute path>` is the whole recipe. Anything else here â€” a template, parameters â€”
    // would be Epoch deciding how somebody else's model behaves, which is the Character's job
    // (ADR-0026) and not a file's.
    std::fs::write(&recipe, format!("FROM {file}\n"))
        .map_err(|err| format!("cannot write {recipe:?}: {err}"))?;

    let ollama = found_in("ollama", None, None)
        .ok_or("Ollama is not on this machine, so there is nothing to import into")?;
    Ok(format!(
        "\"{}\" create {} -f \"{}\"",
        ollama.display(),
        shelf_name(name),
        recipe.display()
    ))
}

/// Run a command in the user's **own** terminal, and step back.
///
/// ## Why Epoch does not install anything itself
///
/// The same rule the agents' sign-in follows: *Epoch opens the door; it never holds the key.*
/// An installer asks for elevation, shows a licence, and sometimes asks questions — all of which
/// belong to the person at the machine, in a window they can read and cancel. Epoch's part is
/// removing the need to know a terminal was involved.
///
/// It is also why nothing here reports success. A terminal opened is all this can honestly
/// claim; whether the install worked is answered by [`survey`] afterwards, by measurement.
/// The mark on every console Epoch opens, and the only thing that says one is Epoch's.
///
/// A person's own terminal that happens to be running the same server is theirs, and closing it
/// would be taking a window away from somebody watching it.
pub const OURS: &str = "Epoch - ";

/// Close the consoles Epoch opened for a command naming `install`, and leave everything else.
///
/// **Called after the server inside one has been stopped.** `cmd /k` keeps a window open on
/// purpose — a run that failed and vanished is a run nobody can diagnose — and that argument
/// covers a server that died. It does not cover one Epoch shut down itself: what stays then is an
/// empty prompt in a window the user never opened, over a World that is meant to be a place.
///
/// Two conditions, and both are somebody's work: the command line names **this install**, and it
/// carries [`OURS`], which is there because [`spawn_terminal`] puts the title inside the shell.
pub fn close_our_terminals(install: &str) -> usize {
    #[cfg(windows)]
    {
        let script = format!(
            "$p = Get-CimInstance Win32_Process -Filter \"Name='cmd.exe'\" | \
             Where-Object {{ $_.CommandLine -like '*{}*' -and $_.CommandLine -like '*{}*' }}; \
             if ($p) {{ $p | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}; \
             ($p | Measure-Object).Count }} else {{ 0 }}",
            install.replace('\'', "''"),
            OURS.replace('\'', "''"),
        );
        let Ok(out) = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", &script])
            .output()
        else {
            return 0;
        };
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse()
            .unwrap_or(0)
    }
    // **Not done on a Mac, and said rather than pretended.** `spawn_terminal` there asks
    // Terminal.app to `do script`, which is a tab inside an application the person is also
    // using — closing it is a different act from closing a console Epoch owns, and it wants
    // AppleScript that targets the right window rather than a guess.
    #[cfg(not(windows))]
    {
        let _ = install;
        0
    }
}

pub fn open_in_terminal(title: &str, command: &str) -> Result<(), String> {
    spawn_terminal(title, command)
}

#[cfg(windows)]
fn spawn_terminal(title: &str, command: &str) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    /*
        **The command is wrapped in one more pair of quotes, and it has to be.**

        `cmd /k <string>` strips the *first and last* quote characters when there are more than
        two of them. A command with one quoted path survives that — which is why `ollama serve`
        worked for weeks — and a command with two does not: the opening quote of the program and
        the closing quote of its argument are the ones removed, leaving both paths malformed.

        Measured on 2026-08-21, the same two shapes side by side:

        ```text
        cmd /c "<exe>" -m "<model>" --version    The filename, directory name, or volume
                                                 label syntax is incorrect.
        cmd /c ""<exe>" -m "<model>" --version"  version: 0.1.2-dev (build 10507)
        ```

        Wrapping the whole thing gives `cmd` the pair it is going to strip anyway, so the inner
        quotes reach the program intact. It is the documented behaviour in `cmd /?`, and the bug
        only appeared once a command carried a second quoted argument.

        `start` re-parses too: its first quoted word is taken as the window title, which is why
        the title is passed explicitly rather than letting a path become one.

        `/k` rather than `/c` so the window stays open with whatever the program said in it. A
        run that failed and vanished is a run nobody can diagnose.
    */
    // **The title goes inside the shell, not only on `start`.**
    //
    // `start "Epoch - ComfyUI"` names the window and then exits; the shell that survives carries
    // only the command, so nothing afterwards can tell a console Epoch opened from one the person
    // opened themselves. `title` is a `cmd` builtin, so the marker lives in the surviving
    // process's own command line — which is the same identification-by-command-line that
    // `stop_server` already uses to find the server itself.
    let line = format!("/c start \"{title}\" cmd /k \"title {title}&&{command}\"");
    std::process::Command::new("cmd.exe")
        .raw_arg(&line)
        .spawn()
        .map(|_| ())
        .map_err(|why| format!("a terminal could not be opened ({why})"))
}

#[cfg(target_os = "macos")]
fn spawn_terminal(_title: &str, command: &str) -> Result<(), String> {
    // Terminal.app is present on every Mac, so this is not a guess about somebody's setup the
    // way picking a Linux terminal would be. The command is passed through a JSON string literal
    // so a quote in it cannot end the script.
    let quoted = serde_json::to_string(command).map_err(|why| why.to_string())?;
    let script = format!("tell application \"Terminal\"\nactivate\ndo script {quoted}\nend tell");
    std::process::Command::new("osascript")
        .args(["-e", &script])
        .spawn()
        .map(|_| ())
        .map_err(|why| format!("a terminal could not be opened ({why})"))
}

/// Elsewhere, name the command rather than guess a terminal.
///
/// There is no portable "open a console" on Linux, and picking one would be wrong on most
/// machines. A sentence somebody can paste is honest; a guess that opens nothing is not.
#[cfg(all(not(windows), not(target_os = "macos")))]
fn spawn_terminal(_title: &str, command: &str) -> Result<(), String> {
    Err(format!("run `{command}` in a terminal"))
}

/// Every model this machine has, wherever it came from.
///
/// ## One answer, because two surfaces ask it
///
/// Ollama's store is the obvious half. The other half is whatever `SAVE THE FILE` put in a
/// directory — on the Host that is the vault, on a lent machine it is `EpochServices/models` —
/// and a download that lands somewhere nothing looks at is a download that did not happen.
///
/// The directories differ per surface, so they are given rather than known here. What must not
/// differ is *what counts as a model this machine has*: the Host and a lent machine disagreeing
/// about that is how one of them starts offering a shelf the other cannot explain.
pub fn everything_here(also: &[std::path::PathBuf]) -> Vec<Weights> {
    let mut held = ollama_weights();
    for dir in also {
        held.extend(gguf_in(dir));
    }
    /*
        **And llama.cpp's own cache, which is a third store nobody was reading.**

        Reported from the AMD machine: `llama serve` offering `unsloth/Qwen3.5-9B-GGUF:Q4_K_M`
        while MODELS read *nothing yet* — so TIME IT and MEASURE could not be used on a model
        that was really there. A model fetched with `llama download` lands here and in neither of
        the two stores above it.

        **Listed, never adopted** (ADR-0032): the file stays exactly where its own program put it,
        and what is added is a row pointing at it.
    */
    for one in cached_weights() {
        if !held.iter().any(|it| it.path == one.path) {
            held.push(one);
        }
    }
    /*
        **And what the running llama.cpp is actually serving, which is a fourth place.**

        Reported by opening the product: CREW LINKS read `10 models` and MODELS read `2`, in the
        same window, about the same machine, with nothing on screen to explain it. The three
        stores above are *files Epoch can find*; a router serves whatever its own configuration
        points at, and that may be a directory none of them look in.

        Two panels measuring one machine and disagreeing is the shape this codebase already has
        a rule for — a shelf is not a card — arriving from the other side: here both readings
        were real and neither was wrong, and the user still had no way to resolve them.

        **Listed, never adopted** (ADR-0032), exactly like the cache above it: the file stays
        where its own program put it and what is added is a row pointing at it.
    */
    for one in served_weights() {
        if !held.iter().any(|it| it.path == one.path) {
            held.push(one);
        }
    }
    held
}

/// The models the running llama.cpp router is configured to serve, as files on this disk.
///
/// **Read from the command it says it would run**, not from a directory walk. `/v1/models`
/// publishes each entry's `status.args` — the whole `llama-server` line — and the two facts that
/// matter are named flags in it:
///
/// ```text
/// "--model",  "…\vault\shelf\Qwen3.5-9B-Q6-K.gguf"
/// "--mmproj", "…\mmproj-Qwen3.8-27B-UD-Q2-K-XL---MTP.gguf"
/// ```
///
/// Taken **by flag**. The projector especially: [`projector_beside`] finds the first `mmproj` in
/// a folder, which is right for a Hugging Face snapshot holding one model and wrong for a flat
/// shelf holding fourteen files — it would hand the same eyes to every model there. The router
/// was told which one, so it is asked rather than guessed (ADR-0024, one shelf further in).
///
/// A path that is no longer on the disk is dropped: a router keeps offering a model whose file
/// somebody deleted, and a row for bytes that are not there is worse than a missing row.
///
/// Nothing here starts a server or waits on one. The connect is guarded first, because a closed
/// port on this machine costs 21 seconds rather than the instant refusal loopback is supposed to
/// give, and a request's own timeout does not cover it.
fn served_weights() -> Vec<Weights> {
    let endpoint = format!("http://127.0.0.1:{}", Runtime::LlamaCpp.usual_port());
    let Some(address) = endpoint
        .trim_start_matches("http://")
        .parse::<std::net::SocketAddr>()
        .ok()
    else {
        return Vec::new();
    };
    if std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(300))
        .is_err()
    {
        return Vec::new();
    }

    let Ok(said) = ureq::builder()
        .timeout_connect(std::time::Duration::from_millis(300))
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .get(&format!("{endpoint}/v1/models"))
        .call()
    else {
        return Vec::new();
    };
    let Ok(body) = said.into_json::<serde_json::Value>() else {
        return Vec::new();
    };
    weights_in(&body)
}

/// The rows in one `/v1/models` answer that name a file this machine still has.
fn weights_in(body: &serde_json::Value) -> Vec<Weights> {
    let after = |args: &[serde_json::Value], flag: &str| -> Option<String> {
        args.iter()
            .position(|it| it.as_str() == Some(flag))
            .and_then(|at| args.get(at + 1))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    body["data"]
        .as_array()
        .map(|all| {
            all.iter()
                .filter_map(|one| {
                    let args = one["status"]["args"].as_array()?;
                    let path = after(args, "--model")?;
                    let bytes = std::fs::metadata(&path).ok()?.len();
                    Some(Weights {
                        name: one["id"].as_str()?.to_owned(),
                        from: "llama.cpp".to_owned(),
                        path,
                        bytes,
                        sees_with: after(args, "--mmproj"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The models llama.cpp downloaded for itself, as files on this disk.
///
/// ## Two questions, and only one of them the program answers
///
/// `--cache-list` gives the names — `ggml-org/gemma-3-270m-GGUF:Q8_0` — and not the paths. The
/// files sit in the Hugging Face hub cache, which is where `llama download` said it put one when
/// it was asked on the machine this was written on:
///
/// ```text
/// …/.cache/huggingface/hub/models--ggml-org--gemma-3-270m-GGUF/snapshots/<hash>/gemma-3-270m-Q8_0.gguf
/// ```
///
/// So the name comes from the program and the file comes from the directory the program named,
/// matched on the repository rather than on the quantisation: a repository holds one file per
/// quant and the cache holds only what was fetched, so the pairing is unambiguous in practice —
/// and where it is not, the name llama.cpp uses is what is kept, because that is the name a
/// request to that server has to carry.
fn cached_weights() -> Vec<Weights> {
    let cached = cached_by_llama_cpp();
    if cached.is_empty() {
        return Vec::new();
    }
    let Some(hub) = home().map(|at| at.join(".cache").join("huggingface").join("hub")) else {
        return Vec::new();
    };

    cached
        .into_iter()
        .filter_map(|name| {
            // `unsloth/Qwen3.5-9B-GGUF:Q4_K_M` -> `models--unsloth--Qwen3.5-9B-GGUF`
            let repo = name.split(':').next()?;
            let (owner, model) = repo.split_once('/')?;
            let at = hub.join(format!("models--{owner}--{model}"));
            let file = biggest_gguf_under(&at.join("snapshots"))?;
            let bytes = std::fs::metadata(&file).ok()?.len();
            Some(Weights {
                name,
                from: "llama.cpp".to_owned(),
                path: file.display().to_string(),
                bytes,
                // The projector, when the repository brought one down beside it.
                sees_with: projector_beside(&file),
            })
        })
        .collect()
}

/// The weights inside a snapshot, which is the largest GGUF there.
///
/// A repository's snapshot holds the model and sometimes an `mmproj` beside it; the model is
/// always the larger of the two by a wide margin, so this needs no rule about names (ADR-0024).
fn biggest_gguf_under(at: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut best: Option<(u64, std::path::PathBuf)> = None;
    let mut look = vec![at.to_path_buf()];
    while let Some(here) = look.pop() {
        for entry in std::fs::read_dir(&here).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                look.push(path);
                continue;
            }
            let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
            if !name.ends_with(".gguf") || name.starts_with("mmproj") {
                continue;
            }
            let size = std::fs::metadata(&path).map(|it| it.len()).unwrap_or(0);
            if best.as_ref().is_none_or(|(had, _)| size > *had) {
                best = Some((size, path));
            }
        }
    }
    best.map(|(_, at)| at)
}

/// Its eyes, if the same snapshot brought one.
fn projector_beside(model: &std::path::Path) -> Option<String> {
    let here = model.parent()?;
    std::fs::read_dir(here)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|at| {
            at.file_name()
                .map(|it| it.to_string_lossy().to_ascii_lowercase())
                .is_some_and(|name| name.starts_with("mmproj") && name.ends_with(".gguf"))
        })
        .map(|at| at.display().to_string())
}

/// Start llama.cpp holding **every** model on the shelf, and say what it is holding.
///
/// The shelf is stocked first: the router reports what is in the directory when it starts, so
/// starting it before linking would serve whatever was there last time.
///
/// An empty shelf is not an error — llama.cpp starting with nothing to serve is a true state,
/// and refusing would leave somebody with a runtime that never starts on a machine that has no
/// models *yet*.
/// `title` names the window, so a person with several terminals open can tell which program
/// asked for this one.
pub fn serve_everything(
    shelf: &std::path::Path,
    models: &[Weights],
    how: &dyn Fn(&Weights) -> crate::loadout::Loadout,
    told: &dyn Fn(&Weights) -> crate::tuning::Tuning,
    title: &str,
) -> Result<String, String> {
    let stocked = stock_shelf(shelf, models, how, told);
    let command = router_command(shelf).ok_or("llama-server is not on this machine")?;
    open_in_terminal(title, &command)?;
    Ok(match stocked.models {
        0 => "A terminal opened, and the shelf is empty. llama.cpp will serve nothing until this \
              machine has a model. Look again once it says it is listening."
            .to_owned(),
        1 => "A terminal opened on llama.cpp, offering the 1 model this machine has. Look again \
              once it says it is listening."
            .to_owned(),
        many => format!(
            "A terminal opened on llama.cpp, offering all {many} models this machine has. It \
             loads one when a character asks for it. Look again once it says it is listening."
        ),
    })
}

/// What llama.cpp itself says this model needs on this machine.
///
/*
    **The program ships a tool that answers the question, so it is asked rather than estimated.**
    `llama-fit-params -m <model> -c <context>` loads the model, measures the free memory, and
    prints the CLI arguments that fit — including an `--override-tensor` regex that keeps
    attention on the card and pushes only the MoE experts to the CPU.

    Measured 2026-08-31 for `Qwen3.6-35B-A3B-UD-IQ4_XS` at 32K on a 12 GB card, from its own
    verbose breakdown:

    ```text
    | memory breakdown [MiB] | total    free    self   model   context   compute |
    |   - CUDA0 (4070 SUPER) | 12281 = 11005 + (9879 =  8680 +     702 +     497) |
    |   - Host               |                  8258 =  8218 +       0 +      40  |
    ```

    8.68 GB of a 17.73 GB model on the card and 8.22 GB in host memory — the offload figure, from
    the program that does the offloading. And the overhead at 32K is 702 + 497 MiB, which is what
    answers *does 11.9 GB fit in 12 GB* without anybody having to guess: it does not.

    Returned opaque. The regex names tensors, it was generated by the program that will consume
    it, and nothing here parses or reasons about it — the same discipline as a Provider's native
    tuning (ADR-0026). Epoch's job is to have asked.

    `None` where the tool is missing or says nothing useful, which is *unasked* rather than
    *nothing to do*.
*/
/// What llama.cpp's own fit worked out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fitted {
    /// `--override-tensor`, opaque and passed through untouched.
    pub override_tensor: String,
    /// `--n-gpu-layers`.
    pub gpu_layers: i32,
    /// Model bytes it says will sit on the card, and in host memory.
    ///
    /// **Its own arithmetic, not a difference of two totals.** A card holds a desktop as well as
    /// a model, so subtracting one total from another charges the model for the browser.
    pub model_on_gpu: Option<u64>,
    pub model_on_host: Option<u64>,
    /// What the context and the compute buffers cost on the card, in bytes. This is the pair that
    /// answers *does 11.9 GB fit in 12 GB* without anybody guessing.
    pub context_bytes: Option<u64>,
    pub compute_bytes: Option<u64>,
}

/// How this model sits on this machine's card at one context.
///
/// **Read from what the fitter said about its own decision**, never from the file's size. A 17.7
/// GB model that fits by moving twenty-two layers' experts to the host and a 6.9 GB model that
/// needed nothing arranged are two different machines to search on, and only one of them has
/// anything to learn from a placement candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FitClass {
    /// Nothing had to be arranged: the fitter answered `-ngl -1` and wrote no `-ot`.
    Comfortable,
    /// Arranged, and all of it is still on the card.
    Tight,
    /// Part of the weights live in host memory. Measured on the 35B at 32K: 22 of 41 layers
    /// overflowing, 8218 MiB of model on the host.
    Hybrid,
    /// The fitter said it could not fit this at this context.
    Unloadable,
    /// The program could not be run, or said something this cannot read.
    ///
    /// **Never "it does not fit".** A question nobody could ask has no answer, and a planner that
    /// read this as *unloadable* would refuse a search on a machine that is merely missing a
    /// sibling binary.
    Unknown,
}

/// Everything one run of the fitter answered.
///
/// One program, one model load, two questions: *how does this sit here* and *what arguments make
/// it sit that way*. They were two calls' worth of work and are one run's worth of output.
#[derive(Debug, Clone, PartialEq)]
pub struct Fitting {
    pub class: FitClass,
    /// The arguments to give llama.cpp, where the fitter had to arrange anything. `None` for a
    /// model that needed none — a candidate identical to the baseline is a configuration
    /// measured twice.
    pub args: Option<Fitted>,
    /// Model bytes the fitter placed in host memory, when it said.
    pub on_host: Option<u64>,
    /// Layers left off the card, out of how many there are.
    pub overflowing: Option<(u32, u32)>,
}

/// Ask the fitter once, and keep everything it said.
pub fn fitting(model: &std::path::Path, context: u32) -> Fitting {
    use crate::quiet::Quiet;
    let Some(exe) = beside_llama("llama-fit-params") else {
        return Fitting {
            class: FitClass::Unknown,
            args: None,
            on_host: None,
            overflowing: None,
        };
    };
    let Ok(out) = std::process::Command::new(exe)
        .quiet()
        .args([
            "-m",
            &model.display().to_string(),
            "-c",
            &context.to_string(),
            "-v",
        ])
        .stdin(std::process::Stdio::null())
        .output()
    else {
        return Fitting {
            class: FitClass::Unknown,
            args: None,
            on_host: None,
            overflowing: None,
        };
    };
    read_fitting(
        &String::from_utf8_lossy(&out.stdout),
        &String::from_utf8_lossy(&out.stderr),
    )
}

/// The classification, out of the two streams. Pure, so the shapes measured on this machine are
/// a test rather than a run.
pub fn read_fitting(said: &str, told: &str) -> Fitting {
    let over = overflowing(told);
    let on_host = host_model(told);

    /*
        **Refusal is a sentence the program prints, and it is not the absence of one.** `failed to
        fit params to free device memory` is what it says when there is no arrangement; silence
        where neither that nor a success line appears is a run nobody could read.
    */
    if told.contains("failed to fit params") {
        return Fitting {
            class: FitClass::Unloadable,
            args: None,
            on_host,
            overflowing: over,
        };
    }
    let Some(line) = said.lines().find(|it| it.contains("-ngl")) else {
        return Fitting {
            class: FitClass::Unknown,
            args: None,
            on_host,
            overflowing: over,
        };
    };

    let args = arguments_in(line, told);
    let class = match (&args, over) {
        // Nothing was arranged: `-ngl -1` and no `-ot`. Measured on `gemma4-12b` at 32K.
        (None, _) => FitClass::Comfortable,
        // Arranged, and the fitter said how much it had to push off the card.
        (Some(_), Some((over, _))) if over > 0 => FitClass::Hybrid,
        (Some(_), Some(_)) => FitClass::Tight,
        /*
            Arranged, and it did not say. `-ot` naming CPU is the arrangement itself saying where
            those tensors went, which is a reading rather than a guess; anything else it arranged
            without overflow is Tight.
        */
        (Some(it), None) if it.override_tensor.contains("CPU") => FitClass::Hybrid,
        (Some(_), None) => FitClass::Tight,
    };
    Fitting {
        class,
        args,
        on_host,
        overflowing: over,
    }
}

/// The arguments out of the fitter's stdout line, or `None` where it arranged nothing.
fn arguments_in(line: &str, told: &str) -> Option<Fitted> {
    let mut words = line.split_whitespace();
    let mut layers: Option<i32> = None;
    while let Some(word) = words.next() {
        if word == "-ngl" {
            layers = words.next().and_then(|it| it.parse().ok());
            break;
        }
    }
    let start = line.find("-ot ")? + 4;
    let offload = line[start..].trim().trim_matches('"');
    if offload.is_empty() {
        return None;
    }
    let (gpu, context_bytes, compute_bytes) = breakdown(told, "CUDA0")
        .or_else(|| breakdown(told, "Metal"))
        .or_else(|| breakdown(told, "Vulkan0"))
        .unzip3();
    Some(Fitted {
        override_tensor: offload.to_owned(),
        gpu_layers: layers?,
        model_on_gpu: gpu,
        model_on_host: host_model(told),
        context_bytes,
        compute_bytes,
    })
}

pub fn fitted_params(model: &std::path::Path, context: u32) -> Option<Fitted> {
    fitting(model, context).args
}

/// The device line of llama.cpp's own memory breakdown, in bytes.
///
/// Measured 2026-08-31, `Qwen3.6-35B-A3B-UD-IQ4_XS` at 32K on a 12 GB card:
///
/// ```text
/// | memory breakdown [MiB] | total    free    self   model   context   compute |
/// |   - CUDA0 (4070 SUPER) | 12281 = 11005 + (9879 =  8680 +     702 +     497) |
/// ```
///
/// Read positionally from inside the parentheses, which is where the three numbers that matter
/// are. `None` for a line that does not parse, which is *nobody could read it* — never a zero.
fn breakdown(said: &str, device: &str) -> Option<(u64, u64, u64)> {
    /*
        **The last block, not the first.** The fitter prints this table once per placement it
        tries. Measured 2026-09-01 on the 35B at 32K, the first says `17579 = 16383 + 702 + 493`
        with `-16302` unaccounted — the all-on-card arrangement it is about to reject — and the
        accepted one, five tables later, says `9879 = 8680 + 702 + 497`. `find` read the rejected
        one.

        **And the numbers are in the third cell.** The line carries a timestamp, a log prefix and
        a device name before them, and the device name has its own parentheses:

        ```text
        0.03.51 I …print: |   - CUDA0 (RTX 4070 SUPER) | 12281 = 11005 + (9879 = 8680 + 702 + 497) + -8603 |
        ```

        `split('(').nth(1)` reads `RTX 4070 SUPER`, whose only number is 4070 — so this answered
        `None` on every machine whose card has a number in its name, which is all of them. Reading
        the table's own cells is what survives both.
    */
    let line = said
        .lines()
        .rfind(|it| !it.contains("memory breakdown") && it.contains(device) && it.contains('|'))?;
    let cell = line.split('|').nth(2)?;
    // **After a `(`, and the one that holds the sum.** The text before any parenthesis —
    // ` 12281 = 11005 + ` — contains an `=` of its own, so looking for the first piece with one
    // finds the total rather than the breakdown.
    let inside = cell
        .split('(')
        .skip(1)
        .find(|it| it.contains('='))?
        .split(')')
        .next()?;
    let numbers: Vec<u64> = inside
        .split(|c: char| !c.is_ascii_digit())
        .filter(|it| !it.is_empty())
        .filter_map(|it| it.parse().ok())
        .collect();
    // `self = model + context + compute` — four numbers, and the last three are the ones wanted.
    let [_, model, context, compute] = numbers[..] else {
        return None;
    };
    let mib = |n: u64| n * 1024 * 1024;
    Some((mib(model), mib(context), mib(compute)))
}

/// What the same breakdown says stayed in host memory.
fn host_model(said: &str) -> Option<u64> {
    /*
        **Past the timestamp, and the last one.** This split the whole line on non-digits, and
        every line begins `0.03.514.348 I` — so the four numbers it found first were the clock,
        and `numbers[1]` was the `00` of `0.03`. It answered `Some(0)` for a model with eight
        gigabytes of weights in host memory, which is worse than answering nothing: a planner
        then dropped the offload candidate for the one model that needed it.

        The third `|`-cell is where the numbers are, the same as the device rows.
    */
    let line = said
        .lines()
        .rfind(|it| it.contains("- Host") && it.contains('|'))?;
    let numbers: Vec<u64> = line
        .split('|')
        .nth(2)?
        .split(|c: char| !c.is_ascii_digit())
        .filter(|it| !it.is_empty())
        .filter_map(|it| it.parse().ok())
        .collect();
    // `total = model + context + compute`; the model is the second.
    numbers.get(1).map(|it| it * 1024 * 1024)
}

/// How many of a model's layers the fitter had to leave off the card, out of how many there are.
///
/// **The fitter's own conclusion, printed once, after it has decided.** Everything else in the
/// log is a placement it was trying out.
///
/// ```text
/// - CUDA0 (NVIDIA GeForce RTX 4070 SUPER): 41 layers (22 overflowing),   9879 MiB used,   1125 MiB free
/// ```
///
/// A model that needed no arranging produces no such line at all — which is a real answer and
/// the reason this is `Option`: *nothing to report* and *nobody could read it* are the same
/// shape here, and both mean the caller must not claim an overflow.
fn overflowing(said: &str) -> Option<(u32, u32)> {
    let line = said
        .lines()
        .rfind(|it| it.contains("overflowing") && it.contains("layers"))?;
    let layers: u32 = line
        .split("layers")
        .next()?
        .split_whitespace()
        .next_back()?
        .parse()
        .ok()?;
    let over: u32 = line
        .split("overflowing")
        .next()?
        .rsplit('(')
        .next()?
        .trim()
        .parse()
        .ok()?;
    Some((over, layers))
}

/// Splitting an `Option<(a, b, c)>` into three, so a device that could not be read leaves all
/// three unanswered rather than two of them zero.
trait Unzip3<A, B, C> {
    fn unzip3(self) -> (Option<A>, Option<B>, Option<C>);
}

impl<A, B, C> Unzip3<A, B, C> for Option<(A, B, C)> {
    fn unzip3(self) -> (Option<A>, Option<B>, Option<C>) {
        match self {
            Some((a, b, c)) => (Some(a), Some(b), Some(c)),
            None => (None, None, None),
        }
    }
}

/// A sibling program in whichever llama.cpp this machine is using.
///
/// **Beside the server, never on the PATH.** A machine may have two llama.cpp installations —
/// this one deliberately prefers `~/.llama/bin` over the winget package — and picking the tool
/// from a different install than the server would answer about a different build.
fn beside_llama(program: &str) -> Option<std::path::PathBuf> {
    let server = found_in("llama-server", Runtime::LlamaCpp.winget_package(), None)?;
    let home = server.parent()?;
    let exe = home.join(format!("{program}{}", std::env::consts::EXE_SUFFIX));
    exe.is_file().then_some(exe)
}

/// Stop the router serving one shelf, and nothing else.
///
/*
    **Never by process name.** LM Studio's inference runtime *is* `llama-server.exe` — measured,
    and already written down in `CLAUDE.md` after it made a user think Epoch was starting the
    wrong program — so killing by name would take somebody's loaded LM Studio model with it.

    What tells them apart is the command line. Epoch's router is spawned with
    `--models-dir <this shelf>`, and nothing else on the machine carries that path. Read from the
    process's own argv rather than from a pid Epoch remembers, because the router outlives Epoch:
    it runs in the user's own terminal, on purpose, and a pid from a previous session is a pid
    that now belongs to somebody else.

    Needed because starting the router does not replace one — it opens a terminal, and a second
    terminal on a bound port fails. A sweep restarts it once per configuration.
*/
pub fn stop_router(shelf: &std::path::Path) -> Result<(), String> {
    #[allow(unused_imports)]
    use crate::quiet::Quiet;
    let shelf = shelf.display().to_string();

    #[cfg(windows)]
    {
        // Doubled apostrophes: PowerShell's escape inside a single-quoted string. A path with one
        // in it is unusual and a script that broke on it would break rarely and confusingly.
        let pattern = shelf.replace('\'', "''");
        let script = format!(
            "Get-CimInstance Win32_Process -Filter \"Name='llama-server.exe'\" | \
             Where-Object {{ $_.CommandLine -like '*{pattern}*' }} | \
             ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}"
        );
        std::process::Command::new("powershell")
            .quiet()
            .args(["-NoProfile", "-Command", &script])
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|why| why.to_string())?;

        /*
            **And the window it was living in.**

            Stopping `llama-server.exe` leaves the `cmd` that launched it, so every restart of the
            router left a console behind — a search that restarts it twenty times leaves twenty.
            Reported from a photograph of four of them stacked on the desktop, which is how a
            person meets this rather than how a test does.

            `close_our_terminals` already existed for the installer's consoles and keys on the
            same two things: the shelf path in the command line, and the `Epoch - ` marker that
            says the window is one Epoch opened. Somebody else's terminal that happens to mention
            the shelf is left alone.
        */
        let _ = close_our_terminals(&shelf);
    }

    #[cfg(unix)]
    {
        std::process::Command::new("pkill")
            .quiet()
            .args(["-f", &format!("llama-server.*{shelf}")])
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|why| why.to_string())?;
    }

    Ok(())
}

/// Put every model this machine has on LM Studio's shelf.
///
/// The same answer as the router in the shape LM Studio takes: it loads from its own directory
/// on its own terms, so what it needs is for everything to be *there* rather than for somebody
/// to pick one.
///
/// Never `lms import`, which defaults to **moving** the file and would empty Ollama's store.
///
/// A model that cannot be linked is counted rather than fatal: a shelf with most of them on it
/// beats none, and the count says how many did not make it.
pub fn lend_everything(models: &[Weights]) -> Result<String, String> {
    let mut lent = 0usize;
    let mut problems = Vec::new();
    for model in models {
        match share_with_lm_studio(&model.name, &model.path, model.sees_with.as_deref()) {
            Ok(_) => lent += 1,
            Err(why) => problems.push(format!("{}: {why}", model.name)),
        }
    }
    if lent == 0 {
        return Err(problems
            .first()
            .cloned()
            .unwrap_or_else(|| "this machine has no models to lend".to_owned()));
    }
    Ok(format!(
        "{lent} {} on LM Studio's shelf, as hard links, so it costs no disk and Ollama still has \
         {}. Load one from LM Studio, then look again.{}",
        if lent == 1 { "model is" } else { "models are" },
        if lent == 1 { "it" } else { "them" },
        if problems.is_empty() {
            String::new()
        } else {
            format!(" {} could not be linked.", problems.len())
        }
    ))
}

/// Loading one model several ways and timing each, so [`crate::loadout::search`] can choose.
///
/// ## A server of its own, on a port of its own
///
/// Never the router the World is using. The search loads the same model three or four times with
/// different flags, and doing that to the running router would take everybody's brains away
/// mid-turn. It is a private `llama-server -m <file>` on a high port, started quiet, asked once,
/// and killed.
///
/// ## What one probe costs, measured
///
/// A load is 20 to 70 seconds depending on whether the file is in the page cache, and one short
/// answer is a few more. Three or four of those is the four to six minutes this is budgeted at,
/// which is why it never blocks a download and is always skippable.
pub struct Bench {
    program: std::path::PathBuf,
    model: std::path::PathBuf,
    port: u16,
    margin: u32,
    /// Whether the file has been pulled into the page cache yet. See [`Bench::warm_the_file`].
    warmed: bool,
}

impl Bench {
    /// `None` when llama.cpp is not on this machine — there is nothing to measure with.
    pub fn new(model: impl Into<std::path::PathBuf>) -> Option<Self> {
        Some(Self {
            program: found_in("llama-server", Runtime::LlamaCpp.winget_package(), None)?,
            model: model.into(),
            // Deliberately not `usual_port`: the router may be serving the World on it, and a
            // search that cannot run beside a working World is one nobody will ever let finish.
            port: 8_099,
            margin: FIT_MARGIN_MIB,
            warmed: false,
        })
    }

    /// Read the whole file once, so the first probe is not measuring a disk.
    ///
    /// ## Two searches, and the first two readings of each were wrong
    ///
    /// Measured through the window and then reproduced from a test:
    ///
    /// ```text
    /// probe 1  16,384 f16    15.6 tok/s     (20.2 when the file is warm)
    /// probe 2  16,384 q8_0   25.4           (34.9 when the file is warm)
    /// probe 3   8,192 q8_0   35.0
    /// probe 4  24,576 q8_0   22.9
    /// ```
    ///
    /// `Bench` itself is exact — the same probe run twice on a warm machine answered 34.98 and
    /// 34.89. What was wrong is that the first loads of a session pull ten gigabytes off a disk
    /// and leave the machine finishing that work into the second.
    ///
    /// **This codebase already knew.** 11.18 recorded a cold llama.cpp reading 111.9 s against
    /// 12.6 s warm and said not to quote it; `speeds::time_it` discards its first answer for the
    /// same reason. The search inherited the rule for *answers* and not for *loads*.
    ///
    /// Six seconds at this machine's 1.8 GB/s, against four minutes of probing — and unlike
    /// discarding a probe, it costs nothing that has to be explained afterwards.
    fn warm_the_file(&self) {
        use std::io::Read;
        let Ok(mut file) = std::fs::File::open(&self.model) else {
            return;
        };
        let mut buffer = vec![0u8; 8 << 20];
        // Read and drop. The point is the operating system's page cache, not this buffer.
        while matches!(file.read(&mut buffer), Ok(read) if read > 0) {}
    }

    /// Time one answer, or `None` if the model never became ready.
    fn once(&self, one: crate::loadout::Loadout) -> Option<crate::loadout::Run> {
        use crate::quiet::Quiet;

        let mut command = std::process::Command::new(&self.program);
        command
            .arg("-m")
            .arg(&self.model)
            .args(["--host", "127.0.0.1"])
            .args(["--port", &self.port.to_string()])
            .args(["-c", &one.context.to_string()])
            .args(["-fitt", &self.margin.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        for flag in one.cache.flags().split_whitespace() {
            command.arg(flag);
        }
        // Read before spawning: this is what the wait afterwards has to get back to, and
        // reading it after the kill would be asking the question the answer depends on.
        let at_rest = crate::machine::Machine::measure().vram_free;
        let mut child = command.quiet().spawn().ok()?;

        let at = format!("http://127.0.0.1:{}", self.port);
        let ready = self.wait_for_it(&at, &mut child);
        // Timed only when it actually loaded. A model that never became ready has no speed, and
        // saying so is the answer — `unknown model architecture` is real information.
        let rate = if ready {
            // The first answer includes reading the file off disk, which is a measurement of a
            // disk and not of a card. Discarded, exactly as `speeds::time_it` discards it.
            let _ = ask_once(&at, 8);
            ask_once(&at, 120)
        } else {
            None
        };

        let _ = child.kill();
        let _ = child.wait();
        self.wait_for_the_card_back(at_rest);

        rate.map(|tokens_per_second| crate::loadout::Run { tokens_per_second })
    }

    /// Wait until the card is actually free again before the next probe binds it.
    ///
    /// ## Three seconds was a guess and it was wrong in a way that reads as a result
    ///
    /// `child.kill()` returns as soon as the process is gone; the driver hands the memory back
    /// afterwards, and for a ten-gigabyte model that takes longer than a fixed sleep. Measured
    /// through the window: a full eight-load search of `Qwen3.8-27B-Uncensored-IQ2_M` produced
    ///
    /// ```text
    ///  8,192  33.0 tok/s   <- the first probe, on a clean card
    /// 16,384  22.7
    /// 24,576  20.3
    /// 32,768  17.6
    /// ```
    ///
    /// against 33.6 measured by hand at 16,384 with the same flags. **The first reading is right
    /// and every reading after it is low**, which is the signature of each probe starting on the
    /// last one's leftovers — and it is not visible as an error, because the curve still slopes
    /// the way a curve should.
    ///
    /// So the card is read back rather than waited on. `None` for free memory means nothing
    /// could be asked, and there is nothing to wait for.
    fn wait_for_the_card_back(&self, at_rest: Option<u64>) {
        // **The line is what this machine read before the probe, not a fraction of the card.**
        // `total - total/10` was 11,054 MiB on a machine whose idle reading is 10,871, so the
        // condition could never be true and every probe spun the full sixty seconds — a wait
        // that always times out is a sleep wearing a measurement's clothes.
        let Some(want) = at_rest else {
            std::thread::sleep(std::time::Duration::from_secs(3));
            return;
        };
        for _ in 0..60 {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if crate::machine::Machine::measure()
                .vram_free
                // Within a quarter of a gigabyte of where it was before this probe started.
                .is_some_and(|free| free + 256 * 1024 * 1024 >= want)
            {
                // One more, because `nvidia-smi` reports the pool before the allocator has
                // settled and the next load starts within milliseconds of this returning.
                std::thread::sleep(std::time::Duration::from_secs(1));
                return;
            }
        }
    }

    /// Poll until it answers, it dies, or long enough that something is wrong.
    ///
    /// **A dead child ends the wait immediately.** A model that refuses to load — the wrong
    /// architecture, or a truncated file — exits in under a second, and a search that spent four
    /// minutes per refusal would take longer to give up than to succeed.
    fn wait_for_it(&self, at: &str, child: &mut std::process::Child) -> bool {
        for _ in 0..240 {
            if matches!(child.try_wait(), Ok(Some(_))) {
                return false;
            }
            if ureq::get(&format!("{at}/v1/models"))
                .timeout(std::time::Duration::from_secs(2))
                .call()
                .is_ok()
                && ask_once(at, 4).is_some()
            {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
        false
    }
}

impl crate::loadout::Probe for Bench {
    fn run(&mut self, one: crate::loadout::Loadout) -> Option<crate::loadout::Run> {
        // Once per search rather than once per probe: after the first, the file is in the page
        // cache and reading it again is time spent proving that.
        if !self.warmed {
            self.warm_the_file();
            self.warmed = true;
        }
        self.once(one)
    }
}

/// One answer, timed by the server's own count rather than by a stopwatch around the request.
///
/// `predicted_per_second` excludes the prompt pass, which is what makes two answers of different
/// prompt lengths comparable — and the prompt pass is not what a loadout changes.
fn ask_once(at: &str, most: u32) -> Option<f64> {
    let said: serde_json::Value = ureq::post(&format!("{at}/v1/chat/completions"))
        .timeout(std::time::Duration::from_secs(900))
        .send_json(serde_json::json!({
            "messages": [{ "role": "user", "content": BENCH_WORDS }],
            "stream": false,
            "max_tokens": most
        }))
        .ok()?
        .into_json()
        .ok()?;
    said["timings"]["predicted_per_second"]
        .as_f64()
        .filter(|rate| *rate > 0.0)
}

/// The same request every time, so two loadouts differ by the loadout.
const BENCH_WORDS: &str = "Explica en un parrafo que es un faro y para que sirve.";

/// Take one model off this machine.
///
/// ## Two shelves, two mechanisms, and the difference is not cosmetic
///
/// A model Ollama pulled is Ollama's: it lives as content-addressed blobs behind a manifest, and
/// several tags can share one blob. Deleting the file would corrupt the store, so `ollama rm`
/// is asked to do it — the program that owns the shelf takes it off the shelf. A GGUF the
/// Workshop saved is a file, and a file is deleted.
///
/// ## The shelf link goes with it
///
/// `stock_shelf` hard-links models into the shelf llama.cpp serves. A hard link keeps the bytes
/// alive after the original is gone, so removing only the original would free nothing and leave
/// llama.cpp still offering a model Ollama no longer has. Both, or neither.
///
/// ## What it refuses
///
/// Only what [`everything_here`] reported holding. A path from anywhere else is not a model this
/// machine offered, and a delete that accepts an arbitrary path is a delete somebody can point
/// at anything.
pub fn remove(held: &Weights, shelf: &std::path::Path) -> Result<String, String> {
    /*
        **Every link Epoch made, not just the ones on its own shelf.**

        The rule was already written here — *leaving one behind after the model is gone would
        keep the bytes on disk while the Workshop says the model was removed* — and it was
        applied to one of the two places Epoch links into. `share_with_lm_studio` hard-links each
        model into `~/.lmstudio/models/ollama/<name>/` so LM Studio can serve it without a second
        download, and nothing ever removed those.

        Measured on the owner's machine after he deleted four models: `ollama rm` had run,
        Epoch's shelf was clean, and `.lmstudio/models/ollama/` still held **58.8 GB** with a
        link count of exactly **1** on every file — Epoch's own link was the last thing keeping
        those bytes alive. The deck said the models were gone and the disk did not move.

        Confined to what `share_with_lm_studio` writes: that one directory, those two names.
        Anything the user downloaded through LM Studio itself lives under a different publisher
        and is never touched — Epoch cleans up after itself and nothing more (ADR-0032).
    */
    let clean = shelf_name(&held.name);
    let mut freed = 0u64;
    let lent_to_lm_studio = home().map(|home| {
        home.join(".lmstudio")
            .join("models")
            .join("ollama")
            .join(&clean)
    });
    for place in [Some(shelf.to_path_buf()), lent_to_lm_studio.clone()]
        .into_iter()
        .flatten()
    {
        for file in [format!("{clean}.gguf"), format!("mmproj-{clean}.gguf")] {
            let link = place.join(file);
            if link.is_file() {
                // Counted before it goes: on the last link this is what the disk gets back, and
                // on any other it is zero. A number read afterwards would be a guess.
                let last = std::fs::metadata(&link)
                    .map(|it| it.len())
                    .unwrap_or_default();
                std::fs::remove_file(&link)
                    .map_err(|err| format!("{} is still here: {err}", link.display()))?;
                freed += last;
            }
        }
    }
    // The directory Epoch made for it, once nothing of Epoch's is in it. `remove_dir` refuses a
    // directory that is not empty, which is the guard: anything else in there is not Epoch's.
    if let Some(lent) = lent_to_lm_studio {
        let _ = std::fs::remove_dir(&lent);
    }

    if held.from == "Ollama" {
        // Never the blob. Several tags can name one, and Ollama's manifest is the only thing
        // that knows which — so the program that filed it is the one asked to unfile it.
        let said = std::process::Command::new("ollama")
            .args(["rm", &held.name])
            .quiet_output()
            .map_err(|err| format!("ollama could not be asked to remove it: {err}"))?;
        if !said.status.success() {
            return Err(format!(
                "ollama refused: {}",
                String::from_utf8_lossy(&said.stderr).trim()
            ));
        }
        return Ok(format!(
            "{} is off Ollama's shelf.{}",
            held.name,
            also_freed(freed)
        ));
    }

    let path = std::path::Path::new(&held.path);
    let last = std::fs::metadata(path)
        .map(|it| it.len())
        .unwrap_or_default();
    std::fs::remove_file(path).map_err(|err| format!("{} is still here: {err}", held.name))?;
    Ok(format!(
        "{} is deleted.{}",
        held.name,
        also_freed(freed + last)
    ))
}

/// What a removal took back, said only when there is something to say.
///
/// **Named, because a delete that reaches into another program's folder must say so.** Epoch put
/// those links there and is entitled to take them away, and the person is entitled to know it
/// happened without reading this file.
fn also_freed(bytes: u64) -> String {
    if bytes == 0 {
        return String::new();
    }
    format!(
        " {:.1} GB of links Epoch had made — including LM Studio's — went with it.",
        bytes as f64 / 1e9
    )
}

/// Run a command with no console window and collect what it said.
trait QuietOutput {
    fn quiet_output(&mut self) -> std::io::Result<std::process::Output>;
}

impl QuietOutput for std::process::Command {
    fn quiet_output(&mut self) -> std::io::Result<std::process::Output> {
        use crate::quiet::Quiet;
        self.quiet().output()
    }
}

/// Which llama.cpp build is on this machine.
///
/// Part of what keys a measured loadout, because the card is not the whole machine: a release
/// that changes kernels changes the answer, and a remembered measurement that cannot tell would
/// go on capping a machine that had got faster. Empty when llama.cpp is not here — which is not
/// a build, and reads as one question nobody could ask.
pub fn llama_build() -> String {
    let Some(program) = found_in("llama-server", Runtime::LlamaCpp.winget_package(), None) else {
        return String::new();
    };
    use crate::quiet::Quiet;
    let Ok(said) = std::process::Command::new(&program)
        .arg("--version")
        .quiet()
        .output()
    else {
        return String::new();
    };
    // `version: 0.3.0-dev (build 10622, commit 3737e4137)` — the commit is the exact answer and
    // the build number is the readable one. Both, because a build number alone repeats across
    // forks and a commit alone means nothing to a person reading the file.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&said.stdout),
        String::from_utf8_lossy(&said.stderr)
    );
    text.lines()
        .find(|line| line.contains("version:"))
        .map(|line| line.trim().to_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod pairing {
    use super::*;

    /// A projector belongs to the model it was published with, and to nothing else.
    ///
    /// Built from real headers: the 27B's projector is 5120 wide, the 27B is 5120 wide, and
    /// `qwen3-14b` is *also* 5120 wide with no eyes at all — which is exactly why the width
    /// filters and the name decides.
    #[test]
    fn eyes_go_to_the_model_they_arrived_with_and_not_to_a_stranger() {
        assert!(arrived_together(
            "Qwen3.8-27B-Uncensored-IQ2_M",
            "Qwen3.8-27B-Uncensored-vision-f16"
        ));
        assert!(
            !arrived_together("qwen3-14b", "Qwen3.8-27B-Uncensored-vision-f16"),
            "same embedding width, different model, no eyes"
        );
        assert!(
            !arrived_together("gemma4-12b", "Qwen3.8-27B-Uncensored-vision-f16"),
            "nothing in common"
        );
    }

    /// A proportion would let two short names agree by accident.
    #[test]
    fn a_short_name_cannot_agree_by_being_short() {
        assert!(!arrived_together("q4", "q8"), "50% alike and unrelated");
        assert!(
            !arrived_together("model", "model-vision"),
            "five is not enough"
        );
    }

    /// Whatever this machine actually has. An empty list is honest — nothing saved here.
    #[test]
    fn on_this_machine_a_saved_vision_model_finds_its_eyes() {
        let vault = std::path::Path::new(r"C:\Users\someone\Epoch\BUILD\vault\models");
        if !vault.is_dir() {
            return;
        }
        let held = gguf_in(vault);
        // The projector is not offered as a model of its own: it is not one, and a shelf that
        // listed it would offer a character a brain that cannot think.
        assert!(
            !held.iter().any(|one| one.name.contains("vision")),
            "a projector is not a model: {:?}",
            held.iter().map(|o| &o.name).collect::<Vec<_>>()
        );
        for one in &held {
            eprintln!(
                "{} — {}",
                one.name,
                one.sees_with.as_deref().unwrap_or("no eyes")
            );
        }
    }
}

/// Ask every local runtime to let go of whatever it is holding on the card.
///
/// ## Why a measurement needs the card to itself
///
/// Found by running a real search through the window, right after pressing TIME IT on another
/// model. Ollama holds a model for five minutes after answering, so `gemma4-12b` was still
/// resident for the whole thing, and the 27B was measured on a card with 7.6 GB already gone.
///
/// It did not fail. It produced a full, plausible curve of wrong numbers — 19.7 tok/s where the
/// same model on a free card answers at 33.7 — **and it picked the wrong cache**, because with
/// less room the trade between them runs the other way. Then it saved that as the model's
/// loadout.
///
/// That is worse than refusing: a wrong answer chosen silently, presented beside a table of
/// measurements that all agree with each other. `speeds` has carried a `fitted` flag since it
/// was written for exactly this reason; a search had nothing.
///
/// ## Best-effort, and then checked
///
/// Each runtime is asked in its own words and none of them is required to answer — a runtime
/// that is not running has nothing to release, and one that refuses is not a reason to fail. The
/// caller reads the card back afterwards ([`crate::machine::Machine::measure`]) and decides
/// whether it got what it asked for. Verifying the side effect rather than the status code is
/// the rule LM Studio's `200`-to-a-route-it-lacks already taught here.
/// Ask **one** runtime to let go of **one** model.
///
/// [`free_the_card`] is the blunt version and it is right where the card has to be empty — a
/// loadout search, a picture about to be drawn. It is wrong after a measurement: a person on the
/// Models deck timing something has not asked for the model a character is mid-conversation with
/// to be dropped too.
///
/// Each runtime in its own words, and they are the three `free_the_card` already measured:
/// Ollama takes `keep_alive: 0`, llama.cpp's router answers `POST /models/unload` (it holds one
/// at a time — `--models-max 1`), and LM Studio reads a `ttl` in a request body because it
/// serves no unload route at all. `ttl: 1` and not `0`: zero reads as *unset* there.
///
/// Best effort, like every release in this file. A model that did not go is a fuller card and
/// the deck's own reading says so.
pub fn let_go_of(runtime: Runtime, model: &str) {
    let at = format!("http://127.0.0.1:{}", runtime.usual_port());
    match runtime {
        Runtime::Ollama => {
            let _ = ureq::post(&format!("{at}/api/generate"))
                .timeout(std::time::Duration::from_secs(10))
                .send_json(serde_json::json!({ "model": model, "keep_alive": 0 }));
        }
        Runtime::LlamaCpp => {
            // **By name.** See `free_the_card` for what an empty body does now.
            let _ = ureq::post(&format!("{at}/models/unload"))
                .timeout(std::time::Duration::from_secs(20))
                .send_json(serde_json::json!({ "model": model }));
        }
        Runtime::LmStudio => {
            let _ = ureq::post(&format!("{at}/api/v0/chat/completions"))
                .timeout(std::time::Duration::from_secs(20))
                .send_json(serde_json::json!({
                    "model": model,
                    "messages": [{ "role": "user", "content": "." }],
                    "max_tokens": 1,
                    "ttl": 1
                }));
        }
    }
}

pub fn free_the_card() {
    // Ollama: a generate call with `keep_alive: 0` and no prompt, its own documented way.
    for (name, _) in crate::ollama_library::held_by(&format!(
        "http://127.0.0.1:{}",
        Runtime::Ollama.usual_port()
    )) {
        let _ = ureq::post(&format!(
            "http://127.0.0.1:{}/api/generate",
            Runtime::Ollama.usual_port()
        ))
        .timeout(std::time::Duration::from_secs(10))
        .send_json(serde_json::json!({ "model": name, "keep_alive": 0 }));
    }

    /*
        **llama.cpp's router, one model at a time and each one named.**

        `POST /models/unload` was measured on 2026-08-25 as the route it answers, and it was
        sent an empty body. On build `b10622` that answers **400 `model is not found`** — and
        nobody was reading the status, so every llama.cpp release Epoch has made since that build
        arrived has been a no-op. Measured 2026-08-30: with `gemma4-12b` resident the card held
        9857 MiB, the empty body changed nothing, and `{"model":"gemma4-12b"}` answered
        `{"success":true}` and took it to 1243 MiB.

        It is not a small thing to have been silently missing. It is what `KeepLoaded::Never`
        does at the end of a turn, what runs before a picture needs the card, and what a loadout
        search calls to make the card its own — which is where it surfaced: MEASURE refused with
        *llama.cpp has gemma4-12b* about a model it had just asked llama.cpp to drop.

        **Somebody else's route is a measurement, not a memory** (ADR-0027). It was true when it
        was written down, and nothing tells you the day it stops being true — except a status code
        nobody looked at.
    */
    let at = format!("http://127.0.0.1:{}", Runtime::LlamaCpp.usual_port());
    for name in resident_at(&at, Runtime::LlamaCpp) {
        let _ = ureq::post(&format!("{at}/models/unload"))
            .timeout(std::time::Duration::from_secs(20))
            .send_json(serde_json::json!({ "model": name }));
    }

    // LM Studio reads a `ttl` in the request body rather than offering a route. `1` and not `0`:
    // measured, zero reads as *unset* there and leaves its own hour in place — the spelling that
    // means "now" everywhere else would have shipped a fix that changed nothing.
    if let Some(loaded) = ureq::get(&format!(
        "http://127.0.0.1:{}/api/v0/models",
        Runtime::LmStudio.usual_port()
    ))
    .timeout(std::time::Duration::from_secs(5))
    .call()
    .ok()
    .and_then(|said| said.into_json::<serde_json::Value>().ok())
    {
        for one in loaded["data"].as_array().unwrap_or(&Vec::new()) {
            if one["state"] == "loaded" {
                let _ = ureq::post(&format!(
                    "http://127.0.0.1:{}/api/v0/chat/completions",
                    Runtime::LmStudio.usual_port()
                ))
                .timeout(std::time::Duration::from_secs(20))
                .send_json(serde_json::json!({
                    "model": one["id"],
                    "messages": [{ "role": "user", "content": "." }],
                    "max_tokens": 1,
                    "ttl": 1
                }));
            }
        }
    }

    // The unload is not instant on any of them, and the next thing this is used for is a
    // measurement. A second is cheap against four minutes.
    std::thread::sleep(std::time::Duration::from_secs(2));
}
