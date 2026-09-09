//! What this computer actually is.
//!
//! ## Why measure at all
//!
//! A Models Workshop that lists what you could download and says nothing about whether it will
//! run is a shop with no prices. The number that decides is **free video memory**: a model that
//! does not fit is not slow, it is a different experience — it falls back to system memory and
//! answers at a fraction of the speed, or does not load.
//!
//! ## Measured, or absent — never guessed
//!
//! `nvidia-smi` is NVIDIA's, so a Windows or Linux machine with an AMD or Intel card has **no
//! VRAM reading here**, and that is reported as *unknown* rather than as zero. A guess would be
//! worse than silence: zero would tell somebody nothing fits, and a made-up number would tell
//! them something fits when it does not.
//!
//! This is the Launcher's rule about cold instruments, one layer down — a gauge nobody can
//! explain is worse than no gauge.
//!
//! ## And silence is not an answer either, where one exists
//!
//! Every Mac used to fall into that silence, because NVIDIA's tool is not the only question a
//! machine will answer. An Apple machine is asked with `sysctl` and `vm_stat` and reports what it
//! has; an Intel Mac's card is asked of `system_profiler`. The rule was never *only ask NVIDIA* —
//! it was *never invent a reading* — and reading `None` as a property of the platform rather than
//! of the question was the same inversion `Declared::uses_tools` made one crate over.
//!
//! On Apple Silicon there is no separate card memory at all, so [`Machine::unified`] says so and
//! every surface words it as what it is. A number that is right and described wrongly is still a
//! gauge nobody can explain.

/// What this machine has, as far as it can be asked.
use crate::quiet::Quiet;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    /// The graphics card's own name for itself. `None` when nothing could be asked.
    pub gpu: Option<String>,
    /// Video memory in bytes, total and free right now.
    ///
    /// Free rather than total is the number that decides, and it moves: a browser with enough
    /// tabs open takes a gigabyte of it. Both are carried so a surface can say *"9.5 of 12 GB
    /// free"* rather than implying the whole card is available.
    pub vram_total: Option<u64>,
    pub vram_free: Option<u64>,
    /// System memory in bytes. Where a model runs when it does not fit on the card.
    pub ram_total: Option<u64>,
    /// **The graphics memory *is* the system memory.** True on Apple Silicon.
    ///
    /// Carried because the numbers above mean something different when it is set, and a surface
    /// that says "12.1 of 17.2 GB free" about unified memory has told the truth in words that
    /// describe a dedicated card. There is nothing to spill *into* here: a model that does not
    /// fit does not quietly become slower, it does not load.
    ///
    /// `false` on every machine with a card of its own, and on every machine nothing could be
    /// asked about — the absence of a reading is not a claim about the architecture.
    pub unified: bool,
}

impl Machine {
    /// Ask this machine what it is.
    ///
    /// Every field is independent: a machine with no NVIDIA card still reports its RAM.
    pub fn measure() -> Self {
        let read = graphics();
        Self {
            gpu: read.0,
            vram_total: read.1,
            vram_free: read.2,
            unified: read.3,
            ram_total: ram(),
        }
    }

    /// Whether a model of this size will fit in free video memory.
    ///
    /// `None` when there is no reading to compare against — **not** `false`. "This will not fit"
    /// and "nobody could tell you" are different answers and only one of them is measured.
    ///
    /// The margin is real rather than superstitious: a model's weights are not the whole cost.
    /// Its context window is KV cache on the same card, and Epoch sizes that per turn
    /// (`provider::window_for`). A tenth is the smallest headroom that does not routinely lie.
    pub fn fits(&self, bytes: u64) -> Option<bool> {
        let free = self.vram_free?;
        Some(bytes.saturating_add(bytes / 10) <= free)
    }

    /// This machine's card, in one line somebody can read.
    ///
    /// **Free of total, not total.** A browser with enough tabs open takes a gigabyte, and a
    /// verdict against the whole card would say a model fits when it does not. Empty when there
    /// is nothing to report — a sentence about a card nobody found is the invented gauge the
    /// Launcher forbids.
    pub fn card(&self) -> String {
        match (&self.gpu, self.vram_free, self.vram_total) {
            (Some(gpu), Some(free), Some(total)) => format!(
                "{gpu} · {:.1} of {:.1} GB {}free",
                free as f64 / 1e9,
                total as f64 / 1e9,
                // Two words, and they are the difference between a card and a whole computer.
                if self.unified { "unified memory " } else { "" }
            ),
            (Some(gpu), _, _) => gpu.clone(),
            _ => String::new(),
        }
    }
}

/// This machine's graphics, however it can be asked.
///
/// Name, total, free, and whether that memory is the system's own. Every part independently
/// optional: a card that answers its name and not its memory says its name.
#[cfg(not(target_os = "macos"))]
fn graphics() -> (Option<String>, Option<u64>, Option<u64>, bool) {
    /*
        **NVIDIA's own tool first, and the runtime second.**

        `nvidia-smi` is the more precise reading of a live NVIDIA card and needs no second program
        installed, so where it answers it wins. Where it does not — an AMD or Intel card, which is
        most machines that are not this one — llama.cpp has already been asked what devices it can
        see, and it answers for the backend that would actually load the model.

        The rule was never *ask NVIDIA*. It was *never invent a reading*, and reading `None` as a
        property of the platform rather than of the question is the inversion this file was
        already corrected for once, on a Mac.
    */
    let (gpu, total, free) = nvidia();
    if gpu.is_some() {
        return (gpu, total, free, false);
    }
    let (gpu, total, free) = from_the_runtime();
    (gpu, total, free, false)
}

/// An Apple machine, which has no `nvidia-smi` and does not need one.
///
/// ## Why this was missing
///
/// `nvidia-smi` is NVIDIA's tool, so every Mac reported no graphics at all — and the Workshop's
/// verdict on whether a model fits went silent on the one platform where the answer is least
/// obvious. That was honest and it was incomplete: the machine can be asked, just not with
/// NVIDIA's question. It is the same defect `ram()` already carries a note about, one field over.
///
/// ## Apple Silicon: one pool, and saying so
///
/// The GPU has no memory of its own — it reads the system's. So the total is `hw.memsize` and the
/// free figure is what the kernel can hand out, and `unified` is set so no surface describes it
/// as a card's private store. **Cross-checked rather than assumed:** `free + inactive +
/// speculative` from `vm_stat` came to 7.10 GB on the owner's M2 at the same moment ComfyUI's own
/// torch build reported `ram_free: 7_108_509_696` — an independent reading of the same quantity,
/// agreeing to three digits.
///
/// The chip's name comes from `machdep.cpu.brand_string` (`Apple M2`) rather than from
/// `system_profiler`, which takes seconds to answer and is asked on every measurement. Checked
/// against it once: `system_profiler` reports `sppci_model: "Apple M2"` for the GPU — the same
/// string, because it is the same chip.
///
/// ## An Intel Mac: a real card, and no free reading
///
/// There the GPU is a separate device, so it is asked about separately, and there is no cheap way
/// to read how much of its memory is in use. Total and name are reported; free stays `None`, which
/// makes `fits` answer *nobody could tell you* rather than a number nobody measured.
#[cfg(target_os = "macos")]
fn graphics() -> (Option<String>, Option<u64>, Option<u64>, bool) {
    if cfg!(target_arch = "aarch64") {
        let name = sysctl("machdep.cpu.brand_string");
        return (name, sysctl_number("hw.memsize"), free_pages(), true);
    }
    let (name, total) = intel_mac_card();
    (name, total, None, false)
}

/// One `sysctl` string.
#[cfg(target_os = "macos")]
fn sysctl(key: &str) -> Option<String> {
    let out = std::process::Command::new("sysctl")
        .quiet()
        .args(["-n", key])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let said = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!said.is_empty()).then_some(said)
}

#[cfg(target_os = "macos")]
fn sysctl_number(key: &str) -> Option<u64> {
    sysctl(key)?.parse().ok()
}

/// What the kernel could hand out right now, in bytes.
///
/// `free` alone is misleadingly small on a Mac — the kernel keeps almost nothing unused. Inactive
/// and speculative pages are reclaimable, and counting them is what makes this agree with what
/// torch reports for the same machine.
#[cfg(target_os = "macos")]
fn free_pages() -> Option<u64> {
    let page = sysctl_number("hw.pagesize")?;
    let out = std::process::Command::new("vm_stat")
        .quiet()
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let said = String::from_utf8_lossy(&out.stdout);
    let count = |label: &str| -> u64 {
        said.lines()
            .find(|line| line.starts_with(label))
            .and_then(|line| line.rsplit(':').next())
            .map(|n| n.trim().trim_end_matches('.'))
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
    };
    let pages = count("Pages free") + count("Pages inactive") + count("Pages speculative");
    (pages > 0).then(|| pages * page)
}

/// The graphics card of an Intel Mac, from the tool that knows about it.
#[cfg(target_os = "macos")]
fn intel_mac_card() -> (Option<String>, Option<u64>) {
    let out = std::process::Command::new("system_profiler")
        .quiet()
        .args(["SPDisplaysDataType", "-json"])
        .stdin(std::process::Stdio::null())
        .output();
    let Ok(out) = out else {
        return (None, None);
    };
    let Ok(said) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return (None, None);
    };
    // The first GPU. A machine with two is one whose second Epoch has no way to choose, which is
    // the same rule `nvidia()` follows.
    let Some(card) = said
        .get("SPDisplaysDataType")
        .and_then(|it| it.as_array())
        .and_then(|all| all.first())
    else {
        return (None, None);
    };
    let name = card
        .get("sppci_model")
        .or_else(|| card.get("_name"))
        .and_then(|it| it.as_str())
        .map(str::to_owned);
    // Reported as words — `"4 GB"`, `"1536 MB"` — because it is written for a person to read.
    let vram = card
        .get("sppci_vram")
        .or_else(|| card.get("spdisplays_vram"))
        .and_then(|it| it.as_str())
        .and_then(|said| {
            let mut parts = said.split_whitespace();
            let amount: u64 = parts.next()?.parse().ok()?;
            match parts.next()? {
                "GB" => Some(amount * 1024 * 1024 * 1024),
                "MB" => Some(amount * 1024 * 1024),
                _ => None,
            }
        });
    (name, vram)
}

/// Ask the program that actually loads models onto the card.
///
/// ## Why this exists, and it is the AMD trip's first finding
///
/// `nvidia-smi` is NVIDIA's. On the owner's second machine — an AMD Radeon RX 9060 XT with 16 GB —
/// `THIS MACHINE` read *34.3 GB system memory* and **nothing about the card at all**, so nothing
/// could be weighed against it and the Workshop could not say whether a model would fit. Reported
/// from that machine, with a photograph of Task Manager showing the card that Epoch could not see.
///
/// The reading was on the screen the whole time, two panels down: `llama-server --list-devices`
/// had already printed
///
/// ```text
/// Vulkan0: AMD Radeon RX 9060 XT (16384 MiB, 15443 MiB free)
/// ```
///
/// **That is the better measurement for Epoch's question, not merely an available one.** What a
/// surface here asks is *will this model load on that card* — and this is the answer given by the
/// program that would be loading it, through the backend it would use. A driver query would report
/// what the hardware has; this reports what the runtime can see, which is what decides.
///
/// Second, therefore, and never first: where `nvidia-smi` answers it is the more precise reading
/// of a live card, and it needs no second program installed.
#[cfg(not(target_os = "macos"))]
fn from_the_runtime() -> (Option<String>, Option<u64>, Option<u64>) {
    for line in crate::runtimes::devices_of(crate::runtimes::Runtime::LlamaCpp) {
        // `Vulkan0: AMD Radeon RX 9060 XT (16384 MiB, 15443 MiB free)`
        let Some((_, rest)) = line.split_once(':') else {
            continue;
        };
        let Some((name, sizes)) = rest.rsplit_once('(') else {
            continue;
        };
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let mib = |raw: &str| -> Option<u64> {
            raw.split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
                .map(|n| n * 1024 * 1024)
        };
        let mut halves = sizes.trim_end_matches(')').split(',');
        let total = halves.next().and_then(mib);
        let free = halves.next().and_then(mib);
        // A backend with no memory of its own — `BLAS: Accelerate (0 MiB, 0 MiB free)` on a Mac,
        // and the CPU device on any machine — is not a card, and reporting it as one would be a
        // real reading of the wrong quantity.
        if total.is_some_and(|n| n > 0) {
            return (Some(name.to_owned()), total, free);
        }
    }
    (None, None, None)
}

/// The graphics driver, where the machine will say.
///
/// ## Why a benchmark records it
///
/// A driver update changes throughput and leaves nothing on screen to say so. A fingerprint that
/// records it lets a later comparison report *the driver changed* instead of *your machine is
/// degraded* — which is the difference between somebody updating a note and somebody hunting a
/// hardware fault that does not exist.
///
/// **`None` on every machine that will not say**, including every Mac: `nvidia-smi` is NVIDIA's,
/// and inventing a version string would put a fabricated value into the one record whose whole
/// job is to be trustworthy. Unrecorded is a state a fingerprint already handles.
///
/// Not part of the required set: a reference taken before this was read is a reference with one
/// fewer guarantee, not a useless one.
pub fn driver() -> Option<String> {
    let out = std::process::Command::new("nvidia-smi")
        .quiet()
        .args(["--query-gpu=driver_version", "--format=csv,noheader"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let said = String::from_utf8_lossy(&out.stdout);
    let first = said.lines().next()?.trim();
    (!first.is_empty()).then(|| first.to_owned())
}

/// Ask NVIDIA's own tool. Absent driver, absent card, absent answer.
///
/// **Not compiled on macOS**, where `graphics()` is a different function and nothing calls this.
/// Without the attribute the Mac warns `never used`, which is nothing on a Windows CI and a
/// failed build on a macOS one — `-D warnings` is set for both.
///
/// It is also where that attribute belonged. It had drifted **one item down** onto `driver()`,
/// between that function's own two doc comments, and took `driver()` off macOS with it — while
/// `driver()`'s doc went on saying *`None` on every machine that will not say, including every
/// Mac*. So `epoch-tauri` called a function that did not exist there and **the whole application
/// stopped compiling on macOS**, which nothing on Windows could see. Found by running the suite
/// on the Mac.
///
/// The same shape as the four found in `4998365`: an attribute that survived an edit its code
/// did not.
#[cfg(not(target_os = "macos"))]
fn nvidia() -> (Option<String>, Option<u64>, Option<u64>) {
    let asked = std::process::Command::new("nvidia-smi")
        .quiet()
        .args([
            "--query-gpu=name,memory.total,memory.free",
            "--format=csv,noheader,nounits",
        ])
        .stdin(std::process::Stdio::null())
        .output();

    let Ok(out) = asked else {
        return (None, None, None);
    };
    if !out.status.success() {
        return (None, None, None);
    }

    let said = String::from_utf8_lossy(&out.stdout);
    // The first card. A machine with two is a machine whose second card Epoch has no way to
    // choose, so reporting the first is honest and reporting a sum would not be.
    let Some(line) = said.lines().next() else {
        return (None, None, None);
    };
    let parts: Vec<&str> = line.split(',').map(str::trim).collect();
    let [name, total, free] = parts[..] else {
        return (None, None, None);
    };
    // `nounits` gives mebibytes.
    let mib = |raw: &str| raw.parse::<u64>().ok().map(|n| n * 1024 * 1024);
    (Some(name.to_owned()), mib(total), mib(free))
}

/// Total system memory, from the operating system.
#[cfg(windows)]
fn ram() -> Option<u64> {
    let out = std::process::Command::new("powershell")
        .quiet()
        .args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

/// macOS keeps this in the kernel, and has no `/proc` to read it from.
///
/// **Found by running it there.** This file was written on Windows with two branches — Windows
/// and "everything else" — and everything else read `/proc/meminfo`, which is Linux. On an Apple
/// machine that path does not exist, so the reading came back `None`, and because an Apple
/// machine also has no NVIDIA card to ask about, the Workshop said *hardware could not be read*
/// about a machine it could read perfectly well.
///
/// Three platforms, three branches. "Not Windows" was never a platform.
#[cfg(target_os = "macos")]
fn ram() -> Option<u64> {
    let out = std::process::Command::new("sysctl")
        .quiet()
        .args(["-n", "hw.memsize"])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout).trim().parse().ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn ram() -> Option<u64> {
    // `/proc/meminfo` reports kibibytes on the `MemTotal:` line.
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// Anything that is neither Windows nor a Unix. There is no reading to give, and saying so is
/// the honest answer rather than a zero.
#[cfg(not(any(windows, unix)))]
fn ram() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {

    /// The line an AMD machine answered with, and every shape beside it.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_card_is_read_from_the_program_that_would_load_on_it() {
        // Measured on the owner's second machine, reported with a photograph of Task Manager
        // showing the card Epoch could not see:
        //   Vulkan0: AMD Radeon RX 9060 XT (16384 MiB, 15443 MiB free)
        let read = |line: &str| -> (Option<String>, Option<u64>, Option<u64>) {
            let Some((_, rest)) = line.split_once(':') else {
                return (None, None, None);
            };
            let Some((name, sizes)) = rest.rsplit_once('(') else {
                return (None, None, None);
            };
            let mib = |raw: &str| -> Option<u64> {
                raw.split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()
                    .map(|n| n * 1024 * 1024)
            };
            let mut halves = sizes.trim_end_matches(')').split(',');
            (
                Some(name.trim().to_owned()),
                halves.next().and_then(mib),
                halves.next().and_then(mib),
            )
        };

        let (name, total, free) =
            read("Vulkan0: AMD Radeon RX 9060 XT (16384 MiB, 15443 MiB free)");
        assert_eq!(name.as_deref(), Some("AMD Radeon RX 9060 XT"));
        assert_eq!(total, Some(16384 * 1024 * 1024));
        assert_eq!(free, Some(15443 * 1024 * 1024));

        // The same shape from a CUDA build, and from a Mac's Accelerate line - which has no memory
        // of its own and must never be reported as a card.
        let (nv, nvt, _) =
            read("Vulkan0: NVIDIA GeForce RTX 4070 SUPER (12281 MiB, 11069 MiB free)");
        assert_eq!(nv.as_deref(), Some("NVIDIA GeForce RTX 4070 SUPER"));
        assert_eq!(nvt, Some(12281 * 1024 * 1024));

        let (_, blas, _) = read("BLAS: Accelerate (0 MiB, 0 MiB free)");
        assert_eq!(blas, Some(0), "and zero is what makes it not a card");
    }
    #[test]
    fn this_machine_reports_its_memory_whatever_it_is_running_on() {
        // The defect this closes: two branches, Windows and "everything else", where everything
        // else read `/proc/meminfo`. On macOS that path does not exist, so an Apple machine —
        // which also has no NVIDIA card to ask about — reported *hardware could not be read*
        // about a machine it could read perfectly well. "Not Windows" was never a platform.
        //
        // Every system this runs on has a way to answer, so on every system this must answer.
        let machine = Machine::measure();
        assert!(
            machine.ram_total.is_some(),
            "no memory reading on {}",
            std::env::consts::OS
        );
        assert!(
            machine.ram_total.unwrap_or(0) > 1_000_000_000,
            "a machine with under a gigabyte is not running this"
        );
    }

    #[test]
    fn a_machine_with_no_card_says_nothing_rather_than_something() {
        // A sentence about a card nobody found is the invented gauge the Launcher forbids.
        assert_eq!(Machine::default().card(), "");
        // And a verdict nobody could reach is `None`, never `false`.
        assert_eq!(Machine::default().fits(1), None);
    }

    /// One pool has to be described as one pool.
    ///
    /// Measured on the owner's M2 (2026-08-25): 17.2 GB of unified memory, 7.1 GB of it free, and
    /// the GPU reads it directly. The numbers are real; describing them as a card's own store is
    /// the failure, because it implies a second pool to fall back into and there is none.
    #[test]
    fn unified_memory_is_named_as_what_it_is() {
        let mac = Machine {
            gpu: Some("Apple M2".to_owned()),
            vram_total: Some(17_179_869_184),
            vram_free: Some(7_100_000_000),
            ram_total: Some(17_179_869_184),
            unified: true,
        };
        assert_eq!(mac.card(), "Apple M2 · 7.1 of 17.2 GB unified memory free");
        // And a machine with a card of its own is untouched by it.
        let card = Machine {
            unified: false,
            ..mac
        };
        assert!(
            !card.card().contains("unified"),
            "a dedicated card must not borrow the words: {}",
            card.card()
        );
    }

    #[test]
    fn what_fits_is_measured_against_what_is_free_and_leaves_room() {
        let card = Machine {
            gpu: Some("A Card".to_owned()),
            vram_total: Some(12_000_000_000),
            vram_free: Some(10_000_000_000),
            ram_total: None,
            unified: false,
        };
        assert!(
            card.card().contains("10.0 of 12.0 GB free"),
            "{}",
            card.card()
        );
        // Nine gigabytes plus a tenth is 9.9, which fits in ten.
        assert_eq!(card.fits(9_000_000_000), Some(true));
        // Ten does not, because the weights are not the whole cost.
        assert_eq!(card.fits(10_000_000_000), Some(false));
    }

    use super::*;

    #[test]
    fn a_machine_with_no_reading_says_unknown_rather_than_no() {
        // The distinction the whole module exists for. "It will not fit" is a measurement;
        // "nobody could tell you" is the absence of one, and a surface must be able to tell
        // them apart.
        let unknown = Machine::default();
        assert_eq!(unknown.fits(1_000), None);
    }

    #[test]
    fn fitting_leaves_room_for_the_context_window() {
        // A model's weights are not its whole cost — its context is KV cache on the same card.
        let card = Machine {
            vram_free: Some(10_000),
            ..Machine::default()
        };
        assert_eq!(card.fits(9_000), Some(true));
        // 9,500 plus a tenth is 10,450, which does not fit in 10,000 — and a workshop that
        // said it did would be recommending a model that thrashes.
        assert_eq!(card.fits(9_500), Some(false));
    }

    #[test]
    #[ignore = "reads this machine's own hardware"]
    fn this_machine_answers() {
        let here = Machine::measure();
        println!("{here:#?}");
        assert!(
            here.ram_total.is_some(),
            "every machine can report its own memory"
        );
    }
}
