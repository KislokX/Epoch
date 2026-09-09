//! Claude Code, hosted as a character's brain (step 6.2, ADR-0027).
//!
//! ## The desktop app is not the thing we can drive
//!
//! Claude Code Desktop is a window. It has no automation surface, and pretending otherwise
//! would mean synthesising keystrokes into somebody else's application — which is not a
//! contract, it is a trick that breaks on the next release.
//!
//! What *is* drivable is the CLI in print mode: `claude -p "…"`, which runs the whole agent
//! loop and prints what happened. Same subscription rather than an API bill — the commercial
//! fact that made this the reachable path at all (a ChatGPT plan has no API; a Claude plan has
//! a signed-in CLI).
//!
//! **The CLI holds its own sign-in.** This was written as "same sign-in as the desktop app",
//! and a machine with the desktop app open answered *"Not logged in · Please run /login"* —
//! the login lives with the CLI, and Epoch cannot perform it on the user's behalf. So it must
//! be *reported*: a character who cannot work says why, and says it as a failure rather than as
//! something she said.
//!
//! ## Detection is measured, not assumed
//!
//! [`Self::probe`] runs the program and reads its version. It does not look for a file and
//! conclude. A path that exists is not a program that runs — the wrong architecture, a broken
//! install and a shim that forwards to nothing all pass a file check and fail a turn.
//!
//! That matters more here than for a Provider: a missing endpoint is a network error somebody
//! expects, while a missing agent would surface as a character who simply never answers.
//!
//! ## The invocation is one value
//!
//! Every flag this depends on lives in [`Invocation`], in one place. These are somebody else's
//! command line and it will change; when it does, the fix is one struct rather than a search
//! through string literals. The parser is deliberately tolerant for the same reason — an
//! unrecognised event is skipped, never fatal.

use epoch_models::quiet::Quiet;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use crate::agent::{Agent, AgentError, AgentStatus, Done, Progress, Task};
use crate::agents::PlanUsage;

/// What Epoch calls it.
pub const ID: &str = "claude-code";
pub const NAME: &str = "Claude Code";

/// How long to wait for the program to say what version it is.
///
/// Short: this runs while a surface is drawing, and a probe that hangs is a Launcher that hangs.
const PROBE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(6);

/// `/usage` is a local account query, not a turn. Keep it short so a changed CLI leaves a cold
/// instrument instead of making the World wait.
const USAGE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(4);

/// How long one alias gets to say what it resolves to.
///
/// Longer than [`USAGE_PATIENCE`] because ten of these start at once and a cold CLI is the
/// slowest of them: measured at about 1.2 s each serially, and a machine under load is not a
/// machine that should lose a model name.
const RESOLVING: std::time::Duration = std::time::Duration::from_secs(15);

/// How often a running turn looks up to see whether it has been stopped.
///
/// Short enough that STOP feels immediate, long enough that a silent agent costs nothing. It is
/// a *wake* interval, not a poll of anything: the work is being pushed to us, and this only
/// decides how often the waiting is interrupted.
const HEARTBEAT: std::time::Duration = std::time::Duration::from_millis(250);

/// Everything about somebody else's command line, in one place.
///
/// Named rather than scattered because it is the part most likely to be wrong today and to
/// change tomorrow. A future version that renames a flag is a one-line edit here.
///
/// **Verified against 2.1.223 rather than remembered.** Two of these were guesses, and both
/// guesses would have failed on the first real run:
///
/// - `stream-json` is refused without `--verbose`: *"When using --print,
///   --output-format=stream-json requires --verbose"*. An arg error, so it would have failed
///   instantly and completely.
/// - on Windows the program really is `claude.cmd`. Plain `claude` is a shell script, and
///   Windows answers *"%1 is not a valid Win32 application"* — a message that names nothing the
///   user could act on.
#[derive(Debug, Clone)]
pub struct Invocation {
    /// The program. On Windows an npm install leaves `claude` (a shell script Windows cannot
    /// execute) beside `claude.cmd` (a batch shim) and, inside, the real `claude.exe`.
    ///
    /// **Resolved to the real program where possible** — see [`resolve`]. A `.cmd` runs through
    /// `cmd.exe`, and Rust refuses to pass it an argument containing a newline at all:
    ///
    /// > `Claude Code could not be started: batch file arguments are invalid`
    ///
    /// That is not a corner case here. The persona is several lines, and a user typing a message
    /// with a line break in it would hit the same wall. There is no escaping that makes a
    /// newline survive `cmd.exe` — so the fix is to stop going through it.
    pub program: String,
    /// Arguments that come before everything else.
    ///
    /// Empty for a real executable. Non-empty only when the shim turned out to point at a script
    /// rather than a program, where the interpreter is the program and the script is its first
    /// argument — an older npm layout does exactly that.
    pub prelude: Vec<String>,
    /// Run once and print, rather than opening a session nobody is sitting in front of.
    pub print: &'static str,
    /// Take the message on **stdin** instead of as an argument.
    ///
    /// There is no `--image` flag. What exists is `--input-format stream-json`, which accepts
    /// API-shaped user messages — and an API user message can carry an `image` content block.
    /// Measured against 2.1.233 with a flat `(16,96,220)` PNG: it answered "Blue."
    ///
    /// Used **only** when a picture is going, because the argument path is the one that has been
    /// in production and there is no reason to move every turn onto a new transport to gain
    /// something only some turns need.
    pub piped: [&'static str; 2],
    /// Machine-readable events rather than prose, so what a tool did reaches the Terminal as a
    /// *fact* instead of being scraped out of a sentence.
    ///
    /// `--verbose` is not optional here — it is what the CLI demands alongside `stream-json`,
    /// and without it nothing runs at all.
    pub format: [&'static str; 3],
    /// Continue an earlier thread.
    ///
    /// Always given a value. `--resume` with none opens an interactive picker, which in a
    /// process nobody is sitting in front of is a hang.
    pub resume: &'static str,
    /// How much the agent may do without asking.
    ///
    /// Its own choices — `manual`, `acceptEdits`, `auto` — happen to be Epoch's three
    /// [`Autonomy`](epoch_kernel::Autonomy) values, and that is worth taking seriously: Epoch
    /// **launches** the process, so it chooses. The character's mode is not decoration for an
    /// agent Epoch spawned; it is passed through.
    ///
    /// What is still not Epoch's is the *prompt*. This version has no way to send a permission
    /// question back to us, so `manual` means "refuse what would need asking" rather than "ask
    /// Epoch" — the same word, a different act, and it has to be said rather than glossed.
    pub mode: &'static str,
    /// Which model, in the agent's own vocabulary.
    ///
    /// Only sent when the character named one. Empty means *whatever the agent is already set
    /// to* — the honest default, because the alternative is Epoch keeping a list of somebody
    /// else's model names and being wrong the day they add one.
    pub model: &'static str,
    /// Ask the CLI which models it takes, without spending a turn.
    ///
    /// The same shape as [`Self::usage`]: `-p` with a slash command, a JSON envelope, and no
    /// session left behind. Measured — 0 tokens, 0 cost, `num_turns: 0`, 94 ms — so this is a
    /// local answer from the program rather than an inference, and Epoch may ask it whenever a
    /// picker opens.
    pub listing: [&'static str; 5],
    /// Where Epoch's own tools live, as JSON on the command line.
    pub servers: &'static str,
    /// Which tool to call before running one of its own.
    ///
    /// **The missing half of Manual.** Measured against 2.1.224 with a throwaway MCP server:
    /// the named tool is called with `{tool_name, input, tool_use_id}` and its answer decides —
    /// `{"behavior":"allow","updatedInput":{…}}` created the file, `{"behavior":"deny",
    /// "message":"…"}` did not. Hidden from `--help`, so it was found by asking the program
    /// rather than by reading about it.
    pub asking: &'static str,
    /// Ask who is signed in, as machine-readable fact.
    ///
    /// `claude auth status --json` answers `{"loggedIn": …, "email": …}`. Verified against
    /// 2.1.224 rather than remembered — and it is the reason Epoch can tell *not installed*
    /// from *installed but signed out*, which have different fixes.
    pub whoami: [&'static str; 3],
    /// Ask Claude Code for its own account windows without starting a conversation.
    ///
    /// Measured on 2.1.226: `claude -p /usage --output-format json --no-session-persistence`
    /// returns a zero-cost JSON result whose `result` names the current session and current week.
    /// The prose within it remains an external CLI shape, so parsing is deliberately strict and
    /// an unfamiliar version simply produces no reading.
    pub usage: [&'static str; 5],
    /// Open the agent's own sign-in.
    ///
    /// Its flow, its browser, its token. Epoch starts it and steps back.
    pub login: [&'static str; 2],
    /// How hard to think.
    ///
    /// `--effort <level>`, taking low, medium, high, xhigh, max — read from the program's own
    /// help rather than remembered, which is also how we know it has no "off".
    pub effort: &'static str,
    /// A system prompt, carried as one.
    ///
    /// The character's identity is not part of what the user asked for, and putting it in the
    /// prompt text made it read as if it were. Separate flag, separate thing.
    pub persona: &'static str,
    /// Where **this account's** sign-in lives. `None` is the program's own default.
    ///
    /// ## Measured, not remembered
    ///
    /// `CLAUDE_CONFIG_DIR` is not in `--help`. Pointed at an empty directory, 2.1.241 answered
    /// `{"loggedIn": false, "authMethod": "none"}`, created its own `.claude.json` inside it, and
    /// **left the real sign-in untouched** — asked again immediately afterwards, the default
    /// still reported `owner@example.com`. That is the whole mechanism: one variable, and
    /// two accounts that cannot see each other.
    ///
    /// Epoch never reads what lands in there. It creates the directory, starts the agent's own
    /// login in the agent's own window, and steps back — the rule that has governed sign-in here
    /// since it existed.
    pub home: Option<std::path::PathBuf>,
}

/// The environment variable Claude Code reads its configuration directory from.
///
/// Undocumented — found by asking the program rather than by remembering, like the two facts of
/// ADR-0027 before it. Somebody else's flag is a measurement, not a memory.
pub const HOME_VAR: &str = "CLAUDE_CONFIG_DIR";

impl Invocation {
    /// The program, ready to be given arguments, pointed at this account's sign-in.
    ///
    /// Every place that runs Claude Code goes through here. A spawn site that built its own
    /// `Command` would run the *default* account while the rest of Epoch believed it was running
    /// another one — a wrong answer with nothing on screen to suggest it.
    pub fn start(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.prelude);
        if let Some(home) = &self.home {
            command.env(HOME_VAR, home);
        }
        command
    }
}

impl Default for Invocation {
    fn default() -> Self {
        let resolved = resolve();
        Self {
            program: resolved.0,
            prelude: resolved.1,
            print: "-p",
            format: ["--output-format", "stream-json", "--verbose"],
            piped: ["--input-format", "stream-json"],
            resume: "--resume",
            mode: "--permission-mode",
            model: "--model",
            effort: "--effort",
            servers: "--mcp-config",
            asking: "--permission-prompt-tool",
            whoami: ["auth", "status", "--json"],
            usage: [
                "-p",
                "/usage",
                "--output-format",
                "json",
                "--no-session-persistence",
            ],
            listing: [
                "-p",
                "/model",
                "--output-format",
                "json",
                "--no-session-persistence",
            ],
            login: ["auth", "login"],
            persona: "--append-system-prompt",
            // The account that was already there. Everything else is one somebody added.
            home: None,
        }
    }
}

/// Find the program to actually run, once.
///
/// On anything but Windows this is the name and nothing else — `claude` on a PATH is a program.
///
/// On Windows it is three answers in order of how well they work:
///
/// 1. **`claude.exe` on the PATH.** A real executable: arguments reach it exactly as written.
/// 2. **the target inside `claude.cmd`.** The npm shim is a batch file whose whole job is to
///    forward to something real. Reading which one, and running *that*, skips `cmd.exe` — and
///    with it the rule that a batch argument may not contain a newline.
/// 3. **`claude.cmd` itself.** Nothing was readable, so the shim is better than giving up: it
///    still works for anything without a line break in it.
///
/// Measured, never assumed — the same discipline as [`ClaudeCode::probe`]. The difference is
/// that this looks at *files* to decide what to run, and the probe then runs it to decide
/// whether it is there. Neither answer is taken from the other.
fn resolve() -> (String, Vec<String>) {
    if !cfg!(windows) {
        return ("claude".into(), Vec::new());
    }
    if let Some(exe) = crate::paths::on_path("claude.exe") {
        return (exe.display().to_string(), Vec::new());
    }
    if let Some(shim) = crate::paths::on_path("claude.cmd") {
        if let Ok(text) = std::fs::read_to_string(&shim) {
            if let Some(found) = forwarded_to(&text, &shim) {
                return found;
            }
        }
        return (shim.display().to_string(), Vec::new());
    }
    ("claude.cmd".into(), Vec::new())
}

/// What a batch shim forwards to.
///
/// Deliberately shallow: it looks for a quoted path ending in `.exe` or `.js` and expands the
/// one variable npm's shims use for their own directory. It is not a batch interpreter, and it
/// is not trying to be — an unrecognised shim returns `None` and the caller falls back to
/// running the shim, which is what happens today anyway.
fn forwarded_to(text: &str, shim: &std::path::Path) -> Option<(String, Vec<String>)> {
    let here = shim.parent()?.display().to_string();

    for line in text.lines() {
        // The first quoted span on the line, when there is one. A line without quotes is not a
        // forward, and skipping it is not a failure — `?` here would end the search at the
        // shim's first `@ECHO off`.
        let Some(quoted) = line.split('"').nth(1) else {
            continue;
        };
        let expanded = quoted
            .replace("%~dp0", &format!("{here}\\"))
            .replace("%dp0%", &format!("{here}\\"))
            .replace("\\\\", "\\");
        let lower = expanded.to_ascii_lowercase();

        if lower.ends_with(".exe") && std::path::Path::new(&expanded).is_file() {
            return Some((expanded, Vec::new()));
        }
        // An older layout: the shim runs a script, so the interpreter is the program. `node` is
        // on the PATH by construction — npm put the shim there.
        if lower.ends_with(".js") && std::path::Path::new(&expanded).is_file() {
            return Some(("node".to_string(), vec![expanded]));
        }
    }
    None
}

/// What Epoch's server is called on the agent's side, and its permission tool within it.
///
/// `mcp__<server>__<tool>` is how the CLI addresses an MCP tool, so this name and the one in
/// [`crate::adopt`] must agree — an agent pointed at a server called something else resolves
/// nothing and silently never asks.
use super::SERVER;
const APPROVAL_TOOL: &str = "mcp__epoch__approve";

/// Epoch's door, as the agent's own configuration.
///
/// Passed as JSON on the command line rather than written to the project: this is true for one
/// run, and a file would outlive the door it describes — a token in a repo that stopped working
/// when the World closed.
fn servers(door: &crate::agent::Door) -> String {
    serde_json::json!({
        "mcpServers": {
            SERVER: {
                "type": "http",
                "url": door.url,
                "headers": { "Authorization": format!("Bearer {}", door.token) },
            }
        }
    })
    .to_string()
}

/// Whether this model can be asked to think harder at all.
///
/// **Not every model on the ladder has one.** The CLI's own model picker says *"Effort not
/// supported for Haiku"* — and the command line does not complain: `--effort max --model haiku`
/// runs happily and changes nothing. A silent no-op is exactly the control the Launcher's rule
/// forbids, so the scale simply does not offer it there.
///
/// Matched on the family rather than a version, because that is what the fact is about: `haiku`
/// and `claude-haiku-4-5` are the same answer, and a list of exact ids would be wrong the day
/// they ship the next one.
pub fn supports_effort(model: &str) -> bool {
    !model.to_ascii_lowercase().contains("haiku")
}

/// What Claude Code called its own models when this was written — **the fallback, not the answer.**
///
/// [`models_offered`] asks the installed CLI, and that is what a person normally sees. This is
/// shown when nobody could be asked: not installed, not signed in, or a release whose `/model`
/// answers in a shape this build cannot read.
///
/// **Aliases, not versions.** The CLI takes `opus` as readily as `claude-opus-5`, and an alias
/// always resolves to the latest — measured: `--model sonnet` reports `claude-sonnet-5` in
/// `modelUsage`. So even stale, these four keep working; what they cannot do is name a value
/// Epoch never heard of, which is the half `/model` supplies.
pub fn models() -> &'static [(&'static str, &'static str)] {
    &[
        ("fable", "Fable 5 — the latest"),
        ("opus", "Opus 5 — the latest"),
        ("sonnet", "Sonnet 5 — the latest"),
        ("haiku", "Haiku 4.5 — the latest"),
    ]
}

/// Epoch's autonomy, in Claude Code's words.
///
/// The three names coincide, which is a coincidence worth *not* relying on silently: this
/// function is where the mapping lives, so a version that renames one is a change here rather
/// than a mode that quietly stops applying.
pub fn mode_of(autonomy: epoch_kernel::Autonomy) -> &'static str {
    match autonomy {
        epoch_kernel::Autonomy::Manual => "manual",
        epoch_kernel::Autonomy::AcceptEdits => "acceptEdits",
        epoch_kernel::Autonomy::Auto => "auto",
    }
}

/// Claude Code as an agent.
///
/// ## Why this carries an identity at all
///
/// One program, and possibly several **accounts** of it. `CLAUDE_CONFIG_DIR` isolates a sign-in
/// completely (measured — see [`Invocation::home`]), so two of these can exist side by side, each
/// pointed at its own directory, each answering `auth status` about its own account.
///
/// The id is therefore per instance rather than the constant: `claude-code` is the account that
/// was already there, `claude-code-2` is one somebody added. A character's brain names an id, so
/// this is what lets Mage work as one account while Paladin works as another.
pub struct ClaudeCode {
    invocation: Invocation,
    /// Stable id for **this account**. `ID` for the program's own default sign-in.
    id: String,
    /// What a person reads. The user's own label, never a measurement.
    name: String,
}

impl Default for ClaudeCode {
    fn default() -> Self {
        Self {
            invocation: Invocation::default(),
            id: ID.to_owned(),
            name: NAME.to_owned(),
        }
    }
}

impl ClaudeCode {
    pub fn with(invocation: Invocation) -> Self {
        Self {
            invocation,
            ..Self::default()
        }
    }

    /// One added account: its own id, its own name, its own sign-in directory.
    pub fn account(
        id: impl Into<String>,
        name: impl Into<String>,
        home: std::path::PathBuf,
    ) -> Self {
        Self {
            invocation: Invocation {
                home: Some(home),
                ..Invocation::default()
            },
            id: id.into(),
            name: name.into(),
        }
    }
}

impl Agent for ClaudeCode {
    fn id(&self) -> &str {
        &self.id
    }

    fn probe(&self) -> AgentStatus {
        let program = self.invocation.program.clone();
        let asked = self
            .invocation
            .start()
            .quiet()
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let mut child = match asked {
            Ok(child) => child,
            // The common case on a machine that has never installed it, and it must read as
            // *not installed* rather than as a fault: this is the sentence that tells somebody
            // what to do next.
            Err(err) => {
                let mut missing = AgentStatus::missing(
                    &self.id,
                    &self.name,
                    format!("`{program}` is not on this machine's PATH ({err})"),
                );
                missing.looked_in = Some(program);
                return missing;
            }
        };

        // Bounded by a wait rather than by hope. A child that never exits would otherwise hold
        // the surface that asked — the same lesson the provider probe learned from a firewall
        // that drops rather than refuses.
        let said = match wait_for(&mut child, PROBE_PATIENCE) {
            Some(text) => text,
            None => {
                let _ = child.kill();
                let mut stuck = AgentStatus::missing(
                    &self.id,
                    &self.name,
                    "it did not answer `--version` in time".to_string(),
                );
                stuck.looked_in = Some(program);
                return stuck;
            }
        };

        // Signed in is a *second* question, asked only once the first is answered. A program
        // that is not here has no account, and asking anyway would cost a process to learn
        // nothing.
        let (signed_in, account) = self.whoami();

        AgentStatus {
            kind: ID.to_owned(),
            id: self.id.clone(),
            name: self.name.clone(),
            installed: true,
            signed_in,
            account,
            // Whatever it said, unparsed. A version is for the user to read, and picking it
            // apart would be a second thing to keep up to date with somebody else's format.
            version: Some(said.trim().to_owned()).filter(|v| !v.is_empty()),
            looked_in: Some(program),
            note: None,
            // Read from its own sign-in status rather than from a chosen method: this agent can
            // be asked directly, so there is nothing left over for `method` to carry.
            method: None,
        }
    }

    fn sign_in(&self) -> Result<(), AgentError> {
        open_sign_in(&self.invocation).map_err(|why| AgentError::Failed {
            agent: NAME.into(),
            why,
        })
    }

    fn work(
        &self,
        task: &Task,
        stopped: &dyn Fn() -> bool,
        _approver: &dyn crate::agent::Approver,
        sink: &mut dyn FnMut(Progress),
    ) -> Result<Done, AgentError> {
        if !task.directory.is_dir() {
            return Err(AgentError::Nowhere);
        }

        // **Two transports, and only pictures choose the second one.**
        //
        // Without an image the message is an argument, which is the path that has been running
        // in production. With one it has to be a stream-json user message on stdin, because
        // there is no image flag — an argument cannot carry a content block.
        //
        // Both are measured. Moving every turn onto the piped form to gain something only some
        // turns need would put the common case on the newer path for no benefit.
        let piped = !task.images.is_empty();

        let mut command = self.invocation.start();
        command.quiet();
        command
            // **In the project, not in Epoch's folder.** An agent's whole world is its working
            // directory, and starting it anywhere else would point it at Epoch's own source.
            .current_dir(&task.directory)
            .arg(self.invocation.print);
        if piped {
            command.args(self.invocation.piped);
        } else {
            // The user's words, unchanged. Nothing of Epoch's is folded into them.
            command.arg(task.intent.trim());
        }
        command
            .args(self.invocation.format)
            // Who they are, as a *system* prompt — a separate flag for a separate thing.
            .arg(self.invocation.persona)
            .arg(persona(task))
            // Epoch launched this, so Epoch chooses. Passing nothing would not be humility, it
            // would be Epoch silently accepting the default.
            .arg(self.invocation.mode)
            .arg(mode_of(task.autonomy))
            // Open only when something is going to be written. A pipe nobody writes to and never
            // closes is a program waiting for input that will never come.
            .stdin(if piped { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if let Some(thread) = &task.thread {
            command.arg(self.invocation.resume).arg(thread);
        }
        // The field existed on `Brain::Agent` from the start and nothing ever passed it — a
        // character could name a model and be silently ignored. Sent only when named, so an
        // unset one still means "the agent's own choice" rather than an empty flag.
        if !task.model.trim().is_empty() {
            command.arg(self.invocation.model).arg(task.model.trim());
        }
        // Sent only when the character asked for a rung this program actually has. `Off` is not
        // one of them — mapping it to `low` would be Epoch deciding that "do not deliberate"
        // means "deliberate a little", which is a different instruction than the one given.
        if let Some(effort) = effort_of(task.reasoning) {
            command.arg(self.invocation.effort).arg(effort);
        }

        // Epoch, reachable — and invited to the permission decision.
        //
        // Both together or neither: naming a permission tool on a server the agent cannot reach
        // would leave every one of its tools waiting on a door that is shut, which is worse than
        // having no gate at all.
        if let Some(door) = &task.door {
            command.arg(self.invocation.servers).arg(servers(door));

            // **Always, including under `Auto` — and that was measured, not reasoned.**
            //
            // This used to be sent only when the mode was stricter than `Auto`, on the reasoning
            // that Auto already means yes and a round trip per tool would cost a network call to
            // say so. The reasoning was sound and the premise stopped being true.
            //
            // 2026-08-24, Claude Code 2.1.241, `--permission-mode auto` with Epoch's server
            // attached and no permission tool named. Asked to call one Epoch capability, it said:
            //
            // ```text
            // I need your permission to call the epoch_watchword tool. Please approve this request.
            // ```
            //
            // …twice, and then gave up. **Every Epoch capability was unreachable for a Claude Code
            // brain in Auto** — the crew, the Quest, the World's knowledge, the brush: the whole
            // answer to *why Epoch rather than the agent alone*. Codex, measured the same
            // afternoon, was unaffected. Asking the program why shows the vocabulary grew a rung:
            // `--permission-mode` now offers `dontAsk` alongside `auto`, so `auto` is no longer
            // the one that never asks.
            //
            // Naming the tool unconditionally is the fix that does not depend on which word means
            // what this month: when Epoch is invited to a decision it answers with the mode it
            // already holds, and under `Auto` that answer is an immediate yes with nobody
            // disturbed (`epoch-tauri/src/agent.rs`, `decide_for_them`). The alternative —
            // mapping `Auto` onto `bypassPermissions` — buys the same behaviour by handing out a
            // blanket bypass Epoch could no longer see through, which is the opposite of being
            // invited to the decision (ADR-0027's amendment).
            //
            // The round trip is real and it is the price of working at all.
            command.arg(self.invocation.asking).arg(APPROVAL_TOOL);
        }

        let mut child = command.spawn().map_err(|why| AgentError::Failed {
            agent: NAME.into(),
            why: why.to_string(),
        })?;

        // The message, when it is going in rather than beside.
        //
        // Written and **closed** immediately: `stream-json` input is a stream, and a program
        // reading one waits for end-of-input before it decides the turn is complete. Leaving the
        // pipe open is a hang with no error in it — the agent is fine, and simply still listening.
        if piped {
            use std::io::Write as _;
            let mut stdin = child.stdin.take().ok_or_else(|| AgentError::Failed {
                agent: NAME.into(),
                why: "it would not take a message".into(),
            })?;
            let message = user_message(task);
            let written = writeln!(stdin, "{message}").and_then(|()| stdin.flush());
            // Dropping closes it, and it must be dropped before the answer is read.
            drop(stdin);
            written.map_err(|why| AgentError::Failed {
                agent: NAME.into(),
                why: format!("could not send the message: {why}"),
            })?;
        }

        let stdout = child.stdout.take().ok_or_else(|| AgentError::Failed {
            agent: NAME.into(),
            why: "it produced no output at all".into(),
        })?;

        // **Read on another thread, so stopping does not depend on the agent speaking.**
        //
        // This was a plain `for line in …`, and `stopped()` was checked once per line — which
        // means an agent that went quiet could not be stopped at all. STOP stayed on screen
        // doing nothing, which is the worst kind of control: one the user reaches for precisely
        // when something has gone wrong.
        //
        // Now the wait has a timeout of its own. The reader blocks; this loop does not.
        let (lines, arriving) = std::sync::mpsc::channel::<String>();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                // A closed channel means nobody is listening any more — the turn was stopped.
                if lines.send(line).is_err() {
                    return;
                }
            }
        });

        let mut reading = Reading::default();
        loop {
            match arriving.recv_timeout(HEARTBEAT) {
                Ok(line) => read_event(&line, &mut reading, sink),
                // Nothing was said this tick. The only thing that changes here is whether the
                // user has asked it to stop — which is exactly why the tick exists.
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                // The agent's output ended: it is finished, or it died.
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if stopped() {
                // Killed, not merely abandoned. An agent left running would keep writing to the
                // user's project after they said stop, which is the opposite of what they asked.
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
        let _ = reader.join();

        let ended = child.wait().map_err(|why| AgentError::Failed {
            agent: NAME.into(),
            why: why.to_string(),
        })?;

        // Said by the CLI, never by the character. Ahead of the exit code because it is the
        // better answer: it exits 0 and explains itself, and the explanation is the useful part.
        if let Some(why) = reading.failed {
            return Err(AgentError::Stopped {
                agent: NAME.into(),
                why,
            });
        }

        // A non-zero exit with nothing said is a failure worth naming. A non-zero exit *with*
        // an answer is not: an agent that reported a problem and exited did its job.
        if !ended.success() && reading.done.text.trim().is_empty() {
            // What it said on the *other* pipe. Without this the user is told the program said
            // nothing while it was explaining itself in detail.
            let why = match super::last_words(&mut child) {
                Some(said) => format!("it exited with {ended}: {said}"),
                None => format!("it exited with {ended} and said nothing"),
            };
            return Err(AgentError::Stopped {
                agent: NAME.into(),
                why,
            });
        }

        Ok(reading.done)
    }
}

impl ClaudeCode {
    /// Who is signed in, measured.
    ///
    /// Never fatal. An unparseable answer, a version without the subcommand, a refused process
    /// — all of them mean *we could not ask*, which is `None`, and `None` prints as nothing
    /// rather than as "signed out". Telling somebody to sign in when they already are would send
    /// them to fix the wrong thing.
    fn whoami(&self) -> (Option<bool>, Option<String>) {
        let asked = self
            .invocation
            .start()
            .quiet()
            .args(self.invocation.whoami)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let Ok(mut child) = asked else {
            return (None, None);
        };
        let Some(said) = wait_for(&mut child, PROBE_PATIENCE) else {
            let _ = child.kill();
            return (None, None);
        };
        read_whoami(&said)
    }
}

/// Ask the installed Claude Code CLI for both subscription windows it exposes.
///
/// This is deliberately separate from [`ClaudeCode::whoami`]: presence has to stay a cheap
/// question for every screen, while this starts a second process and belongs only to the World
/// instrument. A failure is absence, never a zero allowance.
pub fn plan_usage(home: Option<std::path::PathBuf>) -> Option<Vec<PlanUsage>> {
    // Per account: an allowance belongs to a sign-in, not to a program.
    read_plan_usage(&Invocation {
        home,
        ..Invocation::default()
    })
}

fn read_plan_usage(invocation: &Invocation) -> Option<Vec<PlanUsage>> {
    let mut child = invocation
        .start()
        .quiet()
        .args(invocation.usage)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let said = match wait_for(&mut child, USAGE_PATIENCE) {
        Some(said) => said,
        None => {
            let _ = child.kill();
            return None;
        }
    };

    read_usage(&said)
}

/// Which models this Claude Code takes, asked rather than remembered.
///
/// **The correction that produced this is worth more than the function.** An earlier session
/// measured `--help`, found no `models` subcommand, and wrote down that Claude Code *has no
/// listing* — in `CLAUDE.md`, as a measured fact. `--help` answers **which subcommands exist**;
/// it says nothing about what a slash command answers in `-p`. The measurement was real and the
/// question was wrong, and the file three hundred lines up had already been reading `/usage`
/// exactly this way for months.
///
/// `/model` prints its own vocabulary and costs nothing to ask — `num_turns: 0`, no tokens, no
/// API time. It names ten values where Epoch's compiled list had four, including ones Epoch
/// could not have invented: `best`, `opusplan`, `default`, and the long-window `sonnet[1m]`.
///
/// Per account, like the allowance: what a sign-in may reach belongs to the sign-in.
///
/// `None` means **nobody could be asked** — not installed, not signed in, a release that answers
/// differently. The caller falls back to [`models`], which is why that list still exists.
pub fn models_offered(home: Option<std::path::PathBuf>) -> Option<Vec<(String, String)>> {
    read_models_offered(&Invocation {
        home,
        ..Invocation::default()
    })
}

fn read_models_offered(invocation: &Invocation) -> Option<Vec<(String, String)>> {
    let mut child = invocation
        .start()
        .quiet()
        .args(invocation.listing)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let said = match wait_for(&mut child, USAGE_PATIENCE) {
        Some(said) => said,
        None => {
            let _ = child.kill();
            return None;
        }
    };

    let aliases = read_listing(&said)?;

    /*
        **An alias is what you type; the model is what you get, and the program will say which.**

        Reported by the owner with two screenshots side by side: Claude Code's own picker offers
        *Fable 5.1 · Opus 5 · Sonnet 5 · Haiku 4.5*, and Epoch offered `fable · opus · sonnet ·
        haiku`. Both lists are correct and only one of them is readable.

        `-p /model` reports the **session's** model, so passing `--model <alias>` makes it report
        what that alias resolved to — and it costs nothing: `num_turns: 0`, no tokens, no API
        time, measured across all ten. Ten of them serially is 12.5 s, which is why they are
        asked at once; the answer is then kept for ten minutes like the list itself.

        And it is the half of this that updates itself. The day a newer model ships, the CLI
        resolves the same alias to it and Epoch says the new name without a line changing here.
        What Epoch must never do is keep its own table of `opus -> Opus 5`: that is a sentence
        about a moment, and it is wrong the morning after.
    */
    let named: Vec<_> = aliases
        .into_iter()
        .map(|(alias, fallback)| {
            let invocation = Invocation {
                home: invocation.home.clone(),
                ..Invocation::default()
            };
            std::thread::spawn(move || {
                // **Both, because several aliases answer to one model.** `fable` and
                // `best` are both Fable 5 today, and `sonnet`, `sonnet[1m]` and
                // `default` are all Sonnet 5 — a list showing only the name would have
                // three identical rows and no way to tell which one you picked. The
                // name comes first because it is what a person recognises; the alias is
                // what actually goes on the command line.
                let shown = match resolved_name(&invocation, &alias) {
                    Some(name) if name != alias => format!("{name} · {alias}"),
                    Some(name) => name,
                    None => fallback,
                };
                (alias, shown)
            })
        })
        .collect();
    Some(
        named
            .into_iter()
            .filter_map(|asking| asking.join().ok())
            .collect(),
    )
}

/// What one alias resolves to on this machine, in the program's own words.
///
/// `Current model: Opus 5 (effort: medium)` — the name is what precedes the parenthesis, and an
/// answer of another shape yields `None` so the caller shows the alias rather than half a
/// sentence. The effort is deliberately dropped: it belongs to the session this probe made, not
/// to the model, and putting it on a row would be a reading of the wrong quantity.
fn resolved_name(invocation: &Invocation, alias: &str) -> Option<String> {
    let mut child = invocation
        .start()
        .quiet()
        .args(invocation.listing)
        .arg(invocation.model)
        .arg(alias)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let said = match wait_for(&mut child, RESOLVING) {
        Some(said) => said,
        None => {
            let _ = child.kill();
            return None;
        }
    };
    let answer = serde_json::from_str::<serde_json::Value>(said.trim()).ok()?;
    if answer.get("is_error").and_then(serde_json::Value::as_bool) == Some(true) {
        return None;
    }
    let report = answer.get("result")?.as_str()?;
    let (_, rest) = report.split_once("Current model: ")?;
    // **The backticks are 2.1.265's and were not 2.1.247's.** Measured on the same machine
    // an hour apart: `Current model: Opus 5` became `Current model: `Opus 5``. Trimmed
    // rather than matched, so a version that stops using them needs no edit either —
    // somebody else's output shape is a measurement, not a memory.
    let name = rest
        .split(" (effort")
        .next()?
        .lines()
        .next()?
        .trim()
        .trim_matches('`')
        .trim()
        .to_owned();
    (!name.is_empty()).then_some(name)
}

/// The names out of `/model`'s one-line answer.
///
/// ```text
/// Current model: Haiku 4.5 (effort: medium)
/// Usage: /model <name>. Available: sonnet, opus, haiku, fable, best, sonnet[1m], opus[1m],
/// fable[1m], opusplan, default, or a full model ID.
/// ```
///
/// **Deliberately narrow, like [`read_usage`] beside it.** This is person-facing prose inside a
/// machine envelope, so it is read by one landmark — `Available: ` — and abandoned entirely if
/// that landmark moves. A reshaped sentence must extinguish the list rather than turn half of it
/// into model names: a picker offering `or a full model ID` as a model is worse than a picker
/// showing the four aliases Epoch already knew.
///
/// **No labels are invented.** The CLI gives none, and the compiled fallback's *"Opus 5 — the
/// latest"* is the sort of description that is wrong the day Opus 6 ships. An alias is what a
/// person types; it stands on its own.
fn read_listing(said: &str) -> Option<Vec<(String, String)>> {
    let answer = serde_json::from_str::<serde_json::Value>(said.trim()).ok()?;
    if answer.get("is_error").and_then(serde_json::Value::as_bool) == Some(true) {
        return None;
    }
    let report = answer.get("result")?.as_str()?;
    let (_, offered) = report.split_once("Available: ")?;
    // The sentence ends by saying a full id is also accepted. That is true and it is not a name.
    let offered = offered
        .split(", or a full model")
        .next()
        .unwrap_or(offered)
        .trim_end_matches('.');
    let found: Vec<(String, String)> = offered
        .split(',')
        .map(str::trim)
        .filter(|name| {
            !name.is_empty()
                // Nothing with a space in it is an alias — it is prose that got past the split.
                && !name.contains(char::is_whitespace)
        })
        .map(|name| (name.to_owned(), name.to_owned()))
        .collect();
    (!found.is_empty()).then_some(found)
}

/// Read the two explicitly named windows out of Claude Code's zero-turn `/usage` result.
///
/// The JSON envelope keeps this a machine conversation, while the `result` itself is concise
/// person-facing text. We intentionally recognise the two window labels, their percentage, and
/// nothing else: a renamed or expanded report should extinguish the gauge instead of making an
/// approximate plan reading look authoritative.
fn read_usage(said: &str) -> Option<Vec<PlanUsage>> {
    let answer = serde_json::from_str::<serde_json::Value>(said.trim()).ok()?;
    if answer.get("is_error").and_then(serde_json::Value::as_bool) == Some(true) {
        return None;
    }
    let report = answer.get("result")?.as_str()?;
    let mut session = None;
    let mut week = None;

    for line in report.lines() {
        let line = line.trim();
        let (window, rest) = if let Some(rest) = line.strip_prefix("Current session:") {
            (5 * 60, rest)
        } else if let Some(rest) = line.strip_prefix("Current week") {
            // Claude currently spells this "Current week (all models):". Accept only a colon
            // after that known family of labels so unrelated prose cannot become an allowance.
            (
                7 * 24 * 60,
                rest.strip_prefix(" (all models):")
                    .or_else(|| rest.strip_prefix(':'))?,
            )
        } else {
            continue;
        };

        let used = rest
            .trim()
            .split_once('%')?
            .0
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|percent| *percent <= 100)?;
        let reading = PlanUsage {
            used_percent: used,
            remaining_percent: 100 - used,
            window_minutes: Some(window),
            // Claude's result gives a timezone-labelled human sentence, not an epoch instant.
            // Do not silently pretend it was a timestamp.
            resets_at: None,
            // Measured 2026-08-12: `claude -p /usage` reports subscription percentages and no
            // balance of any kind. An agent with no concept of credit shows nothing, never zero.
            credits: None,
        };
        if window == 5 * 60 {
            session = Some(reading);
        } else {
            week = Some(reading);
        }
    }

    Some(vec![session?, week?])
}

/// Read who is signed in out of the agent's answer.
///
/// Split from the process so the *reading* can be tested without one. Tolerant like the event
/// parser and for the same reason: this is somebody else's output format, and a version that
/// renames a field must leave Epoch saying "could not ask" rather than "signed out".
fn read_whoami(said: &str) -> (Option<bool>, Option<String>) {
    let Ok(answer) = serde_json::from_str::<serde_json::Value>(said.trim()) else {
        return (None, None);
    };

    (
        answer.get("loggedIn").and_then(|v| v.as_bool()),
        answer
            .get("email")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .filter(|e| !e.is_empty()),
    )
}

/// Start the agent's sign-in in a window the user can actually use.
///
/// `claude auth login` is interactive: it prints a URL, opens a browser and waits. Run with
/// piped output it would sit there invisibly, and Epoch would be a program that appears to hang
/// while somebody's credential is on the other side of it.
///
/// So it gets a **real console**. Epoch does not read its output, does not parse the URL and
/// never sees the token — this is the one place where getting out of the way is the feature.
#[cfg(windows)]
fn open_sign_in(invocation: &Invocation) -> Result<(), String> {
    use std::os::windows::process::CommandExt;

    // Built as one raw string because `start` re-parses it: the first quoted word would
    // otherwise be taken as the window title, which is how a path with a space in it ends up
    // opening an empty console called `C:\Program`.
    let mut line = String::from("/c start \"Epoch - sign in to Claude Code\" cmd /k ");
    line.push_str(&format!("\"{}\"", invocation.program));
    for arg in &invocation.prelude {
        line.push_str(&format!(" \"{arg}\""));
    }
    for arg in invocation.login {
        line.push_str(&format!(" {arg}"));
    }

    let mut terminal = Command::new("cmd.exe");
    terminal.raw_arg(&line);
    // **The account being signed into, not whichever one is default.** The variable rides on the
    // terminal Epoch opens, because that terminal is the process that will run the login — and
    // Epoch never sees what comes back through it.
    if let Some(home) = &invocation.home {
        let _ = std::fs::create_dir_all(home);
        terminal.env(HOME_VAR, home);
    }
    terminal
        .spawn()
        .map(|_| ())
        .map_err(|why| format!("a terminal could not be opened ({why})"))
}

/// Elsewhere, name the command rather than guess a terminal.
///
/// There is no portable "open a console" on Linux, and picking one would be wrong on most
/// machines. A sentence somebody can paste is honest; a guess that opens nothing is not.
#[cfg(not(windows))]
fn open_sign_in(invocation: &Invocation) -> Result<(), String> {
    // The variable belongs in the sentence too: a command pasted without it would sign the
    // *default* account in, which is the one thing this must not do.
    let prefix = match &invocation.home {
        Some(home) => format!("{HOME_VAR}=\"{}\" ", home.display()),
        None => String::new(),
    };
    if let Some(home) = &invocation.home {
        let _ = std::fs::create_dir_all(home);
    }
    Err(format!(
        "run `{prefix}{} {}` in a terminal to sign in",
        invocation.program,
        invocation.login.join(" ")
    ))
}

/// Epoch's rung, in Claude Code's words.
///
/// `None` where this program has nothing to say it with. The ladder is canonical and finer than
/// any one backend (ADR-0026), so a brain that cannot express a rung declines it rather than
/// substituting a neighbour — the scale offered for this agent excludes `Off` for exactly this
/// reason, and this is the second line of defence for a file authored by hand.
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

/// Who is working, as a system prompt.
///
/// **Not** part of the intent. The user's words are never rewritten and never padded
/// (ADR-0025); folding an identity into them made a character's personality read as something
/// the user had typed, and gave the agent a reason to treat it as a request rather than as who
/// is speaking. `--append-system-prompt` is where this belongs, and it exists.
///
/// Deliberately short. An agent can *ask* Epoch for the crew, the Quest and this World's
/// knowledge through the door (step 5.2) — so this briefs it and stops, rather than dictating
/// context it is perfectly able to go and fetch.
/// One user message, in the shape `--input-format stream-json` reads.
///
/// The API's own content blocks, which is what lets a picture travel at all: an argument is a
/// string and a string cannot carry an image. Measured against 2.1.233 — a flat `(16,96,220)`
/// PNG sent this way came back as "Blue."
///
/// Pictures first, words after. A question about an image reads better once the image is there,
/// and it is the order the same message is drawn in the Chronicle.
///
/// The user's words are still unchanged and unpadded (ADR-0025); only the envelope differs.
fn user_message(task: &Task) -> String {
    use base64::Engine as _;
    let mut content: Vec<serde_json::Value> = task
        .images
        .iter()
        .map(|image| {
            serde_json::json!({
                "type": "image",
                "source": {
                    "type": "base64",
                    "media_type": image.mime,
                    "data": base64::engine::general_purpose::STANDARD.encode(&image.bytes),
                },
            })
        })
        .collect();
    content.push(serde_json::json!({ "type": "text", "text": task.intent.trim() }));

    serde_json::json!({
        "type": "user",
        "message": { "role": "user", "content": content },
    })
    .to_string()
}

fn persona(task: &Task) -> String {
    // What Epoch started it with, said plainly.
    //
    // Asked "what effort level are you on?", the agent answered that it could not see one and
    // then read `settings.json` — reporting the *file* while the session was running on the
    // flag. Both answers were honest and they disagreed, which looks exactly like a broken
    // control. Epoch knows what it passed, so Epoch says it.
    let effort = match effort_of(task.reasoning) {
        Some(level) => format!(
            "

Epoch started this session with reasoning effort `{level}`. If you are asked, that is the answer — a level in a settings file is not what is running now."
        ),
        None => String::new(),
    };
    // Who the crew actually are, in the same words a model is given. Without it "one of a
    // crew" is a phrase with nobody in it.
    let crew = match task.crew.trim() {
        "" => String::new(),
        note => format!("\n\n{note}"),
    };
    // How this crew does these jobs, in those same words. Last, after who they are and who
    // they are with: a method is not an identity, and putting it first would make it one.
    let skills = match task.skills.trim() {
        "" => String::new(),
        note => format!("\n\n{note}"),
    };
    // Where the notes are, and that Epoch's tools are what reach them. Last, because it is a
    // fact about the World rather than about who they are.
    let library = match task.library.trim() {
        "" => String::new(),
        note => format!("\n\n{note}"),
    };
    // What is connected here, and where the user fixes one that is not.
    let connections = match task.connections.trim() {
        "" => String::new(),
        note => format!(
            "

{note}"
        ),
    };
    format!(
        "You are {}, working inside Epoch as one of a crew.

{}

         Epoch's own tools are available to you over MCP: the crew and what they know, the Quest you are on, and this World's knowledge. Keep using your own tools for files, search and commands — they are yours and they are good.{}",
        task.character,
        task.persona.trim(),
        effort,
    ) + &crew
        + &skills
        + &library
        + &connections
}

/// Read one event, tolerantly.
///
/// Unrecognised shapes are skipped rather than treated as errors. This is somebody else's
/// output format and it will grow fields; a parser that failed on an unknown event would turn
/// every upgrade of Claude Code into an outage of Epoch.
/// Everything one run accumulates while its output is read.
///
/// Gathered into a value once a **third** field appeared. A tool's fate arrives on a later line
/// than the tool does, so reading the stream needs memory — and four out-parameters would be
/// four things a caller could pass in the wrong order.
#[derive(Debug, Default)]
struct Reading {
    done: Done,
    /// Why there is no answer, when there is none.
    failed: Option<String>,
    /// Tools that have been *asked for* and not yet answered, by the agent's own id.
    ///
    /// Nothing is reported from here until its result arrives. That is the whole fix: a
    /// `tool_use` is a request, and Epoch was showing requests as work done — the Terminal said
    /// `Write …` twice for a file that was refused both times and never existed.
    pending: std::collections::BTreeMap<String, (String, String)>,
}

fn read_event(line: &str, reading: &mut Reading, sink: &mut dyn FnMut(Progress)) {
    let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
        return;
    };

    // The handle to continue this thread. Kept rather than copied: an agent owns its own
    // history, and Epoch holding a second copy would be a record that drifts from the real one.
    if let Some(id) = event.get("session_id").and_then(|v| v.as_str()) {
        reading.done.thread = Some(id.to_owned());
    }

    match event.get("type").and_then(|v| v.as_str()) {
        // The final answer — **or the reason there is not one**.
        //
        // A run that could not authenticate ends here too, with `is_error` and a sentence
        // explaining what to do. Read as an answer, that sentence became *the character saying
        // it*: Mage appeared to reply "Not logged in · Please run /login", which she did not,
        // and which the World may never claim (the World never lies).
        //
        // The flag is what separates them, and it is on the event we already parse.
        Some("result") => {
            let text = event
                .get("result")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();

            let broke = event
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || event
                    .get("subtype")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s != "success");

            // Its own accounting of its own window. Cache reads are part of what the model was
            // given, so they count: a number that excluded them would say a long conversation
            // was nearly empty.
            if let Some(usage) = event.get("modelUsage").and_then(|v| v.as_object()) {
                if let Some(one) = usage.values().next() {
                    let of = |key: &str| one.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
                    let used = of("inputTokens")
                        + of("cacheReadInputTokens")
                        + of("cacheCreationInputTokens")
                        + of("outputTokens");
                    let budget = of("contextWindow");
                    if budget > 0 {
                        sink(Progress::Window {
                            used,
                            budget,
                            session: reading.done.thread.clone(),
                        });
                    }
                }
            }

            if broke {
                reading.failed = Some(if text.trim().is_empty() {
                    "it ended without saying why".to_string()
                } else {
                    text
                });
            } else {
                reading.done.text = text;
            }
        }
        Some("assistant") => {
            for part in blocks(&event) {
                match part.get("type").and_then(|v| v.as_str()) {
                    Some("text") => {
                        if let Some(said) = part.get("text").and_then(|v| v.as_str()) {
                            sink(Progress::Said(said.to_owned()));
                        }
                    }
                    // Witnessed, never authorised — and **not yet witnessed happening**. Epoch
                    // did not permit this and does not claim to have; it also does not know yet
                    // whether it worked. Held until the result says.
                    Some("tool_use") => {
                        let tool = part
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("a tool")
                            .to_owned();
                        let detail = part.get("input").map(summarise).unwrap_or_default();
                        let id = part
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_owned();
                        reading.pending.insert(id, (tool, detail));
                    }
                    _ => {}
                }
            }
        }
        // What became of it.
        //
        // The agent reports a refused tool here, exactly as it reports a failed one:
        // `{"type":"tool_result","is_error":true,"tool_use_id":…}` carrying *"Claude requested
        // permissions … but you haven't granted it yet"*. Captured from a real refusal rather
        // than imagined, which is how the shape of `is_error` is known at all.
        Some("user") => {
            for part in blocks(&event) {
                if part.get("type").and_then(|v| v.as_str()) != Some("tool_result") {
                    continue;
                }
                let id = part
                    .get("tool_use_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let Some((tool, detail)) = reading.pending.remove(id) else {
                    continue;
                };
                let ok = !part
                    .get("is_error")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                // **Evidence only when it happened.** A Quest that produced nothing must say so
                // (ADR-0025), and a refused write that entered History as an artifact would be
                // the record claiming a file exists that does not.
                if ok {
                    reading.done.evidence.push(crate::capability::Made {
                        // What it touched, when the agent said — its own tools report a path
                        // here. Falling back to the tool's name keeps the record followable to
                        // whatever precision the agent actually gave.
                        reference: if detail.is_empty() {
                            tool.clone()
                        } else {
                            detail.clone()
                        },
                        summary: if detail.is_empty() {
                            tool.clone()
                        } else {
                            format!("{tool} {detail}")
                        },
                    });
                }
                sink(Progress::Ran { tool, detail, ok });
            }
        }
        _ => {}
    }
}

fn blocks(event: &serde_json::Value) -> Vec<serde_json::Value> {
    event
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default()
}

/// One line about what a tool was given.
///
/// Short on purpose: this reaches the Terminal, where a wall of JSON would bury the next thing
/// that happened.
fn summarise(input: &serde_json::Value) -> String {
    let Some(map) = input.as_object() else {
        return String::new();
    };
    for key in ["file_path", "path", "command", "pattern", "query"] {
        if let Some(value) = map.get(key).and_then(|v| v.as_str()) {
            return value.chars().take(120).collect();
        }
    }
    String::new()
}

/// Wait for a child, and give up rather than hang.
pub(crate) fn wait_for(
    child: &mut std::process::Child,
    patience: std::time::Duration,
) -> Option<String> {
    let deadline = std::time::Instant::now() + patience;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(40));
            }
            _ => return None,
        }
    }
    let mut said = String::new();
    if let Some(mut out) = child.stdout.take() {
        use std::io::Read;
        let _ = out.read_to_string(&mut said);
    }
    Some(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_keeps_claudes_two_real_windows_separate() {
        let answer = serde_json::json!({
            "is_error": false,
            "result": "You are currently using your subscription to power your Claude Code usage\n\nCurrent session: 0% used · resets Aug 10, 9:20pm (America/Guatemala)\nCurrent week (all models): 95% used · resets Aug 11, 6pm (America/Guatemala)"
        })
        .to_string();

        assert_eq!(
            read_usage(&answer),
            Some(vec![
                PlanUsage {
                    used_percent: 0,
                    remaining_percent: 100,
                    window_minutes: Some(5 * 60),
                    resets_at: None,
                    credits: None,
                },
                PlanUsage {
                    used_percent: 95,
                    remaining_percent: 5,
                    window_minutes: Some(7 * 24 * 60),
                    resets_at: None,
                    credits: None,
                },
            ])
        );
    }

    #[test]
    fn usage_goes_cold_when_a_required_window_is_not_reported() {
        let answer = serde_json::json!({
            "is_error": false,
            "result": "Current session: 12% used"
        })
        .to_string();
        assert_eq!(read_usage(&answer), None);
    }

    #[test]
    fn a_machine_without_it_says_so_rather_than_failing() {
        // The common case, and it must read as *not installed* — that is the sentence that
        // tells somebody what to do next.
        let agent = ClaudeCode::with(Invocation {
            program: "definitely-not-a-real-program".into(),
            ..Invocation::default()
        });
        let found = agent.probe();

        assert!(!found.installed);
        assert!(found.version.is_none(), "never a guessed version");
        assert_eq!(
            found.looked_in.as_deref(),
            Some("definitely-not-a-real-program")
        );
        assert!(
            found.note.unwrap().contains("PATH"),
            "somewhere to go and look"
        );
    }

    #[test]
    fn an_agent_with_nowhere_to_work_is_refused_before_it_starts() {
        // Its whole world is its working directory. Starting it in Epoch's own folder would
        // point it at Epoch's source, which is the wrong project and nobody's intention.
        let agent = ClaudeCode::default();
        let task = Task {
            intent: "do something".into(),
            character: "Mage".into(),
            model: String::new(),
            reasoning: None,
            door: None,
            persona: "curious".into(),
            crew: String::new(),
            skills: String::new(),
            library: String::new(),
            connections: String::new(),
            images: Vec::new(),
            directory: std::path::PathBuf::from("/definitely/not/a/folder"),
            pictures: std::path::PathBuf::new(),
            thread: None,
            autonomy: epoch_kernel::Autonomy::Manual,
        };
        assert!(matches!(
            agent.work(&task, &|| false, &crate::agent::NobodyToAsk, &mut |_| {}),
            Err(AgentError::Nowhere)
        ));
    }

    #[test]
    fn the_system_prompt_carries_who_they_are_and_nothing_the_user_typed() {
        // The user's words are never rewritten (ADR-0025), and the persona is what makes this
        // Mage rather than a generic agent.
        let task = Task {
            intent: "rename the config key".into(),
            character: "Mage".into(),
            model: String::new(),
            reasoning: None,
            door: None,
            persona: "You name tradeoffs explicitly.".into(),
            crew: String::new(),
            skills: String::new(),
            library: String::new(),
            connections: String::new(),
            images: Vec::new(),
            directory: std::env::temp_dir(),
            pictures: std::path::PathBuf::new(),
            thread: None,
            autonomy: epoch_kernel::Autonomy::Manual,
        };
        let said = persona(&task);

        assert!(said.contains("You are Mage"));
        assert!(said.contains("You name tradeoffs explicitly."));
        // And the intent is **not** in here. It goes as the prompt, unchanged and unpadded —
        // the user's words are never rewritten (ADR-0025).
        assert!(!said.contains("rename the config key"));
    }

    #[test]
    fn a_picture_travels_as_an_api_content_block() {
        // The shape was measured, not read about: there is no `--image` flag, and a flat
        // (16,96,220) PNG sent this way to 2.1.233 came back as "Blue."
        //
        // Asserted because the failure is silent and expensive. A wrong envelope does not error —
        // the CLI reads a user message it does not fully understand, answers about the text
        // alone, and the run looks like a model that ignored the picture.
        let task = Task {
            intent: "what colour is this?".into(),
            character: "Mage".into(),
            model: String::new(),
            reasoning: None,
            door: None,
            persona: String::new(),
            crew: String::new(),
            library: String::new(),
            connections: String::new(),
            skills: String::new(),
            images: vec![crate::agent::SharedImage {
                name: "flat.png".into(),
                bytes: vec![1, 2, 3],
                mime: "image/png",
            }],
            directory: std::env::temp_dir(),
            pictures: std::path::PathBuf::new(),
            thread: None,
            autonomy: epoch_kernel::Autonomy::Manual,
        };

        let sent: serde_json::Value =
            serde_json::from_str(&user_message(&task)).expect("valid JSON on one line");

        assert_eq!(sent["type"], "user");
        let content = sent["message"]["content"].as_array().expect("blocks");
        // The picture first, then the words — the order the same message is drawn in.
        assert_eq!(content[0]["type"], "image");
        assert_eq!(content[0]["source"]["type"], "base64");
        assert_eq!(content[0]["source"]["media_type"], "image/png");
        assert_eq!(content[0]["source"]["data"], "AQID");
        assert_eq!(content[1]["type"], "text");
        // Unchanged and unpadded (ADR-0025): only the envelope differs.
        assert_eq!(content[1]["text"], "what colour is this?");
        // And the filename Epoch chose is nowhere in it — the agent is told what the *user*
        // called the picture, never where Epoch keeps it.
        assert!(!user_message(&task).contains("shared_images"));
    }

    #[test]
    fn a_way_of_working_is_written_into_the_system_prompt() {
        // The same assertion Codex carries, because this is the same defect twice: `Task` can
        // hold a Skill and this function can still drop it, and each agent builds its briefing
        // separately. One test per path, or one of the two paths silently loses it again.
        let task = Task {
            intent: "revisa este codigo".into(),
            character: "Mage".into(),
            model: String::new(),
            reasoning: None,
            door: None,
            persona: "You name tradeoffs explicitly.".into(),
            crew: String::new(),
            library: String::new(),
            connections: String::new(),
            images: Vec::new(),
            skills: "Ways of working you have been given.\n\n## Code review\nRead the diff twice."
                .into(),
            directory: std::env::temp_dir(),
            pictures: std::path::PathBuf::new(),
            thread: None,
            autonomy: epoch_kernel::Autonomy::Manual,
        };
        let said = persona(&task);

        assert!(said.contains("Read the diff twice"));
        // Who they are still comes first. A method read before an identity becomes one.
        assert!(said.find("You name tradeoffs explicitly.") < said.find("Read the diff twice"));
    }

    #[test]
    fn an_unknown_event_is_skipped_rather_than_fatal() {
        // Somebody else's output format, which will grow fields. A parser that failed on an
        // unknown event would turn every upgrade of Claude Code into an outage of Epoch.
        let mut reading = Reading::default();
        read_event("not json at all", &mut reading, &mut |_| {});
        read_event(
            r#"{"type":"something_new","payload":{}}"#,
            &mut reading,
            &mut |_| {},
        );

        assert_eq!(reading.done, Done::default());
    }

    #[test]
    fn what_it_said_and_what_it_ran_both_arrive() {
        let mut reading = Reading::default();
        let mut seen = Vec::new();

        read_event(
            r#"{"type":"assistant","session_id":"abc",
                "message":{"content":[
                  {"type":"text","text":"looking"},
                  {"type":"tool_use","id":"t1","name":"Write","input":{"file_path":"src/main.rs"}}
                ]}}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );
        // Nothing claimed yet: asking to write is not writing.
        assert_eq!(seen.len(), 1);
        assert!(reading.done.evidence.is_empty());

        read_event(
            r#"{"type":"user","message":{"content":[
                {"type":"tool_result","tool_use_id":"t1","content":"ok"}]}}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );
        read_event(
            r#"{"type":"result","result":"done","session_id":"abc"}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );

        assert_eq!(seen[0], Progress::Said("looking".into()));
        assert_eq!(
            seen[1],
            Progress::Ran {
                tool: "Write".into(),
                detail: "src/main.rs".into(),
                ok: true
            }
        );
        assert_eq!(reading.done.text, "done");
        // Evidence, because something now exists that did not before (ADR-0025).
        // Both halves: what it says, and the thing it names so History can be followed.
        assert_eq!(
            reading
                .done
                .evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["Write src/main.rs"]
        );
        assert_eq!(reading.done.evidence[0].reference, "src/main.rs");
        // The handle to continue, kept rather than a copy of its history.
        assert_eq!(reading.done.thread.as_deref(), Some("abc"));
    }

    /// A refused tool is not work, and it is not evidence.
    ///
    /// Captured from a real refusal: with `--permission-mode manual` the agent cannot ask
    /// through Epoch, so it refuses and reports
    /// `{"type":"tool_result","is_error":true,…"but you haven't granted it yet"}`. Epoch showed
    /// `Write …` in the Terminal anyway — twice — for a file that never existed.
    #[test]
    fn a_refused_tool_never_becomes_work_or_evidence() {
        let mut reading = Reading::default();
        let mut seen = Vec::new();

        read_event(
            r#"{"type":"assistant","message":{"content":[
                {"type":"tool_use","id":"t9","name":"Write","input":{"file_path":"notes.md"}}]}}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );
        read_event(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t9",
                "is_error":true,
                "content":"Claude requested permissions to write, but you haven't granted it yet."}]}}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );

        assert_eq!(
            seen,
            vec![Progress::Ran {
                tool: "Write".into(),
                detail: "notes.md".into(),
                ok: false
            }],
            "reported once, as the failure it was"
        );
        // The record must not claim a file exists that does not (ADR-0025).
        assert!(reading.done.evidence.is_empty());
    }

    /// A tool whose fate never arrived is not reported at all.
    #[test]
    fn a_tool_with_no_result_is_never_claimed() {
        // The run was interrupted, or the stream ended early. Silence is the honest reading:
        // Epoch does not know whether it happened, so it says nothing rather than guessing.
        let mut reading = Reading::default();
        let mut seen = Vec::new();
        read_event(
            r#"{"type":"assistant","message":{"content":[
                {"type":"tool_use","id":"t4","name":"Bash","input":{"command":"rm -rf x"}}]}}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );
        assert!(seen.is_empty());
        assert!(reading.done.evidence.is_empty());
    }

    /// The context gauge, filled from the agent's own accounting.
    ///
    /// Epoch's Composer builds nothing for these turns, so the gauge read `—` forever — correct
    /// for an instrument with nothing behind it, and wrong here, because the agent reports its
    /// own usage and window in the event we already parse.
    #[test]
    fn how_full_its_window_is_comes_from_the_agent_itself() {
        let mut reading = Reading::default();
        let mut seen = Vec::new();
        read_event(
            r#"{"type":"result","subtype":"success","result":"done",
                "modelUsage":{"claude-opus-5":{"inputTokens":10,"outputTokens":5,
                  "cacheReadInputTokens":1000,"cacheCreationInputTokens":200,
                  "contextWindow":1000000}}}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );

        // Cache reads count: they are part of what the model was given, and a number that left
        // them out would report a long conversation as nearly empty.
        assert_eq!(
            seen[0],
            Progress::Window {
                used: 1215,
                budget: 1_000_000,
                session: None
            }
        );
    }

    /// A turn that reported no window leaves the gauge alone.
    #[test]
    fn no_accounting_means_no_reading_rather_than_zero() {
        let mut reading = Reading::default();
        let mut seen = Vec::new();
        read_event(
            r#"{"type":"result","subtype":"success","result":"done"}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );
        assert!(
            seen.is_empty(),
            "0 / 0 would read as an empty window, which is a claim"
        );
    }

    /// Every rung this agent's scale offers must reach the command line.
    ///
    /// The pair that would rot silently: a scale offering a level, and a translation that has no
    /// word for it. Asserting them against each other means adding a rung to one without the
    /// other fails here instead of in a run nobody watches.
    #[test]
    fn every_rung_this_agent_offers_has_a_word_on_its_command_line() {
        let brain = epoch_kernel::Brain::Agent {
            agent: ID.into(),
            model: String::new(),
        };
        for rung in crate::deliberation::scale(&brain) {
            assert!(
                effort_of(Some(*rung)).is_some(),
                "{rung:?} has no --effort word"
            );
        }
        // And the one it does not offer stays unsendable: mapping `off` to `low` would be Epoch
        // turning "do not deliberate" into "deliberate a little".
        assert_eq!(effort_of(Some(epoch_kernel::Reasoning::Off)), None);
        // Nothing asked for sends nothing, which is not the same as asking for the least.
        assert_eq!(effort_of(None), None);
    }

    /// Signed out is a different fact from not installed, and it must be *measured*.
    #[test]
    fn who_is_signed_in_is_read_rather_than_assumed() {
        let (signed_in, account) = read_whoami(
            r#"{"loggedIn":true,"authMethod":"claude.ai","email":"someone@example.com"}"#,
        );
        assert_eq!(signed_in, Some(true));
        assert_eq!(account.as_deref(), Some("someone@example.com"));

        let (out, none) = read_whoami(r#"{"loggedIn":false}"#);
        assert_eq!(out, Some(false));
        assert!(none.is_none(), "never an invented account");
    }

    /// An answer we cannot read is *not* an answer of "signed out".
    ///
    /// Telling somebody to sign in when they already are sends them to fix the wrong thing —
    /// and a renamed field in a future version would do exactly that if this guessed.
    #[test]
    fn an_unreadable_answer_means_unasked_rather_than_signed_out() {
        assert_eq!(read_whoami("not json"), (None, None));
        assert_eq!(read_whoami("{}"), (None, None));
        assert_eq!(read_whoami(r#"{"loggedIn":"maybe"}"#), (None, None));
    }

    /// The World may never put words in a character's mouth.
    ///
    /// A run that could not authenticate ends with a `result` event like any other, carrying
    /// *"Not logged in · Please run /login"* — and it exits **0**, so neither the exit code nor
    /// the emptiness check caught it. Read as an answer, it appeared in the Chronicle as Mage
    /// saying it. She did not, and no surface may claim she did.
    #[test]
    fn a_failure_is_a_failure_and_never_something_the_character_said() {
        let mut reading = Reading::default();

        read_event(
            r#"{"type":"result","subtype":"error_during_execution","is_error":true,
                "result":"Not logged in · Please run /login","session_id":"abc"}"#,
            &mut reading,
            &mut |_| {},
        );

        assert_eq!(
            reading.failed.as_deref(),
            Some("Not logged in · Please run /login")
        );
        assert!(reading.done.text.is_empty(), "nothing they said");
    }

    #[test]
    fn a_failure_with_no_reason_still_says_something_actionable() {
        // A blank error is the one thing worse than a wrong one: nothing to act on.
        let mut reading = Reading::default();
        read_event(
            r#"{"type":"result","is_error":true,"result":""}"#,
            &mut reading,
            &mut |_| {},
        );
        assert_eq!(
            reading.failed.as_deref(),
            Some("it ended without saying why")
        );
    }

    #[test]
    fn a_tool_with_nothing_worth_summarising_still_reports_that_it_ran() {
        // A blank line in the Terminal would read as nothing having happened.
        let mut reading = Reading::default();
        let mut seen = Vec::new();
        read_event(
            r#"{"type":"assistant","message":{"content":[
                {"type":"tool_use","id":"t2","name":"TodoWrite","input":{"todos":[]}}]}}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );
        read_event(
            r#"{"type":"user","message":{"content":[
                {"type":"tool_result","tool_use_id":"t2","content":"ok"}]}}"#,
            &mut reading,
            &mut |p| seen.push(p),
        );

        assert_eq!(
            reading
                .done
                .evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["TodoWrite"]
        );
        assert!(matches!(&seen[0], Progress::Ran { tool, .. } if tool == "TodoWrite"));
    }

    #[test]
    fn epochs_mode_is_passed_through_rather_than_left_to_the_default() {
        // Nearly left out, on the reasoning that an agent's autonomy is its own. True of an
        // agent somebody else started; false of one Epoch spawns, where the mode is an argument
        // — so passing nothing would be Epoch silently choosing, not Epoch staying out of it.
        assert_eq!(mode_of(epoch_kernel::Autonomy::Manual), "manual");
        assert_eq!(mode_of(epoch_kernel::Autonomy::AcceptEdits), "acceptEdits");
        assert_eq!(mode_of(epoch_kernel::Autonomy::Auto), "auto");
    }

    /// Against the real program, when there is one.
    ///
    /// Ignored by default: most machines have no Claude Code, and a test that fails there would
    /// be a test people learn to ignore. Run it deliberately —
    /// `cargo test -p epoch-engine -- --ignored claude_code` — on a machine that does.
    ///
    /// It exists because two of this file's flags were guesses that a real run refuted, and the
    /// next version will refute another. This is where that gets caught.
    /// **Two real answers, one for each shape the same command produced in one afternoon.**
    ///
    /// 2.1.247 wrote `Current model: Opus 5`; 2.1.265, installed an hour later, writes
    /// ``Current model: `Fable 5.1` `` — backticks around the name. Both are kept because both
    /// were seen, and a parser that only knows the newer one breaks for anybody who has not
    /// updated.
    #[test]
    fn the_model_a_name_resolves_to_survives_the_shape_it_arrives_in() {
        let said =
            |result: &str| serde_json::json!({ "is_error": false, "result": result }).to_string();
        let name = |raw: &str| {
            let answer = serde_json::from_str::<serde_json::Value>(raw).unwrap();
            let report = answer["result"].as_str().unwrap();
            let (_, rest) = report.split_once("Current model: ").unwrap();
            rest.split(" (effort")
                .next()
                .unwrap()
                .lines()
                .next()
                .unwrap()
                .trim()
                .trim_matches('`')
                .trim()
                .to_owned()
        };
        // 2.1.247
        assert_eq!(
            name(&said("Current model: Opus 5 (effort: medium)")),
            "Opus 5"
        );
        // 2.1.265
        assert_eq!(
            name(&said("Current model: `Fable 5.1` (effort: medium)")),
            "Fable 5.1"
        );
        // And a name that is a whole sentence keeps its parentheses when they are its own.
        assert_eq!(
            name(&said(
                "Current model: `Opus 5 (1M context)` (effort: medium)"
            )),
            "Opus 5 (1M context)"
        );
    }

    /// A real `/model` answer, recorded off Claude Code 2.1.247 on 2026-09-08.
    ///
    /// Kept verbatim: a fixture written from memory tests the memory.
    fn a_real_listing() -> String {
        serde_json::json!({
            "type": "result",
            "subtype": "success",
            "is_error": false,
            "num_turns": 0,
            "total_cost_usd": 0,
            "result": "Current model: Haiku 4.5 (effort: medium)\nUsage: /model <name>. \
        Available: sonnet, opus, haiku, fable, best, sonnet[1m], opus[1m], fable[1m], opusplan, \
        default, or a full model ID.",
        })
        .to_string()
    }

    /// **What the CLI names is what is offered — and the sentence around it is not.**
    ///
    /// Ten values, including four Epoch's compiled list could not have invented. The trailing
    /// *"or a full model ID"* is true and is not a model, so a picker must never show it.
    #[test]
    fn the_models_are_the_ones_the_program_named() {
        let found = read_listing(&a_real_listing()).expect("a list");
        let names: Vec<&str> = found.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "sonnet",
                "opus",
                "haiku",
                "fable",
                "best",
                "sonnet[1m]",
                "opus[1m]",
                "fable[1m]",
                "opusplan",
                "default",
            ]
        );
        // No label is invented: an alias is what a person types and it stands on its own.
        assert!(found.iter().all(|(id, label)| id == label));
    }

    /// **A sentence that changed shape extinguishes the list rather than half-reading it.**
    ///
    /// The same discipline as [`read_usage`] beside it. Half a parse of person-facing prose is
    /// how `or a full model ID` becomes something somebody can select.
    #[test]
    fn a_reshaped_answer_is_no_answer_rather_than_a_wrong_one() {
        let renamed = serde_json::json!({
            "is_error": false,
            "result": "Current model: Haiku 4.5. You may choose: sonnet, opus.",
        })
        .to_string();
        assert!(read_listing(&renamed).is_none(), "no landmark, no list");

        let failed = serde_json::json!({
            "is_error": true,
            "result": "Available: sonnet, opus",
        })
        .to_string();
        assert!(read_listing(&failed).is_none(), "an error is not a list");

        assert!(read_listing("not json at all").is_none());
        assert!(read_listing(&serde_json::json!({ "is_error": false }).to_string()).is_none());
    }

    /// **What this machine's Claude Code actually offers.** Opt-in: it starts the CLI.
    ///
    /// Not asserted against a list of names — that is the hardcoding this replaced, wearing a
    /// test. What must hold is that a real answer came back and that it is at least as complete
    /// as the compiled fallback, because a listing that returns *fewer* names than Epoch already
    /// had would be a downgrade nobody would notice.
    #[test]
    #[ignore = "needs Claude Code installed"]
    fn claude_code_actually_names_its_models_on_this_machine() {
        let found = models_offered(None).expect("`-p /model` should answer");
        assert!(!found.is_empty());
        for (id, label) in &found {
            assert!(!id.trim().is_empty(), "every row is addressable");
            assert!(!label.trim().is_empty(), "every row is readable");
            println!("{id:<12} {label}");
        }
        // **The label is the model, not the alias**, which is the whole of the change:
        // a picker offering `opus` beside Claude Code's own `Opus 5` was correct and
        // unreadable. Asserted on the *set* rather than on a name, because the name is
        // exactly the thing that must be free to change without editing this file.
        assert!(
            found.iter().any(|(id, label)| id != label),
            "at least one alias resolved to something a person recognises"
        );
        for (alias, _) in models() {
            assert!(
                found.iter().any(|(id, _)| id == alias),
                "the fallback's {alias} is still offered"
            );
        }
    }

    #[test]
    #[ignore = "needs Claude Code installed"]
    fn claude_code_is_actually_reachable_on_this_machine() {
        let found = ClaudeCode::default().probe();
        assert!(found.installed, "{:?}", found.note);
        assert!(found.version.is_some());
        println!("{} {}", found.name, found.version.unwrap_or_default());
    }

    /// The bug that got past every test in this file.
    ///
    /// A persona is several lines, and Windows refuses to hand a batch file an argument
    /// containing a newline at all — so every real turn died at `spawn` with *"batch file
    /// arguments are invalid"*, while the probe (`--version`, one word) passed happily. The
    /// unit tests asserted the *arguments*, which were right; nothing asserted that the program
    /// would accept them.
    ///
    /// So this asserts the spawn and nothing else. It is deliberately not a turn: it must not
    /// depend on somebody's quota, network or project folder to catch this again.
    #[test]
    #[ignore = "needs Claude Code installed"]
    fn an_argument_with_a_line_break_in_it_can_actually_be_passed() {
        let invocation = Invocation::default();
        let mut command = invocation.start();
        command.quiet();
        let spawned = command
            .arg(invocation.persona)
            .arg("first line\nsecond line")
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match spawned {
            Ok(mut child) => {
                let _ = child.wait();
            }
            Err(why) => panic!("could not even start `{}`: {why}", invocation.program),
        }
    }
}

#[cfg(test)]
mod one_program_two_accounts {
    use super::*;

    /// Every run of this program carries the account it is meant to be.
    ///
    /// **This is the whole mechanism, so this is the test that matters.** `CLAUDE_CONFIG_DIR` is
    /// undocumented — it is in no `--help` — and it was found by asking the program: pointed at an
    /// empty directory, 2.1.241 answered `{"loggedIn": false, "authMethod": "none"}`, wrote its own
    /// `.claude.json` inside it, and left the real sign-in untouched. Asked again a second later,
    /// the default still reported its own account.
    ///
    /// A spawn site that built its own `Command` would run the *default* account while the rest of
    /// Epoch believed it was running another one — a wrong answer with nothing on screen to
    /// suggest it. So every one of them goes through `start`, and this asserts what `start`
    /// actually hands the process.
    #[test]
    fn a_command_carries_the_account_it_belongs_to() {
        let home = std::path::PathBuf::from("/tmp/epoch-account-two");
        let mine = Invocation {
            home: Some(home.clone()),
            ..Invocation::default()
        };
        let command = mine.start();
        let carried: Vec<_> = command.get_envs().collect();
        assert!(
            carried.contains(&(
                std::ffi::OsStr::new(HOME_VAR),
                Some(std::ffi::OsStr::new(home.as_os_str()))
            )),
            "the account's directory has to reach the process: {carried:?}"
        );
    }

    /// And the account that was already there is left exactly as it was.
    ///
    /// `None` means *the program's own default*, and it must set nothing: an empty value or a
    /// guessed path would move somebody's existing sign-in out from under them.
    #[test]
    fn the_default_account_is_not_pointed_anywhere() {
        let command = Invocation::default().start();
        let carried: Vec<_> = command.get_envs().collect();
        assert!(
            !carried
                .iter()
                .any(|(key, _)| *key == std::ffi::OsStr::new(HOME_VAR)),
            "the default sign-in must be left alone: {carried:?}"
        );
    }
}
