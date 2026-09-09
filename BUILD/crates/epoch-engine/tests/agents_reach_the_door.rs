//! Does a real agent actually reach Epoch's real door?
//!
//! Everything else about this wiring is unit-tested — the config Codex is handed, the mode the
//! gate runs at, the shape of a served reply. None of that answers the only question that
//! matters, and the gap is exactly where the bug lived: the arguments were right, the schema
//! accepted them, and the calls still came back refused.
//!
//! So this runs the parts that cannot be mocked into agreement. Epoch's own `endpoint` listens
//! on a real socket, serving a real `Serving` over a real registry, and the turn is driven
//! through [`Agent::work`] — the same call the shell makes. Nothing between them is a stand-in,
//! and the capability sets a flag when it runs, so the assertion is *Epoch executed something*
//! rather than *the model said it did* — a distinction this project has already been caught by.
//!
//! **Through `app-server`, deliberately.** A first version drove `codex exec`, which passed and
//! proved less than it looked: `exec` and `app-server` are different transports, and Epoch uses
//! the second. Proving the configuration is accepted by the one Epoch does not use is the kind
//! of assumption that caused the bug this file exists for.
//!
//! ## Why it is `#[ignore]`d
//!
//! It needs Codex installed, signed in, and a network round trip to a model, and it spends the
//! user's plan allowance. A test with those requirements that runs by default is a test suite
//! that fails on somebody else's machine for reasons that are not about the code.
//!
//! ## Both agents, one file
//!
//! Two implementations of the same question, sharing one harness. Codex's was written first,
//! after a defect; Claude Code's exists because *working today* is not evidence — its door is
//! configured a different way, through a different flag, and nothing would have noticed the two
//! drifting apart. The asymmetry is real and worth stating: Epoch confines Codex to a sandbox
//! and gives Claude Code none, so these are not the same code path wearing two names.
//!
//! Run them deliberately:
//!
//! ```text
//! cargo test -p epoch-engine --test agents_reach_the_door -- --ignored --nocapture
//! ```

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use epoch_engine::agent::{Agent, NobodyToAsk, Progress, Task};
use epoch_engine::agents::claude::ClaudeCode;
use epoch_engine::agents::codex::Codex;
use epoch_engine::capability::{Capability, CapabilityError, Outcome};
use epoch_engine::serve::{self, Serve, Serving};
use epoch_engine::{endpoint, CapabilityRegistry};
use epoch_kernel::{Arguments, CapabilityId, Descriptor};
use serde_json::Value;

/// The word the capability returns, and nothing else in this test says it.
///
/// Deliberately not a word a model could produce by being helpful: if it appears in the reply,
/// it travelled from this process, through the door, to Codex and back.
const PASSWORD: &str = "zarpaste";

/// A capability that leaves a mark when it runs.
struct Watchword(Arc<AtomicBool>);

impl Capability for Watchword {
    fn describe(&self) -> Descriptor {
        Descriptor::observing(
            CapabilityId::new("epoch_watchword").unwrap(),
            "Returns this World's watchword. Takes no arguments.",
        )
    }

    fn run(&self, _arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        self.0.store(true, Ordering::SeqCst);
        Ok(Outcome::told(PASSWORD))
    }
}

/// A capability that does nothing and remembers what it was asked for.
///
/// **Nothing is generated.** The question this answers is whether an agent can see and reach one
/// of *Epoch's* capabilities through Epoch's door, with the arguments it chose — not whether any
/// particular capability works, which each has its own test for. A fake here keeps the test
/// honest about what it proves and keeps it runnable on a machine with no GPU.
struct Pretend(Arc<std::sync::Mutex<Option<String>>>);

impl Capability for Pretend {
    fn describe(&self) -> Descriptor {
        Descriptor::observing(
            CapabilityId::new("epoch_note").unwrap(),
            "Writes a note in this World's log.",
        )
        .taking([
            epoch_kernel::Parameter::required(
                "about",
                epoch_kernel::ValueKind::Text,
                "What the note is about.",
            ),
            epoch_kernel::Parameter::optional(
                "tone",
                epoch_kernel::ValueKind::Text,
                "How it should be written.",
            ),
        ])
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let about = arguments
            .text("about")
            .map_err(CapabilityError::BadArguments)?;
        let tone = arguments.optional_text("tone").unwrap_or("none");
        *self.0.lock().unwrap() = Some(format!("{about} | {tone}"));
        Ok(Outcome::told("Noted."))
    }
}

/// A capability that *changes* something, so a cautious mode has to ask about it.
struct Marking(Arc<AtomicBool>);

impl Capability for Marking {
    fn describe(&self) -> Descriptor {
        Descriptor::acting(
            CapabilityId::new("epoch_mark").unwrap(),
            "Writes this World's watchword down. Takes no arguments.",
            [epoch_kernel::Effect::Writes],
            epoch_kernel::Reversal::Permanent,
        )
    }

    fn run(&self, _arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        self.0.store(true, Ordering::SeqCst);
        Ok(Outcome::told(PASSWORD))
    }
}

/// Epoch's side of the door: judge, then run — and wait for a person when the verdict says to.
struct Door {
    registry: CapabilityRegistry,
    character: epoch_kernel::CharacterId,
    mode: epoch_kernel::Autonomy,
    /// How long a person takes to answer, simulated. `None` means nothing here ever asks.
    answering_after: Option<std::time::Duration>,
}

impl endpoint::Answering for Door {
    fn answer(&self, message: &Value) -> Option<Value> {
        let serving = Serving {
            registry: &self.registry,
            character: Some(&self.character),
            world: "archipelago",
            mode: self.mode,
            policies: &[],
        };
        match serving.handle(message) {
            Serve::Silent => None,
            Serve::Reply(reply) => Some(reply),
            // **The agent asking about one of its *own* tools**, which every Claude Code turn
            // now does: the permission tool is named unconditionally since 2026-08-24, because
            // `--permission-mode auto` stopped meaning "never ask" (see `agents/claude.rs`).
            //
            // Answered exactly as the shell answers it (`epoch-tauri/src/agent.rs`,
            // `decide_for_them`): under `Auto` an immediate yes with the agent's own arguments
            // handed back untouched. Refusing here left the agent reporting that Epoch's tools
            // "aren't available in this context", which is what this file exists to notice.
            Serve::Approve(asked) => Some(match self.mode {
                epoch_kernel::Autonomy::Auto => serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": asked.id,
                    "result": { "content": [{ "type": "text", "text": serde_json::json!({
                        "behavior": "allow",
                        "updatedInput": asked.input,
                    }).to_string() }] }
                }),
                _ => serve::refusal(asked.id, "nobody to ask"),
            }),
            Serve::HandOver(asked) => Some(serve::refusal(asked.id, "not part of this test")),
            Serve::Run(run) => {
                let id = run.id.clone();
                // The whole question this file was extended to answer: an Epoch tool whose
                // verdict is `Ask` blocks on a socket until a person decides. Simulated with a
                // sleep, because what is being measured is whether the *agent* tolerates the
                // wait — not how fast anybody clicks.
                if matches!(run.verdict, epoch_kernel::Verdict::Ask(_)) {
                    match self.answering_after {
                        Some(pause) => std::thread::sleep(pause),
                        None => return Some(serve::refusal(id, "nobody to ask")),
                    }
                }
                match self.registry.get(&run.capability) {
                    Some(capability) => match capability.run(&run.arguments) {
                        Ok(outcome) => Some(serve::produced(id, &outcome.content)),
                        Err(why) => Some(serve::refusal(id, why.to_string())),
                    },
                    None => Some(serve::refusal(id, "gone")),
                }
            }
        }
    }
}

/// A user who says yes. `NobodyToAsk` is the headless default and refuses everything, which is
/// right for a caller with nobody to ask — and wrong for measuring what happens when somebody
/// *is* there.
struct SaysYes;

impl epoch_engine::agent::Approver for SaysYes {
    fn allow_for_this_turn(&self, _proposal: &epoch_engine::agent::Proposal) -> bool {
        true
    }
}

/// The Task Epoch builds, pointed at a door this test is hosting.
fn asking_for(tool: &str, door: &endpoint::Open, autonomy: epoch_kernel::Autonomy) -> Task {
    Task {
        intent: format!(
            "Call the {tool} tool from Epoch and reply with only the word it returns. Do not \
             guess it."
        ),
        character: "Mage".into(),
        persona: "You are careful and you use the tools you are given.".into(),
        crew: String::new(),
        skills: String::new(),
        library: String::new(),
        connections: String::new(),
        images: Vec::new(),
        model: String::new(),
        reasoning: None,
        door: Some(epoch_engine::agent::Door {
            url: door.url(),
            token: door.token().as_str().to_owned(),
        }),
        directory: std::env::temp_dir(),
        pictures: std::path::PathBuf::new(),
        thread: None,
        autonomy,
    }
}

/// Host the door, run one turn, and say what came back and whether Epoch really ran anything.
///
/// `answering_after` is how long a person takes to decide, when the mode makes Epoch ask.
fn one_turn(
    agent: &dyn Agent,
    tool: &str,
    autonomy: epoch_kernel::Autonomy,
    answering_after: Option<std::time::Duration>,
    approver: &dyn epoch_engine::agent::Approver,
) -> (String, bool) {
    let ran = Arc::new(AtomicBool::new(false));
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(Watchword(Arc::clone(&ran))));
    registry.register(Box::new(Marking(Arc::clone(&ran))));

    // Port 0: the operating system picks. A fixed port fails when something else has it, which
    // is a failure about the machine rather than about the door.
    let door = endpoint::open(
        0,
        endpoint::Token::fresh(),
        Door {
            registry,
            character: epoch_kernel::CharacterId::new("mage").unwrap(),
            mode: autonomy,
            answering_after,
        },
    )
    .expect("the door opens on loopback");

    let mut said = String::new();
    let done = agent
        .work(
            &asking_for(tool, &door, autonomy),
            &|| false,
            approver,
            &mut |progress| {
                if let Progress::Said(words) = progress {
                    said.push_str(&words);
                }
            },
        )
        .expect("the turn runs");
    said.push_str(&done.text);
    (said, ran.load(Ordering::SeqCst))
}

/// Refuse to start when the agent could not answer for itself.
///
/// Two facts, never one — the same separation [`epoch_engine::agent::AgentStatus`] keeps, for the
/// same reason: installed and signed in have different fixes. An installed CLI whose token has
/// expired fails *deep inside a turn* with `401 OAuth access token has expired`, which sends
/// whoever reads it looking at the door rather than at their own login. Measured here instead,
/// before a model is asked anything and before a turn is spent.
fn ready(agent: &dyn Agent, name: &str) {
    let status = agent.probe();
    assert!(
        status.installed,
        "{name} is not installed — nothing to measure. {:?}",
        status.note
    );
    assert_eq!(
        status.signed_in,
        Some(true),
        "{name} is installed but not signed in. Run its own login and try again. {:?}",
        status.note
    );
}

/// The two assertions, in the order that matters. A model can *say* anything; only Epoch can set
/// that flag, and it is set inside the capability rather than anywhere a reply is built.
fn assert_it_reached_epoch(who: &str, said: &str, ran: bool) {
    assert!(
        ran,
        "Epoch never ran the capability, so {who} never reached the door.\n\
         --- it said ---\n{said}"
    );
    assert!(
        said.contains(PASSWORD),
        "{who} ran the capability but its answer did not come back:\n{said}"
    );
}

#[test]
#[ignore = "needs Codex installed and signed in, and spends the user's plan allowance"]
fn codex_reaches_epochs_door_and_epoch_runs_what_it_asked_for() {
    // Measured, not assumed: this test is about the door, and "Codex is missing" and "Codex is
    // signed out" are different facts with different fixes. `probe` reports them separately.
    ready(&Codex::default(), "Codex");

    let (said, ran) = one_turn(
        &Codex::default(),
        "epoch_watchword",
        epoch_kernel::Autonomy::Auto,
        None,
        &NobodyToAsk,
    );
    assert_it_reached_epoch("Codex", &said, ran);
}

#[test]
#[ignore = "needs Claude Code installed and signed in, and spends the user's plan allowance"]
fn claude_code_reaches_epochs_door_and_epoch_runs_what_it_asked_for() {
    // **Working today is not evidence.** This agent's door has worked from the day it was built,
    // which is exactly why nothing would have caught it breaking: its configuration goes in
    // through a different flag, as inline JSON rather than a config override, and its program is
    // resolved once at startup rather than per call. Two arrangements that behave the same are
    // still two arrangements, and only one of them had a test.
    //
    // The asymmetry is real and deliberate, not an oversight this test papers over: Epoch
    // confines Codex to a `workspaceWrite` sandbox with no network and gives Claude Code
    // neither. These are not one code path wearing two names.
    let agent = ClaudeCode::default();
    let status = agent.probe();
    assert!(
        status.installed,
        "Claude Code is not installed — nothing to measure. {:?}",
        status.note
    );

    let (said, ran) = one_turn(
        &agent,
        "epoch_watchword",
        epoch_kernel::Autonomy::Auto,
        None,
        &NobodyToAsk,
    );
    assert_it_reached_epoch("Claude Code", &said, ran);
}

/// One of Codex's own bundled skills, found rather than hard-coded.
///
/// The version folder changes when Codex updates — the same reason its executable is looked up
/// each time rather than resolved once. A path written down here would pass until the next
/// update and then fail as though the sandbox had changed.
fn a_bundled_skill() -> Option<std::path::PathBuf> {
    let root = std::path::PathBuf::from(std::env::var_os("USERPROFILE")?)
        .join(".codex/plugins/cache/openai-bundled");
    fn look(at: &std::path::Path, depth: usize) -> Option<std::path::PathBuf> {
        if depth == 0 {
            return None;
        }
        let mut found = None;
        for entry in std::fs::read_dir(at).ok()?.flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n == "SKILL.md") {
                return Some(path);
            }
            if path.is_dir() {
                found = found.or_else(|| look(&path, depth - 1));
            }
        }
        found
    }
    look(&root, 6)
}

#[test]
#[ignore = "needs Codex installed and signed in, and spends the user's plan allowance"]
fn codex_can_still_read_its_own_skills_while_working_for_epoch() {
    // The observation this exists for: the Terminal panel showed `MAGE shell FAILED` running
    // PowerShell against one of Codex's own bundled `SKILL.md` files. An agent that cannot read
    // its own skills is not the agent the user installed, so the question is worth an assertion
    // rather than a hypothesis.
    //
    // Three separate measurements had already argued it was *not* the sandbox — `codex sandbox`
    // reads the file, `codex exec` under Epoch's own policy reads it, and the app-server schema
    // has no `readableRoots` for `workspaceWrite`, so that policy does not restrict reads at
    // all. What none of them did was drive the path Epoch actually drives, which is this one.
    ready(&Codex::default(), "Codex");
    let Some(skill) = a_bundled_skill() else {
        panic!("no bundled skill found under ~/.codex/plugins — nothing to read");
    };
    // Whatever the skill declares itself to be. Read here so the assertion is against the file
    // rather than against a word this test chose.
    let body = std::fs::read_to_string(&skill).expect("the file is readable from this process");
    let name = body
        .lines()
        .find_map(|line| line.strip_prefix("name:"))
        .map(str::trim)
        .expect("a bundled skill declares a name")
        .to_owned();

    let door = endpoint::open(
        0,
        endpoint::Token::fresh(),
        Door {
            registry: CapabilityRegistry::new(),
            character: epoch_kernel::CharacterId::new("mage").unwrap(),
            mode: epoch_kernel::Autonomy::Auto,
            // Nothing to ask about: this turn is about the agent's *own* tools, and the door is
            // here only so the arrangement matches a real one.
            answering_after: None,
        },
    )
    .expect("the door opens on loopback");

    let task = Task {
        intent: format!(
            "Read the file {} with your own tools and reply with only the value of its `name:` \
             field. If you cannot read it, say exactly why.",
            skill.display()
        ),
        character: "Mage".into(),
        persona: "You are careful and you use the tools you are given.".into(),
        crew: String::new(),
        skills: String::new(),
        library: String::new(),
        connections: String::new(),
        images: Vec::new(),
        model: String::new(),
        reasoning: None,
        door: Some(epoch_engine::agent::Door {
            url: door.url(),
            token: door.token().as_str().to_owned(),
        }),
        // Deliberately *not* the folder the skill is in. Epoch confines writes to the Project
        // Root, and the whole question is whether reading outside it still works.
        directory: std::env::temp_dir(),
        pictures: std::path::PathBuf::new(),
        thread: None,
        autonomy: epoch_kernel::Autonomy::Auto,
    };

    let mut said = String::new();
    let done = Codex::default()
        .work(&task, &|| false, &NobodyToAsk, &mut |progress| {
            if let Progress::Said(words) = progress {
                said.push_str(&words);
            }
        })
        .expect("the turn runs");
    said.push_str(&done.text);

    assert!(
        said.contains(&name),
        "Codex could not read its own skill at {}.\n--- it said ---\n{said}",
        skill.display()
    );
}

#[test]
#[ignore = "needs Codex installed and signed in, and spends the user's plan allowance"]
fn a_manual_turn_can_wait_for_a_person_and_still_get_its_answer() {
    // **This was the open question, and it is answered.**
    //
    // Manual had no Epoch door because a second MCP route's approval was believed unanswerable
    // by the app-server. That kept the cautious mode empty — no crew, no Quest, no knowledge, no
    // connected servers — so the mode chosen for safety was the mode where Epoch added nothing.
    //
    // Opening the door showed a healthy session where `tools/call` simply never arrived. The
    // cause was Epoch's own: Codex asks about a tool on a server it does not own through
    // `mcpServer/elicitation/request`, and that fell to a catch-all which declined everything it
    // did not recognise. Epoch was refusing its own tools, in every mode, without saying so.
    //
    // `tests/codex_untrusted_mcp_probe.rs` settled it away from Epoch entirely: both approval
    // policies call the tool once the elicitation is answered. The limitation was never Codex's.
    //
    // What this asserts is the whole arrangement working at once: an Epoch capability with a
    // real effect, judged in Manual, whose verdict is `Ask`, answered by a person who takes four
    // seconds. Codex waits, and the result comes back.
    ready(&Codex::default(), "Codex");

    let (said, ran) = one_turn(
        &Codex::default(),
        "epoch_mark",
        epoch_kernel::Autonomy::Manual,
        Some(std::time::Duration::from_secs(4)),
        // Somebody is there and they say yes. The headless default refuses everything, which
        // would measure the refusal rather than the transport.
        &SaysYes,
    );

    assert_it_reached_epoch("Codex in Manual", &said, ran);
}

/// Can an agent reach Epoch's brush?
///
/// **The answer to *why Epoch rather than the agent alone*, made checkable.** Claude Code cannot
/// draw. Through Epoch it can — with the user's ComfyUI, the user's models and this World's
/// Trust — and that claim is worth exactly as much as a test of it.
///
/// It costs a turn of the user's plan allowance, like everything else in this file.
///
/// ```text
/// cargo test -p epoch-engine --test agents_reach_the_door -- --ignored --nocapture drawing
/// ```
#[test]
#[ignore = "spends the user's plan allowance"]
fn an_agent_can_reach_the_brush_and_epoch_is_the_one_who_holds_it() {
    let agent = ClaudeCode::default();
    ready(&agent, "Claude Code");

    let asked_for = Arc::new(std::sync::Mutex::new(None));
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(Pretend(Arc::clone(&asked_for))));

    let door = endpoint::open(
        0,
        endpoint::Token::fresh(),
        Door {
            registry,
            character: epoch_kernel::CharacterId::new("mage").unwrap(),
            // Auto: this is about whether the tool is reachable, not about Trust, which the
            // tests above already hold still.
            mode: epoch_kernel::Autonomy::Auto,
            answering_after: None,
        },
    )
    .expect("the door opens on loopback");

    let mut task = asking_for("epoch_note", &door, epoch_kernel::Autonomy::Auto);
    task.intent = "Use Epoch's epoch_note tool to write a note about a lighthouse on a cliff at dawn, with the tone set to pixel art. Then reply with only the word done."
        .to_owned();

    let mut said = String::new();
    let done = agent
        .work(&task, &|| false, &NobodyToAsk, &mut |progress| {
            if let Progress::Said(words) = progress {
                said.push_str(&words);
            }
        })
        .expect("the agent runs");
    said.push_str(&done.text);

    // **Epoch ran it**, rather than the model saying it did — the distinction this file exists
    // for. The arguments are the proof: they came from the agent, through the door, into a
    // capability in this process.
    let asked = asked_for
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| panic!("it never reached the capability. It said: {said}"));
    println!("asked for: {asked}");
    assert!(asked.to_lowercase().contains("lighthouse"), "{asked}");
    assert!(
        asked.to_lowercase().contains("pixel"),
        "the second argument travelled too: {asked}"
    );
}
