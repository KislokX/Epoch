//! What a run costs the machine while it is running.
//!
//! ## Why this is not [`crate::machine::Machine`]
//!
//! `Machine` answers *what is this computer* — asked once, before anything happens. This answers
//! *what did that cost*, and the answer only exists while the work is happening. A model that
//! spills onto the PCIe bus looks identical to one that does not from outside: same answer, same
//! tokens, a third of the speed and no reading anywhere saying why.
//!
//! ## Sampled, and the sampling rate is part of the reading
//!
//! Every value here is a **peak and a mean over samples taken during the run**, with the count
//! carried, because a peak from four samples and a peak from four hundred are not the same
//! evidence. Nothing here is interpolated and nothing is filled in: a machine that cannot be
//! asked reports `None`, which is *nobody asked*, never zero.
//!
//! ## What each thing can and cannot say
//!
//! - **GPU utilisation** — the fraction of the last sampling window in which the card was busy.
//!   A model running mostly off the card shows *low* GPU utilisation while generating, which is
//!   the whole reason it is worth sampling.
//! - **Video memory used** — the whole card, not this process. Something else drawing a window
//!   is in there too, which is why the *change across the run* is worth more than the absolute.
//! - **System memory used** — likewise the whole machine.
//! - **CPU** — the average across cores.
//!
//! **What is deliberately absent: PCIe traffic.** `nvidia-smi` can report it on some cards, and
//! it is not reported here because it was not measurable on the card this was written against —
//! a consumer GeForce answers `[N/A]` to the throughput queries. Offering a column that is empty
//! on the hardware most people have would be an instrument nobody can read. The *consequence* of
//! the traffic is measured instead, and it is the number that decides anything: tokens per second
//! beside how much of the model was on the card.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::quiet::Quiet;

/// One instrument's readings across a run.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub peak: f64,
    pub mean: f64,
    /// How many samples it is made of. **A peak from four samples is not a peak from four
    /// hundred**, and a reading that did not say so would be inviting a comparison it cannot
    /// support.
    pub samples: usize,
}

impl Reading {
    fn of(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        Some(Self {
            peak: values.iter().copied().fold(f64::MIN, f64::max),
            mean: values.iter().sum::<f64>() / values.len() as f64,
            samples: values.len(),
        })
    }
}

/// What a run cost, as far as this machine could be asked.
///
/// Every field independent: a machine with no NVIDIA card still reports its memory and its CPU.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Load {
    /// Percent of the sampling window the card was busy.
    pub gpu_percent: Option<Reading>,
    /// Bytes of video memory in use across the whole card.
    pub vram_used: Option<Reading>,
    /// Bytes of system memory in use across the whole machine.
    pub ram_used: Option<Reading>,
    /// Percent, averaged across cores.
    pub cpu_percent: Option<Reading>,
    /// Video memory in use before anything started, so the *change* can be read rather than only
    /// the absolute — the card holds a desktop as well as a model.
    pub vram_before: Option<u64>,
    pub ram_before: Option<u64>,
    /// Bytes in the GPU's **shared** pool, read once before and once after.
    ///
    /*
        **Never sampled.** It is the clearest signal a model instance has been evicted — measured
        2026-08-31, an eviction moved about 70 MiB into it and halved throughput for the life of
        that instance — and on Windows it comes from `Get-Counter`, which costs the better part of
        a second. Two readings a run is affordable; two a second is the instrument becoming the
        load, which this file has already been guilty of once.
    */
    pub shared_before: Option<u64>,
    pub shared_after: Option<u64>,
    /// How long the sampled stretch was.
    pub seconds: f64,
    /// What the instrument itself was doing, so a reading can be judged.
    ///
    /// **An instrument whose cost is unknown is a reading with an unknown correction on it.**
    /// This says how often each thing was asked; the overhead follows from that and the cost of a
    /// call, both of which are written down where they are chosen.
    pub sampling: Sampling,
}

/// How the readings above were taken.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sampling {
    pub card_every_ms: u64,
    pub machine_every_ms: u64,
    /// Shared GPU memory, which is read this many times in total rather than on a clock.
    pub shared_reads: u32,
}

impl Default for Sampling {
    fn default() -> Self {
        Self {
            card_every_ms: EVERY.as_millis() as u64,
            machine_every_ms: EVERY_SLOW.as_millis() as u64,
            shared_reads: 2,
        }
    }
}

impl Sampling {
    /// Roughly what share of one core this costs, from the measured cost of each call.
    ///
    /// `nvidia-smi` is about 60 ms here and the combined Windows counter about 400 ms. Reported
    /// rather than asserted: the point is that the number is *knowable*, and that it was 160%
    /// before anybody worked it out.
    pub fn overhead(&self) -> f64 {
        let card = 60.0 / self.card_every_ms as f64;
        let machine = 400.0 / self.machine_every_ms as f64;
        card + machine
    }
}

impl Load {
    /// How much more video memory was in use at the peak than before the run began.
    ///
    /// **The number that says whether the model is on the card**, and `None` rather than a
    /// difference where either half was never measured.
    pub fn vram_taken(&self) -> Option<u64> {
        let peak = self.vram_used?.peak;
        let before = self.vram_before? as f64;
        Some((peak - before).max(0.0) as u64)
    }

    pub fn ram_taken(&self) -> Option<u64> {
        let peak = self.ram_used?.peak;
        let before = self.ram_before? as f64;
        Some((peak - before).max(0.0) as u64)
    }
}

/// How often to ask the card.
///
/// **Half a second.** `nvidia-smi` costs about 60 ms here, so this spends roughly an eighth of one
/// core — small against a model, and fine enough to see the shape of a short run.
const EVERY: Duration = Duration::from_millis(500);

/// How often to ask the operating system about memory and the CPU.
///
/*
    **Ten times less often than the card, and the reason is that this instrument was changing its
    own reading.**

    On Windows both answers come from `Get-CimInstance`, which costs three to seven hundred
    milliseconds a call. The first version made two of those every five hundred milliseconds — so
    a PowerShell process was running essentially continuously for the whole benchmark.

    Measured 2026-08-31, the same model at the same context:

    | | CPU peak / mean |
    |---|---|
    | measured by a script that samples nothing | 18% / 13% |
    | measured through this sampler | 85% / 55% |

    The sampler was not observing a busy machine, it was **making** one — and on a model that is
    CPU-bound because two thirds of it sits in host memory, that is not a cosmetic error in a
    gauge. It changes the number being measured.

    > **An instrument that costs a measurable share of the thing it measures is not an instrument.**
    > The rule this project already had — a gauge nobody can explain is worse than no gauge — has a
    > sibling: a gauge that moves the needle it reads is worse than reading it less often.

    Five seconds gives about twelve samples in a minute, which is enough for a mean, and one call
    rather than two halves what is left.
*/
const EVERY_SLOW: Duration = Duration::from_secs(5);

/// Watch the machine until [`Watch::stop`] is called.
///
/// **A thread rather than a reading before and after.** Before-and-after cannot see a peak, and
/// the peak is the interesting half: a model that fits at rest and spills while its cache fills
/// looks fine at both ends.
pub struct Watch {
    running: Arc<AtomicBool>,
    taken: Arc<Mutex<Samples>>,
    began: std::time::Instant,
    started_with: (Option<u64>, Option<u64>),
    shared_before: Option<u64>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[derive(Default)]
struct Samples {
    gpu: Vec<f64>,
    vram: Vec<f64>,
    ram: Vec<f64>,
    cpu: Vec<f64>,
}

impl Watch {
    /// Begin watching.
    ///
    /// The *before* readings are taken on this thread, so they are genuinely before rather than
    /// half a second into the run — which is exactly when a model starts loading.
    pub fn begin() -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let taken = Arc::new(Mutex::new(Samples::default()));
        let started_with = (gpu().map(|it| it.1), ram_used());
        let shared_before = shared();

        let mine = (Arc::clone(&running), Arc::clone(&taken));
        let thread = std::thread::spawn(move || {
            let (running, taken) = mine;
            let mut asked_slowly = std::time::Instant::now() - EVERY_SLOW;
            while running.load(Ordering::Relaxed) {
                let card = gpu();
                // The expensive pair, on its own clock. See `EVERY_SLOW`.
                let slow = if asked_slowly.elapsed() >= EVERY_SLOW {
                    asked_slowly = std::time::Instant::now();
                    Some(machine_load())
                } else {
                    None
                };
                if let Ok(mut into) = taken.lock() {
                    if let Some((busy, used)) = card {
                        into.gpu.push(busy);
                        into.vram.push(used as f64);
                    }
                    if let Some((memory, load)) = slow {
                        if let Some(used) = memory {
                            into.ram.push(used as f64);
                        }
                        if let Some(load) = load {
                            into.cpu.push(load);
                        }
                    }
                }
                // Waited in slices, so a finished run is not sat out for half a second.
                for _ in 0..5 {
                    if !running.load(Ordering::Relaxed) {
                        return;
                    }
                    std::thread::sleep(EVERY / 5);
                }
            }
        });

        Self {
            running,
            taken,
            began: std::time::Instant::now(),
            started_with,
            shared_before,
            thread: Some(thread),
        }
    }

    /// Stop, and say what was seen.
    pub fn stop(mut self) -> Load {
        self.running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let seconds = self.began.elapsed().as_secs_f64();
        // The second and last reading of the expensive one, after the work rather than during it.
        let shared_after = shared();
        let taken = match self.taken.lock() {
            Ok(it) => it,
            // A poisoned lock loses the samples, not the run. Nothing measured is `None`, which
            // is what `None` means everywhere else here.
            Err(_) => {
                return Load {
                    seconds,
                    shared_before: self.shared_before,
                    shared_after,
                    ..Load::default()
                }
            }
        };
        Load {
            gpu_percent: Reading::of(&taken.gpu),
            vram_used: Reading::of(&taken.vram),
            ram_used: Reading::of(&taken.ram),
            cpu_percent: Reading::of(&taken.cpu),
            vram_before: self.started_with.0,
            ram_before: self.started_with.1,
            shared_before: self.shared_before,
            shared_after,
            seconds,
            sampling: Sampling::default(),
        }
    }
}

/// Bytes in the GPU's shared pool — the host memory the driver spills a full card into.
///
/// **The signal that identifies an eviction**, and the most expensive reading on the machine, so
/// it is asked twice a run and never on a clock. `None` where the counter is not there, which is
/// every platform but Windows and some Windows machines too — and `None` never blocks anything:
/// the collapse detector requires *either* corroboration, not both.
#[cfg(windows)]
pub fn shared() -> Option<u64> {
    use crate::quiet::Quiet;
    let out = std::process::Command::new("powershell")
        .quiet()
        .args([
            "-NoProfile",
            "-Command",
            "((Get-Counter '\\GPU Adapter Memory(*)\\Shared Usage' -ErrorAction Stop)\
             .CounterSamples | Measure-Object -Property CookedValue -Sum).Sum",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<f64>()
        .ok()
        .map(|it| it as u64)
}

#[cfg(not(windows))]
pub fn shared() -> Option<u64> {
    // No equivalent counter is read here yet. Unasked, which is what `None` means everywhere in
    // this file — and the detector is built so one missing signal costs it nothing.
    None
}

/// Utilisation percent and bytes used, from the card's own tool.
///
/// One call for both, because two calls are two process spawns and the pair is wanted together.
fn gpu() -> Option<(f64, u64)> {
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
    let line = text.lines().next()?;
    let mut parts = line.split(',').map(str::trim);
    let busy: f64 = parts.next()?.parse().ok()?;
    let mib: u64 = parts.next()?.parse().ok()?;
    Some((busy, mib * 1024 * 1024))
}

/// Memory in use and CPU load, in **one** call where the platform allows it.
///
/// Two separate `Get-CimInstance` invocations were two process spawns of several hundred
/// milliseconds each, ten times a second. One call is half of what is left after `EVERY_SLOW`.
#[cfg(windows)]
fn machine_load() -> (Option<u64>, Option<f64>) {
    use crate::quiet::Quiet;
    let Ok(out) = std::process::Command::new("powershell")
        .quiet()
        .args([
            "-NoProfile",
            "-Command",
            "$o=Get-CimInstance Win32_OperatingSystem; \
             $c=(Get-CimInstance Win32_Processor | \
             Measure-Object -Property LoadPercentage -Average).Average; \
             \"$($o.TotalVisibleMemorySize) $($o.FreePhysicalMemory) $c\"",
        ])
        .stdin(std::process::Stdio::null())
        .output()
    else {
        return (None, None);
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let total: Option<u64> = parts.next().and_then(|it| it.parse().ok());
    let free: Option<u64> = parts.next().and_then(|it| it.parse().ok());
    let cpu: Option<f64> = parts.next().and_then(|it| it.parse().ok());
    (
        total
            .zip(free)
            .map(|(total, free)| total.saturating_sub(free) * 1024),
        cpu,
    )
}

/// Elsewhere the two come from different places and are cheap, so they stay two calls.
#[cfg(not(windows))]
fn machine_load() -> (Option<u64>, Option<f64>) {
    (ram_used(), cpu())
}

#[cfg(windows)]
#[allow(dead_code)]
fn ram_used() -> Option<u64> {
    // Both numbers from one call: in use is total minus free, and asking twice could straddle a
    // change. Reported in kibibytes.
    let out = std::process::Command::new("powershell")
        .quiet()
        .args([
            "-NoProfile",
            "-Command",
            "$o=Get-CimInstance Win32_OperatingSystem; \
             \"$($o.TotalVisibleMemorySize) $($o.FreePhysicalMemory)\"",
        ])
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let mut parts = text.split_whitespace();
    let total: u64 = parts.next()?.parse().ok()?;
    let free: u64 = parts.next()?.parse().ok()?;
    Some(total.saturating_sub(free) * 1024)
}

#[cfg(target_os = "macos")]
fn ram_used() -> Option<u64> {
    // `vm_stat` counts pages. In use is everything that is neither free nor speculative.
    let out = std::process::Command::new("vm_stat")
        .quiet()
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let page: u64 = text
        .lines()
        .next()?
        .split_whitespace()
        .find_map(|word| word.trim_end_matches('.').parse().ok())
        .unwrap_or(4096);
    let count = |name: &str| -> u64 {
        text.lines()
            .find(|line| line.starts_with(name))
            .and_then(|line| line.split(':').nth(1))
            .and_then(|rest| rest.trim().trim_end_matches('.').parse().ok())
            .unwrap_or(0)
    };
    let free = count("Pages free") + count("Pages speculative");
    let total = crate::machine::Machine::measure().ram_total?;
    Some(total.saturating_sub(free * page))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn ram_used() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kib = |name: &str| -> Option<u64> {
        text.lines()
            .find(|line| line.starts_with(name))?
            .split_whitespace()
            .nth(1)?
            .parse()
            .ok()
    };
    Some((kib("MemTotal:")?.saturating_sub(kib("MemAvailable:")?)) * 1024)
}

#[cfg(not(any(windows, unix)))]
fn ram_used() -> Option<u64> {
    None
}

#[cfg(windows)]
#[allow(dead_code)]
fn cpu() -> Option<f64> {
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

/// The one-minute load average, as a percentage of the cores there are.
///
/// **Not the same instrument as Windows'** — a queue length rather than a duty cycle — and worth
/// saying so where it is defined: comparing a CPU figure across platforms compares two different
/// questions, which is exactly the shape of gauge this project keeps having to correct.
#[cfg(unix)]
fn cpu() -> Option<f64> {
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
fn cpu() -> Option<f64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reading_of_nothing_is_nothing_rather_than_zero() {
        // The rule this whole file follows: a machine that could not be asked reports `None`.
        // A zero would say the card was idle, which is a claim nobody measured.
        assert_eq!(Reading::of(&[]), None);
    }

    #[test]
    fn a_reading_carries_how_many_samples_it_is_made_of() {
        let it = Reading::of(&[10.0, 30.0, 20.0]).expect("three samples");
        assert_eq!(it.peak, 30.0);
        assert_eq!(it.mean, 20.0);
        assert_eq!(
            it.samples, 3,
            "a peak from three is not a peak from three hundred"
        );
    }

    #[test]
    fn what_a_run_took_is_the_change_and_not_the_absolute() {
        /*
            The card holds a desktop as well as a model. A run that peaked at 11 GB on a card
            already holding 1 GB of windows took 10, and reporting 11 would charge the model for
            the browser.
        */
        let load = Load {
            vram_used: Some(Reading {
                peak: 11_000_000_000.0,
                mean: 9.0e9,
                samples: 20,
            }),
            vram_before: Some(1_000_000_000),
            ..Load::default()
        };
        assert_eq!(load.vram_taken(), Some(10_000_000_000));
    }

    #[test]
    fn a_change_with_half_of_it_unmeasured_is_not_a_change() {
        // Neither half invented. The same rule as `Machine::fits` answering `None`.
        let no_before = Load {
            vram_used: Some(Reading {
                peak: 11.0e9,
                mean: 9.0e9,
                samples: 20,
            }),
            ..Load::default()
        };
        assert_eq!(no_before.vram_taken(), None);
        assert_eq!(Load::default().vram_taken(), None);
    }

    #[test]
    fn the_instrument_knows_roughly_what_it_costs() {
        /*
            **This is the number that was 160% and nobody had worked it out.** Two
            `Get-CimInstance` calls of about 400 ms every 500 ms is more than three cores' worth
            of asking; it took CPU from 13% to 55% on the model it was measuring.

            Reported rather than asserted at a threshold, because the cost of a call is a property
            of the machine. What is asserted is that the current settings are in the low single
            digits rather than in the hundreds.
        */
        let now = Sampling::default();
        assert!(
            now.overhead() < 0.25,
            "a quarter of one core at most: {:.2}",
            now.overhead(),
        );

        let before = Sampling {
            card_every_ms: 500,
            machine_every_ms: 500,
            shared_reads: 0,
        };
        assert!(
            before.overhead() > 0.8,
            "and what it used to be: {:.2}",
            before.overhead(),
        );
    }

    #[test]
    fn the_shared_pool_is_read_twice_and_never_sampled() {
        // It is the clearest eviction signal and the most expensive reading on the machine. Two a
        // run is affordable; two a second is the instrument becoming the load.
        assert_eq!(Sampling::default().shared_reads, 2);
    }

    #[test]
    fn a_watch_that_is_started_and_stopped_reports_how_long_it_ran() {
        // Not what it read — that depends on the machine — but that it survives being started and
        // stopped, which is the part a test can hold anywhere.
        let watch = Watch::begin();
        std::thread::sleep(Duration::from_millis(120));
        let load = watch.stop();
        assert!(load.seconds >= 0.1, "{}", load.seconds);
    }
}
