//! Does a picture handed to a real agent actually arrive?
//!
//! The unit tests assert the *envelope* — a `data:` URI for Codex, an API content block for
//! Claude Code — and both were measured before they were written. That still leaves the question
//! this file exists for, and it is the one that has caught this project before: the arguments
//! were right, the schema accepted them, and the thing still did not happen.
//!
//! It is not a hypothetical here. A filesystem path in Codex's `ImageUserInput.url` is **accepted
//! without complaint** — `turn/start` returns a turn, the run begins, and it fails minutes later
//! upstream with somebody else's error message. A test of the envelope alone would have passed on
//! that. Only running it says otherwise.
//!
//! ## The picture is drawn here
//!
//! A 64×64 PNG of one flat colour, written byte by byte so the fixture has no dependencies and
//! no ambiguity. `(16, 96, 220)` is blue by any reasonable name, and a model that answers "blue"
//! about it has looked at something — which a model describing a *filename* cannot do.
//!
//! ## Why it is `#[ignore]`d
//!
//! It needs the agent installed and signed in, a network round trip, and it spends the user's
//! plan allowance. Run deliberately:
//!
//! ```text
//! cargo test -p epoch-engine --test agents_can_see -- --ignored --nocapture
//! ```

use epoch_engine::agent::{Agent, NobodyToAsk, SharedImage, Task};

/// A flat-colour PNG, assembled by hand.
///
/// No image crate, and no checked-in binary: a fixture whose contents are computed is a fixture
/// whose *expected answer* is computable, and the assertion below depends on knowing exactly
/// what colour is in it.
fn flat_png(width: u32, height: u32, pixel: [u8; 3]) -> Vec<u8> {
    fn chunk(kind: &[u8], data: &[u8]) -> Vec<u8> {
        let mut out = (data.len() as u32).to_be_bytes().to_vec();
        let body: Vec<u8> = kind.iter().chain(data).copied().collect();
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
        out
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    // Uncompressed deflate: stored blocks, so nothing here needs a compressor either.
    fn stored(raw: &[u8]) -> Vec<u8> {
        let mut out = vec![0x78, 0x01];
        for (at, block) in raw.chunks(65_535).enumerate() {
            let last = u8::from((at + 1) * 65_535 >= raw.len());
            out.push(last);
            out.extend((block.len() as u16).to_le_bytes());
            out.extend((!(block.len() as u16)).to_le_bytes());
            out.extend(block);
        }
        // Adler-32 of the raw data, which the zlib wrapper carries.
        let (mut a, mut b) = (1u32, 0u32);
        for byte in raw {
            a = (a + u32::from(*byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        out.extend(((b << 16) | a).to_be_bytes());
        out
    }

    let mut ihdr = width.to_be_bytes().to_vec();
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 2, 0, 0, 0]); // 8-bit, truecolour

    // Each scanline is a filter byte followed by the row. Built as one row and repeated, which
    // is also what stops clippy reading the filter byte as an accidental duplicate push.
    let mut row = vec![0u8]; // filter: none
    for _ in 0..width {
        row.extend(pixel);
    }
    let raw: Vec<u8> = row.repeat(height as usize);

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    png.extend(chunk(b"IHDR", &ihdr));
    png.extend(chunk(b"IDAT", &stored(&raw)));
    png.extend(chunk(b"IEND", &[]));
    png
}

/// The Task Epoch builds when somebody shares a picture and asks about it.
fn looking_at(picture: Vec<u8>) -> Task {
    Task {
        // One word, so the answer is checkable rather than interpretable.
        intent: "What single colour fills the attached image? Answer with one word.".into(),
        character: "Mage".into(),
        persona: "You answer briefly and you never guess.".into(),
        crew: String::new(),
        skills: String::new(),
        library: String::new(),
        connections: String::new(),
        images: vec![SharedImage {
            name: "flat.png".into(),
            bytes: picture,
            mime: "image/png",
        }],
        model: String::new(),
        reasoning: None,
        // No door: this is about sight, and an open door would let a determined agent go and
        // find the answer some other way. Nothing to reach for means the picture is the only
        // place the answer can have come from.
        door: None,
        directory: std::env::temp_dir(),
        pictures: std::path::PathBuf::new(),
        thread: None,
        autonomy: epoch_kernel::Autonomy::Manual,
    }
}

/// Run one agent against the picture and return what it said.
fn what_it_saw(agent: &dyn Agent) -> String {
    let done = agent
        .work(
            &looking_at(flat_png(64, 64, [16, 96, 220])),
            &|| false,
            &NobodyToAsk,
            &mut |_| {},
        )
        .expect("the agent ran");
    done.text.to_lowercase()
}

#[test]
#[ignore = "needs the agent installed and signed in, and spends plan allowance"]
fn without_the_picture_it_cannot_answer() {
    // **The control**, and this project has twice been caught without one: a test that passes
    // because the code works and a test that passes because the assertion is easy look identical
    // from the outside.
    //
    // The same question with nothing attached. If "blue" comes back anyway, the two tests below
    // are measuring a lucky guess rather than a picture arriving — and everything they claim
    // about envelopes is unsupported.
    let agents = epoch_engine::agents::installed();
    let agent = agents
        .get("claude-code")
        .expect("this build knows Claude Code");

    let blind = Task {
        images: Vec::new(),
        ..looking_at(Vec::new())
    };
    let said = agent
        .work(&blind, &|| false, &NobodyToAsk, &mut |_| {})
        .expect("the agent ran")
        .text
        .to_lowercase();

    assert!(
        !said.contains("blue") && !said.contains("navy"),
        "it named the colour with no picture attached, so the tests below prove nothing: {said}"
    );
}

#[test]
#[ignore = "needs the agent installed and signed in, and spends plan allowance"]
fn claude_code_reads_a_picture_epoch_sent_it() {
    let agents = epoch_engine::agents::installed();
    let agent = agents
        .get("claude-code")
        .expect("this build knows Claude Code");

    let said = what_it_saw(agent);

    // "blue" or "navy" — the colour, not the filename. A model told only that `flat.png` was
    // shared has nothing to say about its contents, which is the whole distinction.
    assert!(
        said.contains("blue") || said.contains("navy"),
        "it did not describe the picture: {said}"
    );
}

#[test]
#[ignore = "needs the agent installed and signed in, and spends plan allowance"]
fn codex_reads_a_picture_epoch_sent_it() {
    // The one where the envelope matters most: a path is accepted here and fails upstream,
    // minutes later, with an error that names neither Epoch nor the cause.
    let agents = epoch_engine::agents::installed();
    let agent = agents.get("codex").expect("this build knows Codex");

    let said = what_it_saw(agent);

    assert!(
        said.contains("blue") || said.contains("navy"),
        "it did not describe the picture: {said}"
    );
}
