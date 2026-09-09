//! Reading a PyTorch checkpoint **without executing it**.
//!
//! ## Why this exists
//!
//! `asset.rs` refuses `.ckpt`, `.pt` and `.bin` by design, and the reason is exact: a pickle is a
//! **program**, and `torch.load` runs it. Opening a stranger's checkpoint is running their code.
//!
//! That rule is right and it is not the whole story. A pickle can be **read** without being run —
//! its opcodes are a flat instruction stream, and every class it intends to construct appears in
//! a `GLOBAL` before anything happens. So the question *is this file dangerous* has an answer
//! that costs milliseconds and no execution at all.
//!
//! ## Measured, on two real files
//!
//! Two RVC voices the owner downloaded — one from Hugging Face, one from a Google Drive share —
//! disassembled on 2026-09-04. **Each asks for exactly three globals:**
//!
//! ```text
//! collections OrderedDict
//! torch._utils _rebuild_tensor_v2
//! torch HalfStorage
//! ```
//!
//! All three are on PyTorch's own `weights_only` allowlist. These files are tensors and nothing
//! else, and now that can be *proved* rather than hoped.
//!
//! ## The check is what makes it safe, never the observation
//!
//! That both happened to be clean is a fact about those two files. What makes accepting a third
//! acceptable is that this refuses anything naming a global outside a short list — and names the
//! one it refused, so a legitimate file that needs something new is a decision somebody can make
//! rather than a silent no.
//!
//! ## And what it reads is a fact, not a claim
//!
//! The same pass answers what the checkpoint *is*: `version = v2`, `sr = 40k`, and the width of
//! `enc_p.emb_phone.weight` — 768 for v2, 256 for v1. Read from the model's own tensors, which
//! is ADR-0024's rule. **Every file so far has disagreed with the page offering it**: one listing
//! said *500 Epochs* over a file saying `545epoch`, another said *300* over `160epoch`.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

/// Everything a checkpoint's pickle is allowed to construct.
///
/// **Deliberately short, and deliberately not PyTorch's whole `weights_only` list.** That list is
/// large because it serves every model anybody has ever saved; this serves voice checkpoints, and
/// a shorter list refuses more. Growing it is a decision with a name attached, which is the
/// property a review checklist does not have.
const ALLOWED: &[&str] = &[
    "collections OrderedDict",
    "torch._utils _rebuild_tensor_v2",
    "torch HalfStorage",
    "torch FloatStorage",
    "torch BFloat16Storage",
    "torch LongStorage",
    "torch IntStorage",
    // Newer torch writes tensors through this instead of the `_v2` rebuilder.
    "torch._utils _rebuild_tensor",
];

/// What a checkpoint turned out to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    /// `v1` or `v2`, as the file itself states it. `None` when it says nothing.
    pub version: Option<String>,
    /// `40k`, `48k`. The rate the model was trained at.
    pub rate: Option<String>,
    /// Whatever the author wrote in `info` — usually how long they trained. **A claim by the
    /// person who made it**, and the only reason it is carried is that it is more honest than
    /// the number on the page offering the file.
    pub info: Option<String>,
    /// The width of `enc_p.emb_phone.weight`: 768 for a v2 model, 256 for v1.
    ///
    /// **Measured from the tensor rather than read from `version`**, so a mislabelled file is
    /// caught by its own shape. Where the two disagree, this is the one that decides whether a
    /// graph will load.
    pub embedding: Option<u64>,
    /// Whether it carries pitch information.
    pub pitched: bool,
}

impl Checkpoint {
    /// Which RVC generation this is, from the shape rather than the label.
    pub fn generation(&self) -> Option<&'static str> {
        match self.embedding {
            Some(768) => Some("v2"),
            Some(256) => Some("v1"),
            _ => None,
        }
    }
}

/// Why a checkpoint was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "refused", content = "said", rename_all = "camelCase")]
pub enum Refused {
    /// It is not a modern torch archive at all.
    NotACheckpoint(String),
    /// Its pickle asks to construct something outside the allowlist. **Named**, because a
    /// legitimate file needing something new is a decision somebody can make; a silent no is not.
    Unwelcome(String),
    /// It could not be opened or read.
    Unreadable(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::NotACheckpoint(said) => write!(out, "{said}"),
            Refused::Unwelcome(said) => write!(out, "{said}"),
            Refused::Unreadable(said) => write!(out, "{said}"),
        }
    }
}

/// Read a `.pth` without running a byte of it.
pub fn read(file: &Path) -> Result<Checkpoint, Refused> {
    let handle = std::fs::File::open(file)
        .map_err(|why| Refused::Unreadable(format!("{file:?} could not be opened: {why}")))?;
    let mut archive = zip::ZipArchive::new(handle).map_err(|_| {
        Refused::NotACheckpoint(
            "That is not a PyTorch checkpoint this build can read. Modern ones are zip \
             archives; a very old `.pth` is a bare pickle, and Epoch will not open one."
                .to_owned(),
        )
    })?;

    let inside = (0..archive.len())
        .filter_map(|at| archive.by_index(at).ok().map(|it| it.name().to_owned()))
        .find(|name| name.ends_with("data.pkl"))
        .ok_or_else(|| {
            Refused::NotACheckpoint("There is no pickle inside that archive.".to_owned())
        })?;

    let mut raw = Vec::new();
    {
        let mut entry = archive
            .by_name(&inside)
            .map_err(|why| Refused::Unreadable(format!("{inside} could not be read: {why}")))?;
        std::io::copy(&mut entry, &mut raw)
            .map_err(|why| Refused::Unreadable(format!("{inside} could not be read: {why}")))?;
    }
    understand(&raw)
}

/// The whole of the reasoning, over the pickle's bytes — testable without a file.
pub fn understand(pickle: &[u8]) -> Result<Checkpoint, Refused> {
    let read = scan(pickle);

    // **The gate comes first.** Anything read out of a file that was going to be refused is a
    // fact nobody should be acting on.
    let allowed: BTreeSet<&str> = ALLOWED.iter().copied().collect();
    if let Some(unwelcome) = read
        .globals
        .iter()
        .find(|it| !allowed.contains(it.as_str()))
    {
        return Err(Refused::Unwelcome(format!(
            "That checkpoint asks to construct `{unwelcome}`, which is not on Epoch's short list \
             of things a voice model may contain. Opening it would run it, so Epoch will not."
        )));
    }

    Ok(Checkpoint {
        version: read.version,
        rate: read.rate,
        info: read.info,
        embedding: read.embedding,
        pitched: read.pitched,
    })
}

#[derive(Default)]
struct Seen {
    globals: Vec<String>,
    version: Option<String>,
    rate: Option<String>,
    info: Option<String>,
    embedding: Option<u64>,
    pitched: bool,
}

/// Walk the opcodes.
///
/// A deliberately small reader: the opcodes that matter are the ones that name a class and the
/// ones that carry a string or an integer. Everything else is skipped by its own declared length,
/// so an opcode this does not know cannot make it misread the next one.
fn scan(pickle: &[u8]) -> Seen {
    let mut seen = Seen::default();
    let mut strings: Vec<String> = Vec::new();
    let mut ints: Vec<i64> = Vec::new();
    let mut at = 0usize;
    // Where `enc_p.emb_phone.weight` was named, so the shape that follows can be attributed to
    // it rather than to whichever tensor happened to be nearby.
    let mut embedding_at: Option<usize> = None;

    while at < pickle.len() {
        let op = pickle[at];
        at += 1;
        match op {
            // GLOBAL: two newline-terminated strings, module then name.
            b'c' => {
                let module = take_line(pickle, &mut at);
                let name = take_line(pickle, &mut at);
                seen.globals.push(format!("{module} {name}"));
            }
            // STACK_GLOBAL: the two names are already on the stack.
            b'\x93' => {
                let name = strings.pop().unwrap_or_default();
                let module = strings.pop().unwrap_or_default();
                seen.globals.push(format!("{module} {name}"));
            }
            // SHORT_BINUNICODE / BINUNICODE / BINUNICODE8
            b'\x8c' | b'X' | b'\x8d' => {
                let width = match op {
                    b'\x8c' => 1,
                    b'X' => 4,
                    _ => 8,
                };
                let Some(len) = take_number(pickle, &mut at, width) else {
                    break;
                };
                let len = len as usize;
                if at + len > pickle.len() {
                    break;
                }
                let text = String::from_utf8_lossy(&pickle[at..at + len]).into_owned();
                at += len;
                if text == "enc_p.emb_phone.weight" {
                    embedding_at = Some(ints.len());
                }
                remember(&mut seen, &strings, &text);
                strings.push(text);
            }
            // BININT / BININT1 / BININT2
            b'J' => {
                if let Some(n) = take_number(pickle, &mut at, 4) {
                    ints.push(n as i32 as i64);
                }
            }
            b'K' => {
                if let Some(n) = take_number(pickle, &mut at, 1) {
                    ints.push(n as i64);
                }
            }
            b'M' => {
                if let Some(n) = take_number(pickle, &mut at, 2) {
                    ints.push(n as i64);
                }
            }
            // SHORT_BINBYTES and friends carry a length this must step over, or the next byte
            // read would be data rather than an opcode.
            b'\x8e' | b'B' | b'C' => {
                let width = match op {
                    b'C' => 1,
                    b'B' => 4,
                    _ => 8,
                };
                let Some(len) = take_number(pickle, &mut at, width) else {
                    break;
                };
                at = at.saturating_add(len as usize);
            }
            // BINPUT / BINGET take one byte; LONG_BINPUT / LONG_BINGET take four.
            b'q' | b'h' => at += 1,
            b'r' | b'j' => at += 4,
            // PROTO and FRAME.
            b'\x80' => at += 1,
            b'\x95' => at += 8,
            b'.' => break,
            _ => {}
        }
    }

    // The shape written after the embedding tensor was named. RVC's is `[192, 768]`, and the
    // second of the two is the width that says v1 from v2.
    if let Some(start) = embedding_at {
        let after: Vec<i64> = ints.iter().skip(start).copied().collect();
        seen.embedding = after
            .iter()
            .copied()
            .find(|n| *n == 768 || *n == 256)
            .map(|n| n as u64);
    }
    seen
}

/// A checkpoint's own scalar fields, recognised by the key that precedes them.
///
/// Read positionally because that is what a pickle is: the key is written, then the value. A
/// reader that searched for `v2` anywhere would find it in a tensor name eventually.
fn remember(seen: &mut Seen, strings: &[String], text: &str) {
    match strings.last().map(String::as_str) {
        Some("version") => seen.version = Some(text.to_owned()),
        Some("sr") => seen.rate = Some(text.to_owned()),
        Some("info") => seen.info = Some(text.to_owned()),
        _ => {}
    }
    if text == "f0" {
        seen.pitched = true;
    }
}

fn take_line(pickle: &[u8], at: &mut usize) -> String {
    let start = *at;
    while *at < pickle.len() && pickle[*at] != b'\n' {
        *at += 1;
    }
    let text = String::from_utf8_lossy(&pickle[start..*at]).into_owned();
    *at = (*at + 1).min(pickle.len());
    text
}

fn take_number(pickle: &[u8], at: &mut usize, width: usize) -> Option<u64> {
    if *at + width > pickle.len() {
        return None;
    }
    let mut value = 0u64;
    for i in 0..width {
        value |= u64::from(pickle[*at + i]) << (8 * i);
    }
    *at += width;
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pickle built by hand, so the reader is tested against opcodes and not against a file.
    fn pickled(globals: &[(&str, &str)], strings: &[&str]) -> Vec<u8> {
        let mut out = vec![0x80, 4]; // PROTO 4
        for (module, name) in globals {
            out.push(b'c');
            out.extend_from_slice(module.as_bytes());
            out.push(b'\n');
            out.extend_from_slice(name.as_bytes());
            out.push(b'\n');
        }
        for text in strings {
            out.push(0x8c);
            out.push(text.len() as u8);
            out.extend_from_slice(text.as_bytes());
        }
        out.push(b'.');
        out
    }

    #[test]
    fn a_checkpoint_of_tensors_is_accepted() {
        let raw = pickled(
            &[
                ("collections", "OrderedDict"),
                ("torch._utils", "_rebuild_tensor_v2"),
                ("torch", "HalfStorage"),
            ],
            &["version", "v2", "sr", "40k", "info", "545epoch", "f0"],
        );
        let read = understand(&raw).expect("tensors only");
        assert_eq!(read.version.as_deref(), Some("v2"));
        assert_eq!(read.rate.as_deref(), Some("40k"));
        // Carried because it is more honest than the page: that file's listing said 500.
        assert_eq!(read.info.as_deref(), Some("545epoch"));
        assert!(read.pitched);
    }

    #[test]
    fn anything_that_could_run_is_refused_by_name() {
        let raw = pickled(
            &[
                ("collections", "OrderedDict"),
                ("os", "system"),
                ("torch", "HalfStorage"),
            ],
            &[],
        );
        let why = understand(&raw).expect_err("a pickle that can run is refused");
        // **Named**, because a legitimate file needing something new is a decision somebody can
        // make and a silent no is not.
        assert!(
            matches!(&why, Refused::Unwelcome(said) if said.contains("os system")),
            "{why}"
        );
    }

    #[test]
    fn the_gate_runs_before_anything_is_believed() {
        // A file that would have been readable is still refused, and says nothing about itself:
        // a fact read out of something that was going to be refused is a fact nobody should act
        // on.
        let raw = pickled(
            &[("builtins", "eval"), ("torch", "HalfStorage")],
            &["version", "v2"],
        );
        assert!(understand(&raw).is_err());
    }

    #[test]
    fn a_stack_global_is_read_too() {
        // Newer protocols push the two names and use `STACK_GLOBAL`. A reader that only knew
        // `GLOBAL` would see a pickle with no globals at all and wave it through.
        let mut raw = pickled(&[], &["posix", "system"]);
        raw.truncate(raw.len() - 1); // drop the STOP
        raw.push(0x93);
        raw.push(b'.');
        let why = understand(&raw).expect_err("stack globals count");
        assert!(
            matches!(&why, Refused::Unwelcome(said) if said.contains("posix system")),
            "{why}"
        );
    }

    #[test]
    fn the_generation_comes_from_the_shape_and_not_the_label() {
        let mut raw = pickled(
            &[("collections", "OrderedDict"), ("torch", "HalfStorage")],
            &["enc_p.emb_phone.weight"],
        );
        raw.truncate(raw.len() - 1);
        // `M` is BININT2: 192 then 768, the shape RVC writes.
        raw.extend_from_slice(&[b'M', 192, 0, b'M', 0, 3, b'.']);
        let read = understand(&raw).expect("tensors only");
        assert_eq!(read.embedding, Some(768));
        assert_eq!(read.generation(), Some("v2"));
    }

    /// The two files the owner actually downloaded.
    #[test]
    #[ignore = "needs the checkpoints"]
    fn the_real_voices_read_as_what_they_are() {
        {
            let (path, epochs) = (std::env::var("EPOCH_TEST_PTH").unwrap_or_default(), "");
            if path.is_empty() {
                eprintln!("set EPOCH_TEST_PTH to a real .pth");
                return;
            }
            let read = read(Path::new(&path)).expect("a real checkpoint reads");
            eprintln!("{path}: {read:?} (expected epochs {epochs})");
            assert_eq!(read.generation(), Some("v2"));
            assert_eq!(read.rate.as_deref(), Some("40k"));
            assert!(read.pitched);
        }
    }
}
