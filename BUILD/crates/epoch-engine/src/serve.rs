//! Epoch, answering the Model Context Protocol (step 5.2).
//!
//! ## Why this exists, and it is not interoperability
//!
//! An agent — Claude Code, Codex — owns its own loop (ADR-0027). It decides what to run and it
//! runs it. That is what makes it an agent rather than a model, and it is also the problem:
//! **an agent running its own permission prompt turns ADR-0009 into decoration.** A character
//! set to `Manual` would be writing files while Epoch's own dropdown said it asks first.
//!
//! So Epoch does not ask the agent to behave. It takes the tools away and offers its own.
//! Claude Code is configured with no filesystem access and one MCP server — this one — and
//! every file it reads, every command it runs, arrives here as a `tools/call` and is judged by
//! the same `decide()` that judges a model's tool call. Same capabilities, same explanations,
//! same Trust store, same record.
//!
//! That is the whole of 5.2, and it is why it blocks 6.x rather than the other way round.
//!
//! ## A connection belongs to a character
//!
//! `decide()` needs a [`Situation`]: who, where, in what mode. An MCP server that answered
//! "some client" would have nowhere to get one, and the obvious repair — a special identity for
//! agents — would be a second permission system wearing the first one's clothes.
//!
//! There is no need to invent anything. An agent is a character's **brain** (ADR-0027), so its
//! calls are that character's calls, in that character's World, at that character's mode. The
//! connection is bound to a character when it opens; a call arriving without one is refused
//! rather than defaulted, because a default here is an unowned action.
//!
//! ## Effects are declared honestly, and relied on never
//!
//! [`crate::mcp`] refuses the hints an outside server offers about itself — `readOnlyHint` and
//! friends — because a third party grading its own risk is the one claim ADR-0008 exists to
//! make inexpressible. Serving is the mirror of that, and the rule does not flip: Epoch emits
//! `readOnlyHint` where its descriptor genuinely declares observation alone, because it is
//! true and a well-behaved client can use it to decide what to even attempt.
//!
//! But nothing here **depends** on the client having read it. The gate is `decide()`, on this
//! side, on every call, whatever the client did with the hint. A hint is a courtesy; the
//! decision is the control — the same sentence the Character parameters follow about a
//! surface's checks.
//!
//! ## Pure, like `decide()` is
//!
//! No sockets, no processes, no filesystem. This turns a JSON-RPC message into either a reply
//! or an **instruction to the caller**: run this capability, having already been judged. The
//! shell executes it and asks the user when the verdict says to — because the user is looking
//! at Epoch, not at the agent's terminal, and that is exactly the point.
//!
//! Keeping the protocol free of I/O is what lets the whole of 5.2 be tested without a socket.

use epoch_kernel::{
    Arguments, CapabilityId, CharacterId, Descriptor, Explanation, Policy, Situation, Verdict,
};
use serde_json::{json, Value};

use crate::capability::CapabilityRegistry;

/// The revision of MCP this speaks.
///
/// Stated rather than echoed back from whatever the client asked for. Agreeing to a version we
/// do not implement is how a protocol mismatch becomes a runtime surprise three calls later.
pub const PROTOCOL: &str = "2025-06-18";

/// What Epoch calls itself to a client.
pub const SERVER: &str = "epoch";

/// What the transport must do with a message.
#[derive(Debug, Clone, PartialEq)]
pub enum Serve {
    /// Send this back, as it is.
    Reply(Value),
    /// Say nothing. A notification has no reply, and inventing one is a protocol error.
    Silent,
    /// **The agent is asking whether its own tool may run.**
    ///
    /// Not a capability call. Claude Code takes `--permission-prompt-tool <name>` and, before
    /// running any tool of *its own*, calls that tool with what it is about to do. Verified
    /// against 2.1.224 with a throwaway server: `{"tool_name":"Write","input":{…},
    /// "tool_use_id":"toolu_…"}` arrives, `{"behavior":"allow","updatedInput":{…}}` lets it
    /// through and `{"behavior":"deny","message":"…"}` stops it — the file was created in one
    /// run and never existed in the other.
    ///
    /// This is what `Manual` was missing. Epoch does not take the agent's tools away
    /// (`CLAUDE.md`, 2026-08-07) — it is *invited* to the decision, by the agent, through a door
    /// the agent was told to knock on.
    Approve(Box<Approval>),
    /// **A character is asking to hand the work to somebody else.**
    ///
    /// Not a capability, and deliberately not an action: ADR-0025 says NPCs do not decide where
    /// a Quest goes, and of the three things allowed to move one, the user's explicit approval
    /// is the strongest — *"hand it to Robo" makes the cause visible, which is what separates
    /// exposing collaboration from simulating it.*
    ///
    /// So this is the same arrangement as [`Approve`](Self::Approve), one level up: the agent is
    /// invited to the decision and does not make it. It says who it wants and exactly what it
    /// would send; Epoch shows the user those words and asks. Nothing moves unless they say yes.
    HandOver(Box<Handover>),
    /// Run a capability and reply with what it produced.
    ///
    /// Carries the verdict rather than a boolean: `Asked` means the **user** must answer before
    /// anything happens, and the reply cannot be composed until they have. The agent waits, and
    /// it waits without knowing why — which is correct. It is not owed an explanation of the
    /// user's own permission model.
    Run(Box<Run>),
}

/// What Epoch's own permission tool is called.
///
/// Reachable to the agent as `mcp__epoch__approve` — the server's name, then this. Deliberately
/// not a `CapabilityId`: Epoch is not offering to write the file, it is offering to *answer*.
pub const APPROVE: &str = "approve";

/// The tool a character calls to *ask* for a handover.
///
/// Reachable as `mcp__epoch__hand_over`. Like `approve`, not a `CapabilityId`: Epoch is not
/// offering to do anything, it is offering to put a question to the user.
pub const HAND_OVER: &str = "hand_over";

/// What one character wants to give another, in its own words.
#[derive(Debug, Clone, PartialEq)]
pub struct Handover {
    pub id: Value,
    /// Who they want. An id or a name — resolved by the caller against the real crew, because a
    /// character naming somebody who does not live here is a thing that will happen.
    pub to: String,
    /// **Exactly what would be sent.** Shown to the user before they answer, and recorded as
    /// this character's own words if they say yes.
    ///
    /// The whole point of the flow: approving a handover you cannot read is approving a rumour.
    pub what: String,
}

/// The agent, asking about one of its own tools.
#[derive(Debug, Clone, PartialEq)]
pub struct Approval {
    /// The JSON-RPC id to answer.
    pub id: Value,
    /// The agent's name for its tool — `Write`, `Bash`. Not ours, and not translated: showing
    /// somebody `write_file` when their agent said `Write` would be Epoch renaming the thing it
    /// is asking them to approve.
    pub tool: String,
    /// Exactly what it intends to do, unedited. Handed back on approval so the agent runs what
    /// was shown rather than something composed here.
    pub input: Value,
    /// The agent's id for this call, carried through untouched.
    pub tool_use_id: Option<String>,
}

/// A call that survived parsing and judgement, ready for the shell to carry out.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// The JSON-RPC id to answer. Carried through so the transport never has to remember it.
    pub id: Value,
    pub capability: CapabilityId,
    pub arguments: Arguments,
    /// What will happen, in the user's terms — the sentence an approval prompt shows.
    pub explanation: Explanation,
    pub verdict: Verdict,
}

/// One connection: a character, their World, and what they are allowed to do.
///
/// Borrowed rather than owned, so nothing here can hold a stale copy of a policy the user has
/// since changed. The Trust store is read at the moment of the call, which is the only moment
/// its answer is true.
pub struct Serving<'a> {
    pub registry: &'a CapabilityRegistry,
    /// Whose hands these are. `None` before a client has said which character it is acting as.
    pub character: Option<&'a CharacterId>,
    pub world: &'a str,
    pub mode: epoch_kernel::Autonomy,
    pub policies: &'a [Policy],
}

impl Serving<'_> {
    /// Answer one JSON-RPC message.
    ///
    /// Everything arriving here came from **outside Epoch**, so nothing is assumed: an unknown
    /// method is an error rather than a panic, a missing field is an error rather than a
    /// default, and an unknown capability is refused by name.
    pub fn handle(&self, message: &Value) -> Serve {
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        // A message with no id is a notification. The protocol forbids answering one, and a
        // reply to a notification is what makes a client hang waiting for the next id.
        let id = message.get("id").cloned();

        match (method, id) {
            ("initialize", Some(id)) => Serve::Reply(result(id, self.hello())),
            ("ping", Some(id)) => Serve::Reply(result(id, json!({}))),
            ("tools/list", Some(id)) => Serve::Reply(result(id, self.catalogue())),
            ("tools/call", Some(id)) => self.call(id, message.get("params")),
            // Notifications, including `notifications/initialized`. Nothing to say, and saying
            // nothing is the correct answer rather than a gap.
            (_, None) => Serve::Silent,
            (unknown, Some(id)) => Serve::Reply(failed(
                id,
                METHOD_NOT_FOUND,
                format!("Epoch does not answer '{unknown}'"),
            )),
        }
    }

    /// What Epoch says about itself when a client opens.
    fn hello(&self) -> Value {
        json!({
            "protocolVersion": PROTOCOL,
            // Only what is actually implemented. Declaring `resources` or `prompts` because
            // they exist in the specification would be a promise a client would then call.
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": SERVER, "version": env!("CARGO_PKG_VERSION") },
            // The agent is told what it is holding. Not a courtesy — an agent that believes it
            // has a filesystem will keep trying to use one, and every attempt is a turn wasted
            // arguing with a tool that is not there.
            "instructions": INSTRUCTIONS,
        })
    }

    /// Every capability, in MCP's shape.
    ///
    /// The whole registry, not a filtered one. What a character may *use* is a Trust decision
    /// made per call with the arguments in hand — hiding a tool here would answer that question
    /// early, with less information, and permanently: an agent that never saw `write_file`
    /// cannot ask for it, so the user is never given the chance to say yes.
    fn catalogue(&self) -> Value {
        let mut tools: Vec<Value> = self.registry.describe_all().iter().map(declare).collect();
        // Listed, because the agent resolves `--permission-prompt-tool` against this list and a
        // name it cannot find is a permission tool that silently never runs.
        // Offered to every brain that can reach the door, which is what makes collaboration
        // work the same way whoever is doing it.
        tools.push(json!({
            "name": HAND_OVER,
            "description":
                "Ask the user to hand this work to another member of the crew. This does NOT \
                 hand it over: it shows the user exactly what you would send and lets them \
                 decide. Use it when the work needs somebody else — say who, and give the \
                 complete instruction you would pass them. If the user agrees, your words are \
                 recorded and they will pick it up next; if not, say so and stop. You can never \
                 act for a colleague or report that one of them did something.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "to": {
                        "type": "string",
                        "description": "The colleague's name or id, from the crew you were told about."
                    },
                    "what": {
                        "type": "string",
                        "description": "The complete instruction to pass on. The user reads this before deciding."
                    }
                },
                "required": ["to", "what"]
            }
        }));
        tools.push(json!({
            "name": APPROVE,
            "description": "Ask the user, in Epoch, whether a tool may run. Called by the agent for its own tools; not something to call directly.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": { "type": "string" },
                    "input": { "type": "object" },
                    "tool_use_id": { "type": "string" }
                },
                "required": ["tool_name", "input"]
            }
        }));
        json!({ "tools": tools })
    }

    /// A tool call: parse it, explain it, judge it.
    fn call(&self, id: Value, params: Option<&Value>) -> Serve {
        let Some(params) = params else {
            return Serve::Reply(failed(id, INVALID_PARAMS, "a call needs params"));
        };
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Serve::Reply(failed(id, INVALID_PARAMS, "a call needs a tool name"));
        };

        // An unowned action. Refused rather than defaulted: every other answer here would be
        // Epoch deciding on somebody's behalf which character just wrote to their disk.
        let Some(character) = self.character else {
            return Serve::Reply(refusal(
                id,
                "This connection is not acting for anybody yet, so nothing can be run. Open \
                 Epoch and give a character an agent brain.",
            ));
        };

        // Asking to bring somebody else in. Before the capability lookup, like `approve`,
        // because it is not a capability: nothing is run, nothing is written, and the only
        // effect is a question put to the user.
        if name == HAND_OVER {
            let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
            let Some(to) = arguments.get("to").and_then(Value::as_str) else {
                return Serve::Reply(failed(id, INVALID_PARAMS, "say who you want it handed to"));
            };
            let what = arguments
                .get("what")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_owned();
            if what.is_empty() {
                return Serve::Reply(failed(
                    id,
                    INVALID_PARAMS,
                    "say what you would be handing over — the user has to be able to read it \
                     before they agree to it",
                ));
            }
            return Serve::HandOver(Box::new(Handover {
                id,
                to: to.to_owned(),
                what,
            }));
        }

        // The agent asking about its own tool, before any capability lookup: `approve` is not
        // one of ours and never will be.
        if name == APPROVE {
            let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
            let Some(tool) = arguments.get("tool_name").and_then(Value::as_str) else {
                return Serve::Reply(failed(id, INVALID_PARAMS, "a decision needs a tool name"));
            };
            return Serve::Approve(Box::new(Approval {
                id,
                tool: tool.to_owned(),
                input: arguments.get("input").cloned().unwrap_or(json!({})),
                tool_use_id: arguments
                    .get("tool_use_id")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            }));
        }

        let Ok(capability) = CapabilityId::new(name) else {
            return Serve::Reply(refusal(id, format!("'{name}' is not a capability name")));
        };
        let Some(found) = self.registry.get(&capability) else {
            return Serve::Reply(refusal(
                id,
                format!("Epoch has no capability called '{name}'"),
            ));
        };

        let arguments = read_arguments(params.get("arguments"));

        // Explaining comes before running, and it is also where a malformed call dies — so the
        // user is never asked to approve something that could not have worked (ADR-0009).
        let explanation = match found.explain(&arguments) {
            Ok(explanation) => explanation,
            Err(err) => return Serve::Reply(refusal(id, err.to_string())),
        };

        let verdict = epoch_kernel::decide(
            &found.describe(),
            &explanation,
            &Situation {
                character,
                world: self.world,
                mode: self.mode,
            },
            self.policies,
        );

        // A refusal is answered here and now. It is a *tool* error rather than a protocol
        // error — `isError` — because the call was well formed and Epoch said no. An agent
        // that received a protocol error would reasonably conclude the server is broken and
        // stop asking; one told "you were refused" can say so and carry on.
        if let Verdict::Refused(why) = &verdict {
            return Serve::Reply(refusal(id, why.clone()));
        }

        Serve::Run(Box::new(Run {
            id,
            capability,
            arguments,
            explanation,
            verdict,
        }))
    }
}

/// What Epoch tells an agent about the ground it is standing on.
///
/// **Additive, not exclusive.** An earlier version said these were the *only* tools the agent
/// had, because Epoch had taken its own away. That was wrong in the most expensive direction:
/// a good agent's file tools are better than these — better diffs, smarter search, far more
/// exercised — and removing them made Epoch worse than using the agent alone.
///
/// So the honest sentence is the narrower one: keep your own, and here is what you could not
/// have had otherwise.
const INSTRUCTIONS: &str = "You are working inside Epoch, as one of a crew. Keep using your own tools for files, search and commands — they are yours and they are good. What Epoch adds is what you could not reach on your own: the other characters and what they know, the Quest you are working on and its history, this World's knowledge, and any tools its owner has connected here.

Every Epoch tool is judged by Epoch's own permission model and may be shown to the user before it happens, so a call can pause. That is not a failure — wait for the result. If one is refused, say so plainly and do not work around it.";

/// Render a capability into MCP's tool shape.
///
/// One translation per dialect, in its own file (ADR-0026). This is the same job `ollama_tool`
/// does for Ollama, and neither knows the other exists — which is what keeps the Kernel's
/// description of a capability free of any API's vocabulary.
fn declare(descriptor: &Descriptor) -> Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for parameter in &descriptor.parameters {
        properties.insert(
            parameter.name.clone(),
            json!({
                "type": match parameter.kind {
                    epoch_kernel::ValueKind::Text => "string",
                    epoch_kernel::ValueKind::Integer => "integer",
                    epoch_kernel::ValueKind::Number => "number",
                    epoch_kernel::ValueKind::Boolean => "boolean",
                },
                "description": parameter.description,
            }),
        );
        if parameter.required {
            required.push(parameter.name.clone());
        }
    }

    let mut tool = json!({
        "name": descriptor.id.as_str(),
        "description": descriptor.summary,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
        },
    });

    // True, and decoration. Emitted because a well-behaved client can use it to decide what to
    // attempt; relied on nowhere, because `decide()` runs on every call regardless of what the
    // client believed. Only ever set when it is *true* — never `false`, which would read as a
    // claim that something is destructive rather than as an absence of the claim.
    if descriptor
        .effects
        .iter()
        .all(|e| *e == epoch_kernel::Effect::Reads)
    {
        tool["annotations"] = json!({ "readOnlyHint": true });
    }

    tool
}

/// Read the arguments a client sent.
///
/// JSON's types are carried across to the Kernel's rather than flattened to text. Flattening
/// would make every argument arrive as a string, and `Arguments::check` — which is the only
/// validation that matters here — would then pass a `"true"` where a boolean was declared. A
/// client that guessed wrong should be *told*, and it cannot be told if the wrongness was
/// erased on the way in.
///
/// Anything JSON has that the Kernel does not — arrays, objects, null — becomes its text form.
/// The Kernel deliberately has four kinds (Earn Complexity); a capability that declared one of
/// those four will refuse this, by name, which is the correct outcome.
fn read_arguments(raw: Option<&Value>) -> Arguments {
    let Some(map) = raw.and_then(Value::as_object) else {
        return Arguments::new();
    };
    map.iter()
        .fold(Arguments::new(), |arguments, (name, value)| {
            let carried = match value {
                Value::Bool(v) => epoch_kernel::Value::Boolean(*v),
                Value::Number(n) if n.is_i64() => {
                    epoch_kernel::Value::Integer(n.as_i64().unwrap_or_default())
                }
                Value::Number(n) => epoch_kernel::Value::Number(n.as_f64().unwrap_or_default()),
                Value::String(s) => epoch_kernel::Value::Text(s.clone()),
                other => epoch_kernel::Value::Text(other.to_string()),
            };
            arguments.with(name, carried)
        })
}

/// Compose the reply for a call that ran.
///
/// The shell calls this once the capability has produced something, so the protocol shape lives
/// here with the rest of the protocol rather than in the transport.
pub fn produced(id: Value, content: &str) -> Value {
    result(
        id,
        json!({ "content": [{ "type": "text", "text": content }], "isError": false }),
    )
}

/// Compose the reply for a call Epoch refused, or that failed.
pub fn refusal(id: Value, why: impl Into<String>) -> Value {
    result(
        id,
        json!({ "content": [{ "type": "text", "text": why.into() }], "isError": true }),
    )
}

fn result(id: Value, value: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": value })
}

fn failed(id: Value, code: i32, message: impl Into<String>) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message.into() } })
}

const METHOD_NOT_FOUND: i32 = -32601;
const INVALID_PARAMS: i32 = -32602;

#[cfg(test)]
mod tests {
    use super::*;

    use epoch_kernel::{Autonomy, Decision, Scope};

    /// A registry over a real folder, because that is the only kind there is.
    ///
    /// A World with no Project Root gets an *empty* registry deliberately — so a serving test
    /// against one would assert nothing about serving.
    fn registry() -> CapabilityRegistry {
        let dir = std::env::temp_dir().join(format!("epoch-serve-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let root = crate::project::ProjectRoot::open(dir.to_str().unwrap()).unwrap();
        crate::capabilities::for_project(
            root,
            crate::capabilities::files::Undo::new(dir.join("vault"), "archipelago"),
            dir.join("vault"),
        )
    }

    fn mage() -> CharacterId {
        CharacterId::new("mage").unwrap()
    }

    fn serving<'a>(
        registry: &'a CapabilityRegistry,
        who: Option<&'a CharacterId>,
        mode: Autonomy,
        policies: &'a [Policy],
    ) -> Serving<'a> {
        Serving {
            registry,
            character: who,
            world: "archipelago",
            mode,
            policies,
        }
    }

    fn call(name: &str, arguments: Value) -> Value {
        json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": { "name": name, "arguments": arguments },
        })
    }

    #[test]
    fn a_notification_is_answered_with_silence() {
        // The protocol forbids replying to one, and a reply to a notification is what makes a
        // client hang waiting for an id that will never come.
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let served =
            serving.handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        assert_eq!(served, Serve::Silent);
    }

    #[test]
    fn epoch_declares_only_what_it_implements() {
        // Declaring `resources` or `prompts` because they exist in the specification would be a
        // promise a client would then call.
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let Serve::Reply(reply) = serving.handle(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": PROTOCOL },
        })) else {
            panic!("initialize must be answered");
        };

        let capabilities = &reply["result"]["capabilities"];
        assert!(capabilities.get("tools").is_some());
        assert!(capabilities.get("resources").is_none());
        assert!(capabilities.get("prompts").is_none());
        // Stated, never echoed: agreeing to a version we do not implement is how a mismatch
        // becomes a surprise three calls later.
        assert_eq!(reply["result"]["protocolVersion"], PROTOCOL);
    }

    #[test]
    fn every_capability_is_offered_and_none_is_hidden_by_permission() {
        // What a character may *use* is decided per call, with the arguments in hand. Hiding a
        // tool here would answer that early and permanently — an agent that never saw
        // `write_file` cannot ask for it, so the user is never given the chance to say yes.
        let registry = registry();
        let who = mage();
        // The strictest mode there is, and the catalogue is still whole.
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let Serve::Reply(reply) = serving.handle(&json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/list",
        })) else {
            panic!("tools/list must be answered");
        };

        let tools = reply["result"]["tools"].as_array().unwrap();
        // Every capability, plus the two tools that are not capabilities. `approve` is how the
        // agent asks *us* about its own tools, and the CLI resolves `--permission-prompt-tool`
        // against this list — a name missing here is a permission gate that silently never runs.
        // `hand_over` is how a character asks to bring a colleague in; it runs nothing and its
        // only effect is a question put to the user.
        assert_eq!(tools.len(), registry.len() + 2);
        assert!(tools.iter().any(|t| t["name"] == "write_file"));
        assert!(tools.iter().any(|t| t["name"] == APPROVE));
        assert!(tools.iter().any(|t| t["name"] == HAND_OVER));
    }

    /// Asking to bring a colleague in is a question, never an act.
    #[test]
    fn a_handover_is_proposed_and_never_performed() {
        let registry = registry();
        let mage = mage();
        // `Auto` deliberately: autonomy is about what a character may *do*, and this is not that.
        let serving = serving(&registry, Some(&mage), Autonomy::Auto, &[]);

        let asked = serving.handle(&call(
            HAND_OVER,
            json!({ "to": "Paladin", "what": "make an html page with a blue button" }),
        ));

        // **Not `Run`, and not `Reply`.** Nothing is executed and no answer is composed here:
        // the only thing that happens is a question reaching the user. Even under `Auto` —
        // autonomy is about what a character may *do*, and this is about bringing a second
        // character, a second model and a second set of hands onto the user's files into the
        // work (ADR-0025: the user's approval is what moves a Quest).
        let Serve::HandOver(proposed) = asked else {
            panic!("a handover must reach the user, not the disk");
        };
        assert_eq!(proposed.to, "Paladin");
        // Their exact words, unedited. Approving a handover you cannot read is approving a
        // rumour.
        assert_eq!(proposed.what, "make an html page with a blue button");
    }

    #[test]
    fn a_handover_with_nothing_in_it_is_refused() {
        // The user has to be able to read what they are agreeing to. "Pass it to Paladin" with
        // no content is a question nobody can answer honestly.
        let registry = registry();
        let mage = mage();
        let serving = serving(&registry, Some(&mage), Autonomy::Manual, &[]);

        let Serve::Reply(reply) =
            serving.handle(&call(HAND_OVER, json!({ "to": "paladin", "what": "   " })))
        else {
            panic!("an empty handover is answered, not queued");
        };
        assert!(reply["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("read it"));

        // And one addressed to nobody.
        let Serve::Reply(reply) = serving.handle(&call(HAND_OVER, json!({ "what": "do it" })))
        else {
            panic!("answered");
        };
        assert!(reply["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("who"));
    }

    /// The agent asking whether its own tool may run.
    ///
    /// Captured from the real CLI (2.1.224) rather than imagined: with a throwaway MCP server
    /// named on `--permission-prompt-tool`, this exact call arrived before a `Write`.
    #[test]
    fn a_permission_question_is_carried_out_rather_than_run() {
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let Serve::Approve(asked) = serving.handle(&call(
            APPROVE,
            json!({
                "tool_name": "Write",
                "input": { "file_path": "notes.md", "content": "hi" },
                "tool_use_id": "toolu_01",
            }),
        )) else {
            panic!("a permission question is not a capability call");
        };

        // Their word for the tool, and their arguments untouched — the user approves what the
        // agent actually intends, not a translation of it.
        assert_eq!(asked.tool, "Write");
        assert_eq!(asked.input["file_path"], "notes.md");
        assert_eq!(asked.tool_use_id.as_deref(), Some("toolu_01"));
    }

    /// A question with nothing in it is refused rather than asked about.
    #[test]
    fn a_permission_question_with_no_tool_is_a_bad_call() {
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        // Never a prompt saying "may an unnamed thing run?" — nobody could answer that.
        let Serve::Reply(reply) = serving.handle(&call(APPROVE, json!({ "input": {} }))) else {
            panic!("a malformed call is answered, not carried out");
        };
        assert!(reply["error"].is_object());
    }

    #[test]
    fn a_read_only_capability_says_so_and_a_writing_one_stays_silent() {
        // Only ever set when true. `readOnlyHint: false` would read as a claim that something
        // is destructive rather than as the absence of a claim.
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let Serve::Reply(reply) = serving.handle(&json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/list",
        })) else {
            panic!("tools/list must be answered");
        };
        let tools = reply["result"]["tools"].as_array().unwrap();

        let reader = tools.iter().find(|t| t["name"] == "read_file").unwrap();
        assert_eq!(reader["annotations"]["readOnlyHint"], json!(true));

        let writer = tools.iter().find(|t| t["name"] == "write_file").unwrap();
        assert!(writer.get("annotations").is_none());
    }

    #[test]
    fn a_call_from_nobody_is_refused_rather_than_attributed() {
        // The whole of 5.2 rests on a call belonging to somebody. A default here would be Epoch
        // deciding on the user's behalf which character just wrote to their disk.
        let registry = registry();
        let serving = serving(&registry, None, Autonomy::Auto, &[]);

        let Serve::Reply(reply) =
            serving.handle(&call("read_file", json!({ "path": "README.md" })))
        else {
            panic!("must be answered rather than run");
        };
        assert_eq!(reply["result"]["isError"], json!(true));
    }

    #[test]
    fn a_capability_epoch_does_not_have_is_refused_by_name() {
        // Everything here arrives from outside Epoch, so nothing is assumed.
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Auto, &[]);

        let Serve::Reply(reply) = serving.handle(&call("rm_rf", json!({}))) else {
            panic!("must be answered");
        };
        assert_eq!(reply["result"]["isError"], json!(true));
        assert!(reply["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("rm_rf"));
    }

    #[test]
    fn a_malformed_call_dies_before_the_user_is_ever_asked() {
        // Explaining comes before running, so nobody is asked to approve a call that could not
        // have worked (ADR-0009).
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        // `read_file` needs a path.
        let Serve::Reply(reply) = serving.handle(&call("read_file", json!({}))) else {
            panic!("a call with no arguments must not reach the user");
        };
        assert_eq!(reply["result"]["isError"], json!(true));
    }

    #[test]
    fn writing_in_manual_mode_reaches_the_user_rather_than_the_disk() {
        // The reason 5.2 exists. An agent owning its own loop would have written this file and
        // told Epoch afterwards; here it becomes a question, and the answer is the user's.
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let Serve::Run(run) = serving.handle(&call(
            "write_file",
            json!({ "path": "hola.md", "content": "hola" }),
        )) else {
            panic!("a write in Manual must become a question, not a reply");
        };

        assert_eq!(run.capability.as_str(), "write_file");
        assert!(matches!(run.verdict, Verdict::Ask(_)), "{:?}", run.verdict);
        // And it carries the sentence the prompt will show, already composed.
        assert!(
            run.explanation.what.contains("hola.md"),
            "{}",
            run.explanation.what
        );
    }

    #[test]
    fn reading_never_asks_even_in_the_strictest_mode() {
        // Observation is the floor of usefulness (ADR-0009 rule 2). An agent that had to ask
        // before every read would be unusable, and there would be nothing to weigh.
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let Serve::Run(run) = serving.handle(&call("read_file", json!({ "path": "README.md" })))
        else {
            panic!("reading must never become a question");
        };
        assert!(
            matches!(run.verdict, Verdict::Allowed(_)),
            "{:?}",
            run.verdict
        );
    }

    #[test]
    fn a_standing_refusal_is_honoured_and_never_reaches_the_shell() {
        // Deny wins outright, whatever the mode — including for an agent, which is the caller
        // most likely to try again.
        let registry = registry();
        let who = mage();
        let policies = vec![Policy {
            capability: "run_command".into(),
            scope: Scope::Everywhere,
            decision: Decision::Deny,
        }];
        let serving = serving(&registry, Some(&who), Autonomy::Auto, &policies);

        let Serve::Reply(reply) = serving.handle(&call("run_command", json!({ "command": "ls" })))
        else {
            panic!("a denied capability must never become a Run");
        };
        assert_eq!(reply["result"]["isError"], json!(true));
    }

    #[test]
    fn a_refusal_is_a_tool_error_rather_than_a_protocol_error() {
        // An agent told the server is broken stops asking. One told "you were refused" can say
        // so and carry on — which is the behaviour the user actually wants to see.
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let Serve::Reply(reply) = serving.handle(&call("read_file", json!({}))) else {
            panic!("must be answered");
        };
        assert!(reply.get("error").is_none(), "not a JSON-RPC error");
        assert_eq!(reply["result"]["isError"], json!(true));
    }

    #[test]
    fn an_unknown_method_is_an_error_rather_than_a_panic() {
        let registry = registry();
        let who = mage();
        let serving = serving(&registry, Some(&who), Autonomy::Manual, &[]);

        let Serve::Reply(reply) = serving.handle(&json!({
            "jsonrpc": "2.0", "id": 9, "method": "resources/list",
        })) else {
            panic!("must be answered");
        };
        assert_eq!(reply["error"]["code"], json!(METHOD_NOT_FOUND));
    }
}
