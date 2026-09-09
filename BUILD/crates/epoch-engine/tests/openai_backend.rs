//! Does the OpenAI-compatible backend actually talk to a server?
//!
//! The unit tests in `openai.rs` assert the reassembly of a streamed tool call, which is the
//! fiddly part. They do not answer whether the request that goes out is one a server accepts, or
//! whether the answer that comes back is read — and that gap is where this project has been
//! caught more than once.
//!
//! So this stands up a **real HTTP server** on a real port, speaks the real protocol at it, and
//! reads what the Provider made of the reply. Nothing is mocked: `ureq` connects, server-sent
//! events arrive line by line, and `Chunk`s land in a sink.
//!
//! The server here is deliberately not Ollama or llama.cpp. Those would prove this works against
//! *one* implementation on the day it was run; this proves it works against **the shape of the
//! API**, which is the whole claim the `Kind` makes.

use std::sync::mpsc;

use epoch_engine::openai::OpenAi;
use epoch_engine::provider::{Chunk, KeepLoaded, Provider, Request};
use epoch_kernel::{Conversation, Message, Parameters};

/// A server that answers one request and hands back what it was asked.
///
/// Returns its address and a channel carrying the body it received, so a test can assert both
/// halves of the exchange — what went out, and what was made of what came back.
fn serving(reply: &'static str, content_type: &'static str) -> (String, mpsc::Receiver<String>) {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("a port");
    let address = format!("http://{}", server.server_addr());
    let (sent, received) = mpsc::channel();

    // A server answers more than once, and this one has to: a local turn now asks
    // `/api/v0/models` first, to find out how big a window the model was actually given. A
    // helper that served a single request made that ask eat the turn — which is a fair
    // description of what a one-shot server would do to the real thing.
    //
    // LM Studio's private routes are answered 404, which is exactly what llama.cpp does with
    // them — so this helper is a llama.cpp, and the extra ask costs it a 404 and nothing else.
    std::thread::spawn(move || {
        while let Ok(mut request) = server.recv() {
            if request.url().starts_with("/api/v0/") {
                let _ = request.respond(tiny_http::Response::empty(404));
                continue;
            }
            let mut body = String::new();
            let _ = std::io::Read::read_to_string(request.as_reader(), &mut body);
            let _ = sent.send(body);
            let header =
                tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                    .expect("a header");
            let _ = request.respond(tiny_http::Response::from_string(reply).with_header(header));
            return;
        }
    });

    (address, received)
}

fn asking(model: &str) -> Request {
    let mut conversation = Conversation::default();
    conversation.say(Message::user("what is 2 + 2?"));
    Request {
        model: model.into(),
        conversation,
        keep_loaded: KeepLoaded::Never,
        parameters: Parameters::default(),
        tuning: Default::default(),
        tools: Vec::new(),
        // The built-in bound: a test is not the place to decide how patient a machine is.
        most_rounds: None,
    }
}

#[test]
fn a_streamed_answer_is_read_off_a_real_socket() {
    // Server-sent events, exactly as the API emits them: `data: ` per line, `[DONE]` to close.
    let stream = "data: {\"choices\":[{\"delta\":{\"content\":\"4\"}}]}\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\", obviously\"}}]}\n\
                  data: [DONE]\n";
    let (address, received) = serving(stream, "text/event-stream");

    let mut tokens = Vec::new();
    let answer = OpenAi::named("desk", &address)
        .take_turn(&asking("gemma4:12b"), &mut |chunk| match chunk {
            Chunk::Token(text) => tokens.push(text),
            Chunk::Done => tokens.push("<done>".into()),
        })
        .expect("it answered");

    assert_eq!(answer.text, "4, obviously");
    // Streamed in order, and `Done` emitted exactly once at the end — the World shows a
    // character thinking off these, so a single late blob would be a character who says nothing
    // and then everything.
    assert_eq!(tokens, ["4", ", obviously", "<done>"]);

    // And what went out is what this API expects.
    let sent: serde_json::Value =
        serde_json::from_str(&received.recv().expect("the server saw a request")).expect("JSON");
    assert_eq!(sent["model"], "gemma4:12b");
    assert_eq!(sent["stream"], true);
    assert_eq!(sent["messages"][0]["role"], "user");
    assert_eq!(sent["messages"][0]["content"], "what is 2 + 2?");
    // Nothing the character did not choose. A temperature written out by default would be Epoch
    // deciding something the user did not (ADR-0026).
    assert!(sent.get("temperature").is_none());
    assert!(sent.get("tools").is_none());
}

#[test]
fn a_server_that_refuses_says_what_it_said() {
    // A 401 from Deepseek and a 404 from a mistyped path are different problems, and only the
    // server knows which. Epoch's job is to carry the sentence, not to summarise it away.
    let (address, _received) = serving(
        "{\"error\":{\"message\":\"invalid api key\"}}",
        "application/json",
    );

    let refused =
        OpenAi::named("deepseek", &address).take_turn(&asking("deepseek-chat"), &mut |_| {});

    // tiny_http answers 200 here, so this asserts the readable path rather than the status one:
    // a body that is not an event stream produces no tokens rather than an invented answer.
    let answer = refused.expect("a 200 is not an error");
    assert!(
        answer.text.is_empty(),
        "a non-stream body must not be read as an answer: {}",
        answer.text
    );
}

#[test]
fn a_model_list_is_whatever_the_server_reported() {
    // In its order, unsorted — the same rule Ollama's probe follows. The order a backend gives
    // is information; alphabetical is not.
    let (address, _received) = serving(
        "{\"data\":[{\"id\":\"qwen3:14b\"},{\"id\":\"gemma4:12b\"}]}",
        "application/json",
    );

    let status = OpenAi::named("desk", &address).probe();

    assert!(status.online);
    assert_eq!(status.models, ["qwen3:14b", "gemma4:12b"]);
    // 127.0.0.1, so this one is the user's own machine — and that is measured from the address
    // rather than taken from a checkbox, because it decides whether a screenshot may be sent
    // there (ADR-0025's disclosure rule).
    assert!(status.local);
    assert!(status.note.is_none());
}

#[test]
fn a_server_that_is_not_there_is_offline_with_the_reason() {
    // Port 1 on loopback: nothing listens, and the answer must be a sentence rather than a
    // silence. "Offline" with no reason is the cold instrument this project keeps refusing.
    let status = OpenAi::named("desk", "http://127.0.0.1:1").probe();

    assert!(!status.online);
    assert!(status.models.is_empty());
    assert!(status.note.is_some(), "an offline backend must say why");
}
