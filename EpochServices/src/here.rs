//! What this machine has to lend: its models and its hardware.
//!
//! ## Measured on the machine that is offering it
//!
//! The Host never estimates what a remote can do. It asks, and this answers — which is the same
//! rule every probe in Epoch follows, applied across a network. A number this file cannot get is
//! reported as **unknown**, never as zero: a machine with no NVIDIA card has no VRAM reading,
//! and *nobody could tell you* is a different answer from *nothing fits*.
//!
//! ## Every runtime stays on localhost
//!
//! This talks to loopback and nothing else does. That is the whole security win of having a
//! companion program at all (ADR-0029 §9): without it, lending a machine means
//! `OLLAMA_HOST=0.0.0.0`, and then anything on that network can use the card.
//!
//! **There are three of them now**, and the rule did not change with the count — it got a
//! second half. A turn may say *which* program should run it, by an id from a closed set this
//! file already knows; it may never say *where*. Resolution happens here, on the machine that
//! owns the ports, so the worst a malformed request can do is name something that does not
//! exist and be told so.

use epoch_kernel::{Have, Runner};

/// Where the local runtime listens. **Loopback, and not configurable by a request** — a remote
/// that could name the address would be a remote that could point this at something else.
const OLLAMA: &str = "http://127.0.0.1:11434";

/// How a runtime is spoken to, which is not the same question as where it is.
///
/// Ollama has its own `/api/chat`; llama.cpp and LM Studio both serve the OpenAI-compatible
/// `/v1/chat/completions`. Epoch has spoken both since Phase 8 — this is the same pair of
/// dialects, one machine further away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Ollama,
    OpenAi,
}

/// Where one runner is and how to speak to it — resolved **here**, from an id.
///
/// `None` for an id this machine does not recognise. Refused rather than guessed at: routing a
/// turn to something other than what was asked for is worse than saying it could not be done.
pub fn runner_at(id: Option<&str>) -> Option<(Dialect, String)> {
    match id.unwrap_or("ollama") {
        // The default, and what every Host meant before an `Ask` could say anything else.
        "ollama" => Some((Dialect::Ollama, OLLAMA.to_owned())),
        other => epoch_models::runtimes::Runtime::ALL
            .into_iter()
            .find(|runtime| runtime.id() == other)
            .map(|runtime| {
                (
                    match runtime {
                        epoch_models::runtimes::Runtime::Ollama => Dialect::Ollama,
                        _ => Dialect::OpenAi,
                    },
                    // Built from the runtime's own usual port, on loopback. Nothing from the
                    // request reaches this string — and it is arithmetic rather than a survey,
                    // because a turn should not pay for a filesystem walk to learn a port it
                    // already knows.
                    format!("http://127.0.0.1:{}", runtime.usual_port()),
                )
            }),
    }
}

/// Every program on this machine that could run a model, and what each is holding.
///
/// **Measured by asking each one**, like everything else here. `installed` and `serving` stay
/// apart because they have different fixes — a machine with LM Studio installed and its server
/// switched off is not a machine without LM Studio.
pub fn runners() -> Vec<Runner> {
    epoch_models::runtimes::survey()
        .into_iter()
        .map(|seen| Runner {
            id: seen.id.to_owned(),
            name: seen.name.to_owned(),
            installed: seen.installed,
            serving: seen.serving,
            models: seen.models,
            // **Measured here, because only here can measure it.** The survey already asks each
            // program what it holds -- Ollama's `/api/ps`, llama.cpp's `loaded`, LM Studio's
            // `state` -- and the page has drawn it for a while. It simply never crossed, so a
            // Host had no reading and hid the control that depends on one.
            //
            // `Some` even when empty: this machine was asked, and *nothing is loaded* is an
            // answer.
            resident: Some(seen.resident),
        })
        .collect()
}

/// Every program on this machine that could make a picture, and what each is holding.
///
/// **A separate list from [`runners`], on purpose.** They are measured alike and drawn alike —
/// installed, serving, holding — and a diffusion server must never end up in the list a
/// character can be assigned a brain from (ADR-0030). One of these answers a turn; the other
/// one cannot hold a conversation at all.
///
/// `install` and `first_run` are carried because they are **this** machine's answers. A Host
/// looking at a MacBook has no business guessing `winget`, and the whole reason a lent machine
/// reports its own facts is that the Host cannot measure them.
pub fn easels() -> Vec<epoch_kernel::Studio> {
    epoch_models::studio::survey()
        .into_iter()
        .map(|seen| epoch_kernel::Studio {
            id: seen.id.to_owned(),
            name: seen.name.to_owned(),
            installed: seen.installed,
            serving: seen.serving,
            endpoint: seen.endpoint,
            models: seen.models,
            install: seen.install.to_owned(),
            first_run: seen.first_run.to_owned(),
        })
        .collect()
}

/// Everything this machine can offer, right now.
///
/// **The hardware half is `epoch-models`, not a copy of it.** This file had its own `nvidia-smi`
/// call and its own `sysctl`, written the same week the Engine's were — and they were not
/// identical: the shared one read `/proc/meminfo` on everything that was not Windows, which is
/// Linux, so it reported nothing on an Apple machine while this file reported 17 GB. Two answers
/// to one question, and the wrong one was the one the Workshop used.
pub fn measure() -> Have {
    let machine = epoch_models::Machine::measure();
    Have {
        models: models(),
        runners: runners(),
        gpu: machine.gpu,
        vram_total: machine.vram_total,
        vram_free: machine.vram_free,
        ram_total: machine.ram_total,
        unified: machine.unified,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        hf: Some(hugging_face()),
        easels: easels(),
    }
}

/// Whether *this* machine can put a GGUF on disk.
///
/// Asked here rather than assumed from the Host: a Host with `hf` installed says nothing about
/// the machine lending the card, and it is that machine a model would be downloaded onto.
///
/// The field copy is here rather than shared because `epoch-models` depends on nothing of
/// Epoch's, deliberately — so the thing it measures cannot be the thing that crosses a Bridge.
pub fn hugging_face() -> epoch_kernel::HuggingFace {
    let read = epoch_models::hf::cli();
    epoch_kernel::HuggingFace {
        installed: read.installed,
        version: read.version,
        found_at: read.found_at,
        user: read.user,
    }
}

/// What the local runtime says it has. An empty list is an honest answer — it means Ollama is
/// not running, or has pulled nothing.
pub fn models() -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct Tag {
        name: String,
    }
    #[derive(serde::Deserialize)]
    struct Tags {
        #[serde(default)]
        models: Vec<Tag>,
    }

    // **`timeout_connect`, and it is the whole cost of this page.** The five seconds below never
    // start until there is a socket, and a port nothing is listening on is not refused
    // instantly on this machine — it sits on the operating system's retry schedule. Measured:
    // with Ollama stopped, this one call took **21 seconds**, and it is called on every render,
    // so the page took 24. With the connect bounded it is 0.4 s and the page is immediate.
    //
    // The rule was already written down for the Bridge and for `look_for`, in those words, and
    // had never been applied to the one call this program makes on its own.
    ureq::builder()
        .timeout_connect(std::time::Duration::from_millis(400))
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .get(&format!("{OLLAMA}/api/tags"))
        .call()
        .ok()
        .and_then(|answer| answer.into_json::<Tags>().ok())
        .map(|tags| tags.models.into_iter().map(|t| t.name).collect())
        .unwrap_or_default()
}

/// Where the local runtime is, for whatever needs to reach it.
pub fn ollama() -> &'static str {
    OLLAMA
}

/// This machine's address on the network it is on.
///
/// **Found by asking the routing table, not by listing interfaces.** A machine with a VPN, a
/// container bridge and a wireless card has several addresses and only one of them is the one a
/// Host on the same network can reach — so it opens a UDP socket toward a public address,
/// reads which local address the kernel chose, and closes it. Nothing is sent.
pub fn address(port: u16) -> String {
    use std::net::UdpSocket;
    let ip = UdpSocket::bind("0.0.0.0:0")
        .and_then(|socket| {
            // Not contacted: connecting a UDP socket only fixes the route.
            socket.connect("8.8.8.8:80")?;
            socket.local_addr()
        })
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_owned());
    // **`https`**, because that is what the Host will find there. The scheme is part of the
    // address a machine reports about itself, and reporting the wrong one is reporting a door
    // that does not open.
    format!("https://{ip}:{port}")
}

/// What this machine calls itself, as a name a person will recognise on the other screen.
pub fn name() -> String {
    for key in ["COMPUTERNAME", "HOSTNAME", "HOST"] {
        if let Some(name) = std::env::var_os(key) {
            let name = name.to_string_lossy().trim().to_owned();
            if !name.is_empty() {
                return name;
            }
        }
    }
    // Unix without HOSTNAME exported, which is common: ask the system.
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "this machine".to_owned())
}

/// Where this machine's own ComfyUI is listening, or why there is nothing to ask.
///
/// **Measured, not assumed.** `studio::look_for` answers whether it is *serving*, which is the
/// difference between a program that is installed and one that can take a job — and the Host
/// asking a lent machine to draw deserves that distinction rather than a connection refused.
///
/// It is loopback on purpose: this address belongs to this machine and is never sent anywhere.
/// The Host reaches it through `/easel/*` and the bearer, which is the only door in.
pub fn easel_at() -> Result<String, String> {
    let seen = epoch_models::studio::look_for(epoch_models::studio::Studio::ComfyUi);
    match (seen.installed, seen.serving) {
        (_, true) => Ok(seen.endpoint),
        (true, false) => Err(format!(
            "{} is installed on this machine and is not running.",
            seen.name
        )),
        (false, false) => Err(format!("{} is not on this machine.", seen.name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_runtime_is_loopback_and_a_request_cannot_move_it() {
        // A remote that could name this address could point the program at something else.
        assert!(ollama().starts_with("http://127.0.0.1"));
    }

    #[test]
    fn a_turn_names_which_runtime_and_never_where_it_is() {
        // The second half of the same rule. Resolution happens on the machine that owns the
        // ports, so every address this function can ever return is one of its own.
        for id in [None, Some("ollama"), Some("llama_cpp"), Some("lm_studio")] {
            if let Some((_, endpoint)) = runner_at(id) {
                assert!(
                    endpoint.starts_with("http://127.0.0.1"),
                    "{id:?} resolved off loopback: {endpoint}"
                );
            }
        }
    }

    #[test]
    fn an_unknown_runtime_is_refused_rather_than_quietly_becoming_the_default() {
        // Running a turn on something other than what was asked for is worse than saying it
        // could not be done — the Host chose a machine *and* a program, and both were meant.
        assert!(runner_at(Some("http://evil.example")).is_none());
        assert!(runner_at(Some("vllm")).is_none());
        assert!(runner_at(Some("")).is_none());
    }

    #[test]
    fn the_default_is_what_every_older_host_meant() {
        // `None` is not a new state. It is what was on the wire before this field existed, and
        // it has to keep meaning exactly what it did.
        assert_eq!(runner_at(None), Some((Dialect::Ollama, OLLAMA.to_owned())));
    }

    #[test]
    fn this_machine_can_say_where_it_is_and_what_it_is_called() {
        // The two facts that stop a person ever typing an IP.
        let address = address(11500);
        // `https`, because that is what the Host will find at the address this machine reports.
        assert!(address.starts_with("https://"), "{address}");
        assert!(address.ends_with(":11500"), "{address}");
        assert!(!name().is_empty());
    }

    #[test]
    fn a_machine_with_no_nvidia_card_reports_no_vram_rather_than_zero() {
        // On an Apple machine memory is unified, and splitting it into a made-up "video" share
        // would be inventing a division the hardware does not have.
        let have = measure();
        if have.gpu.is_none() {
            assert_eq!(have.vram_total, None);
            assert_eq!(have.vram_free, None);
        }
        assert!(!have.version.is_empty(), "it says what it is");
    }
}
