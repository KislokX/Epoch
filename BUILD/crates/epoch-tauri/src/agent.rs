//! The door an agent knocks on, wired to the World behind it (step 5.2).
//!
//! Three pieces already exist and none of them knows about the others:
//!
//! - [`epoch_engine::serve`] turns a JSON-RPC message into a **judged** instruction. Pure.
//! - [`epoch_engine::endpoint`] listens, authenticates, and hands over bytes. Knows no domain.
//! - [`epoch_engine::asking`] stops a thread until a person answers. Knows no protocol.
//!
//! This is the only file that knows all three, and it is in the **shell** rather than the Engine
//! for the reason the shell exists at all: it is where a question becomes something a human can
//! see. The Engine can judge a call headless; it cannot put a prompt in front of anybody.
//!
//! ## Why the connection is opened *for* somebody
//!
//! `decide()` needs a character, and an HTTP request cannot tell us which one. Rather than
//! inventing an identity for agents — a second permission system wearing the first one's
//! clothes — **the user says who the agent is acting as when they open the door**. It is
//! deliberate, it is visible, and it cannot be wrong.
//!
//! ## The World's lock is never held while waiting
//!
//! Every read here takes the lock, copies what it needs, and lets go *before* anything can
//! block. A worker parked on an approval while holding the World's mutex would freeze the
//! window the user has to click in — the deadlock where the only way to answer the prompt is to
//! have already answered it.

use std::sync::Arc;

use tauri::Emitter;

use epoch_engine::asking::{Answer, Ended};
use epoch_engine::serve::{self, Serve, Serving};
use serde_json::Value;

use crate::state::World;

/// What the endpoint calls for every message that got past the guard.
pub struct Door {
    pub world: Arc<World>,
    /// Whose hands these are. Chosen by the user when the door was opened.
    pub character: epoch_kernel::CharacterId,
    /// So a question can reach the window.
    ///
    /// Events rather than the surface polling: the question appears at a moment only this
    /// thread knows about, and a poll fast enough to feel immediate is a timer running all day
    /// for something that happens a few times an hour.
    pub app: tauri::AppHandle,
}

/// Yes, with the agent's own arguments handed back untouched.
///
/// `updatedInput` is where a permission layer may *change* what runs. Epoch does not: the user
/// approved what they were shown, and returning anything else would make the prompt a
/// description of something that did not happen.
fn allowed(id: Value, input: Value) -> Value {
    decision(
        id,
        serde_json::json!({ "behavior": "allow", "updatedInput": input }),
    )
}

/// No, with a reason the agent can repeat to the user.
fn denied(id: Value, why: &str) -> Value {
    decision(
        id,
        serde_json::json!({ "behavior": "deny", "message": why }),
    )
}

/// A decision, in the shape the agent reads.
///
/// JSON inside a text block — verified against the real CLI rather than remembered: the same
/// shape allowed a write in one probe and blocked it in another, and `isError` is deliberately
/// absent because *the decision succeeded*. A refusal is not a broken tool.
fn decision(id: Value, verdict: Value) -> Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{ "type": "text", "text": verdict.to_string() }],
        }
    })
}

/// A question opened. Carries it, so the surface draws without asking for it.
pub const ASKING: &str = "agent:asking";
/// The question closed — answered, refused, or timed out. No payload: it is gone.
pub const SETTLED: &str = "agent:settled";

impl epoch_engine::endpoint::Answering for Door {
    fn answer(&self, message: &Value) -> Option<Value> {
        // Everything the decision depends on, copied out under the lock and then let go of.
        // Held across the approval, this would be the deadlock described above.
        let Some(open) = self.world.serving_state() else {
            return Some(serve::refusal(
                message.get("id").cloned().unwrap_or(Value::Null),
                "No World is open in Epoch, so there is nothing to work in.",
            ));
        };

        let serving = Serving {
            registry: &open.registry,
            character: Some(&self.character),
            world: &open.world,
            mode: open.mode,
            policies: &open.policies,
        };

        match serving.handle(message) {
            Serve::Silent => None,
            Serve::Reply(reply) => Some(reply),
            Serve::Run(run) => Some(self.carry_out(&open.registry, *run)),
            Serve::Approve(asked) => Some(self.decide_for_them(*asked, open.mode)),
            Serve::HandOver(asked) => Some(self.propose_handover(*asked)),
        }
    }
}

impl Door {
    /// Answer the agent about **its own** tool.
    ///
    /// The other half of Manual, and the one that was missing. Epoch does not run this tool,
    /// does not see its result and does not take it away — it is invited to the decision by the
    /// agent itself (`--permission-prompt-tool`), which is the only arrangement that leaves the
    /// agent's good tools intact while still putting a person in front of the change.
    ///
    /// **The mode is honoured here, not re-derived.** `Auto` means the user already said yes to
    /// this World, so asking again would be Epoch ignoring its own setting; anything stricter
    /// asks. Epoch's *own* capabilities keep going through `decide()` — this decides only about
    /// somebody else's, where Epoch has no descriptor and no risk rating, and where the honest
    /// input to the decision is what the agent said it would do.
    fn decide_for_them(&self, asked: serve::Approval, mode: epoch_kernel::Autonomy) -> Value {
        let id = asked.id.clone();

        if matches!(mode, epoch_kernel::Autonomy::Auto) {
            return allowed(id, asked.input);
        }

        let preview = serde_json::to_string_pretty(&asked.input).ok();
        let question = epoch_engine::asking::Question {
            character: self.character.to_string(),
            // Their word for it, not ours. Showing `write_file` for the agent's `Write` would be
            // Epoch renaming the thing somebody is being asked to approve.
            capability: asked.tool.clone(),
            what: format!("{} wants to use {}", self.character, asked.tool),
            preview,
            // A standing decision is meaningful here: this is the same tool, again and again.
            standing: true,
        };

        let app = self.app.clone();
        let notice = question.clone();
        let ended = self.world.ask_about_with(question, move || {
            let _ = app.emit(ASKING, notice);
        });
        let _ = self.app.emit(SETTLED, ());

        match ended {
            Ended::Answered(answer) if answer.approved() => allowed(id, asked.input),
            // Each ending says something different, because an agent that can tell "no" from
            // "nobody was there" behaves differently, and only one of them is worth retrying.
            Ended::Answered(_) => denied(id, "The user said no in Epoch. Do not try this again."),
            Ended::NobodyAnswered => denied(
                id,
                "Nobody answered in Epoch, so this was not allowed. Say so and stop.",
            ),
            Ended::AlreadyAsking => denied(
                id,
                "Epoch is already asking about something else. Try this again shortly.",
            ),
            Ended::Withdrawn => denied(id, "Epoch closed the question. This was not allowed."),
        }
    }

    /// Put a proposed handover to the user, and queue it if they agree.
    ///
    /// **The character asked; the user decides; the Runtime moves the Quest.** ADR-0025 is
    /// explicit that an NPC does not decide where a Quest goes, and that the user's explicit
    /// approval is the strongest of the three things that may move one — because it makes the
    /// cause *visible*, which is what separates exposing collaboration from simulating it.
    ///
    /// Two deliberate differences from [`decide_for_them`](Self::decide_for_them):
    ///
    /// - **`Auto` does not skip this.** Autonomy is about what a character may *do*; this is
    ///   about bringing a second character — a second model, a second bill, a second set of
    ///   hands on the user's files — into the work. That is not a tool call, and it is not
    ///   covered by having said yes to this World once.
    /// - **Never standing.** Every handover is its own decision. A permanent "always let Mage
    ///   hand work to Robo" is exactly the quiet escalation the per-turn permission was written
    ///   to prevent.
    ///
    /// What the user reads is the character's own words, verbatim. Approving a handover you
    /// cannot read is approving a rumour.
    fn propose_handover(&self, asked: serve::Handover) -> Value {
        let id = asked.id.clone();

        // Somebody who actually lives here. A character naming a colleague who does not is a
        // thing that will happen, and the honest answer is to say so rather than to guess who
        // was meant.
        let Some(to) = self.world.crew_member(&asked.to) else {
            return denied(
                id,
                "Nobody by that name is in this World. Look at the crew you were told about, \
                 and use one of those names.",
            );
        };
        // Who is asking. Already an identity — the door knows whose hands it is holding.
        let from = self.character.clone();
        if from == to {
            return denied(
                id,
                "That is you. Handing work to yourself is not a handover.",
            );
        }

        let question = epoch_engine::asking::Question {
            character: self.character.to_string(),
            capability: "hand over".into(),
            what: format!("{} wants to hand this to {}", self.character, asked.to),
            // Exactly what would be sent, in their words, unedited.
            preview: Some(asked.what.clone()),
            standing: false,
        };

        let app = self.app.clone();
        let notice = question.clone();
        let ended = self.world.ask_about_with(question, move || {
            let _ = app.emit(ASKING, notice);
        });
        let _ = self.app.emit(SETTLED, ());

        match ended {
            Ended::Answered(answer) if answer.approved() => {
                self.world.approve_handover(&from, &to, &asked.what);
                allowed(
                    id,
                    serde_json::json!({
                        "handed_over": true,
                        // Said plainly, because the next thing this agent does is decide whether
                        // to keep working. It should not: the work is somebody else's now.
                        "note": format!(
                            "The user agreed. Your message has been recorded and {} will pick \
                             it up next. Say what you passed on, and stop.",
                            asked.to
                        ),
                    }),
                )
            }
            Ended::Answered(_) => denied(
                id,
                "The user said no. The work stays with you — do not hand it on, and do not \
                 claim anybody else has it.",
            ),
            Ended::NobodyAnswered => denied(
                id,
                "Nobody answered in Epoch, so nothing was handed over. Say so and stop.",
            ),
            Ended::AlreadyAsking => denied(
                id,
                "Epoch is already asking about something else. Try this again shortly.",
            ),
            Ended::Withdrawn => denied(id, "Epoch closed the question. Nothing was handed over."),
        }
    }

    /// Ask if the verdict says to, then do it.
    fn carry_out(&self, registry: &epoch_engine::CapabilityRegistry, run: serve::Run) -> Value {
        let id = run.id.clone();

        if let epoch_kernel::Verdict::Ask(_) = &run.verdict {
            // Told before the wait, and told again however the wait ends — including the ends
            // nobody clicked. A prompt left on screen after its own deadline passed is a button
            // that does nothing, which is worse than no button.
            let question = epoch_engine::asking::Question {
                character: self.character.to_string(),
                capability: run.capability.to_string(),
                what: run.explanation.what.clone(),
                preview: run.explanation.preview.clone(),
                standing: true,
            };
            let app = self.app.clone();
            let notice = question.clone();
            let ended = self.world.ask_about_with(question, move || {
                let _ = app.emit(ASKING, notice);
            });
            let _ = self.app.emit(SETTLED, ());

            // Here is the whole of 5.2 in one call: the agent stopped, on a socket, until a
            // person decided. An agent owning its own loop would have written the file and told
            // Epoch afterwards.
            match ended {
                Ended::Answered(answer) if answer.approved() => {}
                Ended::Answered(_) => {
                    return serve::refusal(id, "The user said no. Do not try this again.");
                }
                // Each of these is refused, and each says something different — an agent that
                // can tell "no" from "nobody was there" can behave differently, and only one of
                // them is worth retrying.
                Ended::NobodyAnswered => {
                    return serve::refusal(
                        id,
                        "Nobody answered in Epoch, so this was not done. Say so and stop.",
                    );
                }
                Ended::AlreadyAsking => {
                    return serve::refusal(
                        id,
                        "Epoch is already asking the user about something else. Wait a moment \
                         and try this again.",
                    );
                }
                Ended::Withdrawn => {
                    return serve::refusal(id, "Epoch closed the question. This was not done.");
                }
            }
        }

        // Approved, or never needed asking. Looked up again rather than carried through the
        // wait: the registry is rebuilt when the project root changes, and running against a
        // capability from before that would be running in the old folder.
        let Some(capability) = registry.get(&run.capability) else {
            return serve::refusal(id, format!("'{}' is no longer available", run.capability));
        };

        match capability.run(&run.arguments) {
            Ok(outcome) => {
                self.world.record_agent_run(&self.character, &run, &outcome);
                serve::produced(id, &outcome.content)
            }
            // A capability that failed is a tool error, not a protocol one: the call was well
            // formed and the world said no. The agent can read it and try something else.
            Err(err) => serve::refusal(id, err.to_string()),
        }
    }
}

/// Everything a served call depends on, taken out of the World in one go.
///
/// A snapshot rather than borrows, because the borrows would keep the lock — see the module
/// note. Taken fresh for **every** message: Trust is read at the moment of the call, which is
/// the only moment its answer is true.
pub struct Open {
    pub world: String,
    pub registry: epoch_engine::CapabilityRegistry,
    pub mode: epoch_kernel::Autonomy,
    pub policies: Vec<epoch_kernel::Policy>,
}

/// What a surface shows about the door.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bridge {
    /// `null` when Epoch is not listening.
    pub url: Option<String>,
    /// The shared secret, so it can be pasted into an agent's configuration.
    ///
    /// Shown rather than hidden: a secret nobody can read is a secret nobody can use, and this
    /// one exists precisely to be copied into a file the user owns. It never leaves this
    /// machine, and it is not the World's — it is the door's.
    pub token: Option<String>,
    /// Who the agent is acting as, when the door is open.
    pub character: Option<String>,
    /// What is being asked right now, if anything.
    pub question: Option<epoch_engine::asking::Question>,
}

/// The configuration to paste into an agent, ready to copy.
///
/// Written out rather than described, because the alternative is a support conversation about
/// which quotes go where. Agents differ in the key names; this is the shape Claude Code and
/// Codex both read.
pub fn configuration(url: &str, token: &str) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "mcpServers": {
            "epoch": {
                "type": "http",
                "url": url,
                "headers": { "Authorization": format!("Bearer {token}") },
            }
        }
    }))
    .unwrap_or_default()
}

/// Turn a surface's two booleans into what the Trust store understands.
pub fn answer_of(approve: bool, always: bool) -> Answer {
    match (approve, always) {
        (true, true) => Answer::AllowAlways,
        (true, false) => Answer::Allow,
        (false, true) => Answer::RefuseAlways,
        (false, false) => Answer::Refuse,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_configuration_is_something_a_person_can_paste() {
        let written = configuration("http://127.0.0.1:8792/mcp", "abc123");
        assert!(written.contains("\"epoch\""));
        assert!(written.contains("http://127.0.0.1:8792/mcp"));
        assert!(written.contains("Bearer abc123"));
    }

    #[test]
    fn always_is_the_only_thing_that_becomes_standing() {
        // ADR-0009: "allow once" must not quietly become permanent.
        assert_eq!(answer_of(true, false), Answer::Allow);
        assert!(!answer_of(true, false).standing());
        assert_eq!(answer_of(true, true), Answer::AllowAlways);
        assert!(answer_of(true, true).standing());
        assert_eq!(answer_of(false, true), Answer::RefuseAlways);
        assert!(!answer_of(false, false).approved());
    }
}
