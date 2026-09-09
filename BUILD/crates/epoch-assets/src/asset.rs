//! What a file **is**, read from the file (ADR-0032, Phase 11.2).
//!
//! ## A filename is a claim; the bytes are a fact
//!
//! `pixel_art_final_v3.safetensors` says nothing that can be relied on. It might be a LoRA, a
//! checkpoint, or a VAE somebody renamed. It might say SDXL and be Flux. ADR-0024 established the
//! rule one subsystem over — *the Engine names the file, from the bytes, never from the uploaded
//! name* — and it holds harder here, because filing a FLUX LoRA as SDXL does not fail at the door.
//! It fails four minutes into a render, inside a Quest, as `mat1 and mat2 shapes cannot be
//! multiplied`.
//!
//! ## Unknown stays unknown
//!
//! Every rule below can decline. There is no fallback that picks the likeliest answer, because a
//! likely answer is indistinguishable from a measured one once it is stored, and the whole point
//! of reading the bytes is to stop guessing. An unknown asset is still perfectly usable — it is
//! shown, it is offered, and what is *said about it* is that nobody measured it.
//!
//! ## The header is enough, and it is cheap
//!
//! A `.safetensors` file begins with eight bytes of little-endian length followed by that many
//! bytes of JSON: every tensor's name and shape, plus whatever the author put in `__metadata__`.
//! So a 6.9 GB checkpoint is understood by reading a few hundred kilobytes, and a whole shelf can
//! be read in the time it takes to open a panel.
//!
//! **A hash is not.** Hashing 6.9 GB takes real time, and it is only needed to *ask somebody else*
//! what a file is (Phase 11.4, Civitai's lookup by hash). So [`fingerprint`] is a separate call
//! that the caller decides to pay for; [`understand`] never hashes.
//!
//! ## What cannot be read says so
//!
//! `.ckpt`, `.pt` and `.bin` are Python pickles, and reading one means executing it. Epoch does
//! not, so those are reported as unread with the reason — which is information, not a failure.

use std::io::Read as _;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// What kind of thing a file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A full model: it can draw on its own.
    Checkpoint,
    /// The diffusion half on its own — no text encoder, no VAE.
    ///
    /// **Not a checkpoint, and the distinction is load-bearing.** Flux and Z-Image are normally
    /// distributed this way, and putting a `CheckpointLoaderSimple` in front of one produces a
    /// refusal nobody can act on. It decides which recipe the compiler writes.
    DiffusionModel,
    /// A text encoder — `t5xxl`, `clip_l`. Loaded beside a diffusion model, never alone.
    TextEncoder,
    /// An adapter: it changes how a checkpoint draws and cannot draw alone.
    Lora,
    Vae,
    ControlNet,
    Embedding,
    Upscaler,
    /// A Piper voice — an `.onnx` with an `.onnx.json` beside it.
    ///
    /// **Nothing here draws.** It is on this enum anyway because the alternative is a second
    /// `Kind`, a second `Shelf`, a second catalogue and a second install path — the three
    /// searchers ADR-0031 exists to prevent, arriving in a medium instead of a site. What a
    /// voice shares with a LoRA is the only thing a shelf cares about: it is a large file that
    /// arrives from a catalogue, is filed by what it is, and is consumed by something else.
    Voice,
    /// Read, and it matched nothing known. Not an error.
    Unknown,
}

impl Kind {
    /// The words a person uses, for a refusal that has to be readable.
    pub const fn plainly(self) -> &'static str {
        match self {
            Kind::Checkpoint => "a model",
            Kind::DiffusionModel => "a diffusion model",
            Kind::TextEncoder => "a text encoder",
            Kind::Lora => "a LoRA",
            Kind::Vae => "a VAE",
            Kind::ControlNet => "a ControlNet",
            Kind::Embedding => "an embedding",
            Kind::Upscaler => "an upscaler",
            Kind::Voice => "a voice",
            Kind::Unknown => "something Epoch could not identify",
        }
    }
}

/// Which family of model a file belongs to.
///
/// This is what decides compatibility, and it is why the panel can grey a LoRA rather than let it
/// fail mid-render. A closed set, deliberately: an open one would tempt somebody to store the
/// string a file claimed and treat it as measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Base {
    Sd15,
    Sd2,
    Sdxl,
    Sd3,
    Flux,
    /// Z-Image, and the Lumina 2 architecture it is built on.
    ///
    /// **Added because a family Epoch cannot read is a panel that cannot help.** The signature is
    /// ComfyUI's own (`comfy/model_detection.py`): `cap_embedder.1.weight` beside
    /// `noise_refiner.0.attention.k_norm.weight` is Lumina 2, and the first dimension of that
    /// weight separates the two — 2304 is the original Lumina 2, 3840 is Z-Image. Measured on the
    /// owner's file: `[3840, 2560]`.
    ZImage,
    /// LTX-Video. **The first family here that makes something that moves.**
    ///
    /// The signature is ComfyUI's own (`comfy/model_detection.py`): `patchify_proj.weight` on a
    /// transformer with `adaln_single` and `transformer_blocks.N.scale_shift_table` is LTXV, and
    /// nothing else here has a patchify projection. Measured on the owner's file:
    /// `model.diffusion_model.patchify_proj.weight` at `[2048, 128]`, beside 28 blocks.
    Ltxv,
    /// Stable Audio Open. **The first family here that makes something you listen to.**
    ///
    /// `conditioner.conditioners.seconds_start.` — a conditioner that embeds a *duration*, which
    /// is ComfyUI's own key and a thing no family that draws has any use for. Read from the
    /// owner's file (2026-08-28) beside a `pretransform` and a `project_in`.
    StableAudio,
    /// ACE-Step. Music, and long enough for a song.
    ///
    /// `lyric_embs` beside a `genre_embedder` on the diffusion model — read from the owner's file
    /// (2026-08-28). Nothing that draws has a lyric embedding, and nothing else that makes sound
    /// here has one either: Stable Audio is told a description, and this one is told a genre and
    /// a vocal separately.
    AceStep,
    /// ACE-Step 1.5. The same idea, rebuilt around a language model.
    ///
    /// **A different family rather than a newer file**, and the graph says so: it wants
    /// `TextEncodeAceStepAudio1.5` and `EmptyAceStep1.5LatentAudio`, which are separate node
    /// classes from v1's — read from this ComfyUI's own `comfy_extras/nodes_ace.py`, not
    /// remembered.
    ///
    /// ComfyUI's own key is the text encoder: `ACEStep15.clip_target` looks for
    /// `text_encoders.qwen3_2b.transformer.` or `text_encoders.qwen3_4b.transformer.`, so this
    /// family is told apart by carrying a **Qwen3** where v1 carries a lyric embedding. Read
    /// from the owner's file (2026-08-29): `text_encoders.qwen3_2b` beside
    /// `text_encoders.qwen3_06b`, 1664 tensors, and no `lyric_embs` anywhere.
    AceStep15,
    /// Hunyuan3D 2. A shape rather than a picture.
    ///
    /// `vae.geo_decoder.` — a decoder for *geometry*, which nothing that draws has. Read from the
    /// owner's file (2026-08-29), and it is checked **before Flux**: this model's transformer is
    /// a DiT with `double_blocks.` and `single_blocks.`, the same names Flux uses, so the rule
    /// that was exact for one family stopped being exact the day a second one borrowed its
    /// architecture. Measured as `flux` until this existed, which put a 3D model on the picture
    /// tab and nothing at all on the 3D one.
    Hunyuan3d,
    /// Nobody measured it. Shown as unmeasured, never as incompatible.
    Unknown,
}

/// What a family makes.
///
/// **A third medium is why this is not `moves() -> Option<bool>` any more.** Two answers plus
/// *unknown* worked while there were two media; sound is a third, and a boolean asked to carry
/// three states is the field somebody reads the wrong way round.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Makes {
    Picture,
    Video,
    Sound,
    Model,
}

impl Base {
    pub const fn plainly(self) -> &'static str {
        match self {
            Base::Sd15 => "SD 1.5",
            Base::Sd2 => "SD 2",
            Base::Sdxl => "SDXL",
            Base::Sd3 => "SD 3",
            Base::Flux => "Flux",
            Base::ZImage => "Z-Image",
            Base::Ltxv => "LTXV",
            Base::StableAudio => "Stable Audio",
            Base::AceStep => "ACE-Step",
            Base::AceStep15 => "ACE-Step 1.5",
            Base::Hunyuan3d => "Hunyuan3D",
            Base::Unknown => "unknown",
        }
    }

    /// Which medium this family makes.
    ///
    /// **`None` is the answer that matters.** A family Epoch has measured belongs to one medium;
    /// a family it has *not* measured belongs to none of them, and `None` says so — such a model
    /// is offered under **every** tab. A surface that treated unmeasured as *not this one* would
    /// hide a file the user owns because Epoch could not read it, which is the worst outcome
    /// available: nothing they can do about it, and nothing telling them why.
    pub const fn makes(self) -> Option<Makes> {
        match self {
            Base::Ltxv => Some(Makes::Video),
            Base::StableAudio | Base::AceStep | Base::AceStep15 => Some(Makes::Sound),
            Base::Hunyuan3d => Some(Makes::Model),
            Base::Sd15 | Base::Sd2 | Base::Sdxl | Base::Sd3 | Base::Flux | Base::ZImage => {
                Some(Makes::Picture)
            }
            Base::Unknown => None,
        }
    }

    /// Whether an adapter of this base can be used with a model of that one.
    ///
    /// **Unknown is never an incompatibility.** It is the absence of a measurement, and refusing
    /// on the strength of something nobody measured is the same failure as a gauge nobody can
    /// explain.
    pub const fn works_with(self, model: Base) -> bool {
        matches!(self, Base::Unknown)
            || matches!(model, Base::Unknown)
            || matches!((self, model), (a, b) if a as u8 == b as u8)
    }
}

/// Everything reading a file's own bytes established about it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Understood {
    pub kind: Kind,
    pub base: Base,
    pub bytes: u64,
    /// Words the author said this needs in a prompt, when the file carries them. Most do not —
    /// the catalogue is the better source (Phase 11.4), and this is what can be known offline.
    pub triggers: Vec<String>,
    /// What the file *claimed* about itself, verbatim. Kept even when it was understood, so an
    /// unknown base is still something a person can look at and recognise.
    pub claimed: Option<String>,
    /// Why nothing could be derived, when nothing could. `None` means the bytes were read.
    pub unread: Option<&'static str>,
    /// **What a text encoder is**, when the file is one. `None` for everything else.
    ///
    /// Separate from [`Base`] on purpose, and it is the whole reason this field exists: an
    /// encoder belongs to no diffusion family. `clip_l.safetensors` is byte-for-byte the same
    /// file beside Flux and beside SDXL, which is exactly why asking for its *family* comes back
    /// `Unknown` — correctly — and asking what it *is* comes back exact.
    pub encoder: Option<Encoder>,
    /// Whether this checkpoint **carries its own text encoder**, when it is one.
    ///
    /// ## Why this is measured rather than assumed from the family
    ///
    /// The panel asked for an encoder beside every non-picture checkpoint, on a rule generalised
    /// from the two it had seen: LTXV and Stable Audio both borrow one. ACE-Step does not — it
    /// is all-in-one — and GENERATE sat grey waiting for a file that model has no use for.
    /// Measured in the window, 2026-08-28.
    ///
    /// The three prefixes are read from the files themselves: `conditioner.embedders.` (SDXL),
    /// `cond_stage_model.` (SD 1.5 and 2) and `text_encoders.` (ACE-Step). **Stable Audio is why
    /// the first is `embedders` and not `conditioner.`** — it has
    /// `conditioner.conditioners.seconds_start.`, which is a conditioner that embeds a duration
    /// and not a text encoder at all. A prefix match one segment shorter would have called it
    /// self-contained and left the panel refusing to ask for the T5 it genuinely needs.
    ///
    /// `false` on anything that is not a checkpoint, where the question does not arise.
    pub carries_encoder: bool,
    /// **Which medium a VAE belongs to**, when the file is one. `None` for everything else.
    ///
    /// The same argument as [`Understood::encoder`], for the other part that has no family. A
    /// VAE's *family* is unanswerable — measured on this machine, all four on the shelf came
    /// back `Unknown`, and the one that did claim a family
    /// (`z_image_ae.safetensors`, which says `Flux.1-AE`) was claiming the wrong one. So the
    /// panel offered an **audio** autoencoder to somebody drawing a picture with Flux.
    ///
    /// What *is* answerable is what it decodes into, because that is a property of the medium
    /// rather than of the model. See [`medium_of`].
    pub medium: Option<Makes>,
}

/// A text encoder, read from its own tensors.
///
/// ## Why not a family
///
/// Measured 2026-08-25 against the files on this machine: every text encoder and every VAE came
/// back `Base::Unknown`, so the Studio Panel could not tell a person which parts went with the
/// model they had chosen. Greying by family would have greyed nothing.
///
/// This is the answer that *is* in the bytes. A Flux panel says it wants a CLIP-L and a T5-XXL,
/// and the rows now say which file is which — the two halves meet without Epoch ever claiming a
/// file belongs to a family it cannot see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Encoder {
    /// OpenAI's CLIP text tower, at the width the file reports.
    ///
    /// Measured: `clip_l.safetensors` is `[49408, 768]` at
    /// `text_model.embeddings.token_embedding.weight`. 768 is L, 1280 is G, 1024 is H.
    ClipL,
    ClipG,
    ClipH,
    /// A CLIP at some other width. Still a CLIP, and saying so beats saying nothing.
    Clip,
    /// A T5 encoder. Measured: `t5xxl_fp8_e4m3fn.safetensors` carries `shared.weight`
    /// `[32128, 4096]`; 4096 is the XXL.
    T5Xxl,
    T5,
    /// A language model used as a text encoder — Qwen, Gemma, Llama. Measured:
    /// `qwen_3_4b_fp8_mixed.safetensors` is `model.embed_tokens.weight` `[151936, 2560]`, which
    /// is a decoder's vocabulary, not CLIP's 49408.
    Language,
}

impl Encoder {
    /// The words a person reads on the row, so it can be matched against what the family wants.
    pub const fn plainly(self) -> &'static str {
        match self {
            Encoder::ClipL => "CLIP-L",
            Encoder::ClipG => "CLIP-G",
            Encoder::ClipH => "CLIP-H",
            Encoder::Clip => "a CLIP",
            Encoder::T5Xxl => "T5-XXL",
            Encoder::T5 => "a T5",
            Encoder::Language => "a language model",
        }
    }
}

impl Understood {
    /// A file that exists and told us nothing.
    fn opaque(bytes: u64, why: &'static str) -> Self {
        Self {
            kind: Kind::Unknown,
            base: Base::Unknown,
            bytes,
            triggers: Vec::new(),
            claimed: None,
            unread: Some(why),
            encoder: None,
            carries_encoder: false,
            medium: None,
        }
    }
}

impl Understood {
    /// Which medium this file belongs to, from whichever of the two readings answered.
    ///
    /// **One place, because three surfaces ask it.** The panel, the Creations deck and
    /// EpochServices all group by medium, and a rule written out three times is three rules that
    /// agree until one of them is edited.
    ///
    /// The order is the same one `assembly_for` uses and it is load-bearing: a **family** read
    /// out of a model's own tensors is the stronger measurement, and a part's own shape answers
    /// only where there is no family to have. `z_image_ae.safetensors` is the case that shows
    /// both — it claims `Flux.1-AE`, so its family reads Flux, and its convolutions independently
    /// say *a picture*. They agree. Where they could not, the family is the one that was measured
    /// against a whole model rather than against one part of one.
    ///
    /// `None` is **unplaced**, never *not this one*. Every surface must offer such a file under
    /// every medium.
    pub fn makes(&self) -> Option<Makes> {
        self.base.makes().or(self.medium)
    }
}

/// Why a path could not be looked at at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    Missing(String),
    Refused(String),
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unreadable::Missing(path) => write!(out, "there is no file at {path}"),
            Unreadable::Refused(why) => write!(out, "it could not be read: {why}"),
        }
    }
}

/// Read a file and say what it is.
///
/// Never hashes, never loads weights, never executes anything.
pub fn understand(path: &Path) -> Result<Understood, Unreadable> {
    let meta = std::fs::metadata(path).map_err(|why| match why.kind() {
        std::io::ErrorKind::NotFound => Unreadable::Missing(path.display().to_string()),
        _ => Unreadable::Refused(why.to_string()),
    })?;
    if !meta.is_file() {
        return Err(Unreadable::Missing(path.display().to_string()));
    }
    let bytes = meta.len();

    let extension = path
        .extension()
        .and_then(|it| it.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "safetensors" | "sft" => match read_header(path) {
            Ok(header) => Ok(from_header(&header, bytes)),
            // A safetensors file that will not parse is a fact worth stating rather than an
            // error: it is on the shelf, it is theirs, and Epoch simply cannot say what it is.
            Err(why) => Ok(Understood::opaque(bytes, why)),
        },
        "ckpt" | "pt" | "pth" | "bin" => Ok(Understood::opaque(
            bytes,
            "this format is a Python pickle, and reading one means running it. Epoch does not.",
        )),
        "gguf" => Ok(Understood::opaque(
            bytes,
            "GGUF is a format for language models; nothing here draws with one.",
        )),
        _ => Ok(Understood::opaque(
            bytes,
            "Epoch does not read this format.",
        )),
    }
}

/// The SHA-256 of a file, lowercase hex — what a catalogue is asked with.
///
/// Streamed, so a 7 GB checkpoint costs time and not memory. Separate from [`understand`] because
/// it is the expensive half and only the catalogue needs it.
pub fn fingerprint(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest as _, Sha256};
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

/// The safetensors header: eight bytes of length, then that much JSON.
fn read_header(path: &Path) -> Result<serde_json::Value, &'static str> {
    let mut file = std::fs::File::open(path).map_err(|_| "it could not be opened.")?;
    let mut length = [0u8; 8];
    file.read_exact(&mut length)
        .map_err(|_| "it is too small to be a safetensors file.")?;
    let length = u64::from_le_bytes(length);
    // A real header is tens of kilobytes. The bound exists so a corrupt or hostile file cannot
    // ask Epoch to allocate its own size before anything has been checked.
    if length == 0 || length > 64 * 1024 * 1024 {
        return Err("its header is not a size a safetensors header can be.");
    }
    let mut header = vec![0u8; length as usize];
    file.read_exact(&mut header)
        .map_err(|_| "its header is shorter than it says it is.")?;
    serde_json::from_slice(&header).map_err(|_| "its header is not readable JSON.")
}

/// Everything the classification does, over a parsed header.
///
/// Separated so every rule below is testable without a seven-gigabyte file to hand.
pub fn from_header(header: &serde_json::Value, bytes: u64) -> Understood {
    let names: Vec<&str> = header
        .as_object()
        .map(|it| {
            it.keys()
                .map(String::as_str)
                .filter(|k| *k != "__metadata__")
                .collect()
        })
        .unwrap_or_default();
    let said = header.get("__metadata__");

    let claimed = ["modelspec.architecture", "ss_base_model_version"]
        .iter()
        .find_map(|key| said?.get(key)?.as_str())
        .map(str::to_owned);

    let triggers = said
        .and_then(|it| it.get("modelspec.trigger_phrase"))
        .and_then(|it| it.as_str())
        .map(|phrase| {
            phrase
                .split(',')
                .map(str::trim)
                .filter(|word| !word.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    let kind = kind_of(&names, header);
    Understood {
        base: claimed
            .as_deref()
            .and_then(base_from_claim)
            .unwrap_or_else(|| base_from_names(&names, header)),
        // Only asked of a file that is one. Reading a checkpoint's built-in CLIP as if it were a
        // standalone encoder would put a description on the wrong row.
        encoder: (kind == Kind::TextEncoder)
            .then(|| encoder_of(header))
            .flatten(),
        // **Asked of the bytes, not of the classification.** It was gated on
        // `kind == Kind::Checkpoint`, and `kind_of` did not know ACE-Step was one — so a model
        // that carries its own encoder reported that it does not, and the panel held GENERATE
        // grey waiting for a file it has no use for.
        //
        // A gate whose condition is itself a measurement in progress makes the answer wrong
        // twice. The only thing excluded is a file that *is* an encoder, which is read exactly.
        carries_encoder: kind != Kind::TextEncoder && encoder_inside(&names),
        // Only asked of a file that is one, for the same reason `encoder` is: a checkpoint
        // carries a VAE inside it and its own family already answers which medium it makes.
        medium: (kind == Kind::Vae).then(|| medium_of(header)).flatten(),
        kind,
        bytes,
        triggers,
        claimed,
        unread: None,
    }
}

/// Which text encoder this is, from the width of its own embedding table.
///
/// **The name is not asked.** `clip_l.safetensors` is a claim; `[49408, 768]` is a fact, and the
/// same file renamed is still 768 wide (ADR-0024's rule, one shelf over).
/// Which medium an autoencoder belongs to, read from the shape of its convolutions.
///
/// ## Why a shape and not a name
///
/// A VAE has no family to read. `clip_l.safetensors` is byte-for-byte the same file beside Flux
/// and beside SDXL, and a VAE is the same kind of part — measured on this machine, all four on
/// the shelf answered `Base::Unknown`, and the only one that *claimed* a family claimed the
/// wrong one (`z_image_ae.safetensors` says `Flux.1-AE`). Reading the file name is forbidden
/// (ADR-0024) and would have been the only other option.
///
/// So the question is changed from *which model does this belong to* — unanswerable — to **what
/// does it decode into**, which is a property of the medium and is written into every tensor:
///
/// | medium | convolutions | because |
/// |---|---|---|
/// | a picture | rank **4** — `[out, in, H, W]` | two spatial dimensions |
/// | a video | rank **5** — `[out, in, T, H, W]` | and time |
/// | a sound | rank **3** — `[out, in, L]` | one dimension, and no space |
///
/// Measured 2026-08-30 across six files from five families, none of which shares an author:
/// `flux-vae-bf16` and `z_image_ae` and SDXL's `first_stage_model.` are rank 4; MiniMax-H3's
/// video VAE and LTXV's `vae.` are rank 5; MiniMax-H3's audio VAE and Stable Audio's
/// `pretransform.` are rank 3 with no rank 4 at all.
///
/// ## The one that is not what it looks like
///
/// ACE-Step decodes **a spectrogram**, so its autoencoder is 2-D like a picture's — and its
/// `decoder.conv_out.weight` emits **2** channels rather than 3. That is the second reading and
/// the reason the rule is not rank alone: an image decoder must end in three channels, because
/// that is what a colour is, and a stereo spectrogram ends in two.
///
/// `None` wherever the file says none of this. **Unknown stays unknown** — a VAE Epoch cannot
/// place is offered under every medium rather than hidden from the one it belongs to, which is
/// the worst thing a filter can do with somebody's own file.
/// Whether anything in this file is a convolution.
///
/// Rank 3 and above: `[out, in, L]` for a waveform, `[out, in, H, W]` for a picture, one more for
/// time. A transformer has none — every weight it holds is a matrix or a bias.
fn convolves(header: &serde_json::Value) -> bool {
    header.as_object().is_some_and(|it| {
        it.iter().any(|(name, value)| {
            name != "__metadata__"
                && value
                    .get("shape")
                    .and_then(|it| it.as_array())
                    .is_some_and(|shape| shape.len() >= 3)
        })
    })
}

fn medium_of(header: &serde_json::Value) -> Option<Makes> {
    let object = header.as_object()?;

    let mut ranks = [false; 6];
    // What the decoder finally emits. `ends_with` because the same tensor is
    // `decoder.conv_out.weight` standing alone and `first_stage_model.decoder.conv_out.weight`
    // inside a checkpoint — the prefix belongs to the container, never to the measurement.
    let mut emits: Option<u64> = None;

    for (name, value) in object {
        if name == "__metadata__" {
            continue;
        }
        let Some(shape) = value.get("shape").and_then(|it| it.as_array()) else {
            continue;
        };
        if let Some(slot) = ranks.get_mut(shape.len()) {
            *slot = true;
        }
        if name.ends_with("decoder.conv_out.weight") {
            emits = shape.first().and_then(serde_json::Value::as_u64);
        }
    }

    // Order is load-bearing, the way it already is in `base_from_names`: a video autoencoder
    // also holds rank-4 tensors, so asking about time first is what keeps it from reading as a
    // picture.
    if ranks[5] {
        return Some(Makes::Video);
    }
    match emits {
        // Red, green, blue. Nothing else a decoder can end in means a picture.
        Some(3) => return Some(Makes::Picture),
        // Mono or stereo, out of a spectrogram.
        Some(1 | 2) => return Some(Makes::Sound),
        _ => {}
    }
    // One dimension and no space at all: a waveform.
    if ranks[3] && !ranks[4] {
        return Some(Makes::Sound);
    }
    // Rank 4 with nothing saying what it emits. It is probably a picture and *probably* is not
    // a thing to act on when the consequence is hiding somebody's file from the tab it belongs
    // to.
    None
}

fn encoder_of(header: &serde_json::Value) -> Option<Encoder> {
    let width =
        |key: &str| -> Option<u64> { header.get(key)?.get("shape")?.as_array()?.get(1)?.as_u64() };

    // CLIP's text tower. Its vocabulary is 49408 and its width says which one.
    if let Some(wide) = width("text_model.embeddings.token_embedding.weight") {
        return Some(match wide {
            768 => Encoder::ClipL,
            1024 => Encoder::ClipH,
            1280 => Encoder::ClipG,
            _ => Encoder::Clip,
        });
    }
    // A T5. `shared.weight` is the encoder/decoder tied embedding, and 4096 is the XXL.
    if let Some(wide) = width("shared.weight").or_else(|| width("encoder.embed_tokens.weight")) {
        return Some(if wide >= 4096 {
            Encoder::T5Xxl
        } else {
            Encoder::T5
        });
    }
    // A decoder-only language model pressed into service as an encoder — Qwen, Gemma, Llama.
    // Its vocabulary is in the hundred thousands, which is what tells it from CLIP.
    width("model.embed_tokens.weight").map(|_| Encoder::Language)
}

/// Which tensors are present decides what a file is.
///
/// Ordered from the most specific signature to the least: a LoRA of a ControlNet holds both
/// families of name, and it is a LoRA.
fn kind_of(names: &[&str], header: &serde_json::Value) -> Kind {
    let any = |needle: &str| names.iter().any(|name| name.contains(needle));

    if any("lora_down.")
        || any("lora_up.")
        || any(".lora_A.")
        || any(".lora_B.")
        || any("lora_unet_")
    {
        return Kind::Lora;
    }
    if any("emb_params") || any("string_to_param") {
        return Kind::Embedding;
    }
    if any("control_model.") || any("input_hint_block") {
        return Kind::ControlNet;
    }
    // A checkpoint holds the diffusion model *and* the things around it. A bare diffusion model
    // (Flux's `double_blocks`, a distributed UNet) is not one, and calling it one is what puts a
    // `CheckpointLoaderSimple` in front of something that cannot answer it.
    //
    // **`vae.` is here because three families spell it that way** — measured on this machine:
    // LTXV, Stable Audio and ACE-Step all carry `model.diffusion_model.` beside a `vae.` and
    // none of them has a `first_stage_model.`. Without it they were not checkpoints, which is
    // wrong in the direction that matters: `Shelf::for_kind` sends an unknown to the checkpoint
    // shelf, so they *landed* correctly and everything that asked what they were got a different
    // answer from everything that asked where they lived.
    if any("model.diffusion_model.")
        && (any("first_stage_model.")
            || any("cond_stage_model.")
            || any("conditioner.")
            || any("vae."))
    {
        return Kind::Checkpoint;
    }
    // A standalone VAE: an encoder and a decoder, **something that convolves**, and nothing
    // that draws.
    //
    // **The convolution is why a T5 stopped being a VAE.** `t5_base.safetensors` is an
    // encoder-decoder transformer, so it carries both prefixes and matched this rule exactly —
    // measured on this machine, it was listed as *a VAE* on the Creations deck beside the real
    // ones. Nothing about the *names* separates them; the shapes do, completely. A T5's tensors
    // are rank 1 and rank 2 and nothing else, because it has no convolutions at all, while an
    // autoencoder for pixels or for sound is convolutions almost end to end.
    //
    // A rule about text models by name would need one entry per architecture. This is one
    // measurement that covers every text model there will ever be.
    if any("decoder.") && any("encoder.") && !any("model.diffusion_model.") && convolves(header) {
        return Kind::Vae;
    }
    // The diffusion half on its own. Every family names its blocks differently and none of them
    // carries a text encoder or a VAE, which is exactly what makes it *not* a checkpoint.
    if any("double_blocks.")
        || any("single_blocks.")
        || any("joint_blocks.")
        || any("model.diffusion_model.")
    {
        return Kind::DiffusionModel;
    }
    // A text encoder, by the families anything here would load: T5, CLIP, and a decoder-only
    // language model.
    //
    // **The third was missing and it is not exotic.** Measured on this machine:
    // `qwen_3_4b_fp8_mixed.safetensors` sits on ComfyUI's `text_encoders` shelf and is read by
    // `CLIPLoader`, and it is `model.embed_tokens.weight` + `model.layers.*` — neither of the two
    // shapes above. Epoch filed it as *something it could not identify*, so it appeared in the
    // panel with nothing said about it. Qwen-Image, HiDream and the whole newer half of the field
    // are loaded this way.
    if any("encoder.block.") || any("text_model.encoder.layers.") {
        return Kind::TextEncoder;
    }
    if any("model.embed_tokens.") && any("model.layers.") {
        return Kind::TextEncoder;
    }
    // **The diffusion half of a model whose blocks are simply `layers.N`.**
    //
    // Measured: `z_image_turbo_int8_convrot.safetensors` is 857 tensors of `layers.0…` and
    // nothing else — no `double_blocks`, no `model.diffusion_model.`, so every rule above declined
    // and it came back `Unknown`. That is not cosmetic: `Shelf::for_kind` sends `Unknown` to the
    // **checkpoints** shelf, which is where a `CheckpointLoaderSimple` gets put in front of
    // something that cannot answer one — the refusal this module's own header warns about.
    //
    // Deliberately last, and deliberately narrow: it fires only when nothing else matched, the
    // tensors are transformer blocks at the top level, and there is no text encoder or VAE among
    // them — which is what makes it a bare diffusion model rather than anything else.
    if any("layers.0.") && !any("text_model.") && !any("decoder.") && !any("encoder.") {
        return Kind::DiffusionModel;
    }
    Kind::Unknown
}

/// What the file said about itself, when it said something recognisable.
///
/// Two vocabularies, because two tools write them: `modelspec.architecture` is the community
/// standard, and `ss_base_model_version` is what kohya's trainer writes into every LoRA it makes.
fn base_from_claim(claim: &str) -> Option<Base> {
    let claim = claim.to_ascii_lowercase();
    // Ordered longest-first: `stable-diffusion-xl` also contains `stable-diffusion`.
    if claim.contains("flux") {
        return Some(Base::Flux);
    }
    if claim.contains("xl") {
        return Some(Base::Sdxl);
    }
    if claim.contains("-v3") || claim.contains("_v3") || claim.contains("sd3") {
        return Some(Base::Sd3);
    }
    if claim.contains("-v2") || claim.contains("_v2") || claim.contains("sd_v2") {
        return Some(Base::Sd2);
    }
    if claim.contains("-v1") || claim.contains("_v1") || claim.contains("sd_v1") {
        return Some(Base::Sd15);
    }
    None
}

/// Whether a checkpoint's own text encoder is inside it.
///
/// A named set of prefixes, each read from a file on this machine — never a guess from the
/// family, which is the rule that broke. See [`Understood::carries_encoder`].
fn encoder_inside(names: &[&str]) -> bool {
    const INSIDE: [&str; 3] = [
        // SDXL, SD 2.1: one or two CLIPs under the conditioner. **`embedders`, not
        // `conditioner.`** — Stable Audio has a conditioner that embeds a duration and no text
        // encoder at all.
        "conditioner.embedders.",
        // SD 1.5 and SD 2's own spelling.
        "cond_stage_model.",
        // ACE-Step's, and the one that made this a measurement rather than a rule.
        "text_encoders.",
    ];
    names
        .iter()
        .any(|name| INSIDE.iter().any(|prefix| name.contains(prefix)))
}

/// The family, from the tensors themselves — which is the answer no metadata can be wrong about.
///
/// Takes the whole header rather than only the names, because one family is told from its
/// neighbour by a **width**: `cap_embedder.1.weight` is 2304 wide in Lumina 2 and 3840 in
/// Z-Image. The same discipline `encoder_of` already follows — a name is a claim, a shape is a
/// fact (ADR-0024).
fn base_from_names(names: &[&str], header: &serde_json::Value) -> Base {
    let any = |needle: &str| names.iter().any(|name| name.contains(needle));

    // **Before Flux, because Hunyuan3D borrowed Flux's architecture.** Its transformer is a DiT
    // with `double_blocks.` and `single_blocks.` under exactly those names; what it does *not*
    // share is a decoder for geometry. Measured as `flux` until this line existed, which put a
    // 3D model on the picture tab and left the 3D one empty.
    //
    // The general shape is worth naming: a rule that was exact for one family stops being exact
    // the day a second one borrows its architecture, and the fix is a key about what the model
    // *does* rather than how its blocks are named.
    if any("vae.geo_decoder.") {
        return Base::Hunyuan3d;
    }
    if any("double_blocks.") || any("single_blocks.") || any("single_transformer_blocks.") {
        return Base::Flux;
    }
    // Lumina 2's arrangement, and Z-Image is built on it. ComfyUI's own two-key test, then its
    // own dimension test — a rule read from the program rather than remembered about it.
    if any("cap_embedder.") && any("noise_refiner.") {
        let wide = header
            .get("cap_embedder.1.weight")
            .and_then(|it| it.get("shape"))
            .and_then(|it| it.as_array())
            .and_then(|it| it.first())
            .and_then(serde_json::Value::as_u64);
        // Only Z-Image is claimed. Plain Lumina 2 shares every one of these tensors and Epoch has
        // never drawn with one, so it stays unmeasured rather than being filed by association.
        if wide == Some(3840) {
            return Base::ZImage;
        }
        return Base::Unknown;
    }
    // Stable Audio, and ComfyUI's own key for it: a conditioner that embeds a *duration*.
    // Checked before the text-encoder tests below for the same reason LTXV is — this checkpoint
    // has a `conditioner.` prefix of its own and would otherwise be read by a rule written about
    // somebody else's.
    if any("conditioner.conditioners.seconds_start.") {
        return Base::StableAudio;
    }
    // ACE-Step: a diffusion model with a **lyric** embedding beside a genre one. Checked before
    // everything below for the same reason as the other two — it carries a `text_encoders.`
    // prefix of its own and would otherwise be read by a rule written about somebody else's.
    if any("lyric_embs") && any("genre_embedder") {
        return Base::AceStep;
    }
    // ACE-Step 1.5, told apart by **ComfyUI's own key**: `ACEStep15.clip_target` looks for a
    // Qwen3 under `text_encoders.`, and v1 carries no language model at all. Read from
    // `supported_models.py` rather than remembered, and confirmed against the owner's file —
    // which carries `qwen3_2b` and `qwen3_06b` and none of v1's lyric embeddings.
    //
    // Checked here for the same reason as its neighbours: a `text_encoders.` prefix would
    // otherwise be read by a rule written about somebody else's.
    if any("text_encoders.qwen3_") {
        return Base::AceStep15;
    }
    // LTX-Video, and ComfyUI's own key for it. A patchify projection beside `adaln_single`
    // exists nowhere else on these shelves — and it is checked before the text-encoder tests
    // below, which an LTXV checkpoint would fail for a reason that says nothing about family:
    // it carries no text encoder at all.
    if any("patchify_proj.") && any("adaln_single.") {
        return Base::Ltxv;
    }
    if any("joint_blocks.") {
        return Base::Sd3;
    }
    // SDXL is the one with two text encoders — as a checkpoint (`embedders.1`) and as a LoRA
    // (`lora_te2_`). Nothing else in the family has a second one.
    if any("conditioner.embedders.1") || any("lora_te2_") {
        return Base::Sdxl;
    }
    // SD 2 swapped OpenAI's CLIP for OpenCLIP, and the tensor names changed with it:
    // `cond_stage_model.model.` where SD 1 has `cond_stage_model.transformer.`.
    if any("cond_stage_model.model.") {
        return Base::Sd2;
    }
    if any("cond_stage_model.transformer.") || any("lora_te_") || any("lora_te1_") {
        return Base::Sd15;
    }
    Base::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn header(names: &[&str]) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for name in names {
            map.insert((*name).to_owned(), json!({"dtype": "F16", "shape": [1]}));
        }
        serde_json::Value::Object(map)
    }

    /// What this machine actually holds — the measurement, rather than the rules.
    ///
    /// Every rule above was written from what these formats are documented to contain. This is
    /// the one that checks them against files somebody really downloaded, and it is how a rule
    /// that reads well and answers wrongly gets found.
    /// A header with real shapes, because the width is what names an encoder.
    fn shaped(entries: &[(&str, &[u64])]) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for (name, shape) in entries {
            map.insert(
                (*name).to_owned(),
                json!({"dtype": "F16", "shape": shape.to_vec()}),
            );
        }
        serde_json::Value::Object(map)
    }

    /// Every VAE on the owner's machine, by the shapes their headers actually carry.
    ///
    /// Read on 2026-08-30 with a script that printed each file's rank histogram and its
    /// `decoder.conv_out.weight`; the tensors named here are the ones that decided each answer,
    /// and the counts they came from are in `medium_of`'s own comment. Six files, five
    /// families, no shared author — which is what makes this a rule about the medium rather
    /// than a fingerprint of two files.
    #[test]
    fn a_vae_says_which_medium_it_decodes_into() {
        let picture = shaped(&[
            ("decoder.conv_in.weight", &[512, 16, 3, 3]),
            ("decoder.conv_out.weight", &[3, 128, 3, 3]),
            ("encoder.conv_out.weight", &[32, 512, 3, 3]),
        ]);
        assert_eq!(medium_of(&picture), Some(Makes::Picture), "flux-vae-bf16");

        // Inside a checkpoint the same tensor carries a prefix, and the prefix belongs to the
        // container rather than to the measurement.
        let inside = shaped(&[("first_stage_model.decoder.conv_out.weight", &[3, 128, 3, 3])]);
        assert_eq!(medium_of(&inside), Some(Makes::Picture), "SDXL's own VAE");

        // A video autoencoder holds rank-4 tensors too, so the order in `medium_of` is what
        // keeps this from reading as a picture. Both of these are real: MiniMax-H3's standalone
        // video VAE, and the `vae.` half of the LTXV checkpoint.
        let moving = shaped(&[
            ("encoder.conv_out.weight", &[48, 1024, 3, 3, 3]),
            ("decoder.proj_out.weight", &[3072, 2048]),
        ]);
        assert_eq!(medium_of(&moving), Some(Makes::Video), "minimax video VAE");

        // One dimension and no space at all.
        let waveform = shaped(&[
            ("dec_in_proj.weight", &[2048, 32, 1]),
            ("decoder.activation_post.upsample.filter", &[1, 1, 12]),
        ]);
        assert_eq!(
            medium_of(&waveform),
            Some(Makes::Sound),
            "minimax audio VAE"
        );

        // **And the one that is not what it looks like.** ACE-Step decodes a spectrogram, so
        // its autoencoder is 2-D exactly like a picture's — and ends in two channels rather
        // than three, because a stereo pair is not a colour.
        let spectrogram = shaped(&[
            ("vae.dcae.decoder.conv_out.weight", &[2, 128, 3, 3]),
            ("vae.dcae.encoder.conv_in.weight", &[128, 2, 3, 3]),
        ]);
        assert_eq!(medium_of(&spectrogram), Some(Makes::Sound), "ACE-Step");
    }

    /// **Unknown stays unknown**, and this is the assertion that keeps it that way.
    ///
    /// A VAE Epoch cannot place must be offered under every medium rather than hidden from the
    /// one it belongs to: there is nothing the user can do about a failure of Epoch's, and
    /// nothing telling them why. Guessing *picture* from rank 4 alone would be the same
    /// inversion as reading silence as no.
    #[test]
    fn a_vae_nobody_can_place_stays_unplaced() {
        assert_eq!(
            medium_of(&shaped(&[("something.weight", &[512, 512])])),
            None
        );
        assert_eq!(
            medium_of(&shaped(&[("decoder.mystery.weight", &[64, 64, 3, 3])])),
            None,
            "rank 4 with nothing saying what it emits is a guess, not a measurement"
        );
        assert_eq!(medium_of(&serde_json::json!({})), None);
    }

    /// And it is asked of a VAE and of nothing else — the same gate `encoder` is behind.
    ///
    /// A checkpoint carries a VAE inside it, and its own measured family already answers which
    /// medium it makes. Two answers to one question is how they come to disagree.
    #[test]
    fn only_a_vae_is_asked_which_medium_it_decodes_into() {
        let vae = shaped(&[
            ("decoder.conv_in.weight", &[512, 16, 3, 3]),
            ("decoder.conv_out.weight", &[3, 128, 3, 3]),
            ("encoder.conv_out.weight", &[32, 512, 3, 3]),
        ]);
        let read = from_header(&vae, 1);
        assert_eq!(read.kind, Kind::Vae, "the fixture has to be one");
        assert_eq!(read.medium, Some(Makes::Picture));

        let checkpoint = shaped(&[
            (
                "model.diffusion_model.input_blocks.0.0.weight",
                &[320, 4, 3, 3],
            ),
            ("first_stage_model.decoder.conv_out.weight", &[3, 128, 3, 3]),
            (
                "cond_stage_model.transformer.text_model.embeddings.token_embedding.weight",
                &[49408, 768],
            ),
        ]);
        let read = from_header(&checkpoint, 1);
        assert_ne!(read.kind, Kind::Vae, "the fixture has to not be one");
        assert_eq!(
            read.medium, None,
            "a checkpoint's family already answers this"
        );
    }

    /// **Three families spell their VAE `vae.`**, and none of them was a checkpoint until this.
    ///
    /// Measured on this machine: LTXV, Stable Audio and ACE-Step all carry `model.diffusion_model.`
    /// beside a `vae.` and none has a `first_stage_model.`. `Shelf::for_kind` sends an unknown to
    /// the checkpoint shelf, so they *landed* right — and everything that asked what they were got
    /// a different answer from everything that asked where they lived.
    #[test]
    fn a_model_with_its_vae_beside_it_is_a_checkpoint() {
        for spelling in [
            "first_stage_model.",
            "cond_stage_model.",
            "conditioner.",
            "vae.",
        ] {
            let names = [
                "model.diffusion_model.something.weight".to_owned(),
                format!("{spelling}decoder.weight"),
            ];
            let borrowed: Vec<&str> = names.iter().map(String::as_str).collect();
            assert_eq!(
                kind_of(&borrowed, &header(&borrowed)),
                Kind::Checkpoint,
                "{spelling}"
            );
        }
        // A bare diffusion model still is not one: putting a `CheckpointLoaderSimple` in front
        // of something that cannot answer it is what this rule exists to prevent.
        assert_ne!(
            kind_of(
                &["model.diffusion_model.double_blocks.0.weight"],
                &header(&["model.diffusion_model.double_blocks.0.weight"])
            ),
            Kind::Checkpoint
        );
    }

    /// **A rule that was exact for one family stops being exact when a second borrows it.**
    ///
    /// Hunyuan3D's transformer is a DiT with `double_blocks.` and `single_blocks.` under exactly
    /// the names Flux uses, so it read as `flux` — a 3D model on the picture tab and an empty 3D
    /// one. What it does not share is a decoder for *geometry*, which is a key about what the
    /// model does rather than how its blocks are named.
    #[test]
    fn a_model_that_decodes_geometry_is_not_flux() {
        let shape = base_from_names(
            &[
                "model.double_blocks.0.img_attn.norm.key_norm.scale",
                "model.single_blocks.0.linear1.weight",
                "vae.geo_decoder.cross_attn_decoder.attn.attention.k_norm.weight",
                "conditioner.main_image_encoder.model.something.weight",
            ],
            &header(&[]),
        );
        assert_eq!(shape, Base::Hunyuan3d);
        assert_eq!(shape.makes(), Some(Makes::Model));

        // And Flux, which shares the blocks and decodes no geometry, is still Flux.
        assert_eq!(
            base_from_names(
                &["model.diffusion_model.double_blocks.0.img_attn.proj.weight"],
                &header(&[])
            ),
            Base::Flux
        );
    }

    /// **Which checkpoints bring their own text encoder**, and the one that made this a
    /// measurement rather than a rule.
    ///
    /// The panel asked for an encoder beside every non-picture checkpoint, generalised from the
    /// two it had seen. ACE-Step is all-in-one, and GENERATE sat grey waiting for a file that
    /// model has no use for.
    #[test]
    fn a_checkpoint_says_whether_its_encoder_is_inside_it() {
        // SDXL and SD 1.5 carry theirs, in their own two spellings.
        assert!(encoder_inside(&[
            "conditioner.embedders.1.model.ln_final.bias"
        ]));
        assert!(encoder_inside(&[
            "cond_stage_model.transformer.text_model.final.weight"
        ]));
        // ACE-Step carries one too, under a third.
        assert!(encoder_inside(&["text_encoders.t5.encoder.block.0.weight"]));

        // LTXV brings none: a model and a VAE and nothing else.
        assert!(!encoder_inside(&[
            "model.diffusion_model.patchify_proj.weight",
            "vae.decoder.conv_in.conv.weight",
        ]));

        // **And Stable Audio is why the SDXL prefix is `embedders` rather than `conditioner.`.**
        // It has a conditioner that embeds a *duration* and no text encoder at all; one segment
        // shorter and this would call it self-contained, leaving the panel refusing to ask for
        // the T5 it genuinely needs.
        assert!(!encoder_inside(&[
            "conditioner.conditioners.seconds_start.embedder.embedding.0.weights",
            "model.model.transformer.project_in.weight",
        ]));
    }

    /// **LTXV, by the key ComfyUI itself uses.**
    ///
    /// Read from the owner's own file (2026-08-28): `model.diffusion_model.patchify_proj.weight`
    /// at `[2048, 128]` beside `adaln_single` and 28 `transformer_blocks.N.scale_shift_table`.
    /// Nothing that draws a still has a patchify projection.
    ///
    /// It is checked **before** the text-encoder tests, and that ordering is the point: an LTXV
    /// checkpoint carries no text encoder at all, so every one of those tests fails for a reason
    /// that says nothing about which family it is.
    #[test]
    fn a_video_model_is_read_as_one_and_says_it_moves() {
        let ltxv = base_from_names(
            &[
                "model.diffusion_model.patchify_proj.weight",
                "model.diffusion_model.adaln_single.linear.weight",
                "model.diffusion_model.transformer_blocks.0.scale_shift_table",
                "vae.decoder.conv_in.conv.weight",
            ],
            &header(&[]),
        );
        assert_eq!(ltxv, Base::Ltxv);
        assert_eq!(ltxv.makes(), Some(Makes::Video));

        // **And sound, by a conditioner that embeds a duration** — read from the owner's own
        // Stable Audio Open file, which has a `conditioner.` prefix that is not SDXL's.
        let audio = base_from_names(
            &[
                "conditioner.conditioners.seconds_start.embedder.embedding.0.weights",
                "model.model.transformer.project_in.weight",
                "pretransform.model.decoder.layers.0.weight",
            ],
            &header(&[]),
        );
        assert_eq!(audio, Base::StableAudio);
        assert_eq!(audio.makes(), Some(Makes::Sound));

        // **And the other sound family, by a lyric embedding.** Two families making one medium,
        // told apart by what each is *told*: Stable Audio takes a description, ACE-Step takes a
        // genre and a vocal — which is why it has an embedder for each.
        let song = base_from_names(
            &[
                "model.diffusion_model.lyric_embs.weight",
                "model.diffusion_model.genre_embedder.weight",
                "text_encoders.something.weight",
            ],
            &header(&[]),
        );
        assert_eq!(song, Base::AceStep);
        assert_eq!(song.makes(), Some(Makes::Sound));

        // And every family that draws stills says so, rather than saying nothing.
        for still in [
            Base::Sd15,
            Base::Sd2,
            Base::Sdxl,
            Base::Sd3,
            Base::Flux,
            Base::ZImage,
        ] {
            assert_eq!(still.makes(), Some(Makes::Picture), "{still:?}");
        }

        // **Unknown is none of them, and that is the answer the panel needs.** A model Epoch
        // could not read is offered under every tab; hiding somebody's file for a failure of
        // Epoch's is the worst outcome available.
        assert_eq!(Base::Unknown.makes(), None);
    }

    ///
    /// `cap_embedder.1.weight` beside `noise_refiner.0.…` is the Lumina 2 arrangement, and the
    /// first dimension of that weight separates the two: 2304 is the original Lumina 2 and 3840
    /// is Z-Image. Measured on the owner's file: `[3840, 2560]`.
    ///
    /// **Why it was worth adding.** With the family unreadable, the panel fell back to guessing
    /// from whichever encoder had been picked — so choosing a CLIP-L for a Z-Image made Epoch
    /// answer *"Epoch measured this model as Stable Diffusion"* and then mark that CLIP-L as the
    /// one the model needs. Reading the model closes the circle.
    ///
    /// **And plain Lumina 2 stays unmeasured.** Epoch has never drawn with one, so filing it by
    /// association with its neighbour would be exactly the guess this module refuses.
    #[test]
    fn z_image_is_told_from_its_neighbour_by_a_width() {
        let z = from_header(
            &shaped(&[
                ("cap_embedder.1.weight", &[3840, 2560]),
                ("noise_refiner.0.attention.k_norm.weight", &[128]),
                ("layers.0.attention.qkv.weight", &[1, 1]),
            ]),
            6_000_000_000,
        );
        assert_eq!(z.base, Base::ZImage);
        assert_eq!(z.kind, Kind::DiffusionModel);

        let lumina = from_header(
            &shaped(&[
                ("cap_embedder.1.weight", &[2304, 2048]),
                ("noise_refiner.0.attention.k_norm.weight", &[128]),
                ("layers.0.attention.qkv.weight", &[1, 1]),
            ]),
            6_000_000_000,
        );
        assert_eq!(
            lumina.base,
            Base::Unknown,
            "a family nobody drew with is unmeasured, not filed by association"
        );
    }

    /// Every one of these is a shape read off a file really on this machine, 2026-08-25.
    ///
    /// The panel could not help somebody choose the parts of a model that arrives in parts,
    /// because the *family* of a part is unmeasurable — `clip_l.safetensors` is the same file
    /// beside Flux and beside SDXL. What it is, is not: 768 wide is a CLIP-L, and no amount of
    /// renaming changes that.
    #[test]
    fn a_text_encoder_is_named_by_the_width_of_its_own_embedding_table() {
        let clip_l = from_header(
            &shaped(&[
                (
                    "text_model.embeddings.token_embedding.weight",
                    &[49408, 768],
                ),
                ("text_model.encoder.layers.0.mlp.fc1.weight", &[3072, 768]),
            ]),
            246_144_152,
        );
        assert_eq!(clip_l.kind, Kind::TextEncoder);
        assert_eq!(clip_l.encoder, Some(Encoder::ClipL));

        let clip_g = from_header(
            &shaped(&[
                (
                    "text_model.embeddings.token_embedding.weight",
                    &[49408, 1280],
                ),
                ("text_model.encoder.layers.0.mlp.fc1.weight", &[5120, 1280]),
            ]),
            1_389_382_176,
        );
        assert_eq!(clip_g.encoder, Some(Encoder::ClipG));

        // `t5xxl_fp8_e4m3fn.safetensors`, measured.
        let t5 = from_header(
            &shaped(&[
                ("shared.weight", &[32128, 4096]),
                (
                    "encoder.block.0.layer.0.SelfAttention.q.weight",
                    &[4096, 4096],
                ),
            ]),
            4_893_934_904,
        );
        assert_eq!(t5.kind, Kind::TextEncoder);
        assert_eq!(t5.encoder, Some(Encoder::T5Xxl));

        // And the family is still unknown, which is correct and is the point: an encoder does not
        // belong to one.
        assert_eq!(clip_l.base, Base::Unknown);
        assert_eq!(t5.base, Base::Unknown);
    }

    /// A decoder-only language model on the text-encoder shelf.
    ///
    /// **This shape was missing entirely.** `qwen_3_4b_fp8_mixed.safetensors` is loaded by
    /// ComfyUI's `CLIPLoader` and is neither a T5 nor a CLIP, so Epoch filed it as *something it
    /// could not identify* and the panel offered it with nothing said about it. Qwen-Image and
    /// HiDream are loaded this way; it is not a curiosity.
    #[test]
    fn a_language_model_used_as_an_encoder_is_read_as_one() {
        let qwen = from_header(
            &shaped(&[
                ("model.embed_tokens.weight", &[151_936, 2560]),
                ("model.layers.0.self_attn.q_proj.weight", &[4096, 2560]),
                ("model.norm.weight", &[2560]),
            ]),
            5_631_000_000,
        );
        assert_eq!(qwen.kind, Kind::TextEncoder);
        assert_eq!(qwen.encoder, Some(Encoder::Language));
    }

    /// A diffusion model whose blocks are simply `layers.N`.
    ///
    /// Measured: `z_image_turbo_int8_convrot.safetensors` is 857 tensors of `layers.0…` and
    /// nothing else. Every earlier rule declined, so it came back `Unknown` — and `Shelf::for_kind`
    /// sends `Unknown` to the *checkpoints* shelf, which is where a `CheckpointLoaderSimple` ends
    /// up in front of something that cannot answer one.
    #[test]
    fn a_bare_transformer_with_no_encoder_and_no_vae_is_a_diffusion_model() {
        let z = from_header(
            &shaped(&[
                ("layers.0.attn.qkv.weight", &[9216, 3072]),
                ("layers.1.attn.qkv.weight", &[9216, 3072]),
                ("final_layer.linear.weight", &[64, 3072]),
            ]),
            6_201_000_000,
        );
        assert_eq!(z.kind, Kind::DiffusionModel);
        // Unmeasured family, and that stays unmeasured. The rule identifies a *shape*, not a
        // family, and inventing one here would be exactly the guess this module refuses.
        assert_eq!(z.base, Base::Unknown);

        // And it must not swallow the things that already had answers.
        let vae = from_header(
            &shaped(&[
                ("decoder.conv_in.weight", &[512, 16, 3, 3]),
                ("encoder.conv_in.weight", &[128, 3, 3, 3]),
            ]),
            167_000_000,
        );
        assert_eq!(
            vae.kind,
            Kind::Vae,
            "a VAE has both halves and draws nothing"
        );
        let clip = from_header(
            &shaped(&[
                (
                    "text_model.embeddings.token_embedding.weight",
                    &[49408, 768],
                ),
                ("text_model.encoder.layers.0.mlp.fc1.weight", &[3072, 768]),
            ]),
            246_000_000,
        );
        assert_eq!(clip.kind, Kind::TextEncoder);
    }

    #[test]
    #[ignore = "prints what this machine has"]
    fn what_is_here() {
        let mut roots: Vec<std::path::PathBuf> = Vec::new();
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            roots.push(std::path::PathBuf::from(local).join("Comfy-Desktop/ComfyUI-Shared/models"));
        }
        if let Some(roaming) = std::env::var_os("APPDATA") {
            roots.push(std::path::PathBuf::from(roaming).join("Epoch/library/generative"));
        }
        for root in roots {
            println!(
                "
== {}",
                root.display()
            );
            let Ok(shelves) = std::fs::read_dir(&root) else {
                println!("   (not on this machine)");
                continue;
            };
            for shelf in shelves.flatten() {
                let Ok(files) = std::fs::read_dir(shelf.path()) else {
                    continue;
                };
                for file in files.flatten() {
                    if !file.path().is_file() {
                        continue;
                    }
                    match understand(&file.path()) {
                        Ok(read) => println!(
                            "   {:<44} {:<10} {:<8} {:>7} MB  {:<16} {}",
                            file.file_name().to_string_lossy(),
                            read.kind.plainly(),
                            read.base.plainly(),
                            read.bytes / 1_000_000,
                            read.encoder.map(|it| it.plainly()).unwrap_or(""),
                            read.unread.unwrap_or("")
                        ),
                        Err(why) => println!("   {:<44} {why}", file.file_name().to_string_lossy()),
                    }
                }
            }
        }
    }

    #[test]
    fn an_sdxl_checkpoint_is_read_as_one() {
        let read = from_header(
            &header(&[
                "model.diffusion_model.input_blocks.0.0.weight",
                "first_stage_model.decoder.conv_in.weight",
                "conditioner.embedders.1.model.ln_final.weight",
            ]),
            6_938_040_714,
        );
        assert_eq!(read.kind, Kind::Checkpoint);
        assert_eq!(read.base, Base::Sdxl);
    }

    #[test]
    fn an_sd15_checkpoint_is_not_mistaken_for_sdxl() {
        let read = from_header(
            &header(&[
                "model.diffusion_model.input_blocks.0.0.weight",
                "first_stage_model.decoder.conv_in.weight",
                "cond_stage_model.transformer.text_model.final_layer_norm.weight",
            ]),
            4_265_146_304,
        );
        assert_eq!(read.kind, Kind::Checkpoint);
        assert_eq!(read.base, Base::Sd15);
    }

    #[test]
    fn a_lora_is_a_lora_and_its_second_text_encoder_names_the_family() {
        let sdxl = from_header(
            &header(&[
                "lora_unet_input_blocks_1_1_proj_in.lora_down.weight",
                "lora_te2_text_model_encoder_layers_0_mlp_fc1.lora_up.weight",
            ]),
            228_449_016,
        );
        assert_eq!(sdxl.kind, Kind::Lora);
        assert_eq!(sdxl.base, Base::Sdxl);

        let sd15 = from_header(
            &header(&[
                "lora_unet_down_blocks_0_attentions_0_proj_in.lora_down.weight",
                "lora_te_text_model_encoder_layers_0_mlp_fc1.lora_up.weight",
            ]),
            37_876_688,
        );
        assert_eq!(sd15.kind, Kind::Lora);
        assert_eq!(sd15.base, Base::Sd15);
    }

    #[test]
    fn a_flux_lora_is_flux_however_it_is_named() {
        let read = from_header(
            &header(&[
                "transformer.single_transformer_blocks.0.attn.to_q.lora_A.weight",
                "transformer.single_transformer_blocks.0.attn.to_q.lora_B.weight",
            ]),
            171_969_616,
        );
        assert_eq!(read.kind, Kind::Lora);
        // The whole point of ADR-0032's rule: a file called `pixel_art_sdxl_v3` that is Flux
        // inside is read as Flux, because nothing here ever looked at the name.
        assert_eq!(read.base, Base::Flux);
    }

    #[test]
    fn a_text_encoder_is_not_a_model() {
        // T5 names its stack `encoder.block.N`; CLIP names it `text_model.encoder.layers.N`.
        // Neither draws anything, and offering one as a model is offering a render that cannot
        // start.
        let t5 = from_header(
            &header(&["encoder.block.0.layer.0.SelfAttention.q.weight"]),
            1,
        );
        assert_eq!(t5.kind, Kind::TextEncoder);
        let clip = from_header(
            &header(&["text_model.encoder.layers.0.self_attn.q_proj.weight"]),
            1,
        );
        assert_eq!(clip.kind, Kind::TextEncoder);
    }

    #[test]
    fn a_bare_diffusion_model_is_not_a_checkpoint() {
        // The instance on the paired MacBook opened with exactly this shape — `z_image_turbo`,
        // loaded by `Load Diffusion Model`. Filing it as a checkpoint is what would put a
        // `CheckpointLoaderSimple` in front of something that cannot answer one.
        let read = from_header(
            &header(&["double_blocks.0.img_attn.qkv.weight"]),
            11_901_224_016,
        );
        assert_eq!(read.kind, Kind::DiffusionModel);
        assert_eq!(read.base, Base::Flux);
    }

    #[test]
    fn a_standalone_vae_is_a_vae() {
        // **Real shapes, because the shape is now part of the answer.** A T5 carries `encoder.`
        // and `decoder.` too and is not a VAE; what separates them completely is that an
        // autoencoder convolves and a transformer never does.
        let read = from_header(
            &shaped(&[
                ("encoder.conv_in.weight", &[128, 3, 3, 3]),
                ("decoder.conv_out.weight", &[3, 128, 3, 3]),
            ]),
            334_641_162,
        );
        assert_eq!(read.kind, Kind::Vae);
    }

    /// An encoder-decoder **text** model is not an autoencoder.
    ///
    /// Measured on the owner's own `t5_base.safetensors`: `decoder.` (159 tensors), `encoder.`
    /// (98) and `shared.weight`, every one of them rank 1 or rank 2 — no convolution anywhere.
    /// It was listed as *a VAE* on the Creations deck beside the real ones until 2026-08-30.
    ///
    /// A rule naming text architectures would need an entry per model. This is one measurement
    /// that covers every text model there will ever be.
    #[test]
    fn a_text_model_with_both_halves_is_not_a_vae() {
        let read = from_header(
            &shaped(&[
                ("shared.weight", &[32128, 768]),
                (
                    "encoder.block.0.layer.0.SelfAttention.q.weight",
                    &[768, 768],
                ),
                (
                    "decoder.block.0.layer.0.SelfAttention.q.weight",
                    &[768, 768],
                ),
            ]),
            892_000_000,
        );
        assert_ne!(read.kind, Kind::Vae, "a T5 has no convolutions");
    }

    #[test]
    fn what_a_file_claims_is_kept_even_when_it_is_not_understood() {
        let mut head = header(&["some.tensor"]);
        head.as_object_mut().unwrap().insert(
            "__metadata__".to_owned(),
            json!({"modelspec.architecture": "something-nobody-has-heard-of"}),
        );
        let read = from_header(&head, 12);
        assert_eq!(read.base, Base::Unknown);
        assert_eq!(
            read.claimed.as_deref(),
            Some("something-nobody-has-heard-of")
        );
    }

    #[test]
    fn the_tensors_outrank_nothing_but_they_answer_when_metadata_does_not() {
        // No claim at all, and it is still read correctly. This is the case that matters: most
        // files downloaded from anywhere carry no `modelspec` block.
        let read = from_header(&header(&["joint_blocks.0.x_block.attn.qkv.weight"]), 1);
        assert_eq!(read.base, Base::Sd3);
    }

    #[test]
    fn unknown_is_compatible_with_everything_because_nobody_measured_it() {
        assert!(Base::Unknown.works_with(Base::Sdxl));
        assert!(Base::Sdxl.works_with(Base::Unknown));
        assert!(Base::Sdxl.works_with(Base::Sdxl));
        assert!(!Base::Sd15.works_with(Base::Sdxl));
        assert!(!Base::Flux.works_with(Base::Sdxl));
    }

    #[test]
    fn a_pickle_is_reported_unread_rather_than_guessed() {
        let path = std::env::temp_dir().join("epoch-asset-test.ckpt");
        std::fs::write(&path, b"not really a pickle").unwrap();
        let read = understand(&path).unwrap();
        assert_eq!(read.kind, Kind::Unknown);
        assert!(read.unread.unwrap().contains("pickle"), "{read:?}");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_real_safetensors_file_is_read_from_its_bytes() {
        // Written here rather than fetched: the format is eight bytes of length and then JSON,
        // and a test that builds one proves the reader against the format rather than against a
        // file somebody happened to have.
        let head = serde_json::to_vec(&json!({
            "__metadata__": {"modelspec.architecture": "stable-diffusion-xl-v1-base"},
            "lora_unet_x.lora_down.weight": {"dtype": "F16", "shape": [1, 1], "data_offsets": [0, 4]},
        }))
        .unwrap();
        let path = std::env::temp_dir().join("epoch-asset-test.safetensors");
        let mut file = Vec::new();
        file.extend_from_slice(&(head.len() as u64).to_le_bytes());
        file.extend_from_slice(&head);
        file.extend_from_slice(&[0u8; 4]);
        std::fs::write(&path, &file).unwrap();

        let read = understand(&path).unwrap();
        assert_eq!(read.kind, Kind::Lora);
        assert_eq!(read.base, Base::Sdxl);
        assert_eq!(read.unread, None);
        assert_eq!(read.bytes, file.len() as u64);

        // And the same file has a fingerprint, which is the other half of ADR-0032's rule.
        let hash = fingerprint(&path).unwrap();
        assert_eq!(hash.len(), 64, "{hash}");
        assert_eq!(hash, fingerprint(&path).unwrap());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_header_that_lies_about_its_own_length_is_refused_rather_than_allocated() {
        let path = std::env::temp_dir().join("epoch-asset-hostile.safetensors");
        let mut file = u64::MAX.to_le_bytes().to_vec();
        file.extend_from_slice(b"{}");
        std::fs::write(&path, &file).unwrap();
        let read = understand(&path).unwrap();
        assert!(read.unread.is_some(), "{read:?}");
        let _ = std::fs::remove_file(path);
    }
}
