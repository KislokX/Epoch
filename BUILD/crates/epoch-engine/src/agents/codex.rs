//! Codex, hosted as a character's brain — the second agent, and the test of ADR-0027.
//!
//! ## Why this file is the evidence
//!
//! `Agent` was written against exactly one implementation, deliberately: the same discipline
//! `Provider` followed with Ollama. A trait shaped for agents nobody has run is a guess. So the
//! question this file answers is not "does Codex work" but **"did the abstraction survive a
//! second, differently-shaped program?"**
//!
//! It did, with one honest gap: Codex reports how many tokens it used and **not how large its
//! window is**, so Epoch's context gauge stays empty for it rather than being given a denominator
//! nobody measured.
//!
//! ## Everything here was measured against the real program
//!
//! Not one line of this is remembered. Two of the findings would have been wrong from memory, and
//! one of them is the kind that breaks a week after shipping:
//!
//! - **Its executable moves.** Codex installs into `%LOCALAPPDATA%\OpenAI\Codex\bin\<hash>\`, and
//!   the hash changes when it updates — it changed *between two measurements* during this
//!   session, from `68de26ad08be95cd` to `cfac6bda2d141e07`, along with the version. Resolving it
//!   once at startup, as [`super::claude`] safely does, would have worked until the first update
//!   and then failed with "not installed" on a machine where it plainly is.
//! - **`--strict-config` is a way to ask.** Codex rejects unknown `-c` keys under it, so the
//!   config surface can be *interrogated* rather than guessed: `base_instructions` is refused,
//!   `instructions` is accepted, and a run with `-c instructions="reply BANANA"` replied BANANA
//!   while still executing a command — which is how we know it carries a persona **without**
//!   replacing the agent's own tool guidance.
//!
//! ## Manual has a real gate
//!
//! `codex exec` has no native permission callback. Its non-interactive MCP client otherwise
//! treats a missing terminal answer as a refusal *before the request reaches the server*.
//! Epoch therefore speaks Codex's bidirectional `app-server` protocol for turns. In Manual, its
//! bounded `workspaceWrite` sandbox combines with the `untrusted` approval policy, which makes
//! the server ask Epoch before an ordinary write. Epoch exposes that exact request through the
//! same one-turn question surface as every other agent. Auto remains silent within that same
//! Project Root; neither mode selects `dangerFullAccess`.

use crate::guard::guard;
use epoch_models::quiet::Quiet;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;

use crate::agent::{
    Agent, AgentError, AgentStatus, Approver, Done, Needs, Progress, Proposal, Task,
};
use crate::agents::SERVER;
use crate::agents::{Credits, PlanUsage};

pub const ID: &str = "codex";
pub const NAME: &str = "Codex";

/// How long to wait for the program to say what version it is.
const PROBE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(6);

/// A bridge instrument must not make opening the World wait on a second CLI for long.
///
/// `app-server` is a local, experimental protocol. Four seconds is generous enough for its
/// initial account request on this machine, while still making a changed or broken version a cold
/// instrument rather than a stalled World.
const USAGE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(4);

/// How often a running turn looks up to see whether it has been stopped.
const HEARTBEAT: std::time::Duration = std::time::Duration::from_millis(250);

/// Codex, as an agent.
pub struct Codex {
    /// Stable id for **this account**. [`ID`] for the sign-in that was already there.
    id: String,
    /// What a person reads. The user's own label, because Codex cannot be asked who it is.
    name: String,
    /// Where this account's sign-in lives. `None` is the program's own default.
    ///
    /// ## Measured
    ///
    /// `CODEX_HOME` is real and it isolates completely: pointed at an empty directory, 0.149.0
    /// answered `Not logged in` while the default still answered `Logged in using ChatGPT`.
    ///
    /// **One trap, measured with it:** Codex refuses to create its PATH helper binaries when its
    /// home is under a temporary directory, and says so. Epoch's accounts therefore live in the
    /// vault, which is a real directory that persists.
    home: Option<PathBuf>,
}

/// The environment variable Codex reads its home from. Named in its own `--help`.
pub const HOME_VAR: &str = "CODEX_HOME";

impl Default for Codex {
    fn default() -> Self {
        Self {
            id: ID.to_owned(),
            name: NAME.to_owned(),
            home: None,
        }
    }
}

impl Codex {
    /// One added account: its own id, its own name, its own sign-in directory.
    pub fn account(id: impl Into<String>, name: impl Into<String>, home: PathBuf) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            home: Some(home),
        }
    }

    /// The program, pointed at **this account's** sign-in.
    ///
    /// Every place that runs Codex goes through here. A spawn site building its own `Command`
    /// would run the default account while the rest of Epoch believed it was running another —
    /// a wrong answer with nothing on screen to suggest it.
    fn start(&self, program: &Path) -> Command {
        let mut command = Command::new(program);
        if let Some(home) = &self.home {
            command.env(HOME_VAR, home);
        }
        command
    }

    /// Where the executable is **right now**.
    ///
    /// Resolved per call rather than once, because the path genuinely moves: Codex installs into a
    /// content-hashed directory and swaps it on update. The newest one wins, chosen by modified
    /// time rather than by name — a hash has no order, so sorting it would be sorting noise.
    ///
    /// `PATH` first regardless: somebody who put `codex` on their `PATH` meant that one. The
    /// desktop app's WindowsApps resource is the exception: it is automatically placed there,
    /// but Windows refuses a direct spawn of it. Skipping it lets the active CLI installation
    /// below answer instead of calling an installed, signed-in Codex "not installed".
    ///
    /// ## `PATH` first, but a **complete** install first of all
    ///
    /// This machine has two, and only one of them works:
    ///
    /// | | `Programs\OpenAI\Codex\bin` (on `PATH`) | `OpenAI\Codex\bin\<hash>` |
    /// |---|---|---|
    /// | `codex.exe` | yes | yes |
    /// | `codex-windows-sandbox-setup.exe` | **no** | yes |
    ///
    /// The desktop app's bundled CLI starts, answers `--version`, reports itself signed in — and
    /// then **every command it runs fails**, because it cannot build its Windows sandbox without
    /// that helper. Not the read that was asked for: any command at all. A character on this
    /// brain had no working shell and could only describe what it would have done, which read as
    /// a weaker agent rather than a broken installation.
    ///
    /// It cost three wrong hypotheses to find, and each was disproved by measuring the *wrong*
    /// executable: every manual check named the version-managed copy explicitly, so the sandbox
    /// was exonerated three times while Epoch went on launching the other one.
    ///
    /// So completeness outranks `PATH`. **Preferred, never required** — a candidate without the
    /// helper is still used when no complete one exists, because a future Codex that stops
    /// shipping it must degrade to today's behaviour rather than to "not installed".
    fn program() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(on_path) = crate::paths::on_path_matching("codex.exe", runnable_from_epoch)
            .or_else(|| crate::paths::on_path_matching("codex", runnable_from_epoch))
        {
            candidates.push(on_path);
        }
        if let Some(managed) = newest_managed() {
            candidates.push(managed);
        }

        choose(candidates)
    }
}

/// The first complete candidate, or the first one at all.
///
/// Its own function so the rule can be tested without a Codex installation — the machine that
/// found this bug had two, and a machine that reproduces it on purpose is a temporary directory.
fn choose(candidates: Vec<PathBuf>) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|exe| complete(exe))
        .or_else(|| candidates.first())
        .cloned()
}

/// The helper Codex needs to build a sandbox for anything it runs.
#[cfg(windows)]
const SANDBOX_HELPER: &str = "codex-windows-sandbox-setup.exe";

/// Whether this executable has what it needs beside it to actually run a command.
///
/// Only asks the Windows question, because it is only a Windows install that ships the helper as
/// a separate file. Everywhere else every candidate is equally complete and the first still wins.
fn complete(exe: &Path) -> bool {
    #[cfg(windows)]
    {
        exe.parent()
            .is_some_and(|beside| beside.join(SANDBOX_HELPER).is_file())
    }
    #[cfg(not(windows))]
    {
        let _ = exe;
        true
    }
}

/// The newest version-managed install, chosen by modified time.
///
/// A hash has no order, so sorting the directory names would be sorting noise.
fn newest_managed() -> Option<PathBuf> {
    let bin = installed_root()?.join("bin");
    std::fs::read_dir(bin)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let exe = entry
                .path()
                .join(if cfg!(windows) { "codex.exe" } else { "codex" });
            let when = entry.metadata().and_then(|m| m.modified()).ok()?;
            exe.is_file().then_some((when, exe))
        })
        .max_by_key(|(when, _)| *when)
        .map(|(_, exe)| exe)
}

/// Whether Windows can start this PATH entry directly as the Codex CLI.
///
/// The desktop package mirrors its bundled executable under `Program Files\\WindowsApps`. It is
/// a real file, so a normal PATH lookup finds it, but this process receives `os error 5` when it
/// tries to start it. The version-managed copy in `LOCALAPPDATA` is the runnable CLI. This check
/// deliberately names only that desktop resource: a real user override on PATH still wins.
fn runnable_from_epoch(candidate: &Path) -> bool {
    if !cfg!(windows) {
        return true;
    }

    let written = candidate
        .to_string_lossy()
        .replace('/', "\\")
        .to_ascii_lowercase();
    !written.contains(r"\program files\windowsapps\openai.codex_")
        || !(written.ends_with(r"\app\resources\codex.exe")
            || written.ends_with(r"\app\resources\codex"))
}

/// Ask the installed Codex CLI for its own current plan allowance.
///
/// The interactive `/status` screen is useful evidence for a person, but feeding keystrokes to a
/// terminal would be a fragile imitation of a contract. Codex 0.147.0 instead exposes the same
/// account rate-limit snapshot over its local app-server. We negotiate the experimental API,
/// perform exactly one read-only request, and kill the short-lived process immediately after its
/// answer. Any protocol mismatch is `None`: a cold instrument is preferable to an invented one.
pub fn plan_usage(home: Option<&Path>) -> Option<PlanUsage> {
    let program = Codex::program()?;
    read_plan_usage(&program, home)
}

fn read_plan_usage(program: &Path, home: Option<&Path>) -> Option<PlanUsage> {
    let mut asking = Command::new(program);
    if let Some(home) = home {
        asking.env(HOME_VAR, home);
    }
    let mut child = asking
        .quiet()
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        // The protocol's diagnostics are not a person-facing failure, and the only useful fact
        // here is whether it returned the specifically requested JSON-RPC response.
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut input = child.stdin.take()?;
    let output = child.stdout.take()?;
    let (lines, arriving) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                return;
            }
        }
    });

    let started = send_app_request(
        &mut input,
        1,
        "initialize",
        Some(serde_json::json!({
            "clientInfo": { "name": "Epoch", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "experimentalApi": true }
        })),
    );

    let answer = if started {
        loop {
            let Ok(line) = arriving.recv_timeout(USAGE_PATIENCE) else {
                break None;
            };
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            match message.get("id").and_then(serde_json::Value::as_u64) {
                Some(1) => {
                    if !send_app_request(&mut input, 2, "account/rateLimits/read", None) {
                        break None;
                    }
                }
                Some(2) => break message.get("result").and_then(read_plan_snapshot),
                _ => {}
            }
        }
    } else {
        None
    };

    // The server is intentionally not a daemon owned by Epoch. Closing stdin and reaping it now
    // means a refresh cannot leave a second background Codex with the account attached.
    drop(input);
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    answer
}

/// Ask the installed Codex CLI which models it offers **today**.
///
/// **Measured, not remembered.** A list compiled into Epoch is wrong the day OpenAI ships one,
/// and it is wrong in the expensive direction: a character is offered six names, the seventh is
/// the one that exists, and nobody using the picker can tell. Codex 0.147.0 answers `model/list`
/// over the same local app-server the plan reading already uses — measured, not read out of a
/// help page, because it appears in neither `--help` nor the documented protocol.
///
/// Per account, like the allowance: what a sign-in may reach is a property of that sign-in.
///
/// `None` means **nobody could be asked**, never *there are none*. The caller falls back to the
/// suggestions in [`models`] — a gate with nothing behind it stays open.
pub fn models_offered(home: Option<&Path>) -> Option<Vec<(String, String)>> {
    let program = Codex::program()?;
    read_offered_models(&program, home)
}

fn read_offered_models(program: &Path, home: Option<&Path>) -> Option<Vec<(String, String)>> {
    let mut asking = Command::new(program);
    if let Some(home) = home {
        asking.env(HOME_VAR, home);
    }
    let mut child = asking
        .quiet()
        .arg("app-server")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut input = child.stdin.take()?;
    let output = child.stdout.take()?;
    let (lines, arriving) = mpsc::channel::<String>();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(output).lines().map_while(Result::ok) {
            if lines.send(line).is_err() {
                return;
            }
        }
    });

    let started = send_app_request(
        &mut input,
        1,
        "initialize",
        Some(serde_json::json!({
            "clientInfo": { "name": "Epoch", "version": env!("CARGO_PKG_VERSION") },
            "capabilities": { "experimentalApi": true }
        })),
    );

    let answer = if started {
        loop {
            let Ok(line) = arriving.recv_timeout(USAGE_PATIENCE) else {
                break None;
            };
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            match message.get("id").and_then(serde_json::Value::as_u64) {
                Some(1) => {
                    if !send_app_request(&mut input, 2, "model/list", Some(serde_json::json!({}))) {
                        break None;
                    }
                }
                Some(2) => break message.get("result").and_then(read_model_list),
                _ => {}
            }
        }
    } else {
        None
    };

    drop(input);
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    answer
}

/// The models in one `model/list` answer, in the order Codex itself lists them.
///
/// **`hidden` is Codex's own word for "not in my picker".** Passing those through would offer a
/// person a model their own CLI declines to show them.
///
/// An answer that parses to nothing is `None` rather than an empty list, because *nothing usable
/// came back* and *this account has no models* are different facts and only the first is one
/// Epoch can support. The empty list would silently replace the suggestions.
fn read_model_list(answer: &serde_json::Value) -> Option<Vec<(String, String)>> {
    let found: Vec<(String, String)> = answer
        .get("data")?
        .as_array()?
        .iter()
        .filter(|model| {
            !model
                .get("hidden")
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        })
        .filter_map(|model| {
            let id = model.get("id").and_then(serde_json::Value::as_str)?;
            let name = model
                .get("displayName")
                .and_then(serde_json::Value::as_str)
                .filter(|it| !it.trim().is_empty())
                .unwrap_or(id);
            let about = model
                .get("description")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .trim();
            Some((
                id.to_owned(),
                if about.is_empty() {
                    name.to_owned()
                } else {
                    format!("{name} — {about}")
                },
            ))
        })
        .collect();
    (!found.is_empty()).then_some(found)
}

/// Where the turn happens, written the way another program can read it.
///
/// Containment canonicalises a Project Root, and on Windows that is an extended-length path —
/// `\\?\C:\Users\…`. Gemini's Node runtime splits it into a root of `\\?\` and a first
/// segment of `C:` and answers `EISDIR: illegal operation on a directory, lstat 'C:'`. Codex
/// tolerates it, and tolerating a thing is not a reason to keep sending it: the same string is
/// handed to somebody else, and a defect that is absent only because of which language the other
/// program happens to be written in is a defect waiting for the next agent.
///
/// **Both halves measured before this was adopted**, because a change to a path that
/// demonstrably works is not one to make on reasoning. The allowance ran out on the first
/// attempt and the request path was put back until it returned; with the plain form, an allowed
/// write lands in the installed window — `codex-si2.txt`, with `HOLA` in it.
///
/// `sandbox_policy`'s `writableRoots` is built from this too, so the sandbox and the directory
/// cannot disagree inside one request.
fn here(task: &Task) -> String {
    crate::library::plainly(&task.directory.display().to_string())
}

/// Write one JSON-RPC request without claiming a request succeeded because it was constructed.
fn send_app_request<W: Write>(
    input: &mut W,
    id: u64,
    method: &str,
    params: Option<serde_json::Value>,
) -> bool {
    let mut request = serde_json::json!({ "id": id, "method": method });
    if let Some(params) = params {
        request["params"] = params;
    }
    if serde_json::to_writer(&mut *input, &request).is_err() {
        return false;
    }
    input.write_all(b"\n").and_then(|_| input.flush()).is_ok()
}

/// Keep only the non-sensitive, bounded fields that drive the bridge instrument.
fn read_plan_snapshot(answer: &serde_json::Value) -> Option<PlanUsage> {
    let primary = answer.get("rateLimits")?.get("primary")?.as_object()?;
    let used = primary.get("usedPercent")?.as_u64()?;
    let used_percent = u8::try_from(used).ok().filter(|percent| *percent <= 100)?;
    Some(PlanUsage {
        remaining_percent: 100 - used_percent,
        used_percent,
        window_minutes: primary
            .get("windowDurationMins")
            .and_then(serde_json::Value::as_u64),
        resets_at: primary.get("resetsAt").and_then(serde_json::Value::as_i64),
        credits: read_credits(answer.get("rateLimits")),
    })
}

/// What Codex says is left once the window is spent.
///
/// **Measured against the real reply**, which sends the balance as a *string* -
/// `"379.1741100000"` - beside `hasCredits` and `unlimited`. A number that arrives as text is
/// exactly the kind of thing a remembered shape gets wrong.
///
/// `hasCredits: false` is `None`, not a zero balance: it means this account does not carry
/// credit, which is a different sentence from having none left.
fn read_credits(limits: Option<&serde_json::Value>) -> Option<Credits> {
    let credits = limits?.get("credits")?;
    if credits
        .get("hasCredits")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return None;
    }
    let unlimited = credits
        .get("unlimited")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let balance = credits
        .get("balance")
        .and_then(|value| {
            value
                .as_f64()
                .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
        })
        .filter(|amount: &f64| amount.is_finite() && *amount >= 0.0);
    match (unlimited, balance) {
        (true, _) => Some(Credits {
            balance: 0.0,
            unlimited: true,
        }),
        // A balance it would not put a number on is not a balance anybody can read.
        (false, Some(balance)) => Some(Credits {
            balance,
            unlimited: false,
        }),
        (false, None) => None,
    }
}

/// Where this platform's Codex install lives.
fn installed_root() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA").map(|base| PathBuf::from(base).join("OpenAI/Codex"))
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex"))
    }
}

/// Epoch's rung, in Codex's words.
///
/// **Measured, and the same five as Claude Code's.** `low`, `medium`, `high`, `xhigh` and `max`
/// were each run against the real service; `extra-high` — the wording its own picker shows — is
/// rejected with `invalid_request_error`, which is why the label a user reads and the value a
/// program takes are kept apart.
///
/// This is also the second vendor to have `xhigh`. Adding that rung to the Kernel was the one
/// decision in ADR-0026 that looked like a vendor leaking into a canonical ladder; two
/// independent programs having it says it is a rung of the concept.
///
/// `Off` maps to nothing: Codex has no level that means "do not deliberate", and turning that
/// into `low` would be Epoch deciding the instruction meant something else.
pub fn effort_of(reasoning: Option<epoch_kernel::Reasoning>) -> Option<&'static str> {
    match reasoning? {
        epoch_kernel::Reasoning::Off => None,
        epoch_kernel::Reasoning::Low => Some("low"),
        epoch_kernel::Reasoning::Medium => Some("medium"),
        epoch_kernel::Reasoning::High => Some("high"),
        epoch_kernel::Reasoning::XHigh => Some("xhigh"),
        epoch_kernel::Reasoning::Max => Some("max"),
    }
}

/// What Codex called its own models when this was written — **the fallback, not the answer.**
///
/// [`models_offered`] asks the installed CLI, and that is what a person normally sees. This is
/// what is shown when nobody could be asked: not installed, not signed in, or a release that no
/// longer speaks `model/list`. Showing nothing there would read as *this agent has no models*,
/// which is the inversion this codebase keeps paying for.
///
/// It goes stale, and that is survivable precisely because it is second: the field also stays
/// typeable, so a model shipped this morning works this morning either way.
pub fn models() -> &'static [(&'static str, &'static str)] {
    &[
        ("gpt-5.6-sol", "Sol — the frontier agentic model"),
        ("gpt-5.6-terra", "Terra — balanced, for everyday work"),
        ("gpt-5.6-luna", "Luna — fast and affordable"),
        ("gpt-5.5", "GPT-5.5 — complex coding and research"),
        ("gpt-5.4", "GPT-5.4 — everyday coding"),
        ("gpt-5.4-mini", "GPT-5.4 mini — small and fast"),
    ]
}

/// The app-server approval policy for an Epoch autonomy mode.
///
/// Manual deliberately uses the interactive `untrusted` policy. `on-request` only pauses for
/// escalations, so it would let an ordinary Project Root write begin without the person's word.
/// `untrusted` pauses that ordinary command or file change and Epoch asks at the exact point.
/// Auto remains silent, but both
/// modes stay confined to the Project Root through [`sandbox_policy`].
fn approval_policy(autonomy: epoch_kernel::Autonomy) -> &'static str {
    match autonomy {
        epoch_kernel::Autonomy::Manual => "untrusted",
        epoch_kernel::Autonomy::AcceptEdits | epoch_kernel::Autonomy::Auto => "never",
    }
}

/// Epoch never grants Codex unrestricted machine access.
///
/// **Writes are confined; the network is not.** Those are two separate decisions and they had
/// been made as one.
///
/// The confinement that matters is `writableRoots`: a Quest works inside its Project Root, and
/// nothing here selects `dangerFullAccess`. That stays.
///
/// The network restriction was removed deliberately, by the user, and the reason is that it was
/// not a boundary. Epoch gives Claude Code **no sandbox at all** — so the same World ran one
/// agent with the whole machine and the other with no `npm install`, no `git fetch`, no `pip`,
/// no test suite that downloads anything. That is not a security policy, it is a policy-shaped
/// asymmetry: it cost one agent most of its usefulness and bought nothing, because the
/// unrestricted one was always a click away.
///
/// A limit only one agent respects is not a limit. If network egress is to be closed, it has to
/// be closed for every brain, and that is a decision about Epoch rather than about Codex.
/// What the user actually sent this turn: their words, and any pictures.
///
/// `UserInput` is a `oneOf` in Codex's own schema — `{type:"text"}` or `{type:"image",url}` —
/// and both go in the one array.
///
/// **The `url` must be a `data:` URI, and that is the measured part.** A filesystem path is
/// accepted without complaint: `turn/start` returns a turn, the run begins, and it fails minutes
/// later *upstream* with `Invalid 'input[4].content[0].image…` — the API's error, not Codex's,
/// arriving long after the thing that caused it. Building on the path form would have produced a
/// feature that looks wired, costs a turn every time, and blames somebody else.
///
/// Pictures first, words after. A question about an image reads better when the image is already
/// there, and it matches how the same message is drawn in the Chronicle.
fn turn_input(task: &Task) -> serde_json::Value {
    use base64::Engine as _;
    let mut items: Vec<serde_json::Value> = task
        .images
        .iter()
        .map(|image| {
            let encoded = base64::engine::general_purpose::STANDARD.encode(&image.bytes);
            serde_json::json!({
                "type": "image",
                "url": format!("data:{};base64,{encoded}", image.mime),
            })
        })
        .collect();
    items.push(serde_json::json!({ "type": "text", "text": task.intent.trim() }));
    serde_json::Value::Array(items)
}

fn sandbox_policy(task: &Task) -> serde_json::Value {
    // The same plain form as `here`, so the sandbox and the working directory are one spelling.
    let mut writable = vec![std::path::PathBuf::from(here(task))];
    writable.extend(package_caches());
    serde_json::json!({
        "type": "workspaceWrite",
        "writableRoots": writable,
        "networkAccess": true,
    })
}

/// The package managers' caches, when they exist on this machine.
///
/// Watched, not theorised: `npm view react version` failed inside the sandbox, and the agent
/// recovered by pointing `npm_config_cache` at a folder inside the Project Root. It worked, and
/// the cost is a stray `.tmp-npm-cache` in somebody's repository plus a cold download on every
/// single command — so every install, every build, forever.
///
/// **Caches only, and the distinction is the whole point.** `~/.cargo` is not granted: it
/// contains `bin`, which is on `PATH`, and writing there would let a turn leave an executable
/// that runs later outside any sandbox. `registry` and `git` are downloads; `bin` is a loaded
/// gun. Two directories apart, and nothing about "let cargo cache its crates" implies the second.
///
/// Only paths that already exist are named. A writable root for a directory nobody has created
/// is a grant to create it, which is a different and larger permission than the one intended.
fn package_caches() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut consider = |path: Option<PathBuf>| {
        if let Some(path) = path.filter(|p| p.is_dir()) {
            found.push(path);
        }
    };

    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);

    // Measured on Windows: `npm config get cache` answers `%LOCALAPPDATA%\npm-cache`, and pip
    // keeps `%LOCALAPPDATA%\pip\Cache`. The Unix spellings are the documented defaults.
    consider(local.as_ref().map(|l| l.join("npm-cache")));
    consider(home.as_ref().map(|h| h.join(".npm")));
    consider(local.as_ref().map(|l| l.join("pip/Cache")));
    consider(home.as_ref().map(|h| h.join(".cache/pip")));

    let cargo = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|h| h.join(".cargo")));
    consider(cargo.as_ref().map(|c| c.join("registry")));
    consider(cargo.as_ref().map(|c| c.join("git")));

    found
}

impl Agent for Codex {
    fn id(&self) -> &str {
        &self.id
    }

    fn probe(&self) -> AgentStatus {
        let Some(program) = Self::program() else {
            return AgentStatus::missing(
                &self.id,
                &self.name,
                "not on this machine's PATH, and no install found under %LOCALAPPDATA%\\OpenAI\\Codex",
            );
        };

        let asked = self
            .start(&program)
            .quiet()
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let Ok(mut child) = asked else {
            let mut missing =
                AgentStatus::missing(&self.id, &self.name, "it is here but would not start");
            missing.looked_in = Some(program.display().to_string());
            return missing;
        };

        let Some(said) = super::claude::wait_for(&mut child, PROBE_PATIENCE) else {
            let _ = child.kill();
            let mut stuck = AgentStatus::missing(
                &self.id,
                &self.name,
                "it did not answer `--version` in time",
            );
            stuck.looked_in = Some(program.display().to_string());
            return stuck;
        };

        let (signed_in, account) = whoami(&program, self.home.as_deref());

        AgentStatus {
            kind: ID.to_owned(),
            id: self.id.clone(),
            name: self.name.clone(),
            installed: true,
            version: Some(said.trim().to_owned()).filter(|v| !v.is_empty()),
            looked_in: Some(program.display().to_string()),
            note: None,
            signed_in,
            account,
            // Read from its own sign-in status rather than from a chosen method: this agent can
            // be asked directly, so there is nothing left over for `method` to carry.
            method: None,
        }
    }

    fn work(
        &self,
        task: &Task,
        stopped: &dyn Fn() -> bool,
        approver: &dyn Approver,
        sink: &mut dyn FnMut(Progress),
    ) -> Result<Done, AgentError> {
        self.work_interactively(task, stopped, approver, sink)
    }
}

impl Codex {
    /// Work through Codex's bidirectional app-server protocol.
    ///
    /// Unlike `codex exec`, this transport receives the server's approval request before the
    /// command or file change runs. That is the only place Manual can truthfully show
    /// `MAGE NEEDS YOUR WORD`: the agent is paused, no write has happened, and the exact command
    /// or write root is known. Item approvals answer `accept` or `decline`; Codex's broader
    /// permission request receives only the requested profile, scoped to this turn. Epoch never
    /// grants a lasting Codex permission and never enables `dangerFullAccess`.
    fn work_interactively(
        &self,
        task: &Task,
        stopped: &dyn Fn() -> bool,
        approver: &dyn Approver,
        sink: &mut dyn FnMut(Progress),
    ) -> Result<Done, AgentError> {
        if !task.directory.is_dir() {
            return Err(AgentError::Nowhere);
        }
        let program = Self::program().ok_or_else(|| AgentError::Missing(NAME.into()))?;
        let mut command = self.start(&program);
        command.quiet();
        command
            .arg("app-server")
            .args(app_server_arguments(task))
            .current_dir(&task.directory);
        // The token reaches the child through its environment rather than its command line.
        //
        // Codex names an **environment variable** to read a bearer token from, which is a better
        // door than the one Claude Code offers: a command line is visible to anything that can
        // list processes, and this is not. Set only alongside the config that names it.
        if let Some(door) = door_for(task) {
            command.env(TOKEN_VARIABLE, &door.token);
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().map_err(|why| AgentError::Failed {
            agent: NAME.into(),
            why: why.to_string(),
        })?;
        let mut input = child.stdin.take().ok_or_else(|| AgentError::Failed {
            agent: NAME.into(),
            why: "it could not accept Epoch's replies".into(),
        })?;
        let output = child.stdout.take().ok_or_else(|| AgentError::Failed {
            agent: NAME.into(),
            why: "it produced no protocol output".into(),
        })?;
        let complaints = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let watching = child.stderr.take().map(|pipe| {
            let heard = std::sync::Arc::clone(&complaints);
            std::thread::spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    guard(&heard).push(line);
                }
            })
        });
        let (lines, arriving) = mpsc::channel::<String>();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                if lines.send(line).is_err() {
                    return;
                }
            }
        });

        if !send_app_request(
            &mut input,
            1,
            "initialize",
            Some(serde_json::json!({
                "clientInfo": { "name": "Epoch", "version": env!("CARGO_PKG_VERSION") },
                "capabilities": { "experimentalApi": true }
            })),
        ) {
            return Err(AgentError::Failed {
                agent: NAME.into(),
                why: "it would not start its interactive protocol".into(),
            });
        }

        let mut done = Done::default();
        let mut initialized = false;
        let mut turn_started = false;
        // Which request id carries the conversation this turn will use.
        //
        // Two, unless a resume found nothing and a fresh start was sent as four.
        let mut expecting: u64 = 2;
        // Whether the request in flight is a resume, so a refusal knows where to go.
        let mut resuming = false;
        // One retry, never two: a start that fails is a real failure and must say so.
        let mut retried = false;
        let mut streamed = String::new();
        let mut failure = None;
        'turn: loop {
            match arriving.recv_timeout(HEARTBEAT) {
                Ok(line) => {
                    let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                        continue;
                    };
                    if let Some(method) = message.get("method").and_then(serde_json::Value::as_str)
                    {
                        if message.get("id").is_some() {
                            reply_to_server_request(&mut input, &message, task, approver);
                            continue;
                        }
                        if let Heard::Ended(why) = read_app_notification(
                            method,
                            &message,
                            &mut done,
                            &mut streamed,
                            sink,
                            task,
                        ) {
                            failure = why;
                            break 'turn;
                        }
                        continue;
                    }

                    let id = message.get("id").and_then(serde_json::Value::as_u64);
                    if id == Some(1) && !initialized {
                        initialized = true;
                        let _ = send_app_notification(&mut input, "initialized", None);
                        resuming = task.thread.is_some();
                        let (method, params) = if let Some(thread) = &task.thread {
                            ("thread/resume", serde_json::json!({ "threadId": thread }))
                        } else {
                            ("thread/start", fresh_thread(task))
                        };
                        if !send_app_request(&mut input, 2, method, Some(params)) {
                            failure = Some(
                                "it would not create or resume this Codex conversation".into(),
                            );
                            break;
                        }
                    } else if id == Some(expecting) && !turn_started {
                        let Some(thread) = thread_id_from(&message) else {
                            // **A resume that cannot find its conversation starts a new one.**
                            //
                            // Measured on a real machine: a Chronicle that had spoken to Codex
                            // before answered
                            //
                            //   no rollout found for thread id 36a19af5-…
                            //
                            // and every later turn answered the same, for ever. Codex keeps its
                            // own rollout of a thread and can lose it — a pruned history, a
                            // different `CODEX_HOME`, a reinstall — and the id Epoch stored then
                            // names nothing. There was no way out of that from the window.
                            //
                            // The thread id is **continuity, not the record**. Epoch owns the
                            // Chronicle; what a lost rollout costs is the agent's own memory of
                            // the exchange, not the exchange. So a failed resume falls back to a
                            // fresh conversation rather than becoming a dead end.
                            //
                            // **And it is visible.** `Progress::Window` carries the session id
                            // precisely so *is this a new conversation?* is answerable from the
                            // screen; a new thread reports a new id, and the user sees it. Epoch
                            // does not narrate it in the character's voice, which would be
                            // putting its own words in somebody's mouth.
                            //
                            // A failure on a **start** has nowhere to fall back to, and says
                            // what the program said.
                            if resuming && !retried {
                                retried = true;
                                resuming = false;
                                expecting = 4;
                                if !send_app_request(
                                    &mut input,
                                    4,
                                    "thread/start",
                                    Some(fresh_thread(task)),
                                ) {
                                    failure =
                                        Some("it would not start a Codex conversation".into());
                                    break;
                                }
                                continue;
                            }
                            // **A reply with no thread is usually a reply with a reason.**
                            //
                            // JSON-RPC answers either a `result` or an `error`, and this branch
                            // only ever read the first. Measured on a fresh install: the turn
                            // said *it did not return a Codex conversation id* with nothing on
                            // stderr either, so the one place the reason could be was the half
                            // of the message nobody looked at.
                            failure = Some(match super::refusal_in(&message) {
                                Some(said) => {
                                    format!("it would not start a conversation: {said}")
                                }
                                None => "it did not return a Codex conversation id".into(),
                            });
                            break;
                        };
                        done.thread = Some(thread.to_owned());
                        turn_started = true;
                        let params = serde_json::json!({
                            "threadId": thread,
                            "input": turn_input(task),
                            "cwd": here(task),
                            "model": model_value(task),
                            "effort": effort_of(task.reasoning),
                            "approvalPolicy": approval_policy(task.autonomy),
                            // Keep Epoch in the approval loop even if a future app-server
                            // default changes.  Manual must never silently fall back to an
                            // automatic reviewer.
                            "approvalsReviewer": "user",
                            "sandboxPolicy": sandbox_policy(task),
                        });
                        if !send_app_request(&mut input, 3, "turn/start", Some(params)) {
                            failure = Some("it would not start this Codex turn".into());
                            break;
                        }
                    } else if let Some(error) = message
                        .pointer("/error/message")
                        .and_then(serde_json::Value::as_str)
                    {
                        failure = Some(error.to_owned());
                        break;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            if stopped() {
                let _ = child.kill();
                let _ = child.wait();
                drop(arriving);
                let _ = reader.join();
                return Err(AgentError::Stopped {
                    agent: NAME.into(),
                    why: "you asked it to stop".into(),
                });
            }
        }

        drop(input);
        let _ = child.kill();
        let ended = child.wait().map_err(|why| AgentError::Failed {
            agent: NAME.into(),
            why: why.to_string(),
        })?;
        drop(arriving);
        let _ = reader.join();
        if let Some(watching) = watching {
            let _ = watching.join();
        }
        if let Some(why) = failure {
            // **And what Codex said, if it said anything.**
            //
            // These lines were collected and then used on one path only — the one where the
            // process exits badly. A protocol step that goes wrong took the other path and
            // reported Epoch's sentence alone.
            //
            // Measured on a fresh install: a Codex turn answered *it did not return a Codex
            // conversation id* and nothing else, while the same handshake driven by hand against
            // the same binary answered `result.thread.id` three times out of three. The sentence
            // was true and useless — it says what Epoch expected and not what happened, and the
            // one thing that could have said what happened was already in memory.
            //
            // The far program's own words are evidence. That is the same rule `blame` follows
            // for ComfyUI, and the reason the transport's words are *not* kept: these are
            // Codex's.
            return Err(AgentError::Failed {
                agent: NAME.into(),
                why: match super::said(&complaints) {
                    Some(said) => format!("{why} — Codex said: {said}"),
                    None => why,
                },
            });
        }
        if !ended.success() && done.text.trim().is_empty() {
            return Err(AgentError::Failed {
                agent: NAME.into(),
                why: match super::said(&complaints) {
                    Some(said) => format!("it exited with {ended}: {said}"),
                    None => format!("it exited with {ended} and did not complete the turn"),
                },
            });
        }
        Ok(done)
    }
}

/// The parameters that open a brand-new Codex conversation.
///
/// One function because it is sent from two places now — the first attempt, and the retry after
/// a resume finds nothing. Two copies of this would drift the day one of them gains a field.
fn fresh_thread(task: &Task) -> serde_json::Value {
    serde_json::json!({
        "cwd": here(task),
        "model": model_value(task),
    })
}

/// The thread start shape has changed once across Codex releases. Keep the parser tolerant while
/// still requiring an actual opaque id before starting a turn.
fn thread_id_from(message: &serde_json::Value) -> Option<&str> {
    message
        .pointer("/result/thread/id")
        .or_else(|| message.pointer("/result/id"))
        .and_then(serde_json::Value::as_str)
}

fn model_value(task: &Task) -> serde_json::Value {
    let model = task.model.trim();
    if model.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::String(model.to_owned())
    }
}

/// The environment variable Codex is told to read the door's token from.
const TOKEN_VARIABLE: &str = "EPOCH_DOOR_TOKEN";

/// The door this turn should be given, or `None`.
///
/// **Auto only, and the restriction is the original finding rather than caution.** Epoch's door
/// was kept off this invocation entirely because the app-server is already the permission
/// transport: in Manual it asks Epoch before a command or file change runs, and a second MCP
/// route made Codex reach the generic Epoch tools first — whose one-shot approval the app-server
/// cannot answer, so it was cancelled before the visible native question existed.
///
/// Every word of that is about **approval**, and in Auto there is none to answer: an MCP tool
/// does not ask in Auto, which is the same reason [`super::claude`] names its permission tool
/// only outside Auto. So Auto gets the door and Manual keeps the native gate, unchanged, until
/// the approval bridge can answer an app-server turn.
///
/// The cost of leaving it off was not theoretical. A character with a Codex brain was shown
/// `Spotify · 15` ticked, was told nothing about any of it, had no route to reach it, and
/// answered *"ya lo disparé"* about a login it had never called — the Chronicle recorded zero
/// evidence for the turn while its Claude-brained colleague's identical request recorded two.
fn door_for(task: &Task) -> Option<&crate::agent::Door> {
    task.door.as_ref()
}

/// Configures Codex's invocation-scoped behaviour: who it is, and how to reach Epoch.
///
/// ## The shape was measured, not remembered
///
/// `--strict-config` answers questions about the config surface, and it was asked three of them
/// against `codex-cli 0.147.0-alpha.6.6`. `mcp_servers.<name>.bogus_key` is refused with
/// *unknown configuration field*; `url` and `bearer_token_env_var` are accepted. The refusal is
/// what makes the acceptance mean anything — without a control, a run that starts proves only
/// that nothing crashed.
///
/// The token is named rather than written: Codex reads it from the environment, so it never
/// appears on a command line that any process listing can read. Nothing is written to
/// `~/.codex/config.toml` — a token in a file outlives the door it describes, which is the same
/// reason [`super::claude`] passes its configuration inline.
fn app_server_arguments(task: &Task) -> Vec<String> {
    let mut arguments = vec![
        "-c".into(),
        format!("instructions={}", toml_string(&persona(task))),
    ];
    if let Some(door) = door_for(task) {
        arguments.push("-c".into());
        arguments.push(format!(
            "mcp_servers.{SERVER}.url={}",
            toml_string(&door.url)
        ));
        arguments.push("-c".into());
        arguments.push(format!(
            "mcp_servers.{SERVER}.bearer_token_env_var={}",
            toml_string(TOKEN_VARIABLE)
        ));
    }
    arguments
}

/// JSON-RPC notifications have no id and never receive a response.
fn send_app_notification<W: Write>(
    input: &mut W,
    method: &str,
    params: Option<serde_json::Value>,
) -> bool {
    let mut notification = serde_json::json!({ "method": method });
    if let Some(params) = params {
        notification["params"] = params;
    }
    if serde_json::to_writer(&mut *input, &notification).is_err() {
        return false;
    }
    input.write_all(b"\n").and_then(|_| input.flush()).is_ok()
}

/// Respond to the server's own request, preserving its opaque JSON-RPC id.
fn send_app_response<W: Write>(
    input: &mut W,
    id: &serde_json::Value,
    result: serde_json::Value,
) -> bool {
    let response = serde_json::json!({ "id": id, "result": result });
    if serde_json::to_writer(&mut *input, &response).is_err() {
        return false;
    }
    input.write_all(b"\n").and_then(|_| input.flush()).is_ok()
}

/// Give the server exactly one per-request decision. Manual opens Epoch's modal synchronously;
/// Auto only confirms a request that should already be within its bounded workspace policy.
fn reply_to_server_request<W: Write>(
    input: &mut W,
    request: &serde_json::Value,
    task: &Task,
    approver: &dyn Approver,
) {
    let Some(id) = request.get("id") else {
        return;
    };
    let method = request
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let params = request.get("params").unwrap_or(&serde_json::Value::Null);
    // **This match already knew which of the four it was.** Every arm went on to say
    // `Needs::Write`, so a person answering `item/commandExecution/requestApproval` read *write
    // access* above a command. The arms are unchanged; what they are labelled with is not.
    //
    // No previews on this side yet, and that is a measurement rather than an omission: Gemini's
    // `toolCall` was recorded off the wire and carries a diff, and Codex's approval params have
    // not been recorded here. A preview assembled from field names somebody remembered is the
    // thing this whole change exists to stop.
    let (needs, what) = match method {
        "item/commandExecution/requestApproval" => (
            Needs::Execute,
            params
                .get("command")
                .and_then(serde_json::Value::as_str)
                .or_else(|| params.get("reason").and_then(serde_json::Value::as_str))
                .unwrap_or("run a command"),
        ),
        "item/fileChange/requestApproval" => (
            Needs::Write,
            params
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .or_else(|| params.get("grantRoot").and_then(serde_json::Value::as_str))
                .unwrap_or("change files in the Project Root"),
        ),
        // The current Codex app-server asks this broader, native question before it emits a
        // concrete command or file-change item.  It is still a *write* request, and this is the
        // first point at which Epoch can truthfully put the agent on hold before anything runs.
        "item/permissions/requestApproval" => (
            Needs::Write,
            params
                .get("reason")
                .and_then(serde_json::Value::as_str)
                .or_else(|| params.get("cwd").and_then(serde_json::Value::as_str))
                .unwrap_or("change files in the Project Root"),
        ),
        // **An outside MCP tool, asked about as an elicitation.**
        //
        // Measured, and it overturns a belief this file carried for a while. Codex asks its
        // client before calling a tool belonging to an MCP server it does not own, and it asks
        // through `mcpServer/elicitation/request` rather than any of the `requestApproval`
        // methods above:
        //
        //     {"method":"mcpServer/elicitation/request","id":0,"params":{
        //        "serverName":"probe","mode":"form",
        //        "_meta":{"codex_approval_kind":"mcp_tool_call","persist":["session","always"]},
        //        "message":"Allow the probe MCP server to run tool \"probe_watchword\"?"}}
        //
        // It fell to the catch-all below and was declined — so **Epoch refused every one of its
        // own tools** and the turn simply stopped, with no message naming a cause. That is what
        // "Manual cannot have the door" was actually made of, and it was never about Manual:
        // `tests/codex_untrusted_mcp_probe.rs` runs both approval policies against an outside
        // server, and with this answered, both call the tool.
        //
        // Epoch is *invited* to this decision rather than making it (`CLAUDE.md`, 2026-08-08):
        // the sentence shown is Codex's own, unedited, because Epoch has no descriptor for
        // somebody else's tool and must not compose one.
        "mcpServer/elicitation/request" => {
            let what = params
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("use a tool from a connected server");
            let allowed = match task.autonomy {
                epoch_kernel::Autonomy::Manual => {
                    approver.allow_for_this_turn(&Proposal::plain(Needs::Outside, what))
                }
                epoch_kernel::Autonomy::AcceptEdits | epoch_kernel::Autonomy::Auto => true,
            };
            // Its own response contract: `action`, not `decision`. Answering the wrong shape is
            // indistinguishable from not answering, which is the failure this arm replaces.
            let _ = send_app_response(
                input,
                id,
                serde_json::json!({
                    "action": if allowed { "accept" } else { "decline" },
                    "content": {},
                }),
            );
            return;
        }
        // Handover keeps using its existing MCP path. The interactive app server can still ask
        // about it, so do not silently accept an unrecognised external request in Manual.
        _ => {
            let _ = send_app_response(input, id, serde_json::json!({ "decision": "decline" }));
            return;
        }
    };
    let allowed = match task.autonomy {
        epoch_kernel::Autonomy::Manual => {
            approver.allow_for_this_turn(&Proposal::plain(needs, what))
        }
        epoch_kernel::Autonomy::AcceptEdits | epoch_kernel::Autonomy::Auto => true,
    };
    // `permissions/requestApproval` has a different response contract from the item-level
    // approvals above. Grant only the requested filesystem profile, never network access, and
    // keep it scoped to this turn. `strictAutoReview` makes a Manual grant a pause before every
    // following command instead of silently turning it into a broad session allowance.
    let response = if method == "item/permissions/requestApproval" {
        serde_json::json!({
            "permissions": if allowed {
                serde_json::json!({
                    "fileSystem": params.pointer("/permissions/fileSystem").cloned(),
                })
            } else {
                serde_json::json!({})
            },
            "scope": "turn",
            "strictAutoReview": allowed && matches!(task.autonomy, epoch_kernel::Autonomy::Manual),
        })
    } else {
        serde_json::json!({ "decision": if allowed { "accept" } else { "decline" } })
    };
    let _ = send_app_response(input, id, response);
}

/// What one server notification meant for the turn.
#[derive(Debug, PartialEq, Eq)]
enum Heard {
    /// Read, and the turn continues.
    Nothing,
    /// The turn is over. `Some` when it ended badly, carrying what the server said.
    Ended(Option<String>),
}

/// Translate one app-server notification.
///
/// **Its own function so it can be read and tested.** It was four arms inside the receive loop,
/// which meant the only way to assert any of them was to run a real Codex process — and the
/// consequence was concrete: every evidence and usage test in this file drove `read_event`, the
/// retired `codex exec --json` parser, while this path had none. Behaviour is unchanged; what
/// changed is that there is now somewhere to point a test.
fn read_app_notification(
    method: &str,
    message: &serde_json::Value,
    done: &mut Done,
    streamed: &mut String,
    sink: &mut dyn FnMut(Progress),
    task: &Task,
) -> Heard {
    match method {
        "item/agentMessage/delta" => {
            if let Some(delta) = message
                .pointer("/params/delta")
                .and_then(serde_json::Value::as_str)
            {
                streamed.push_str(delta);
                sink(Progress::Said(delta.to_owned()));
            }
        }
        "item/completed" => {
            if let Some(item) = message.pointer("/params/item") {
                read_app_item(item, done, streamed, sink, task);
            }
        }
        "thread/tokenUsage/updated" => {
            if let Some(used) = message
                .pointer("/params/tokenUsage/total/totalTokens")
                .and_then(serde_json::Value::as_u64)
            {
                // **Budget zero, deliberately.** Codex reports what a turn used and never what it
                // is allowed; a context gauge with an invented denominator is the cold-instrument
                // rule broken at its most tempting. The surface shows a count with an unknown
                // budget, which is what `ContextGauge` already keeps separate.
                sink(Progress::Window {
                    used,
                    budget: 0,
                    session: done.thread.clone(),
                });
            }
        }
        "turn/completed" => {
            let turn = message.pointer("/params/turn");
            let failed = turn
                .and_then(|value| value.get("status").and_then(serde_json::Value::as_str))
                == Some("failed");
            return Heard::Ended(failed.then(|| {
                turn.and_then(|value| value.pointer("/error/message"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_owned)
                    // It failed and would not say why. Better than reporting a silent success.
                    .unwrap_or_else(|| "the Codex turn failed".into())
            }));
        }
        // Codex updates itself often. An unrecognised notification is skipped, never fatal.
        _ => {}
    }
    Heard::Nothing
}

/// Translate completed native app-server items into the evidence and terminal facts Epoch owns.
fn read_app_item(
    item: &serde_json::Value,
    done: &mut Done,
    streamed: &mut String,
    sink: &mut dyn FnMut(Progress),
    task: &Task,
) {
    match item.get("type").and_then(serde_json::Value::as_str) {
        Some("agentMessage") => {
            let Some(text) = item.get("text").and_then(serde_json::Value::as_str) else {
                return;
            };
            // Delta notifications have already reached the Chronicle. Only send a missing suffix
            // so a completed item never duplicates an answer in the conversation.
            if let Some(rest) = text.strip_prefix(streamed.as_str()) {
                if !rest.is_empty() {
                    sink(Progress::Said(rest.to_owned()));
                }
            } else if streamed != text {
                sink(Progress::Said(text.to_owned()));
            }
            *streamed = text.to_owned();
            done.text = text.to_owned();
        }
        Some("commandExecution") => {
            let command = item
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("a command");
            let ok = item.get("status").and_then(serde_json::Value::as_str) == Some("completed");
            let detail = first_line(command);
            if ok {
                done.evidence.push(crate::capability::Made {
                    reference: detail.clone(),
                    summary: format!("ran {detail}"),
                });
            }
            sink(Progress::Ran {
                tool: "shell".into(),
                detail,
                ok,
            });
        }
        // **Codex draws with its own tool, and Epoch's job is to show what it made.**
        //
        // Measured 2026-08-24 against `codex.exe app-server`, asked for a pixel-art frog knight.
        // The item that arrived, 36 seconds after the request and 54 seconds into the turn:
        //
        // ```json
        // { "type": "imageGeneration", "id": "exec-…", "status": "completed",
        //   "revisedPrompt": "Use case: stylized-concept…", "result": "iVBORw0KGgo…" }
        // ```
        //
        // Three things that could only be learned by running it: the type is `imageGeneration`
        // and not the `image_generation` its own binary is full of; the picture is **base64 in
        // the notification** rather than a path; and the working directory was left **empty** —
        // so a turn Epoch does not decode produced nothing, and until now `_ => {}` dropped it.
        //
        // Epoch neither authorised nor performed this (`CLAUDE.md`, 2026-08-07 — Epoch adds tools,
        // it does not take them away). It is witnessing, and the same rule applies as everywhere
        // else here: the evidence is the file that now exists, never the sentence about it.
        Some("imageGeneration") => {
            let ok = item.get("status").and_then(serde_json::Value::as_str) == Some("completed");
            let drawn = item
                .get("result")
                .and_then(serde_json::Value::as_str)
                .filter(|encoded| !encoded.is_empty());
            let kept = match (ok, drawn) {
                (true, Some(encoded)) => keep_picture(encoded, &task.pictures),
                _ => None,
            };
            match kept {
                Some(file) => {
                    done.evidence.push(crate::capability::Made {
                        reference: file.clone(),
                        summary: format!("drew {file}"),
                    });
                    sink(Progress::Ran {
                        tool: "drew".into(),
                        detail: file,
                        ok: true,
                    });
                }
                // It tried and Epoch has nothing to show. Said plainly rather than silently: a
                // picture reported as made and not kept is the one thing worse than no picture.
                None => sink(Progress::Ran {
                    tool: "drew".into(),
                    detail: "a picture Epoch could not keep".into(),
                    ok: false,
                }),
            }
        }
        Some("fileChange") => {
            let ok = item.get("status").and_then(serde_json::Value::as_str) == Some("completed");
            for change in item
                .get("changes")
                .and_then(serde_json::Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                let path = change
                    .get("path")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("a file");
                let kind = change
                    .pointer("/kind/type")
                    .or_else(|| change.get("kind"))
                    .and_then(serde_json::Value::as_str);
                let tool = match kind {
                    Some("add") => "created",
                    Some("delete") => "deleted",
                    Some("update") => "edited",
                    _ => "changed",
                };
                let detail = within(&task.directory, path);
                if ok {
                    done.evidence.push(crate::capability::Made {
                        reference: detail.clone(),
                        summary: format!("{tool} {detail}"),
                    });
                }
                sink(Progress::Ran {
                    tool: tool.into(),
                    detail,
                    ok,
                });
            }
        }
        _ => {}
    }
}

/// Write a base64 picture into this World's pictures, and answer with the name it now has.
///
/// **Named from the bytes, never from anything the agent said** — ADR-0024's rule, one subsystem
/// over. `revisedPrompt` is text an agent wrote and would make a fine-looking filename and a
/// traversal one turn later; the format is read out of the file itself, the same way an imported
/// picture's is.
///
/// Best effort in every direction. A picture that cannot be written must never fail a turn that
/// otherwise worked — the caller reports the miss rather than claiming a success.
fn keep_picture(encoded: &str, into: &Path) -> Option<String> {
    use base64::Engine as _;
    if into.as_os_str().is_empty() {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    let format = crate::import::ImageFormat::sniff(&bytes)?;
    std::fs::create_dir_all(into).ok()?;
    let name = format!(
        "codex-{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_millis(),
        format.extension()
    );
    std::fs::write(into.join(&name), &bytes).ok()?;
    Some(name)
}

/// Who is signed in, measured.
///
/// `codex login status` prints a sentence rather than JSON — *"Logged in using ChatGPT"* — and it
/// prints it to **stderr**, not stdout. Measured, because the first version of this read stdout,
/// found it empty, and reported a signed-in machine as *could not ask*: a plausible-looking
/// `None` that was simply pointed at the wrong pipe.
///
/// An answer we cannot parse stays `None`, never *signed out*: telling somebody to sign in when
/// they already are sends them to fix the wrong thing.
fn whoami(program: &Path, home: Option<&Path>) -> (Option<bool>, Option<String>) {
    let mut asking = Command::new(program);
    if let Some(home) = home {
        asking.env(HOME_VAR, home);
    }
    let asked = asking
        .quiet()
        .arg("login")
        .arg("status")
        .stdin(Stdio::null())
        // Merged, because the answer arrives on stderr and a reader watching only stdout sees an
        // empty string — which is indistinguishable from a program that said nothing.
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let Ok(mut child) = asked else {
        return (None, None);
    };
    // Both pipes: whichever one it used.
    let out = super::claude::wait_for(&mut child, PROBE_PATIENCE).unwrap_or_default();
    let err = child
        .stderr
        .take()
        .map(|mut pipe| {
            use std::io::Read;
            let mut said = String::new();
            let _ = pipe.read_to_string(&mut said);
            said
        })
        .unwrap_or_default();

    read_login(if out.trim().is_empty() { &err } else { &out })
}

/// What `login status` said, as two facts.
fn read_login(said: &str) -> (Option<bool>, Option<String>) {
    let line = said.trim();
    if line.is_empty() {
        return (None, None);
    }
    let lower = line.to_ascii_lowercase();
    if lower.contains("logged in") && !lower.contains("not logged in") {
        (Some(true), Some(line.to_owned()))
    } else if lower.contains("not logged in") || lower.contains("no credentials") {
        (Some(false), None)
    } else {
        // Something else entirely. Unasked beats a guess.
        (None, None)
    }
}

/// The first line of a command, for a Terminal that shows one.
fn first_line(command: &str) -> String {
    command.lines().next().unwrap_or(command).trim().to_owned()
}

/// Who is working, as instructions.
///
/// ## Why a sentence about the approval gate is in here
///
/// Manual runs in the bounded Project Root, but Codex's interactive `untrusted` policy pauses a
/// native command or file change before it reaches disk. Telling the model the workspace is
/// read-only made it decline in prose before there was an action for Epoch to approve. The native
/// protocol now gives us the truthful order: request, Epoch's visible answer, then the action.
fn persona(task: &Task) -> String {
    let mut written = format!(
        "You are {}, working inside Epoch as one of a crew.\n\n{}",
        task.character,
        task.persona.trim()
    );

    // Who the crew actually are, in the same words a model is given.
    if !task.crew.trim().is_empty() {
        written.push_str("\n\n");
        written.push_str(task.crew.trim());
    }

    // How this crew does these jobs, in those same words. After who they are and who they are
    // with — a method is not an identity, and reading it first makes it one.
    if !task.skills.trim().is_empty() {
        written.push_str("\n\n");
        written.push_str(task.skills.trim());
    }

    // Where the notes are, and that Epoch's tools are what reach them.
    if !task.library.trim().is_empty() {
        written.push_str(
            "

",
        );
        written.push_str(task.library.trim());
    }

    // What is connected here, and where the user fixes one that is not. The paragraph that
    // exists because a character with *this* brain invented a settings screen.
    if !task.connections.trim().is_empty() {
        written.push_str(
            "

",
        );
        written.push_str(task.connections.trim());
    }

    if task.autonomy == epoch_kernel::Autonomy::Manual {
        written.push_str(
            "\n\nThis is a Manual turn. Read normally. When a command or file change is needed, \
             use your built-in native Codex tool directly: Codex will pause and Epoch will ask the user before it runs. \
             Use your own tools for files, search and commands rather than an Epoch tool — \
             yours are better and they are already gated. \
             A handover to another crew member is the separate HAND IT OVER flow in Epoch. \
             Do not claim a change succeeded until the approved tool returns. Do not ask for or \
             use access outside the Project Root.",
        );
        // Only when it is true, which is now only when no door was opened.
        //
        // This sentence was unconditional, written when Manual genuinely had no door — and it
        // outlived that. With the door open, the tools were right there and the character
        // answered *"I can't access Epoch's `epoch_mark` tool this turn"*: obeying Epoch,
        // correctly, about a world that had changed underneath the instruction.
        //
        // The same failure as a stale gauge, one layer in. A character reads what it is told
        // and cannot check it, so an instruction that stops being true is worse than a missing
        // one — it produces a confident refusal instead of an attempt.
        match door_for(task) {
            // The wording is Epoch's own (`serve::INSTRUCTIONS`), because there is one answer to
            // what Epoch adds and writing it twice differently is how the two begin to disagree.
            Some(_) => written.push_str(
                " Epoch's own tools are a different thing and they are yours to use: the crew \
                 and what they know, the Quest and its history, this World's knowledge, and any \
                 servers its owner has connected. Calling one **pauses** while Epoch asks the \
                 user — that is not a failure, wait for the result. If one is refused, say so \
                 plainly and do not work around it.",
            ),
            // And when there is none, saying so is what stops an invented answer.
            None => written.push_str(
                " Epoch's own tools — the crew's connected servers among them — are **not \
                 reachable this turn**; if one is needed, say which and that this turn cannot \
                 reach it, and never describe having used it.",
            ),
        }
    }

    written
}

/// A path as somebody working in this project would say it.
///
/// Codex reports absolute paths, and an absolute Windows path is most of a Terminal line before
/// it says anything. Inside the project it is written relative to the root; outside it is left
/// whole, because a file being *outside* the project is the most interesting thing about it.
fn within(root: &Path, path: &str) -> String {
    Path::new(path)
        .strip_prefix(root)
        .ok()
        .map(|rest| rest.display().to_string())
        .unwrap_or_else(|| path.to_owned())
}

/// A TOML string literal, because `-c key=value` parses the value as TOML.
///
/// Without this a persona containing a quote or a newline becomes a parse error, and the whole run
/// fails on a character somebody happened to type.
fn toml_string(raw: &str) -> String {
    let escaped = raw
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder of this test's own, in the same style the rest of the crate uses.
    fn a_world(tag: &str) -> std::path::PathBuf {
        let here = std::env::temp_dir().join(format!(
            "epoch-codex-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&here);
        here
    }

    /// A one-pixel PNG, so the sniffing is real rather than mocked.
    const A_PNG: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];

    /// The item shape, measured 2026-08-24 against `codex.exe app-server`.
    fn an_image_item(encoded: &str) -> serde_json::Value {
        serde_json::json!({
            "type": "imageGeneration",
            "id": "exec-0e2181e7",
            "status": "completed",
            "revisedPrompt": "a green frog knight in ornate plate armour",
            "result": encoded,
        })
    }

    #[test]
    fn a_picture_codex_drew_itself_is_kept_and_counted_as_evidence() {
        use base64::Engine as _;
        let world = a_world("kept");
        let mut task = task(None);
        task.pictures = world.join("pictures");

        let mut done = Done::default();
        let mut streamed = String::new();
        let mut heard = Vec::new();
        read_app_item(
            &an_image_item(&base64::engine::general_purpose::STANDARD.encode(A_PNG)),
            &mut done,
            &mut streamed,
            &mut |progress| heard.push(progress),
            &task,
        );

        // The file exists. That is the whole claim: Codex left nothing on disk itself — measured
        // — so a turn Epoch does not write down produced nothing (ADR-0025).
        let kept: Vec<_> = std::fs::read_dir(&task.pictures)
            .expect("the folder was made")
            .filter_map(Result::ok)
            .collect();
        assert_eq!(kept.len(), 1, "{kept:?}");
        assert!(
            kept[0].file_name().to_string_lossy().ends_with(".png"),
            "named from the bytes: {:?}",
            kept[0].file_name()
        );
        assert_eq!(std::fs::read(kept[0].path()).expect("bytes"), A_PNG);

        assert_eq!(done.evidence.len(), 1, "{:?}", done.evidence);
        assert!(matches!(&heard[..], [Progress::Ran { ok: true, .. }]));
        let _ = std::fs::remove_dir_all(&world);
    }

    #[test]
    fn the_agents_own_words_never_become_the_file_name() {
        use base64::Engine as _;
        // ADR-0024's rule, one subsystem over. `revisedPrompt` is text an agent wrote, and it
        // would make a fine-looking filename and a traversal one turn later.
        let world = a_world("named");
        let mut task = task(None);
        task.pictures = world.join("pictures");
        let mut item = an_image_item(&base64::engine::general_purpose::STANDARD.encode(A_PNG));
        item["revisedPrompt"] = serde_json::json!("../../../../escaped");

        read_app_item(
            &item,
            &mut Done::default(),
            &mut String::new(),
            &mut |_| {},
            &task,
        );

        let kept = std::fs::read_dir(&task.pictures)
            .expect("the folder was made")
            .filter_map(Result::ok)
            .next()
            .expect("one picture");
        let name = kept.file_name().to_string_lossy().to_string();
        assert!(
            name.starts_with("codex-") && name.ends_with(".png"),
            "{name}"
        );
        let _ = std::fs::remove_dir_all(&world);
    }

    #[test]
    fn a_picture_that_cannot_be_kept_is_reported_rather_than_claimed() {
        // Nowhere to put one. Better than an evidence entry naming a file nobody can open —
        // History must be followable, not only readable.
        let mut done = Done::default();
        let mut heard = Vec::new();
        read_app_item(
            &an_image_item("not base64 at all !!!"),
            &mut done,
            &mut String::new(),
            &mut |progress| heard.push(progress),
            &task(None),
        );
        assert!(done.evidence.is_empty());
        assert!(matches!(&heard[..], [Progress::Ran { ok: false, .. }]));
    }

    struct Allows(bool);

    impl Approver for Allows {
        fn allow_for_this_turn(&self, _proposal: &Proposal) -> bool {
            self.0
        }
    }

    fn task(thread: Option<&str>) -> Task {
        Task {
            intent: "  say ok  ".into(),
            character: "Mage".into(),
            model: "gpt-5.4".into(),
            reasoning: Some(epoch_kernel::Reasoning::High),
            persona: "You are careful.".into(),
            crew: String::new(),
            skills: String::new(),
            library: String::new(),
            connections: String::new(),
            images: Vec::new(),
            directory: std::env::temp_dir(),
            pictures: std::path::PathBuf::new(),
            thread: thread.map(str::to_owned),
            autonomy: epoch_kernel::Autonomy::Manual,
            door: None,
        }
    }

    /// The exact line the real program printed when it refused its own patch.
    /// The other refusal, which never says "sandbox" — and the one the user actually hit.
    ///
    /// Captured from a real read-only run: the shell path is refused by *Windows*, as a file
    /// permission, so the only words that exist are the operating system's.
    #[test]
    fn a_picture_travels_as_a_data_uri_and_never_as_a_path() {
        // **The measured part, and the reason this assertion exists at all.**
        //
        // `ImageUserInput.url` is `string` in Codex's schema and says nothing more. A filesystem
        // path is accepted without complaint — `turn/start` returns a turn, the run begins — and
        // fails minutes later *upstream* with `Invalid 'input[4].content[0].image…`, the API's
        // error rather than Codex's, long after the thing that caused it.
        //
        // So the failure this guards against is not a crash. It is a feature that looks wired,
        // costs a turn every time, and blames somebody else.
        let sent = turn_input(&Task {
            images: vec![crate::agent::SharedImage {
                name: "flat.png".into(),
                bytes: vec![1, 2, 3],
                mime: "image/png",
            }],
            ..task(None)
        });

        let items = sent.as_array().expect("an array of UserInput");
        // The picture first, then the words.
        assert_eq!(items[0]["type"], "image");
        assert_eq!(items[0]["url"], "data:image/png;base64,AQID");
        assert_eq!(items[1]["type"], "text");
        // Trimmed, and otherwise the user's own words (ADR-0025).
        assert_eq!(items[1]["text"], "say ok");

        // With nothing shared it is exactly what it always was: one text item.
        let plain = turn_input(&task(None));
        assert_eq!(plain.as_array().expect("array").len(), 1);
        assert_eq!(plain[0]["type"], "text");
    }

    #[test]
    fn a_way_of_working_is_written_into_the_instructions() {
        // The last stretch of the same journey `turn::agent` asserts the start of. `Task` can
        // carry a Skill and this function can still ignore the field — which is exactly the
        // shape of the defect that put it here, one layer further out.
        let said = persona(&Task {
            library: String::new(),
            connections: String::new(),
            images: Vec::new(),
            skills: "Ways of working you have been given.\n\n## Code review\nRead the diff twice."
                .into(),
            ..task(None)
        });

        assert!(said.contains("Read the diff twice"));
        // Order is the meaning: who they are, then who they are with, then how the work is done.
        assert!(said.starts_with("You are Mage,"));
        assert!(said.find("You are careful.") < said.find("Read the diff twice"));
    }

    /// Manual is a real permission path, not an instruction to manufacture a failed native write.
    #[test]
    fn a_manual_turn_tells_codex_that_epoch_will_ask_before_a_native_write() {
        let read_only = persona(&Task {
            door: Some(crate::agent::Door {
                url: "http://127.0.0.1:8792/mcp".into(),
                token: "temporary".into(),
            }),
            ..task(None)
        });
        // Who they are comes first and is never displaced.
        assert!(read_only.starts_with("You are Mage,"));
        assert!(read_only.contains("You are careful."));
        assert!(read_only.contains("Codex will pause"));
        assert!(read_only.contains("built-in native Codex tool"));
        assert!(read_only.contains("separate HAND IT OVER flow"));
        assert!(read_only.contains("Project Root"));

        // Nothing to arrange when the sandbox is not what would stop it: the note would be about
        // a refusal that is not going to happen.
        let free = Task {
            autonomy: epoch_kernel::Autonomy::Auto,
            ..task(None)
        };
        assert!(!persona(&free).contains("Codex will pause"));
    }

    #[test]
    fn a_manual_turn_is_told_it_has_epochs_tools_and_that_they_pause() {
        // The pair that has to stay true together: what a Manual turn is *given*, and what it is
        // *told*. Split them and the character invents — that is how "ya lo disparé" happened
        // about a login it never called.
        //
        // Both halves moved here, twice, on evidence. Manual had no door and the sentence said
        // so; the door was opened while the sentence still denied it; and now the door is open
        // for a measured reason and the sentence says what is true.
        let manual = Task {
            autonomy: epoch_kernel::Autonomy::Manual,
            door: Some(door()),
            ..task(None)
        };
        let alone = Task {
            autonomy: epoch_kernel::Autonomy::Manual,
            door: None,
            ..task(None)
        };

        assert!(door_for(&manual).is_some(), "Manual has a door now");
        assert!(
            persona(&manual).contains("yours to use"),
            "and is told so, including that a call pauses rather than fails"
        );
        assert!(persona(&manual).contains("pauses"));
        assert!(
            persona(&alone).contains("not reachable this turn"),
            "with no door, saying so is what stops an invented answer"
        );
        // The native gate is a different question and it is untouched: it governs Codex's *own*
        // commands and file changes.
        assert!(persona(&manual).contains("Codex will pause"));
    }

    #[test]
    fn manual_uses_a_real_one_turn_native_approval() {
        assert_eq!(approval_policy(epoch_kernel::Autonomy::Manual), "untrusted");
        assert_eq!(approval_policy(epoch_kernel::Autonomy::Auto), "never");
        let sandbox = sandbox_policy(&task(None));
        assert_eq!(
            sandbox.get("type").and_then(serde_json::Value::as_str),
            Some("workspaceWrite")
        );
        // **Writes, not the network.** This asserted `networkAccess: false` until the two were
        // separated: Epoch gives Claude Code no sandbox at all, so closing Codex's egress was a
        // policy-shaped asymmetry that cost one agent `npm install` and bought nothing. What
        // must not slip is the confinement — where it may write, and never the whole machine.
        let roots = sandbox
            .get("writableRoots")
            .and_then(|r| r.as_array())
            .expect("writable roots are declared");
        assert_eq!(
            roots.first(),
            Some(&serde_json::json!(std::env::temp_dir())),
            "the Project Root comes first, and it is the only one the turn is about"
        );
        // **The security property, and it is the reason the caches are named one by one.**
        // `~/.cargo` holds `bin`, which is on PATH: writing there would let a turn leave an
        // executable that runs later outside any sandbox. `registry` and `git` are downloads.
        assert!(
            !roots.iter().any(|root| root
                .as_str()
                .is_some_and(|path| path.replace('\\', "/").ends_with("/.cargo/bin"))),
            "no root reaches cargo's bin directory: {roots:?}"
        );
        assert_ne!(
            sandbox.get("type").and_then(serde_json::Value::as_str),
            Some("dangerFullAccess")
        );
    }

    #[test]
    fn a_manual_command_is_only_accepted_after_epochs_answer() {
        let mut accepted = Vec::new();
        let request = serde_json::json!({
            "id": "ask-1",
            "method": "item/commandExecution/requestApproval",
            "params": { "command": "Set-Content hola.txt hola" }
        });
        reply_to_server_request(&mut accepted, &request, &task(None), &Allows(true));
        let answer: serde_json::Value = serde_json::from_slice(&accepted).expect("valid response");
        assert_eq!(
            answer
                .pointer("/result/decision")
                .and_then(serde_json::Value::as_str),
            Some("accept")
        );

        let mut declined = Vec::new();
        reply_to_server_request(&mut declined, &request, &task(None), &Allows(false));
        let answer: serde_json::Value = serde_json::from_slice(&declined).expect("valid response");
        assert_eq!(
            answer
                .pointer("/result/decision")
                .and_then(serde_json::Value::as_str),
            Some("decline")
        );
    }

    #[test]
    fn an_outside_mcp_tool_is_asked_about_as_an_elicitation_and_answered_in_its_own_shape() {
        // Captured from a real app-server, not composed here: this is the message that decides
        // whether Codex may call a tool on a server it does not own.
        let request = serde_json::json!({
            "id": 0,
            "method": "mcpServer/elicitation/request",
            "params": {
                "serverName": "epoch",
                "mode": "form",
                "_meta": { "codex_approval_kind": "mcp_tool_call", "persist": ["session", "always"] },
                "message": "Allow the epoch MCP server to run tool \"epoch_watchword\"?",
                "requestedSchema": { "type": "object", "properties": {} },
            },
        });
        let manual = Task {
            autonomy: epoch_kernel::Autonomy::Manual,
            ..task(None)
        };

        let mut accepted = Vec::new();
        reply_to_server_request(&mut accepted, &request, &manual, &Allows(true));
        let answer: serde_json::Value = serde_json::from_slice(&accepted).expect("valid response");
        // **`action`, not `decision`.** Its own response contract, and answering the wrong shape
        // is indistinguishable from not answering — which is the failure this replaced: the
        // request fell to the catch-all, was declined, and the turn stopped with no cause named.
        assert_eq!(
            answer
                .pointer("/result/action")
                .and_then(serde_json::Value::as_str),
            Some("accept")
        );

        let mut declined = Vec::new();
        reply_to_server_request(&mut declined, &request, &manual, &Allows(false));
        let answer: serde_json::Value = serde_json::from_slice(&declined).expect("valid response");
        assert_eq!(
            answer
                .pointer("/result/action")
                .and_then(serde_json::Value::as_str),
            Some("decline")
        );

        // Auto does not ask, and does not need an approver to say yes. Spelled out rather than
        // taken from the shared helper, which is Manual — an assumption this assertion caught
        // when it was left implicit.
        let mut automatic = Vec::new();
        reply_to_server_request(
            &mut automatic,
            &request,
            &Task {
                autonomy: epoch_kernel::Autonomy::Auto,
                ..task(None)
            },
            &Allows(false),
        );
        let answer: serde_json::Value = serde_json::from_slice(&automatic).expect("valid response");
        assert_eq!(
            answer
                .pointer("/result/action")
                .and_then(serde_json::Value::as_str),
            Some("accept"),
            "in Auto the mode has already answered, whatever an approver would say"
        );
    }

    #[test]
    fn a_manual_native_permission_grant_waits_for_epochs_answer_and_expires_with_the_turn() {
        // This is the request current Codex app-server sends before it has materialised a
        // command item.  Missing this method meant Epoch silently declined it and never opened
        // `MAGE NEEDS YOUR WORD`.
        let request = serde_json::json!({
            "id": "ask-permissions",
            "method": "item/permissions/requestApproval",
            "params": {
                "reason": "create manual-test.txt",
                "cwd": "C:\\work",
                "permissions": {
                    "fileSystem": { "write": ["C:\\work"] }
                }
            }
        });

        let mut accepted = Vec::new();
        reply_to_server_request(&mut accepted, &request, &task(None), &Allows(true));
        let answer: serde_json::Value = serde_json::from_slice(&accepted).expect("valid response");
        assert_eq!(
            answer
                .pointer("/result/scope")
                .and_then(serde_json::Value::as_str),
            Some("turn")
        );
        assert_eq!(
            answer
                .pointer("/result/permissions/fileSystem/write/0")
                .and_then(serde_json::Value::as_str),
            Some("C:\\work"),
            "Epoch must echo only the server-requested profile, never manufacture broader access"
        );
        assert_eq!(
            answer
                .pointer("/result/strictAutoReview")
                .and_then(serde_json::Value::as_bool),
            Some(true),
            "a Manual grant must keep reviewing each following native action in this turn"
        );
        assert!(
            answer.pointer("/result/permissions/network").is_none(),
            "a filesystem approval must never turn into a network grant"
        );

        let mut declined = Vec::new();
        reply_to_server_request(&mut declined, &request, &task(None), &Allows(false));
        let answer: serde_json::Value = serde_json::from_slice(&declined).expect("valid response");
        assert_eq!(
            answer.pointer("/result/permissions"),
            Some(&serde_json::json!({}))
        );
        assert_eq!(
            answer
                .pointer("/result/scope")
                .and_then(serde_json::Value::as_str),
            Some("turn")
        );
    }

    // ---------------------------------------------------------------- the live transport
    //
    // Everything below drives `read_app_item`, which is what production actually runs. The
    // evidence rules were captured from `codex exec --json` and asserted there for months, and
    // that parser is now `#[cfg(test)]`: the suite was proving the owner's "we have no way of
    // knowing who did it" fix against a code path Epoch no longer takes.
    //
    // The shapes are read from the app-server's own published schema rather than remembered:
    // `FileChangeThreadItem` carries `changes[].kind` as an **object** (`PatchChangeKind` is a
    // tagged union), and both it and `CommandExecutionThreadItem` carry a status that includes
    // `declined` — the ending a Manual **NO** produces.

    /// Drive one completed item, and collect what the surface was told.
    fn item(json: &str) -> (Done, Vec<Progress>) {
        let mut done = Done::default();
        let mut seen = Vec::new();
        let mut streamed = String::new();
        read_app_item(
            &serde_json::from_str(json).expect("json"),
            &mut done,
            &mut streamed,
            &mut |p| seen.push(p),
            &task(None),
        );
        (done, seen)
    }

    /// Drive one server notification, and collect what the surface was told.
    fn heard(method: &str, params: serde_json::Value) -> (Heard, Done, Vec<Progress>) {
        let mut done = Done::default();
        let mut seen = Vec::new();
        let mut streamed = String::new();
        let ending = read_app_notification(
            method,
            &serde_json::json!({ "method": method, "params": params }),
            &mut done,
            &mut streamed,
            &mut |p| seen.push(p),
            &task(None),
        );
        (ending, done, seen)
    }

    #[test]
    fn what_a_turn_used_is_reported_and_its_budget_is_admitted_to_be_unknown() {
        // Codex reports what a turn used and never what it is allowed. A gauge with an invented
        // denominator is the cold-instrument rule broken at its most tempting, so the budget is
        // sent as unknown and the surface keeps a count separate from a fraction.
        let (ending, _, seen) = heard(
            "thread/tokenUsage/updated",
            serde_json::json!({ "tokenUsage": { "total": { "totalTokens": 12_345 } } }),
        );

        assert_eq!(ending, Heard::Nothing);
        assert_eq!(
            seen,
            vec![Progress::Window {
                used: 12_345,
                budget: 0,
                session: None,
            }]
        );
    }

    #[test]
    fn a_usage_report_without_a_number_says_nothing_at_all() {
        // Rather than a reassuring zero. The shape has changed across Codex releases before.
        let (_, _, seen) = heard(
            "thread/tokenUsage/updated",
            serde_json::json!({ "tokenUsage": { "total": {} } }),
        );
        assert!(seen.is_empty());
    }

    #[test]
    fn a_failed_turn_carries_what_the_server_said() {
        let (ending, ..) = heard(
            "turn/completed",
            serde_json::json!({ "turn": { "status": "failed", "error": { "message": "no room left" } } }),
        );
        assert_eq!(ending, Heard::Ended(Some("no room left".into())));
    }

    #[test]
    fn a_turn_that_failed_silently_is_still_a_failure() {
        // Reporting a silent success would put an empty answer in the Chronicle as if it were
        // the agent's reply.
        let (ending, ..) = heard(
            "turn/completed",
            serde_json::json!({ "turn": { "status": "failed" } }),
        );
        assert_eq!(ending, Heard::Ended(Some("the Codex turn failed".into())));
    }

    #[test]
    fn a_turn_that_completed_ends_with_nothing_to_report() {
        let (ending, ..) = heard(
            "turn/completed",
            serde_json::json!({ "turn": { "status": "completed" } }),
        );
        assert_eq!(ending, Heard::Ended(None));
    }

    #[test]
    fn a_delta_is_said_as_it_arrives_and_is_remembered() {
        // Remembered, so the completed item that repeats the whole answer can send only what is
        // missing instead of saying it twice.
        let mut done = Done::default();
        let mut streamed = String::from("Hel");
        let mut seen = Vec::new();
        read_app_notification(
            "item/agentMessage/delta",
            &serde_json::json!({ "params": { "delta": "lo" } }),
            &mut done,
            &mut streamed,
            &mut |p| seen.push(p),
            &task(None),
        );

        assert_eq!(seen, vec![Progress::Said("lo".into())]);
        assert_eq!(streamed, "Hello");
    }

    #[test]
    fn an_unknown_notification_is_skipped_rather_than_fatal() {
        // Codex updated itself twice while this file was being written. An upgrade must not be
        // an outage.
        let (ending, done, seen) = heard("thread/somethingNew", serde_json::json!({}));
        assert_eq!(ending, Heard::Nothing);
        assert_eq!(done, Done::default());
        assert!(seen.is_empty());
    }

    #[test]
    fn a_file_the_agent_added_is_work_and_is_evidence() {
        let inside = std::env::temp_dir().join("notes.md");
        let (done, seen) = item(&format!(
            r#"{{"type":"fileChange","id":"i1","status":"completed",
                "changes":[{{"path":{},"kind":{{"type":"add"}},"diff":""}}]}}"#,
            serde_json::to_string(&inside.display().to_string()).expect("json")
        ));

        // Said in its own words for what it did: adding a file is a different act from editing
        // one, and a reader deserves to know which.
        assert_eq!(
            seen,
            vec![Progress::Ran {
                tool: "created".into(),
                // Relative to the project, because an absolute Windows path is most of a
                // Terminal line before it has said anything.
                detail: "notes.md".into(),
                ok: true,
            }]
        );
        assert_eq!(
            done.evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["created notes.md"]
        );
        assert_eq!(done.evidence[0].reference, "notes.md");
    }

    #[test]
    fn a_patch_across_three_files_is_three_things_that_happened() {
        // One line naming the first file would hide the other two.
        let (done, seen) = item(
            r#"{"type":"fileChange","id":"i1","status":"completed","changes":[
                {"path":"a.md","kind":{"type":"add"},"diff":""},
                {"path":"b.md","kind":{"type":"update"},"diff":""},
                {"path":"c.md","kind":{"type":"delete"},"diff":""}]}"#,
        );

        assert_eq!(seen.len(), 3);
        assert_eq!(
            done.evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["created a.md", "edited b.md", "deleted c.md"]
        );
        // And each names the file, so History can be followed rather than only read.
        assert_eq!(
            done.evidence
                .iter()
                .map(|m| m.reference.as_str())
                .collect::<Vec<_>>(),
            ["a.md", "b.md", "c.md"]
        );
    }

    #[test]
    fn a_patch_that_did_not_land_is_not_evidence() {
        // What is reported is what *happened*, and a Quest that produced nothing must be able to
        // say so (ADR-0025).
        let (done, seen) = item(
            r#"{"type":"fileChange","id":"i1","status":"failed",
                "changes":[{"path":"C:\\elsewhere\\notes.md","kind":{"type":"update"},"diff":""}]}"#,
        );

        assert!(done.evidence.is_empty(), "nothing was written");
        assert!(matches!(&seen[0], Progress::Ran { ok: false, .. }));
        // Outside the project, so it keeps its whole path - a file being *outside* is the most
        // interesting thing about it.
        assert!(matches!(&seen[0], Progress::Ran { detail, .. } if detail.contains("elsewhere")));
    }

    #[test]
    fn a_change_the_user_refused_is_not_evidence_either() {
        // The ending a Manual **NO** produces. `declined` is a real status in the app-server's
        // schema, and it is the one case where nothing happened *because the user said so* -
        // recording it as work would make the approval a formality.
        let (done, seen) = item(
            r#"{"type":"fileChange","id":"i1","status":"declined",
                "changes":[{"path":"hola.txt","kind":{"type":"add"},"diff":""}]}"#,
        );

        assert!(done.evidence.is_empty(), "the user said no");
        assert!(matches!(&seen[0], Progress::Ran { ok: false, .. }));
    }

    #[test]
    fn a_command_that_worked_is_evidence_and_one_that_did_not_is_not() {
        let (ran, _) = item(
            r#"{"type":"commandExecution","id":"i1","status":"completed",
                "command":"cargo test\nsecond line","cwd":"."}"#,
        );
        // The first line only: a Terminal row is one line, and a command that spans several
        // would push everything after it off the screen.
        assert_eq!(
            ran.evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["ran cargo test"]
        );
        assert_eq!(ran.evidence[0].reference, "cargo test");

        let (refused, seen) = item(
            r#"{"type":"commandExecution","id":"i1","status":"declined","command":"rm -rf /","cwd":"."}"#,
        );
        assert!(refused.evidence.is_empty());
        assert!(matches!(&seen[0], Progress::Ran { ok: false, .. }));
    }

    #[test]
    fn a_completed_message_does_not_say_again_what_was_already_streamed() {
        // Deltas reach the Chronicle as they arrive and the completed item repeats the whole
        // answer. Without the prefix rule every agent turn would appear twice.
        let mut done = Done::default();
        let mut streamed = String::from("Hello");
        let mut seen = Vec::new();
        read_app_item(
            &serde_json::json!({ "type": "agentMessage", "id": "i1", "text": "Hello there" }),
            &mut done,
            &mut streamed,
            &mut |p| seen.push(p),
            &task(None),
        );

        assert_eq!(seen, vec![Progress::Said(" there".into())]);
        assert_eq!(done.text, "Hello there");
        assert_eq!(streamed, "Hello there");
    }

    #[test]
    fn a_message_that_replaced_what_was_streamed_is_said_in_full() {
        // Not a suffix, so there is no way to send only the difference. Saying it whole is the
        // honest ending: the Chronicle keeps what the agent finally said.
        let mut done = Done::default();
        let mut streamed = String::from("Thinking...");
        let mut seen = Vec::new();
        read_app_item(
            &serde_json::json!({ "type": "agentMessage", "id": "i1", "text": "Done." }),
            &mut done,
            &mut streamed,
            &mut |p| seen.push(p),
            &task(None),
        );

        assert_eq!(seen, vec![Progress::Said("Done.".into())]);
    }

    #[test]
    fn an_unknown_item_is_skipped_rather_than_fatal() {
        // Codex updated itself twice while this file was being written. An upgrade must not be
        // an outage.
        let (done, seen) = item(r#"{"type":"somethingNew","id":"i1"}"#);
        assert_eq!(done, Done::default());
        assert!(seen.is_empty());
    }

    /// Every rung the scale offers must reach the command line.
    #[test]
    fn every_rung_this_agent_offers_has_a_word_for_it() {
        let brain = epoch_kernel::Brain::Agent {
            agent: ID.into(),
            model: String::new(),
        };
        for rung in crate::deliberation::scale(&brain) {
            assert!(
                effort_of(Some(*rung)).is_some(),
                "{rung:?} has no effort value"
            );
        }
        // `Off` stays unsendable: Codex has no level meaning "do not deliberate", and mapping it
        // to `low` would be Epoch deciding the instruction meant something else.
        assert_eq!(effort_of(Some(epoch_kernel::Reasoning::Off)), None);
        assert_eq!(effort_of(None), None);
    }

    /// The label a person reads is not the value the service takes.
    #[test]
    fn the_value_sent_is_the_one_the_service_accepts() {
        // Codex's own picker says "Extra high"; the API rejects `extra-high` with
        // invalid_request_error. Measured, and the reason these two are kept apart.
        assert_eq!(
            effort_of(Some(epoch_kernel::Reasoning::XHigh)),
            Some("xhigh")
        );
    }

    #[test]
    fn no_epoch_mode_turns_the_sandbox_off() {
        // Codex offers `danger-full-access`, which unsandboxes the whole machine. No Epoch mode
        // means that, and mapping `Auto` onto it would grant more than the word the user read.
        //
        // Asserted across every mode rather than for one task: this is the guarantee, and a
        // single-case check would not notice a fourth rung arriving with a different answer.
        for mode in [
            epoch_kernel::Autonomy::Manual,
            epoch_kernel::Autonomy::AcceptEdits,
            epoch_kernel::Autonomy::Auto,
        ] {
            let sandbox = sandbox_policy(&Task {
                autonomy: mode,
                ..task(None)
            });
            assert_eq!(
                sandbox.get("type").and_then(serde_json::Value::as_str),
                Some("workspaceWrite"),
                "every mode stays inside the Project Root"
            );
            assert_eq!(
                sandbox
                    .get("writableRoots")
                    .and_then(|r| r.as_array())
                    .and_then(|roots| roots.first()),
                Some(&serde_json::json!(std::env::temp_dir())),
                "and the Project Root leads the list, whatever the mode"
            );
            assert!(
                matches!(approval_policy(mode), "untrusted" | "never"),
                "an unknown approval policy is a permission nobody has read"
            );
        }
    }

    #[test]
    fn the_session_handle_is_read_from_whichever_shape_the_server_used() {
        // The thread-start reply has changed once across Codex releases. A missing handle must
        // stay missing rather than become an empty string that looks like a resumable session.
        assert_eq!(
            thread_id_from(&serde_json::json!({ "result": { "thread": { "id": "019fe4fe" } } })),
            Some("019fe4fe")
        );
        assert_eq!(
            thread_id_from(&serde_json::json!({ "result": { "id": "019fe4fe" } })),
            Some("019fe4fe")
        );
        assert_eq!(thread_id_from(&serde_json::json!({ "result": {} })), None);
    }

    #[test]
    fn a_persona_with_quotes_and_newlines_survives_the_command_line() {
        // `-c key=value` parses the value as TOML, so an unescaped quote is a parse error and the
        // whole run dies on a character somebody happened to type.
        let quoted = toml_string("You say \"no\" when\nit is unsafe.\\ok");
        assert_eq!(quoted, "\"You say \\\"no\\\" when\\nit is unsafe.\\\\ok\"");
        assert!(quoted.starts_with('"') && quoted.ends_with('"'));
    }

    /// The half of all edits that used to leave no trace.
    ///
    /// Captured from a real run. Asked to edit a file, Codex sometimes shells out and sometimes
    /// uses its own patch tool, and the two are reported as entirely different events - so a
    /// Quest that had written a file could honestly say `NO EVIDENCE YET`, and the Terminal
    /// could say nothing had run while a file was being created.
    #[test]
    fn login_status_is_read_and_never_guessed() {
        // It prints a sentence, not JSON.
        assert_eq!(
            read_login("Logged in using ChatGPT"),
            (Some(true), Some("Logged in using ChatGPT".into()))
        );
        assert_eq!(read_login("Not logged in"), (Some(false), None));
        // Anything else is *unasked*. Telling somebody to sign in when they already are sends
        // them to fix the wrong thing.
        assert_eq!(read_login("something else entirely"), (None, None));
        assert_eq!(read_login(""), (None, None));
    }

    #[test]
    fn plan_usage_is_the_remaining_amount_from_the_cli_snapshot() {
        // Captured from Codex 0.147.0's read-only `account/rateLimits/read` response, with the
        // account-specific fields removed. The CLI reports *used*, while the bridge promises the
        // person the amount still available.
        let snapshot = serde_json::json!({
            "rateLimits": {
                "primary": {
                    "usedPercent": 88,
                    "windowDurationMins": 10080,
                    "resetsAt": 1786857538
                }
            }
        });
        assert_eq!(
            read_plan_snapshot(&snapshot),
            Some(PlanUsage {
                used_percent: 88,
                remaining_percent: 12,
                window_minutes: Some(10080),
                resets_at: Some(1786857538),
                // Nothing said there was credit, so there is none to report.
                credits: None,
            })
        );
    }

    #[test]
    fn a_credit_balance_is_read_from_the_string_the_server_actually_sends() {
        // Measured against a real reply: the balance arrives as **text**, beside `hasCredits`
        // and `unlimited`. A remembered shape would have looked for a number and found nothing.
        let snapshot = serde_json::json!({
            "rateLimits": {
                "primary": { "usedPercent": 100, "windowDurationMins": 10080 },
                "credits": { "hasCredits": true, "unlimited": false, "balance": "379.1741100000" }
            }
        });

        let read = read_plan_snapshot(&snapshot).expect("a snapshot");
        assert_eq!(read.remaining_percent, 0);
        assert_eq!(
            read.credits,
            Some(Credits {
                balance: 379.17411,
                unlimited: false
            }),
            "a window at 0% with credit left is not a character that has stopped working"
        );
    }

    #[test]
    fn an_account_that_carries_no_credit_reports_none_rather_than_nothing_left() {
        // Two different sentences. `hasCredits: false` means this account does not work that
        // way; a zero balance would mean it does and is empty.
        let snapshot = serde_json::json!({
            "rateLimits": {
                "primary": { "usedPercent": 10 },
                "credits": { "hasCredits": false, "unlimited": false, "balance": "0" }
            }
        });
        assert_eq!(
            read_plan_snapshot(&snapshot).expect("a snapshot").credits,
            None
        );
    }

    #[test]
    fn an_unlimited_balance_is_said_rather_than_counted() {
        let snapshot = serde_json::json!({
            "rateLimits": {
                "primary": { "usedPercent": 10 },
                "credits": { "hasCredits": true, "unlimited": true }
            }
        });
        assert_eq!(
            read_plan_snapshot(&snapshot).expect("a snapshot").credits,
            Some(Credits {
                balance: 0.0,
                unlimited: true
            })
        );
    }

    #[test]
    fn a_balance_nobody_put_a_number_on_is_not_a_balance() {
        let snapshot = serde_json::json!({
            "rateLimits": {
                "primary": { "usedPercent": 10 },
                "credits": { "hasCredits": true, "unlimited": false, "balance": "lots" }
            }
        });
        assert_eq!(
            read_plan_snapshot(&snapshot).expect("a snapshot").credits,
            None
        );
    }

    #[test]
    fn an_unrecognised_or_impossible_plan_snapshot_stays_unavailable() {
        assert!(read_plan_snapshot(&serde_json::json!({})).is_none());
        assert!(read_plan_snapshot(&serde_json::json!({
            "rateLimits": { "primary": { "usedPercent": 101 } }
        }))
        .is_none());
    }

    #[test]
    #[cfg(windows)]
    fn a_windowsapps_desktop_copy_is_not_a_runnable_codex_cli() {
        // This is the actual path shape the desktop app puts on PATH. Treating a visible file as
        // executable made the Launcher say Codex was absent although its active CLI was signed in.
        //
        // Windows-only, because `WindowsApps` is: the rule under test is *this store-packaged
        // copy is not runnable*, and on a Mac those strings are relative names that no part of
        // this reasoning applies to.
        let packaged = Path::new(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.803.5235.0_x64__2p2nqsd0c76g0\app\resources\codex.exe",
        );
        let extensionless_packaged = Path::new(
            r"C:\Program Files\WindowsApps\OpenAI.Codex_26.803.5235.0_x64__2p2nqsd0c76g0\app\resources\codex",
        );
        let active = Path::new(
            r"C:\Users\somebody\AppData\Local\OpenAI\Codex\bin\cfac6bda2d141e07\codex.exe",
        );

        assert!(!runnable_from_epoch(packaged));
        assert!(!runnable_from_epoch(extensionless_packaged));
        assert!(runnable_from_epoch(active), "the active CLI stays usable");
    }

    /// **What Epoch tells another program where it is must be a path that program can read.**
    ///
    /// The defect, as the value that was being sent: containment canonicalises a Project Root,
    /// and on Windows that is `\\?\C:\Users\...`. Gemini's Node runtime split it into a root
    /// of `\\?\` and a first segment of `C:` and answered `EISDIR: illegal operation on a
    /// directory, lstat 'C:'` — every turn in such a World, before the model was reached.
    ///
    /// Asserted as the **property** rather than against one string: nothing Epoch hands over as
    /// a place may carry the prefix, whatever the path is.
    #[test]
    fn the_place_a_turn_happens_is_written_plainly() {
        let mut task = task(None);
        task.directory = std::path::PathBuf::from(r"\\?\C:\Users\someone\a project");
        assert_eq!(here(&task), r"C:\Users\someone\a project");

        task.directory = std::path::PathBuf::from(r"\\?\UNC\server\share\work");
        assert_eq!(here(&task), r"\\server\share\work");

        // A path without the prefix is untouched, so this cannot quietly rewrite anything else.
        task.directory = std::path::PathBuf::from(r"C:\Users\someone\plain");
        assert_eq!(here(&task), r"C:\Users\someone\plain");
    }

    /// Against the real program, when there is one.
    #[test]
    #[ignore = "needs Codex installed"]
    fn codex_is_actually_reachable_on_this_machine() {
        let found = Codex::default().probe();
        assert!(found.installed, "{:?}", found.note);
        println!(
            "{} {} · {:?}",
            found.name,
            found.version.unwrap_or_default(),
            found.account
        );
    }

    /// **What this machine's Codex actually offers.** Opt-in: it starts the app-server.
    ///
    /// The assertion is deliberately not a list of names — that would be the hardcoding this
    /// replaced, wearing a test. What must hold is that a real answer came back, that every row
    /// has an id and a label, and that nothing Codex marks `hidden` was passed through.
    #[test]
    #[ignore = "needs a signed-in Codex CLI"]
    fn codex_actually_names_its_models_on_this_machine() {
        let found = models_offered(None).expect("Codex should answer model/list");
        assert!(!found.is_empty());
        for (id, label) in &found {
            assert!(!id.trim().is_empty(), "every row is addressable");
            assert!(!label.trim().is_empty(), "every row is readable");
            println!("{id} · {label}");
        }
    }

    /// A deliberately opt-in integration check: it consults the currently signed-in local CLI.
    #[test]
    #[ignore = "needs a signed-in Codex CLI"]
    fn codex_plan_usage_is_actually_measured_on_this_machine() {
        let found =
            plan_usage(None).expect("Codex app-server should report its rate-limit snapshot");
        assert!(found.used_percent <= 100);
        assert_eq!(found.remaining_percent, 100 - found.used_percent);
        println!(
            "{}% remaining in {:?} minutes",
            found.remaining_percent, found.window_minutes
        );
    }
    /// A directory holding a `codex.exe`, and its sandbox helper when asked for one.
    ///
    /// Windows-only, like the two tests that use it: a `.exe` and a sandbox helper are facts
    /// about that platform. Without the gate the Mac warns `never used`, and `-D warnings` is
    /// set on every platform this could be built on.
    #[cfg(windows)]
    fn install(tag: &str, complete: bool) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("epoch-codex-{tag}-{n}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("codex.exe"), b"").unwrap();
        if complete {
            std::fs::write(dir.join("codex-windows-sandbox-setup.exe"), b"").unwrap();
        }
        dir.join("codex.exe")
    }

    #[test]
    #[cfg(windows)]
    fn a_complete_install_is_preferred_over_one_that_merely_starts() {
        // The defect, as the choice that was being made. The desktop app puts a `codex.exe` on
        // PATH without `codex-windows-sandbox-setup.exe` beside it. It starts, answers
        // `--version` and reports itself signed in — and then every command it runs fails,
        // because it cannot build a sandbox. A character on that brain had no shell at all.
        //
        // PATH still comes first among equals; completeness is what outranks it.
        let broken = install("desktop", false);
        let whole = install("managed", true);

        assert_eq!(
            choose(vec![broken.clone(), whole.clone()]),
            Some(whole.clone()),
            "the one that can actually run a command wins"
        );
        assert_eq!(
            choose(vec![whole.clone(), broken.clone()]),
            Some(whole),
            "and order does not rescue the broken one"
        );

        let _ = std::fs::remove_dir_all(broken.parent().unwrap());
    }

    #[test]
    #[cfg(windows)]
    fn an_incomplete_install_is_still_used_when_it_is_the_only_one() {
        // Preferred, never required. A future Codex that stops shipping the helper must degrade
        // to today's behaviour rather than to "not installed" — refusing to run the only Codex
        // on the machine would be worse than the bug this rule fixes.
        let only = install("alone", false);
        assert_eq!(choose(vec![only.clone()]), Some(only.clone()));
        assert_eq!(choose(Vec::new()), None);
        let _ = std::fs::remove_dir_all(only.parent().unwrap());
    }

    fn door() -> crate::agent::Door {
        crate::agent::Door {
            url: "http://127.0.0.1:8792/mcp".into(),
            token: "temporary".into(),
        }
    }

    #[test]
    fn an_auto_turn_is_given_epochs_door_in_the_shape_codex_accepts() {
        // Measured against `codex-cli 0.147.0-alpha.6.6` with `--strict-config`, which refuses
        // `mcp_servers.<name>.bogus_key` as an *unknown configuration field* and accepts these
        // two. The refusal is what makes the acceptance mean anything: without a control, a run
        // that starts proves only that nothing crashed.
        let arguments = app_server_arguments(&Task {
            autonomy: epoch_kernel::Autonomy::Auto,
            door: Some(door()),
            ..task(None)
        });

        assert!(
            arguments.contains(&format!(
                "mcp_servers.{SERVER}.url=\"http://127.0.0.1:8792/mcp\""
            )),
            "{arguments:?}"
        );
        assert!(
            arguments.contains(&format!(
                "mcp_servers.{SERVER}.bearer_token_env_var=\"EPOCH_DOOR_TOKEN\""
            )),
            "{arguments:?}"
        );
        // Named, never written: the token reaches the child through its environment, so it is
        // not on a command line that any process listing can read.
        assert!(
            !arguments.iter().any(|a| a.contains("temporary")),
            "the token is never an argument: {arguments:?}"
        );
    }

    #[test]
    fn a_manual_turn_gets_the_door_and_keeps_its_native_gate() {
        // **This replaces the opposite assertion, and the belief under it was wrong.**
        //
        // Manual was given no door because a second MCP route's approval was said to be
        // unanswerable by the app-server. Driving it showed the session healthy and `tools/call`
        // simply never arriving — and the cause was Epoch's own: Codex asks about an outside
        // tool through `mcpServer/elicitation/request`, which fell to a catch-all that declined
        // everything it did not recognise. Epoch was refusing its own tools.
        //
        // `tests/codex_untrusted_mcp_probe.rs` runs both approval policies against an outside
        // MCP server with only that field varied. With the elicitation answered, `never` and
        // `untrusted` both call the tool. The limitation was never Codex's.
        //
        // Two gates now, both real: the native `untrusted` policy governs Codex's own commands
        // and file changes, and the elicitation governs Epoch's tools — and in Manual the user
        // answers both.
        let arguments = app_server_arguments(&Task {
            autonomy: epoch_kernel::Autonomy::Manual,
            door: Some(door()),
            ..task(None)
        });

        assert!(
            arguments
                .iter()
                .any(|a| a.starts_with(&format!("mcp_servers.{SERVER}.url="))),
            "{arguments:?}"
        );
        assert_eq!(
            approval_policy(epoch_kernel::Autonomy::Manual),
            "untrusted",
            "and the agent's own tools still stop for the user"
        );
    }

    #[test]
    fn a_turn_with_no_door_configures_no_server_in_either_mode() {
        for autonomy in [epoch_kernel::Autonomy::Auto, epoch_kernel::Autonomy::Manual] {
            let arguments = app_server_arguments(&Task {
                autonomy,
                door: None,
                ..task(None)
            });
            assert!(
                !arguments.iter().any(|a| a.starts_with("mcp_servers.")),
                "{autonomy:?}: {arguments:?}"
            );
        }
    }
}
