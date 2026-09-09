//! Which brains on this machine reach for the brush, and how long each takes.
//!
//! `#[ignore]` — needs the local runtimes serving. Start them from Epoch's Connections deck;
//! that is the path a user has, and it is the one being measured.
//!
//! ## Why this exists next to `a_model_reaches_for_the_brush`
//!
//! That test asks whether **one** model reaches for the tool. This one asks whether the
//! *runtime* changes the answer. Epoch speaks one wire shape to Ollama, llama.cpp and LM Studio
//! — the same OpenAI-compatible `tool_calls` — and "they all speak an API Epoch already speaks"
//! is a claim the Connections deck makes on the user's behalf. It is cheap to check and
//! expensive to be wrong about.
//!
//! It also **records latency**, which no other test here does. A tool call that arrives after
//! three minutes is technically a success and practically a failure, and the only way to know
//! which turns are worth optimising is to measure where the time goes.
//!
//! It asserts nothing about speed. A number measured on one card is not a threshold, and a test
//! that failed because a machine was busy would teach people to ignore it. It prints.

use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Every local runtime Epoch can start, with **the door Epoch actually knocks on**.
///
/// This distinction is the whole point and it was nearly missed. Epoch drives Ollama through its
/// native `/api/chat` (`provider.rs`) and everything else through `/v1/chat/completions`
/// (`openai.rs`). Measuring all three through the OpenAI door would have measured a path Epoch
/// never uses for Ollama — and, worse, would have made the two doors look alike when they do not
/// behave alike.
const RUNTIMES: [(&str, &str, Door); 3] = [
    ("Ollama", "http://127.0.0.1:11434", Door::Native),
    ("llama.cpp", "http://127.0.0.1:8080", Door::OpenAi),
    ("LM Studio", "http://127.0.0.1:1234", Door::OpenAi),
];

#[derive(Clone, Copy, PartialEq)]
enum Door {
    /// Ollama's own `/api/chat`, newline-delimited frames.
    Native,
    /// `/v1/chat/completions`, one JSON body.
    OpenAi,
}

/// The tools, exactly as Epoch declares them.
///
/// Built from the descriptors, never typed out: a copy would keep passing after somebody changed
/// the real thing, which is the failure this whole file exists to catch.
fn as_tools() -> Vec<Value> {
    [epoch_engine::capabilities::draw::open_studio()]
        .into_iter()
        .map(|descriptor| {
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();
            for parameter in &descriptor.parameters {
                properties.insert(
                    parameter.name.clone(),
                    json!({ "type": "string", "description": parameter.description }),
                );
                if parameter.required {
                    required.push(parameter.name.clone());
                }
            }
            json!({
                "type": "function",
                "function": {
                    "name": descriptor.id.to_string(),
                    "description": descriptor.summary,
                    "parameters": {
                        "type": "object",
                        "properties": properties,
                        "required": required,
                    }
                }
            })
        })
        .collect()
}

/// What one exchange cost and what came back.
struct Turn {
    took: Duration,
    called: Option<String>,
    arguments: Value,
    said: String,
}

fn ask(host: &str, door: Door, model: &str, said: &str, tools: bool) -> Result<Turn, String> {
    let mut body = json!({
        "model": model,
        "messages": [{ "role": "user", "content": said }],
        "stream": false,
    });
    if tools {
        body["tools"] = json!(as_tools());
    }
    let url = match door {
        Door::Native => format!("{host}/api/chat"),
        Door::OpenAi => format!("{host}/v1/chat/completions"),
    };
    let began = Instant::now();
    let answered = ureq::post(&url)
        .timeout(Duration::from_secs(900))
        .send_json(body);
    let took = began.elapsed();
    let answer: Value = match answered {
        Ok(response) => response.into_json().map_err(|why| why.to_string())?,
        Err(ureq::Error::Status(code, response)) => {
            return Err(format!(
                "{code}: {}",
                response
                    .into_string()
                    .unwrap_or_default()
                    .chars()
                    .take(200)
                    .collect::<String>()
            ))
        }
        Err(other) => return Err(other.to_string()),
    };
    // Both doors put the reply under `message`; only the OpenAI one wraps it in `choices`.
    let message = match door {
        Door::Native => &answer["message"],
        Door::OpenAi => &answer["choices"][0]["message"],
    };
    let call = message["tool_calls"]
        .as_array()
        .and_then(|calls| calls.first());
    Ok(Turn {
        took,
        called: call
            .and_then(|c| c["function"]["name"].as_str())
            .map(str::to_owned),
        // Ollama hands arguments back as an object; the OpenAI door hands back a string
        // holding JSON. Read both rather than assuming either.
        arguments: call
            .map(|c| &c["function"]["arguments"])
            .map(|raw| match raw.as_str() {
                Some(text) => serde_json::from_str(text).unwrap_or(Value::String(text.to_owned())),
                None => raw.clone(),
            })
            .unwrap_or(Value::Null),
        said: message["content"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_owned(),
    })
}

/// Which models a runtime is actually holding, asked rather than assumed.
fn models_of(host: &str) -> Vec<String> {
    let Ok(response) = ureq::get(&format!("{host}/v1/models"))
        .timeout(Duration::from_secs(5))
        .call()
    else {
        return Vec::new();
    };
    let Ok(listing) = response.into_json::<Value>() else {
        return Vec::new();
    };
    listing["data"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|row| row["id"].as_str())
                .filter(|id| !id.contains("embed"))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
#[ignore = "needs the local runtimes serving; start them from Epoch's Connections deck"]
fn every_local_runtime_reaches_for_the_brush() {
    // One family of brains, three runtimes. The variable under test is the runtime and the door,
    // so the model is held as still as three different naming schemes allow (`gemma4:12b` here,
    // `gemma4-12b` there).
    //
    // A second, better tool-caller is measured alongside it because one model failing proves
    // nothing about a door: if `qwen3-14b` calls natively where `gemma4` writes prose, the door
    // works and the brain is the limit — which is a different sentence to tell the user, and a
    // different thing to fix.
    for wanted in ["gemma4", "qwen314b"] {
        println!(
            "
=== {wanted} ==="
        );
        println!("running   | door    | warm  | no tools | with tools | reached for");
        println!("----------|---------|-------|----------|------------|------------");

        for (name, host, door) in RUNTIMES {
            let held = models_of(host);
            if held.is_empty() {
                println!("{name:<10}| not serving");
                continue;
            }
            // Matched on a *flattened* name. Ollama calls it `qwen3:14b` and the other two
            // call it `qwen3-14b`, and the first run of this reported "holds nothing matching"
            // for a model the machine plainly had — a blank cell that looked like a finding and
            // was a naming scheme.
            let flat = |id: &str| id.replace([':', '-', '_', '.'], "");
            let Some(model) = held.iter().find(|id| flat(id).contains(wanted)).cloned() else {
                println!("{name:<10}| holds nothing matching {wanted}");
                continue;
            };

            // **Load it first, and say so.** The first run of this measured 111.9s for
            // llama.cpp against 12.6s for Ollama and the difference was almost entirely a cold
            // model being read off disk. A number that includes a one-time load is not a
            // latency, and quoting one would send somebody optimising the wrong thing.
            let warm = ask(host, door, &model, "hi", false);
            let warm_took = match &warm {
                Ok(turn) => format!("{:.0}s", turn.took.as_secs_f32()),
                Err(why) => {
                    println!("{name:<10}| {model}: {why}");
                    continue;
                }
            };

            let plain = ask(
                host,
                door,
                &model,
                "Say the word ready and nothing else.",
                false,
            );
            let drew = ask(
                host,
                door,
                &model,
                "Haz una imagen de Frog de Chrono Trigger.",
                true,
            );

            let plain_took = match &plain {
                Ok(turn) => format!("{:.1}s", turn.took.as_secs_f32()),
                Err(why) => format!("failed: {why}"),
            };
            let door_name = if door == Door::Native {
                "native"
            } else {
                "openai"
            };
            match drew {
                Ok(turn) => {
                    println!(
                        "{name:<10}| {door_name:<8}| {warm_took:<6}| {plain_took:<9}| {:<11.1}| {}",
                        turn.took.as_secs_f32(),
                        turn.called.clone().unwrap_or_else(|| "NOTHING".to_owned())
                    );
                    if turn.called.is_none() {
                        println!(
                            "    it wrote instead: {}",
                            turn.said
                                .replace('\n', " ")
                                .chars()
                                .take(180)
                                .collect::<String>()
                        );
                    } else {
                        println!("    with: {}", turn.arguments);
                    }
                }
                Err(why) => println!(
                    "{name:<10}| {door_name:<8}| {warm_took:<6}| {plain_took:<9}| failed: {why}"
                ),
            }
        }
    }
    println!();
}
