//! What a `.gguf` file says about itself.
//!
//! ## Why Epoch needs to read this at all
//!
//! Everything Epoch knew about a local model came from its **filename** and its **size on
//! disk**. That was enough to put it on a shelf and no more, and it is the same claim/fact
//! distinction ADR-0024 draws for artwork: `Qwen3.8-27B-Uncensored-vision-f16` is a claim, and
//! `general.architecture = "clip"` is a fact.
//!
//! Three things this answers that nothing else could:
//!
//! 1. **Which file is a model's eyes.** A vision model is weights plus a projector, and
//!    `gguf_files` refused to pair them because guessing at `*-vision-*.gguf` would be inventing
//!    a pairing nobody stated. A projector says what it is, and says which model it belongs to.
//! 2. **Why a model will not load, before it is loaded.** `unknown model architecture: 'gptoss'`
//!    took ninety seconds and a terminal to find out. The architecture is in the first kilobyte.
//! 3. **What context it was trained for**, which is a real bound on what a turn may ask for.
//!
//! ## The format, and what this reader does with it
//!
//! ```text
//! "GGUF"  version:u32  tensor_count:u64  kv_count:u64
//! then kv_count of:  key:string  type:u32  value
//! ```
//!
//! A string is a `u64` length and that many bytes; an array is an element type, a `u64` count,
//! and the elements. The tensor table and the tensor data follow, and are never read here.
//!
//! **Values are skipped rather than collected.** A tokenizer's vocabulary is a single array of a
//! quarter of a million strings, and reading one to find `block_count` would cost more than
//! loading the model. Only the keys this asks for are kept; everything else is stepped over with
//! a `seek`.
//!
//! ## Refusing rather than trusting
//!
//! Every length is checked before it is used. A corrupt or hostile header can claim a string of
//! eighteen exabytes, and the answer to that is an error rather than an allocation — the file is
//! read from a shelf that anything on the machine can write into.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

/// A single metadata value, in the shapes Epoch actually reads.
///
/// Numbers arrive as several widths and mean one thing, so they land as one variant: a caller
/// asking for `block_count` does not care that it was written as a `u32`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(i128),
    Float(f64),
    Bool(bool),
    Text(String),
    /// Kept only for short arrays. A long one is stepped over — see [`MAX_KEPT_ARRAY`].
    Numbers(Vec<i128>),
}

impl Value {
    pub fn number(&self) -> Option<i128> {
        match self {
            Value::Number(n) => Some(*n),
            Value::Bool(b) => Some(i128::from(*b)),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Value::Text(said) => Some(said),
            _ => None,
        }
    }

    pub fn truth(&self) -> bool {
        matches!(self, Value::Bool(true)) || matches!(self, Value::Number(n) if *n != 0)
    }
}

/// What one `.gguf` file said about itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub version: u32,
    pub tensors: u64,
    /// Only the keys that were asked for.
    pub said: BTreeMap<String, Value>,
}

impl Header {
    /// `general.architecture` — `qwen3`, `gemma4`, `clip`, `gptoss`.
    ///
    /// This is the name llama.cpp matches against, so a build that answers
    /// `unknown model architecture: 'gptoss'` is answering about *this* string.
    pub fn architecture(&self) -> Option<&str> {
        self.said.get("general.architecture")?.text()
    }

    /// A key namespaced under this file's own architecture: `qwen3.block_count`.
    pub fn about(&self, suffix: &str) -> Option<&Value> {
        let arch = self.architecture()?;
        self.said.get(&format!("{arch}.{suffix}"))
    }

    /// Whether this file is a **projector** — a model's eyes rather than a model.
    ///
    /// Both halves, because `clip` is also the architecture of a stand-alone CLIP and only the
    /// flag says there is a vision encoder inside.
    pub fn is_projector(&self) -> bool {
        self.architecture() == Some("clip")
            && self
                .said
                .get("clip.has_vision_encoder")
                .is_some_and(Value::truth)
    }

    /// The width a projector hands its model, or the width a model receives.
    ///
    /// Measured on this machine: the 27B's projector says `clip.vision.projection_dim = 5120`
    /// and the 27B says `qwen35.embedding_length = 5120`; gemma's pair says 3840 and 3840.
    ///
    /// **It is a filter and never an identification.** `qwen3-14b` also embeds at 5120 while
    /// having no eyes at all, so a matching width says *not impossible*, not *this one* — a
    /// default is only safe where the measurement is a comparison, and this one has ties.
    pub fn width(&self) -> Option<i128> {
        if self.is_projector() {
            return self
                .said
                .get("clip.vision.projection_dim")
                .and_then(Value::number);
        }
        self.about("embedding_length").and_then(Value::number)
    }

    /// What this model was trained to hold, in tokens.
    pub fn trained_context(&self) -> Option<i128> {
        self.about("context_length").and_then(Value::number)
    }

    pub fn layers(&self) -> Option<i128> {
        self.about("block_count").and_then(Value::number)
    }
}

/// The keys worth keeping. Everything else is stepped over.
///
/// Matched by suffix rather than by full name because every model namespaces its own:
/// `qwen3.block_count`, `gemma4.block_count`. Kept deliberately short — this list is the reason
/// a header read costs a few kilobytes instead of a vocabulary.
const WANTED: &[&str] = &[
    "general.architecture",
    "general.name",
    "general.size_label",
    ".block_count",
    ".context_length",
    ".embedding_length",
    ".attention.head_count",
    ".attention.head_count_kv",
    ".attention.key_length",
    ".attention.value_length",
    // **What tells an MTP artefact from a plain one.** Measured 2026-08-31: the MTP build of
    // Qwen3.6-35B-A3B carries `nextn_predict_layers = 1` and twenty extra `blk.40.nextn.*`
    // tensors; the plain build carries the key not at all. Read here rather than from a filename,
    // which is a claim (ADR-0024) and which the two repositories make identically.
    ".nextn_predict_layers",
    "clip.has_vision_encoder",
    "clip.has_audio_encoder",
    "clip.vision.projection_dim",
    "clip.projector_type",
    "clip.vision.projector_type",
];

/// The longest string this will allocate. A key or a value beyond this is a corrupt header.
const MAX_STRING: u64 = 1 << 20;
/// The most metadata entries a real file has; llama.cpp's own writers produce dozens.
const MAX_KV: u64 = 100_000;
/// Arrays longer than this are stepped over rather than kept — a vocabulary, not a shape.
const MAX_KEPT_ARRAY: u64 = 64;

#[derive(Debug)]
pub enum Trouble {
    NotHere,
    NotGguf,
    Unreadable(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Trouble::NotHere => write!(f, "there is no file there"),
            Trouble::NotGguf => write!(f, "that is not a GGUF file"),
            Trouble::Unreadable(why) => write!(f, "the header could not be read ({why})"),
        }
    }
}

/// Read one file's header.
///
/// Cheap by construction: the file is opened, a few kilobytes are read, and the handle is
/// dropped. Nothing here touches the tensor data, so this is safe on a read path — unlike
/// hashing, which `recipes` deliberately keeps off one.
pub fn read(path: &Path) -> Result<Header, Trouble> {
    if !path.is_file() {
        return Err(Trouble::NotHere);
    }

    /*
        **Read once per file, per version of that file.**

        A header is a few kilobytes at the front of a file that can be twenty gigabytes, and it
        is asked for repeatedly by things that have no idea the others exist: `pair_up` reads
        every file on the shelf looking for projectors, and `deck::about` then reads the same
        files again for the trained context. Measured through the real window on an 80 GB shelf:
        **20.9 s** for the first pass and **9.1 s** for the second, every time MODELS opens.

        The key is what makes this a measurement rather than a memory: **length and modified
        time**, not the path. A file replaced in place is a different file and is read again,
        which is the same discipline as filing a model by its bytes (ADR-0032) — a path is a
        claim, and the pair below is what the filesystem actually says about it right now.

        Deliberately not an eviction cache: the entries are a handful of small maps, one per
        model on the shelf, and a policy for throwing them away would be more machinery than the
        thing it manages.
    */
    /// A header, keyed by the file it came from: path, size and modification time together.
    /// Any of the three changing is a different file, which is what makes this safe to keep.
    type Read = std::collections::HashMap<(std::path::PathBuf, u64, i64), Header>;
    static READ: std::sync::OnceLock<std::sync::Mutex<Read>> = std::sync::OnceLock::new();

    let stamp = std::fs::metadata(path).ok().map(|it| {
        let modified = it
            .modified()
            .ok()
            .and_then(|at| at.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_millis() as i64);
        (path.to_path_buf(), it.len(), modified)
    });

    if let Some(key) = &stamp {
        if let Some(held) = READ
            .get_or_init(Default::default)
            .lock()
            .ok()
            .and_then(|kept| kept.get(key).cloned())
        {
            return Ok(held);
        }
    }

    let file = File::open(path).map_err(|why| Trouble::Unreadable(why.to_string()))?;
    let header = read_from(&mut BufReader::new(file))?;

    if let Some(key) = stamp {
        if let Ok(mut kept) = READ.get_or_init(Default::default).lock() {
            kept.insert(key, header.clone());
        }
    }
    Ok(header)
}

fn read_from<R: Read + Seek>(r: &mut R) -> Result<Header, Trouble> {
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)
        .map_err(|why| Trouble::Unreadable(why.to_string()))?;
    if &magic != b"GGUF" {
        return Err(Trouble::NotGguf);
    }

    let version = u32(r)?;
    let tensors = u64(r)?;
    let count = u64(r)?;
    if count > MAX_KV {
        return Err(Trouble::Unreadable(format!("{count} metadata entries")));
    }

    let mut said = BTreeMap::new();
    for _ in 0..count {
        let key = text(r)?;
        let kind = u32(r)?;
        let keep = WANTED.iter().any(|want| {
            if let Some(suffix) = want.strip_prefix('.') {
                key.ends_with(suffix)
            } else {
                key == *want
            }
        });
        match value(r, kind, keep)? {
            Some(one) if keep => {
                said.insert(key, one);
            }
            _ => {}
        }
    }

    Ok(Header {
        version,
        tensors,
        said,
    })
}

/// One value: kept when `keep`, stepped over otherwise.
///
/// The numeric widths are all read rather than skipped even when unwanted, because reading four
/// bytes and dropping them is cheaper than the seek that would replace it. Only strings and
/// arrays — the ones that can be enormous — are actually stepped over.
fn value<R: Read + Seek>(r: &mut R, kind: u32, keep: bool) -> Result<Option<Value>, Trouble> {
    Ok(match kind {
        0 => Some(Value::Number(i128::from(byte(r)?))),
        1 => Some(Value::Number(i128::from(byte(r)? as i8))),
        2 => Some(Value::Number(i128::from(u16::from_le_bytes(fixed(r)?)))),
        3 => Some(Value::Number(i128::from(i16::from_le_bytes(fixed(r)?)))),
        4 => Some(Value::Number(i128::from(u32::from_le_bytes(fixed(r)?)))),
        5 => Some(Value::Number(i128::from(i32::from_le_bytes(fixed(r)?)))),
        6 => Some(Value::Float(f64::from(f32::from_le_bytes(fixed(r)?)))),
        7 => Some(Value::Bool(byte(r)? != 0)),
        8 => {
            if keep {
                Some(Value::Text(text(r)?))
            } else {
                let n = u64(r)?;
                step(r, n)?;
                None
            }
        }
        9 => array(r, keep)?,
        10 => Some(Value::Number(i128::from(u64::from_le_bytes(fixed(r)?)))),
        11 => Some(Value::Number(i128::from(i64::from_le_bytes(fixed(r)?)))),
        12 => Some(Value::Float(f64::from_le_bytes(fixed(r)?))),
        _ => return Err(Trouble::Unreadable(format!("value type {kind}"))),
    })
}

/// An array, kept only when it is short enough to be a shape rather than a vocabulary.
fn array<R: Read + Seek>(r: &mut R, keep: bool) -> Result<Option<Value>, Trouble> {
    let inner = u32(r)?;
    let n = u64(r)?;
    let short = keep && n <= MAX_KEPT_ARRAY;

    // A fixed-width element type has a known stride, so a long array costs one seek.
    if let Some(width) = stride(inner) {
        if !short {
            step(r, n.saturating_mul(width))?;
            return Ok(None);
        }
        let mut held = Vec::with_capacity(n as usize);
        for _ in 0..n {
            if let Some(one) = value(r, inner, true)?.and_then(|v| v.number()) {
                held.push(one);
            }
        }
        return Ok(Some(Value::Numbers(held)));
    }

    // Strings, and arrays of arrays: each has to be walked.
    for _ in 0..n {
        let _ = value(r, inner, false)?;
    }
    Ok(None)
}

/// Bytes one fixed-width value occupies, or `None` when it has no fixed width.
fn stride(kind: u32) -> Option<u64> {
    Some(match kind {
        0 | 1 | 7 => 1,
        2 | 3 => 2,
        4..=6 => 4,
        10..=12 => 8,
        _ => return None,
    })
}

fn step<R: Seek>(r: &mut R, by: u64) -> Result<(), Trouble> {
    r.seek(SeekFrom::Current(i64::try_from(by).map_err(|_| {
        Trouble::Unreadable(format!("a {by} byte value"))
    })?))
    .map_err(|why| Trouble::Unreadable(why.to_string()))?;
    Ok(())
}

fn fixed<R: Read, const N: usize>(r: &mut R) -> Result<[u8; N], Trouble> {
    let mut buf = [0u8; N];
    r.read_exact(&mut buf)
        .map_err(|why| Trouble::Unreadable(why.to_string()))?;
    Ok(buf)
}

fn byte<R: Read>(r: &mut R) -> Result<u8, Trouble> {
    Ok(fixed::<_, 1>(r)?[0])
}

fn u32<R: Read>(r: &mut R) -> Result<u32, Trouble> {
    Ok(u32::from_le_bytes(fixed(r)?))
}

fn u64<R: Read>(r: &mut R) -> Result<u64, Trouble> {
    Ok(u64::from_le_bytes(fixed(r)?))
}

fn text<R: Read>(r: &mut R) -> Result<String, Trouble> {
    let n = u64(r)?;
    if n > MAX_STRING {
        return Err(Trouble::Unreadable(format!("a {n} byte string")));
    }
    let mut buf = vec![0u8; n as usize];
    r.read_exact(&mut buf)
        .map_err(|why| Trouble::Unreadable(why.to_string()))?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}
