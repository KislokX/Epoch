//! Agents: brains that **work**, not brains that answer (step 6.1, ADR-0027).
//!
//! ## Why this is not a `Provider`
//!
//! A [`Provider`](crate::provider::Provider) takes *one turn*: it is given a conversation, it
//! answers, and if it wants a tool it says so and stops — **Epoch owns the loop**. Every
//! capability call goes through `decide()` because Epoch is the one making it.
//!
//! An agent owns its own loop. You hand it an intention and it goes away and does the work,
//! deciding for itself what to read, what to run and when it is finished. There is no seam to
//! insert Epoch into, and forcing one would mean building a worse agent.
//!
//! That is not a shade of `Provider`; it is the opposite arrangement, and putting it behind the
//! same trait would mean `take_turn` sometimes returning tool calls to run and sometimes
//! returning a finished job. A caller that has to ask which kind it got is a caller with two
//! code paths and one type — which is the shape the enum exists to prevent (ADR-0027).
//!
//! ## What it is given, and what it is not
//!
//! Deliberately much less than a Provider gets. A Provider needs the whole composed
//! conversation because it cannot go and look; an agent **can** — Epoch's door is open to it, so
//! the crew, the Quest and this World's knowledge are a tool call away (step 5.2).
//!
//! So a [`Task`] is the intention, who is doing it, and where. Everything else, it fetches.
//! That is not a saving: it is the difference between briefing somebody and dictating to them.
//!
//! ## What Epoch keeps
//!
//! Its own permission model over its **own** tools, and nothing more. The agent's own file and
//! shell tools are its business and they are good ones — Epoch adds, it does not take away
//! (`CLAUDE.md`, 2026-08-07). Which means a character with an agent brain has the *agent's*
//! autonomy, and no Epoch mode may be shown for it.
//!
//! ## Shaped by one real implementation
//!
//! The same discipline `Provider` followed with Ollama: this trait is not designed in advance
//! for agents nobody has run. It is the shape [`crate::agents::claude`] actually needs, and it
//! will move when the second one does not fit.

use std::path::PathBuf;

/// Whether this machine actually has an agent, and which.
///
/// Measured, never assumed — the same rule as a Provider's probe. An agent that is offered and
/// then fails on first use is worse than one that was never on the list, because the failure
/// arrives after somebody has already given it work.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    /// Stable id. What a character's brain names.
    ///
    /// One per **account**, not per program: `claude-code` is the sign-in that was always there
    /// and `claude-code-2` is one somebody added. That is the whole of what lets one character
    /// work as one account while another works as a second.
    pub id: String,
    pub name: String,
    /// Which **program** this is a sign-in of.
    ///
    /// Carried because everything a program can be asked — its models, what each autonomy rung
    /// means, how to install it — belongs to the program and not to the account, and a surface
    /// keyed on `id` would answer nothing for every account after the first.
    pub kind: String,
    /// True only when the program answered.
    pub installed: bool,
    /// What it said it was. `None` when it is not there — never a guess.
    pub version: Option<String>,
    /// Where Epoch looked, so "not installed" is a fact somebody can go and check.
    pub looked_in: Option<String>,
    /// Why it is not available, in plain words.
    pub note: Option<String>,
    /// Whether it is signed in. `None` when the question could not be asked.
    ///
    /// Separate from `installed` because they are separate facts with separate fixes: one is
    /// solved by installing, the other by signing in, and a surface that collapsed them into
    /// "not available" would tell somebody to do the wrong thing.
    ///
    /// Learned the hard way. An installed, signed-out CLI answered *"Not logged in · Please run
    /// /login"* and Epoch had nowhere to put that, so it arrived as the character speaking.
    pub signed_in: Option<bool>,
    /// Who is signed in, as the agent reports it. Shown so the user can see *which* account —
    /// never a guess, and never a credential.
    pub account: Option<String>,
    /// Which sign-in **method** the user chose, when that is readable and the credential is not.
    ///
    /// A third thing, and deliberately not folded into the two above. `account` answers *who*
    /// and implies a working session; `signed_in` answers *does it work*. This answers only
    /// *what did they pick* — which is what remains readable when a CLI has no command to ask
    /// and asking anyway would cost the user a request.
    ///
    /// Its own field because collapsing it into `account` made an agent that reports a real
    /// account with an unreadable session look identical to one that reports no account and a
    /// readable choice. A test caught that immediately, which is the argument for the field.
    pub method: Option<String>,
}

impl AgentStatus {
    /// Not here, and why.
    pub fn missing(id: &str, name: &str, note: impl Into<String>) -> Self {
        Self {
            kind: crate::agents::kind_of(id).to_owned(),
            id: id.to_owned(),
            name: name.to_owned(),
            installed: false,
            version: None,
            looked_in: None,
            note: Some(note.into()),
            // Unasked, not unknown-and-assumed. A program that is not here cannot be signed in
            // *or* signed out, and saying either would be inventing an instrument.
            signed_in: None,
            account: None,
            method: None,
        }
    }
}

/// A picture travelling to an agent, ready to send.
///
/// Loaded by the caller, because reading the vault is the shell's job and this type is handed to
/// a program. `name` is only ever shown — an agent is told what the user called it, never where
/// Epoch keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedImage {
    /// What the user called it.
    pub name: String,
    /// The bytes themselves.
    pub bytes: Vec<u8>,
    /// What kind of picture, for the `data:` URI. Sniffed by the Engine when it was stored
    /// (ADR-0024) rather than taken from a filename.
    pub mime: &'static str,
}

/// The work, as an agent receives it.
///
/// Not a `Conversation`. That is what the Composer builds for **one turn** of a model
/// (ADR-0025's correction: a Conversation is the request, never the record) — and an agent does
/// not take turns. It is given the intention and the ground, and it goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Task {
    /// What the user actually asked for, in their words. Never rewritten.
    pub intent: String,
    /// Who is doing it. The agent works *as* somebody, and its calls are theirs.
    pub character: String,
    /// Their identity, as authored. The one thing that makes an agent this character rather
    /// than a generic one — the model is infrastructure, the Character is the product.
    pub persona: String,
    /// Who else is in this World, and the one thing a speaker must not do.
    ///
    /// Composed by [`crate::context::crew_note`] — the *same* sentence a model is given, because
    /// there is exactly one answer to how a crew works and it should not be written twice.
    ///
    /// Empty when nobody else lives here. It was empty *always* until now, which is how an agent
    /// asked to involve a colleague came to do the work itself and report that the colleague had
    /// done it: nothing had ever told it they existed.
    pub crew: String,
    /// How this crew does these jobs — the Skills this character was given.
    ///
    /// Composed by [`crate::context::skills_note`], the *same* words a model receives, for the
    /// same reason `crew` is: one answer, written once.
    ///
    /// Empty when nobody gave them any. It was empty *always* until the day somebody ticked a
    /// Skill for a character with an agent brain and watched it change nothing — the wording
    /// lived inside the Composer, and the Composer never runs on this path.
    pub skills: String,
    /// That this World has a library of notes, and that Epoch's tools are what reach it.
    ///
    /// Composed by [`crate::context::library_note`]. Written for both brains at once, because
    /// `crew` and `skills` each had to be fixed after the fact for having been written for one.
    ///
    /// Empty when the World has none.
    pub library: String,
    /// Pictures the user shared with this message, as bytes.
    ///
    /// **Bytes rather than paths, and that was measured.** Codex accepts a `data:` URI and takes
    /// a filesystem path *without complaint* — the app-server returns a turn, the run begins, and
    /// it fails minutes later upstream with somebody else's error message. Claude Code has no
    /// image flag at all and reads a base64 block off stdin. Neither wants a path, so a path is
    /// not what travels.
    ///
    /// Empty when nothing was shared, or when this agent does not take pictures — the caller
    /// decides, because whether a brain can see is a measured fact (`provider::Declared`) and not
    /// something this type should guess at.
    pub images: Vec<SharedImage>,
    /// What is connected to this World, and where the user fixes a connection that is not.
    ///
    /// Composed by [`crate::context::connections_note`]. **The text that exists because an agent
    /// got this wrong**, and which until now only a model received — see that function.
    ///
    /// Empty when nothing is configured.
    pub connections: String,
    /// Which model, in the **agent's** vocabulary — `opus`, not `gemma4:12b`.
    ///
    /// Empty means the agent's own choice stands. Epoch does not keep a list of somebody else's
    /// model names, because that list is wrong the day they add one.
    pub model: String,
    /// How hard to think, when the character asked for something in particular.
    ///
    /// `None` means the agent's own default stands — which is not the same as asking for the
    /// least. Canonical (ADR-0026): the rung is character identity and travels with them, and
    /// each brain maps it onto whatever its own command line offers.
    pub reasoning: Option<epoch_kernel::Reasoning>,
    /// Where to reach Epoch, when a door is open for this character.
    ///
    /// `None` means the agent works alone: its own tools, no crew, no Quest, and **no approval
    /// prompt** — Epoch cannot be asked about something it cannot be reached for.
    pub door: Option<Door>,
    /// Where to work. An agent with nowhere to work is refused before it starts.
    pub directory: PathBuf,
    /// Where a picture the agent makes itself is kept.
    ///
    /// **Not the Project Root, deliberately.** Codex draws with its own `image_gen` tool and
    /// hands the PNG back *inside the notification* — measured 2026-08-24: the turn left no file
    /// behind at all, so a picture Epoch does not write down does not exist. Writing it into the
    /// user's repository would put a generated asset in their source tree because of where the
    /// agent happened to be standing; it belongs with every other picture this World holds, which
    /// is also the only folder the Chronicle can show one from.
    ///
    /// Empty means nowhere to put one, and then nothing is written and nothing is claimed.
    pub pictures: PathBuf,
    /// Continue an earlier thread, when there is one.
    ///
    /// An agent keeps its own conversation state; Epoch keeps the handle rather than a copy.
    /// Copying it would give Epoch a second, drifting record of a history that is not its own.
    pub thread: Option<String>,
    /// How much it may do without asking.
    ///
    /// **Carried, because Epoch launches the process.** This was nearly left out on the
    /// reasoning that an agent's autonomy is its own — and that is true of an agent somebody
    /// else started. One Epoch spawns is different: the mode is an argument, so declining to
    /// pass it would not be humility, it would be Epoch silently choosing the default.
    ///
    /// Manual never becomes an unrestricted process. An agent that exposes a native permission
    /// callback asks Epoch through its door; Codex's read-only process uses Epoch capabilities
    /// for file changes, whose own calls ask before they run. The surface names that concrete
    /// path rather than pretending all agents implement the same protocol.
    pub autonomy: epoch_kernel::Autonomy,
}

/// How an agent reaches Epoch back.
///
/// Two fields and no logic: the agent is a separate process on this machine, and the only way
/// it can consult the crew, the Quest or the user is by calling in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Door {
    pub url: String,
    /// Loopback is not authentication (`endpoint.rs`), so every call carries this.
    pub token: String,
}

/// What is happening, while it happens.
///
/// The World must show work rather than a spinner (Build From Life). An agent that ran for two
/// minutes in silence is indistinguishable from one that hung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    /// Words, as they arrive.
    Said(String),
    /// How full its window is, as **it** measures it.
    ///
    /// An agent composes its own context, so Epoch's Composer has nothing to report for these
    /// turns and the gauge read `—` forever. That is the right reading for an instrument with
    /// nothing behind it — but there *is* something behind it: the agent reports its own token
    /// usage and window, and passing that through is a measurement rather than an estimate.
    ///
    /// Carries the **session** it is measuring, because "is this a new conversation?" turned out
    /// to be a question nobody could answer from the screen: a fresh Claude Code session starts
    /// near 26k tokens (its system prompt, its tool definitions, the project's CLAUDE.md), so
    /// "the number did not go to zero" looked exactly like "the old session was resumed". The id
    /// settles it — a different session is a different id, and the user can see it.
    Window {
        used: u64,
        budget: u64,
        session: Option<String>,
    },
    /// Which model actually did the thinking, **as the agent reports it afterwards**.
    ///
    /// Not the model Epoch asked for. Gemini CLI's default is `auto` and it routes: a measured
    /// run declared tools against `gemini-3.1-pro-preview-customtools` and then thought with
    /// `gemini-3-flash-preview`, and its opening line says only `"model":"auto"`. So the name a
    /// character carries is a *request*, and showing it as the answer would be the surface
    /// stating something it was never told.
    ///
    /// Emitted at the end rather than the start, because that is when the fact exists. An agent
    /// that does not report this emits nothing and the surface shows nothing — a cold instrument
    /// beats an invented reading, which is the rule this whole file is built on.
    Thought { model: String },
    /// It used one of its own tools, **and this is how that went**.
    ///
    /// Reported so the Terminal can show the work — Epoch did not authorise this and does not
    /// claim to have (`CLAUDE.md`, 2026-08-07); it is *witnessing*. Which is exactly why `ok`
    /// exists: witnessing a request and reporting it as a result is not witnessing, it is
    /// assuming. A refused write showed up in the Terminal as `Write …` while no file was ever
    /// created.
    Ran {
        tool: String,
        detail: String,
        ok: bool,
    },
}

/// More permission than this turn was given.
///
/// Deliberately about the need, not the transport. A CLI agent and an interactive app-server can
/// discover the same need at different points, but Epoch asks the person the same question and
/// never turns a one-turn answer into a standing permission.
///
/// ## Why there are four of these and not one
///
/// There was one — `Write` — and every agent question was labelled with it. Codex asks about a
/// **command** through `item/commandExecution/requestApproval` and about a file through
/// `item/fileChange/requestApproval`, and both arrived as *write access*; Gemini says
/// `"kind":"execute"` on the wire and that was thrown away. So a person could approve *write
/// access* and get `git init`.
///
/// ## And why not more than four
///
/// **Every variant here was seen on a wire.** ACP's vocabulary is wider — `delete`, `move`,
/// `fetch`, `search` — and Gemini was asked for a delete on 2026-09-08 and did not produce one,
/// so there is no `Delete` here. A named set grows the day somebody measures one, never the day
/// somebody reads a specification: *a control assembled from what is conceivable will offer
/// combinations that do not exist* (`CLAUDE.md`, 11.20).
///
/// [`Needs::Unstated`] is what an unmeasured kind becomes. It is not a gap — it is the honest
/// answer, and it is why adding `delete` to somebody else's agent tomorrow degrades to *did not
/// say* rather than to a confident *write access*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    /// To change files inside the Project Root.
    Write,
    /// To run a command on this machine.
    Execute,
    /// To use a tool belonging to a server Epoch does not own.
    ///
    /// Epoch has no descriptor for it and must not compose one, so the sentence shown is the
    /// agent's own (`CLAUDE.md`, 2026-08-08).
    Outside,
    /// The agent stopped to ask and did not say what kind of thing it is.
    ///
    /// Never folded into [`Needs::Write`]. Reading silence as the most common answer is the
    /// same invention as reading it as *no*, and it fails in the direction that looks familiar.
    Unstated,
}

impl Needs {
    /// How this reads in the question the user answers.
    pub fn as_str(self) -> &'static str {
        match self {
            Needs::Write => "write access",
            Needs::Execute => "to run a command",
            Needs::Outside => "a tool from a connected server",
            Needs::Unstated => "permission it did not describe",
        }
    }
}

/// One thing an agent has stopped to ask about.
///
/// The three facts travel together to one dialogue, so they are one value rather than a widening
/// list of positional arguments. [`Proposal::shown`] is the part that was missing: the diff or
/// the command itself, which the agent had already described and Epoch was dropping.
///
/// **A description tells you what will happen; this lets you notice that the wrong thing is
/// about to.** Approving *Writing to notes.txt* without the two lines that replace its contents
/// is approving a filename.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Proposal {
    /// What kind of thing this is, read from what the agent said rather than assumed.
    pub needs: Needs,
    /// What will happen, in the agent's own words.
    pub what: String,
    /// The change or the command itself, when the agent described it without doing it.
    ///
    /// `None` when the agent genuinely said nothing more than its title — which is a real state
    /// and reads as one. An empty string would claim there was a preview and that it was blank.
    pub shown: Option<String>,
}

impl Proposal {
    /// A proposal with nothing but a sentence, for an agent that offered nothing else.
    pub fn plain(needs: Needs, what: impl Into<String>) -> Self {
        Proposal {
            needs,
            what: what.into(),
            shown: None,
        }
    }
}

/// Who can grant one more permission for this turn.
///
/// This lives with [`Agent`] rather than with one turn runner because interactive agents need the
/// answer while they are still paused on the exact command or file change. The runner keeps using
/// it for any agent that only discovers a refusal after an attempt.
pub trait Approver {
    /// Ask, and wait. `false` covers an explicit refusal, a withdrawn question and a headless
    /// caller: an agent must never infer permission from silence.
    fn allow_for_this_turn(&self, proposal: &Proposal) -> bool;
}

/// Nobody to ask, so nothing is granted.
///
/// Useful to headless callers and tests. Silence is a refusal, never an approval.
pub struct NobodyToAsk;

impl Approver for NobodyToAsk {
    fn allow_for_this_turn(&self, _proposal: &Proposal) -> bool {
        false
    }
}

/// What an agent left behind.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Done {
    /// Everything it said, in order.
    pub text: String,
    /// The handle to continue this thread next time. `None` when it kept none.
    pub thread: Option<String>,
    /// Durable things that now exist because of this. What makes History real rather than
    /// narrated (ADR-0025) — an agent that produced none produced nothing.
    ///
    /// Each names the thing as well as describing it, so History can be followed rather than
    /// only read — the same shape a capability reports (`crate::capability::Made`).
    pub evidence: Vec<crate::capability::Made>,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("{0} is not installed on this machine")]
    Missing(String),
    #[error("{agent} could not be started: {why}")]
    Failed { agent: String, why: String },
    #[error("{agent} stopped: {why}")]
    Stopped { agent: String, why: String },
    #[error("this World has no project folder, and an agent works in one")]
    Nowhere,
}

/// A brain that works.
pub trait Agent: Send + Sync {
    /// Stable id. What a character's `agent` field names.
    fn id(&self) -> &str;

    /// Whether this machine has it. Allowed to reach the filesystem and run the program: a
    /// measured answer is a fact, and a constant in our source is a guess that goes stale.
    fn probe(&self) -> AgentStatus;

    /// Open the agent's own sign-in, in its own window.
    ///
    /// **Epoch never handles the credential.** It does not show a password field, does not read
    /// the browser, does not store a token: it starts the agent's official login and gets out of
    /// the way. The user signs in with the agent, exactly as they would have in a terminal — the
    /// only thing Epoch removes is having to know that a terminal was needed.
    ///
    /// Returns as soon as the window is open. Whether it *worked* is not this function's answer
    /// — [`Self::probe`] measures that afterwards, which is also why the surface has a "check
    /// again" rather than a spinner waiting on somebody else's browser.
    fn sign_in(&self) -> Result<(), AgentError> {
        Err(AgentError::Failed {
            agent: self.id().to_owned(),
            why: "this agent has no sign-in Epoch can open".into(),
        })
    }

    /// Hand over the work and let it run.
    ///
    /// `sink` is called as things happen, in order. `stopped` is checked between steps so the
    /// user's stop reaches a process that is not ours — the only way to interrupt something
    /// that owns its own loop is to ask it to stop between the parts we can see.
    fn work(
        &self,
        task: &Task,
        stopped: &dyn Fn() -> bool,
        approver: &dyn Approver,
        sink: &mut dyn FnMut(Progress),
    ) -> Result<Done, AgentError>;
}

/// Every agent this build knows how to host.
///
/// Deliberately a list rather than a configured set. A Provider is infrastructure the user
/// points at — an endpoint, a key — and there are many. An agent is a program that is installed
/// or is not, so the interesting question is *is it here*, which `probe` answers.
#[derive(Default)]
pub struct AgentRegistry {
    agents: Vec<Box<dyn Agent>>,
}

impl AgentRegistry {
    pub fn of(agents: Vec<Box<dyn Agent>>) -> Self {
        Self { agents }
    }

    pub fn get(&self, id: &str) -> Option<&dyn Agent> {
        self.agents.iter().find(|a| a.id() == id).map(AsRef::as_ref)
    }

    /// What is here, asked now.
    ///
    /// Every one, including the missing — a surface that only listed what was installed could
    /// not say *Claude Code is not installed*, which is the sentence somebody needs in order to
    /// go and install it.
    pub fn survey(&self) -> Vec<AgentStatus> {
        self.agents.iter().map(|a| a.probe()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Absent;
    impl Agent for Absent {
        fn id(&self) -> &str {
            "nowhere"
        }
        fn probe(&self) -> AgentStatus {
            AgentStatus::missing("nowhere", "Nowhere", "not installed")
        }
        fn work(
            &self,
            _task: &Task,
            _stopped: &dyn Fn() -> bool,
            _approver: &dyn Approver,
            _sink: &mut dyn FnMut(Progress),
        ) -> Result<Done, AgentError> {
            Err(AgentError::Missing("Nowhere".into()))
        }
    }

    #[test]
    fn a_registry_with_nothing_in_it_is_valid_and_says_so() {
        // A machine with no agent installed is the common case, not a failure.
        let registry = AgentRegistry::default();
        assert!(registry.is_empty());
        assert!(registry.survey().is_empty());
        assert!(registry.get("claude-code").is_none());
    }

    #[test]
    fn a_missing_agent_is_listed_rather_than_hidden() {
        // A surface that only listed what was installed could not say "Claude Code is not
        // installed" — which is the sentence somebody needs in order to go and install it.
        let registry = AgentRegistry::of(vec![Box::new(Absent)]);
        let found = registry.survey();

        assert_eq!(found.len(), 1);
        assert!(!found[0].installed);
        assert_eq!(found[0].note.as_deref(), Some("not installed"));
        assert!(found[0].version.is_none(), "never a guessed version");
    }

    #[test]
    fn an_agent_is_addressed_by_id_and_never_by_name() {
        let registry = AgentRegistry::of(vec![Box::new(Absent)]);
        assert!(registry.get("nowhere").is_some());
        assert!(registry.get("Nowhere").is_none());
    }
}
