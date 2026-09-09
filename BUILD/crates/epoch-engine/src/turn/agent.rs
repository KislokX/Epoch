//! The agent turn, in the Engine where it belongs.
//!
//! ## What this is fixing
//!
//! `main.rs` opens by saying the desktop shell "owns **no business logic** (ADR-0003)". It did:
//! `prepare_agent_turn` and `run_agent_turn` were ~210 lines in `epoch-tauri/src/state.rs`
//! deciding whether somebody can work, what they are being asked, how they are judged and what
//! becomes evidence. `grep -rn "agent" crates/epoch-engine/src/turn.rs` returned **0**.
//!
//! So the model turn was headless and the agent turn was not — half the runtime could not exist
//! without a window, which makes the CLI surface `PRODUCT_ARCHITECTURE.md` calls architecturally
//! equal impossible to build.
//!
//! ## The split, and why it is here rather than in one function
//!
//! Two things happen in an agent turn, and only one of them is a decision:
//!
//! - [`assign`] — **decides**. Can this character work, with what, on what, under which mode. It
//!   is pure: no locks, no clock, no disk. It is the part that was untestable, and it is the part
//!   that was wrong to have in a shell.
//! - [`run`] — **carries out**. Hands the work over and translates what comes back into things a
//!   surface can show, through a [`Witness`].
//!
//! The shell keeps what is genuinely its own: holding the mutex, reading the vault, emitting
//! events to a window. Gathering is the shell's job; deciding is not.
//!
//! ## Why `Witness` rather than a callback per event
//!
//! Four `&mut dyn FnMut` parameters is four things a caller can pass in the wrong order, and
//! adding a fifth changes every call site. One trait names them, and a test implements it by
//! pushing into a `Vec` — which is how the behaviour below is asserted at all.

use epoch_kernel::{Autonomy, Brain, CharacterDefinition};

use crate::agent::{Agent, AgentError, Approver, Done, Progress, Task};
use crate::project::ProjectRoot;
use crate::turn::Step;

/// What a surface is told while an agent works.
///
/// Named methods rather than a stream of one enum: each of these reaches a different place in the
/// World — the Chronicle, the Terminal, the context gauge — and a surface that had to match on a
/// variant to find out which would be re-deriving what the Engine already knew.
pub trait Witness {
    /// Words, as they arrive.
    fn said(&mut self, text: &str);
    /// Work, once it has actually happened — including when it failed.
    fn step(&mut self, step: &Step);
    /// How full the agent's own context window is, as **it** measures it.
    ///
    /// `session` is the agent's id for the conversation. It is carried because "is this a new
    /// session?" could not be answered from the numbers: a fresh Claude Code session starts near
    /// 26,000 tokens, so *not returning to zero* looked exactly like *resuming the old one*.
    fn window(&mut self, used: u64, budget: u64, session: Option<&str>);
    /// Which model actually thought, when the agent says so afterwards.
    ///
    /// Defaulted to nothing because most agents never report it, and a surface that had to
    /// implement an empty method for every one of them would be a surface that eventually
    /// implements it wrongly. See [`Progress::Thought`] for why the requested model is not an
    /// answer to this question.
    fn thought(&mut self, _model: &str) {}
}

/// Why an agent cannot be asked to work.
///
/// Each is a sentence the user can act on, and each is refused **before** anything starts: an
/// agent that begins and then discovers it has nowhere to work has already spent a minute and
/// some of the user's quota.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssignError {
    #[error("{name} has no brain assigned — pick one in Characters")]
    NoBrain { name: String },
    #[error("{name} thinks with a model, not an agent")]
    NotAnAgent { name: String },
    #[error("say what you want done")]
    NothingSaid,
}

/// What a character is told, whichever brain answers.
///
/// **Named because it kept growing one field at a time**, and each field arrived the same way:
/// written for a model, and discovered months later to have never reached an agent. `crew` was
/// fixed after a character reported that a colleague had done work nobody had asked them to do;
/// `skills` after a Skill was ticked and changed nothing. Three of them made `assign` an
/// eight-argument function, which is the compiler's way of noticing they belong together.
///
/// Everything here is composed by a function in [`crate::context`], the same one the Composer
/// calls for a model. That is the whole point of the type: there is one answer to each of these
/// questions, and adding a fourth means adding it here rather than to a parameter list.
#[derive(Debug, Default, Clone, Copy)]
pub struct Briefing<'a> {
    /// Everybody else who lives in this World.
    pub crew: &'a [&'a CharacterDefinition],
    /// The ways of working this character has been given, already looked up.
    pub skills: &'a [epoch_kernel::SkillDefinition],
    /// Where this World's notes are, when it has any.
    pub library: Option<&'a str>,
    /// The outside connections this World has, and what is wrong with them.
    ///
    /// The fourth field, and the one that proves the type was worth naming: it was found missing
    /// by *reading this struct against the Composer's block list*, not by a character getting it
    /// wrong again. Three of these were each discovered the expensive way.
    pub connections: &'a [crate::context::ConnectionNote],
}

/// Work out what this character is being asked to do, and whether they can.
///
/// Pure on purpose. Everything it needs is a parameter, so the decision can be asserted without a
/// vault, a window, a lock or a running agent — which is what made this worth moving.
///
/// Returns the agent's id alongside the task: the caller has to look the agent up in a registry
/// this function deliberately does not know about, because *which programs are installed* is a
/// fact about a machine and not about a turn.
pub fn assign(
    definition: &CharacterDefinition,
    briefing: &Briefing<'_>,
    root: &ProjectRoot,
    autonomy: Autonomy,
    thread: Option<String>,
    said: &str,
) -> Result<(String, Task), AssignError> {
    let said = said.trim();
    if said.is_empty() {
        return Err(AssignError::NothingSaid);
    }

    let mind = definition
        .mind
        .as_ref()
        .ok_or_else(|| AssignError::NoBrain {
            name: definition.name.clone(),
        })?;

    let Brain::Agent { agent, .. } = &mind.brain else {
        return Err(AssignError::NotAnAgent {
            name: definition.name.clone(),
        });
    };

    Ok((
        agent.clone(),
        Task {
            // The user's words, unchanged and unpadded (ADR-0025).
            intent: said.to_owned(),
            // Filled in by the caller, like `door`: these are bytes out of the vault, and
            // reading the vault is the shell's job — this function stays pure. It is also the
            // caller that knows whether this agent can see, which is a measured fact rather
            // than something to decide here.
            images: Vec::new(),
            character: definition.name.clone(),
            model: mind.model().to_owned(),
            // Character identity, carried into the agent's own vocabulary (ADR-0026).
            reasoning: mind.parameters.reasoning,
            persona: definition.prompt.clone(),
            // The same sentence the Composer gives a model. An agent that does not know its
            // colleagues exist cannot involve them, and will invent having done so.
            crew: crate::context::crew_note(&definition.name, briefing.crew).unwrap_or_default(),
            // Resolved by the caller, exactly like the crew: which Skills exist is a fact about
            // a vault, and this function deliberately knows nothing about one.
            skills: crate::context::skills_note(briefing.skills).unwrap_or_default(),
            library: crate::context::library_note(briefing.library).unwrap_or_default(),
            connections: crate::context::connections_note(briefing.connections).unwrap_or_default(),
            directory: root.path().to_path_buf(),
            // Filled in by the caller, like `door` and `images`: where a World keeps its pictures
            // is a fact about a vault, and this function knows nothing about one.
            pictures: std::path::PathBuf::new(),
            thread,
            // **Passed through, not decided here.** Epoch launches the process, so the mode is an
            // argument; declining to pass it would not be humility, it would be Epoch silently
            // choosing the default.
            autonomy,
            // Filled in by the caller. Opening a door needs a window handle, and deciding a turn
            // deliberately does not — which is the whole reason this function is testable.
            door: None,
        },
    ))
}

/// Hand the work over, and translate what comes back.
///
/// Every mapping here is a claim about what the user is being shown, and each was a defect once:
///
/// - a tool is reported **when its result arrives**, with whether it worked — a refused `Write`
///   appeared in the Terminal as work done, for a file that never existed;
/// - the window is reported as the agent measures it, because Epoch composed none of it;
/// - words arrive as they are said, because an agent that ran for two minutes in silence is
///   indistinguishable from one that hung.
///
/// ## One attempt, because permission is asked *during* it
///
/// This used to run the turn, notice afterwards that something had been refused for want of
/// permission, ask, and run the whole turn again at a higher rung. That was the only shape
/// available while the need was discovered coarsely — Codex finished under a read-only sandbox
/// and said on stderr what it could not do — and it is gone with that transport (ADR-0027
/// amendment: *invited to the decision, not in charge of it*).
///
/// The agent now pauses **before** the action and asks through [`Approver`], so there is nothing
/// to retry: the answer arrives while the turn is still standing at the door. Keeping the retry
/// would have kept a branch that cannot execute in the middle of the permission path, and a
/// second `Task` at an autonomy the user never selected sitting one `Some` away from running.
pub fn run(
    agent: &dyn Agent,
    task: &Task,
    stopped: &dyn Fn() -> bool,
    witness: &mut dyn Witness,
    approver: &dyn Approver,
) -> Result<Done, AgentError> {
    agent.work(task, stopped, approver, &mut |progress| match progress {
        Progress::Said(text) => witness.said(&text),
        Progress::Window {
            used,
            budget,
            session,
        } => witness.window(used, budget, session.as_deref()),
        // `Used`, not a variant of its own: what the Terminal shows is "this ran, and this is
        // what it touched" — the same sentence whichever brain ran it. A separate variant would
        // let a surface style an agent's work differently for no reason a user could name.
        Progress::Thought { model } => witness.thought(&model),
        Progress::Ran { tool, detail, ok } => witness.step(&Step::Used {
            capability: tool,
            ok,
            detail,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::NobodyToAsk;
    use epoch_kernel::{
        CharacterArchetype, CharacterId, IdleBehavior, Mind, Parameters, PresenceProfile, Reasoning,
    };

    /// Somebody who works with an agent.
    fn character(brain: Option<Brain>) -> CharacterDefinition {
        CharacterDefinition {
            id: CharacterId::new("mage").expect("id"),
            name: "Mage".into(),
            archetype: CharacterArchetype::Researcher,
            role: "Turns goals into plans".into(),
            worlds: Default::default(),
            prompt: "You name tradeoffs explicitly.".into(),
            skills: Default::default(),
            requested_capabilities: None,
            mind: brain.map(|brain| Mind {
                brain,
                parameters: Parameters {
                    reasoning: Some(Reasoning::High),
                    ..Default::default()
                },
                tuning: Default::default(),
            }),
            appearance: None,
            draws_in: None,
            speaks_with: None,
            sounds_like: None,
            presence: PresenceProfile {
                idle: vec![IdleBehavior {
                    activity: "reading".into(),
                    seconds: 30,
                }],
                authored_home: None,
            },
        }
    }

    fn claude() -> Brain {
        Brain::Agent {
            agent: "claude-code".into(),
            model: "opus".into(),
        }
    }

    fn somewhere() -> ProjectRoot {
        ProjectRoot::open(std::env::temp_dir().to_string_lossy().as_ref()).expect("a real folder")
    }

    /// A way of working, as the vault holds it.
    fn skill(name: &str, method: &str) -> epoch_kernel::SkillDefinition {
        epoch_kernel::SkillDefinition {
            id: epoch_kernel::SkillId::new("code-review").expect("id"),
            name: name.into(),
            summary: "How this project reviews a change.".into(),
            method: method.into(),
            requires: Default::default(),
        }
    }

    #[test]
    fn a_skill_reaches_an_agent_turn() {
        // **The defect this file did not have a test for.** The Composer put Skills into a model's
        // turn and this path builds its own briefing, so a character with an agent brain was
        // ticked as having a way of working and silently worked without it. Nothing failed;
        // the answers were simply the ones they would have been anyway, which is why it took
        // asking twice with a marker word to see it at all.
        //
        // Asserted on `assign` rather than on either agent's `persona`, because this is the
        // seam where it went missing: both of those read the same field.
        let (_, task) = assign(
            &character(Some(claude())),
            &Briefing {
                skills: &[skill(
                    "Code review",
                    "Read the diff twice before saying anything.",
                )],
                ..Briefing::default()
            },
            &somewhere(),
            Autonomy::Manual,
            None,
            "revisa este codigo",
        )
        .expect("a character with an agent brain and somewhere to work");

        assert!(
            task.skills.contains("Read the diff twice"),
            "the method itself has to travel, not just its name: {}",
            task.skills
        );
        // And it must arrive as a method rather than as an identity — the same distinction the
        // Composer's block draws, which is the whole reason one function writes both.
        assert!(task.skills.contains("not who you are"));
    }

    #[test]
    fn a_library_reaches_an_agent_turn() {
        // Written at the same time as the model's, not after somebody noticed. `crew` and
        // `skills` were each fixed *after* a turn was found to be missing them, and the cost
        // both times was that nothing failed — the answers were simply worse for a reason
        // nobody could see.
        //
        // It matters more here than for a model: an agent has excellent file tools of its own,
        // they are pointed at the Project Root, and the library is somewhere else entirely.
        let (_, task) = assign(
            &character(Some(claude())),
            &Briefing {
                library: Some("C:/Users/someone/Second Brain"),
                ..Briefing::default()
            },
            &somewhere(),
            Autonomy::Manual,
            None,
            "what did we decide about the runway?",
        )
        .expect("assigned");

        assert!(task.library.contains("Second Brain"));
        // The sentence an agent specifically needs: its own tools do not reach the vault.
        assert!(task.library.contains("your own file tools do not reach it"));
    }

    #[test]
    fn a_briefing_carries_every_shared_note_the_composer_writes() {
        // **The test that should have existed three defects ago.**
        //
        // `crew`, `skills`, `library` and `connections` were each written for a model, each
        // reached an agent only after somebody noticed, and each noticing cost a real failure:
        // a character reporting that a colleague had done work, a Skill that changed nothing,
        // and — the first of all of them — a character inventing "the connectors section of
        // ChatGPT" because nothing had told it where Epoch keeps a server's credentials. That
        // character has an agent brain, so the fix written for it never reached it.
        //
        // Asserted as a **set**, so the next `*_note` added to `context.rs` fails here until it
        // is given to both brains. One at a time is how the first four were found.
        let source = include_str!("../context.rs");
        for note in [
            "crew_note",
            "skills_note",
            "library_note",
            "connections_note",
        ] {
            assert!(
                source.contains(&format!("pub fn {note}(")),
                "{note} is gone; update this list rather than deleting the assertion"
            );
        }

        let (_, task) = assign(
            &character(Some(claude())),
            &Briefing {
                crew: &[],
                skills: &[skill("Code review", "Read the diff twice.")],
                library: Some("C:/Notes"),
                connections: &[crate::context::ConnectionNote {
                    id: "spotify".into(),
                    missing: vec!["SPOTIFY_CLIENT_ID".into()],
                    enabled: true,
                }],
            },
            &somewhere(),
            Autonomy::Manual,
            None,
            "hola",
        )
        .expect("assigned");

        assert!(task.skills.contains("Read the diff twice"));
        assert!(task.library.contains("C:/Notes"));
        assert!(task.connections.contains("SPOTIFY_CLIENT_ID"));
        // The sentence the whole thing exists for: where the user actually fixes it.
        assert!(task.connections.contains("MCP deck"));
    }

    #[test]
    fn a_world_with_no_library_tells_an_agent_nothing_about_one() {
        let (_, task) = assign(
            &character(Some(claude())),
            &Briefing::default(),
            &somewhere(),
            Autonomy::Manual,
            None,
            "hola",
        )
        .expect("assigned");

        assert!(task.library.is_empty());
    }

    #[test]
    fn a_character_given_no_skills_is_told_nothing_about_them() {
        // Empty, not an explanation of emptiness. A paragraph saying "you have no ways of
        // working" is a sentence about Epoch's internals in somebody's system prompt.
        let (_, task) = assign(
            &character(Some(claude())),
            &Briefing::default(),
            &somewhere(),
            Autonomy::Manual,
            None,
            "hola",
        )
        .expect("assigned");

        assert!(task.skills.is_empty());
    }

    #[test]
    fn a_character_with_no_brain_is_refused_before_anything_starts() {
        // Not "it failed to answer": nobody was ever asked. The sentence names the fix.
        let refused = assign(
            &character(None),
            &Briefing::default(),
            &somewhere(),
            Autonomy::Manual,
            None,
            "hola",
        );
        assert_eq!(
            refused,
            Err(AssignError::NoBrain {
                name: "Mage".into()
            })
        );
    }

    #[test]
    fn a_character_who_thinks_with_a_model_is_not_an_agent_turn() {
        // The dispatch happens once, at the top (ADR-0027). Landing here means a caller asked the
        // wrong runtime, and saying so beats producing an empty turn.
        let model = Brain::Model {
            provider: "ollama".into(),
            model: "qwen3:14b".into(),
        };
        let refused = assign(
            &character(Some(model)),
            &Briefing::default(),
            &somewhere(),
            Autonomy::Auto,
            None,
            "hola",
        );
        assert_eq!(
            refused,
            Err(AssignError::NotAnAgent {
                name: "Mage".into()
            })
        );
    }

    #[test]
    fn nothing_said_is_nothing_to_do() {
        let refused = assign(
            &character(Some(claude())),
            &Briefing::default(),
            &somewhere(),
            Autonomy::Auto,
            None,
            "   ",
        );
        assert_eq!(refused, Err(AssignError::NothingSaid));
    }

    #[test]
    fn the_task_carries_who_they_are_and_what_was_actually_said() {
        let (agent, task) = assign(
            &character(Some(claude())),
            &Briefing::default(),
            &somewhere(),
            Autonomy::AcceptEdits,
            Some("session-7".into()),
            "  rename the config key  ",
        )
        .expect("assigned");

        assert_eq!(agent, "claude-code");
        // Trimmed, never rewritten or padded (ADR-0025).
        assert_eq!(task.intent, "rename the config key");
        assert_eq!(task.character, "Mage");
        assert_eq!(task.persona, "You name tradeoffs explicitly.");
        // The agent's own vocabulary for a model, and the canonical rung beside it (ADR-0026).
        assert_eq!(task.model, "opus");
        assert_eq!(task.reasoning, Some(Reasoning::High));
        // Epoch spawns the process, so Epoch passes the mode rather than accepting a default.
        assert_eq!(task.autonomy, Autonomy::AcceptEdits);
        // The conversation to continue, and no door until the caller opens one.
        assert_eq!(task.thread.as_deref(), Some("session-7"));
        assert!(task.door.is_none());
    }

    /// An agent that replays a captured stream. No process, no network, no CLI.
    struct Scripted(Vec<Progress>, Result<Done, ()>);

    impl Agent for Scripted {
        fn id(&self) -> &str {
            "scripted"
        }
        fn probe(&self) -> crate::agent::AgentStatus {
            crate::agent::AgentStatus::missing("scripted", "Scripted", "a test")
        }
        fn work(
            &self,
            _task: &Task,
            stopped: &dyn Fn() -> bool,
            _approver: &dyn Approver,
            sink: &mut dyn FnMut(Progress),
        ) -> Result<Done, AgentError> {
            for progress in &self.0 {
                if stopped() {
                    return Err(AgentError::Stopped {
                        agent: "Scripted".into(),
                        why: "you asked it to stop".into(),
                    });
                }
                sink(progress.clone());
            }
            self.1.clone().map_err(|()| AgentError::Failed {
                agent: "Scripted".into(),
                why: "as scripted".into(),
            })
        }
    }

    /// A `Witness` that remembers instead of drawing.
    #[derive(Default)]
    struct Watched {
        said: Vec<String>,
        steps: Vec<Step>,
        windows: Vec<(u64, u64, Option<String>)>,
    }

    impl Witness for Watched {
        fn said(&mut self, text: &str) {
            self.said.push(text.to_owned());
        }
        fn step(&mut self, step: &Step) {
            self.steps.push(step.clone());
        }
        fn window(&mut self, used: u64, budget: u64, session: Option<&str>) {
            self.windows
                .push((used, budget, session.map(str::to_owned)));
        }
    }

    fn task() -> Task {
        assign(
            &character(Some(claude())),
            &Briefing::default(),
            &somewhere(),
            Autonomy::Manual,
            None,
            "do the thing",
        )
        .expect("assigned")
        .1
    }

    #[test]
    fn everything_the_agent_reports_reaches_the_surface_that_shows_it() {
        let agent = Scripted(
            vec![
                Progress::Said("looking".into()),
                Progress::Ran {
                    tool: "Write".into(),
                    detail: "notes.md".into(),
                    ok: true,
                },
                Progress::Window {
                    used: 26_101,
                    budget: 200_000,
                    session: Some("35c410cc".into()),
                },
            ],
            Ok(Done {
                text: "done".into(),
                thread: Some("35c410cc".into()),
                evidence: vec![crate::capability::Made {
                    reference: "notes.md".into(),
                    summary: "Write notes.md".into(),
                }],
            }),
        );

        let mut watched = Watched::default();
        let done = run(&agent, &task(), &|| false, &mut watched, &NobodyToAsk).expect("worked");

        assert_eq!(watched.said, vec!["looking"]);
        assert_eq!(
            watched.steps,
            vec![Step::Used {
                capability: "Write".into(),
                ok: true,
                detail: "notes.md".into(),
            }]
        );
        // The session travels with the numbers, because the numbers alone cannot answer whether
        // this is a new conversation: a fresh session starts near 26,000 tokens.
        assert_eq!(
            watched.windows,
            vec![(26_101, 200_000, Some("35c410cc".into()))]
        );
        assert_eq!(
            done.evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["Write notes.md"]
        );
    }

    #[test]
    fn a_refused_tool_is_shown_as_the_failure_it_was() {
        // The defect this guards: a `Write` the user denied appeared in the Terminal as work
        // done, for a file that never existed.
        let agent = Scripted(
            vec![Progress::Ran {
                tool: "Write".into(),
                detail: "notes.md".into(),
                ok: false,
            }],
            Ok(Done::default()),
        );

        let mut watched = Watched::default();
        run(&agent, &task(), &|| false, &mut watched, &NobodyToAsk).expect("worked");

        assert_eq!(
            watched.steps,
            vec![Step::Used {
                capability: "Write".into(),
                ok: false,
                detail: "notes.md".into(),
            }]
        );
    }

    #[test]
    fn stopping_reaches_an_agent_that_owns_its_own_loop() {
        // The only seam there is: the loop belongs to the agent, so stopping means declining to
        // wait for the next thing it says.
        let agent = Scripted(vec![Progress::Said("one".into())], Ok(Done::default()));
        let mut watched = Watched::default();

        let ended = run(&agent, &task(), &|| true, &mut watched, &NobodyToAsk);

        assert!(matches!(ended, Err(AgentError::Stopped { .. })));
        assert!(watched.said.is_empty(), "nothing is claimed after a stop");
    }
}
