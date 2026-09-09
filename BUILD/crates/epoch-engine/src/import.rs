//! Accepting artwork from the user.
//!
//! Epoch consumes authored content; it never generates it (`CLAUDE.md` — "Assets are authored,
//! never generated"). This is the door that content comes through.
//!
//! ## Why there is no file dialog
//!
//! The browser already has one. The webview reads the bytes with `<input type="file">` and
//! hands them here base64-encoded, which means no `tauri-plugin-dialog`, no
//! `tauri-plugin-fs`, and — the part that matters — **no capability that grants the frontend
//! filesystem access**. The surface can offer a file, and only the Engine can write one.
//!
//! ## Why the caller never names the file
//!
//! The destination name is chosen by the Engine from the *sniffed* format, never from what
//! the user's file was called. That removes an entire class of problem rather than defending
//! against it: there is no path to traverse, no extension to spoof, no name to collide.
//!
//! The claimed extension is likewise ignored. A `.png` containing a Windows executable is a
//! thing that exists; the magic bytes are not.

use std::path::{Path, PathBuf};

use base64::Engine as _;

/// The most artwork Epoch will accept in one import.
///
/// Every asset ends up base64-encoded inside a projection that crosses the IPC boundary on
/// each change, so a 50 MB illustration would not merely be large — it would be re-sent. This
/// is generous for key art and firmly finite.
pub const MAX_BYTES: usize = 8 * 1024 * 1024;

// **There is no ceiling on a picture Epoch made, and there was one twice.**
//
// The first was `MAX_BYTES` — the import cap, applied to a produced picture whose argument it
// does not carry. The second replaced it with a number derived from what the panel could ask for
// at the time: 2048² through a 4× upscaler. Then 11.26 gave the panel the sizes people name out
// loud, 4K UHD among them, and 4K through 4× is 15360×8640. The derivation was still written
// beside the constant and the constant had not moved.
//
// That is `CLAUDE.md`'s rule — *when two things must agree, derive both from the same
// measurement* — and the fix is not a third number. It is that **the question has nothing behind
// it**: when this is called, the picture already exists on disk. ComfyUI wrote it before Epoch
// was told about it; the file that produced this comment is 136,951,962 bytes and is still
// sitting in that server's output folder, because refusing it here saved nothing and lost only
// Epoch's copy of something the user had already paid seconds of card for.
//
// A gate with nothing behind it stays open. The costs that *are* real live elsewhere and are
// answered where they happen: the paste door still caps, because those bytes cross the IPC as
// base64; and the model door refuses, because a hosted API will.

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("that file is not valid image data")]
    NotDecodable,
    #[error("that image is {size} bytes, and more than this door accepts")]
    TooLarge { size: usize },
    #[error("that file is empty")]
    Empty,
    /// Deliberately the same message for "unknown format" and "renamed executable": the user
    /// only needs to know Epoch will not take it.
    #[error("Epoch reads PNG, JPEG, WebP and SVG images; that file is none of them")]
    UnsupportedFormat,
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// A format Epoch can both store and draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Svg,
}

impl ImageFormat {
    /// Every format, in one place.
    ///
    /// Three things walk this list — clearing an earlier import, forgetting one, and the test
    /// that holds it against what [`crate::asset`] can draw. They were three literals, which is
    /// three chances for a new format to be added to two of them.
    pub const ALL: [ImageFormat; 4] = [
        ImageFormat::Png,
        ImageFormat::Jpeg,
        ImageFormat::Webp,
        ImageFormat::Svg,
    ];

    pub const fn extension(self) -> &'static str {
        match self {
            ImageFormat::Png => "png",
            // `jpg` rather than `jpeg`: both are the same format, and one spelling on disk
            // means one file rather than two for the same picture.
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
            ImageFormat::Svg => "svg",
        }
    }

    /// What to call this on a wire — a `data:` URI, or an API content block.
    ///
    /// From the **sniffed** format, like everything else here, so a picture is described to
    /// somebody else by what it contains rather than by what its file was called.
    pub const fn mime(self) -> &'static str {
        match self {
            ImageFormat::Png => "image/png",
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Svg => "image/svg+xml",
        }
    }

    /// Identify an image by what it *is*, never by what it is called.
    ///
    /// Only the formats [`crate::asset`] can already deliver — the two lists must not drift,
    /// or Epoch would happily store something it cannot draw.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Some(ImageFormat::Png);
        }
        // SOI, then the first marker. Every JPEG variant Epoch might be handed — JFIF, Exif
        // out of a phone, a screenshot tool's own — starts this way.
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(ImageFormat::Jpeg);
        }
        if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
            return Some(ImageFormat::Webp);
        }

        // SVG is text, so there is no magic number — only a shape. Scan a bounded prefix so a
        // large binary file cannot be searched end to end looking for something to match.
        let head = &bytes[..bytes.len().min(1024)];
        let text = std::str::from_utf8(head).ok()?;
        let start = text.trim_start();
        if start.starts_with("<svg") || (start.starts_with("<?xml") && text.contains("<svg")) {
            return Some(ImageFormat::Svg);
        }
        None
    }

    /// How big this image is, in pixels.
    ///
    /// Headers only — no decoding, and no image-processing dependency. The one caller needs a
    /// *size*, and pulling in a decoder to learn two numbers that are written in the first
    /// twenty bytes is complexity that has not earned itself.
    ///
    /// `None` when the header is malformed or the format does not say. That is a real answer:
    /// a World whose land could not be measured keeps whatever extent it already had, and the
    /// land still draws.
    pub fn measure(self, bytes: &[u8]) -> Option<(u32, u32)> {
        match self {
            // IHDR is always the first chunk, and its width and height are big-endian u32 at a
            // fixed offset. Guaranteed by the format, not by convention.
            ImageFormat::Png => {
                if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
                    return None;
                }
                let read = |at: usize| {
                    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
                };
                Some((read(16), read(20)))
            }
            // No fixed offset: the size lives in a start-of-frame marker somewhere after a
            // run of metadata segments (an Exif thumbnail can be tens of kilobytes). So walk
            // the segment chain — each carries its own length — and read the first SOF found.
            ImageFormat::Jpeg => {
                let mut at = 2; // past SOI
                loop {
                    // Fill bytes are legal between segments.
                    while bytes.get(at) == Some(&0xFF) && bytes.get(at + 1) == Some(&0xFF) {
                        at += 1;
                    }
                    if bytes.get(at)? != &0xFF {
                        return None;
                    }
                    let marker = *bytes.get(at + 1)?;
                    // SOFn, excluding DHT (C4), JPG (C8) and DAC (CC), which share the range
                    // and are not frame headers.
                    if (0xC0..=0xCF).contains(&marker) && !matches!(marker, 0xC4 | 0xC8 | 0xCC) {
                        let d = bytes.get(at + 5..at + 9)?;
                        let h = u16::from_be_bytes([d[0], d[1]]);
                        let w = u16::from_be_bytes([d[2], d[3]]);
                        return (w > 0 && h > 0).then_some((u32::from(w), u32::from(h)));
                    }
                    // Start of scan: the entropy-coded data begins and there is no header left
                    // to find. Stop rather than reading compressed bytes as lengths.
                    if marker == 0xDA {
                        return None;
                    }
                    let length = u16::from_be_bytes([*bytes.get(at + 2)?, *bytes.get(at + 3)?]);
                    at += 2 + usize::from(length);
                }
            }
            // Three container shapes, and each stores the size differently. Handled explicitly
            // rather than guessed: a wrong size here silently rescales somebody's whole World.
            ImageFormat::Webp => {
                let chunk = bytes.get(12..16)?;
                match chunk {
                    // Extended: 24-bit widths minus one, little-endian.
                    b"VP8X" => {
                        let d = bytes.get(24..30)?;
                        let w = u32::from_le_bytes([d[0], d[1], d[2], 0]) + 1;
                        let h = u32::from_le_bytes([d[3], d[4], d[5], 0]) + 1;
                        Some((w, h))
                    }
                    // Lossy: 14 bits each, after the start code.
                    b"VP8 " => {
                        let d = bytes.get(26..30)?;
                        let w = u16::from_le_bytes([d[0], d[1]]) & 0x3FFF;
                        let h = u16::from_le_bytes([d[2], d[3]]) & 0x3FFF;
                        Some((u32::from(w), u32::from(h)))
                    }
                    // Lossless: 14 bits each, packed across four bytes.
                    b"VP8L" => {
                        let d = bytes.get(21..25)?;
                        let bits = u32::from_le_bytes([d[0], d[1], d[2], d[3]]);
                        Some(((bits & 0x3FFF) + 1, ((bits >> 14) & 0x3FFF) + 1))
                    }
                    _ => None,
                }
            }
            // Vector art has no pixels, so the honest size is the coordinate space it was drawn
            // in. `viewBox` is that; `width`/`height` may be units this does not understand.
            ImageFormat::Svg => {
                let head = &bytes[..bytes.len().min(4096)];
                let text = std::str::from_utf8(head).ok()?;
                let at = text.find("viewBox")?;
                let open = text[at..].find(['"', '\''])? + at + 1;
                let rest = &text[open..];
                let close = rest.find(['"', '\''])?;
                let numbers: Vec<f64> = rest[..close]
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| part.parse().ok())
                    .collect();
                match numbers.as_slice() {
                    [_, _, w, h] if *w > 0.0 && *h > 0.0 => Some((*w as u32, *h as u32)),
                    _ => None,
                }
            }
        }
    }
}

/// A moving picture, which is a thing Epoch can now make and can never accept as artwork.
///
/// ## Why this is not another arm of [`ImageFormat`]
///
/// `ImageFormat` is the list of things Epoch will **take in** — a World's key art, a portrait, a
/// picture pasted into a conversation, something handed to a model that can see. ADR-0024 holds
/// it to exactly what [`crate::asset`] can deliver, with a test, and every one of those callers
/// means *a picture* rather than *a file*. Adding `Mp4` to it would let a video be imported as a
/// World's land and handed to a vision model as an image, both of which are nonsense that would
/// arrive as a confusing failure somewhere else entirely.
///
/// So this is the other list: what a **capability produced**, which travels one path — into the
/// vault, onto a Quest as evidence, and out through `epoch://` to a window that can play it.
///
/// ## One format, because one is what the server offers
///
/// Asked of this ComfyUI (2026-08-28), `SaveVideo` publishes `format: [auto, mp4]` and
/// `codec: [auto, h264]`. WebM is here because ComfyUI's own `SaveWEBM` writes one and a video
/// arriving from an agent could be either — sniffed, never assumed, like everything else here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovingFormat {
    Mp4,
    WebM,
}

/// Sound Epoch made.
///
/// **FLAC, because that is what the server writes.** Measured 2026-08-28: `SaveAudio` takes an
/// `AUDIO` and a prefix and answers `epoch_00112.flac`, `fLaC` in its first four bytes. MP3 and
/// Opus have their own nodes and are not offered — a format choice with no measured reason
/// behind it is a control nobody can answer — but they are sniffed, because a sound could arrive
/// from an agent or a workflow somebody imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoundFormat {
    Flac,
    Mp3,
    Ogg,
    Wav,
}

impl SoundFormat {
    pub const ALL: [SoundFormat; 4] = [
        SoundFormat::Flac,
        SoundFormat::Mp3,
        SoundFormat::Ogg,
        SoundFormat::Wav,
    ];

    pub const fn extension(self) -> &'static str {
        match self {
            SoundFormat::Flac => "flac",
            SoundFormat::Mp3 => "mp3",
            // Opus and Vorbis both ride in an Ogg container, and both play in the same element.
            // Telling them apart means reading past the header for a codec nothing here asks
            // about — a measurement with no consumer.
            SoundFormat::Ogg => "ogg",
            SoundFormat::Wav => "wav",
        }
    }

    pub const fn mime(self) -> &'static str {
        match self {
            SoundFormat::Flac => "audio/flac",
            SoundFormat::Mp3 => "audio/mpeg",
            SoundFormat::Ogg => "audio/ogg",
            SoundFormat::Wav => "audio/wav",
        }
    }

    /// Identify it by what it *is*, never by what it is called (ADR-0024).
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"fLaC") {
            return Some(SoundFormat::Flac);
        }
        if bytes.starts_with(b"OggS") {
            return Some(SoundFormat::Ogg);
        }
        // RIFF, then the form type. The same shape WebP uses, which is why the form is checked
        // rather than the container alone.
        if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE" {
            return Some(SoundFormat::Wav);
        }
        // An ID3 tag, or a bare frame header. Both are how an MP3 starts in the wild.
        if bytes.starts_with(b"ID3")
            || (bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0)
        {
            return Some(SoundFormat::Mp3);
        }
        None
    }
}

impl MovingFormat {
    pub const ALL: [MovingFormat; 2] = [MovingFormat::Mp4, MovingFormat::WebM];

    pub const fn extension(self) -> &'static str {
        match self {
            MovingFormat::Mp4 => "mp4",
            MovingFormat::WebM => "webm",
        }
    }

    pub const fn mime(self) -> &'static str {
        match self {
            MovingFormat::Mp4 => "video/mp4",
            MovingFormat::WebM => "video/webm",
        }
    }

    /// Identify it by what it *is*, never by what it is called (ADR-0024).
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        // ISO base media: a length, then the literal `ftyp`, at a fixed offset. Every MP4 a
        // muxer writes starts this way, whatever brand follows.
        if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
            return Some(MovingFormat::Mp4);
        }
        // Matroska/WebM's EBML header. The doctype that separates the two sits further in and
        // is not read: both play in a `<video>`, and a webm claiming to be a matroska is a
        // distinction with no consumer.
        if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
            return Some(MovingFormat::WebM);
        }
        None
    }
}

/// A mesh Epoch made.
///
/// **glTF binary, because that is what the server writes.** Measured 2026-08-29: `SaveGLB` takes
/// a `MESH` and a prefix and answers `epoch_00118_.glb`, `glTF` in its first four bytes and a
/// version right after.
///
/// Nothing in the window can display one, which is the whole reason a mesh gets a *preview* —
/// see `turntable`. This type exists so the mesh itself is kept and named by its bytes like
/// everything else, rather than being the one thing that arrives and is not filed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshFormat {
    Glb,
}

impl MeshFormat {
    pub const ALL: [MeshFormat; 1] = [MeshFormat::Glb];

    pub const fn extension(self) -> &'static str {
        match self {
            MeshFormat::Glb => "glb",
        }
    }

    pub const fn mime(self) -> &'static str {
        match self {
            MeshFormat::Glb => "model/gltf-binary",
        }
    }

    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        // The container's own magic, then a version it is worth having read: a `glTF` with a
        // version this does not know is still a glTF, so the version is not checked. What is
        // checked is that four bytes are there to be checked.
        if bytes.len() >= 12 && bytes.starts_with(b"glTF") {
            return Some(MeshFormat::Glb);
        }
        None
    }
}

/// What a finished piece of work turned out to be.
///
/// The union of the two lists above, and the only place they meet: a capability hands over bytes
/// and does not know which it made — `SaveImage` and `SaveVideo` are the same *shape* of answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MadeFormat {
    Still(ImageFormat),
    Moving(MovingFormat),
    Sound(SoundFormat),
    Mesh(MeshFormat),
}

impl MadeFormat {
    /// Pictures first, because a picture is the ordinary case and both tests are cheap.
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if let Some(still) = ImageFormat::sniff(bytes) {
            return Some(MadeFormat::Still(still));
        }
        if let Some(moving) = MovingFormat::sniff(bytes) {
            return Some(MadeFormat::Moving(moving));
        }
        if let Some(sound) = SoundFormat::sniff(bytes) {
            return Some(MadeFormat::Sound(sound));
        }
        MeshFormat::sniff(bytes).map(MadeFormat::Mesh)
    }

    pub const fn extension(self) -> &'static str {
        match self {
            MadeFormat::Still(it) => it.extension(),
            MadeFormat::Moving(it) => it.extension(),
            MadeFormat::Sound(it) => it.extension(),
            MadeFormat::Mesh(it) => it.extension(),
        }
    }

    pub const fn mime(self) -> &'static str {
        match self {
            MadeFormat::Still(it) => it.mime(),
            MadeFormat::Moving(it) => it.mime(),
            MadeFormat::Sound(it) => it.mime(),
            MadeFormat::Mesh(it) => it.mime(),
        }
    }

    /// Whether this is something that plays rather than something that is looked at.
    ///
    /// Asked by the window, which draws one with `<img>` and the other with `<video>`, and by the
    /// Chronicle, which must not offer a video to a model that can see.
    pub const fn moves(self) -> bool {
        matches!(self, MadeFormat::Moving(_))
    }
}

/// Accept image bytes and write them into `dir` as `<stem>.<sniffed extension>`.
///
/// Returns the reference to store in a manifest, relative to `dir`. Any earlier import under
/// the same stem is removed first, so changing a PNG for an SVG does not silently leave the
/// old one behind for the fallback chain to find.
pub fn accept_image(dir: &Path, stem: &str, base64_data: &str) -> Result<String, ImportError> {
    accept_image_up_to(dir, stem, base64_data, MAX_BYTES)
}

/// The same door, with the caller saying which limit applies.
///
/// Two callers and two reasons: what a person hands over rides a projection, and what Epoch made
/// is fetched once by name. Passing the limit in keeps one implementation and makes the
/// difference visible at the call site rather than hidden in a constant everything shares.
pub fn accept_image_up_to(
    dir: &Path,
    stem: &str,
    base64_data: &str,
    most: usize,
) -> Result<String, ImportError> {
    let bytes = decode(base64_data)?;

    if bytes.len() > most {
        return Err(ImportError::TooLarge { size: bytes.len() });
    }

    write_sniffed(dir, stem, &bytes)
}

/// Write bytes that are already bytes, under a name taken from what they contain.
///
/// **Split out because one caller was paying a toll for a road it never took.** `keep_made` holds
/// a picture in memory, and reached the disk through `accept_image_up_to` — which meant encoding
/// it to base64 and immediately decoding it back to the same bytes. At the 137 MB render that
/// started all this, that is about 320 MB of transient allocation to arrive where it began.
///
/// It happened because the import door was the only way in, and the import door takes base64
/// because its bytes come from the webview. Same shape as the cap: a constraint that belongs to
/// one path, applied to a path whose argument it does not carry.
///
/// Everything the door actually does for a caller — sniffing the format rather than trusting a
/// name (ADR-0024), clearing the other formats this stem could have been, naming the file — lives
/// here and both callers get it.
fn write_sniffed(dir: &Path, stem: &str, bytes: &[u8]) -> Result<String, ImportError> {
    // **Empty before unreadable, and a test holds the order.** Nothing sniffs as anything, so
    // sniffing first would answer *that is not a format Epoch knows* about a file with no bytes
    // in it — true, useless, and pointing the person at the wrong problem.
    if bytes.is_empty() {
        return Err(ImportError::Empty);
    }
    let format = ImageFormat::sniff(bytes).ok_or(ImportError::UnsupportedFormat)?;
    write_as(dir, stem, bytes, MadeFormat::Still(format))
}

/// Accept sound bytes and write them into `dir` as `<stem>.<sniffed extension>`.
///
/// **Its own door, for the reason `write_made` is.** A World supplying `sfx.click` may hand over
/// a WAV and may not hand over a PNG — not because a PNG is dangerous but because a picture
/// filed as a click is a failure nobody can diagnose from where it surfaces. Sniffed from the
/// bytes like everything else, never from the name the file arrived with.
pub fn accept_sound(dir: &Path, stem: &str, base64_data: &str) -> Result<String, ImportError> {
    let bytes = decode(base64_data)?;
    if bytes.len() > MAX_BYTES {
        return Err(ImportError::TooLarge { size: bytes.len() });
    }
    if bytes.is_empty() {
        return Err(ImportError::Empty);
    }
    let format = SoundFormat::sniff(&bytes).ok_or(ImportError::UnsupportedFormat)?;
    write_as(dir, stem, &bytes, MadeFormat::Sound(format))
}

/// Take a sound back out again, whichever of the four formats it landed as.
pub fn forget_sound(dir: &Path, stem: &str) {
    for format in SoundFormat::ALL {
        let _ = std::fs::remove_file(dir.join(format!("{stem}.{}", format.extension())));
    }
}

/// The same, for bytes that may also be a video.
///
/// **A separate door on purpose.** What a person imports must stay exactly what [`crate::asset`]
/// can deliver (ADR-0024, held by a test); what a capability *made* has a wider range, because
/// a video is a legitimate result and an illegitimate portrait. Two doors is how that stays a
/// property of the type rather than a rule somebody has to remember at each call site.
fn write_made(dir: &Path, stem: &str, bytes: &[u8]) -> Result<String, ImportError> {
    if bytes.is_empty() {
        return Err(ImportError::Empty);
    }
    let format = MadeFormat::sniff(bytes).ok_or(ImportError::UnsupportedFormat)?;
    write_as(dir, stem, bytes, format)
}

fn write_as(
    dir: &Path,
    stem: &str,
    bytes: &[u8],
    format: MadeFormat,
) -> Result<String, ImportError> {
    if bytes.is_empty() {
        return Err(ImportError::Empty);
    }

    std::fs::create_dir_all(dir).map_err(|source| ImportError::Write {
        path: dir.to_path_buf(),
        source,
    })?;

    // Clear every format this stem could previously have been, not just the one we are about
    // to write. Otherwise `preview.png` would linger after the user replaced it with an SVG.
    //
    // Videos are included even though a stem is the hash of its own bytes and can therefore
    // never change what it is: this list is what makes that guarantee unnecessary to rely on,
    // and a list that covers half the formats is the one that will be wrong later.
    for existing in ImageFormat::ALL
        .map(MadeFormat::Still)
        .iter()
        .chain(MovingFormat::ALL.map(MadeFormat::Moving).iter())
        .chain(SoundFormat::ALL.map(MadeFormat::Sound).iter())
        .chain(MeshFormat::ALL.map(MadeFormat::Mesh).iter())
    {
        if *existing == format {
            continue;
        }
        let _ = std::fs::remove_file(dir.join(format!("{stem}.{}", existing.extension())));
    }

    let name = format!("{stem}.{}", format.extension());
    let path = dir.join(&name);
    std::fs::write(&path, bytes).map_err(|source| ImportError::Write {
        path: path.clone(),
        source,
    })?;

    Ok(name)
}

/// Remove whatever was imported under `stem`, in any format. Missing is success.
pub fn forget_image(dir: &Path, stem: &str) {
    for format in ImageFormat::ALL {
        let _ = std::fs::remove_file(dir.join(format!("{stem}.{}", format.extension())));
    }
}

/// Decode a `data:` URI or bare base64, for a caller outside this module.
///
/// The same door [`decode`] is, made public because a reference picture for a studio is not an
/// import: nothing is filed, nothing is named after it on this disk, and the bytes go straight
/// to a server that will name them itself. Reusing this rather than writing a second decoder is
/// what keeps *what a `FileReader` produces* meaning one thing.
pub fn decode_shared(payload: &str) -> Result<Vec<u8>, ImportError> {
    decode(payload)
}

/// A stable short name for these exact bytes.
///
/// Public for the same reason: something outside has to name a picture, and naming it from what
/// it contains is ADR-0024's rule rather than a new one.
pub fn fingerprint(bytes: &[u8]) -> u64 {
    fnv1a(bytes)
}

/// Decode, tolerating a `data:` URI prefix so the caller can hand over what `FileReader`
/// produced without unwrapping it first.
fn decode(payload: &str) -> Result<Vec<u8>, ImportError> {
    let body = match payload.split_once(";base64,") {
        Some((head, rest)) if head.starts_with("data:") => rest,
        _ => payload,
    };
    base64::engine::general_purpose::STANDARD
        .decode(body.trim())
        .map_err(|_| ImportError::NotDecodable)
}

/// Keep an image the user shared in a conversation, and say which file it became.
///
/// **The bytes choose the name**, which is ADR-0024's rule taken to its end rather than a new
/// one. [`accept_image`] is given a stem by its caller because artwork replaces artwork — a
/// World has one `preview`, a character has one `sprite`. A conversation has no such slot: every
/// image is another image, so something has to name them, and the only thing that can name them
/// without inventing anything is what they contain.
///
/// Two consequences worth having, both falling out rather than designed for:
///
/// - the same picture pasted twice is **one file**, referenced twice;
/// - a Chronicle entry can be deleted without wondering whether its file is shared.
///
/// FNV-1a, not a cryptographic hash. Nothing here is defending against somebody *constructing* a
/// collision — the input is the user's own screenshot, and the worst case of an accidental one
/// is two of their images becoming one. At 64 bits that needs on the order of a billion images
/// in one vault before it is worth thinking about, and it costs sixteen lines instead of a
/// dependency in a binary that ships to people.
pub fn keep_shared_image(
    vault: &Path,
    name: &str,
    base64_data: &str,
) -> Result<epoch_kernel::ImageAttachment, ImportError> {
    let bytes = decode(base64_data)?;
    let stem = format!("{:016x}", fnv1a(&bytes));
    let file = accept_image(&shared_images(vault), &stem, base64_data)?;
    Ok(epoch_kernel::ImageAttachment {
        // What the user's file was called, kept only to be shown back to them. It never
        // reaches the filesystem — see the type's own note.
        name: name.trim().to_owned(),
        file,
    })
}

/// Keep bytes Epoch itself produced, named from the bytes.
///
/// The other door into the same folder. [`keep_shared_image`] takes what a person handed over,
/// as base64 across IPC; this takes what a capability made, which is already bytes and never
/// crossed a surface.
///
/// Named by content for the reason everything here is: two identical pictures are one file, and
/// nothing downstream has to trust a name. It lands beside the shared ones because the Chronicle
/// draws that folder — a produced picture and a pasted one are the same kind of thing to a
/// conversation.
pub fn keep_bytes(vault: &Path, bytes: &[u8]) -> Result<String, ImportError> {
    Ok(keep_made(vault, None, bytes)?.file)
}

/// Where a picture Epoch made ended up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// Its name inside the vault, which is what the Chronicle renders from.
    pub file: String,
    /// Where it was also written, when the World has a Project Root. `None` when it has none.
    pub beside_the_work: Option<PathBuf>,
}

/// Keep a picture Epoch made: in the vault always, and **where the work is** when there is one.
///
/// ## Two copies, and they answer to different people
///
/// The vault copy is the **record**. The Chronicle renders from that folder through a resolver
/// that takes a filename and never a path (ADR-0024), evidence on a Quest points at it, and
/// History must not lose a picture because somebody tidied their project afterwards.
///
/// The Project Root copy is the **work**. A person who asked for a picture wants it where they
/// are working, not inside an application's data folder they would have to be told about.
///
/// Duplication is deliberate here and is not the thing ADR-0032 refuses: that rule is about not
/// copying tens of gigabytes of somebody else's model files, and it refuses copying because two
/// copies of an asset *diverge* when one is updated. A finished picture is bytes that never
/// change, named by its own hash — so the two copies are the same file by construction, and the
/// day one is deleted the other is still exactly what it was.
///
/// ## In a folder, never loose in the root
///
/// Epoch writes into somebody's project; it does not get to scatter. `pictures/` is created if
/// it is missing and is otherwise left entirely alone — Epoch owns the file it writes and
/// nothing else in there.
///
/// ## Failing to write the second copy is not failing to draw
///
/// A read-only tree, a full disk, a path that stopped existing — none of that should throw away
/// a picture the card already spent seconds making. The vault copy is what everything downstream
/// needs; the other is reported when it happened and simply absent when it did not.
pub fn keep_made(vault: &Path, root: Option<&Path>, bytes: &[u8]) -> Result<Kept, ImportError> {
    let stem = format!("{:016x}", fnv1a(bytes));
    let file = write_made(&shared_images(vault), &stem, bytes)?;

    let beside_the_work = root.and_then(|root| {
        // **`creations`, because it is not only pictures any more** — and nothing is moved out of
        // an older `pictures/`. That folder is in *the user's project*, under their version
        // control and possibly referenced by their own work; Epoch may add to their repository
        // because they asked for a picture, and may not rearrange it because a name got better.
        let into = root.join("creations");
        std::fs::create_dir_all(&into).ok()?;
        // The same name as the vault's, so the two are visibly one file rather than two
        // pictures that happen to look alike.
        let at = into.join(&file);
        std::fs::write(&at, bytes).ok()?;
        Some(at)
    });

    Ok(Kept {
        file,
        beside_the_work,
    })
}

/// Where everything shared in or made by a conversation lives.
///
/// One folder for the whole vault rather than one per World. The bytes are the identity, so the
/// same screenshot pasted into two Worlds is the same file — and a per-World folder would make
/// two copies of it to express a separation nothing needs.
///
/// **It was called `images` and now holds video and sound too.** The name was true when a
/// picture was all that could arrive; a folder called `images` holding `.mp4` and `.flac` is a
/// label that teaches the wrong shape of the thing. Renamed to what it is, with the contents
/// carried over once — see [`gather_what_was_made`].
pub fn shared_images(vault: &Path) -> PathBuf {
    let media = vault.join("media");
    gather_what_was_made(vault, &media);
    media
}

/// Carry an older vault's `images/` into `media/`, once.
///
/// **Moved rather than left behind.** Every Chronicle refers to these by filename and resolves
/// them through the folder above, so a half-moved store is a conversation with holes in it. One
/// pass, on the first read after an update, and the old folder is removed only when it is empty.
///
/// A file that will not move is left where it is and the rest still move: a locked handle on one
/// screenshot is not a reason to strand the other hundred. What that costs is that the picture
/// stops resolving, which is the same thing that happens to a file the user deleted, and the
/// Chronicle already says so.
///
/// **Epoch's own store, which is why this is allowed at all.** Nothing here touches a folder in
/// the user's project — see `keep_made`, where the older name is left alone precisely because it
/// is theirs.
fn gather_what_was_made(vault: &Path, into: &Path) {
    let older = vault.join("images");
    if !older.is_dir() {
        return;
    }
    if std::fs::create_dir_all(into).is_err() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(&older) else {
        return;
    };
    for entry in entries.flatten() {
        let from = entry.path();
        let Some(name) = from.file_name() else {
            continue;
        };
        let to = into.join(name);
        if to.exists() {
            // Already carried over. Named by the hash of its own bytes, so the same name is the
            // same file — and the old one is simply the copy to drop.
            let _ = if from.is_dir() {
                std::fs::remove_dir_all(&from)
            } else {
                std::fs::remove_file(&from)
            };
            continue;
        }
        let _ = std::fs::rename(&from, &to);
    }
    // Only when nothing is left. A folder with a stuck file in it stays, and stays visible.
    let _ = std::fs::remove_dir(&older);
}

#[cfg(test)]
mod carried_over {
    use super::*;

    struct Dir(PathBuf);
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn vault(name: &str) -> Dir {
        let at = std::env::temp_dir().join(format!("epoch-carry-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("a vault");
        Dir(at)
    }

    /// **An older vault's pictures follow it**, because every Chronicle names them by filename
    /// and resolves them through this one folder. A half-moved store is a conversation with
    /// holes in it.
    #[test]
    fn what_was_in_images_is_in_media_afterwards() {
        let v = vault("move");
        std::fs::create_dir_all(v.0.join("images")).unwrap();
        std::fs::write(v.0.join("images").join("a1b2.png"), b"a picture").unwrap();
        std::fs::write(v.0.join("images").join("c3d4.flac"), b"a sound").unwrap();

        let media = shared_images(&v.0);
        assert!(media.ends_with("media"));
        assert_eq!(std::fs::read(media.join("a1b2.png")).unwrap(), b"a picture");
        assert_eq!(std::fs::read(media.join("c3d4.flac")).unwrap(), b"a sound");
        // And the old folder is gone rather than left as a second place to look.
        assert!(!v.0.join("images").exists());
    }

    /// Twice is not twice as much: the second pass has nothing to carry and nothing to lose.
    #[test]
    fn carrying_it_over_again_changes_nothing() {
        let v = vault("twice");
        std::fs::create_dir_all(v.0.join("images")).unwrap();
        std::fs::write(v.0.join("images").join("a1b2.png"), b"a picture").unwrap();

        let once = shared_images(&v.0);
        std::fs::write(once.join("e5f6.png"), b"made since").unwrap();
        let again = shared_images(&v.0);

        assert_eq!(once, again);
        assert_eq!(std::fs::read(again.join("a1b2.png")).unwrap(), b"a picture");
        assert_eq!(
            std::fs::read(again.join("e5f6.png")).unwrap(),
            b"made since"
        );
    }

    /// **The name is the hash of the bytes**, so a name that exists in both places is one file
    /// twice. The older copy is dropped rather than overwriting the newer, which would be the
    /// same bytes written for nothing.
    #[test]
    fn a_file_already_carried_over_is_not_carried_twice() {
        let v = vault("dup");
        std::fs::create_dir_all(v.0.join("images")).unwrap();
        std::fs::create_dir_all(v.0.join("media")).unwrap();
        std::fs::write(v.0.join("images").join("a1b2.png"), b"the same bytes").unwrap();
        std::fs::write(v.0.join("media").join("a1b2.png"), b"the same bytes").unwrap();

        let media = shared_images(&v.0);
        assert_eq!(
            std::fs::read(media.join("a1b2.png")).unwrap(),
            b"the same bytes"
        );
        assert!(!v.0.join("images").exists());
    }

    /// A vault that never had the older folder is untouched, and is not given one.
    #[test]
    fn a_fresh_vault_gains_nothing_to_clean_up() {
        let v = vault("fresh");
        let media = shared_images(&v.0);
        assert!(
            media.is_dir() || !media.exists(),
            "either made or not, never half"
        );
        assert!(!v.0.join("images").exists());
    }
}

/// FNV-1a, 64-bit. See [`keep_shared_image`] for why this rather than a real hash.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smallest real PNG: an 1x1 transparent pixel.
    const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    struct Dir(PathBuf);

    impl Dir {
        fn new(name: &str) -> Self {
            let d = std::env::temp_dir().join(format!("epoch-import-{name}"));
            let _ = std::fs::remove_dir_all(&d);
            Self(d)
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn the_same_picture_shared_twice_is_one_file() {
        // The reason the bytes name the file. A conversation has no slot to replace — every
        // image is another image — so something has to name them, and the only thing that can
        // name them without inventing anything is what they contain.
        let d = Dir::new("shared-same");
        let first = keep_shared_image(&d.0, "screenshot.png", PNG).expect("kept");
        let again = keep_shared_image(&d.0, "pasted again.png", PNG).expect("kept");

        assert_eq!(first.file, again.file, "one picture, one file");
        // And each keeps the name its own user typed, because that is only ever shown back.
        assert_eq!(first.name, "screenshot.png");
        assert_eq!(again.name, "pasted again.png");

        let kept: Vec<_> = std::fs::read_dir(shared_images(&d.0))
            .expect("folder")
            .flatten()
            .collect();
        assert_eq!(kept.len(), 1);
    }

    #[test]
    fn a_shared_image_is_named_from_its_bytes_and_never_from_its_name() {
        // ADR-0024's rule, at the one door where the caller most obviously *could* have supplied
        // a name. There is no path to traverse and no extension to spoof because neither is read.
        let d = Dir::new("shared-naming");
        let kept = keep_shared_image(&d.0, "../../etc/passwd.svg", PNG).expect("kept");

        assert!(
            kept.file.ends_with(".png"),
            "sniffed, not claimed: {}",
            kept.file
        );
        assert!(
            !kept.file.contains(std::path::is_separator),
            "no path survives: {}",
            kept.file
        );
        assert!(shared_images(&d.0).join(&kept.file).is_file());
    }

    #[test]
    fn a_png_is_accepted_and_named_by_the_engine() {
        let d = Dir::new("png");
        let name = accept_image(&d.0, "preview", PNG).unwrap();
        assert_eq!(name, "preview.png");
        assert!(d.0.join("preview.png").is_file());
    }

    #[test]
    fn a_data_uri_is_accepted_as_the_browser_produces_it() {
        let d = Dir::new("datauri");
        let name = accept_image(&d.0, "preview", &format!("data:image/png;base64,{PNG}")).unwrap();
        assert_eq!(name, "preview.png");
    }

    #[test]
    fn an_svg_is_accepted() {
        let d = Dir::new("svg");
        let name = accept_image(
            &d.0,
            "preview",
            &b64(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
        )
        .unwrap();
        assert_eq!(name, "preview.svg");
    }

    #[test]
    fn the_format_comes_from_the_bytes_and_never_from_the_name() {
        // A renamed executable is a thing that exists. The magic bytes are not.
        let d = Dir::new("liar");
        let err =
            accept_image(&d.0, "preview", &b64(b"MZ\x90\x00this is a windows binary")).unwrap_err();
        assert!(matches!(err, ImportError::UnsupportedFormat));
        assert!(!d.0.join("preview.png").exists(), "nothing may be written");
    }

    #[test]
    fn a_png_is_measured_from_its_header_without_decoding_it() {
        // IHDR is the first chunk by the format's own guarantee, so this is a fact about PNG
        // rather than a convention that could change.
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&13u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&1920u32.to_be_bytes());
        png.extend_from_slice(&1080u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);

        assert_eq!(ImageFormat::Png.measure(&png), Some((1920, 1080)));
    }

    #[test]
    fn a_truncated_header_measures_to_nothing_rather_than_to_a_guess() {
        // A wrong size silently rescales somebody's whole World, so there is no fallback here.
        // `None` means "the World keeps the extent it had", which is always safe.
        let stub = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        assert_eq!(ImageFormat::Png.measure(&stub), None);
    }

    #[test]
    fn vector_art_is_measured_by_the_space_it_was_drawn_in() {
        // It has no pixels, so `viewBox` is the honest answer — `width`/`height` may be in
        // units this does not understand.
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 800 600"></svg>"#;
        assert_eq!(ImageFormat::Svg.measure(svg), Some((800, 600)));

        let no_box = br#"<svg xmlns="http://www.w3.org/2000/svg" width="4cm"></svg>"#;
        assert_eq!(ImageFormat::Svg.measure(no_box), None);
    }

    #[test]
    fn replacing_an_import_leaves_no_earlier_format_behind() {
        // Otherwise the fallback chain would find a stale PNG after the user chose an SVG.
        let d = Dir::new("replace");
        accept_image(&d.0, "preview", PNG).unwrap();
        assert!(d.0.join("preview.png").is_file());

        let name = accept_image(
            &d.0,
            "preview",
            &b64(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
        )
        .unwrap();
        assert_eq!(name, "preview.svg");
        assert!(
            !d.0.join("preview.png").exists(),
            "the old one must be gone"
        );
    }

    #[test]
    fn oversized_and_empty_imports_are_refused() {
        let d = Dir::new("size");
        assert!(matches!(
            accept_image(&d.0, "preview", "").unwrap_err(),
            ImportError::Empty
        ));

        let huge = b64(&vec![0u8; MAX_BYTES + 1]);
        assert!(matches!(
            accept_image(&d.0, "preview", &huge).unwrap_err(),
            ImportError::TooLarge { .. }
        ));
    }

    /// A picture Epoch made is not an import, and no import limit is its limit.
    ///
    /// **Two caps died here, and the second is the interesting one.** `MAX_BYTES` went first —
    /// a 1024 render through a 4× upscaler is 4096 square and weighs 29 MB, so the first
    /// upscaled picture this product ever made was drawn on the card and thrown away by a limit
    /// whose stated reason is about assets riding a projection.
    ///
    /// Its replacement was derived from what the panel could ask for *that day*: 2048² through
    /// 4×. Then the panel gained the sizes people name out loud, and 4K UHD through 4× is
    /// 15360×8640 — 136,951,962 bytes, measured, refused, and still sitting in ComfyUI's output
    /// folder afterwards. **Which is the whole argument:** by the time this is called the picture
    /// exists on disk, so refusing it saves nothing and loses only Epoch's copy.
    #[test]
    fn what_epoch_drew_is_kept_however_big_it_is() {
        let dir = std::env::temp_dir().join(format!("epoch-made-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let png = base64::engine::general_purpose::STANDARD
            .decode(PNG)
            .unwrap();
        let mut big = png.clone();
        big.resize(MAX_BYTES + 1, 0);
        assert!(
            keep_bytes(&dir, &big).is_ok(),
            "an import limit must not decide what Epoch may keep of its own render"
        );

        // The size that was actually refused, to the byte. A number chosen to be comfortably
        // over a cap is a test about arithmetic; this one is about a picture that existed.
        let mut real = png.clone();
        real.resize(136_951_962, 0);
        assert!(
            keep_bytes(&dir, &real).is_ok(),
            "4K UHD through a 4x upscaler is a thing this panel can ask for"
        );

        // Still not a hole: bytes that are not a picture are refused as firmly as ever, and
        // that is a question about content rather than about size.
        assert!(matches!(
            keep_bytes(&dir, &[0u8; 64]),
            Err(ImportError::UnsupportedFormat)
        ));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A picture belongs where the person is working, and the record belongs to Epoch.
    #[test]
    fn a_picture_is_kept_in_the_vault_and_beside_the_work() {
        let dir = std::env::temp_dir().join(format!("epoch-beside-{}", std::process::id()));
        let root = dir.join("their-project");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&root).unwrap();

        let png = base64::engine::general_purpose::STANDARD
            .decode(PNG)
            .unwrap();
        let kept = keep_made(&dir, Some(&root), &png).expect("kept");

        // The record: the folder the Chronicle renders from, reached by filename and never by
        // path (ADR-0024).
        assert!(shared_images(&dir).join(&kept.file).exists());

        // The work: a folder inside their project, and the same name, so the two are visibly
        // one file rather than two pictures that happen to look alike.
        let at = kept.beside_the_work.expect("it went where the work is");
        assert_eq!(at, root.join("creations").join(&kept.file));
        assert_eq!(std::fs::read(&at).unwrap(), png, "byte for byte");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_world_with_nowhere_to_work_still_draws() {
        // A Project Root is not required to make a picture, and a World without one is not a
        // degraded World — it simply has no second place to put it.
        let dir = std::env::temp_dir().join(format!("epoch-nowhere-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let png = base64::engine::general_purpose::STANDARD
            .decode(PNG)
            .unwrap();
        let kept = keep_made(&dir, None, &png).expect("kept");
        assert!(shared_images(&dir).join(&kept.file).exists());
        assert_eq!(kept.beside_the_work, None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_project_that_cannot_be_written_to_does_not_lose_the_picture() {
        // The card already spent the seconds. A read-only tree, a full disk or a path that
        // stopped existing must not throw away what it made — the vault copy is what everything
        // downstream reads, and the other is reported when it happened.
        let dir = std::env::temp_dir().join(format!("epoch-noroot-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let png = base64::engine::general_purpose::STANDARD
            .decode(PNG)
            .unwrap();
        // A file where the folder should be: `create_dir_all` cannot make `pictures` under it.
        let root = dir.join("not-a-folder");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&root, b"i am a file").unwrap();

        let kept = keep_made(&dir, Some(&root), &png).expect("the picture survives");
        assert!(shared_images(&dir).join(&kept.file).exists());
        assert_eq!(
            kept.beside_the_work, None,
            "reported as not written, not as drawn"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rubbish_that_is_not_base64_is_refused_rather_than_panicking() {
        let d = Dir::new("garbage");
        assert!(matches!(
            accept_image(&d.0, "preview", "!!!! not base64 !!!!").unwrap_err(),
            ImportError::NotDecodable
        ));
    }

    #[test]
    fn forgetting_removes_every_format_and_tolerates_absence() {
        let d = Dir::new("forget");
        accept_image(&d.0, "preview", PNG).unwrap();
        forget_image(&d.0, "preview");
        assert!(!d.0.join("preview.png").exists());
        // Idempotent: forgetting nothing is not an error.
        forget_image(&d.0, "preview");
    }

    /// A real 7x3 JPEG, produced by an encoder rather than typed from memory — its segment
    /// chain carries a JFIF header and two quantisation tables before the frame, which is the
    /// whole reason `measure` walks the chain instead of reading a fixed offset.
    const JPEG: &str = "/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAAgGBgcGBQgHBwcJCQgKDBQNDAsLDBkSEw8UHRofHh0aHBwgJC4nICIsIxwcKDcpLDAxNDQ0Hyc5PTgyPC4zNDL/2wBDAQkJCQwLDBgNDRgyIRwhMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjIyMjL/wAARCAADAAcDASIAAhEBAxEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFhcYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oADAMBAAIRAxEAPwDxGiiitjI//9k=";

    #[test]
    fn a_jpeg_is_recognised_and_measured_through_its_segment_chain() {
        let bytes = decode(JPEG).expect("real JPEG bytes");
        assert_eq!(ImageFormat::sniff(&bytes), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::Jpeg.measure(&bytes), Some((7, 3)));

        // And it survives the whole door, landing under the name its bytes chose.
        let d = Dir::new("jpeg");
        let kept = keep_shared_image(&d.0, "photo.JPEG", JPEG).expect("kept");
        assert!(kept.file.ends_with(".jpg"), "named {}", kept.file);
        assert!(shared_images(&d.0).join(&kept.file).exists());
    }

    #[test]
    fn a_truncated_jpeg_is_measured_as_unknown_rather_than_guessed() {
        // Header only, no frame. `None` is a real answer; a size read out of compressed data
        // would silently rescale somebody's artwork.
        let bytes = decode(JPEG).expect("bytes");
        assert_eq!(ImageFormat::Jpeg.measure(&bytes[..20]), None);
    }

    #[test]
    fn every_accepted_format_is_one_the_asset_pipeline_can_deliver() {
        // The two lists must not drift, or Epoch would store artwork it cannot draw.
        let d = Dir::new("agree");
        for (payload, ext) in [
            (b64(PNG.as_bytes()), "png"),
            (b64(b"<svg xmlns='http://www.w3.org/2000/svg'/>"), "svg"),
        ] {
            let _ = (payload, ext);
        }
        for format in ImageFormat::ALL {
            let name = format!("x.{}", format.extension());
            std::fs::create_dir_all(&d.0).unwrap();
            std::fs::write(d.0.join(&name), b"placeholder").unwrap();
            assert!(
                crate::asset::resolve(&d.0, &name).is_ok(),
                "the asset pipeline cannot deliver .{}",
                format.extension()
            );
        }
    }
}
