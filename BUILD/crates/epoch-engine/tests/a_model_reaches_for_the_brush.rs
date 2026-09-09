//! Does a local model actually *use* `open_studio`?
//!
//! `#[ignore]` — needs LM Studio serving on 1234.
//!
//! A descriptor can be well written and ignored. Every other test here holds Epoch's own
//! behaviour still; this one asks the question none of them can: given the tool exactly as Epoch
//! declares it, does a 14B model on somebody's desk reach for it, and does it fill in the right
//! things?
//!
//! **Run them one at a time** — `--test-threads=1`. Two of these at once reach one LM Studio
//! while it is loading a model, and the second gets an HTML error page rather than JSON. The
//! failure is the harness's, not the code's, and it looks alarming.
//!
//! It matters because the descriptor is the *only* instruction the model gets. There is no
//! prompt engineering behind it, no examples, no retry — ADR-0030 put the encouragement in the
//! descriptor precisely because a result that reads as an instruction makes models call again
//! and again (the `see_image` defect, 2026-08-18).
//!
//! **What it asks changed on 2026-08-28** and the question did not. `draw_image` is gone: one
//! thing draws and it is the panel, so what a model reaches for when somebody asks for a picture
//! is `open_studio`. It takes no arguments, so the halves of this that read a `describe` and a
//! `style` out of the call went with it — that a model can be trusted to invent neither is now
//! true by construction rather than by measurement, which is the stronger form.

use serde_json::{json, Value};

const HOST: &str = "http://127.0.0.1:1234";

/// The tool, exactly as Epoch declares it to a model.
///
/// Built from the descriptor rather than typed out here: a test against a hand-written copy
/// would keep passing after somebody changed the real one.
fn as_a_tool() -> Value {
    let descriptor = epoch_engine::capabilities::draw::open_studio();
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
}

fn ask(said: &str) -> Value {
    let answered = ureq::post(&format!("{HOST}/v1/chat/completions"))
        .timeout(std::time::Duration::from_secs(300))
        .send_json(json!({
            "model": "qwen3-14b",
            "messages": [{ "role": "user", "content": said }],
            "tools": [as_a_tool()],
            "max_tokens": 400,
            "stream": false,
        }));
    match answered {
        Ok(response) => response.into_json().expect("JSON"),
        Err(ureq::Error::Status(_, response)) => {
            panic!("refused: {}", response.into_string().unwrap_or_default())
        }
        Err(other) => panic!("{other}"),
    }
}

fn called(answer: &Value) -> Option<Value> {
    let call = answer["choices"][0]["message"]["tool_calls"]
        .as_array()?
        .first()?;
    let arguments = call["function"]["arguments"].as_str()?;
    Some(json!({
        "name": call["function"]["name"],
        "arguments": serde_json::from_str::<Value>(arguments).unwrap_or(Value::Null),
    }))
}

#[test]
#[ignore = "needs LM Studio serving qwen3-14b on 1234"]
fn asked_for_a_picture_it_reaches_for_the_brush() {
    let answer = ask("Make me a picture of a lighthouse on a cliff at dawn, in pixel art.");
    let call = called(&answer).unwrap_or_else(|| {
        panic!(
            "it did not reach for the tool. It said: {}",
            answer["choices"][0]["message"]["content"]
        )
    });

    assert_eq!(call["name"], "epoch_open_studio", "{call}");

    // Measured 2026-08-28, in the real window rather than here: asked *"hazme una imagen de
    // Chrono"*, a character opened the panel in 11.4s and GENERATE drew in 13.0s. What this
    // holds still is the half that happens before Epoch is involved — whether the model reaches
    // at all.
    println!("{}", serde_json::to_string_pretty(&call).unwrap());
}

/// And it does not reach for it when nobody asked for a picture.
///
/// The other half, and the one that costs a user something when it is wrong: a tool that fires
/// on *"describe a lighthouse"* would draw when somebody wanted a sentence.
#[test]
#[ignore = "needs LM Studio serving qwen3-14b on 1234"]
fn asked_for_words_it_leaves_the_brush_alone() {
    let answer = ask("In one sentence, what is a lighthouse for?");
    assert!(
        called(&answer).is_none(),
        "it drew a picture nobody asked for: {answer}"
    );
}
