//! What reaches a Chronicle that was never meant to be read by a person.
//!
//! ## Why this exists
//!
//! Driving the real window on 2026-08-25, one message from `gemma4:12b` on Ollama arrived with
//! `<channel|>` in the middle of it — a chat-template marker, printed to the user, inside a
//! sentence. One sighting is not a vocabulary, and a guessed list of tokens to strip would be
//! the invented gauge this codebase keeps deleting. So this measures instead: **every local
//! runtime, every model, through the door Epoch actually knocks on**, and prints what comes back
//! raw so the list is read off the machine rather than remembered.
//!
//! ## It goes through the Provider, not the wire
//!
//! 11.18's lesson. A harness that posts its own JSON proves what a *server* does and proves
//! nothing about what Epoch would show. Everything here is `Provider::take_turn` followed by the
//! same two repairs `turn.rs` applies, in the same order — so what this prints is exactly what a
//! Chronicle would have held.
//!
//! `#[ignore]` — needs the three runtimes serving. Start them from Epoch's Connections deck.
//! `cargo test -p epoch-engine --test what_the_template_leaves_behind -- --ignored --nocapture`

use std::time::Instant;

use epoch_engine::openai::OpenAi;
use epoch_engine::provider::{KeepLoaded, Ollama, Provider, Request};
use epoch_kernel::{Conversation, Message};

/// Every local runtime, with the door Epoch drives it through.
const RUNTIMES: [(&str, &str, bool); 3] = [
    ("Ollama", "http://127.0.0.1:11434", true),
    ("llama.cpp", "http://127.0.0.1:8080", false),
    ("LM Studio", "http://127.0.0.1:1234", false),
];

/// Models to put the same question to, matched loosely because each runtime spells them its own
/// way (`gemma4:12b` against `gemma4-12b`).
const MODELS: [&str; 2] = ["gemma412b", "gptoss20b"];

/// Marker shapes worth reporting, and **not a strip list**.
///
/// Every one of these is a delimiter that belongs to a chat template rather than to a sentence.
/// The test asserts nothing about which appear: the point is to find out, on this machine, with
/// these templates. A fix built before this ran would have been built on one sighting.
const SUSPICIOUS: [&str; 12] = [
    "<channel",
    "<|",
    "|>",
    "<start_of_turn>",
    "<end_of_turn>",
    "<tool_call>",
    "</tool_call>",
    "<think>",
    "</think>",
    "<|im_start|>",
    "<|im_end|>",
    "<s>",
];

fn door(endpoint: &str, native: bool) -> Box<dyn Provider> {
    if native {
        Box::new(Ollama::named("probe", endpoint))
    } else {
        Box::new(OpenAi::named("probe", endpoint))
    }
}

/// The two questions that produced it: one that makes a model reason and then reach for a tool,
/// and one that is pure prose. A marker that only appears beside a tool call is a different
/// defect from one that appears in every answer.
const ASKS: [(&str, bool); 2] = [
    (
        "Draw me a picture of asuka evangelion in a neon city at night.",
        true,
    ),
    (
        "In two sentences, what makes a city look like the 1980s?",
        false,
    ),
];

#[test]
#[ignore = "needs Ollama, llama.cpp and LM Studio serving"]
fn no_template_marker_should_reach_a_chronicle() {
    let tools = vec![epoch_engine::capabilities::draw::open_studio()];
    let mut found: Vec<String> = Vec::new();

    for (name, endpoint, native) in RUNTIMES {
        let provider = door(endpoint, native);
        let status = provider.probe();
        if !status.online {
            println!("{name}: not answering at {endpoint}\n");
            continue;
        }
        let flat = |id: &str| id.replace([':', '-', '_', '.'], "");

        for wanted in MODELS {
            let Some(model) = status
                .models
                .iter()
                .find(|id| flat(id).contains(wanted))
                .cloned()
            else {
                println!("{name}: holds nothing matching {wanted}");
                continue;
            };

            for (ask, with_tools) in ASKS {
                let mut conversation = Conversation::opening(
                    "You are a crew member in a world that can draw pictures.",
                );
                conversation.say(Message::user(ask));
                let request = Request {
                    model: model.clone(),
                    conversation,
                    keep_loaded: KeepLoaded::Never,
                    parameters: Default::default(),
                    tuning: Default::default(),
                    tools: if with_tools {
                        tools.clone()
                    } else {
                        Vec::new()
                    },
                    // The built-in bound: a test is not the place to decide how patient a
                    // machine is.
                    most_rounds: None,
                };

                let began = Instant::now();
                let mut answer = match provider.take_turn(&request, &mut |_| {}) {
                    Ok(answer) => answer,
                    Err(err) => {
                        println!("{name} / {model} / tools={with_tools}: FAILED — {err}\n");
                        continue;
                    }
                };
                // Exactly what the turn loop does next, in that order — so what is printed is
                // what a Chronicle would have shown.
                answer.recover_written_call(&request.tools);
                answer.hush_echoed_calls();

                let hits: Vec<&str> = SUSPICIOUS
                    .into_iter()
                    .filter(|marker| answer.text.contains(marker))
                    .collect();
                println!(
                    "{name:<10} {model:<16} tools={with_tools:<5} {:>5.1}s  calls={}  markers={hits:?}",
                    began.elapsed().as_secs_f32(),
                    answer.calls.len()
                );
                // The whole message when something leaked, because the fix has to know where in
                // the text it sits — the one sighting had it mid-sentence, not at either end.
                if !hits.is_empty() {
                    println!(
                        "    ---8<--- {}\n    ---8<---",
                        answer.text.replace('\n', "\n    ")
                    );
                    found.push(format!("{name} / {model} / tools={with_tools}: {hits:?}"));
                } else if answer.text.trim().is_empty() && answer.calls.is_empty() {
                    println!("    (said nothing and reached for nothing)");
                }
            }
        }
        println!();
    }

    // **Reported, not enforced — for now.** This is a survey: it exists to produce the list a fix
    // would be built from. Once the strip is written this assertion is what holds it.
    if !found.is_empty() {
        println!("LEAKED:\n  {}", found.join("\n  "));
    }
}
