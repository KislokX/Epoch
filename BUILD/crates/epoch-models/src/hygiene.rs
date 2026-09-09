//! Whether this machine is quiet enough for a measurement to mean anything.
//!
//! ## The run that made this necessary
//!
//! On 2026-08-31 a sequence of seven configurations was measured while the same machine was
//! compiling Rust. One row read **18.0 tok/s** for a model that measures 45.8 — and every other
//! row in that sequence was quietly wrong by an unknown amount. The only reason it was caught is
//! that 18 is absurd; a row that had come out 8% low would have been believed and would have
//! become a profile.
//!
//! > **A benchmark on a machine that is also building something is measuring the build.**
//!
//! ## What is checked, and why these numbers
//!
//! Measured on this machine the same day, both states:
//!
//! | | CPU peak / mean | GPU peak / mean |
//! |---|---|---|
//! | quiet, model answering | 18% / 13% | 42% / 28% |
//! | compiling at the same time | 88% / 32% | — |
//! | nothing running at all | — | 7% / — |
//!
//! So the thresholds are read off that gap rather than chosen: **a fifth of the CPU** is well
//! above a quiet machine serving a model and well below one building something, and **a fifth of
//! the card** is far above idle. Both are checked *before* anything is loaded, when this program
//! should be using neither.
//!
//! ## It waits rather than refusing
//!
//! A compilation finishes. A refusal would send somebody back to press the button again, and the
//! thing they would be waiting for is exactly what this can watch for them. What it will not do
//! is record the run anyway.

use std::time::{Duration, Instant};

/// What was found running.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Busy {
    /// Percent, averaged across cores. `None` where the machine could not be asked — which is
    /// *unasked*, and does not block a run.
    pub cpu: Option<f64>,
    pub gpu: Option<f64>,
    /// Bytes of video memory in use before anything of ours is loaded.
    pub vram_held: Option<u64>,
    /// Programs that would make a reading meaningless, named so somebody can close one.
    pub also_running: Vec<String>,
}

impl Busy {
    /// Whether this is too much to measure through.
    ///
    /// **A reading that could not be taken never blocks.** `None` is nobody asked, and refusing to
    /// benchmark on a machine whose CPU cannot be read would be reading silence as a fault — the
    /// inversion this codebase keeps paying for.
    pub fn too_busy(&self) -> bool {
        self.cpu.is_some_and(|it| it > CPU_LIMIT)
            || self.gpu.is_some_and(|it| it > GPU_LIMIT)
            || self.vram_held.is_some_and(|it| it > VRAM_HELD)
            || !self.also_running.is_empty()
    }

    /// One sentence somebody can act on.
    pub fn why(&self) -> String {
        let mut said = Vec::new();
        if let Some(cpu) = self.cpu.filter(|it| *it > CPU_LIMIT) {
            said.push(format!("the CPU is at {cpu:.0}%"));
        }
        if let Some(gpu) = self.gpu.filter(|it| *it > GPU_LIMIT) {
            said.push(format!("the card is at {gpu:.0}%"));
        }
        if let Some(held) = self.vram_held.filter(|it| *it > VRAM_HELD) {
            said.push(format!(
                "{:.1} GB of the card is already in use",
                held as f64 / 1e9
            ));
        }
        if !self.also_running.is_empty() {
            said.push(self.also_running.join(", "));
        }
        if said.is_empty() {
            return "the machine is quiet".to_owned();
        }
        said.join(" and ")
    }
}

/// Above this, the CPU is doing something other than serving the model.
///
/// Quiet and answering: 13% mean, 18% peak. Compiling: 32% mean, 88% peak. Twenty is between
/// them and not near either.
const CPU_LIMIT: f64 = 20.0;

/// Above this, something else is on the card. Idle here reads 7%.
const GPU_LIMIT: f64 = 20.0;

/// How long to watch before deciding. One sample can catch a spike that means nothing.
const WINDOW: Duration = Duration::from_secs(3);

/// Programs whose running makes a benchmark meaningless.
///
/// ## Narrowed twice, and the second time by being refused
///
/// The first version listed `python.exe` and `node.exe`, which would have blocked the search from
/// any harness written in either. Narrowed to inference servers and compilers — and the very
/// first real run was then refused with *LM Studio is running*.
///
/// Measured at that moment: LM Studio was offering two models and **holding none**, the card was
/// at 8% with 1206 MiB in use, which is the desktop. It was running and doing nothing.
///
/// > **Name what a program is doing, not that it exists.** Twice in one file the rule reached for
/// > a process list where a measurement was available, and the second time it had already been
/// > written down one constant above.
///
/// So a compiler is still named — `cargo` starting is CPU that has not arrived yet, and the
/// threshold cannot see the future. An inference server is asked what it is **holding** instead
/// ([`servers_holding_a_model`]), and a card somebody else is filling is caught by [`VRAM_HELD`]
/// whatever is responsible.
const NOISY: &[(&str, &str)] = &[
    ("cargo.exe", "cargo is running"),
    ("rustc.exe", "rustc is running"),
];

/// Which other inference servers have a model on the card right now.
///
/// **Holding, not running.** Each server is asked its own word for what is resident — the same
/// reading the Connections deck shows — so a server that is up and empty costs nothing and one
/// with a 7B loaded blocks. Epoch's own llama.cpp is not asked: it is what is being measured.
/// What every other inference server on this machine is doing, whether or not it is a problem.
///
/// ## Running is not the same as holding, and the report has to say both
///
/// The owner's correction, 2026-08-31: *"no quiero volver a cometer el error de LM Studio process
/// exists = benchmark invalid"*. A program can sit open with nothing on the card and cost the
/// measurement nothing — measured, repeatedly. So the gate has always keyed on what a server is
/// **holding** (`resident`), never on whether it is running.
///
/// What was missing is the other half. A server that is open and empty was reported as **nothing
/// at all**, so somebody reading a preflight could not tell *Epoch checked LM Studio and it is
/// idle* from *Epoch never looked*. That is the cold-instrument rule from its third direction:
/// not an invented reading and not a wrong one, but a real reading that was withheld.
///
/// `conflict` is the half that stops a benchmark. The rest is information.
pub fn other_servers() -> Vec<Neighbour> {
    [
        crate::runtimes::Runtime::Ollama,
        crate::runtimes::Runtime::LmStudio,
    ]
    .into_iter()
    .map(|runtime| {
        let seen = crate::runtimes::look_for(runtime);
        Neighbour {
            name: seen.name.to_string(),
            running: seen.serving,
            holding: seen.resident.clone(),
        }
    })
    .collect()
}

/// One other inference server, and what it is actually doing.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbour {
    pub name: String,
    pub running: bool,
    /// What it has on the card, in its own words. Empty is a measurement, not a silence.
    pub holding: Vec<String>,
}

impl Neighbour {
    /// Whether this would make a benchmark invalid.
    pub fn conflict(&self) -> bool {
        self.running && !self.holding.is_empty()
    }

    /// One line for a preflight report.
    pub fn say(&self) -> String {
        match (self.running, self.holding.is_empty()) {
            (false, _) => format!("{}: not running", self.name),
            (true, true) => format!("{}: running, holding nothing — informational", self.name),
            (true, false) => format!(
                "{}: running and holding {} — benchmark conflict",
                self.name,
                self.holding.join(", "),
            ),
        }
    }
}

fn servers_holding_a_model() -> Vec<String> {
    [
        crate::runtimes::Runtime::Ollama,
        crate::runtimes::Runtime::LmStudio,
    ]
    .into_iter()
    .filter_map(|runtime| {
        let seen = crate::runtimes::look_for(runtime);
        (seen.serving && !seen.resident.is_empty())
            .then(|| format!("{} is holding {}", seen.name, seen.resident.join(", ")))
    })
    .collect()
}

/// More video memory in use than a desktop needs means something else is on the card.
///
/// Measured across today's runs, before any model was loaded: 1.00 to 1.30 GB, which is the
/// window manager and a browser. Two gigabytes is above every one of those readings and far below
/// anything holding a model — a stopped-but-loaded ComfyUI sits at 2.2 GB and a served 7B at 7.5.
///
/// This is what the over-broad process list was really trying to catch, and it catches it by
/// measuring rather than by naming.
const VRAM_HELD: u64 = 2 * 1024 * 1024 * 1024;

/// Look at the machine now.
pub fn look() -> Busy {
    let mut cpu = Vec::new();
    let mut gpu = Vec::new();
    let mut held = Vec::new();
    let began = Instant::now();
    while began.elapsed() < WINDOW {
        if let Some(one) = cpu_percent() {
            cpu.push(one);
        }
        if let Some((busy, used)) = card() {
            gpu.push(busy);
            held.push(used as f64);
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    let mean = |of: &[f64]| (!of.is_empty()).then(|| of.iter().sum::<f64>() / of.len() as f64);
    let mut also = noisy_processes();
    also.extend(servers_holding_a_model());
    Busy {
        cpu: mean(&cpu),
        gpu: mean(&gpu),
        // The **least** seen rather than the mean: what is being asked is *what is somebody else
        // holding*, and a momentary spike is not a holding. The floor across the window is.
        vram_held: held
            .iter()
            .copied()
            .fold(None::<f64>, |low, it| {
                Some(low.map_or(it, |low| low.min(it)))
            })
            .map(|it| it as u64),
        also_running: also,
    }
}

/// Wait until the machine is quiet, or give up saying what it is doing.
///
/// `watching` is called with each reading so a surface can say *System busy — waiting for a clean
/// benchmark* rather than appearing to hang.
pub fn wait_until_quiet(patience: Duration, watching: &dyn Fn(&Busy)) -> Result<(), Busy> {
    let began = Instant::now();
    loop {
        let seen = look();
        if !seen.too_busy() {
            return Ok(());
        }
        watching(&seen);
        if began.elapsed() >= patience {
            return Err(seen);
        }
        std::thread::sleep(Duration::from_secs(5));
    }
}

#[cfg(windows)]
fn cpu_percent() -> Option<f64> {
    use crate::quiet::Quiet;
    let out = std::process::Command::new("powershell")
        .quiet()
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Processor | \
             Measure-Object -Property LoadPercentage -Average).Average",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(unix)]
fn cpu_percent() -> Option<f64> {
    use crate::quiet::Quiet;
    let out = std::process::Command::new("uptime")
        .quiet()
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let after = text.rsplit("load average").next()?;
    let one: f64 = after
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .split([',', ' '])
        .next()?
        .parse()
        .ok()?;
    let cores = std::thread::available_parallelism().ok()?.get() as f64;
    Some((one / cores * 100.0).min(100.0))
}

#[cfg(not(any(windows, unix)))]
fn cpu_percent() -> Option<f64> {
    None
}

/// Utilisation percent and bytes in use, from the card's own tool. One call for both.
fn card() -> Option<(f64, u64)> {
    use crate::quiet::Quiet;
    let out = std::process::Command::new("nvidia-smi")
        .quiet()
        .args([
            "--query-gpu=utilization.gpu,memory.used",
            "--format=csv,noheader,nounits",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.lines().next()?.split(',').map(str::trim);
    let busy: f64 = parts.next()?.parse().ok()?;
    let mib: u64 = parts.next()?.parse().ok()?;
    Some((busy, mib * 1024 * 1024))
}

/// Which of the noisy programs are running.
#[cfg(windows)]
fn noisy_processes() -> Vec<String> {
    use crate::quiet::Quiet;
    let Ok(out) = std::process::Command::new("tasklist")
        .quiet()
        .args(["/FO", "CSV", "/NH"])
        .stdin(std::process::Stdio::null())
        .output()
    else {
        // Unasked. An empty list here means *nothing was found*, and a machine that cannot be
        // asked has found nothing — which is the honest answer and does not block a run.
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut found: Vec<String> = NOISY
        .iter()
        .filter(|(exe, _)| text.contains(exe))
        .map(|(_, name)| (*name).to_owned())
        .collect();
    found.sort();
    found.dedup();
    found
}

#[cfg(not(windows))]
fn noisy_processes() -> Vec<String> {
    use crate::quiet::Quiet;
    let Ok(out) = std::process::Command::new("ps")
        .quiet()
        .args(["-Ao", "comm"])
        .stdin(std::process::Stdio::null())
        .output()
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut found: Vec<String> = NOISY
        .iter()
        .filter(|(exe, _)| text.contains(exe.trim_end_matches(".exe")))
        .map(|(_, name)| (*name).to_owned())
        .collect();
    found.sort();
    found.dedup();
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_machine_nobody_could_ask_never_blocks_a_run() {
        /*
            `None` is *unasked*. Refusing to benchmark on a machine whose CPU cannot be read would
            be reading silence as a fault — the inversion this codebase keeps paying for, arriving
            in a preflight check.
        */
        assert!(!Busy::default().too_busy());
        assert_eq!(Busy::default().why(), "the machine is quiet");
    }

    #[test]
    fn the_thresholds_sit_between_the_two_states_that_were_measured() {
        // Quiet and answering: 13% CPU mean. Compiling at the same time: 32%.
        let quiet = Busy {
            cpu: Some(13.0),
            gpu: Some(28.0),
            ..Busy::default()
        };
        assert!(
            quiet.cpu.is_some_and(|it| it < CPU_LIMIT),
            "a model answering is not a busy machine",
        );
        let building = Busy {
            cpu: Some(32.0),
            ..Busy::default()
        };
        assert!(building.too_busy(), "and a compilation is");
        assert!(building.why().contains("CPU"));
    }

    #[test]
    fn another_inference_server_blocks_however_idle_it_looks() {
        /*
            A second server can sit at 2% CPU holding four gigabytes of the card and change every
            number here. Named rather than inferred from load, for exactly that reason.
        */
        let quiet_but_shared = Busy {
            cpu: Some(4.0),
            gpu: Some(1.0),
            vram_held: None,
            also_running: vec!["Ollama is holding qwen3:14b".to_owned()],
        };
        assert!(quiet_but_shared.too_busy());
        assert!(quiet_but_shared.why().contains("Ollama"));

        // And a server that is up and empty is not a busy machine. Measured 2026-08-31: the first
        // real run of the search was refused because LM Studio was running, while LM Studio was
        // offering two models, holding none, on a card at 8%.
        assert!(!Busy {
            cpu: Some(4.0),
            gpu: Some(8.0),
            ..Busy::default()
        }
        .too_busy());
    }

    #[test]
    fn a_card_somebody_else_is_holding_blocks_even_at_nought_percent() {
        /*
            This is what the over-broad process list was reaching for. A stopped-but-loaded
            ComfyUI sits at 2.2 GB and 0% utilisation; measured before any model loaded today, a
            plain desktop is 1.00 to 1.30 GB. Measuring it beats naming `python.exe`, which would
            have blocked the search from any harness written in Python.
        */
        let desktop = Busy {
            vram_held: Some(1_200_000_000),
            ..Busy::default()
        };
        assert!(!desktop.too_busy());
        let shared = Busy {
            vram_held: Some(7_500_000_000),
            ..Busy::default()
        };
        assert!(shared.too_busy());
        assert!(
            shared.why().contains("card is already in use"),
            "{}",
            shared.why()
        );
    }

    #[test]
    fn what_is_wrong_is_said_in_one_sentence_somebody_can_act_on() {
        let both = Busy {
            cpu: Some(70.0),
            gpu: Some(90.0),
            vram_held: None,
            also_running: vec!["cargo is running".to_owned()],
        };
        let why = both.why();
        assert!(
            why.contains("70%") && why.contains("90%") && why.contains("cargo"),
            "{why}"
        );
    }

    #[test]
    fn looking_at_this_machine_answers_something() {
        // Not *what* — that depends on the machine — but that the readings are taken and the
        // structure survives whatever this one says.
        let seen = look();
        assert!(seen.cpu.is_none() || seen.cpu.is_some_and(|it| (0.0..=100.0).contains(&it)));
        assert!(seen.gpu.is_none() || seen.gpu.is_some_and(|it| (0.0..=100.0).contains(&it)));
    }
}
