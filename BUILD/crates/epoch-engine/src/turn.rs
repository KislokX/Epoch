//! One turn, including the work (ADR-0008).
//!
//! A turn is not one request. A character asked to find something will look, read what it
//! found, look again, and only then answer — so a turn is a **loop**: ask the model, run what
//! it asked for, give it the results, ask again.
//!
//! ```text
//! think → asked for tools? → judge each → run the allowed ones → hand back → think
//!                     └── no ──→ done
//! ```
//!
//! ## Nothing runs that was not judged
//!
//! Every call goes through the Trust Engine, individually, with the arguments the model
//! actually produced. There is no path from a `tool_calls` array to a filesystem in this file
//! that does not pass through [`TrustEngine::judge`].
//!
//! ## The model is told when it is refused
//!
//! A refusal comes back as a tool result saying why, not as silence and not as a failed turn.
//! A model that is ignored assumes the tool is broken and tries again; a model that is told
//! "this World is in Plan mode" stops asking and says so to the user.
//!
//! ## Running out of rounds is an outcome, not an error
//!
//! Whatever was found is kept and the user is told the work is unfinished. Throwing away eight
//! rounds of real work because the ninth was needed is the worst possible answer.

/// The same turn, for a brain that **works** instead of answering (ADR-0027).
///
/// Its own module rather than a branch in `drive`: a model is given a composed conversation and
/// hands back tool calls for Epoch to run, while an agent is given an intention and comes back
/// finished. One function with a branch inside would mean every caller asking which half it got —
/// the shape `Brain` exists to prevent.
pub mod agent;

use epoch_kernel::{Explanation, Message, Verdict};

use crate::capability::{Capability, CapabilityError, CapabilityRegistry, Source};
use crate::provider::{Chunk, Provider, ProviderError, Request, ToolCall};
use crate::trust::TrustEngine;

/// How many times round the loop before stopping and saying so.
///
/// Enough for look → read → read → answer, which is the shape of nearly every real request.
/// Bounded because a model that has misunderstood will loop happily forever, and each round
/// costs a full model call.
pub const MAX_ROUNDS: usize = 8;

/// Why the loop stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    /// The model had nothing more to ask for. The ordinary ending.
    Finished,
    /// The round limit was reached. Everything found so far is kept.
    RoundsExhausted,
    /// Something needs the user's word before it can run (ADR-0009).
    ///
    /// The turn stops **before** the call, with nothing attempted. [`resume`] continues from
    /// exactly here once the user has answered.
    NeedsApproval(Box<Paused>),
    /// The user said stop.
    ///
    /// An **outcome, not an error** — the same rule the round limit follows. Whatever was found
    /// is kept and the Chronicle records what actually happened, because a turn somebody
    /// interrupted still did the work it did before the interruption.
    Stopped,
    /// The character reached for something this build can do and they were never given.
    ///
    /// Distinct from "no such capability" on purpose. A model told a tool does not exist stops
    /// asking; a model that could have it, if somebody said yes, has produced a **question** —
    /// and answering it is a decision only the user can make.
    ///
    /// Granting is not approving. Once given, the call still goes through the Trust Engine like
    /// any other; otherwise "may I have write_file?" becomes a way around the gate.
    NeedsCapability(Box<Wanted>),
}

/// A capability a character reached for and does not have.
#[derive(Debug, Clone, PartialEq)]
pub struct Wanted {
    /// What it is and what it would cost, so the user chooses with the facts in front of them.
    pub descriptor: epoch_kernel::Descriptor,
    /// What they were trying to do with it. Judged, then run, if it is granted.
    pub call: ToolCall,
    pub request: Request,
}

/// A turn stopped mid-thought, waiting on the user.
///
/// Everything needed to carry on and nothing more: what would happen, what to run if they say
/// yes, and the conversation as it stood. Held rather than recomputed, because re-deriving it
/// from a later state could silently produce a *different* call than the one that was shown.
#[derive(Debug, Clone, PartialEq)]
pub struct Paused {
    /// What the user is being asked about, in the capability's own words.
    pub explanation: Explanation,
    /// The exact call. Run as-is on approval — never re-judged, never rebuilt.
    pub call: ToolCall,
    /// Where the conversation had got to. Resuming continues from here.
    pub request: Request,
}

/// What one turn produced.
#[derive(Debug, Clone, PartialEq)]
pub struct Completed {
    /// Everything the character said, in order, across every round.
    pub text: String,
    /// Durable things produced. What makes a Quest's History real rather than narrated
    /// (ADR-0025) — a Quest with none of these produced nothing, and History must say so.
    ///
    /// Each one names **the thing**, not only a sentence about it. A History that reads
    /// "created pepe.txt" cannot be followed; one that also holds `pepe.txt` can be.
    pub evidence: Vec<crate::capability::Made>,
    /// Where anything read this turn came from, in the order it was read.
    ///
    /// Separate from `evidence` on purpose: evidence is what the Quest *produced*, sources are
    /// what it *consulted*. A Quest that only searched still produced nothing (ADR-0025), and a
    /// History that counted reading as achievement would be counting intentions.
    pub sources: Vec<Source>,
    /// How many times round the loop it went. One means it answered without using anything.
    pub rounds: usize,
    /// How fast the last round decoded, **as the backend measured it**.
    ///
    /// The last round rather than a mean over the turn: rounds are separate decodes
    /// with model loads and tool runs between them, and an average across them
    /// describes no moment that happened.
    ///
    /// `None` where the backend does not report it — an agent, a hosted model, a
    /// machine across the bridge. Nothing is timed here with a stopwatch: the wall
    /// clock of a turn is mostly the prompt and the model load, and calling that a
    /// generation rate is a real reading of the wrong quantity.
    pub pace: Option<crate::provider::Pace>,
    pub stop: Stop,
}

/// What the World can watch happening, as it happens.
///
/// Separate from the token stream: tokens are what a character is *saying*, these are what a
/// character is *doing*, and the World renders work differently from talk (ADR-0018).
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// About to run something, already judged and allowed.
    Using(Explanation),
    /// A line it printed while still running.
    ///
    /// Only capabilities that stream emit these — a build, a test run, a large clone. They are
    /// observations, never evidence: what a run *left behind* is still `Outcome::evidence`, and
    /// a surface that mistook these for results would be reporting a program's chatter as fact.
    Said { capability: String, line: String },
    /// It finished. `detail` is the first line of what came back, or the reason it failed.
    Used {
        capability: String,
        ok: bool,
        detail: String,
    },
    /// It **started** something that will finish later (ADR-0034).
    ///
    /// Reported rather than recorded here, because a Job belongs to a Quest and a character and
    /// the turn loop knows neither — capabilities do the work and the layer above owns the
    /// cross-cutting concerns (ADR-0008). The shell listening to these is the half that knows
    /// whose work this is, and it takes both **now**, not when the work lands.
    Began {
        capability: String,
        /// The name the capability minted, and the one its result will land under.
        id: String,
        what: String,
        waiting_on: String,
    },
}

/// Run one turn to completion.
///
/// `on_chunk` receives tokens as they arrive; `on_step` receives the work. Both are called from
/// this thread, so a caller that wants them elsewhere sends them somewhere.
pub fn run(
    provider: &dyn Provider,
    trust: &TrustEngine<'_>,
    registry: &CapabilityRegistry,
    request: Request,
    stopped: &dyn Fn() -> bool,
    on_chunk: &mut dyn FnMut(Chunk),
    on_step: &mut dyn FnMut(Step),
) -> Result<Completed, ProviderError> {
    drive(
        provider, trust, registry, request, None, stopped, on_chunk, on_step,
    )
}

/// What to do with the call a turn stopped on, once the user has answered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Answered<'a> {
    /// They approved it. Runs **without being judged again**: the judgement already happened
    /// and they answered it, and re-asking would either loop forever or silently accept a
    /// different answer than the one they gave.
    Approved(&'a ToolCall),
    /// They granted the capability. The call is now *possible*, which is not the same as
    /// *allowed* — it goes through the Trust Engine like any other and may stop again to ask.
    /// Two questions, because there are two decisions.
    Granted(&'a ToolCall),
}

/// Continue a turn the user has just answered.
///
/// The conversation carried in `request` is the one from where it stopped, so the model picks
/// up with everything it already knew plus the result of what it asked for.
///
/// A denial is not this function: it is a tool result saying the user said no, and the ordinary
/// loop from there. That way the model learns rather than being cut off.
/// Eight arguments, and each is a distinct thing this needs: who answers, who judges, what can be
/// run, the conversation, the answer that unblocked it, a way to stop, and two sinks. Bundling
/// them into a struct would move the same eight names one level down and add a type whose only
/// job is to carry them — the honest reading is that a turn depends on eight things.
#[allow(clippy::too_many_arguments)]
pub fn resume(
    provider: &dyn Provider,
    trust: &TrustEngine<'_>,
    registry: &CapabilityRegistry,
    request: Request,
    answered: Answered<'_>,
    stopped: &dyn Fn() -> bool,
    on_chunk: &mut dyn FnMut(Chunk),
    on_step: &mut dyn FnMut(Step),
) -> Result<Completed, ProviderError> {
    drive(
        provider,
        trust,
        registry,
        request,
        Some(answered),
        stopped,
        on_chunk,
        on_step,
    )
}

/// Run the turn, then give the machine's memory back.
///
/// **The release belongs here and not in a Provider**, because only this function knows when the
/// answer is finished. A Provider sees rounds, and a turn with a tool call is several of them:
/// releasing at each seam threw the model out while a capability ran and reloaded it to say one
/// sentence about the result. Measured through the window, one Flux picture: 213.6 s on
/// llama.cpp and 203.5 s on LM Studio against Ollama's 84.3 s, almost all of it load paid twice.
///
/// Every exit releases, including the pause for approval — a turn waiting on a person may wait
/// for an hour, and that is exactly when the card should be somebody else's.
#[allow(clippy::too_many_arguments)]
fn drive(
    provider: &dyn Provider,
    trust: &TrustEngine<'_>,
    registry: &CapabilityRegistry,
    request: Request,
    answered: Option<Answered<'_>>,
    // Asked between rounds. See the check inside the loop for why that is the only seam.
    stopped: &dyn Fn() -> bool,
    on_chunk: &mut dyn FnMut(Chunk),
    on_step: &mut dyn FnMut(Step),
) -> Result<Completed, ProviderError> {
    let hold = request.keep_loaded;
    let model = request.model.clone();
    let out = rounds(
        provider, trust, registry, request, answered, stopped, on_chunk, on_step,
    );
    if hold == crate::provider::KeepLoaded::Never {
        provider.release(&model);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn rounds(
    provider: &dyn Provider,
    trust: &TrustEngine<'_>,
    registry: &CapabilityRegistry,
    mut request: Request,
    answered: Option<Answered<'_>>,
    stopped: &dyn Fn() -> bool,
    on_chunk: &mut dyn FnMut(Chunk),
    on_step: &mut dyn FnMut(Step),
) -> Result<Completed, ProviderError> {
    // What the backend last said about its own decoding. `None` until one says something.
    let mut last_pace: Option<crate::provider::Pace> = None;
    let mut said: Vec<String> = Vec::new();
    let mut evidence: Vec<crate::capability::Made> = Vec::new();
    let mut sources: Vec<Source> = Vec::new();

    // The one call the user has already answered for. Run before asking the model anything,
    // because the model is mid-thought and its next words depend on the result.
    match answered {
        Some(Answered::Approved(call)) => {
            if let Some(capability) = registry.get(&call.capability) {
                let name = call.capability.to_string();
                if let Ok(explanation) = capability.explain(&call.arguments) {
                    on_step(Step::Using(explanation));
                }
                perform(
                    capability,
                    call,
                    &name,
                    &mut request,
                    &mut evidence,
                    &mut sources,
                    on_step,
                );
            }
        }
        // Newly granted, so it exists for this character now — and has never been judged. It
        // goes through the gate like anything else, and may well stop again to ask.
        Some(Answered::Granted(call)) => {
            if let Some(capability) = registry.get(&call.capability) {
                let name = call.capability.to_string();
                match trust.judge(capability, &call.arguments) {
                    Ok(Verdict::Allowed(_)) => {
                        if let Ok(explanation) = capability.explain(&call.arguments) {
                            on_step(Step::Using(explanation));
                        }
                        perform(
                            capability,
                            call,
                            &name,
                            &mut request,
                            &mut evidence,
                            &mut sources,
                            on_step,
                        );
                    }
                    Ok(Verdict::Ask(explanation)) => {
                        return Ok(Completed {
                            text: String::new(),
                            evidence,
                            sources,
                            rounds: 0,
                            pace: last_pace,
                            stop: Stop::NeedsApproval(Box::new(Paused {
                                explanation: *explanation,
                                call: call.clone(),
                                request,
                            })),
                        });
                    }
                    Ok(Verdict::Refused(why)) => {
                        request.conversation.say(Message::tool_result(&name, &why));
                        on_step(Step::Used {
                            capability: name,
                            ok: false,
                            detail: why,
                        });
                    }
                    Err(err) => {
                        let why = err.to_string();
                        request.conversation.say(Message::tool_result(&name, &why));
                        on_step(Step::Used {
                            capability: name,
                            ok: false,
                            detail: why,
                        });
                    }
                }
            }
        }
        None => {}
    }

    /*
        **The budget, and `0` means there is none.**

        `MAX_ROUNDS` stays the default because each round is a full model call and a model that
        has misunderstood loops happily forever. What changed is that the number belongs to the
        machine now: 8 was chosen for *look → read → read → answer* and that was written before
        a character could have thirty-two MCP tools attached to it.

        `usize::MAX` rather than a second loop shape: an unbounded run and a very long one are
        the same code, and the only difference anybody can observe is whether it ever ends on
        its own. STOP is checked before every model call and before every tool, so *no limit*
        is still one press from ending.
    */
    let most = budget(request.most_rounds);

    for round in 1..=most {
        // Asked *between* rounds rather than during one. A round is a whole model call and a
        // whole tool run, and cutting into either would mean abandoning something already in
        // flight — a half-written file, a request whose reply we would never read. Stopping at
        // the seam is the only place where "stop" and "leave nothing torn" are the same thing.
        //
        // This is what a runaway loop actually needs: a model repeating one call forever spends
        // its life *at* this seam, so the check fires almost immediately.
        if stopped() {
            return Ok(Completed {
                text: said.join("\n\n"),
                evidence,
                sources,
                rounds: round.saturating_sub(1),
                pace: last_pace,
                stop: Stop::Stopped,
            });
        }

        let mut answer = provider.take_turn(&request, on_chunk)?;
        // **The round that just decoded, not an average over the turn.** Kept rather
        // than overwritten with `None`: a round that ended in a tool call reports no
        // pace, and losing the last real measurement to it would blank the reading on
        // exactly the turns that did the most work.
        if answer.pace.is_some() {
            last_pace = answer.pace;
        }
        // A model that writes its own tool call into its answer said nothing: the call is
        // already held, structurally, and the sentence is a duplicate of it.
        // Order matters: recover a call the model only wrote down, *then* hush an echo of a
        // call it really made. The first cannot fire when the second would.
        answer.recover_written_call(&request.tools);
        answer.hush_echoed_calls();

        if !answer.text.trim().is_empty() {
            said.push(answer.text.clone());
            request.conversation.say(Message::assistant(&answer.text));
        }

        if !answer.wants_more() {
            return Ok(Completed {
                text: said.join("\n\n"),
                evidence,
                sources,
                rounds: round,
                pace: last_pace,
                stop: Stop::Finished,
            });
        }

        // **What the model asked for, put back where the model can see it.**
        //
        // `answer.text` is empty whenever a turn is nothing but tool calls, so the block above
        // recorded nothing at all and the next round showed the model a tool result with no
        // request in front of it. From the model's point of view it reached for something and
        // the reaching vanished. So it reached again — fourteen identical navigations of one
        // page in a single turn, every one of them a success, the turn ending with nothing to
        // show. Truncating oversized results was a reasonable guess at this and was not it.
        //
        // **And as structure, not only as a sentence.** It was a sentence alone, on the
        // reasoning that a Message is a role and a string and the wire shape is the Provider's
        // business. The premise is right and the conclusion was wrong: what the model reached
        // for is a fact about the exchange, not prose about it, and a model handed prose does
        // not connect it to the result underneath.
        //
        // Measured, same picture and same result text, changing only this: with the sentence,
        // `gemma4:12b` answered "a man with a beard and glasses" — fiction, fluently. With
        // native `tool_calls` it answered with what the image actually showed. See
        // `epoch_kernel::ToolCall`.
        //
        // The sentence stays as the content, because it is what a surface shows and what a
        // backend with no tool-call shape can still read.
        request.conversation.say(Message::reaching(
            asked_for(&answer.calls),
            answer.calls.clone(),
        ));

        for call in &answer.calls {
            let name = call.capability.to_string();

            // Asked before each call, not only between rounds. A model looping on one tool
            // spends its whole life inside this loop, and a stop that only lands at the top of
            // the next round waits out every remaining call first — which is exactly the delay
            // that makes a stop button feel broken.
            if stopped() {
                return Ok(Completed {
                    text: said.join("\n\n"),
                    evidence,
                    sources,
                    rounds: round,
                    pace: last_pace,
                    stop: Stop::Stopped,
                });
            }

            // Three cases, and telling them apart is the whole point of this block.
            //
            // Not in the registry at all: hallucinated, or remembered from another product.
            // Told plainly, because "that does not exist" is something a model can act on and
            // silence is not.
            let Some(capability) = registry.get(&call.capability) else {
                request.conversation.say(Message::tool_result(
                    &name,
                    format!("there is no capability called '{name}' here"),
                ));
                on_step(Step::Used {
                    capability: name,
                    ok: false,
                    detail: "no such capability".into(),
                });
                continue;
            };

            // In the registry, but not on this turn's list: it exists and this character was
            // never given it. That is a question for the user, not a dead end for the model —
            // and it stops here rather than answering it on their behalf.
            if !request.tools.iter().any(|t| t.id == call.capability) {
                return Ok(Completed {
                    text: said.join("\n\n"),
                    evidence,
                    sources,
                    rounds: round,
                    pace: last_pace,
                    stop: Stop::NeedsCapability(Box::new(Wanted {
                        descriptor: capability.describe(),
                        call: call.clone(),
                        request: request.clone(),
                    })),
                });
            }

            let verdict = match trust.judge(capability, &call.arguments) {
                Ok(verdict) => verdict,
                // Bad arguments never reach the user as an approval prompt — nobody should be
                // asked to approve a call that could not have worked.
                Err(err) => {
                    request
                        .conversation
                        .say(Message::tool_result(&name, err.to_string()));
                    on_step(Step::Used {
                        capability: name,
                        ok: false,
                        detail: err.to_string(),
                    });
                    continue;
                }
            };

            let explanation = match verdict {
                Verdict::Allowed(_) => capability
                    .explain(&call.arguments)
                    .expect("judging already built this explanation successfully"),
                Verdict::Ask(explanation) => {
                    // Stop with nothing attempted. What was said and found so far is kept:
                    // stopping is not the same as losing the work that led here.
                    return Ok(Completed {
                        text: said.join("\n\n"),
                        evidence,
                        sources,
                        rounds: round,
                        pace: last_pace,
                        stop: Stop::NeedsApproval(Box::new(Paused {
                            explanation: *explanation,
                            call: call.clone(),
                            request: request.clone(),
                        })),
                    });
                }
                Verdict::Refused(why) => {
                    request.conversation.say(Message::tool_result(&name, &why));
                    on_step(Step::Used {
                        capability: name,
                        ok: false,
                        detail: why,
                    });
                    continue;
                }
            };

            on_step(Step::Using(explanation));
            perform(
                capability,
                call,
                &name,
                &mut request,
                &mut evidence,
                &mut sources,
                on_step,
            );
        }
    }

    Ok(Completed {
        text: said.join("\n\n"),
        evidence,
        sources,
        rounds: MAX_ROUNDS,
        pace: last_pace,
        stop: Stop::RoundsExhausted,
    })
}

/// Run one already-allowed call, and put what happened in front of the model.
fn perform(
    capability: &dyn Capability,
    call: &ToolCall,
    name: &str,
    request: &mut Request,
    evidence: &mut Vec<crate::capability::Made>,
    sources: &mut Vec<Source>,
    on_step: &mut dyn FnMut(Step),
) {
    // Watched rather than simply run. The default implementation says nothing, so this costs
    // the other capabilities exactly one indirection and changes none of their behaviour.
    let watched = capability.run_watched(&call.arguments, &mut |line| {
        on_step(Step::Said {
            capability: name.to_owned(),
            line: line.to_owned(),
        });
    });
    match watched {
        Ok(outcome) => {
            if let Some(made) = &outcome.evidence {
                evidence.push(made.clone());
            }
            sources.extend(outcome.sources.iter().cloned());
            // **A start is not a result, and saying so is the whole of ADR-0034 here.** Nothing
            // was made, so nothing is evidence yet; what lands later is filed against the Quest
            // this began under, which is why the Job is registered by the layer that knows it.
            if let Some(begun) = &outcome.begun {
                on_step(Step::Began {
                    capability: name.to_owned(),
                    id: begun.id.clone(),
                    what: begun.what.clone(),
                    waiting_on: begun.waiting_on.clone(),
                });
            }
            on_step(Step::Used {
                capability: name.to_owned(),
                ok: true,
                detail: first_line(&outcome.content),
            });
            // Wrapped, always. There is no unwrapped constructor to reach for.
            request
                .conversation
                .say(Message::tool_result(name, outcome.content));
        }
        Err(err) => {
            let why = match &err {
                // A refusal is not a failure, and a model told the difference stops retrying
                // and explains itself instead.
                CapabilityError::Refused(reason) => reason.clone(),
                other => other.to_string(),
            };
            request.conversation.say(Message::tool_result(name, &why));
            on_step(Step::Used {
                capability: name.to_owned(),
                ok: false,
                detail: why,
            });
        }
    }
}

/// How many rounds this turn may take.
///
/// Its own function so it can be asserted on — the three states are the whole decision, and an
/// inline `match` inside a loop header is invisible to a test.
pub fn budget(most_rounds: Option<usize>) -> usize {
    match most_rounds {
        None => MAX_ROUNDS,
        // **No limit**, spelled as a very long one. An unbounded run and a very long run are the
        // same code, and the only difference anybody can observe is whether it ends on its own.
        Some(0) => usize::MAX,
        Some(said) => said,
    }
}

/// What a character just reached for, in its own voice.
///
/// One message for the whole round rather than one each, because the calls were produced in a
/// single breath and splitting them would suggest an order the model did not choose.
fn asked_for(calls: &[ToolCall]) -> String {
    let each = calls
        .iter()
        .map(|call| {
            let args = call
                .arguments
                .iter()
                .map(|(name, value)| format!("{name}: {value}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}({args})", call.capability)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[I called {each}]")
}

/// The first line of a result, for a surface that shows one line.
fn first_line(content: &str) -> String {
    content.lines().next().unwrap_or("").to_owned()
}

#[cfg(test)]
mod tests {
    /// The three states of a round budget.
    ///
    /// 8 stays the default because **each round is a full model call** and a model that has
    /// misunderstood loops happily forever. What changed is that the number belongs to the
    /// machine: its reasoning was *look → read → read → answer*, written before a character
    /// could have an MCP server's thirty-two tools attached — and the owner watched one spend
    /// the budget on searches, announce *"Te lo reproduzco ahora:"* and stop, correctly, with
    /// nothing left to play it with.
    #[test]
    fn a_round_budget_has_three_states_and_one_of_them_is_none() {
        assert_eq!(super::budget(None), super::MAX_ROUNDS, "the built-in bound");
        assert_eq!(super::budget(Some(24)), 24, "what the machine said");
        assert_eq!(
            super::budget(Some(0)),
            usize::MAX,
            "no limit, and STOP still ends it"
        );
    }

    /// Tests never interrupt themselves. Named rather than inlined so a call site reads as
    /// "nobody pressed stop" instead of as an anonymous `false`.
    fn never() -> bool {
        false
    }

    use super::*;

    use std::cell::RefCell;

    use epoch_kernel::{
        Arguments, Autonomy, CapabilityId, CharacterId, Conversation, Descriptor, Effect,
        Parameter, Policy, Reversal, Role, Scope, Value, ValueKind,
    };

    use crate::capability::{Capability, Outcome};
    use crate::provider::{Answer, KeepLoaded, ProviderStatus, ToolCall};
    use crate::trust::TrustStore;

    fn id(raw: &str) -> CapabilityId {
        CapabilityId::new(raw).unwrap()
    }

    /// A provider that replays a scripted sequence of answers.
    struct Scripted {
        answers: RefCell<Vec<Answer>>,
        /// Every conversation it was handed, so a test can assert what the model actually saw.
        seen: RefCell<Vec<Conversation>>,
    }

    impl Scripted {
        fn new(answers: Vec<Answer>) -> Self {
            Self {
                answers: RefCell::new(answers),
                seen: RefCell::new(Vec::new()),
            }
        }
    }

    // The fakes are single-threaded; the trait's bound is about real providers.
    unsafe impl Sync for Scripted {}

    impl Provider for Scripted {
        fn id(&self) -> &'static str {
            "scripted"
        }
        fn probe(&self) -> ProviderStatus {
            unreachable!("not probed in these tests")
        }
        fn take_turn(
            &self,
            request: &Request,
            sink: &mut dyn FnMut(Chunk),
        ) -> Result<Answer, ProviderError> {
            self.seen.borrow_mut().push(request.conversation.clone());
            let mut answers = self.answers.borrow_mut();
            let answer = if answers.is_empty() {
                Answer::default()
            } else {
                answers.remove(0)
            };
            sink(Chunk::Done);
            Ok(answer)
        }
    }

    struct Reader;
    impl Capability for Reader {
        fn describe(&self) -> Descriptor {
            Descriptor::observing(id("read_file"), "Read a file.").taking([Parameter::required(
                "path",
                ValueKind::Text,
                "Path.",
            )])
        }
        fn run(&self, a: &Arguments) -> Result<Outcome, CapabilityError> {
            let path = a.text("path").map_err(CapabilityError::BadArguments)?;
            if path.contains("..") {
                return Err(CapabilityError::Refused("outside the project".into()));
            }
            Ok(Outcome::told(format!("{path}\nthe contents")))
        }
    }

    struct Deleter;
    impl Capability for Deleter {
        fn describe(&self) -> Descriptor {
            Descriptor::acting(
                id("delete_file"),
                "Delete a file.",
                [Effect::Deletes],
                Reversal::Permanent,
            )
            .taking([Parameter::required("path", ValueKind::Text, "Path.")])
        }
        fn run(&self, _a: &Arguments) -> Result<Outcome, CapabilityError> {
            Ok(Outcome::made("gone", "a/file", "one file removed"))
        }
    }

    fn registry() -> CapabilityRegistry {
        let mut r = CapabilityRegistry::new();
        r.register(Box::new(Reader));
        r.register(Box::new(Deleter));
        r
    }

    /// A turn where the character has been given everything this build can do.
    ///
    /// `tools` is what was *offered*, and the loop now treats "in the registry but not offered"
    /// as a question for the user rather than a call — so a fixture that declared no tools
    /// would test that path and nothing else.
    fn request() -> Request {
        offering(registry().describe_all())
    }

    /// A turn where the character has been given exactly these.
    fn offering(tools: Vec<Descriptor>) -> Request {
        Request {
            model: "m".into(),
            conversation: Conversation::opening("You are precise."),
            keep_loaded: KeepLoaded::Never,
            parameters: Default::default(),
            tuning: Default::default(),
            tools,
            // The built-in bound: a test is not the place to decide how patient a machine is.
            most_rounds: None,
        }
    }

    fn call(name: &str, path: &str) -> ToolCall {
        ToolCall {
            capability: id(name),
            arguments: Arguments::new().with("path", Value::Text(path.into())),
        }
    }

    fn answer(text: &str, calls: Vec<ToolCall>) -> Answer {
        Answer {
            text: text.into(),
            calls,
            pace: None,
        }
    }

    fn go(
        provider: &dyn Provider,
        store: &TrustStore,
        registry: &CapabilityRegistry,
    ) -> (Completed, Vec<Step>) {
        let who = CharacterId::new("robo").unwrap();
        let trust = TrustEngine::new(store, &who, "archipelago", store.mode("archipelago"));
        let steps = RefCell::new(Vec::new());
        let completed = run(
            provider,
            &trust,
            registry,
            request(),
            &never,
            &mut |_| {},
            &mut |step| steps.borrow_mut().push(step),
        )
        .unwrap();
        (completed, steps.into_inner())
    }

    #[test]
    fn a_turn_with_no_tools_is_one_round() {
        let provider = Scripted::new(vec![answer("Hello.", vec![])]);
        let (done, steps) = go(&provider, &TrustStore::default(), &registry());
        assert_eq!(done.text, "Hello.");
        assert_eq!(done.rounds, 1);
        assert_eq!(done.stop, Stop::Finished);
        assert!(steps.is_empty(), "nothing was done, only said");
    }

    #[test]
    fn the_model_sees_that_it_already_made_the_call() {
        // The fourteen-navigations bug. A turn that is nothing but a tool call has no text, so
        // nothing was written to the conversation and round two showed the model a result with
        // no request in front of it. It could not tell it had already asked, so it asked again,
        // and again, until the round limit ended the turn with nothing to show.
        let provider = Scripted::new(vec![
            // No words at all — the shape that used to record nothing.
            answer("", vec![call("read_file", "src/main.rs")]),
            answer("Read it.", vec![]),
        ]);
        let (_done, _steps) = go(&provider, &TrustStore::default(), &registry());

        let second = &provider.seen.borrow()[1];
        let mine = second
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            mine.contains("read_file"),
            "the call it made is in its own conversation"
        );
        assert!(
            mine.contains("src/main.rs"),
            "with the arguments, or it cannot tell two apart"
        );

        // And it comes before the result, because a result answering nothing is the bug.
        let asked = second
            .messages
            .iter()
            .position(|m| m.role == Role::Assistant)
            .unwrap();
        let told = second
            .messages
            .iter()
            .position(|m| m.role == Role::Tool)
            .unwrap();
        assert!(asked < told, "the request precedes its answer");
    }

    /// A turn gives the card back **once**, when the answer is finished.
    ///
    /// ## The measurement that made this a test
    ///
    /// `KeepLoaded::Never` used to be honoured inside the Provider, per model call. A turn with a
    /// tool call is several model calls with a capability running between them, so the model was
    /// unloaded while ComfyUI drew and reloaded to say one sentence about the picture. Through
    /// the window, one Flux render: **213.6 s on llama.cpp, 203.5 s on LM Studio, 84.3 s on
    /// Ollama** — and the gap was almost entirely a 12B model read off disk twice.
    ///
    /// So the assertion is not "it releases"; it is *two rounds, one release*. That is the whole
    /// content of the fix, and a version that released per round would still pass a test that
    /// only checked the model was let go.
    #[test]
    fn a_turn_with_a_tool_call_lets_the_model_go_once_and_not_once_per_round() {
        struct Counting {
            inner: Scripted,
            released: RefCell<Vec<String>>,
        }
        unsafe impl Sync for Counting {}
        impl Provider for Counting {
            fn id(&self) -> &'static str {
                "counting"
            }
            fn probe(&self) -> ProviderStatus {
                unreachable!("not probed in these tests")
            }
            fn take_turn(
                &self,
                request: &Request,
                sink: &mut dyn FnMut(Chunk),
            ) -> Result<Answer, ProviderError> {
                // Nothing here releases. A Provider sees rounds and cannot know which is last.
                self.inner.take_turn(request, sink)
            }
            fn release(&self, model: &str) {
                self.released.borrow_mut().push(model.to_owned());
            }
        }

        let provider = Counting {
            inner: Scripted::new(vec![
                answer("Let me look.", vec![call("read_file", "src/main.rs")]),
                answer("It contains the contents.", vec![]),
            ]),
            released: RefCell::new(Vec::new()),
        };
        let (done, _) = go(&provider, &TrustStore::default(), &registry());

        assert_eq!(done.rounds, 2, "the tool call cost a second model call");
        assert_eq!(
            provider.released.borrow().len(),
            1,
            "released once per turn, not once per round"
        );
        assert_eq!(provider.released.borrow()[0], "m");
    }

    /// And a character the user asked to stay warm is not evicted at all.
    #[test]
    fn keeping_the_crew_warm_releases_nothing() {
        struct Counting {
            inner: Scripted,
            released: RefCell<usize>,
        }
        unsafe impl Sync for Counting {}
        impl Provider for Counting {
            fn id(&self) -> &'static str {
                "counting"
            }
            fn probe(&self) -> ProviderStatus {
                unreachable!("not probed in these tests")
            }
            fn take_turn(
                &self,
                request: &Request,
                sink: &mut dyn FnMut(Chunk),
            ) -> Result<Answer, ProviderError> {
                self.inner.take_turn(request, sink)
            }
            fn release(&self, _model: &str) {
                *self.released.borrow_mut() += 1;
            }
        }

        let provider = Counting {
            inner: Scripted::new(vec![answer("Here you go.", vec![])]),
            released: RefCell::new(0),
        };
        let who = CharacterId::new("robo").unwrap();
        let store = TrustStore::default();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));
        let mut warm = request();
        warm.keep_loaded = KeepLoaded::For(std::time::Duration::from_secs(300));

        run(
            &provider,
            &trust,
            &registry(),
            warm,
            &never,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();

        assert_eq!(*provider.released.borrow(), 0);
    }

    #[test]
    fn a_tool_result_reaches_the_model_wrapped_and_it_answers_from_it() {
        let provider = Scripted::new(vec![
            answer("Let me look.", vec![call("read_file", "src/main.rs")]),
            answer("It contains the contents.", vec![]),
        ]);
        let (done, steps) = go(&provider, &TrustStore::default(), &registry());

        assert_eq!(done.rounds, 2);
        assert_eq!(done.stop, Stop::Finished);
        assert_eq!(done.text, "Let me look.\n\nIt contains the contents.");

        // The second round saw the result, in the tool role, wrapped.
        let second = &provider.seen.borrow()[1];
        let result = second
            .messages
            .iter()
            .find(|m| m.role == Role::Tool)
            .unwrap();
        assert!(result
            .content
            .starts_with(epoch_kernel::conversation::UNTRUSTED));
        assert!(result.content.contains("the contents"));

        assert_eq!(
            steps,
            vec![
                Step::Using(
                    Reader
                        .explain(&Arguments::new().with("path", Value::Text("src/main.rs".into())))
                        .unwrap()
                ),
                Step::Used {
                    capability: "read_file".into(),
                    ok: true,
                    detail: "src/main.rs".into(),
                },
            ]
        );
    }

    #[test]
    fn a_capability_that_does_not_exist_is_told_to_the_model_rather_than_ignored() {
        // A model that is ignored assumes the tool is broken and tries again. One that is told
        // stops asking.
        let provider = Scripted::new(vec![
            answer(
                "",
                vec![ToolCall {
                    capability: id("web_search"),
                    arguments: Arguments::new(),
                }],
            ),
            answer("I cannot search the web.", vec![]),
        ]);
        let (done, steps) = go(&provider, &TrustStore::default(), &registry());
        assert_eq!(done.stop, Stop::Finished);
        assert!(
            matches!(&steps[0], Step::Used { ok: false, detail, .. } if detail.contains("no such"))
        );

        let second = &provider.seen.borrow()[1];
        let told = second
            .messages
            .iter()
            .find(|m| m.role == Role::Tool)
            .unwrap();
        assert!(told.content.contains("no capability called 'web_search'"));
    }

    #[test]
    fn bad_arguments_come_back_as_something_the_model_can_fix() {
        let provider = Scripted::new(vec![
            answer(
                "",
                vec![ToolCall {
                    capability: id("read_file"),
                    arguments: Arguments::new(),
                }],
            ),
            answer("I need a path.", vec![]),
        ]);
        let (done, _) = go(&provider, &TrustStore::default(), &registry());
        assert_eq!(done.stop, Stop::Finished);
        let second = &provider.seen.borrow()[1];
        let told = second
            .messages
            .iter()
            .find(|m| m.role == Role::Tool)
            .unwrap();
        assert!(told.content.contains("'path' is required"));
    }

    #[test]
    fn a_refusal_reaches_the_model_as_a_reason_and_the_turn_continues() {
        let provider = Scripted::new(vec![
            answer("", vec![call("read_file", "../../secrets")]),
            answer("That is outside the project.", vec![]),
        ]);
        let (done, steps) = go(&provider, &TrustStore::default(), &registry());
        assert_eq!(done.stop, Stop::Finished, "a refusal does not end the turn");
        assert!(
            matches!(&steps[1], Step::Used { ok: false, detail, .. } if detail.contains("outside"))
        );
    }

    #[test]
    fn nothing_permanent_runs_without_the_user() {
        // Builder mode asks before deleting, and the turn stops with nothing attempted.
        let provider = Scripted::new(vec![
            answer("I will remove it.", vec![call("delete_file", "old.md")]),
            answer("should never be reached", vec![]),
        ]);
        let (done, steps) = go(&provider, &TrustStore::default(), &registry());

        match &done.stop {
            Stop::NeedsApproval(paused) => {
                assert_eq!(paused.explanation.capability.as_str(), "delete_file");
                assert!(paused.explanation.reversal.is_permanent());
                // The exact call is held, so approving runs what was shown rather than
                // something rebuilt from a later state.
                assert_eq!(paused.call.capability.as_str(), "delete_file");
            }
            other => panic!("{other:?}"),
        }
        // What was said before it stopped is kept — stopping is not losing the work.
        assert_eq!(done.text, "I will remove it.");
        assert!(done.evidence.is_empty(), "nothing was attempted");
        assert!(steps.is_empty(), "and nothing was reported as done");
        assert_eq!(
            provider.seen.borrow().len(),
            1,
            "the model was not asked again"
        );
    }

    #[test]
    fn approving_runs_the_call_that_was_shown_and_the_model_carries_on() {
        let store = TrustStore::default();
        let registry = registry();
        let who = CharacterId::new("robo").unwrap();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));

        // Stop.
        let provider = Scripted::new(vec![answer(
            "I will remove it.",
            vec![call("delete_file", "old.md")],
        )]);
        let paused = match run(
            &provider,
            &trust,
            &registry,
            request(),
            &never,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap()
        .stop
        {
            Stop::NeedsApproval(paused) => paused,
            other => panic!("{other:?}"),
        };

        // Answer yes.
        let after = Scripted::new(vec![answer("Removed it.", vec![])]);
        let steps = RefCell::new(Vec::new());
        let done = resume(
            &after,
            &trust,
            &registry,
            paused.request.clone(),
            Answered::Approved(&paused.call),
            &never,
            &mut |_| {},
            &mut |s| steps.borrow_mut().push(s),
        )
        .unwrap();

        assert_eq!(done.stop, Stop::Finished);
        assert_eq!(done.text, "Removed it.");
        // It actually ran, and it left evidence.
        assert_eq!(
            done.evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["one file removed"]
        );

        // The model was asked again *with the result in front of it*, carrying everything it
        // had already said.
        let seen = &after.seen.borrow()[0];
        let result = seen.messages.iter().find(|m| m.role == Role::Tool).unwrap();
        assert!(result.content.contains("gone"));
        assert!(seen
            .messages
            .iter()
            .any(|m| m.content.contains("I will remove it.")));
    }

    #[test]
    fn approving_does_not_ask_again() {
        // Re-judging what the user just answered would either loop forever or quietly accept a
        // different answer than the one they gave. Nothing here calls `judge` for the approved
        // call — asserted by the mode that would otherwise refuse it outright.
        let mut store = TrustStore::default();
        store.set_mode("archipelago", Autonomy::Auto);
        let registry = registry();
        let who = CharacterId::new("robo").unwrap();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));

        let approved = ToolCall {
            capability: id("delete_file"),
            arguments: Arguments::new().with("path", Value::Text("old.md".into())),
        };
        let after = Scripted::new(vec![answer("Done.", vec![])]);
        let done = resume(
            &after,
            &trust,
            &registry,
            request(),
            Answered::Approved(&approved),
            &never,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(
            done.evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["one file removed"]
        );
    }

    #[test]
    fn reaching_for_something_they_were_never_given_is_a_question_not_a_dead_end() {
        // The distinction that makes this worth having: a model told a tool does not exist
        // stops asking. A model that could have it, if somebody said yes, has produced a
        // question — and the user is the only one who can answer it.
        let registry = registry();
        let store = TrustStore::default();
        let who = CharacterId::new("robo").unwrap();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));

        // Given only the reader, and it reaches for something the build does have.
        let only_reading = offering(vec![Reader.describe()]);
        let provider = Scripted::new(vec![answer("", vec![call("delete_file", "old.md")])]);
        let done = run(
            &provider,
            &trust,
            &registry,
            only_reading,
            &never,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();

        match &done.stop {
            Stop::NeedsCapability(wanted) => {
                assert_eq!(wanted.descriptor.id.as_str(), "delete_file");
                // The facts travel with the question, so the user chooses knowing the cost.
                assert!(wanted.descriptor.reversal.is_permanent());
                assert_eq!(wanted.call.capability.as_str(), "delete_file");
            }
            other => panic!("{other:?}"),
        }
        assert_eq!(
            provider.seen.borrow().len(),
            1,
            "the model was not asked again"
        );
    }

    #[test]
    fn something_this_build_cannot_do_at_all_is_still_a_dead_end() {
        // Only capabilities that exist become questions. A hallucinated one is told plainly,
        // because there is nothing a user could say yes to.
        let registry = registry();
        let provider = Scripted::new(vec![
            answer(
                "",
                vec![ToolCall {
                    capability: id("web_search"),
                    arguments: Arguments::new(),
                }],
            ),
            answer("I cannot search the web.", vec![]),
        ]);
        let (done, _) = go(&provider, &TrustStore::default(), &registry);
        assert_eq!(done.stop, Stop::Finished);
    }

    #[test]
    fn granting_a_capability_is_not_approving_the_call() {
        // Otherwise "may I have delete_file?" becomes a way around the gate. Two decisions,
        // two questions.
        let registry = registry();
        let store = TrustStore::default();
        let who = CharacterId::new("robo").unwrap();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));
        let granted = call("delete_file", "old.md");

        let after = Scripted::new(vec![answer("done", vec![])]);
        let done = resume(
            &after,
            &trust,
            &registry,
            request(),
            Answered::Granted(&granted),
            &never,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();

        // Now that they have it, Manual mode asks about it — it did not simply run.
        assert!(
            matches!(done.stop, Stop::NeedsApproval(_)),
            "{:?}",
            done.stop
        );
        assert!(done.evidence.is_empty());
        assert_eq!(
            after.seen.borrow().len(),
            0,
            "and the model was not asked anything yet"
        );
    }

    #[test]
    fn a_standing_never_refuses_instead_of_asking_and_the_model_learns_why() {
        // A `Deny` is not a prompt. The turn carries on and the model is told, so it explains
        // itself rather than trying again — and it wins even in Auto.
        let mut store = TrustStore::default();
        store.set_mode("archipelago", Autonomy::Auto);
        store.remember(Policy::deny("delete_file", Scope::Everywhere));

        let provider = Scripted::new(vec![
            answer("", vec![call("delete_file", "old.md")]),
            answer("I am not allowed to delete anything here.", vec![]),
        ]);
        let (done, _) = go(&provider, &store, &registry());
        assert_eq!(done.stop, Stop::Finished, "refused, not paused");
        assert!(done.evidence.is_empty(), "and nothing happened");

        let second = &provider.seen.borrow()[1];
        let told = second
            .messages
            .iter()
            .find(|m| m.role == Role::Tool)
            .unwrap();
        assert!(told.content.contains("never"), "{}", told.content);
    }

    #[test]
    fn stopping_keeps_the_work_and_says_it_was_stopped() {
        // The case this exists for: a model repeating one call forever. It spends its whole life
        // at the seam between rounds, so the check fires almost immediately.
        let looping: Vec<Answer> = (0..MAX_ROUNDS)
            .map(|n| {
                answer(
                    &format!("looking again ({n})"),
                    vec![call("read_file", "a.md")],
                )
            })
            .collect();
        let provider = Scripted::new(looping);

        let store = TrustStore::default();
        let registry = registry();
        let who = epoch_kernel::CharacterId::new("mage").unwrap();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));

        // Stop after the second round has begun.
        let rounds = std::cell::Cell::new(0);
        let done = run(
            &provider,
            &trust,
            &registry,
            request(),
            &|| {
                rounds.set(rounds.get() + 1);
                rounds.get() > 2
            },
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();

        assert_eq!(done.stop, Stop::Stopped);
        assert!(
            done.rounds < MAX_ROUNDS,
            "it stopped early: {}",
            done.rounds
        );
        // Interrupted is not discarded. A turn somebody stopped still did the work it did
        // before the interruption, and throwing that away would be the same mistake the round
        // limit was written to avoid.
        assert!(done.text.contains("looking again (0)"), "{}", done.text);
    }

    #[test]
    fn a_stop_left_over_from_before_would_stop_the_wrong_turn() {
        // Which is why the shell clears it when a turn begins. Asserted here because the
        // consequence lives in this file: a turn that is told to stop before it starts produces
        // no rounds at all, and that must read as stopped rather than as a broken model.
        let provider = Scripted::new(vec![answer("never asked", vec![])]);
        let store = TrustStore::default();
        let registry = registry();
        let who = epoch_kernel::CharacterId::new("mage").unwrap();
        let trust = TrustEngine::new(&store, &who, "archipelago", store.mode("archipelago"));

        let done = run(
            &provider,
            &trust,
            &registry,
            request(),
            &|| true,
            &mut |_| {},
            &mut |_| {},
        )
        .unwrap();
        assert_eq!(done.stop, Stop::Stopped);
        assert_eq!(done.rounds, 0);
        assert!(done.text.is_empty());
    }

    #[test]
    fn accept_edits_writes_without_asking_and_still_stops_before_deleting() {
        let mut store = TrustStore::default();
        store.set_mode("archipelago", Autonomy::AcceptEdits);

        // Deleting is not a file edit however cheap it looks, so this still stops.
        let provider = Scripted::new(vec![answer("", vec![call("delete_file", "old.md")])]);
        let (done, _) = go(&provider, &store, &registry());
        assert!(matches!(done.stop, Stop::NeedsApproval(_)));
    }

    #[test]
    fn what_the_user_already_allowed_runs_without_asking_and_leaves_evidence() {
        let mut store = TrustStore::default();
        store.remember(Policy::allow(
            "delete_file",
            Scope::World("archipelago".into()),
        ));
        let provider = Scripted::new(vec![
            answer("", vec![call("delete_file", "old.md")]),
            answer("Removed.", vec![]),
        ]);
        let (done, _) = go(&provider, &store, &registry());
        assert_eq!(done.stop, Stop::Finished);
        // Talking is not evidence; deleting a file is.
        assert_eq!(
            done.evidence
                .iter()
                .map(|m| m.summary.as_str())
                .collect::<Vec<_>>(),
            ["one file removed"]
        );
    }

    #[test]
    fn running_out_of_rounds_keeps_the_work_and_says_it_is_unfinished() {
        // Throwing away eight rounds of real work because the ninth was needed is the worst
        // available answer.
        let looping: Vec<Answer> = (0..MAX_ROUNDS + 2)
            .map(|i| answer(&format!("step {i}"), vec![call("read_file", "src/main.rs")]))
            .collect();
        let provider = Scripted::new(looping);
        let (done, _) = go(&provider, &TrustStore::default(), &registry());

        assert_eq!(done.stop, Stop::RoundsExhausted);
        assert_eq!(done.rounds, MAX_ROUNDS);
        assert!(done.text.contains("step 0"), "everything said is kept");
        assert!(done.text.contains(&format!("step {}", MAX_ROUNDS - 1)));
    }

    #[test]
    fn several_calls_in_one_breath_all_run_in_order() {
        let provider = Scripted::new(vec![
            answer(
                "Reading both.",
                vec![call("read_file", "a.rs"), call("read_file", "b.rs")],
            ),
            answer("Done.", vec![]),
        ]);
        let (done, steps) = go(&provider, &TrustStore::default(), &registry());
        assert_eq!(done.stop, Stop::Finished);
        assert_eq!(steps.len(), 4, "two Using and two Used");

        let second = &provider.seen.borrow()[1];
        let results: Vec<&Message> = second
            .messages
            .iter()
            .filter(|m| m.role == Role::Tool)
            .collect();
        assert_eq!(results.len(), 2);
        assert!(results[0].content.contains("a.rs"));
        assert!(results[1].content.contains("b.rs"));
    }
}
