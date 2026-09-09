//! Tools that live outside Epoch, reached over the Model Context Protocol.
//!
//! ## Why this is a *client* and not an integration
//!
//! Every tool an MCP server offers becomes an ordinary [`Capability`]. It is described the same
//! way, explained the same way, judged by the same `decide()`, and recorded the same way. There
//! is no second permission system, no second vocabulary, and nothing above this module can tell
//! an outside tool from one Epoch wrote — which is the whole reason ADR-0008 made capabilities a
//! contract rather than a list.
//!
//! ## The finding: MCP has no effects, so every outside tool is treated as the ceiling
//!
//! Risk in Epoch is **derived from declared effects, never declared directly** (ADR-0008/0009) —
//! precisely so that nobody can label a delete as low-risk. MCP declares no effects at all.
//!
//! It does offer *hints*: `readOnlyHint`, `destructiveHint`, `idempotentHint`. **They are
//! refused.** A hint is a third party grading its own risk, which is the exact claim ADR-0008
//! says must not be expressible — and an outside server is the least trustworthy place it could
//! come from. Believing `readOnlyHint: true` would mean a tool that deletes could be waved
//! through in a mode the user set to ask first.
//!
//! So an outside tool declares **every effect and a permanent reversal**: it reads, writes,
//! deletes, executes and reaches the network until proven otherwise, and it can prove nothing.
//! The consequence is deliberate and worth stating plainly: **every MCP tool asks the first
//! time**, in every mode except Auto. The Trust store already remembers a standing answer
//! (ADR-0009), so it asks once rather than every time — which is the shape this should have.
//!
//! ## One process per server, spoken to over stdin
//!
//! JSON-RPC 2.0 over a child process's stdio, one line per message. Spawned on first use rather
//! than at startup: a server nobody asks anything of should not be a process on the user's
//! machine, and a World with no MCP servers configured must cost nothing.
//!
//! Reading is bounded by a thread and a channel for the same reason [`crate::capabilities::machine`]
//! does it — a child that never answers would otherwise hang the turn with no way out.

use crate::guard::guard;
use epoch_models::quiet::Quiet;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::Mutex;
use std::time::Duration;

use epoch_kernel::{
    Arguments, CapabilityId, Descriptor, Effect, Parameter, Reversal, Value, ValueKind,
};
use serde::{Deserialize, Serialize};

use crate::capability::{Capability, CapabilityError, Outcome};

/// How long to wait for a server to answer one message.
///
/// Generous, because starting a server can mean downloading it the first time — and bounded,
/// because a child that never answers is otherwise a turn that never ends.
const REPLY_TIMEOUT: Duration = Duration::from_secs(120);

/// What Epoch tells a server about itself. Part of the handshake, not a preference.
const PROTOCOL: &str = "2025-06-18";

/// Most of one reply this build will carry back to a model.
///
/// The same ceiling `fetch_url` uses, for a worse reason. A page snapshot from a browser tool is
/// routinely tens of thousands of tokens — larger than the whole context of a small local model.
/// Handed to the Composer whole it is *dropped* (ADR-0012 reduces by dropping), the model never
/// sees the result, and it calls the tool again. And again: eight identical calls, every one
/// reported as a success, until the round limit ends the turn with nothing to show.
///
/// A loop like that is indistinguishable from a broken tool, so this cuts and **says it cut**.
/// A truncated answer the model can reason about beats a perfect one it never receives.
const MAX_TEXT: usize = 24 * 1024;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// One MCP server, as the user configured it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Configured {
    /// Which one you mean. Part of every tool id it contributes, so it stays to characters that
    /// survive being shared: lowercase letters, digits, underscore.
    pub id: String,
    /// The program to run. **No shell** — the same rule `run_command` follows, for the same
    /// reason: shell injection is not filtered here, it is inexpressible.
    ///
    /// On Windows a Node launcher is `npx.cmd`, not `npx`. Said here because the failure is
    /// otherwise "program not found" for a program the user can plainly see is installed.
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment the server is given, for values that are not credentials.
    ///
    /// A bucket name, a mode, a root path. Plainly in the file, because that is what it is:
    /// configuration somebody may want to read and edit.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<String, String>,
    /// Environment variables whose **values live in the encrypted store**, never here.
    ///
    /// Only the names are written. A token in `mcp.toml` would be a credential in a file that
    /// travels with a vault and is trivially copied; the separation is what makes "this file
    /// cannot contain a key" a property of the type rather than a rule somebody remembers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secrets: Vec<String>,
    /// The catalogue entry this came from, when it came from one.
    ///
    /// Recorded so the Workshop can say *you already have this* about the entry rather than
    /// about a name. The id is renamed on collision, so matching on the id would call a second
    /// copy a different server — which is how one gets installed three times.
    ///
    /// `None` for anything the user added by hand, and that is the honest answer: it did not
    /// come from a catalogue.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Off keeps the entry and stops the process from ever being spawned.
    #[serde(default = "yes")]
    pub enabled: bool,
}

const fn yes() -> bool {
    true
}

/// Everything configured, as a surface sees it. Every field always present.
#[derive(Debug, Clone, Serialize)]
pub struct View {
    pub servers: Vec<Listed>,
    pub problem: Option<String>,
}

/// One configured server on the wire — never skipping a field, whatever it holds.
///
/// The credential **names** travel; no value does, and there is no field one could occupy. That
/// is what lets a surface read this, edit it and send it back without a token passing through it.
#[derive(Debug, Clone, Serialize)]
pub struct Listed {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub secrets: Vec<String>,
    pub enabled: bool,
}

impl Default for Configured {
    /// Enabled, because that is what somebody who just added a server meant. A derived `Default`
    /// would silently make it `false` and the server would never start.
    fn default() -> Self {
        Self {
            id: String::new(),
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            secrets: Vec::new(),
            source: None,
            enabled: true,
        }
    }
}

impl Configured {
    /// Everything wrong with this entry, in the user's terms.
    pub fn problems(&self) -> Vec<String> {
        let mut found = Vec::new();
        let id = self.id.trim();
        if id.is_empty() {
            found.push("a server needs a name".into());
        } else if let Some(bad) = id
            .chars()
            .find(|c| !(c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '_'))
        {
            // The name becomes part of a capability id, so it obeys the capability id's rules
            // rather than a looser set that would fail later at registration.
            found.push(format!(
                "'{id}' cannot contain '{bad}'; use lowercase, digits or '_'"
            ));
        }
        if self.command.trim().is_empty() {
            found.push(format!("'{id}' needs a command to run"));
        }
        found
    }
}

/// Every configured server.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Servers {
    #[serde(default)]
    pub servers: Vec<Configured>,
    /// Why the file could not be read, if it could not. Never written back — the same rule the
    /// backend configuration follows, for the same reason.
    #[serde(default, skip_deserializing)]
    pub problem: Option<String>,
}

/// The shape that reaches `mcp.toml`. Separate so an IPC-only field cannot land in the file.
#[derive(Serialize)]
struct OnDisk<'a> {
    servers: &'a [Configured],
}

fn path(vault: &Path) -> PathBuf {
    vault.join("mcp.toml")
}

impl Servers {
    /// Read the configuration. Never fails.
    ///
    /// **Nothing ships configured.** An MCP server is a program on the user's machine that Epoch
    /// would start; shipping a default would mean spawning something nobody asked for the first
    /// time the app opened.
    pub fn load(vault: &Path) -> Self {
        let file = path(vault);
        let Ok(raw) = std::fs::read_to_string(&file) else {
            return Self::default();
        };
        toml::from_str::<Servers>(&raw).unwrap_or_else(|err| Self {
            servers: Vec::new(),
            problem: Some(format!(
                "{} could not be read ({err}). No outside tools are offered; your file is \
                 untouched.",
                file.display()
            )),
        })
    }

    /// Write it back. Refuses to save over a file it could not read.
    pub fn save(&self, vault: &Path) -> Result<(), String> {
        if let Some(problem) = &self.problem {
            return Err(format!(
                "not overwriting a file that could not be read: {problem}"
            ));
        }
        if let Some(reason) = self.problems().into_iter().next() {
            return Err(reason);
        }
        let raw = toml::to_string_pretty(&OnDisk {
            servers: &self.servers,
        })
        .map_err(|e| e.to_string())?;
        std::fs::create_dir_all(vault).map_err(|e| e.to_string())?;
        std::fs::write(path(vault), raw).map_err(|e| e.to_string())
    }

    pub fn problems(&self) -> Vec<String> {
        let mut found: Vec<String> = self.servers.iter().flat_map(Configured::problems).collect();
        let mut seen: Vec<&str> = Vec::new();
        for server in &self.servers {
            let id = server.id.trim();
            if seen.contains(&id) {
                found.push(format!("two servers are both called '{id}'"));
            }
            seen.push(id);
        }
        found
    }

    pub fn remember(&mut self, server: Configured) {
        match self.servers.iter_mut().find(|s| s.id == server.id) {
            Some(existing) => *existing = server,
            None => self.servers.push(server),
        }
    }

    /// Save an edit made by a surface, and say which credentials it left behind.
    ///
    /// `remember` replaces an entry wholesale, which is right for an install — everything is
    /// known at once. An edit is different, in exactly two ways.
    ///
    /// **Provenance survives.** `source` records the catalogue entry a server came from and is
    /// how the Workshop says *you already have this* about the entry rather than about a name.
    /// No surface is ever shown it, so `None` here means *unchanged*, never *from nowhere* — and
    /// a form that could silently erase it is how one server gets installed twice.
    ///
    /// **A dropped credential is returned, not left.** A stored value whose name is no longer on
    /// the server is one nobody can see, nobody can use and nobody will think to remove; the
    /// same rule uninstalling follows. The names come back rather than being deleted here,
    /// because this type knows nothing about the vault.
    ///
    /// Everything else — including the environment and the *names* of the credentials — comes
    /// from the surface, because the surface now holds it. That is only safe because `secrets`
    /// carries names and never values: a form can read, edit and send this entry back without a
    /// token ever having passed through it.
    ///
    /// Found by reading the boundary: the form could edit `id`, `command` and `args`, and sent
    /// exactly those. Everything else was `#[serde(default)]` on the way in, so pressing SAVE on
    /// a Workshop-installed server erased its environment, orphaned its token and forgot where
    /// it came from — and the first visible symptom arrived later, as a server that would not
    /// start for a reason that named none of this.
    pub fn amend(&mut self, mut server: Configured) -> Vec<String> {
        let mut orphaned = Vec::new();
        if let Some(existing) = self.servers.iter().find(|s| s.id == server.id) {
            if server.source.is_none() {
                server.source = existing.source.clone();
            }
            orphaned = existing
                .secrets
                .iter()
                .filter(|name| !server.secrets.contains(name))
                .cloned()
                .collect();
        }
        self.remember(server);
        orphaned
    }

    /// The servers as a **surface** sees them.
    ///
    /// A projection rather than the type itself, and the reason is a crash: `Configured` skips
    /// its empty fields when it serialises, which is right for `mcp.toml` — a file full of empty
    /// tables is worse to read — and wrong for a wire. On the wire a missing key and an empty
    /// list are different things, so a server with no environment arrived with no `env` at all
    /// and the panel's first `.map` threw. The contract said the field was always there. It was
    /// not, and nothing could have caught that: one type was answering to a file format and to a
    /// surface at once, and the two disagree about what "nothing" looks like.
    ///
    /// `source` is deliberately absent. No surface holds provenance — that is what makes
    /// `amend` able to treat its absence as *unchanged* rather than as *erased*.
    pub fn view(&self) -> View {
        View {
            servers: self
                .servers
                .iter()
                .map(|server| Listed {
                    id: server.id.clone(),
                    command: server.command.clone(),
                    args: server.args.clone(),
                    env: server.env.clone(),
                    secrets: server.secrets.clone(),
                    enabled: server.enabled,
                })
                .collect(),
            problem: self.problem.clone(),
        }
    }

    /// Remove a server, and say which credentials it was holding.
    ///
    /// The names are returned rather than deleted here, because this type knows nothing about
    /// the vault. The caller is expected to forget them: a credential left encrypted on disk
    /// after its server is gone is a secret nobody can see, nobody can use, and nobody will ever
    /// think to remove.
    pub fn forget(&mut self, id: &str) -> Option<Vec<String>> {
        let found = self.servers.iter().position(|s| s.id == id)?;
        Some(self.servers.remove(found).secrets)
    }
}

// ---------------------------------------------------------------------------
// The connection
// ---------------------------------------------------------------------------

/// A running server, and the line-oriented conversation with it.
struct Live {
    child: Child,
    stdin: ChildStdin,
    /// Every line the server wrote, delivered by a thread so a read can be bounded.
    lines: Receiver<String>,
    next: u64,
}

impl Drop for Live {
    /// Kill it rather than leave it. A server outliving the app is a process the user never
    /// started and cannot find.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One configured server, spawned on first use.
pub struct Server {
    id: String,
    command: String,
    args: Vec<String>,
    /// Everything the process is given, **already resolved** — plain values and decrypted
    /// credentials in one map. Resolved by the Bridge, which holds the vault, so this type never
    /// learns that a secret store exists and there is exactly one place a credential is read.
    env: BTreeMap<String, String>,
    live: Mutex<Option<Live>>,
}

impl std::fmt::Debug for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Server")
            .field("id", &self.id)
            .field("command", &self.command)
            .finish()
    }
}

impl Server {
    /// A server with no environment of its own. Everything configured before installs existed.
    pub fn new(configured: &Configured) -> Self {
        Self::with_env(configured, configured.env.clone())
    }

    /// A server whose environment has already been resolved, credentials included.
    pub fn with_env(configured: &Configured, env: BTreeMap<String, String>) -> Self {
        Self {
            id: configured.id.clone(),
            command: configured.command.clone(),
            args: configured.args.clone(),
            env,
            live: Mutex::new(None),
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// Ask one thing and wait for that answer.
    ///
    /// Spawns the process if it is not already running, and performs the handshake once. A
    /// failure leaves nothing running: a half-initialised server that answers some messages and
    /// not others is worse to debug than one that is plainly not there.
    fn ask(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
        let mut held = self
            .live
            .lock()
            .map_err(|_| "the server lock was poisoned".to_owned())?;
        if held.is_none() {
            *held = Some(self.start()?);
        }
        let live = held.as_mut().expect("just started");

        match speak(live, method, params) {
            Ok(value) => Ok(value),
            Err(err) => {
                // A dead or confused server is dropped rather than kept. The next call starts a
                // fresh one, which is the only recovery that does not depend on guessing what
                // state the old one was left in.
                *held = None;
                Err(err)
            }
        }
    }

    fn start(&self) -> Result<Live, String> {
        // No shell, ever. The user configured a program and its arguments; there is no string
        // for a shell to reinterpret, which is the same guarantee `run_command` makes.
        let mut child = Command::new(&self.command)
            .quiet()
            .args(&self.args)
            // Added to the inherited environment rather than replacing it: a runner needs PATH,
            // HOME and the rest to work at all, and clearing them would break every server on
            // the way to protecting none of them.
            .envs(&self.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // The server's own diagnostics are its business and would otherwise interleave with
            // the protocol. Discarded rather than parsed.
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| {
                format!(
                    "could not start '{}' ({err}). On Windows a Node launcher is `npx.cmd`, \
                     not `npx`.",
                    self.command
                )
            })?;

        let stdin = child.stdin.take().ok_or("the server has no stdin")?;
        let stdout = child.stdout.take().ok_or("the server has no stdout")?;

        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });

        let mut live = Live {
            child,
            stdin,
            lines,
            next: 0,
        };

        // The handshake, once per process. A server that will not agree on a protocol version is
        // one whose answers we would be guessing at.
        speak(
            &mut live,
            "initialize",
            serde_json::json!({
                "protocolVersion": PROTOCOL,
                "capabilities": {},
                "clientInfo": { "name": "Epoch", "version": env!("CARGO_PKG_VERSION") },
            }),
        )?;
        notify(&mut live, "notifications/initialized")?;

        Ok(live)
    }

    /// Everything this server offers, as capabilities.
    ///
    /// A tool whose parameters this build cannot model is **left out with a reason** rather than
    /// offered broken — a capability that fails on first use is worse than one that was never
    /// on the table (the same argument that gives a World with no project root an empty
    /// registry).
    pub fn tools(self: &std::sync::Arc<Self>) -> (Vec<Tool>, Vec<String>) {
        let (raw, mut skipped) = self.declarations();
        let mut offered = Vec::new();
        for entry in &raw {
            let Some(name) = entry["name"].as_str() else {
                continue;
            };
            match Tool::read(std::sync::Arc::clone(self), entry) {
                Ok(tool) => offered.push(tool),
                Err(why) => skipped.push(format!("'{}' offers '{name}', {why}", self.id)),
            }
        }
        (offered, skipped)
    }

    /// The server's own `tools/list` entries, untouched.
    ///
    /// Split out so the same answer can be both used now and written down — the remembered form
    /// is *this*, verbatim, which is what keeps one reader for both paths.
    fn declarations(self: &std::sync::Arc<Self>) -> (Vec<serde_json::Value>, Vec<String>) {
        let listed = match self.ask("tools/list", serde_json::json!({})) {
            Ok(value) => value,
            Err(err) => {
                return (
                    Vec::new(),
                    vec![format!("'{}' could not be asked: {err}", self.id)],
                )
            }
        };
        let raw = listed["tools"]
            .as_array()
            .map(|entries| entries.to_vec())
            .unwrap_or_default();
        (raw, Vec::new())
    }
}

/// Send one request and wait for the reply with the matching id.
///
/// Notifications and any other traffic are skipped rather than treated as the answer — a
/// server is allowed to talk while we are waiting, and reading the first line back would
/// eventually pair a reply with the wrong request.
fn speak(
    live: &mut Live,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    live.next += 1;
    let id = live.next;
    let message = serde_json::json!({
        "jsonrpc": "2.0", "id": id, "method": method, "params": params,
    });
    writeln!(live.stdin, "{message}").map_err(|e| format!("could not write to the server: {e}"))?;
    live.stdin
        .flush()
        .map_err(|e| format!("could not write to the server: {e}"))?;

    let deadline = std::time::Instant::now() + REPLY_TIMEOUT;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() {
            return Err(format!("the server did not answer '{method}' in time"));
        }
        let line = live
            .lines
            .recv_timeout(left)
            .map_err(|_| format!("the server stopped answering during '{method}'"))?;

        let Ok(frame) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if frame["id"].as_u64() != Some(id) {
            continue;
        }
        if let Some(error) = frame.get("error") {
            return Err(error["message"].as_str().unwrap_or("no detail").to_owned());
        }
        return Ok(frame
            .get("result")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})));
    }
}

/// Tell it something without expecting an answer.
fn notify(live: &mut Live, method: &str) -> Result<(), String> {
    let message = serde_json::json!({ "jsonrpc": "2.0", "method": method, "params": {} });
    writeln!(live.stdin, "{message}").map_err(|e| format!("could not write to the server: {e}"))?;
    live.stdin
        .flush()
        .map_err(|e| format!("could not write to the server: {e}"))
}

// ---------------------------------------------------------------------------
// One outside tool, as a capability
// ---------------------------------------------------------------------------

/// What each server said it offers, the last time it was asked.
///
/// ## Why this is remembered at all
///
/// Asking costs a process. Every restart re-spawned every configured server just to learn a
/// list that had not changed — so opening Epoch got slower the more tools you configured, and
/// the tools were *missing* until each one had answered. A capability that appears a few seconds
/// after the World does is one nobody can rely on being there.
///
/// ## It stores the server's own answer, verbatim
///
/// Not a parsed summary. The raw `tools/list` entries go in and come back out through the
/// **same** [`Tool::read`] the live path uses, so there is exactly one thing that understands
/// what a tool declaration means. A second parser for the remembered form would eventually
/// disagree with the first, and the symptom would be a tool that works when freshly asked and
/// breaks after a restart.
///
/// ## It is a memory, not a source of truth
///
/// A server is still what decides what it offers. This says *what it said last time*, which is
/// the right answer to "what can this World do" at a moment when nothing is running — and it is
/// replaced wholesale the next time anybody asks. Refreshing is one call away, and a tool that
/// has since disappeared fails at the moment it is called, which is what a stale in-memory list
/// already did.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Remembered {
    /// Raw `tools/list` entries, by server id.
    #[serde(default)]
    pub offered: std::collections::BTreeMap<String, Vec<serde_json::Value>>,
}

impl Remembered {
    fn path(vault: &Path) -> std::path::PathBuf {
        vault.join("mcp-tools.json")
    }

    /// Read what was remembered. Unreadable is empty — never an error.
    ///
    /// A cache that could refuse to load would be a way for a file nobody edits by hand to stop
    /// Epoch from opening. The cost of it being empty is one round of asking.
    pub fn load(vault: &Path) -> Self {
        std::fs::read_to_string(Self::path(vault))
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default()
    }

    /// Write it. Failure is reported, never fatal: the tools still work this session.
    pub fn save(&self, vault: &Path) -> Result<(), String> {
        let body = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(Self::path(vault), body).map_err(|e| e.to_string())
    }

    /// Every capability id that belonged to one server: the group, and each tool it last offered.
    ///
    /// Read from what the server itself said, through the same `namespaced` the live path uses,
    /// rather than by matching the prefix `server_`. A prefix is nearly right and wrong in a way
    /// nobody would find: a World with `spotify` and `spotify_beta` configured has one whose
    /// every tool id begins with the other's name, and removing the first would take the
    /// second's grants with it.
    ///
    /// The remembered list is the honest source because it is the only thing a user could have
    /// been shown — a tool id nobody ever saw is one nobody could have ticked.
    pub fn capabilities_of(&self, server: &str) -> Vec<String> {
        let mut found = vec![format!("mcp:{server}")];
        found.extend(
            self.offered
                .get(server)
                .into_iter()
                .flatten()
                .filter_map(|raw| raw["name"].as_str())
                .map(|tool| namespaced(server, tool)),
        );
        found
    }

    /// Drop what one server said, because that server is gone.
    ///
    /// Removing a server left its remembered tool list behind, and a memory is not harmless
    /// residue: reinstalling the same server under the same id would replay a tool list nobody
    /// had asked for since, from a version that may no longer exist. Removing means removing.
    pub fn forget(vault: &Path, server: &str) -> Result<(), String> {
        let mut held = Self::load(vault);
        if held.offered.remove(server).is_none() {
            return Ok(());
        }
        held.save(vault)
    }
}

/// Every configured server, and what they offer.
///
/// **Long-lived on purpose.** A registry is rebuilt every turn; a server is a process. Building
/// the bridge once and asking it for tools each turn is the difference between one process per
/// server and one process per turn.
///
/// Listing is cached for the same reason. A server may announce that its tools changed
/// (`notifications/tools/list_changed`) and this build does not yet listen for it — said here
/// rather than left as a silent limitation: a server whose tool set changes mid-session needs
/// Epoch reopened.
#[derive(Debug, Default)]
pub struct Bridge {
    servers: Vec<std::sync::Arc<Server>>,
    /// What each server offered, asked once. `None` until asked at all.
    listed: Mutex<Option<(Vec<Tool>, Vec<String>)>>,
    /// Why the configuration could not be read, and which servers were left out before anything
    /// was ever started.
    problem: Vec<String>,
    /// Every id in the configuration, including the ones that were never prepared.
    ///
    /// A server can be switched off, or missing the credential it was installed with, or refused
    /// for a malformed name — none of which reach [`Self::servers`]. They still exist, and a
    /// character that was given one still asks for it, so they still get a box.
    configured: Vec<String>,
    /// What they said last time Epoch ran, so a restart does not have to ask.
    remembered: Remembered,
    /// Where to write an updated memory. Kept so `refresh` needs no argument it could be
    /// given wrongly.
    vault: std::path::PathBuf,
}

impl Bridge {
    /// Read the configuration and prepare the servers. **Spawns nothing.**
    pub fn load(vault: &Path) -> Self {
        let configured = Servers::load(vault);
        let store = crate::secrets::Secrets::at(vault);
        let mut problem: Vec<String> = configured.problem.into_iter().collect();
        let mut servers = Vec::new();

        for entry in configured
            .servers
            .iter()
            .filter(|s| s.enabled && s.problems().is_empty())
        {
            let mut env = entry.env.clone();
            let mut missing = Vec::new();
            for variable in &entry.secrets {
                match store.get(&epoch_kernel::SecretName::for_mcp_input(
                    &entry.id, variable,
                )) {
                    Some(secret) => {
                        env.insert(variable.clone(), secret.expose().to_owned());
                    }
                    None => missing.push(variable.clone()),
                }
            }
            // Not started at all, rather than started incomplete. A server missing the credential
            // it was installed with fails at its first tool call, in its own words, somewhere far
            // from the screen where the credential is entered — which reads as Epoch being
            // broken. Said here instead, where the servers are listed.
            if !missing.is_empty() {
                problem.push(format!(
                    "'{}' is not running: {} {} not in the secret store. Re-enter it in the \
                     Workshop.",
                    entry.id,
                    missing.join(", "),
                    if missing.len() == 1 { "is" } else { "are" }
                ));
                continue;
            }
            servers.push(std::sync::Arc::new(Server::with_env(entry, env)));
        }

        Self {
            servers,
            configured: configured.servers.iter().map(|s| s.id.clone()).collect(),
            listed: Mutex::new(None),
            problem,
            remembered: Remembered::load(vault),
            vault: vault.to_path_buf(),
        }
    }

    /// Whether any server is configured at all. A World with none costs nothing.
    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    /// Everything the configured servers offer, and everything that could not be offered.
    ///
    /// The first call starts the processes; later calls are free. Problems are returned rather
    /// than logged, because a tool that is missing needs to say why on the screen where somebody
    /// is wondering where it went.
    pub fn offer(&self) -> (Vec<Tool>, Vec<String>) {
        let mut held = guard(&self.listed);
        if held.is_none() {
            let mut tools = Vec::new();
            let mut problems: Vec<String> = self.problem.to_vec();
            for server in &self.servers {
                // What it said last time, replayed through the same reader the live path uses.
                // A server Epoch has met before costs nothing to open.
                match self.remembered.offered.get(server.id()) {
                    Some(raw) => {
                        for entry in raw {
                            match Tool::read(std::sync::Arc::clone(server), entry) {
                                Ok(tool) => tools.push(tool),
                                // A remembered entry that no longer reads is reported like a
                                // live one. Silently dropping it would make a tool vanish with
                                // no way to find out why.
                                Err(why) => problems.push(format!(
                                    "'{}' remembers a tool that no longer reads: {why}",
                                    server.id()
                                )),
                            }
                        }
                    }
                    // Never met. This is the only path that starts a process, and it happens
                    // once per server ever rather than once per launch.
                    None => {
                        let (offered, skipped) = server.tools();
                        tools.extend(offered);
                        problems.extend(skipped);
                    }
                }
            }
            *held = Some((tools, problems));
        }
        held.clone().expect("just filled")
    }

    /// Each connected server as one thing a character can be given.
    ///
    /// A character asks for `mcp:playwright`, not for twenty-four ids that were true on the day
    /// somebody ticked a box. Which tools that turns out to mean is answered here, live, every
    /// time it is asked — so a tool the server adds tomorrow is already included and a tool it
    /// drops simply stops appearing.
    ///
    /// Built from the same [`offer`](Self::offer) every turn uses, so what the editor shows and
    /// what a turn resolves cannot disagree. Servers with nothing to offer are omitted: an empty
    /// box that grants nothing is not a choice.
    pub fn groups(&self) -> Vec<crate::capabilities::Group> {
        let mut by_server: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for tool in self.offer().0 {
            by_server
                .entry(tool.server.id().to_owned())
                .or_default()
                .push(tool.descriptor.id.to_string());
        }
        // Every **configured** server, not every server that answered. A silent one keeps its
        // box and says it is silent; dropping it would take a character's request off the screen
        // and let the next save delete it. See `Group::answering`.
        let mut groups: Vec<crate::capabilities::Group> = self
            .configured
            .iter()
            .map(|id| {
                let mut tools = by_server.get(id).cloned().unwrap_or_default();
                tools.sort();
                crate::capabilities::Group {
                    label: readable(id),
                    id: format!("mcp:{id}"),
                    answering: by_server.contains_key(id.as_str()),
                    tools,
                }
            })
            .collect();
        groups.sort_by(|a, b| a.id.cmp(&b.id));
        groups
    }

    /// Ask every server again, and remember what they say.
    ///
    /// The explicit way to pick up a server whose tools changed — which this build cannot
    /// notice on its own, because it does not yet listen for
    /// `notifications/tools/list_changed`. Said plainly rather than left as a silent
    /// limitation: refreshing is a button, not a restart.
    pub fn refresh(&self) -> (Vec<Tool>, Vec<String>) {
        let mut tools = Vec::new();
        let mut problems: Vec<String> = self.problem.to_vec();
        let mut fresh = Remembered::default();

        for server in &self.servers {
            let (raw, skipped) = server.declarations();
            problems.extend(skipped);
            for entry in &raw {
                match Tool::read(std::sync::Arc::clone(server), entry) {
                    Ok(tool) => tools.push(tool),
                    Err(why) => problems.push(format!(
                        "'{}' offers a tool Epoch cannot use: {why}",
                        server.id()
                    )),
                }
            }
            // Only servers that actually answered. A server that was unreachable keeps whatever
            // was remembered rather than being recorded as offering nothing — losing a tool
            // list because a laptop was offline would be worse than a stale one.
            if !raw.is_empty() {
                fresh.offered.insert(server.id().to_owned(), raw);
            } else if let Some(kept) = self.remembered.offered.get(server.id()) {
                fresh.offered.insert(server.id().to_owned(), kept.clone());
            }
        }

        if let Err(err) = fresh.save(&self.vault) {
            problems.push(format!("the tool list could not be remembered: {err}"));
        }
        *guard(&self.listed) = Some((tools.clone(), problems.clone()));
        (tools, problems)
    }
}

/// A tool an MCP server offers, wearing Epoch's contract.
#[derive(Clone)]
pub struct Tool {
    server: std::sync::Arc<Server>,
    /// What the server calls it. Sent back on every call, unchanged.
    remote: String,
    descriptor: Descriptor,
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tool")
            .field("id", &self.descriptor.id)
            .finish()
    }
}

impl Tool {
    fn read(server: std::sync::Arc<Server>, raw: &serde_json::Value) -> Result<Self, String> {
        let remote = raw["name"].as_str().ok_or("which has no name")?.to_owned();
        let id = CapabilityId::new(&namespaced(server.id(), &remote))
            .map_err(|why| format!("whose name cannot be a capability id: {why}"))?;

        let summary = raw["description"]
            .as_str()
            .filter(|d| !d.trim().is_empty())
            .map(str::to_owned)
            // A model chooses a tool almost entirely from its description, so an undescribed one
            // is said to be undescribed rather than given an invented purpose.
            .unwrap_or_else(|| format!("'{remote}', which its server did not describe"));

        let parameters = parameters(&raw["inputSchema"])?;

        Ok(Self {
            server,
            remote,
            // Every effect, and permanent. See the module docs: MCP declares no effects, and its
            // hints are a third party grading its own risk — the one claim ADR-0008 says must
            // not be expressible.
            descriptor: Descriptor::acting(
                id,
                summary,
                BTreeSet::from(Effect::ALL),
                Reversal::Permanent,
            )
            .taking(parameters),
        })
    }
}

/// A server id as a person would write it: `my_browser` reads "My Browser".
///
/// Derived rather than authored, because a configured server has no display name and inventing a
/// field for one would make every existing configuration incomplete. If a server ever wants to
/// name itself, that is a thing it declares — not a thing Epoch guesses better.
fn readable(id: &str) -> String {
    id.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `server_tool`, so two servers offering `search` are two capabilities.
fn namespaced(server: &str, tool: &str) -> String {
    let tidy: String = tool
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    format!("{server}_{tidy}")
}

/// The parameters this build can actually carry.
///
/// Refuses the whole tool when a **required** parameter has a shape Epoch cannot model. Arrays
/// and objects are not in [`ValueKind`], and sending a tool without a parameter it requires would
/// fail on first use — which is exactly the thing this returns an error to prevent.
///
/// An *optional* one of an unknown shape is dropped instead: the tool still works without it.
fn parameters(schema: &serde_json::Value) -> Result<Vec<Parameter>, String> {
    let required: BTreeSet<&str> = schema["required"]
        .as_array()
        .map(|all| all.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let mut parameters = Vec::new();
    for (name, described) in schema["properties"].as_object().into_iter().flatten() {
        let needed = required.contains(name.as_str());
        let kind = match described["type"].as_str() {
            Some("string") => ValueKind::Text,
            Some("integer") => ValueKind::Integer,
            Some("number") => ValueKind::Number,
            Some("boolean") => ValueKind::Boolean,
            other => {
                if needed {
                    return Err(format!(
                        "which requires '{name}' as {} — a shape Epoch cannot carry yet",
                        other.unwrap_or("an unnamed type")
                    ));
                }
                continue;
            }
        };
        let description = described["description"]
            .as_str()
            .unwrap_or("no description from the server")
            .to_owned();
        parameters.push(Parameter {
            name: name.clone(),
            kind,
            description,
            required: needed,
        });
    }
    Ok(parameters)
}

impl Capability for Tool {
    fn describe(&self) -> Descriptor {
        self.descriptor.clone()
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }

        let mut sent = serde_json::Map::new();
        for parameter in &self.descriptor.parameters {
            let Some(value) = arguments.get(&parameter.name) else {
                continue;
            };
            sent.insert(
                parameter.name.clone(),
                match value {
                    Value::Text(v) => serde_json::Value::from(v.clone()),
                    Value::Integer(v) => serde_json::Value::from(*v),
                    Value::Number(v) => serde_json::Value::from(*v),
                    Value::Boolean(v) => serde_json::Value::from(*v),
                },
            );
        }

        let answered = self
            .server
            .ask(
                "tools/call",
                serde_json::json!({ "name": self.remote, "arguments": sent }),
            )
            .map_err(CapabilityError::Failed)?;

        // A server reports a tool's own failure inside a successful reply. Surfaced as a failure
        // rather than as content, or the model would read an error message as a result.
        if answered["isError"].as_bool() == Some(true) {
            return Err(CapabilityError::Failed(text_of(&answered)));
        }

        // Told, never made. Whatever an outside tool changed, Epoch did not see it happen and
        // holds no artifact — and evidence is what a run left behind, not what it reported
        // (ADR-0025). Claiming otherwise would put a stranger's sentence into a Quest's History
        // as proof.
        Ok(Outcome::told(text_of(&answered)))
    }
}

/// Everything readable in a tool's reply, joined.
///
/// Only text blocks. Images and embedded resources are named rather than carried: a capability
/// returns a string, and inventing a description of a picture nobody looked at would be worse
/// than saying a picture came back.
fn text_of(answered: &serde_json::Value) -> String {
    let mut said = String::new();
    for block in answered["content"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let piece = match block["type"].as_str() {
            Some("text") => block["text"].as_str().unwrap_or_default().to_owned(),
            Some(other) => format!("[{other} returned, which Epoch cannot yet carry]"),
            None => continue,
        };
        if piece.is_empty() {
            continue;
        }
        if !said.is_empty() {
            said.push('\n');
        }
        said.push_str(&piece);
    }
    if said.is_empty() {
        said.push_str("the tool returned nothing");
        return said;
    }

    if said.len() > MAX_TEXT {
        // On a character boundary, so the cut cannot produce invalid text.
        let mut end = MAX_TEXT;
        while end > 0 && !said.is_char_boundary(end) {
            end -= 1;
        }
        let cut = said.len() - end;
        said.truncate(end);
        // Said in the result itself, because the model is the reader who needs to know. A
        // silent truncation reads as a tool that answers strangely; a stated one reads as a
        // tool that answered and there is more.
        said.push_str(&format!(
            "

[cut here — {cut} more bytes this build did not carry back. Ask for a narrower part of the page rather than repeating this call.]"
        ));
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A vault holding a configuration and a remembered tool list. `offer` replays the memory,
    /// so nothing is spawned and the test drives the same reader the live path does.
    fn vault_with(servers: &str, remembered: serde_json::Value) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "epoch-mcp-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("mcp.toml"), servers).unwrap();
        std::fs::write(
            dir.join("mcp-tools.json"),
            serde_json::to_string(&serde_json::json!({ "offered": remembered })).unwrap(),
        )
        .unwrap();
        dir
    }

    fn declares(name: &str) -> serde_json::Value {
        serde_json::json!({
            "name": name,
            "description": "does a thing",
            "inputSchema": { "type": "object", "properties": {} },
        })
    }

    #[test]
    fn a_server_is_one_thing_a_character_can_be_given() {
        // The wall this replaced: Playwright alone put two dozen tick-boxes on the screen, and
        // ticking them froze that day's list into the character's file forever.
        let vault = vault_with(
            "[[servers]]\nid = 'playwright'\ncommand = 'true'\n\n\
             [[servers]]\nid = 'my_notes'\ncommand = 'true'\n",
            serde_json::json!({
                "playwright": [declares("browser_click"), declares("browser_close")],
                "my_notes": [declares("search")],
            }),
        );

        let groups = Bridge::load(&vault).groups();

        assert_eq!(groups.len(), 2, "one entry per server, never one per tool");
        assert_eq!(groups[0].id, "mcp:my_notes");
        assert_eq!(groups[0].label, "My Notes");
        assert_eq!(groups[1].id, "mcp:playwright");
        assert_eq!(groups[1].label, "Playwright");
        // Membership is stated by the source. Nothing infers it from the id, which is why a
        // server free to name its tools anything cannot mislead the grouping.
        assert_eq!(
            groups[1].tools,
            vec!["playwright_browser_click", "playwright_browser_close"]
        );
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn a_server_missing_its_credential_does_not_start_half_configured() {
        // Started without it, the server fails at its first tool call in its own words, far from
        // the screen where the credential is entered — which reads as Epoch being broken. Said
        // here instead, where the servers are listed.
        let vault = vault_with(
            "[[servers]]\nid = 'notion'\ncommand = 'true'\nsecrets = ['NOTION_TOKEN']\n",
            serde_json::json!({ "notion": [declares("search")] }),
        );

        let bridge = Bridge::load(&vault);
        let (tools, problems) = bridge.offer();

        assert!(
            tools.is_empty(),
            "nothing is offered by a server that is not running"
        );
        assert!(
            problems.iter().any(|p| p.contains("NOTION_TOKEN")),
            "the reason names the variable: {problems:?}"
        );
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn plain_configuration_reaches_the_process_and_a_secret_is_only_a_name_in_the_file() {
        let vault = vault_with(
            "[[servers]]\nid = 'notes'\ncommand = 'true'\n\n\
             [servers.env]\nROOT = '/tmp/notes'\n",
            serde_json::json!({ "notes": [declares("search")] }),
        );

        let written = Servers::load(&vault);
        assert_eq!(
            written.servers[0].env.get("ROOT").map(String::as_str),
            Some("/tmp/notes")
        );
        // And it round-trips: an older Epoch rewriting this file must not drop what it holds.
        written.save(&vault).unwrap();
        let again = Servers::load(&vault);
        assert_eq!(again.servers[0].env, written.servers[0].env);
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn an_edit_keeps_the_provenance_no_surface_was_ever_shown() {
        // **This assertion is narrower than the one first written here, and the first one was
        // wrong.** It claimed an edit could not drop a credential the form "was never shown" —
        // by sending no `secrets` at all, which is what the old form did. But the fix to that
        // defect was to put the names *into the contract*, so the form now always sends them,
        // and an empty list is a real instruction: the user removed the last one. Preserving it
        // would have made "remove the only credential" impossible and blamed the file.
        //
        // What genuinely cannot be decided by a surface is what no surface holds. That is
        // `source` alone — the catalogue entry this came from, which is how the Workshop says
        // *you already have this* about the entry rather than about a name. Erasing it is how
        // one server gets installed twice.
        let mut servers = Servers::default();
        servers.remember(Configured {
            id: "spotify".into(),
            command: "npx.cmd".into(),
            args: vec!["-y".into(), "@xavifabregat/spotify-mcp".into()],
            env: [("SPOTIFY_MARKET".to_owned(), "CR".to_owned())]
                .into_iter()
                .collect(),
            secrets: vec!["SPOTIFY_CLIENT_ID".into()],
            source: Some("Spotify".into()),
            enabled: true,
        });

        // What the form sends: everything it holds, and provenance is not among it.
        let orphaned = servers.amend(Configured {
            id: "spotify".into(),
            command: "npx.cmd".into(),
            args: vec!["-y".into(), "@xavifabregat/spotify-mcp@1.2".into()],
            env: [("SPOTIFY_MARKET".to_owned(), "CR".to_owned())]
                .into_iter()
                .collect(),
            secrets: vec!["SPOTIFY_CLIENT_ID".into()],
            ..Default::default()
        });

        let kept = &servers.servers[0];
        assert_eq!(
            kept.args[1], "@xavifabregat/spotify-mcp@1.2",
            "the edit lands"
        );
        assert_eq!(
            kept.source.as_deref(),
            Some("Spotify"),
            "provenance is not the surface's to erase"
        );
        assert!(orphaned.is_empty(), "nothing was dropped: {orphaned:?}");
        assert_eq!(kept.secrets, vec!["SPOTIFY_CLIENT_ID".to_owned()]);
        assert_eq!(
            kept.env.get("SPOTIFY_MARKET").map(String::as_str),
            Some("CR")
        );
    }

    #[test]
    fn dropping_a_credential_by_name_reports_it_so_no_value_is_left_behind() {
        // The other direction, and it must stay distinguishable from the one above: a name the
        // surface *does* hold and deliberately removed. Returned rather than deleted here —
        // this type knows nothing about the vault — and the caller is expected to forget it, or
        // the value outlives every reference to it and nobody will think to look for it again.
        let mut servers = Servers::default();
        servers.remember(Configured {
            id: "notion".into(),
            command: "npx.cmd".into(),
            secrets: vec!["NOTION_TOKEN".into(), "NOTION_ROOT".into()],
            ..Default::default()
        });

        let orphaned = servers.amend(Configured {
            id: "notion".into(),
            command: "npx.cmd".into(),
            secrets: vec!["NOTION_ROOT".into()],
            ..Default::default()
        });

        assert_eq!(orphaned, vec!["NOTION_TOKEN".to_owned()]);
        assert_eq!(servers.servers[0].secrets, vec!["NOTION_ROOT".to_owned()]);
    }

    #[test]
    fn what_belongs_to_a_server_is_what_it_said_and_never_a_name_that_starts_the_same() {
        // Removing a server has to take its grants with it, which means knowing which
        // capability ids were its. The obvious way is the prefix `spotify_` — and it is wrong
        // in a way nobody would find: a World with `spotify` and `spotify_beta` configured has
        // one whose every tool id begins with the other's name, so forgetting the first would
        // silently revoke the second's.
        //
        // The remembered list is the honest source, because it is the only thing a user could
        // have been shown: a tool id nobody ever saw is one nobody could have ticked.
        let mut held = Remembered::default();
        held.offered.insert(
            "spotify".into(),
            vec![
                serde_json::json!({ "name": "login" }),
                serde_json::json!({ "name": "status" }),
            ],
        );
        held.offered.insert(
            "spotify_beta".into(),
            vec![serde_json::json!({ "name": "login" })],
        );

        let doomed = held.capabilities_of("spotify");

        assert_eq!(
            doomed,
            vec![
                "mcp:spotify".to_owned(),
                "spotify_login".to_owned(),
                "spotify_status".to_owned()
            ]
        );
        assert!(
            !doomed.contains(&"spotify_beta_login".to_owned()),
            "the neighbour keeps everything: {doomed:?}"
        );
    }

    #[test]
    fn amending_a_server_nobody_configured_simply_adds_it() {
        let mut servers = Servers::default();
        let orphaned = servers.amend(Configured {
            id: "fresh".into(),
            command: "true".into(),
            ..Default::default()
        });
        assert!(orphaned.is_empty());
        assert_eq!(servers.servers.len(), 1);
    }

    #[test]
    fn a_configured_server_keeps_its_box_even_when_it_answers_nothing() {
        // **This replaces the opposite assertion.** The first version dropped a silent server,
        // reasoning that a box granting nothing is not a choice. Using it proved that wrong:
        // a server that stopped starting — one malformed argument was enough — took its
        // tick-box off the character editor, so somebody who *had* been given it appeared not
        // to have it, and the next save wrote that appearance to disk. The request was then
        // really gone, and nothing had ever said so.
        //
        // A cold box is information. A missing one is a silent edit to somebody's character.
        let vault = vault_with(
            "[[servers]]\nid = 'silent'\ncommand = 'true'\n",
            serde_json::json!({ "silent": [] }),
        );

        let groups = Bridge::load(&vault).groups();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, "mcp:silent");
        assert!(groups[0].tools.is_empty());
        assert!(!groups[0].answering, "and it says it is not answering");
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn a_server_switched_off_or_missing_its_credential_still_has_a_box() {
        // Neither reaches the list of prepared servers, and both are things somebody's
        // character may already be asking for. Same rule, same reason.
        let vault = vault_with(
            "[[servers]]\nid = 'resting'\ncommand = 'true'\nenabled = false\n\n\
             [[servers]]\nid = 'locked'\ncommand = 'true'\nsecrets = ['TOKEN']\n",
            serde_json::json!({}),
        );

        let ids: Vec<String> = Bridge::load(&vault)
            .groups()
            .into_iter()
            .map(|g| g.id)
            .collect();
        assert_eq!(ids, vec!["mcp:locked", "mcp:resting"]);
        let _ = std::fs::remove_dir_all(&vault);
    }

    #[test]
    fn an_outside_tool_is_treated_as_the_ceiling_whatever_it_hints() {
        // The whole point. MCP declares no effects and its hints are a third party grading its
        // own risk — the one claim ADR-0008 says must not be expressible. Believing
        // `readOnlyHint` would let a tool that deletes through a mode set to ask first.
        let raw = serde_json::json!({
            "name": "delete_everything",
            "description": "totally safe",
            "annotations": { "readOnlyHint": true, "destructiveHint": false },
            "inputSchema": { "type": "object", "properties": {} },
        });
        let server = std::sync::Arc::new(Server::new(&Configured {
            id: "probe".into(),
            command: "true".into(),
            args: Vec::new(),
            enabled: true,
            ..Default::default()
        }));
        let tool = Tool::read(server, &raw).unwrap();
        let descriptor = tool.describe();

        for effect in Effect::ALL {
            assert!(
                descriptor.effects.contains(&effect),
                "{effect:?} must be assumed"
            );
        }
        assert!(descriptor.reversal.is_permanent());
        assert_eq!(descriptor.id.as_str(), "probe_delete_everything");
    }

    #[test]
    fn a_tool_requiring_a_shape_epoch_cannot_carry_is_left_out_rather_than_offered_broken() {
        // A capability that fails on first use is worse than one that was never on the table.
        let refused = parameters(&serde_json::json!({
            "type": "object",
            "properties": { "files": { "type": "array" } },
            "required": ["files"],
        }));
        assert!(refused.unwrap_err().contains("array"));

        // The same shape, optional, costs only itself.
        let kept = parameters(&serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "where" },
                "extras": { "type": "array" },
            },
            "required": ["path"],
        }))
        .unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].name, "path");
        assert!(kept[0].required);
    }

    #[test]
    fn a_tool_nobody_described_says_so_rather_than_being_given_a_purpose() {
        // A model chooses a tool almost entirely from its description, so an invented one is a
        // sentence Epoch put in a stranger's mouth.
        let raw = serde_json::json!({
            "name": "x",
            "inputSchema": { "type": "object", "properties": {} },
        });
        let server = std::sync::Arc::new(Server::new(&Configured {
            id: "probe".into(),
            command: "true".into(),
            args: Vec::new(),
            enabled: true,
            ..Default::default()
        }));
        let tool = Tool::read(server, &raw).unwrap();
        assert!(tool.describe().summary.contains("did not describe"));
    }

    #[test]
    fn a_name_that_cannot_be_a_capability_id_is_made_into_one() {
        // Server names are the user's; tool names are a stranger's. Both end up in an id that
        // must survive being shared.
        assert_eq!(
            namespaced("play", "browser.take-Screenshot"),
            "play_browser_take_screenshot"
        );
    }

    #[test]
    fn an_enormous_reply_is_cut_and_says_so_rather_than_being_dropped_later() {
        // The failure this prevents: a page snapshot too large for a small model's context is
        // dropped by the Composer, the model never sees its own result, and it calls the tool
        // again — eight identical "successful" calls and a turn that ends with nothing.
        let huge = "x".repeat(MAX_TEXT * 2);
        let said = text_of(&serde_json::json!({
            "content": [{ "type": "text", "text": huge }],
        }));

        assert!(said.len() < MAX_TEXT + 512, "it was cut");
        assert!(
            said.contains("cut here"),
            "and the model is told, or it will just try again"
        );
        assert!(
            said.contains("narrower"),
            "with something better to do than repeat itself"
        );
    }

    #[test]
    fn a_reply_carrying_something_unreadable_names_it_rather_than_inventing_it() {
        let said = text_of(&serde_json::json!({
            "content": [
                { "type": "text", "text": "here it is" },
                { "type": "image", "data": "…" },
            ],
        }));
        assert!(said.contains("here it is"));
        assert!(said.contains("image returned"));
    }

    #[test]
    fn a_server_that_will_not_start_says_which_program_and_what_to_check() {
        // "program not found" for a program the user can plainly see installed is the failure
        // this message exists to pre-empt.
        let server = Server::new(&Configured {
            id: "missing".into(),
            command: "epoch-no-such-program".into(),
            args: Vec::new(),
            enabled: true,
            ..Default::default()
        });
        let refused = server.ask("tools/list", serde_json::json!({})).unwrap_err();
        assert!(refused.contains("epoch-no-such-program"));
        assert!(refused.contains("npx.cmd"), "the Windows trap is named");
    }

    #[test]
    fn nothing_ships_configured() {
        // An MCP server is a program Epoch would start. Shipping a default would spawn something
        // nobody asked for the first time the app opened.
        let nowhere = std::env::temp_dir().join("epoch-mcp-unconfigured");
        let _ = std::fs::remove_dir_all(&nowhere);
        let servers = Servers::load(&nowhere);
        assert!(servers.servers.is_empty());
        assert!(servers.problem.is_none());
    }

    #[test]
    fn two_servers_may_not_answer_to_one_name() {
        // Their names namespace the tools, so an ambiguous one is an ambiguous capability id.
        let one = Configured {
            id: "same".into(),
            command: "a".into(),
            args: Vec::new(),
            enabled: true,
            ..Default::default()
        };
        let mut servers = Servers::default();
        servers.servers.push(one.clone());
        servers.servers.push(one);
        assert!(servers.problems().iter().any(|p| p.contains("both called")));
    }

    #[test]
    fn a_server_name_obeys_the_capability_ids_rules_rather_than_a_looser_set() {
        // Checked here, where the user can see it, rather than failing later at registration.
        let bad = Configured {
            id: "My Server".into(),
            command: "a".into(),
            args: Vec::new(),
            enabled: true,
            ..Default::default()
        };
        assert!(bad.problems().iter().any(|p| p.contains("cannot contain")));
    }
}

#[cfg(test)]
mod memory_tests {
    use super::*;

    fn dir(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("epoch-mcp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn nothing_remembered_is_empty_rather_than_an_error() {
        // A cache that could refuse to load would be a way for a file nobody edits by hand to
        // stop Epoch from opening. The cost of it being empty is one round of asking.
        let d = dir("missing");
        assert!(Remembered::load(&d).offered.is_empty());

        std::fs::write(Remembered::path(&d), "this is not json {{{").unwrap();
        assert!(Remembered::load(&d).offered.is_empty());

        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn what_a_server_said_survives_the_file_verbatim() {
        // Stored raw, so it comes back out through the same `Tool::read` the live path uses.
        // A second parser for the remembered form would eventually disagree with the first, and
        // the symptom would be a tool that works when freshly asked and breaks after a restart.
        let d = dir("roundtrip");
        let declaration = serde_json::json!({
            "name": "search",
            "description": "look something up",
            "inputSchema": { "type": "object", "properties": { "q": { "type": "string" } } },
        });

        let mut memory = Remembered::default();
        memory
            .offered
            .insert("books".into(), vec![declaration.clone()]);
        memory.save(&d).unwrap();

        let read = Remembered::load(&d);
        assert_eq!(read.offered["books"], vec![declaration]);

        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_bridge_with_a_memory_offers_without_starting_anything() {
        // The whole point: opening Epoch does not cost one process per configured server, and
        // the tools are there before anybody looks rather than a few seconds afterwards.
        let d = dir("offer");
        std::fs::write(
            d.join("mcp.toml"),
            "[[servers]]
id = \"books\"
command = \"definitely-not-a-real-program\"
",
        )
        .unwrap();

        let mut memory = Remembered::default();
        memory.offered.insert(
            "books".into(),
            vec![serde_json::json!({
                "name": "search",
                "description": "look something up",
                "inputSchema": { "type": "object", "properties": {} },
            })],
        );
        memory.save(&d).unwrap();

        let bridge = Bridge::load(&d);
        let (tools, problems) = bridge.offer();

        // The command does not exist, so anything that had to ask would have failed here.
        assert_eq!(tools.len(), 1, "remembered, not asked: {problems:?}");
        assert_eq!(tools[0].describe().id.as_str(), "books_search");

        let _ = std::fs::remove_dir_all(&d);
    }
}
