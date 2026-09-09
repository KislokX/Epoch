//! Live check against the real Ollama on this machine. Not part of the suite: it needs a
//! machine with Ollama running, and a test that only passes there is a test nobody runs.

use epoch_engine::{Chunk, KeepLoaded, Ollama, Provider, Request};
use epoch_kernel::{Conversation, Message};

fn main() {
    let ollama = Ollama::default();

    let started = std::time::Instant::now();
    let status = ollama.probe();
    println!(
        "probe: online={} local={} in {:?}\n  endpoint: {}\n  models: {:?}\n  note: {:?}",
        status.online,
        status.local,
        started.elapsed(),
        status.endpoint,
        status.models,
        status.note
    );

    let Some(model) = status.models.first().cloned() else {
        println!("no models — stopping here");
        return;
    };

    // Mage's real authored prompt, as the vault holds it.
    let mut conversation = Conversation::opening(
        "You explore the solution space before committing, and you name tradeoffs explicitly.\n\
         You would rather understand a problem than reach for a familiar answer.\n\
         You are enthusiastic and precise, and you never talk down to anyone.",
    );
    conversation.say(Message::user(
        "In one short sentence: what are you working on right now?",
    ));

    println!("\nasking {model}...");
    let started = std::time::Instant::now();
    let mut tokens = 0usize;
    let mut first_token_at = None;

    match ollama.take_turn(
        &Request {
            model: model.clone(),
            conversation,
            keep_loaded: KeepLoaded::Never,
            parameters: Default::default(),
            tuning: Default::default(),
            tools: Vec::new(),
            most_rounds: None,
        },
        &mut |chunk| match chunk {
            Chunk::Token(_) => {
                if first_token_at.is_none() {
                    first_token_at = Some(started.elapsed());
                }
                tokens += 1;
            }
            Chunk::Done => {}
        },
    ) {
        Ok(answer) => println!(
            "OK in {:?} · {tokens} tokens · first token at {:?}\n\n{}",
            started.elapsed(),
            first_token_at,
            answer.text.trim()
        ),
        Err(err) => println!("FAILED: {err}"),
    }
}
