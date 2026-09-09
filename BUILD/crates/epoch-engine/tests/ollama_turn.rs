//! A complete turn, against a stand-in Ollama.
//!
//! The streaming parser is the part of a Provider most likely to break and least likely to be
//! noticed breaking: it reads newline-delimited JSON off a socket, one token at a time. So it
//! is tested against a real socket serving real bytes, rather than against a mocked HTTP client.
//!
//! Deliberately does **not** require Ollama to be installed. A test that only passes on a
//! machine with a 20B model pulled is a test nobody runs.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;

use epoch_engine::{Chunk, KeepLoaded, Ollama, Provider, Request};
use epoch_kernel::{Conversation, Message};

/// A one-request HTTP server that answers with `body`, then closes.
///
/// Returns the endpoint to point a provider at. Binds port 0 so the operating system picks a
/// free one — a hard-coded port makes tests fail when they run in parallel or on a busy box.
fn serve_once(status: &'static str, body: String) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind a free port");
    let endpoint = format!("http://{}", listener.local_addr().unwrap());

    thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };

        // Read past the request head so the client is not writing into a closed socket.
        {
            let mut reader = BufReader::new(&stream);
            let mut line = String::new();
            let mut length = 0usize;
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let trimmed = line.trim_end();
                if let Some(value) = trimmed.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
                if trimmed.is_empty() {
                    break;
                }
                line.clear();
            }
            if length > 0 {
                let mut body = vec![0u8; length];
                use std::io::Read;
                let _ = reader.read_exact(&mut body);
            }
        }

        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.flush();
    });

    endpoint
}

fn ask(model: &str) -> Request {
    let mut conversation = Conversation::opening("You are precise.");
    conversation.say(Message::user("Say hello."));
    Request {
        model: model.into(),
        conversation,
        keep_loaded: KeepLoaded::Never,
        parameters: Default::default(),
        tuning: Default::default(),
        tools: Vec::new(),
        most_rounds: None,
    }
}

#[test]
fn a_streamed_answer_arrives_token_by_token_and_whole() {
    // Ollama's real shape: one JSON object per line, the last one carrying `done`.
    let body = [
        r#"{"message":{"role":"assistant","content":"Hel"},"done":false}"#,
        r#"{"message":{"role":"assistant","content":"lo"},"done":false}"#,
        r#"{"message":{"role":"assistant","content":", world"},"done":false}"#,
        r#"{"message":{"role":"assistant","content":""},"done":true}"#,
    ]
    .join("\n");

    let provider = Ollama::new(serve_once("200 OK", body));
    let mut chunks = Vec::new();
    let answer = provider
        .take_turn(&ask("qwen3:14b"), &mut |c| chunks.push(c))
        .expect("the turn must succeed");

    assert_eq!(answer.text, "Hello, world", "the whole answer is returned");
    assert_eq!(
        chunks,
        vec![
            Chunk::Token("Hel".into()),
            Chunk::Token("lo".into()),
            Chunk::Token(", world".into()),
            Chunk::Done,
        ],
        "and it also arrived in pieces, in order — that is what lets the World show her thinking"
    );
}

#[test]
fn an_empty_token_is_not_streamed_as_one() {
    // The final frame carries `content: ""`. Forwarding it would make the World render an
    // update in which nothing changed.
    let body = [
        r#"{"message":{"content":"hi"},"done":false}"#,
        r#"{"message":{"content":""},"done":true}"#,
    ]
    .join("\n");

    let mut chunks = Vec::new();
    Ollama::new(serve_once("200 OK", body))
        .take_turn(&ask("qwen3:14b"), &mut |c| chunks.push(c))
        .unwrap();

    assert_eq!(chunks, vec![Chunk::Token("hi".into()), Chunk::Done]);
}

#[test]
fn reasoning_without_a_visible_answer_is_named_not_saved_as_a_blank_reply() {
    // Current Ollama models may stream `thinking` before `content`. If their output budget ends
    // there, forwarding an empty success makes the dialogue look like it lost a turn.
    let body = [
        r#"{"message":{"thinking":"checking the request"},"done":false}"#,
        r#"{"message":{"content":""},"done":true,"done_reason":"length"}"#,
    ]
    .join("\n");

    let err = Ollama::new(serve_once("200 OK", body))
        .take_turn(&ask("gemma4:12b"), &mut |_| {})
        .expect_err("hidden reasoning alone is not a user-visible answer");

    assert!(err.to_string().contains("visible content"), "{err}");
    assert!(err.to_string().contains("length"), "{err}");
}

#[test]
fn a_model_error_inside_a_200_body_is_still_an_error() {
    // Ollama reports "model not found" with HTTP 200 and an `error` field. Treating that as a
    // successful empty answer would have the Character reply with silence and no explanation.
    let body = r#"{"error":"model 'nope:1b' not found, try pulling it first"}"#.to_string();

    let err = Ollama::new(serve_once("200 OK", body))
        .take_turn(&ask("nope:1b"), &mut |_| {})
        .expect_err("a model error must not look like an answer");

    assert!(
        err.to_string().contains("not found"),
        "and it must say why: {err}"
    );
}

#[test]
fn a_truncated_stream_keeps_whatever_she_managed_to_say() {
    // The connection dies mid-thought, with no `done` frame. What arrived is still what she
    // said — discarding it would lose real work to a network hiccup.
    let body = r#"{"message":{"content":"I was saying"},"done":false}"#.to_string();

    let mut chunks = Vec::new();
    let answer = Ollama::new(serve_once("200 OK", body))
        .take_turn(&ask("qwen3:14b"), &mut |c| chunks.push(c))
        .expect("a truncated stream is not a failure");

    assert_eq!(answer.text, "I was saying");
    assert_eq!(
        chunks.last(),
        Some(&Chunk::Done),
        "the caller is still told it ended"
    );
}

#[test]
fn a_refusal_names_the_status_and_the_detail() {
    let err = Ollama::new(serve_once("404 Not Found", "no such route".into()))
        .take_turn(&ask("qwen3:14b"), &mut |_| {})
        .expect_err("a 404 is a refusal");
    assert!(err.to_string().contains("404"), "{err}");
}

#[test]
fn probing_reports_the_models_the_endpoint_actually_has() {
    let body = r#"{"models":[{"name":"gpt-oss:20b"},{"name":"qwen3:14b"}]}"#.to_string();
    let status = Ollama::new(serve_once("200 OK", body)).probe();

    assert!(status.online);
    assert!(status.local, "Ollama is the user's own machine");
    // Reported in the order the endpoint gave them: that order is information, and an
    // alphabetical list is not.
    assert_eq!(status.models, vec!["gpt-oss:20b", "qwen3:14b"]);
    assert!(
        status.note.is_none(),
        "online with models needs no explanation"
    );
}

#[test]
fn running_with_no_models_is_online_and_says_what_to_do() {
    // A real and confusing state: the server answers, and nothing can be asked of it.
    let status = Ollama::new(serve_once("200 OK", r#"{"models":[]}"#.into())).probe();

    assert!(status.online, "it answered, so it is online");
    assert!(status.models.is_empty());
    let note = status.note.expect("this state must explain itself");
    assert!(note.contains("pull"), "and say what to do about it: {note}");
}
