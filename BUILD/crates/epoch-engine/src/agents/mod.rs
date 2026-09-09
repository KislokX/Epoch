//! Agents this build knows how to host (ADR-0027, step 6.2).
//!
//! One today, and the trait was shaped by it rather than for it — the same discipline
//! [`crate::provider`] followed with Ollama. The second implementation is what will show which
//! parts of [`crate::agent::Task`] were Claude Code's shape rather than an agent's.

pub mod claude;
/// The second agent, and the evidence that [`Agent`](crate::agent::Agent) was not shaped around
/// exactly one program (ADR-0027).
pub mod codex;
pub mod gemini;

use crate::agent::AgentRegistry;

/// What Epoch's own MCP server is called on an agent's side.
///
/// **One definition, because two would drift silently.** An agent pointed at a server under a
/// different name resolves nothing, asks nothing and reports nothing wrong — the failure looks
/// like a model that ignored its tools. It already lived here twice, spelled the same by hand.
pub const SERVER: &str = "epoch";

/// One rolling allowance window, reported by the signed-in agent that owns it.
///
/// Kept at the agent boundary rather than in a screen: Claude Code reports two windows while
/// Codex may report one. Neither is a turn's token count, and neither carries an account id or
/// credential into the UI.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanUsage {
    /// The amount still available, from 0 through 100.
    pub remaining_percent: u8,
    /// The amount this agent said has already been spent, from 0 through 100.
    pub used_percent: u8,
    /// The rolling window in minutes when the agent names one.
    pub window_minutes: Option<u64>,
    /// Unix seconds at which the window resets, when the agent supplies a machine timestamp.
    pub resets_at: Option<i64>,
    /// What is left to spend once the window is exhausted, when the agent keeps such a balance.
    ///
    /// **`None` is the ordinary answer**, and it is not zero. Claude Code reports subscription
    /// percentages and no balance at all; Codex reports one. An agent with no concept of credit
    /// must show nothing rather than an empty purse.
    ///
    /// This exists because a window at 0% was true and read as "this character is finished",
    /// while the agent went on working out of a balance the screen never mentioned. A reading
    /// that is accurate and leads to the wrong conclusion is still the wrong instrument.
    pub credits: Option<Credits>,
}

/// What an agent says is left to spend beyond its window.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Credits {
    /// The balance, as the agent counts it. **Not assumed to be money** — the agent names no
    /// currency, so neither does Epoch.
    pub balance: f64,
    /// The agent said the balance does not run out. Rendered as such, never as a large number.
    pub unlimited: bool,
}

/// One sign-in of one agent program.
///
/// ## Why this exists, when it once did not
///
/// The old comment here said an agent has *nothing to configure*: a program is installed or it is
/// not. That was true of the **program** and never of the **account**, and the difference stopped
/// being academic the moment somebody wanted a work login and a personal one — or simply two, so
/// that two characters can work at the same time without sharing one allowance.
///
/// Both programs support it, and both were measured rather than remembered. `CLAUDE_CONFIG_DIR`
/// and `CODEX_HOME` each isolate a sign-in completely: pointed at an empty directory the CLI
/// reports *not logged in* while the real one, asked a second later, still reports its account.
///
/// **The label is the user's word and never a measurement.** Claude Code will say which email is
/// signed in (`auth status --json`) and Epoch shows that beside the label; Codex has no way to be
/// asked at all — `login status` says only *"Logged in using ChatGPT"* — so for that one the
/// label is all there is, and inventing an address would be the invented gauge the Launcher
/// forbids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    /// Which program: [`claude::ID`], [`codex::ID`], [`gemini::ID`].
    pub kind: String,
    /// Stable id for this sign-in. Equal to `kind` for the one that was already there.
    ///
    /// A character's brain names an id (`Brain::Agent { agent, .. }`), so this is the whole of
    /// what lets one character work as one account while another works as a second.
    pub id: String,
    /// What a person reads on the row.
    pub name: String,
    /// Where this sign-in lives. `None` is the program's own default — the account that existed
    /// before Epoch knew there could be more than one.
    pub home: Option<std::path::PathBuf>,
}

impl Account {
    /// The account every machine starts with: the program's own sign-in, under its own name.
    pub fn default_of(kind: &str) -> Self {
        Self {
            kind: kind.to_owned(),
            id: kind.to_owned(),
            name: name_of(kind).unwrap_or(kind).to_owned(),
            home: None,
        }
    }
}

/// The accounts a machine has before anybody adds one: each program's own sign-in.
pub fn default_accounts() -> Vec<Account> {
    [claude::ID, codex::ID, gemini::ID]
        .into_iter()
        .map(Account::default_of)
        .collect()
}

/// Every agent, ready to be asked whether it is here.
pub fn installed() -> AgentRegistry {
    installed_for(&default_accounts())
}

/// Every **account**, ready to be asked whether it is here and who is signed into it.
///
/// One `Agent` per account rather than per program. Two Claude Code entries are two processes
/// with two `CLAUDE_CONFIG_DIR`s, which is why they can work at the same time and why each
/// reports its own allowance.
///
/// An account naming a program this build does not know is skipped rather than guessed at.
pub fn installed_for(accounts: &[Account]) -> AgentRegistry {
    AgentRegistry::of(
        accounts
            .iter()
            .filter_map(|account| -> Option<Box<dyn crate::agent::Agent>> {
                match (account.kind.as_str(), account.home.clone()) {
                    (claude::ID, None) => Some(Box::new(claude::ClaudeCode::default())),
                    (claude::ID, Some(home)) => Some(Box::new(claude::ClaudeCode::account(
                        &account.id,
                        &account.name,
                        home,
                    ))),
                    (codex::ID, None) => Some(Box::new(codex::Codex::default())),
                    (codex::ID, Some(home)) => Some(Box::new(codex::Codex::account(
                        &account.id,
                        &account.name,
                        home,
                    ))),
                    // **One only, and measured.** Gemini CLI signs in with an API key and its own
                    // probe already reports that it has no way to be asked whether that key still
                    // works. A second entry would be a second row nobody could tell apart.
                    (gemini::ID, None) => Some(Box::new(gemini::Gemini)),
                    _ => None,
                }
            })
            .collect(),
    )
}

/// What an agent is called, without asking whether it is here.
///
/// Separate from `installed()` on purpose: that one **runs each program**, and a surface showing
/// a name should not start a process to learn it. A character's file names `claude-code`; a
/// person reads *Claude Code*.
/// What an agent calls its own models, for a surface that suggests rather than enumerates.
///
/// **Asked of the Engine, because the answer differs per agent** — Claude Code takes aliases like
/// `opus`, Codex takes `gpt-5.6-sol`. A list in a component was correct while there was one agent
/// and became wrong the moment there were two, silently: the second agent would have been offered
/// the first one's models.
/// What a failed program said on the other pipe.
///
/// **Because "said nothing" was a lie we told twice.** A malformed command line makes Codex exit 2
/// with a perfectly clear sentence on stderr — `error: unexpected argument '--sandbox' found` —
/// and Epoch reported *"it exited with exit code: 2 and said nothing"*. The program said plenty;
/// nobody was reading that pipe.
///
/// Trimmed to a few lines: a stack trace in a Chronicle is not an explanation, and the first line
/// of an argument error is the whole of it.
pub(crate) fn last_words(child: &mut std::process::Child) -> Option<String> {
    use std::io::Read;

    let mut said = String::new();
    child.stderr.take()?.read_to_string(&mut said).ok()?;

    let tidy: Vec<&str> = said
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .collect();

    (!tidy.is_empty()).then(|| tidy.join(" · "))
}

/// What this agent's gate actually does under a given mode.
///
/// **Because the sentence has to name this agent's actual path to permission.** Claude Code asks
/// Epoch before each of its own tools. Codex cannot expose that callback in `exec`, so Manual
/// Codex keeps its native sandbox read-only and routes file changes through Epoch's MCP door.
/// The door asks about the concrete `write_file`, `edit_file` or `delete_file` call before it
/// performs it. Calling either path the other one is a gauge nobody can explain.
///
/// In the Engine because it is a fact about a program, not about a screen, and because the two
/// agents genuinely differ.
/// Which **program** an account id belongs to.
///
/// ## Why this is not parsing a name
///
/// Everything a program can be asked — which models it takes, what each autonomy rung means, what
/// it is called — belongs to the program and not to the sign-in. Once one program can have several
/// accounts, a lookup keyed on the account id answers `None` for every one of them but the first.
///
/// Measured as a defect waiting to happen: `models_for("claude-code-2")` fell straight through to
/// the empty list, so the second account would have been offered no models at all with nothing on
/// screen to say why.
///
/// The id is Epoch's own encoding — [`Account::id`] is either a kind or a kind this crate gave a
/// suffix to — so resolving it against the list of kinds is reading back what was written, not
/// guessing at a filename. An id belonging to no known kind is returned unchanged.
pub fn kind_of(id: &str) -> &str {
    for kind in [claude::ID, codex::ID, gemini::ID] {
        if id == kind
            || id
                .strip_prefix(kind)
                .is_some_and(|rest| rest.starts_with('-'))
        {
            return kind;
        }
    }
    id
}

// **Shared, because both adapters need them and a helper typed twice drifts.**
//
// These were Codex's alone until Gemini's transport also had to say why a run failed. Both
// decide what a person is shown when somebody else's program refuses, which is exactly the
// kind of sentence that must not exist in two spellings.

/// What the program complained about, bounded, or nothing.
///
/// **Bounded because it is somebody else's output and this ends up in a Chronicle.** Codex is
/// chatty on start-up; the last few lines are where a failure is, and a wall of them would bury
/// the sentence they are attached to. `None` rather than an empty string, so a caller cannot
/// print a dangling separator — the same shape `because_of` uses on the Bridge.
pub(crate) fn said(complaints: &std::sync::Mutex<Vec<String>>) -> Option<String> {
    let Ok(held) = complaints.lock() else {
        return None;
    };
    let tail: Vec<&str> = held
        .iter()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty())
        .rev()
        .take(4)
        .collect();
    if tail.is_empty() {
        return None;
    }
    Some(
        tail.into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" · ")
            .chars()
            .take(400)
            .collect(),
    )
}

/// What a JSON-RPC reply refused with, in the program's own words.
///
/// `error.message` is where a server says why; `error.data` is where it sometimes says more.
/// Both are Codex's own text, which makes them evidence — the same reason `blame` passes
/// ComfyUI's refusal through and the same reason the Bridge passes none of `ureq`'s.
///
/// Bounded, because it ends up in a Chronicle.
pub(crate) fn refusal_in(message: &serde_json::Value) -> Option<String> {
    let error = message.get("error")?;
    let said = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .or_else(|| Some(error.to_string()))?;
    let more = error
        .get("data")
        .filter(|it| !it.is_null())
        .map(std::string::ToString::to_string);
    let whole = match more {
        Some(more) if !said.contains(more.trim_matches('"')) => format!("{said} — {more}"),
        _ => said,
    };
    Some(whole.chars().take(400).collect())
}

pub fn gate(id: &str, autonomy: epoch_kernel::Autonomy) -> Option<&'static str> {
    use epoch_kernel::Autonomy::*;
    // The program, never the account: what a rung *means* belongs to the program.
    match (kind_of(id), autonomy) {
        // Invited to the decision by the agent itself (`--permission-prompt-tool`).
        (claude::ID, Manual) => Some("asks Epoch before each of its own tools, and waits for you"),
        (claude::ID, AcceptEdits) => Some("writes without asking; anything heavier still asks"),
        (claude::ID, Auto) => Some("uses its own tools freely"),
        (codex::ID, Manual) => Some("uses Epoch file tools and asks before each change"),
        (codex::ID, AcceptEdits | Auto) => {
            Some("works inside the project folder; it is sandboxed there, and never asks")
        }
        // **This said Gemini CLI has no permission callback. It has one, and it was measured
        // on 2026-09-08.** `gemini --acp` speaks the Agent Client Protocol and sends the client
        // `session/request_permission` before running its own tool, with the tool call, a diff,
        // and three options — `allow_always`, `allow_once`, `reject_once`. Answered
        // `reject_once` by hand, the file was not written. That is the same arrangement Codex
        // has and exactly what `Manual` is supposed to mean.
        //
        // The sentence below is what is true of the transport Epoch uses **today**, which is
        // `--prompt` with `--approval-mode`. It has been narrowed because the old one was a
        // promise this build cannot keep: asked in Manual to write a file, Gemini wrote it —
        // `plan` is its own read-only mode, and it did not hold. Epoch was never asked, because
        // this transport has nobody to ask.
        //
        // A gauge nobody can explain is worse than no gauge, and one that promises a guard that
        // is not there is worse than both. So it says who is deciding, and it is not Epoch.
        (gemini::ID, Manual) => {
            Some("is asked to only read and plan — Epoch cannot enforce it, and is not consulted")
        }
        (gemini::ID, AcceptEdits) => Some("edits files without asking; it asks Epoch nothing"),
        (gemini::ID, Auto) => Some("uses its own tools freely; it asks Epoch nothing"),
        _ => None,
    }
}

/// What one account's agent calls its own models.
///
/// **Asked, then remembered as a fallback.** All three programs will say, and none of them says
/// so in `--help`: Codex answers `model/list` over its app-server, Gemini names them in
/// `session/new`, and Claude Code prints them from `-p /model` for nothing — `num_turns: 0`, no
/// tokens, no API time.
///
/// **The third one was written down here as impossible, and that is the lesson.** An earlier
/// pass measured `--help`, found no `models` subcommand, and recorded that Claude Code *has no
/// listing* — as a measured fact, in `CLAUDE.md`. `--help` answers which **subcommands** exist;
/// a slash command in `-p` is a different question, and `claude.rs` had been reading `/usage`
/// exactly that way for months. The measurement was real and it answered the wrong question,
/// which is the failure mode this codebase already has a section about, arriving one layer in.
///
/// The cost of the wrong answer was ten values reduced to four: `/model` names `best`,
/// `opusplan`, `default` and the long-window `sonnet[1m]` alongside the aliases, and a
/// character cannot be pointed at a name its picker never showed.
///
/// A brand-new *family* nobody has shipped is still answered by the picker's "name it myself"
/// field rather than by probing — a control assembled from what is conceivable will offer
/// combinations that do not exist.
///
/// **A program nobody could ask keeps its suggestions.** Not installed, not signed in, a release
/// that no longer speaks the protocol: the fallback list is shown and the field stays typeable.
/// Answering *there are no models* to a question nobody could put is the inversion this codebase
/// keeps paying for.
pub fn models_for(account: &Account) -> Vec<(String, String)> {
    if let Some(asked) = asked_recently(account) {
        return asked;
    }
    let measured = match account.kind.as_str() {
        claude::ID => claude::models_offered(account.home.clone()),
        codex::ID => codex::models_offered(account.home.as_deref()),
        gemini::ID => gemini::models_offered(),
        _ => None,
    };
    let answer = measured.unwrap_or_else(|| {
        suggested(&account.kind)
            .iter()
            .map(|(id, label)| ((*id).to_owned(), (*label).to_owned()))
            .collect()
    });
    remember(account, &answer);
    answer
}

/// What each program called its models when this was written — **the fallback, never the answer.**
fn suggested(kind: &str) -> &'static [(&'static str, &'static str)] {
    match kind_of(kind) {
        claude::ID => claude::models(),
        codex::ID => codex::models(),
        // Deliberately empty. Gemini's own default is `auto` and it routes; a list written here
        // would be Epoch second-guessing a router it cannot see. When the CLI can be asked it
        // says exactly what it offers, and when it cannot there is nothing honest to put here.
        _ => &[],
    }
}

/// Asking costs a short-lived process — a few seconds for Gemini, which starts Node twice — and
/// the picker asks every time a person opens a character. So the answer is kept for a while.
///
/// **Short on purpose.** The whole point is that a model shipped this morning appears today, so
/// a cache that outlives the session would reintroduce the staleness this replaced. Ten minutes
/// is long enough that opening the same panel twice is instant and short enough that installing
/// a new CLI mid-session is noticed without restarting Epoch.
const STAYS_FRESH: std::time::Duration = std::time::Duration::from_secs(600);

type Remembered = std::collections::HashMap<String, (std::time::Instant, Vec<(String, String)>)>;

fn recent() -> &'static std::sync::Mutex<Remembered> {
    static ASKED: std::sync::OnceLock<std::sync::Mutex<Remembered>> = std::sync::OnceLock::new();
    ASKED.get_or_init(|| std::sync::Mutex::new(Remembered::new()))
}

/// Per **account**, because what a sign-in may reach is a property of that sign-in. Two Codex
/// accounts on one machine are two answers, and drawing one of them on both rows would be a real
/// reading of the wrong quantity.
fn asked_recently(account: &Account) -> Option<Vec<(String, String)>> {
    let held = recent().lock().ok()?;
    let (when, found) = held.get(&account.id)?;
    (when.elapsed() < STAYS_FRESH).then(|| found.clone())
}

fn remember(account: &Account, found: &[(String, String)]) {
    if let Ok(mut held) = recent().lock() {
        held.insert(
            account.id.clone(),
            (std::time::Instant::now(), found.to_vec()),
        );
    }
}

/// The signed-in agent account's plan allowance, when that agent exposes one as a local
/// machine-readable reading.
///
/// This is intentionally a narrow side channel rather than a field added to `AgentStatus`:
/// presence is quick and useful in several panels, while a plan reading may start a separate
/// local protocol. Calling `list_agents` must never make every connection screen wait for that.
pub fn plan_usage_for(account: &Account) -> Option<Vec<PlanUsage>> {
    match account.kind.as_str() {
        // Per account, because an allowance belongs to a sign-in and not to a program. Two
        // Claude Code accounts have two separate windows, and showing one of them twice would be
        // a real reading of the wrong quantity.
        claude::ID => claude::plan_usage(account.home.clone()),
        codex::ID => codex::plan_usage(account.home.as_deref()).map(|reading| vec![reading]),
        _ => None,
    }
}

/// What a program is called.
///
/// The **program**, which is why an added account resolves to the same name: `claude-code-2` is
/// still Claude Code. The user's own label for that sign-in lives in `Account::name`, and a
/// surface that has one should prefer it — this is the fallback for the places that only ever
/// have an id.
pub fn name_of(id: &str) -> Option<&'static str> {
    match kind_of(id) {
        claude::ID => Some(claude::NAME),
        codex::ID => Some(codex::NAME),
        gemini::ID => Some(gemini::NAME),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::Autonomy;

    #[test]
    fn codex_manual_routes_file_changes_through_epochs_door() {
        assert_eq!(
            gate(codex::ID, Autonomy::Manual),
            Some("uses Epoch file tools and asks before each change")
        );
    }
}

#[cfg(test)]
mod several_accounts_of_one_program {
    use super::*;

    /// Everything a *program* can be asked must survive the account id.
    ///
    /// **The defect this closes, found while wiring it rather than after:** `models_for` and
    /// `gate` matched on the agent id, and the id stopped being the program's the moment a second
    /// sign-in existed. `models_for("claude-code-2")` fell straight through to the empty list, so
    /// the second account would have been offered no models at all — with nothing on screen to
    /// say why, which is the shape of failure this codebase keeps finding.
    #[test]
    fn a_second_account_is_still_the_same_program() {
        assert_eq!(kind_of("claude-code-2"), claude::ID);
        assert_eq!(kind_of("codex-3"), codex::ID);
        // The default is its own kind.
        assert_eq!(kind_of(claude::ID), claude::ID);
        // Something this build has never heard of comes back untouched rather than guessed at.
        assert_eq!(kind_of("something-else"), "something-else");

        // The suggestions are the program's, so a second sign-in has the same ones. Asserted on
        // `suggested` rather than `models_for`, because `models_for` now *asks the program* and
        // a unit test must not depend on what is installed on the machine running it.
        assert_eq!(suggested("claude-code-2"), suggested(claude::ID));
        assert!(!suggested("claude-code-2").is_empty(), "a real list");
        assert_eq!(
            gate("codex-2", epoch_kernel::Autonomy::Manual),
            gate(codex::ID, epoch_kernel::Autonomy::Manual)
        );
        assert_eq!(name_of("claude-code-2"), Some(claude::NAME));
    }

    /// **A program nobody could ask keeps its suggestions; one Epoch never heard of has none.**
    ///
    /// The two halves are different facts and the second must not be spelled as the first. An
    /// agent this build does not know about has nothing to suggest — there is no list to be
    /// stale. A known agent that could not be reached has one, and showing an empty picker there
    /// would read as *this agent has no models*, which is the inversion that has cost this
    /// codebase a section in `CLAUDE.md` four times.
    #[test]
    fn a_program_that_cannot_be_asked_still_offers_what_it_had() {
        assert!(suggested("something-else").is_empty(), "nothing to suggest");
        assert!(!suggested(claude::ID).is_empty());
        assert!(!suggested(codex::ID).is_empty());
        // Gemini is deliberately empty: when it can be asked it says exactly what it offers, and
        // when it cannot there is nothing honest to put here.
        assert!(suggested(gemini::ID).is_empty());
    }

    /// One `Agent` per account, each with its own id.
    #[test]
    fn each_account_becomes_its_own_agent() {
        let accounts = vec![
            Account::default_of(claude::ID),
            Account {
                kind: claude::ID.to_owned(),
                id: "claude-code-2".to_owned(),
                name: "work".to_owned(),
                home: Some(std::path::PathBuf::from("/tmp/epoch-test-account")),
            },
        ];
        let registry = installed_for(&accounts);
        assert!(registry.get(claude::ID).is_some());
        assert!(
            registry.get("claude-code-2").is_some(),
            "a character's brain names this id; nothing resolves without it"
        );
        assert!(registry.get("claude-code-3").is_none());
    }

    /// An account naming a program this build does not know is skipped, never guessed at.
    #[test]
    fn an_unknown_program_is_left_out_rather_than_invented() {
        let registry = installed_for(&[Account {
            kind: "some-future-agent".to_owned(),
            id: "some-future-agent".to_owned(),
            name: "whatever".to_owned(),
            home: None,
        }]);
        assert!(registry.is_empty());
    }
}
