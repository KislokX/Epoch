//! Gemini CLI as an Epoch agent, over the Agent Client Protocol.
//!
//! ## Everything here was measured — 0.54.4 on 2026-08-19, 0.56.0 on 2026-09-07 and 09-08
//!
//! The program was asked what it is rather than remembered, because most of what matters is in
//! neither `--help` nor any document: the ACP method names, the shape of a permission question,
//! which mode actually asks, and the list of models it offers.
//!
//! ## Why this transport and not the other one
//!
//! Until 2026-09-08 this adapter ran `--prompt "" --output-format stream-json`, a **one-way**
//! stream. It could not be asked anything, so [`Approver`] was never consulted and `Manual`
//! mapped to `--approval-mode plan` — Gemini's own *read-only* mode rather than a permission
//! one. Measured through the installed application: asked in Manual to write a file, it wrote
//! the file. There was nobody for it to ask.
//!
//! `gemini --acp` is bidirectional, which is the whole difference. The agent stops before
//! running its own tool and asks the client:
//!
//! ```text
//! -> {"id":1,"method":"initialize","params":{"protocolVersion":1,
//!      "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false}}}}
//! <- {"id":1,"result":{"protocolVersion":1,"authMethods":[...],
//!      "agentInfo":{"name":"gemini-cli","version":"0.56.0"},"agentCapabilities":{...}}}
//! -> {"id":2,"method":"session/new","params":{"cwd":"...","mcpServers":[]}}
//! <- {"id":2,"result":{"sessionId":"...",
//!      "modes":{"currentModeId":"default",
//!               "availableModes":[{"id":"default","description":"Prompts for approval"},
//!                                 {"id":"autoEdit"},{"id":"yolo"},{"id":"plan"}]},
//!      "models":{"currentModelId":"auto","availableModels":[...]}}}
//! -> {"id":9,"method":"session/set_mode","params":{"sessionId":"...","modeId":"default"}}
//! <- {"id":9,"result":{}}                        // wait for this - see below
//! -> {"id":4,"method":"session/prompt","params":{"sessionId":"...",
//!      "prompt":[{"type":"text","text":"..."}]}}
//! <- session/update  agent_thought_chunk . agent_message_chunk . tool_call . tool_call_update
//! <- {"id":0,"method":"session/request_permission","params":{"sessionId":"...",
//!      "options":[{"optionId":"proceed_always","kind":"allow_always"},
//!                 {"optionId":"proceed_once","kind":"allow_once"},
//!                 {"optionId":"cancel","kind":"reject_once"}],
//!      "toolCall":{"toolCallId":"write_file__call_493889","status":"pending",
//!                  "title":"Writing to acp-probe.txt",
//!                  "content":[{"type":"diff","path":"...","oldText":null}]}}}
//! -> {"id":0,"result":{"outcome":{"outcome":"selected","optionId":"cancel"}}}
//! <- {"id":4,"result":{"stopReason":"end_turn","_meta":{"quota":{...}}}}
//! ```
//!
//! That is Epoch's prompt, in the agent's own words, with a diff — the arrangement ADR-0027
//! describes and the one Codex already has. **Verified end to end through [`Agent::work`]
//! itself:** refused, the file does not exist; allowed, it does.
//!
//! ## Four decisions this settles
//!
//! - **`clientCapabilities.fs` stays false.** Declaring it true makes the agent send
//!   `fs/read_text_file` and `fs/write_text_file` to the *client* — Epoch would be doing the
//!   file work. Measured, and refused: Epoch does not govern an agent's own tools, it governs
//!   its own (`CLAUDE.md`, 2026-08-07). A good agent's file tools are better than Epoch's.
//! - **No `authenticate`.** The method exists and answers `{}`; `session/new` succeeds without
//!   it — measured both ways. One fewer step that can fail for a reason nobody asked about.
//! - **The mode is per session**, and `default` *is* "Prompts for approval". Epoch's `Manual`
//!   maps to it; `AcceptEdits` to `autoEdit`; `Auto` to `yolo`. `plan` maps to nothing — it is
//!   read-only rather than permissive, and using it for `Manual` is what let the write through.
//! - **Wait for `session/set_mode` before prompting.** Sent back to back, the agent answers the
//!   mode and *sometimes drops the prompt* — one run reached the permission question in 13 s and
//!   the next never answered at all. It is a safety fix before it is a hang fix: a prompt racing
//!   the mode it depends on could run under whatever the session was set to before.
//!
//! ## Two things only the wire could show
//!
//! Both are why [`trace`] is still here. Neither was reachable from the Python probe that
//! "proved the protocol", from the test suite, or from reading the code.
//!
//! The probe had a `sleep` between `set_mode` and `prompt` and Epoch had none. **A harness that
//! waits measures a protocol the product does not speak** — 11.18's rule in the time dimension.
//!
//! And [`Agent::work`] once finished a turn without returning: the answer was written, `done`
//! was set, and `join()` waited forever. `Child::kill` ends `gemini.cmd`; the real program is
//! `node gemini.js`, which re-executes itself, and the grandchild kept the reader's pipe open.
//! [`epoch_models::quiet::stop_tree`] is the fix and it is shared, because
//! `capabilities::machine` had found the same fact about a shell months earlier.
//!
//! ## The model it answers with is not the model it was asked for
//!
//! Its default is `auto` and it routes. A run on 0.56.0 that requested nothing declared tools
//! against `gemini-3.1-pro-preview-customtools` at a cost of zero tokens and spent all 19,761 of
//! them in `gemini-3-flash-preview`. So `--model` is a **request**, and a surface that displayed
//! it as the answer would be stating something Epoch was never told. [`Progress::Thought`]
//! carries what actually ran, read from `_meta.quota.model_usage`; a model that spent no tokens
//! did not think, and is left out.
//!
//! Which models exist is asked rather than remembered — see [`models_offered`]. Three names an
//! earlier session tested from memory did not exist at all, and the quota refusal it read as
//! *the account is exhausted* was one model's.
//!
//! ## Two things this still does not claim
//!
//! **It has no door.** MCP servers are configured globally by `gemini mcp add`, and there is no
//! per-run `--mcp-config` the way Claude Code has one. Writing Epoch's per-session token into
//! somebody's global config would outlive the door it describes, so a Gemini character works
//! with its own tools and Epoch says so rather than offering a crew it cannot reach.
//!
//! **Its sign-in cannot be *verified*.** There is no command to ask, and asking anyway is
//! expensive: `query` is a positional argument, so `gemini auth status` is not a rejected
//! subcommand — it is a **billed prompt**. Three guesses at a flag name exhausted a quota. So
//! nothing here may guess at this program's command line.
//!
//! What *is* readable is the choice the user made, in the same file the CLI's own `/about`
//! reads it from: `~/.gemini/settings.json` -> `security.auth.selectedType`. That separates
//! **never configured** (actionable: open `gemini`, run `/auth`) from **configured**, which is
//! as far as a free reading goes — a chosen method is not a working credential, as an exhausted
//! quota demonstrates. A configured agent therefore stays `None`; only an absent choice is
//! `false`.
//!
//! Reading another program's configuration file is a deliberate, narrow exception, bounded by
//! the same rule as everything else: missing, unreadable, or shaped differently in a later
//! version all yield `None` — *unasked* — never a negative answer.
//!
//! ## What is reported, and what is left out
//!
//! `_meta.quota.token_count` carries tokens used but **no window size**, so no
//! [`Progress::Window`] is emitted. A gauge needs both halves and the second one does not exist
//! here; a budget guessed from a model name would be the invented reading this project removes
//! everywhere else.

use epoch_models::quiet::Quiet;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;

use crate::agent::{
    Agent, AgentError, AgentStatus, Approver, Done, Needs, Progress, Proposal, Task,
};

pub const ID: &str = "gemini";
pub const NAME: &str = "Gemini CLI";

/// What it is called on a command line. `.cmd` on Windows, where npm installs a shim.
fn program() -> &'static str {
    if cfg!(windows) {
        "gemini.cmd"
    } else {
        "gemini"
    }
}

/// Which models this Gemini CLI offers **today**.
///
/// **Measured, not remembered.** `--help` documents `--model` and names none of them, and the
/// three model names an earlier session confidently tested (`gemini-3-flash`, `gemini-3-pro`,
/// `gemini-2.5-flash`) turned out not to exist — a quota refusal was read as *no quota anywhere*
/// when it was one model's. The real list is in `session/new`'s own answer, under
/// `models.availableModels`, and it costs one short-lived process and no request to the service.
///
/// `None` means **nobody could be asked** — not installed, not signed in, a release without
/// this field. Never *there are none*: the picker keeps its "name it myself" field either way,
/// and Epoch has nothing of its own to suggest here (see [`super::models_for`]).
pub fn models_offered() -> Option<Vec<(String, String)>> {
    let mut child = Command::new(program())
        .quiet()
        // Its own working directory, because `session/new` is asked for one and a Gemini
        // starting in the user's project would read that project to answer a question about
        // model names.
        .current_dir(std::env::temp_dir())
        .arg("--acp")
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

    let started = send_rpc(
        &mut input,
        1,
        "initialize",
        serde_json::json!({
            "protocolVersion": 1,
            "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false } },
        }),
    );

    let answer = if started {
        loop {
            let Ok(line) = arriving.recv_timeout(LISTING) else {
                break None;
            };
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            match message.get("id").and_then(serde_json::Value::as_u64) {
                Some(1) => {
                    let opened = send_rpc(
                        &mut input,
                        2,
                        "session/new",
                        serde_json::json!({
                            "cwd": std::env::temp_dir().display().to_string(),
                            "mcpServers": [],
                        }),
                    );
                    if !opened {
                        break None;
                    }
                }
                Some(2) => break read_model_list(&message),
                _ => {}
            }
        }
    } else {
        None
    };

    // The tree, for the reason `stop_tree` documents: a `.cmd` shim's grandchild would otherwise
    // outlive this and hold the reader's pipe open.
    epoch_models::quiet::stop_tree(&mut child);
    drop(input);
    let _ = child.wait();
    drop(arriving);
    let _ = reader.join();
    answer
}

/// The models named in a `session/new` answer, in the order Gemini itself lists them.
///
/// `auto` is kept rather than filtered: it is a real choice — the CLI routes the turn — and its
/// own description names what it routes between, which is more information than Epoch could add.
///
/// An answer that parses to nothing is `None`, because *nothing usable came back* and *this
/// sign-in has no models* are different facts and only the first is one Epoch has measured.
fn read_model_list(message: &serde_json::Value) -> Option<Vec<(String, String)>> {
    let found: Vec<(String, String)> = message
        .pointer("/result/models/availableModels")?
        .as_array()?
        .iter()
        .filter_map(|model| {
            let id = model.get("modelId").and_then(serde_json::Value::as_str)?;
            let name = model
                .get("name")
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

/// How long a listing waits. Generous next to a probe and nothing like a turn: the CLI has to
/// start Node twice before it says anything, and measured here that is a few seconds.
const LISTING: std::time::Duration = std::time::Duration::from_secs(30);

pub struct Gemini;

impl Agent for Gemini {
    fn id(&self) -> &str {
        ID
    }

    fn probe(&self) -> AgentStatus {
        let program = program();
        let asked = Command::new(program)
            .quiet()
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();

        let mut child = match asked {
            Ok(child) => child,
            Err(err) => {
                let mut missing = AgentStatus::missing(
                    ID,
                    NAME,
                    format!("`{program}` is not on this machine's PATH ({err})"),
                );
                missing.looked_in = Some(program.to_owned());
                return missing;
            }
        };

        let Some(said) = super::claude::wait_for(&mut child, PROBE_PATIENCE) else {
            let _ = child.kill();
            let mut stuck = AgentStatus::missing(
                ID,
                NAME,
                "it did not answer `--version` in time".to_string(),
            );
            stuck.looked_in = Some(program.to_owned());
            return stuck;
        };

        let (signed_in, method) = chosen_method();
        AgentStatus {
            kind: ID.to_owned(),
            id: ID.to_owned(),
            name: NAME.to_owned(),
            installed: true,
            version: Some(said.trim().to_owned()).filter(|v| !v.is_empty()),
            looked_in: Some(program.to_owned()),
            note: Some(match (&signed_in, &method) {
                // The one actionable case, and it names the exact keystrokes.
                (Some(false), _) => {
                    "Nobody has signed in yet. Open `gemini` and run `/auth`.".to_owned()
                }
                (_, Some(method)) => format!(
                    "Signed in with {method}. Epoch cannot verify that it still works: this CLI has no way to ask, and asking anyway would cost a request."
                ),
                _ => "Epoch cannot read whether this is signed in: the CLI offers no way to ask. If a turn fails for authentication, sign in and try again."
                    .to_owned(),
            }),
            // **Unasked, not signed out.** Only an explicitly empty choice is `false`; a chosen
            // method is not a working credential, and an unreadable file is not an answer.
            signed_in,
            // **No account.** This CLI reports no identity, and the API-key method has none to
            // report — inventing one from the method's name would be a face for somebody who
            // has not chosen one.
            account: None,
            // What they chose, never a credential. `gemini-api-key` is a method name, and the
            // key itself never leaves that program.
            method,
        }
    }

    /// One turn, over the Agent Client Protocol.
    ///
    /// ## Why this replaced a one-way stream
    ///
    /// The old transport was `--prompt ""` with `--output-format stream-json`: the briefing went
    /// in, lines came out, and there was **no channel for a question**. So `Manual` could not
    /// mean what it means everywhere else — Epoch was never asked, and the mode it fell back on,
    /// Gemini's own `plan`, is read-only rather than permissive and did not hold: asked in
    /// Manual to write a file, it wrote it.
    ///
    /// `--acp` is bidirectional and sends the client `session/request_permission` before the
    /// agent runs its own tool. Every message shape below was measured against gemini-cli 0.56.0
    /// on 2026-09-08 and is written out in this module's header.
    ///
    /// ## What is deliberately not done
    ///
    /// **`clientCapabilities.fs` stays false.** Declaring it true makes the agent send
    /// `fs/read_text_file` and `fs/write_text_file` to the *client*, which would put Epoch in
    /// charge of the agent's file work. Measured, and refused: Epoch does not govern an agent's
    /// own tools, it governs its own — and a good agent's file tools are better than Epoch's.
    ///
    /// **No `authenticate`.** The method exists and answers `{}`, and `session/new` succeeds
    /// without it — measured both ways. One fewer step that can fail for a reason nobody asked
    /// about.
    fn work(
        &self,
        task: &Task,
        stopped: &dyn Fn() -> bool,
        approver: &dyn Approver,
        sink: &mut dyn FnMut(Progress),
    ) -> Result<Done, AgentError> {
        let mut command = Command::new(program());
        command.quiet();
        // The `.cmd` shim's grandchild is the real program; a group is how it is ended.
        command.own_group();
        command
            .current_dir(&task.directory)
            .args(acp_arguments(task))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Its own warnings live here — true colour, ripgrep — and they are noise rather than
            // failure. Kept rather than discarded so a real complaint is still readable.
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|err| AgentError::Failed {
            agent: ID.to_owned(),
            why: format!("could not start `{}`: {err}", program()),
        })?;
        let mut input = child.stdin.take().expect("stdin was piped");
        let output = child.stdout.take().expect("stdout was piped");

        let complaints = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let watching = child.stderr.take().map(|pipe| {
            let heard = std::sync::Arc::clone(&complaints);
            std::thread::spawn(move || {
                for line in BufReader::new(pipe).lines().map_while(Result::ok) {
                    if !is_noise(&line) {
                        if let Ok(mut held) = heard.lock() {
                            held.push(line);
                        }
                    }
                }
            })
        });

        // Read on its own thread so the turn can poll `stopped` between messages rather than
        // blocking on a line that may never come.
        let (lines, arriving) = mpsc::channel::<serde_json::Value>();
        let reader = std::thread::spawn(move || {
            for line in BufReader::new(output).lines().map_while(Result::ok) {
                let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                    continue;
                };
                trace('<', &line);
                if lines.send(message).is_err() {
                    return;
                }
            }
        });

        let mut turn = Turn {
            said: String::new(),
            session: None,
            reaching: std::collections::HashMap::new(),
            done: false,
            failure: None,
        };

        let _ = send_rpc(
            &mut input,
            1,
            "initialize",
            serde_json::json!({
                "protocolVersion": 1,
                // **False, on purpose.** See the note above this function.
                "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false } },
            }),
        );

        // Which request opens the conversation, and whether it is a continuation. A load that
        // finds nothing falls back to a new session exactly once — the handle is continuity,
        // never the record, and a Chronicle must not become a dead end because the agent forgot.
        let mut opening = if task.thread.is_some() { 3 } else { 2 };
        let mut retried = false;
        let mut asked_to_speak = false;
        let began = std::time::Instant::now();

        while !turn.done && turn.failure.is_none() {
            if stopped() {
                // **A cancelled turn reads as cancelled.** Breaking out landed on the
                // `said.trim().is_empty()` arm below, so pressing STOP before the agent
                // had spoken put `gemini could not be started: it said nothing` in the
                // Chronicle — a sentence about a failure to start, for something the user
                // did on purpose. Measured in the installed window: STOP at 2.1 s, idle at
                // 4.1 s, and that sentence. Codex and Claude Code both answer `Stopped`
                // here; this is the third place needing the same thing and the one that
                // did not have it.
                epoch_models::quiet::stop_tree(&mut child);
                drop(arriving);
                let _ = reader.join();
                if let Some(watching) = watching {
                    let _ = watching.join();
                }
                return Err(AgentError::Stopped {
                    agent: NAME.into(),
                    why: "you asked it to stop".into(),
                });
            }
            // **The opening is bounded; the turn is not.** A turn may legitimately take twenty
            // minutes and nothing here may cut it short. Getting as far as *asking the question*
            // is a handshake — three messages, milliseconds each when it works — and this is the
            // exact stretch that hung: a `session/prompt` the agent silently dropped left the
            // card reading `WORKING` with nothing behind it. That defect is fixed upstream of
            // this; the bound is so the next one of its shape reads as a failure rather than as
            // a character who never speaks.
            if !asked_to_speak && began.elapsed() > OPENING {
                turn.failure = Some(format!(
                    "it did not open a conversation within {} seconds",
                    OPENING.as_secs()
                ));
                continue;
            }
            let Ok(message) = arriving.recv_timeout(std::time::Duration::from_millis(200)) else {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                continue;
            };

            if let Some(id) = message.get("id").and_then(serde_json::Value::as_u64) {
                if message.get("method").is_some() {
                    // A request *from* the agent. The only one Epoch answers is the permission
                    // question, which is the entire reason this transport exists.
                    answer_request(&mut input, &message, id, task, approver, sink);
                    continue;
                }
                match id {
                    1 => {
                        let params = if task.thread.is_some() {
                            serde_json::json!({
                                "sessionId": task.thread.clone().unwrap_or_default(),
                                "cwd": here(task),
                                "mcpServers": [],
                            })
                        } else {
                            serde_json::json!({
                                "cwd": here(task),
                                "mcpServers": [],
                            })
                        };
                        let method = if task.thread.is_some() {
                            "session/load"
                        } else {
                            "session/new"
                        };
                        let _ = send_rpc(&mut input, opening, method, params);
                    }
                    id if id == opening => {
                        if message.get("error").is_some() {
                            if !retried {
                                // The stored session is gone. Start one rather than stopping.
                                retried = true;
                                opening = 2;
                                turn.session = None;
                                let _ = send_rpc(
                                    &mut input,
                                    2,
                                    "session/new",
                                    serde_json::json!({
                                        "cwd": here(task),
                                        "mcpServers": [],
                                    }),
                                );
                                continue;
                            }
                            turn.failure = Some(
                                super::refusal_in(&message)
                                    .unwrap_or_else(|| "it would not open a conversation".into()),
                            );
                            continue;
                        }
                        // `session/load` answers the modes and no id, because the id was ours.
                        let session = message
                            .pointer("/result/sessionId")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_owned)
                            .or_else(|| task.thread.clone());
                        turn.session = session.clone();
                        let Some(session) = session else {
                            turn.failure = Some("it did not name the conversation".into());
                            continue;
                        };
                        // The mode is a property of the session, and `default` is the one that
                        // asks. `plan` maps to nothing here: it is read-only rather than
                        // permissive, and using it for Manual is what let a write through.
                        let _ = send_rpc(
                            &mut input,
                            9,
                            "session/set_mode",
                            serde_json::json!({
                                "sessionId": session,
                                "modeId": mode_for(task.autonomy),
                            }),
                        );
                    }
                    // **The mode has to be in effect before the turn starts, and waiting for
                    // this reply is what makes that true.** Measured, and it was the whole hang:
                    // sending `session/prompt` immediately after `session/set_mode` produced a
                    // conversation that answered neither — the mode reply arrived, the prompt
                    // never did, and Epoch waited on a turn the agent had dropped. The Python
                    // probe that "proved the protocol" had waited between the two, which is
                    // exactly the kind of difference a harness hides (`CLAUDE.md`, 11.18).
                    //
                    // The safety half matters more than the hang: Manual means *ask me*, and a
                    // prompt racing the mode it depends on could run a tool under whatever the
                    // session was set to before.
                    //
                    // An error here still speaks. A CLI too old to have modes must not become a
                    // character who never answers — a gate with nothing behind it stays open,
                    // and the autonomy note already says Epoch is not the one enforcing this.
                    9 => {
                        let Some(session) = turn.session.clone() else {
                            turn.failure = Some("it did not name the conversation".into());
                            continue;
                        };
                        if !asked_to_speak {
                            asked_to_speak = true;
                            let _ = send_rpc(
                                &mut input,
                                4,
                                "session/prompt",
                                serde_json::json!({
                                    "sessionId": session,
                                    "prompt": [{ "type": "text", "text": prompt(task) }],
                                }),
                            );
                        }
                    }
                    4 => {
                        if let Some(why) = super::refusal_in(&message) {
                            turn.failure = Some(why);
                        }
                        if let Some(model) = who_thought_acp(&message) {
                            sink(Progress::Thought { model });
                        }
                        turn.done = true;
                    }
                    _ => {}
                }
                continue;
            }

            if message.get("method").and_then(serde_json::Value::as_str) == Some("session/update") {
                update(&message, &mut turn, sink);
            }
        }

        // The tree, not the shim. Joining the reader below is only safe because this ends
        // the grandchild that holds the other end of its pipe.
        epoch_models::quiet::stop_tree(&mut child);
        drop(arriving);
        let _ = reader.join();
        if let Some(watching) = watching {
            let _ = watching.join();
        }

        if let Some(why) = turn.failure {
            return Err(AgentError::Failed {
                agent: ID.to_owned(),
                why: match super::said(&complaints) {
                    Some(heard) if !why.contains(heard.trim()) => format!("{why} — {heard}"),
                    _ => why,
                },
            });
        }

        if turn.said.trim().is_empty() {
            return Err(AgentError::Failed {
                agent: ID.to_owned(),
                why: super::said(&complaints).unwrap_or_else(|| "it said nothing".to_owned()),
            });
        }

        Ok(Done {
            text: turn.said,
            thread: turn.session,
            // Nothing yet. Evidence is a thing that now exists, and the wire reports which tools
            // ran without reporting what they left behind — so claiming an artifact here would
            // be narration, which is the one thing History is not made of (ADR-0025).
            evidence: Vec::new(),
        })
    }
}

/// What one turn has gathered so far.
struct Turn {
    said: String,
    session: Option<String>,
    /// Tool calls by id, so an update that carries only an id can still be named.
    reaching: std::collections::HashMap<String, String>,
    done: bool,
    failure: Option<String>,
}

/// Where the turn happens, written the way another program can read it.
///
/// **A resolved path is not a presentable path.** Containment canonicalises a Project Root on
/// purpose, and on Windows that produces an extended-length path — `\\?\C:\Users\…`. Node
/// splits that into a root of `\\?\` and a first segment of `C:`, calls `lstat` on it, and
/// answers `EISDIR: illegal operation on a directory, lstat 'C:'`. Measured: every turn in a
/// World whose root had been resolved failed at `session/new`, before the model was reached.
///
/// [`crate::library::plainly`] was written for exactly this — *"the resolved form is exactly
/// what must never be assumed to be presentable"* — for Obsidian, months earlier, and no
/// adapter used it. The live test could not see it because `std::env::temp_dir()` carries no
/// prefix: **a fixture that happens to supply clean input measures the wiring and nothing
/// else** (`CLAUDE.md`, 11.30).
fn here(task: &Task) -> String {
    crate::library::plainly(&task.directory.display().to_string())
}

/// How long the handshake may take before the turn is called a failure.
///
/// Generous next to what it measures — `initialize`, `session/new` and `session/set_mode` answer
/// in milliseconds once the CLI has started, and starting it means Node twice. Deliberately not
/// applied to the turn itself.
const OPENING: std::time::Duration = std::time::Duration::from_secs(90);

/// The wire, when somebody asks for it: `EPOCH_TRACE_ACP=1`.
///
/// **Kept, because it is what found the two defects this transport shipped with.** Both were
/// invisible to every test and to the window: a `session/prompt` sent before `session/set_mode`
/// had replied, which the agent sometimes dropped without a word; and a `join()` on a reader
/// whose pipe was held open by an orphaned grandchild of the `.cmd` shim. Neither is reachable
/// by reasoning about the code, and both were obvious in one trace.
///
/// Off unless asked for, truncated, and on stderr — where a person who asked for it is looking
/// and a Chronicle never is. It carries whatever the two programs said to each other, so it is
/// deliberately something somebody turns on rather than something Epoch writes to a file.
fn trace(direction: char, line: &str) {
    if std::env::var_os("EPOCH_TRACE_ACP").is_some() {
        eprintln!("{direction}{direction} {}", &line[..line.len().min(300)]);
    }
}

/// Everything on the command line for an ACP turn.
///
/// Short and flagless by comparison with the old transport: the briefing is a protocol message
/// now rather than something squeezed through stdin, so the newline problem that shaped the
/// previous version cannot arise here at all.
fn acp_arguments(task: &Task) -> Vec<String> {
    let mut said = vec!["--acp".to_owned()];
    if !task.model.trim().is_empty() {
        said.push("--model".to_owned());
        said.push(task.model.trim().to_owned());
    }
    said
}

/// Epoch's autonomy in ACP's session modes, measured from `session/new`'s own answer.
///
/// ```text
/// default  "Prompts for approval"
/// autoEdit "Auto-approves edit tools"
/// yolo     "Auto-approves all tools"
/// plan     "Read-only mode"
/// ```
///
/// **`plan` maps to nothing.** It is a read-only mode rather than a permission one, and using it
/// for `Manual` is what produced a write nobody approved: Epoch's Manual means *ask me*, and the
/// mode that asks is `default`.
fn mode_for(autonomy: epoch_kernel::Autonomy) -> &'static str {
    match autonomy {
        epoch_kernel::Autonomy::Manual => "default",
        epoch_kernel::Autonomy::AcceptEdits => "autoEdit",
        epoch_kernel::Autonomy::Auto => "yolo",
    }
}

/// How much of one description reaches the dialogue.
///
/// A `write_file` on a large file carries its whole contents, and a preview is read by a person
/// in a box on top of a conversation. Bounded here rather than in the surface: a value the Engine
/// hands over is a value the Engine is responsible for.
const SHOWN_LIMIT: usize = 8 * 1024;

/// What the agent stopped to ask about, read out of its own `toolCall`.
///
/// ## Every field here was seen on a wire
///
/// Recorded off gemini-cli 0.56.0 on 2026-09-08, asking for a write and for `git init`:
///
/// ```text
/// "toolCall": { "kind": "edit", "title": "Writing to notes.txt",
///               "locations": [{ "path": "C:\\...\\notes.txt" }],
///               "content": [{ "type": "diff", "path": "C:\\...\\notes.txt",
///                             "oldText": "", "newText": "ADIOS",
///                             "_meta": { "kind": "add" } }] }
///
/// "toolCall": { "kind": "execute", "title": "git init", "locations": [],
///               "content": [{ "type": "content",
///                             "content": { "type": "text", "text": "[current working
///                                          directory C:\\...] (Initializing a new Git ...)" } }] }
/// ```
///
/// **`title` alone is what used to be shown, and it is a filename or a command with no body.**
/// *Writing to notes.txt* does not say what it writes; `git init` does not say where. Both were
/// on the wire already.
///
/// A `kind` this has never seen becomes [`Needs::Unstated`] rather than the commonest answer.
fn proposal_in(call: Option<&serde_json::Value>) -> Proposal {
    let Some(call) = call else {
        return Proposal::plain(Needs::Unstated, "run one of its own tools");
    };
    let text = |at: &str| {
        call.pointer(at)
            .and_then(serde_json::Value::as_str)
            .filter(|found| !found.is_empty())
            .map(str::to_owned)
    };

    // ACP names the kind on the call itself. `think` is not permission-gated — measured, it goes
    // straight to `in_progress` — so it is not a case here; anything unmeasured says so.
    let needs = match call.get("kind").and_then(serde_json::Value::as_str) {
        Some("edit") => Needs::Write,
        Some("execute") => Needs::Execute,
        _ => Needs::Unstated,
    };

    let what = text("/title")
        .or_else(|| text("/toolCallId"))
        .unwrap_or_else(|| "run one of its own tools".to_owned());

    Proposal {
        needs,
        what,
        shown: described(call),
    }
}

/// The change or the command itself, rendered for somebody about to answer for it.
///
/// **Not a diff algorithm.** Computing one would be a second implementation of something the
/// agent may send properly later, and the question a person is answering is *is this the thing I
/// meant* rather than *which lines moved*. Whole contents for a new file, both sides when
/// something is being replaced, and the path above either — because a correct change to the
/// wrong file is the failure a preview exists to catch.
fn described(call: &serde_json::Value) -> Option<String> {
    let parts = call.get("content").and_then(serde_json::Value::as_array)?;
    let mut said = String::new();
    for part in parts {
        let one = match part.get("type").and_then(serde_json::Value::as_str) {
            Some("diff") => a_diff(part),
            // The agent's own prose about what it is about to run, which is where a command's
            // working directory and reason live.
            Some("content") => part
                .pointer("/content/text")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            _ => None,
        };
        let Some(one) = one else { continue };
        if !said.is_empty() {
            said.push_str("\n\n");
        }
        said.push_str(&one);
    }
    if said.is_empty() {
        return None;
    }
    Some(within(said))
}

/// One `diff` entry, as a person reads it.
fn a_diff(part: &serde_json::Value) -> Option<String> {
    let text = |key: &str| {
        part.get(key)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
    };
    let path = text("path");
    let old = text("oldText");
    let new = text("newText");
    if path.is_empty() && old.is_empty() && new.is_empty() {
        return None;
    }
    let mut said = String::new();
    if !path.is_empty() {
        said.push_str(path);
        said.push_str("\n\n");
    }
    // A file being created has nothing to compare against, and printing an empty WAS block would
    // invite somebody to read a blank as a deletion.
    if old.is_empty() {
        said.push_str(new);
    } else {
        said.push_str("WAS\n");
        said.push_str(old);
        said.push_str("\n\nBECOMES\n");
        said.push_str(new);
    }
    Some(said)
}

/// Cut to [`SHOWN_LIMIT`], on a character boundary, saying that it was cut.
///
/// Silently truncating a preview is worse than not showing one: the part a person cannot see is
/// the part they would have objected to, and nothing on screen would say it existed.
fn within(said: String) -> String {
    if said.len() <= SHOWN_LIMIT {
        return said;
    }
    let mut end = SHOWN_LIMIT;
    while end > 0 && !said.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n\n… {} more characters, not shown.",
        &said[..end],
        said.len() - end
    )
}

/// One JSON-RPC request, on its own line.
fn send_rpc<W: std::io::Write>(
    input: &mut W,
    id: u64,
    method: &str,
    params: serde_json::Value,
) -> bool {
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    });
    trace('>', &message.to_string());
    serde_json::to_writer(&mut *input, &message).is_ok()
        && input.write_all(b"\n").is_ok()
        && input.flush().is_ok()
}

/// Answer a request the agent made of Epoch.
///
/// **One method matters and the rest are refused politely.** `session/request_permission` is the
/// whole reason this transport exists: the agent is paused, nothing has run, and the tool call it
/// wants is described in its own words. Anything else Epoch does not implement is answered with
/// a method-not-found rather than left hanging, because an agent waiting on a reply that never
/// comes is a turn that never ends.
fn answer_request<W: std::io::Write>(
    input: &mut W,
    message: &serde_json::Value,
    id: u64,
    task: &Task,
    approver: &dyn Approver,
    sink: &mut dyn FnMut(Progress),
) {
    let method = message
        .get("method")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if method != "session/request_permission" {
        let _ = reply_rpc(
            input,
            id,
            serde_json::json!({ "code": -32601, "message": "Epoch does not implement that" }),
            true,
        );
        return;
    }

    let params = message.get("params").cloned().unwrap_or_default();
    let proposal = proposal_in(params.get("toolCall"));

    sink(Progress::Said(String::new()));
    let allowed = approver.allow_for_this_turn(&proposal);

    // The options are the agent's to name, so they are read rather than assumed: pick the one
    // whose `kind` says what Epoch decided, and fall back to nothing rather than guessing an id.
    let want = if allowed { "allow_once" } else { "reject_once" };
    let option = params
        .get("options")
        .and_then(serde_json::Value::as_array)
        .and_then(|all| {
            all.iter()
                .find(|one| one.get("kind").and_then(serde_json::Value::as_str) == Some(want))
        })
        .and_then(|one| one.get("optionId").and_then(serde_json::Value::as_str))
        .map(str::to_owned);

    let outcome = match option {
        Some(option) => {
            serde_json::json!({ "outcome": { "outcome": "selected", "optionId": option } })
        }
        // An agent that offers no way to say what Epoch decided is answered *cancelled*, which
        // ACP defines and which is the safe direction: silence must never read as permission.
        None => serde_json::json!({ "outcome": { "outcome": "cancelled" } }),
    };
    let _ = reply_rpc(input, id, outcome, false);
    let _ = task;
}

/// One JSON-RPC reply, result or error, on its own line.
fn reply_rpc<W: std::io::Write>(
    input: &mut W,
    id: u64,
    body: serde_json::Value,
    is_error: bool,
) -> bool {
    let message = if is_error {
        serde_json::json!({ "jsonrpc": "2.0", "id": id, "error": body })
    } else {
        serde_json::json!({ "jsonrpc": "2.0", "id": id, "result": body })
    };
    trace('>', &message.to_string());
    serde_json::to_writer(&mut *input, &message).is_ok()
        && input.write_all(b"\n").is_ok()
        && input.flush().is_ok()
}

/// One `session/update` notification, folded into the turn.
fn update(message: &serde_json::Value, turn: &mut Turn, sink: &mut dyn FnMut(Progress)) {
    let update = message
        .pointer("/params/update")
        .unwrap_or(&serde_json::Value::Null);
    let kind = update
        .get("sessionUpdate")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    match kind {
        "agent_message_chunk" => {
            if let Some(text) = update
                .pointer("/content/text")
                .and_then(serde_json::Value::as_str)
            {
                turn.said.push_str(text);
                sink(Progress::Said(text.to_owned()));
            }
        }
        // **Thoughts are not what the character said.** They arrive on their own channel here,
        // which is the one thing the old transport could not tell apart, and they stay out of
        // the Chronicle.
        "agent_thought_chunk" => {}
        "tool_call" => {
            if let (Some(id), Some(title)) = (
                update.get("toolCallId").and_then(serde_json::Value::as_str),
                update.get("title").and_then(serde_json::Value::as_str),
            ) {
                turn.reaching.insert(id.to_owned(), title.to_owned());
            }
        }
        "tool_call_update" => {
            let Some(id) = update.get("toolCallId").and_then(serde_json::Value::as_str) else {
                return;
            };
            let status = update
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("finished");
            if status == "pending" || status == "in_progress" {
                return;
            }
            let tool = turn
                .reaching
                .remove(id)
                // An id with no name is still a thing that ran, and saying so beats silence.
                .unwrap_or_else(|| id.to_owned());
            sink(Progress::Ran {
                tool,
                detail: status.to_owned(),
                ok: status == "completed",
            });
        }
        _ => {}
    }
}

/// Which model actually answered, from the quota the turn reports about itself.
///
/// The same fact the old transport read out of `stats`, in the place ACP puts it. `None` when it
/// says nothing — an invented name is worse than an empty gauge.
fn who_thought_acp(message: &serde_json::Value) -> Option<String> {
    let used = message
        .pointer("/result/_meta/quota/model_usage")?
        .as_array()?;
    let mut names: Vec<&str> = used
        .iter()
        .filter_map(|one| one.get("model").and_then(serde_json::Value::as_str))
        .collect();
    names.dedup();
    if names.is_empty() {
        return None;
    }
    Some(names.join(" and "))
}

const PROBE_PATIENCE: std::time::Duration = std::time::Duration::from_secs(6);

/// Which sign-in method the user chose, and whether they chose one at all.
///
/// `(signed_in, method)` — the pair, because the two answers have different fixes: *nobody has
/// configured this yet* is solved by `/auth`, and *a configured method that fails* is solved
/// somewhere else entirely.
///
/// **Free, and offline.** The alternative is a request, and a request against this CLI costs the
/// user tokens whether or not it succeeds — `query` is a positional argument, so a guessed flag
/// is a billed prompt rather than a rejected one.
///
/// Read from the same file the CLI's own `/about` reports it from. Every failure to read yields
/// `None` — *unasked* — never a negative answer.
fn chosen_method() -> (Option<bool>, Option<String>) {
    let Some(home) = home() else {
        return (None, None);
    };
    let Ok(raw) = std::fs::read_to_string(home.join(".gemini").join("settings.json")) else {
        // No file at all is not the same as a file that says nothing: this program writes it on
        // first run, so its absence means it has never been run rather than never signed in.
        return (None, None);
    };
    let Ok(settings) = serde_json::from_str::<serde_json::Value>(&raw) else {
        // A shape we cannot read is not an answer. A later version that reorganises this must
        // make Epoch quiet, never make it wrong.
        return (None, None);
    };

    match settings
        .get("security")
        .and_then(|security| security.get("auth"))
        .and_then(|auth| auth.get("selectedType"))
        .and_then(serde_json::Value::as_str)
        .filter(|chosen| !chosen.is_empty())
    {
        // Chosen, not verified. Saying `true` here would promise a working credential, and a
        // chosen method against an exhausted quota is exactly the case that disproves it.
        Some(method) => (None, Some(in_words(method))),
        // The file exists and names no method: this program has run and nobody has signed in.
        // The one case where `false` is a measurement rather than a guess.
        None => (Some(false), None),
    }
}

/// One of this CLI's auth ids, in words a person chose from a menu.
///
/// The mapping lives here rather than in a surface, for the same reason a Provider declares its
/// own controls (ADR-0003): `gemini-api-key` is Gemini's vocabulary, and a frontend that knew it
/// would need editing to add the next agent.
///
/// **An id nobody here recognises is shown as it is**, never dropped and never guessed at. This
/// program can add a method any day, and an unfamiliar word on the screen is information; a
/// blank space is not.
fn in_words(method: &str) -> String {
    match method {
        // The three offered by `/auth`, in its own order.
        "oauth-personal" => "Google Account".to_owned(),
        "gemini-api-key" => "Gemini API Key".to_owned(),
        "vertex-ai" => "Vertex AI".to_owned(),
        other => other.to_owned(),
    }
}

fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
}

/// Its own startup remarks, which are not complaints.
fn is_noise(line: &str) -> bool {
    let line = line.trim();
    line.is_empty()
        || line.starts_with("Warning: True color")
        || line.starts_with("Ripgrep is not available")
}

/// Who is working, and what they were asked — the same shape Codex is given.
fn prompt(task: &Task) -> String {
    let mut written = format!(
        "You are {}, working inside Epoch as one of a crew.\n\n{}",
        task.character,
        task.persona.trim()
    );
    for paragraph in [&task.crew, &task.skills, &task.library, &task.connections] {
        if !paragraph.trim().is_empty() {
            written.push_str("\n\n");
            written.push_str(paragraph.trim());
        }
    }
    written.push_str("\n\n---\n\n");
    written.push_str(task.intent.trim());
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **`Manual` is the mode that asks, and it is never `plan`.**
    ///
    /// `plan` is Gemini's read-only mode, not a permission one. Mapping Manual onto it is what
    /// produced a write nobody approved: asked in Manual to create a file, it created it. The
    /// names come from the agent's own `session/new` answer — `default` is described there as
    /// *"Prompts for approval"*.
    #[test]
    fn manual_is_the_mode_that_asks() {
        use epoch_kernel::Autonomy::*;
        assert_eq!(mode_for(Manual), "default");
        assert_eq!(mode_for(AcceptEdits), "autoEdit");
        assert_eq!(mode_for(Auto), "yolo");
        for autonomy in [Manual, AcceptEdits, Auto] {
            assert_ne!(
                mode_for(autonomy),
                "plan",
                "read-only is not a permission mode"
            );
        }
    }

    /// The command line is `--acp`, plus a model only when one was chosen.
    #[test]
    fn the_transport_is_named_and_the_model_only_when_there_is_one() {
        let mut task = a_real_briefing();
        let said = acp_arguments(&task);
        assert_eq!(said.first().map(String::as_str), Some("--acp"));
        assert_eq!(said.iter().filter(|it| *it == "--model").count(), 1);

        task.model = String::new();
        let bare = acp_arguments(&task);
        assert_eq!(bare, vec!["--acp".to_owned()]);
    }

    /// The three options every question carries, recorded off gemini-cli 0.56.0.
    fn the_usual_options() -> serde_json::Value {
        serde_json::json!([
            { "optionId": "proceed_always", "name": "Allow for this session",
              "kind": "allow_always" },
            { "optionId": "proceed_once", "name": "Allow", "kind": "allow_once" },
            { "optionId": "cancel", "name": "Reject", "kind": "reject_once" }
        ])
    }

    /// A real `session/request_permission` for a **write**, off gemini-cli 0.56.0, 2026-09-08.
    ///
    /// Kept verbatim rather than paraphrased: a fixture written from memory tests the memory.
    ///
    /// **The first version of this fixture was abridged** -- an elided path, no `newText`, no
    /// `kind` and no `locations` -- recorded when only the reply was under test. Every field
    /// Epoch now shows a person was already on the wire and had been left out of the record of
    /// it, which is how the preview came to be missing for a year of this file's life.
    fn a_real_question() -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "session/request_permission",
            "params": {
                "sessionId": "cff5ef18-3dea-447b-8075-fc146cf73470",
                "options": the_usual_options(),
                "toolCall": {
                    "toolCallId": "write_file__call_563873",
                    "status": "pending",
                    "title": "Writing to notes.txt",
                    "content": [{
                        "type": "diff",
                        "path": "C:\\Users\\someone\\Temp\\acp-probe\\notes.txt",
                        "oldText": "",
                        "newText": "ADIOS",
                        "_meta": { "kind": "add" }
                    }],
                    "locations": [
                        { "path": "C:\\Users\\someone\\Temp\\acp-probe\\notes.txt" }
                    ],
                    "kind": "edit"
                }
            }
        })
    }

    /// A real one for a **command**, recorded the same afternoon by asking for `git init`.
    ///
    /// `echo hola` never reached this point -- gemini-cli approves what it judges safe on its
    /// own -- so the fixture is the first command that actually stopped.
    fn a_real_command() -> serde_json::Value {
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 0,
            "method": "session/request_permission",
            "params": {
                "sessionId": "a2f422b8-401d-47c2-a75b-09e5753a2eef",
                "options": the_usual_options(),
                "toolCall": {
                    "toolCallId": "run_shell_command__call_795465",
                    "status": "pending",
                    "title": "git init",
                    "content": [{
                        "type": "content",
                        "content": {
                            "type": "text",
                            "text": "[current working directory C:\\Users\\someone\\Temp\\acp-probe] (Initializing a new Git repository.)"
                        }
                    }],
                    "locations": [],
                    "kind": "execute"
                }
            }
        })
    }

    /// Answers the same way every time, and keeps what it was asked.
    ///
    /// Keeping it is the point: the reply that goes back on the wire proves the decision
    /// travelled, and only the proposal proves the *question* did.
    struct Answers {
        say: bool,
        seen: std::cell::RefCell<Option<Proposal>>,
    }

    impl Answers {
        fn saying(say: bool) -> Self {
            Answers {
                say,
                seen: std::cell::RefCell::new(None),
            }
        }

        fn asked(&self) -> Proposal {
            self.seen.borrow().clone().expect("it was asked")
        }
    }

    impl Approver for Answers {
        fn allow_for_this_turn(&self, proposal: &Proposal) -> bool {
            *self.seen.borrow_mut() = Some(proposal.clone());
            self.say
        }
    }

    /// **A person answering for a write sees the write.**
    ///
    /// The defect this replaces: every agent question arrived as *write access* with the title
    /// alone -- *Writing to notes.txt*, which names a file and not one thing that goes into it.
    /// Everything asserted here was already on the wire and was being dropped.
    #[test]
    fn a_write_is_shown_with_the_change_it_would_make() {
        let asked = Answers::saying(false);
        let mut wire = Vec::new();
        let mut nothing = |_: Progress| {};
        answer_request(
            &mut wire,
            &a_real_question(),
            0,
            &a_real_briefing(),
            &asked,
            &mut nothing,
        );

        let proposal = asked.asked();
        assert_eq!(proposal.needs, Needs::Write);
        assert_eq!(proposal.what, "Writing to notes.txt");
        let shown = proposal.shown.expect("the diff was on the wire");
        assert!(shown.contains("notes.txt"), "{shown}");
        assert!(shown.contains("ADIOS"), "{shown}");
    }

    /// **A command is not a write, and saying so is the point.**
    ///
    /// `git init` was labelled *write access*, so somebody could approve a sentence about files
    /// and get a program run. Codex had the same flattening through a different door: it knows
    /// `item/commandExecution/requestApproval` from `item/fileChange/requestApproval` and said
    /// `Needs::Write` for both.
    #[test]
    fn a_command_is_not_called_a_write() {
        let asked = Answers::saying(false);
        let mut wire = Vec::new();
        let mut nothing = |_: Progress| {};
        answer_request(
            &mut wire,
            &a_real_command(),
            0,
            &a_real_briefing(),
            &asked,
            &mut nothing,
        );

        let proposal = asked.asked();
        assert_eq!(proposal.needs, Needs::Execute);
        assert_eq!(proposal.what, "git init");
        // Where it would run. A command without its working directory is half a question.
        let shown = proposal.shown.expect("the agent said where and why");
        assert!(shown.contains("acp-probe"), "{shown}");
    }

    /// **A kind nobody has measured says so, rather than borrowing the commonest answer.**
    ///
    /// ACP names more kinds than Epoch has seen. Mapping the unseen ones onto `Write` would put
    /// a confident, wrong word above somebody's decision -- the same inversion as reading
    /// silence as *no*, failing in the direction that looks familiar.
    #[test]
    fn a_kind_nobody_has_measured_is_not_called_a_write() {
        let mut question = a_real_question();
        question["params"]["toolCall"]["kind"] = serde_json::json!("delete");
        let asked = Answers::saying(false);
        let mut wire = Vec::new();
        let mut nothing = |_: Progress| {};
        answer_request(
            &mut wire,
            &question,
            0,
            &a_real_briefing(),
            &asked,
            &mut nothing,
        );
        assert_eq!(asked.asked().needs, Needs::Unstated);
    }

    /// **Replacing something shows what is being replaced.**
    ///
    /// A new file has nothing to compare against and prints its whole contents; a change has two
    /// sides, and showing only the new one hides the deletion inside it.
    #[test]
    fn a_replacement_shows_both_sides() {
        let mut question = a_real_question();
        question["params"]["toolCall"]["content"][0]["oldText"] = serde_json::json!("HOLA");
        let shown = proposal_in(question["params"].get("toolCall"))
            .shown
            .expect("a diff");
        assert!(shown.contains("HOLA"), "{shown}");
        assert!(shown.contains("ADIOS"), "{shown}");

        // And a file being created must not print an empty WAS block, which reads as a deletion.
        let fresh = proposal_in(a_real_question()["params"].get("toolCall"))
            .shown
            .expect("a diff");
        assert!(!fresh.contains("WAS"), "{fresh}");
    }

    /// **A preview too big to show says it was cut.**
    ///
    /// Silently truncating is worse than showing nothing: the part nobody can see is the part
    /// they would have objected to, and the screen would not say it existed.
    #[test]
    fn a_preview_that_does_not_fit_says_so() {
        let mut question = a_real_question();
        question["params"]["toolCall"]["content"][0]["newText"] =
            serde_json::json!("x".repeat(SHOWN_LIMIT * 2));
        let shown = proposal_in(question["params"].get("toolCall"))
            .shown
            .expect("a diff");
        assert!(shown.len() < SHOWN_LIMIT * 2, "it was cut");
        assert!(shown.contains("not shown"), "and it says so: {shown}");
    }

    /// **A question with no `toolCall` at all is still answerable, and claims nothing.**
    #[test]
    fn nothing_described_is_not_nothing_asked() {
        let proposal = proposal_in(None);
        assert_eq!(proposal.needs, Needs::Unstated);
        assert_eq!(proposal.shown, None);
    }

    /// **What Epoch decided is what goes back on the wire.**
    ///
    /// The option ids are the agent's to choose, so they are read out of the question rather
    /// than assumed: the reply carries whichever `optionId` has the `kind` Epoch meant. A test
    /// that asserted the literal `proceed_once` would pass against a version that hardcoded it
    /// and fail the day Gemini renames one — this asserts the *lookup*, by also proving the
    /// no-match case is not silently an approval.
    #[test]
    fn the_answer_epoch_gave_is_the_option_that_goes_back() {
        let mut wire = Vec::new();
        let mut nothing = |_: Progress| {};
        answer_request(
            &mut wire,
            &a_real_question(),
            0,
            &a_real_briefing(),
            &Answers::saying(false),
            &mut nothing,
        );
        let said: serde_json::Value =
            serde_json::from_slice(&wire).expect("one JSON-RPC reply per line");
        assert_eq!(said["result"]["outcome"]["optionId"], "cancel");

        let mut wire = Vec::new();
        answer_request(
            &mut wire,
            &a_real_question(),
            0,
            &a_real_briefing(),
            &Answers::saying(true),
            &mut nothing,
        );
        let said: serde_json::Value = serde_json::from_slice(&wire).expect("a reply");
        assert_eq!(said["result"]["outcome"]["optionId"], "proceed_once");
    }

    /// **An agent offering no way to say what Epoch decided is answered *cancelled*.**
    ///
    /// Never the first option it happens to list. Silence and confusion must both fall on the
    /// side of *nothing ran*: an agent must never infer permission from a client that could not
    /// express a refusal.
    #[test]
    fn no_option_matching_the_decision_is_a_refusal() {
        let mut question = a_real_question();
        question["params"]["options"] = serde_json::json!([
            { "optionId": "proceed_always", "kind": "allow_always" }
        ]);
        let mut wire = Vec::new();
        let mut nothing = |_: Progress| {};
        answer_request(
            &mut wire,
            &question,
            0,
            &a_real_briefing(),
            &Answers::saying(true),
            &mut nothing,
        );
        let said: serde_json::Value = serde_json::from_slice(&wire).expect("a reply");
        assert_eq!(said["result"]["outcome"]["outcome"], "cancelled");
        assert!(said["result"]["outcome"]["optionId"].is_null());
    }

    /// Anything else the agent asks of Epoch is refused rather than left hanging.
    #[test]
    fn a_request_epoch_does_not_implement_is_answered() {
        let mut wire = Vec::new();
        let mut nothing = |_: Progress| {};
        answer_request(
            &mut wire,
            &serde_json::json!({ "id": 7, "method": "fs/write_text_file", "params": {} }),
            7,
            &a_real_briefing(),
            &Answers::saying(true),
            &mut nothing,
        );
        let said: serde_json::Value = serde_json::from_slice(&wire).expect("a reply");
        assert_eq!(said["error"]["code"], -32601);
    }

    /// **What the character said reaches the Chronicle; what it thought does not.**
    ///
    /// The old transport could not tell them apart. ACP puts them on separate updates, and a
    /// thought is working-out rather than an answer.
    #[test]
    fn thinking_out_loud_is_not_what_was_said() {
        let mut turn = Turn {
            said: String::new(),
            session: None,
            reaching: std::collections::HashMap::new(),
            done: false,
            failure: None,
        };
        let mut heard = String::new();
        let mut listening = |progress: Progress| {
            if let Progress::Said(text) = progress {
                heard.push_str(&text);
            }
        };
        for (kind, text) in [
            ("agent_thought_chunk", "**Focusing on Guidelines**"),
            ("agent_message_chunk", "LISTO"),
        ] {
            update(
                &serde_json::json!({
                    "method": "session/update",
                    "params": { "update": {
                        "sessionUpdate": kind,
                        "content": { "type": "text", "text": text }
                    } }
                }),
                &mut turn,
                &mut listening,
            );
        }
        assert_eq!(turn.said, "LISTO");
        assert_eq!(heard, "LISTO");
    }

    /// A tool is named from when it was asked for, because the update that ends it carries an id.
    #[test]
    fn a_tool_that_finished_is_named_rather_than_numbered() {
        let mut turn = Turn {
            said: String::new(),
            session: None,
            reaching: std::collections::HashMap::new(),
            done: false,
            failure: None,
        };
        let mut ran = Vec::new();
        let mut listening = |progress: Progress| {
            if let Progress::Ran { tool, ok, .. } = progress {
                ran.push((tool, ok));
            }
        };
        let note = |kind: &str, extra: serde_json::Value| {
            let mut update = serde_json::json!({
                "sessionUpdate": kind,
                "toolCallId": "write_file__call_563873",
            });
            for (key, value) in extra.as_object().cloned().unwrap_or_default() {
                update[key] = value;
            }
            serde_json::json!({ "method": "session/update", "params": { "update": update } })
        };
        update(
            &note(
                "tool_call",
                serde_json::json!({ "title": "Writing to acp-probe.txt" }),
            ),
            &mut turn,
            &mut listening,
        );
        // In flight is not finished, and reporting it as finished would be a lie twice a turn.
        update(
            &note(
                "tool_call_update",
                serde_json::json!({ "status": "in_progress" }),
            ),
            &mut turn,
            &mut listening,
        );
        update(
            &note(
                "tool_call_update",
                serde_json::json!({ "status": "completed" }),
            ),
            &mut turn,
            &mut listening,
        );
        // In flight is not finished: exactly one report, and it is the one that ended.
        let ran = {
            let _ = &listening;
            ran
        };
        assert_eq!(ran, vec![("Writing to acp-probe.txt".to_owned(), true)]);
    }

    #[test]
    fn a_chosen_method_is_named_but_never_promised_to_work() {
        // Measured on this machine: `/about` reports "Auth Method: gemini-api-key", and the
        // settings file is where it reads that. Choosing a method is not proof the credential
        // works — an exhausted quota is a configured account that fails — so `signed_in` stays
        // unasked and only the method is reported.
        let dir = std::env::temp_dir().join("epoch-gemini-auth-chosen");
        let home = dir.join(".gemini");
        std::fs::create_dir_all(&home).expect("a temporary home");
        std::fs::write(
            home.join("settings.json"),
            r#"{"security":{"auth":{"selectedType":"gemini-api-key"}}}"#,
        )
        .expect("writes");

        let said = with_home(&dir, chosen_method);
        // Shown the way it was offered, not as the id in the file.
        assert_eq!(said, (None, Some("Gemini API Key".to_owned())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_that_names_no_method_is_the_one_case_that_is_a_real_no() {
        // The file exists, so the program has run. Nobody signed in, and that has a fix worth
        // naming: open `gemini`, run `/auth`.
        let dir = std::env::temp_dir().join("epoch-gemini-auth-none");
        let home = dir.join(".gemini");
        std::fs::create_dir_all(&home).expect("a temporary home");
        std::fs::write(home.join("settings.json"), "{}").expect("writes");

        assert_eq!(with_home(&dir, chosen_method), (Some(false), None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_or_absent_file_says_nothing_rather_than_no() {
        // The rule the whole integration is built on: a later version that reorganises this
        // file must make Epoch quiet, never make it wrong.
        let dir = std::env::temp_dir().join("epoch-gemini-auth-broken");
        let home = dir.join(".gemini");
        std::fs::create_dir_all(&home).expect("a temporary home");
        std::fs::write(home.join("settings.json"), "not json at all").expect("writes");
        assert_eq!(with_home(&dir, chosen_method), (None, None));

        std::fs::remove_file(home.join("settings.json")).expect("removes");
        assert_eq!(with_home(&dir, chosen_method), (None, None));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Run something with `HOME`/`USERPROFILE` pointed somewhere disposable.
    ///
    /// Serialised, because these are process-wide: two of these running at once would each be
    /// reading the other's home directory, and the failure would look like a flaky test rather
    /// than the shared mutable state it is.
    fn with_home<T>(dir: &std::path::Path, run: impl FnOnce() -> T) -> T {
        static ONE_AT_A_TIME: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _held = ONE_AT_A_TIME
            .lock()
            .unwrap_or_else(|held| held.into_inner());

        let old_home = std::env::var_os("HOME");
        let old_profile = std::env::var_os("USERPROFILE");
        std::env::set_var("HOME", dir);
        std::env::set_var("USERPROFILE", dir);
        let said = run();
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match old_profile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
        said
    }

    #[test]
    fn a_method_nobody_here_recognises_is_shown_rather_than_hidden() {
        // This program can add one any day. An unfamiliar word on the screen is information;
        // a blank space is not.
        assert_eq!(in_words("gemini-api-key"), "Gemini API Key");
        assert_eq!(in_words("oauth-personal"), "Google Account");
        assert_eq!(in_words("something-new-2027"), "something-new-2027");
    }

    /// A task shaped like the ones that failed: paragraphs in the persona and in the intent.
    fn a_real_briefing() -> Task {
        Task {
            intent: "list the files\n\nthen say DONE".into(),
            character: "Mage".into(),
            model: "gemini-3.1-pro-preview".into(),
            reasoning: None,
            persona: "You explore the solution space\nbefore committing.".into(),
            crew: String::new(),
            skills: String::new(),
            library: String::new(),
            connections: String::new(),
            images: Vec::new(),
            directory: std::env::temp_dir(),
            pictures: std::path::PathBuf::new(),
            thread: Some("269fd7e0-270d-4e51-969d-ae8bd2676b14".into()),
            autonomy: epoch_kernel::Autonomy::Manual,
            door: None,
        }
    }

    #[test]
    fn its_own_startup_remarks_are_not_complaints() {
        assert!(is_noise(
            "Warning: True color (24-bit) support not detected."
        ));
        assert!(is_noise(
            "Ripgrep is not available. Falling back to GrepTool."
        ));
        assert!(!is_noise("error: unexpected argument '--nope' found"));
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
        let mut task = a_real_briefing();
        task.directory = std::path::PathBuf::from(r"\\?\C:\Users\someone\a project");
        assert_eq!(here(&task), r"C:\Users\someone\a project");

        task.directory = std::path::PathBuf::from(r"\\?\UNC\server\share\work");
        assert_eq!(here(&task), r"\\server\share\work");

        // A path without the prefix is untouched, so this cannot quietly rewrite anything else.
        task.directory = std::path::PathBuf::from(r"C:\Users\someone\plain");
        assert_eq!(here(&task), r"C:\Users\someone\plain");
    }

    /// **What this machine's Gemini actually offers.** Opt-in: it starts the CLI.
    ///
    /// Not asserted against a list of names, which is the thing this replaced. What must hold is
    /// that an answer came back and that `auto` — the router that is Gemini's own default, and
    /// the one a character gets when nobody chose — is among them.
    #[test]
    #[ignore = "needs a signed-in Gemini CLI"]
    fn gemini_actually_names_its_models_on_this_machine() {
        let found = models_offered().expect("Gemini should name its models in session/new");
        assert!(!found.is_empty());
        for (id, label) in &found {
            assert!(!id.trim().is_empty(), "every row is addressable");
            assert!(!label.trim().is_empty(), "every row is readable");
            println!("{id} · {label}");
        }
        assert!(found.iter().any(|(id, _)| id == "auto"), "its own default");
    }

    /// **STOP is not a failure to start, and the difference is a sentence the user reads.**
    ///
    /// Measured in the installed window: STOP pressed 2.1 s into a turn, idle again at 4.1 s, and
    /// the Chronicle read `gemini could not be started: it said nothing`. Nothing had gone wrong;
    /// the user had cancelled. Asserted as the *variant* rather than the wording, because what
    /// must hold is that the turn loop can tell the two apart.
    #[test]
    #[ignore = "needs a signed-in Gemini CLI"]
    fn a_cancelled_turn_says_it_was_cancelled() {
        let mut task = a_real_briefing();
        task.thread = None;
        task.directory = std::env::temp_dir();
        task.intent = "Count slowly from one to two hundred, one per line.".into();

        let began = std::time::Instant::now();
        // Stopped from the first check, so this never reaches the model and costs nothing.
        let out = Gemini.work(&task, &|| true, &crate::agent::NobodyToAsk, &mut |_| {});
        println!("after {:.1}s: {out:?}", began.elapsed().as_secs_f32());
        assert!(
            matches!(out, Err(AgentError::Stopped { .. })),
            "a cancelled turn is Stopped, never Failed"
        );
    }

    /// **The real program, through the real transport.** Opt-in, because it costs a request.
    ///
    /// `cargo test -p epoch-engine -- --ignored --nocapture gemini_actually` on a machine with a
    /// signed-in Gemini CLI. Manual autonomy and an approver that always refuses, so it must
    /// arrive at a permission question and the file must not exist afterwards.
    ///
    /// Measuring the wire from a script proves the protocol; only this proves the path Epoch
    /// takes (`CLAUDE.md`, 11.18).
    #[test]
    #[ignore = "needs a signed-in Gemini CLI"]
    fn gemini_actually_asks_before_it_writes() {
        let dir = std::env::temp_dir().join("epoch-gemini-live");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // **Canonicalised on purpose.** A Project Root is, so a fixture that hands over
        // `std::env::temp_dir()` is measuring a path the product does not take — which is
        // exactly how the extended-length `cwd` reached a release.
        let dir = std::fs::canonicalize(&dir).unwrap();
        let file = dir.join("live.txt");

        let mut task = a_real_briefing();
        task.thread = None;
        task.directory = dir.clone();
        task.model = std::env::var("EPOCH_GEMINI_MODEL")
            .unwrap_or_else(|_| "gemini-3.1-flash-lite".to_owned());
        task.intent = "Write a file named live.txt with the word HOLA in it.".into();
        task.persona = "You are Robo.".into();

        struct Refuses;
        impl Approver for Refuses {
            fn allow_for_this_turn(&self, proposal: &Proposal) -> bool {
                println!("ASKED for {:?}: {}", proposal.needs, proposal.what);
                // Printed, because the whole point of this run is that the person answering
                // sees more than a filename. A live test asserting only a boolean would have
                // passed against the version that dropped it.
                if let Some(shown) = &proposal.shown {
                    println!("---- shown ----");
                    println!("{shown}");
                    println!("---------------");
                }
                false
            }
        }

        let began = std::time::Instant::now();
        let out = Gemini.work(&task, &|| false, &Refuses, &mut |progress| {
            println!("[{:>6.1}s] {progress:?}", began.elapsed().as_secs_f32())
        });
        println!("after {:.1}s: {out:?}", began.elapsed().as_secs_f32());
        assert!(!file.exists(), "a refused write must leave nothing behind");
    }
}
