//! A ComfyUI workflow, read the way people actually have them.
//!
//! ## Two formats, and the one that matters is not the obvious one
//!
//! ComfyUI has an **API format** — `{ "6": { "class_type": …, "inputs": … } }` — which is what
//! its server executes, and which its editor produces under *Export (API)*.
//!
//! It also has a **UI format**: `nodes[]`, `links[]` and `widgets_values[]`, which is what *Save*
//! produces, what its 522 bundled templates are written in, and what every workflow shared on
//! Civitai or in a repository turns out to be. Measured 2026-08-22 against the templates on this
//! machine: every one of them is UI format.
//!
//! So reading only the API format would mean telling somebody to open ComfyUI and re-export
//! before Epoch could look at a file they already have — the exact friction this exists to
//! remove. Both are read.
//!
//! ## The conversion needs the server, and that is a feature
//!
//! A UI workflow's widget values are **positional**: `[25, 8, "euler", "normal"]` means nothing
//! without knowing that this node's inputs are `steps, cfg, sampler_name, scheduler`. That order
//! lives in the server's own `/object_info`.
//!
//! Depending on it is not a weakness. It means a workflow is converted against **the ComfyUI that
//! will run it**, so a graph needing a custom node this machine does not have fails at import
//! with the right sentence — rather than at generation time with a stack trace.
//!
//! ## Three traps, all found by running the result
//!
//! Each of these was a defect first and a rule second (2026-08-22, converting ComfyUI's own
//! `sdxl_simple_example` and running the output):
//!
//! 1. **A widget converted to a link keeps its place in `widgets_values`.** Skipping it shifted
//!    every later value by one and produced `cfg: 25, sampler_name: 8, scheduler: "euler"` — a
//!    request the server refuses. The value is consumed either way; the link overwrites it.
//! 2. **A seed widget is followed by a value that is not an input.** `control_after_generate`
//!    (`"fixed"`, `"randomize"`) sits in the list and belongs to the editor.
//! 3. **Not every node is a node.** `Note` and `MarkdownNote` are text for people;
//!    `PrimitiveNode` and `Reroute` are values and wires that must be *followed through* rather
//!    than sent to the server, which knows none of them.
//!
//! With those three, ComfyUI's own SDXL template — 25 nodes down to 11 executable — ran and
//! produced its own prompt in 9.1 s.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Nodes that carry text for a person and execute nothing.
const DECORATION: [&str; 2] = ["Note", "MarkdownNote"];

/// Nodes that are a value or a wire. They are followed through, never sent.
const PASSTHROUGH: [&str; 7] = [
    "PrimitiveNode",
    "Reroute",
    "PrimitiveInt",
    "PrimitiveFloat",
    "PrimitiveString",
    "PrimitiveStringMultiline",
    "PrimitiveBoolean",
];

/// A node the editor has muted or bypassed. Both mean *do not run this*.
fn silenced(mode: Option<i64>) -> bool {
    matches!(mode, Some(2) | Some(4))
}

/// What one input of one node is set to.
///
/// Either a literal the person typed, or the output of another node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Input {
    /// `["6", 0]` — node `6`, output slot `0`. The wire shape ComfyUI's API uses.
    From(Vec<serde_json::Value>),
    Value(serde_json::Value),
}

/// One executable node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub class_type: String,
    pub inputs: BTreeMap<String, Input>,
}

/// A workflow in the shape the server executes.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Workflow {
    pub nodes: BTreeMap<String, Node>,
}

/// A workflow Epoch has taken in: what arrived, what it compiled to, and what it means.
///
/// ## The original is kept, and an earlier version of this file was wrong about that
///
/// It stored only the compiled form, reasoning that *the import is where that question is asked
/// and answered once*. The counterexample is the feature standing right next to it: an import
/// fails with `needs IPAdapterApply`, the person installs that node — and there is **nothing to
/// retry**, because the only thing kept was a conversion that never happened.
///
/// The same holds when ComfyUI's schema changes under a workflow that imported fine, and when
/// this compiler improves. So the source is immutable and the compiled form is disposable, which
/// is the arrangement ADR-0018 already uses for snapshots: *a snapshot preserves continuity; it
/// never defines reality.*
///
/// ## Three representations, and the middle one is a description
///
/// `source` is the truth. `compiled` is for **this** ComfyUI and is thrown away and rebuilt.
/// `opening` is what the thing means — what it can do, what it takes, what it needs — and it is
/// the only one of the three with no ComfyUI in it. A second engine reads that, not the graph.
///
/// A general node-level intermediate representation was proposed and is **not built**: with one
/// backend it would be ComfyUI's own node model under different names, which is worse than
/// either end of it. `Opening` already carries everything an engine swap needs, and the day
/// there is a second engine the IR can be designed against two real ones instead of one
/// imagined.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Imported {
    /// Exactly the bytes that arrived, parsed and otherwise untouched.
    pub source: serde_json::Value,
    /// Compiled for the ComfyUI that was asked at import time.
    pub compiled: Workflow,
    /// What it means, with no ComfyUI in it.
    pub opening: Opening,
}

/// The plainest workflow that draws, written against what a machine actually reports.
///
/// ## Why Epoch builds one rather than shipping one
///
/// `CONTENT_PHILOSOPHY` says nothing may come in the box that quietly becomes the house style,
/// and that stands. Nothing here is shipped: this is **derived from what one ComfyUI answered**,
/// exactly like everything else Epoch knows about a machine. A packaged `.json` would name a
/// checkpoint the user may not have and fail with nothing to fix; this names the checkpoint the
/// server itself listed a moment ago.
///
/// It also earns itself against the real alternative, which is seven steps across two
/// applications: export from ComfyUI, find the file, import it, attach it. Somebody who has
/// never opened ComfyUI cannot draw at all until they learn ComfyUI, which is the opposite of
/// the promise.
///
/// ## Seven nodes, and every one of them is core
///
/// `CheckpointLoaderSimple · CLIPTextEncode ×2 · EmptyLatentImage · KSampler · VAEDecode ·
/// SaveImage`. No custom nodes, so this compiles on any ComfyUI — and if one of them is somehow
/// missing, [`Imported::of`] refuses by name rather than producing a graph that fails later.
///
/// The result goes through the ordinary import path, so it is an ordinary workflow afterwards:
/// same `Opening`, same recompile, same FORGET. Nothing downstream can tell it apart, which is
/// the point — a built one is not a second kind of thing.
///
/// ## The classes are checked here, and that turned out to be necessary
///
/// An API-format workflow is passed through without consulting the schema — deliberately, since
/// the server is the authority on its own graph and will say what it does not know. That is the
/// right call for **somebody else's file** and the wrong one for a file Epoch is writing: it
/// would mean building a graph, storing it, attaching it to General and only discovering
/// halfway through a Quest that this ComfyUI has no `KSampler`. A test caught it.
/// Which loader takes that many encoder files.
///
/// ComfyUI's own three, by the only thing that separates them.
/// Whether the loader this many encoders selects has a `type` input at all.
///
/// `TripleCLIPLoader` does not — asked of a live ComfyUI 2026-08-24, its required inputs are
/// `clip_name1`, `clip_name2`, `clip_name3` and nothing more.
pub const fn takes_a_type(encoders: usize) -> bool {
    encoders <= 2
}

const fn clip_loader(encoders: usize) -> &'static str {
    match encoders {
        0 | 1 => "CLIPLoader",
        2 => "DualCLIPLoader",
        _ => "TripleCLIPLoader",
    }
}

/// What somebody asked the Studio Panel for (ADR-0033).
///
/// Every field is a decision the **user** made. Nothing here is inferred from a sentence, which
/// is the whole difference between this and `draw_image`: a character guessing twelve things from
/// one line will guess some of them wrong, and the person had no way to correct it that was not
/// another sentence.
/// What will actually be loaded, and therefore which graph gets written.
///
/// **A checkpoint and a diffusion model are not the same thing**, and the difference is the whole
/// reason this is an enum. A checkpoint carries its own text encoder and VAE; Flux and Z-Image
/// are normally distributed as the diffusion half alone, and putting a `CheckpointLoaderSimple`
/// in front of one produces a refusal nobody can act on.
#[derive(Debug, Clone, PartialEq)]
pub enum Loading {
    /// One file that carries everything. SD 1.5, SD 2, SDXL, and Flux's all-in-one builds.
    Checkpoint(String),
    /// A checkpoint that carries its model and its VAE, and **no text encoder**.
    ///
    /// ## Found by drawing, not by reading
    ///
    /// `ltxv-2b-0.9.6-distilled` loads through `CheckpointLoaderSimple` like any checkpoint and
    /// answers `CLIP = None`. Measured 2026-08-28, in the window, and ComfyUI says it in as many
    /// words: *"clip input is invalid: None — if the clip is from a checkpoint loader node your
    /// checkpoint does not contain a valid clip or text encoder model."*
    ///
    /// It is a real third shape rather than a special case. `Checkpoint` means *everything is in
    /// here*; `Assembled` means *nothing is*; this means *the encoder is not*, which is how most
    /// video families ship — the encoder is a T5 the size of the model itself, and shipping it
    /// inside every checkpoint would double every download.
    ///
    /// **Derived from what the person picked, never from the file.** A checkpoint with encoders
    /// chosen beside it is this; the same checkpoint with none is `Checkpoint`. Epoch does not
    /// read a checkpoint and decide it is missing something (ADR-0033).
    Encoded {
        checkpoint: String,
        /// One, two or three encoder files, in the order their loader takes them.
        clip: Vec<String>,
        /// The `type`, in the vocabulary of the loader this many encoders selects.
        clip_type: String,
    },
    /// Anything that arrives in parts: a diffusion model, its text encoders, and a VAE.
    ///
    /// ## One recipe rather than fifteen
    ///
    /// Flux, Flux.2, SD 3.5, Z-Image, Qwen-Image, Hunyuan, HiDream, Chroma, PixArt, Wan, LTXV —
    /// they differ in **one string**. The graph is the same three loaders into the same sampler,
    /// and what separates them is the `type` a `CLIPLoader` is given.
    ///
    /// **And ComfyUI publishes that list — but it publishes a *different one per loader*, and
    /// that took a refused picture to learn.** Measured 2026-08-24, asked of the server:
    ///
    /// ```text
    /// CLIPLoader        28 types  stable_diffusion, sd3, flux2, qwen_image, chroma, krea2 …
    /// DualCLIPLoader    12 types  sdxl, sd3, flux, hunyuan_video, hidream, ltxv, ace …
    /// TripleCLIPLoader  no `type` input at all — three file names and nothing else
    /// ```
    ///
    /// The lists barely overlap. `stable_diffusion` exists only on the single loader; `flux` and
    /// `sdxl` exist only on the double. So the panel offered `CLIPLoader`'s 28 whatever the user
    /// picked, somebody chose two encoders and `stable_diffusion`, and ComfyUI answered:
    ///
    /// ```text
    /// Value not in list: type: 'stable_diffusion' not in ['sdxl','sd3','flux', …]
    /// class_type: "DualCLIPLoader"
    /// ```
    ///
    /// **The measurement was right and it was attributed to the wrong node.** One list was read
    /// and then treated as ComfyUI's vocabulary rather than as one node's. The fix keeps the rule
    /// — the server is still the only source — and asks the loader that will actually run.
    ///
    /// ## How many encoders is how many the person chose
    ///
    /// One is `CLIPLoader`, two is `DualCLIPLoader`, three is `TripleCLIPLoader` (SD 3's shape).
    /// Nothing here decides that: a family needs what it needs, and the panel is where somebody
    /// says which files they have.
    Assembled {
        unet: String,
        /// One, two or three encoder files, in the order their loader takes them.
        clip: Vec<String>,
        /// The `type`, in the vocabulary of **the loader this many encoders selects**. Never
        /// invented here, and never borrowed from a different loader.
        ///
        /// Empty for `TripleCLIPLoader`, which has no such input — sending one would be an
        /// unknown key rather than a wrong value, which ComfyUI refuses just as firmly.
        clip_type: String,
        vae: String,
    },
}
/// Which two nodes a family of video models needs, and nothing else.
///
/// **A video graph is the picture graph with three things changed** — measured, not assumed: the
/// same checkpoint loader, the same two `CLIPTextEncode`s, the same `KSampler`, the same
/// `VAEDecode`. What differs is the latent it starts from, one conditioning node some families
/// interpose, and that the decode is muxed into a file instead of saved as a PNG.
///
/// That is why this is three strings rather than a second `compose`. A parallel graph writer
/// would duplicate the loader selection, the LoRA chain and the ControlNet chain, and the two
/// would drift — and *when two things must agree, derive both from the same measurement*.
// No `Eq`: two of these fields are durations in seconds, and a family is compared by what
// it *is* rather than by an exact float. `PartialEq` is what the one comparison needs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Family {
    /// The empty latent this family samples from, and what it is measured in.
    pub latent: &'static str,
    /// A conditioning node the family interposes between the text encoders and the sampler,
    /// where it has one. It takes both conditionings and answers both.
    pub conditioning: Option<&'static str>,
    /// What turns words into a conditioning.
    ///
    /// `CLIPTextEncode` for almost everything, and it takes a `text`. ACE-Step's takes **`tags`
    /// and `lyrics`** instead, which is not a rename — a music model is told a genre and a
    /// vocal separately, and sending its prose to `text` is an unknown input.
    pub encode: &'static str,
    /// What turns the sampler's output back into something. `VAEDecode` gives frames;
    /// `VAEDecodeAudio` gives sound, and they are different nodes rather than one with a flag.
    pub decode: &'static str,
    /// How what came out of the decode becomes a file.
    pub keeping: Keeping,
    /// How long this family is *usually* asked for, in seconds. Ignored by anything that is not
    /// measured in seconds.
    ///
    /// **The latent node's own declared default**, asked of the server rather than remembered:
    /// `EmptyLatentAudio` declares `47.6` and `EmptyAceStepLatentAudio` declares `120.0`, which
    /// is each family's trained length said by the thing that would know.
    pub usual: f64,
    /// The most it will accept. **Also the node's**, and it is the same number for both — 1000 —
    /// because the ceiling belongs to ComfyUI rather than to the model.
    ///
    /// Asking Stable Audio for 200 seconds is not refused; it pads. That is the user's call and
    /// the panel says which number is the trained one, the same way the sizes are marked.
    pub longest: f64,
    /// Whether this family's encoder takes **words that are sung**, beside the words that
    /// describe the sound.
    ///
    /// A property of the node rather than of the medium: Stable Audio makes sound and is told a
    /// description only, so a LYRICS box on its tab would be a control that reaches nothing.
    pub lyrical: bool,
}

/// The last step of a graph, which is a different pair of nodes per medium.
///
/// **Named rather than inferred from the family.** Two families could share a medium and a third
/// could arrive that saves differently, and a `match` on the family name would be the second
/// place that has to agree with the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keeping {
    /// A batch of frames muxed into one file: `CreateVideo` then `SaveVideo`.
    Video,
    /// Sound straight to a file. **Measured 2026-08-28**: `SaveAudio` takes an `AUDIO` and a
    /// prefix and writes FLAC, reported under the `audio` key rather than `images`.
    Audio,
    /// A mesh. **Measured 2026-08-29**: the decode gives a `VOXEL`, `VoxelToMeshBasic` turns
    /// that into a `MESH`, and `SaveGLB` writes a `.glb` reported under the `3d` key.
    Mesh,
}

impl Keeping {
    /// The classes this needs on the server, in the order they run.
    pub const fn nodes(self) -> &'static [&'static str] {
        match self {
            Keeping::Video => &["CreateVideo", "SaveVideo"],
            Keeping::Audio => &["SaveAudio"],
            Keeping::Mesh => &["VoxelToMesh", "SaveGLB"],
        }
    }
}

/// The video families Epoch can compose a graph for.
///
/// **One entry, because one is what has been measured.** Drawn on this machine 2026-08-28:
/// `ltxv-2b-0.9.6-distilled`, 49 frames at 512x320, **10.1 s**, through exactly the graph below.
///
/// Wan and Hunyuan are the obvious next two and are deliberately absent. Their nodes are on this
/// server (`EmptyHunyuanLatentVideo`, `WanImageToVideo`) and no model of either family is, so
/// writing their recipes would be writing them from memory — which is the guess this codebase
/// keeps deleting. `PREPARATIONS` above is the same list for the same reason, and it grows the
/// day somebody measures one.
pub const FAMILIES: [(&str, Family); 5] = [
    (
        // LTX-Video. `LTXVConditioning` carries the frame rate into the conditioning, which is
        // how this family is told how fast the thing it is making moves.
        "LTXV",
        Family {
            latent: "EmptyLTXVLatentVideo",
            conditioning: Some("LTXVConditioning"),
            encode: "CLIPTextEncode",
            decode: "VAEDecode",
            keeping: Keeping::Video,
            usual: 0.0,
            longest: 0.0,
            lyrical: false,
        },
    ),
    (
        // **Stable Audio Open, and the shape is the same one.** Measured on this machine
        // 2026-08-28: ten seconds of sound in 6.1 s, through exactly this graph.
        //
        // `ConditioningStableAudio` is this family's `LTXVConditioning` — it takes both
        // conditionings and carries the duration into them, and it answers both. The only thing
        // that is genuinely different is that sound has no width and no height, so its latent
        // takes `seconds` where the others take pixels.
        "Stable Audio",
        Family {
            latent: "EmptyLatentAudio",
            conditioning: Some("ConditioningStableAudio"),
            encode: "CLIPTextEncode",
            decode: "VAEDecodeAudio",
            keeping: Keeping::Audio,
            usual: 47.6,
            longest: 1000.0,
            lyrical: false,
        },
    ),
    (
        // **Hunyuan3D 2, and it is the one family that is not this graph at all.**
        //
        // Video and sound were the picture graph with three nodes swapped. This one is a
        // different graph and the table says so honestly: its loader is
        // `ImageOnlyCheckpointLoader` rather than `CheckpointLoaderSimple`, and its conditioning
        // comes from **a picture** through `CLIPVisionEncode` rather than from words. There is
        // no `CLIPTextEncode` in it anywhere.
        //
        // So `encode` names the vision encoder and `compose` branches to `shape_graph`. Forcing
        // it through the shared chain would have meant a sampler wired from a text encoder that
        // never runs — the maze that a shared function becomes when the thing it shares stops
        // being shared. Measured on this machine 2026-08-29: a cow in **51.2 s**, text through
        // picture to mesh, in one graph.
        // **One family, and the graph variant is the person's choice rather than the file's.**
        //
        // Measured 2026-08-29: `hunyuan3d-dit-v2_fp16` and `hunyuan3d-dit-v2-mv_fp16` are
        // structurally identical — 1645 tensors, the same prefixes, the same `geo_decoder`.
        // Nothing in the bytes says which is the multi-view one; only the filename does, and
        // reading filenames is what ADR-0024 forbids. So Epoch cannot choose, and does not: the
        // panel asks how many views, and says that the four-view graph needs a checkpoint Epoch
        // has no way to recognise.
        "Hunyuan3D",
        Family {
            latent: "EmptyLatentHunyuan3Dv2",
            conditioning: Some("Hunyuan3Dv2Conditioning"),
            encode: "CLIPVisionEncode",
            decode: "VAEDecodeHunyuan3D",
            keeping: Keeping::Mesh,
            usual: 0.0,
            longest: 0.0,
            lyrical: false,
        },
    ),
    (
        // **ACE-Step, and it is here for one reason: length.** Stable Audio Open is trained to
        // 47 seconds; `EmptyAceStepLatentAudio` defaults to **120** and takes more. Measured on
        // this machine 2026-08-28: 90 seconds of music in **18.2 s**.
        //
        // It has no conditioning node — the duration lives on the latent alone — and its text
        // encoder is its own: `TextEncodeAceStepAudio` takes `tags` and `lyrics` rather than a
        // `text`, which is why `encode` had to become a field rather than a constant.
        "ACE-Step",
        Family {
            latent: "EmptyAceStepLatentAudio",
            conditioning: None,
            encode: "TextEncodeAceStepAudio",
            decode: "VAEDecodeAudio",
            keeping: Keeping::Audio,
            usual: 120.0,
            longest: 1000.0,
            lyrical: true,
        },
    ),
    (
        // **ACE-Step 1.5, and it is a family rather than a newer file.** Its nodes are their own
        // classes — `TextEncodeAceStepAudio1.5` and `EmptyAceStep1.5LatentAudio` — read from
        // this ComfyUI's `comfy_extras/nodes_ace.py`, so a server that has one version and not
        // the other says so through `families_where` without anything here being conditional.
        //
        // The encoder is where the two genuinely differ: v1 takes `tags` and `lyrics`, and this
        // one takes those plus a **language, a key, a time signature, a tempo and a duration**,
        // then runs a Qwen3 over them. Everything Epoch does not know is filled from the
        // server's own declared default, the way a preparation's thresholds are.
        "ACE-Step 1.5",
        Family {
            latent: "EmptyAceStep1.5LatentAudio",
            conditioning: None,
            encode: "TextEncodeAceStepAudio1.5",
            decode: "VAEDecodeAudio",
            keeping: Keeping::Audio,
            usual: 120.0,
            longest: 2000.0,
            lyrical: true,
        },
    ),
];

/// Which of them this server can actually run, in the order they are offered.
///
/// The same shape as [`preparations`]: a named set filtered by what the server has, never a
/// category and never a shape.
pub fn families(schema: &Schema) -> Vec<String> {
    families_where(|class| schema.classes.contains_key(class))
}

/// The same question, for a caller that has no [`Schema`] in hand.
///
/// **The panel is that caller and it is not an accident.** Opening the panel asks the server one
/// node at a time — measured 2026-08-24, fetching the whole of `/object_info` took about thirty
/// seconds — so it cannot answer `contains_key`. Written as one function taking a predicate
/// rather than two functions with the same list in them: *when two things must agree, derive
/// both from the same measurement*.
pub fn families_where(has: impl Fn(&str) -> bool) -> Vec<String> {
    FAMILIES
        .iter()
        .filter(|(_, family)| {
            has(family.latent)
                && has(family.decode)
                && family.conditioning.is_none_or(&has)
                && family.keeping.nodes().iter().all(|class| has(class))
        })
        .map(|(name, _)| (*name).to_owned())
        .collect()
}

/// Look one up by the name a surface offered.
pub fn family(name: &str) -> Option<Family> {
    FAMILIES
        .iter()
        .find(|(known, _)| known.eq_ignore_ascii_case(name))
        .map(|(_, family)| *family)
}

/// How a mesh's surface is pulled out of the voxels it was sampled on.
///
/// ## Named looks, not two dials
///
/// The two things that decide it are `VoxelToMesh`'s `algorithm` and the decode's
/// `octree_resolution`. Offering them raw is offering somebody a choice between *surface net at
/// 256* and *basic at 512*, which is four combinations and no way to reason about any of them.
/// A person means *smooth* or *blocky*, the way they mean a Style rather than a workflow file.
///
/// **Three, all measured on this card with the same picture and the same seed:**
///
/// | | algorithm · octree | through the panel | what it looks like |
/// |---|---|---|---|
/// | `Blocky` | basic · 256 | ~1 min | every step shows, on purpose |
/// | `Smooth` | surface net · 256 | **62 s** | a cow with a face, ears, horns and a tail |
/// | `Fine` | surface net · 512 | **366 s** | the same cow, and a clean base |
///
/// ## Why `Fine` exists, and why it took a second look to find
///
/// Smooth and Fine were called identical here for an afternoon, on the strength of comparing the
/// *animal*: same face, same ears, same horns. The owner looked at the **ground** and was right —
/// at 256 the disc under the cow has concentric steps and the grass tufts are jagged; at 512 the
/// disc is a clean ellipse. A voxel grid shows itself on flat surfaces and thin detail, which is
/// exactly where nobody was looking.
///
/// > **Comparing the subject is not comparing the render.** The thing being generated is where
/// > the eye goes and the last place a sampling artefact appears.
///
/// So the six minutes buys something real, and it is six minutes: named on the control, and the
/// person decides. Smooth is the default because a minute is what most asks are worth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Surface {
    /// Surface nets: each vertex sits where the field actually crosses the threshold rather than
    /// on a cell boundary.
    #[default]
    Smooth,
    /// The same, on a grid twice as fine. Flat surfaces and thin detail stop showing the steps,
    /// and it costs about six times as long.
    Fine,
    /// The voxel grid as it is. Every step shows, on purpose.
    Blocky,
}

impl Surface {
    /// Which conversion node reads the voxels, and how it is told to read them.
    ///
    /// Asked of the server rather than remembered (2026-08-29): `VoxelToMesh` publishes
    /// `["surface net", "basic"]` and `VoxelToMeshBasic` publishes no algorithm at all.
    pub const fn algorithm(self) -> &'static str {
        match self {
            Surface::Smooth | Surface::Fine => "surface net",
            Surface::Blocky => "basic",
        }
    }

    /// The grid the shape is sampled on. The node's own range is 16 to 512.
    ///
    /// **256 unless somebody asked for the fine one.** Measured through the panel on this card:
    /// 62 s at 256 against 366 s at 512, for the same animal and a visibly cleaner base. That is
    /// a trade with two real sides, so it is a control rather than a constant.
    pub const fn octree(self) -> i64 {
        match self {
            Surface::Fine => 512,
            Surface::Smooth | Surface::Blocky => 256,
        }
    }
}

/// Asking for something that is not a still picture.
///
/// `None` on an [`Ask`] means a picture, and the graph is exactly what it was before this
/// existed — the same rule `upscale` and `control` follow.
///
/// **Two shapes because the two media are measured in different units**, not because they are
/// different kinds of request. A video has a length in frames and a rate; sound has a duration
/// and nothing else. A single struct carrying `frames`, `fps` *and* `seconds` would leave two
/// of the three meaningless in each case, which is three fields to keep true and two chances to
/// read the wrong one.
#[derive(Debug, Clone, PartialEq)]
pub enum Beyond {
    Moving {
        /// The family, by the name [`families`] offered.
        family: String,
        /// How many frames. **Not clamped here**: what a family accepts is a property of the
        /// family (LTXV wants `8n+1`) and the surface that knows which one was picked is the one
        /// that can round it. A number this file invented would be wrong for the next family.
        frames: i64,
        /// How fast they play, and — where the family has a conditioning node — what that node
        /// is told. One number, because two that could disagree is how a video ends up playing
        /// at a speed it was not made for.
        fps: f64,
    },
    Sound {
        family: String,
        /// How long, in seconds. It goes to the latent **and** to the conditioning, which is
        /// what `ConditioningStableAudio` is for: a graph that told them different numbers would
        /// generate one length and describe another.
        seconds: f64,
        /// The words that are **sung**, which are not the words that describe the song.
        ///
        /// **A music model is told two things and Epoch was only ever passing one.** ACE-Step's
        /// encoder takes `tags` and `lyrics` separately, and this field was an empty string in
        /// the composer with a comment saying so — correctly, because the panel had one box and
        /// splitting one prompt into two would have been Epoch writing words nobody typed.
        ///
        /// The fix is the panel's, not the composer's: a second box for a family whose encoder
        /// takes one. Empty stays perfectly valid — that is an instrumental.
        lyrics: String,
    },
    /// Something with three dimensions.
    ///
    /// **It is conditioned on a picture, not on words** — measured: `Hunyuan3Dv2Conditioning`
    /// takes a `CLIP_VISION_OUTPUT` and there is no text encoder in the graph at all. Which
    /// picture is the whole of what this carries.
    Shape {
        family: String,
        from: From3d,
        /// Smooth, or the voxel grid on purpose.
        surface: Surface,
    },
}

/// The four sides a mesh model can be shown, in the order the node takes them.
///
/// **Named views, not "several pictures".** Measured 2026-08-29:
/// `Hunyuan3Dv2ConditioningMultiView` has no required inputs and four optional ones — `front`,
/// `left`, `back`, `right` — each a `CLIP_VISION_OUTPUT`. A bag of images would have to guess
/// which is which, and the guess would be wrong half the time.
pub const SIDES: [&str; 4] = ["front", "left", "back", "right"];

/// One picture per side, in `SIDES` order. `None` is a side nobody supplied.
///
/// **Front alone is the ordinary case**, and it is the same request the single-view node takes —
/// which is why one filled slot composes the single-view graph and two or more compose the
/// multi-view one. The count is the person's choice; the node follows it.
pub type Views<T> = [Option<T>; 4];

/// Where the pictures a mesh is built from come from.
///
/// **Both, because the owner asked for both and both are real.** One is files the person already
/// has; the other is drawn in the same graph, in the same press, by a checkpoint they also
/// chose. Neither is Epoch picking a model — the panel asks for each.
#[derive(Debug, Clone, PartialEq)]
pub enum From3d {
    /// Pictures the person handed over, by the names the server answered with. The same confined
    /// path a ControlNet reference takes, one per side.
    Pictures(Views<String>),
    /// Drawn here, first, from words — one prompt per side.
    ///
    /// **A prompt per view rather than one with the side appended.** Epoch does not write words
    /// nobody typed: it refused to invent lyrics for a song and it refuses to invent *"…, seen
    /// from the left"* here. What a side should look like is the person's sentence.
    Drawn {
        checkpoint: String,
        prompts: Views<String>,
        negative: String,
        width: i64,
        height: i64,
        steps: i64,
        cfg: f64,
        seed: i64,
    },
}

impl From3d {
    /// How many sides were actually filled in. One is the single-view graph; more is multi-view.
    pub fn views(&self) -> usize {
        match self {
            From3d::Pictures(views) => views.iter().flatten().count(),
            From3d::Drawn { prompts, .. } => prompts
                .iter()
                .filter(|it| it.as_deref().is_some_and(|said| !said.trim().is_empty()))
                .count(),
        }
    }
}

impl Beyond {
    pub fn family(&self) -> &str {
        match self {
            Beyond::Moving { family, .. }
            | Beyond::Sound { family, .. }
            | Beyond::Shape { family, .. } => family,
        }
    }
}

impl Loading {
    /// The one file that names the model, whichever shape this is.
    ///
    /// For a graph that loads through `ImageOnlyCheckpointLoader` — a mesh — there is nothing
    /// else to load: the vision encoder and the VAE are inside the checkpoint, measured.
    pub fn checkpoint(&self) -> &str {
        match self {
            Loading::Checkpoint(file)
            | Loading::Encoded {
                checkpoint: file, ..
            }
            | Loading::Assembled { unet: file, .. } => file,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ask {
    pub loading: Loading,
    /// Each LoRA and how strongly, in the order they should apply. Chained, because that is what
    /// stacking two of them means.
    pub loras: Vec<(String, f64)>,
    pub prompt: String,
    /// Left empty unless somebody typed one. Epoch has no opinion about what pictures should not
    /// contain.
    pub negative: String,
    pub width: i64,
    pub height: i64,
    pub steps: i64,
    pub cfg: f64,
    pub seed: i64,
    pub batch: i64,
    /// Flux's own dial, and meaningless to anything else.
    ///
    /// Flux is trained without classifier-free guidance: its `cfg` is always 1.0 and the number
    /// that behaves like cfg is this one. Carried on the Ask rather than hidden in the recipe
    /// because it is a decision somebody makes, and defaulted so nobody has to.
    pub guidance: f64,
    /// An upscale model to run the finished picture through, by the name the server lists.
    ///
    /// `None` is the ordinary case and changes the graph not at all — the picture is saved
    /// straight off the decode, exactly as it was before this existed.
    ///
    /// A second sampler pass was the other shape and is not this: that changes what is *in* the
    /// picture, and this enlarges the one that was made. Somebody who asked for their picture
    /// bigger did not ask for a different picture.
    pub upscale: Option<String>,
    /// A picture to steer the composition by, and the ControlNet that reads it.
    ///
    /// `None` is the ordinary case and changes the graph not at all.
    ///
    /// **The user's, never the character's.** A ControlNet is a *file* and a reference picture is
    /// a *file*, and ADR-0033 settled where those are chosen: in the panel, by the person who
    /// owns the machine. `draw_image` gains nothing here — a character still asks for a mood.
    /// Each ControlNet and the picture it steers by, applied in order.
    ///
    /// **A list rather than one**, because they chain: every apply node takes a pair of
    /// conditionings and answers a pair, so the second reads what the first produced. A pose
    /// from one picture and a depth from another is what that is for.
    ///
    /// Empty is the ordinary case and changes the graph not at all. No ceiling is imposed here:
    /// ComfyUI has none, the card does, and a limit written into this file would be a number
    /// nobody measured (`CLAUDE.md` — a control assembled from what is conceivable offers
    /// combinations that do not exist).
    pub control: Vec<Steering>,
    /// **A picture this one is drawn on top of**, and how much of it survives.
    ///
    /// ## Why this exists and a ControlNet is not it
    ///
    /// Every picture Epoch has ever made began as noise: `latent_image` came from an empty
    /// latent and `denoise` was `1.0` on every sampler. So there was no way to say *this photo,
    /// in another style* — the one thing somebody asks for the moment they have a photo.
    ///
    /// A ControlNet is not the same thing and does not replace it. It carries **outlines**, so
    /// composition survives and everything else is reinvented; this carries the *picture*, and
    /// the dial says how much of it the model is allowed to overwrite. For a face, that
    /// difference is the whole question.
    ///
    /// `None` is a picture from nothing, which is what every other one is. The `f64` is
    /// ComfyUI's own `denoise`: **1.0 keeps nothing** and 0.0 keeps everything, so a low number
    /// is a light touch. The name is the one the server answered with when the picture was
    /// handed over — the same confined path a ControlNet reference takes, never a path from
    /// this machine (ADR-0024).
    pub from: Option<(String, f64)>,
    /// Something that is not a still picture: a video, or sound.
    ///
    /// `None` is the ordinary case and changes the graph not at all.
    pub beyond: Option<Beyond>,
}

/// The node classes Epoch knows turn a picture into a control map.
///
/// **A named set, filtered by what the server actually has** — never a category and never a
/// shape. Asked of this ComfyUI (2026-08-27), `Canny` sits in `image/filters` beside `ImageBlur`,
/// `ImageSharpen` and `Morphology`, and no category on the machine contains the word at all. A
/// list derived from *takes an IMAGE and answers an IMAGE* is 60 classes wide and mostly
/// hosted-API editors: a control assembled from what is conceivable offers combinations that do
/// not exist.
///
/// One entry, because one is what has been measured. `comfyui_controlnet_aux` publishes the
/// depth and pose preparations and is not installed here, and writing its class names from memory
/// is exactly the guess this codebase keeps deleting. The list grows the day one is measured.
pub const PREPARATIONS: [&str; 1] = ["Canny"];

/// Which of them this server has, in the order they are offered.
pub fn preparations(schema: &Schema) -> Vec<String> {
    PREPARATIONS
        .iter()
        .filter(|class| schema.classes.contains_key(**class))
        .map(|class| (*class).to_owned())
        .collect()
}

/// One ControlNet, its reference picture, and how hard it steers.
///
/// **`Steering` rather than `Control`**, because this module already has a `Control` and it is a
/// different thing entirely: a knob a surface offers. Two meanings on one word in one file is
/// how somebody reads the wrong one at four in the morning.
#[derive(Debug, Clone, PartialEq)]
pub struct Steering {
    /// The ControlNet model, by the name the server lists.
    pub model: String,
    /// The reference picture, **by the name ComfyUI answered with** after it was handed over.
    ///
    /// Never a path from this machine: a lent ComfyUI has its own disk, which is the whole
    /// reason `Comfy::hand_over` exists and returns a name rather than taking one.
    pub image: String,
    /// What turns the reference into something this ControlNet can read, by node class.
    ///
    /// `None` means **the picture is already a control map** — an edge drawing, a depth map, a
    /// pose — and it is handed over untouched.
    ///
    /// ## Why this is not optional, and why it is not guessed
    ///
    /// A canny ControlNet reads *edges*, not a photograph. Measured 2026-08-27 against this
    /// machine, one steering, same seed, one variable at a time:
    ///
    /// ```text
    /// no steering                    a picture
    /// one steering, raw reference    unusable noise
    /// the same without `vae`         the same noise — so the vae was never the cause
    /// the same through `Canny`       the composition of the reference, followed
    /// ```
    ///
    /// So handing the reference straight to the apply node was a defect, and it hid for as long
    /// as it did because the references it was first tested with were black-and-white shapes —
    /// already edge maps. A test that happens to supply prepared input proves the wiring and
    /// says nothing about what a person will actually drop in.
    ///
    /// **And Epoch may not choose this.** Which preparation a ControlNet wants is a property of
    /// the ControlNet, and the only thing naming it is the file name — which ADR-0024 forbids
    /// reading. Nor can the picture be measured: an edge map and a photograph are both an image.
    /// The person who picked the ControlNet is the one who knows, so they say, and `GENERATE`
    /// waits until they have. Drawing noise silently is the worse half of that trade.
    pub prepare: Option<String>,
    /// How strongly it steers. 1.0 is the server's own default.
    pub strength: f64,
    /// Where in the sampling it starts and stops steering, as fractions.
    ///
    /// Both here because `ControlNetApplyAdvanced` requires them — measured, they are not
    /// optional — and 0.0 to 1.0 is *the whole of it*, which is what somebody who has not
    /// thought about it means.
    pub start: f64,
    pub end: f64,
}

impl Ask {
    /// A plain ask for one picture from one checkpoint.
    pub fn of(checkpoint: &str, prompt: &str) -> Self {
        let (width, height) = trained_at(checkpoint);
        Self {
            loading: Loading::Checkpoint(checkpoint.to_owned()),
            loras: Vec::new(),
            upscale: None,
            control: Vec::new(),
            from: None,
            prompt: prompt.to_owned(),
            negative: String::new(),
            width: width.into(),
            height: height.into(),
            steps: 20,
            cfg: 7.0,
            seed: 0,
            batch: 1,
            guidance: 3.5,
            beyond: None,
        }
    }
}

/// Write the graph one [`Ask`] means, against the schema of the server that will run it.
///
/// ## Why this exists rather than a `.json` with holes in it
///
/// `starter` writes one plain graph. This writes **the graph a request implies** — which is the
/// piece that was missing, and the reason a downloaded LoRA was inert however correctly it landed:
/// measured 2026-08-23, `LoraLoader` appeared nowhere in this codebase, so there was no path from
/// a file on disk to a picture that did not go through hand-editing a workflow in ComfyUI.
///
/// ## Chained, not merged
///
/// Each LoRA takes the model **and** the CLIP of the one before it, so two of them stack the way
/// stacking means in ComfyUI. Wiring both from the checkpoint would silently apply only the last.
///
/// ## Compiled against *that* server
///
/// The classes are checked here, exactly as `starter` checks them, and for the reason a test once
/// caught: a graph that names a node this ComfyUI does not have should be refused at the door
/// rather than four minutes into a render.
/// The loader that reads however many encoder files were chosen.
///
/// **Shared by the two shapes that need one**, which is the whole reason it is a function: a
/// model that arrives in parts and a checkpoint whose encoder is not in it load their text
/// encoders identically, and two copies of this would be two places for `clip_name1` to be
/// spelled differently.
fn encoders(clip: &[String], clip_type: &str) -> Node {
    let mut inputs: Vec<(String, Input)> = clip
        .iter()
        .enumerate()
        .map(|(at, file)| {
            (
                // `clip_name` alone for one, `clip_name1..3` for the rest — which is how
                // ComfyUI names them, and the only reason this is not one loader.
                if clip.len() == 1 {
                    "clip_name".to_owned()
                } else {
                    format!("clip_name{}", at + 1)
                },
                Input::Value(file.clone().into()),
            )
        })
        .collect();
    // **Only where the loader has one.** `TripleCLIPLoader` takes three file names and nothing
    // else — measured, by asking it. An extra input is refused as surely as a wrong value, and
    // the empty string was never a valid `type` for anybody.
    if takes_a_type(clip.len()) && !clip_type.is_empty() {
        inputs.push(("type".to_owned(), Input::Value(clip_type.to_owned().into())));
    }
    Node {
        class_type: clip_loader(clip.len()).to_owned(),
        inputs: inputs.into_iter().collect(),
    }
}

/// The graph a mesh needs, which is **not** the one everything else needs.
///
/// ## Why this is its own function when video and sound were not
///
/// Those were the picture graph with three nodes swapped: same loader, same two text encoders,
/// same sampler, same decode-then-save. Sharing one `compose` was right because the thing being
/// shared really was shared, and a test asserts the picture graph is unchanged by either.
///
/// A mesh shares almost none of it, measured against this server:
///
/// - the loader is `ImageOnlyCheckpointLoader`, which answers a `CLIP_VISION` where the other
///   answers a `CLIP`;
/// - **there is no text encoder in it at all** — the conditioning comes from a *picture*,
///   through `CLIPVisionEncode` into `Hunyuan3Dv2Conditioning`;
/// - and when the picture is drawn here rather than handed over, the graph carries a second
///   complete model with its own sampler.
///
/// Forcing that through the shared chain means a sampler wired from a text encoder that never
/// runs, and two loaders where the code says one. **A shared function stops being right the
/// moment the thing it shares stops being shared**, and this is that moment.
///
/// The two ways in are the owner's, and both are the person's choice rather than Epoch's: a
/// picture they already have, or one drawn in the same press by a checkpoint they also picked.
/// The node that takes four named sides. Its own constant because two places name it: the
/// class check at the door, and the wiring below.
const MULTI_VIEW: &str = "Hunyuan3Dv2ConditioningMultiView";

fn shape_graph(
    ask: &Ask,
    family: Family,
    from: &From3d,
    surface: Surface,
    schema: &Schema,
) -> Result<serde_json::Value, Unreadable> {
    let mut core: Vec<&str> = vec![
        "ImageOnlyCheckpointLoader",
        "KSampler",
        family.encode,
        family.latent,
        family.decode,
    ];
    core.extend(family.conditioning);
    core.extend(family.keeping.nodes());
    // **The conditioning node follows the number of views, not the family.** One side is the
    // single-view node; two or more is the multi-view one, which takes them by name.
    let sides = from.views().max(1);
    if sides > 1 {
        core.push(MULTI_VIEW);
    }
    match from {
        From3d::Pictures(_) => core.push("LoadImage"),
        // The picture half is the ordinary picture graph, so it wants the ordinary classes.
        From3d::Drawn { .. } => core.extend([
            "CheckpointLoaderSimple",
            "CLIPTextEncode",
            "EmptyLatentImage",
            "VAEDecode",
        ]),
    }
    let missing: Vec<String> = core
        .iter()
        .filter(|class| !schema.classes.contains_key(**class))
        .map(|class| (*class).to_owned())
        .collect();
    if !missing.is_empty() {
        return Err(Unreadable::Missing(missing));
    }

    let wire = |node: &str, slot: i64| Input::From(vec![node.to_owned().into(), slot.into()]);
    let value = Input::Value;
    let node = |class: &str, inputs: Vec<(&str, Input)>| Node {
        class_type: class.to_owned(),
        inputs: inputs
            .into_iter()
            .map(|(name, input)| (name.to_owned(), input))
            .collect(),
    };
    let mut nodes: Vec<(String, Node)> = Vec::new();

    // Where each picture comes from. Both arms end at a wire per side, so nothing below knows
    // which — and the ids are blocked by side so a fourth view cannot collide with a third.
    let mut pictures: Vec<(String, usize)> = Vec::new();
    match from {
        From3d::Pictures(views) => {
            for (side, name) in views.iter().enumerate() {
                let Some(name) = name else { continue };
                let id = format!("{}", 100 + side * 10);
                nodes.push((
                    id.clone(),
                    node("LoadImage", vec![("image", value(name.clone().into()))]),
                ));
                pictures.push((id, side));
            }
        }
        From3d::Drawn {
            checkpoint,
            prompts,
            negative,
            width,
            height,
            steps,
            cfg,
            seed,
        } => {
            nodes.push((
                "20".to_owned(),
                node(
                    "CheckpointLoaderSimple",
                    vec![("ckpt_name", value(checkpoint.clone().into()))],
                ),
            ));
            // One negative for every side: it says what none of them should contain, and a
            // separate one per view would be four fields asking the same question.
            nodes.push((
                "22".to_owned(),
                node(
                    "CLIPTextEncode",
                    vec![
                        ("text", value(negative.clone().into())),
                        ("clip", wire("20", 1)),
                    ],
                ),
            ));
            nodes.push((
                "23".to_owned(),
                node(
                    "EmptyLatentImage",
                    vec![
                        ("width", value((*width).into())),
                        ("height", value((*height).into())),
                        ("batch_size", value(1.into())),
                    ],
                ),
            ));
            for (side, said) in prompts.iter().enumerate() {
                let Some(said) = said.as_deref().filter(|it| !it.trim().is_empty()) else {
                    continue;
                };
                let base = 100 + side * 10;
                let (words, drew, seen) = (
                    format!("{base}"),
                    format!("{}", base + 1),
                    format!("{}", base + 2),
                );
                nodes.push((
                    words.clone(),
                    node(
                        "CLIPTextEncode",
                        vec![
                            ("text", value(said.to_owned().into())),
                            ("clip", wire("20", 1)),
                        ],
                    ),
                ));
                nodes.push((
                    drew.clone(),
                    node(
                        "KSampler",
                        vec![
                            // **A seed per side.** One seed for four views draws the same
                            // picture four times, which is one view wearing four labels.
                            ("seed", value((*seed + side as i64).into())),
                            ("steps", value((*steps).into())),
                            ("cfg", value((*cfg).into())),
                            ("sampler_name", value("euler".into())),
                            ("scheduler", value("normal".into())),
                            ("denoise", value(1.0.into())),
                            ("model", wire("20", 0)),
                            ("positive", wire(&words, 0)),
                            ("negative", wire("22", 0)),
                            ("latent_image", wire("23", 0)),
                        ],
                    ),
                ));
                nodes.push((
                    seen.clone(),
                    node(
                        "VAEDecode",
                        vec![("samples", wire(&drew, 0)), ("vae", wire("20", 2))],
                    ),
                ));
                pictures.push((seen, side));
            }
        }
    }

    // The mesh half. `ImageOnlyCheckpointLoader` answers a model, a **vision** encoder and a VAE
    // — the vision encoder is inside the file, which is why nothing here asks for one.
    nodes.push((
        "1".to_owned(),
        node(
            "ImageOnlyCheckpointLoader",
            vec![("ckpt_name", value(ask.loading.checkpoint().into()))],
        ),
    ));
    // One vision encoder per side. The checkpoint carries the vision model, which is why
    // nothing here asks for one.
    let mut seen: Vec<(String, usize)> = Vec::new();
    for (from_node, side) in &pictures {
        let id = format!("{}", 103 + side * 10);
        nodes.push((
            id.clone(),
            node(
                family.encode,
                vec![
                    ("clip_vision", wire("1", 1)),
                    ("image", wire(from_node, 0)),
                    // Centre-cropped, which is the node's own default and the right one for a
                    // subject a person framed on purpose.
                    ("crop", value("center".into())),
                ],
            ),
        ));
        seen.push((id, *side));
    }
    // **The node follows the number of views.** One is the single-view conditioning; more is the
    // multi-view one, whose inputs are named sides rather than a list — measured, it has no
    // required inputs and four optional ones.
    let conditioning = if seen.len() > 1 {
        let mut inputs: Vec<(&str, Input)> = Vec::new();
        for (id, side) in &seen {
            inputs.push((SIDES[*side], wire(id, 0)));
        }
        node(MULTI_VIEW, inputs)
    } else {
        let only = seen.first().map(|(id, _)| id.clone()).unwrap_or_default();
        node(
            family.conditioning.unwrap_or("Hunyuan3Dv2Conditioning"),
            vec![("clip_vision_output", wire(&only, 0))],
        )
    };
    nodes.push(("10".to_owned(), conditioning));
    nodes.push((
        "4".to_owned(),
        node(
            family.latent,
            vec![
                // The node's own default. Its unit is not pixels and not seconds — it is how
                // finely the shape is sampled — and a number typed here would be one nobody
                // measured.
                ("resolution", value(3072.into())),
                ("batch_size", value(1.into())),
            ],
        ),
    ));
    nodes.push((
        "5".to_owned(),
        node(
            "KSampler",
            vec![
                ("seed", value(ask.seed.into())),
                // **The family's numbers, not the panel's.** The panel's steps and cfg belong to
                // the *picture* half of this graph and go there (`From3d::Drawn`); handing them
                // to the mesh sampler as well gave Hunyuan3D 20 steps at cfg 7 and produced a
                // blob — measured in the window, twice, before this line existed. Fifty at 5.0
                // is what was measured drawing a cow through the API, and it is the same kind of
                // number as `resolution: 3072` beside it: the family's, said once, here.
                ("steps", value(50.into())),
                ("cfg", value(5.0.into())),
                ("sampler_name", value("euler".into())),
                ("scheduler", value("normal".into())),
                ("denoise", value(1.0.into())),
                ("model", wire("1", 0)),
                ("positive", wire("10", 0)),
                ("negative", wire("10", 1)),
                ("latent_image", wire("4", 0)),
            ],
        ),
    ));
    nodes.push((
        "6".to_owned(),
        node(
            family.decode,
            vec![
                ("samples", wire("5", 0)),
                ("vae", wire("1", 2)),
                ("num_chunks", value(8000.into())),
                // The node's own default. It was 512 for an afternoon and was measured back
                // out — see `Surface::octree`.
                ("octree_resolution", value(surface.octree().into())),
            ],
        ),
    ));
    // **`surface net`, not `basic`** — the other half of why the models looked cubic.
    //
    // `VoxelToMeshBasic` walks the voxel grid and emits its faces, so the grid *is* the surface:
    // every step shows. `VoxelToMesh` publishes an `algorithm` with two options, and surface
    // nets place each vertex where the field actually crosses the threshold rather than on the
    // cell boundary. Measured, same picture and seed: the difference is a stepped silhouette
    // against a smooth one, and it costs nothing.
    //
    // Asked of the server rather than remembered: `["surface net", "basic"]`, in that order.
    nodes.push((
        "11".to_owned(),
        node(
            "VoxelToMesh",
            vec![
                ("voxel", wire("6", 0)),
                ("algorithm", value(surface.algorithm().into())),
                ("threshold", value(0.6.into())),
            ],
        ),
    ));
    nodes.push((
        "7".to_owned(),
        node(
            "SaveGLB",
            vec![
                ("filename_prefix", value("epoch".into())),
                ("mesh", wire("11", 0)),
            ],
        ),
    ));

    let workflow = Workflow {
        nodes: nodes.into_iter().collect(),
    };
    Ok(serde_json::to_value(&workflow.nodes).unwrap_or_default())
}

pub fn compose(ask: &Ask, schema: &Schema) -> Result<serde_json::Value, Unreadable> {
    // Assembled or all-in-one: the only difference in the graph is what fills the three sockets.
    //
    // **`Encoded` answers `None` here, and that is the point of it being its own shape.** This
    // value decides the sampler's settings — cfg, the scheduler, the sixteen-channel latent —
    // and every one of those follows the *checkpoint*, which an encoded one still is. It is a
    // checkpoint that borrows an encoder, not a model that arrives in parts.
    let assembled = match &ask.loading {
        Loading::Assembled {
            clip, clip_type, ..
        } => Some((clip.len(), clip_type.as_str())),
        Loading::Checkpoint(_) | Loading::Encoded { .. } => None,
    };
    // **Flux is the one family with a dial of its own.** Derived from the type rather than
    // carried as a flag: a second thing to keep true is a second thing to get wrong.
    let guided = matches!(assembled, Some((_, "flux")));

    // **A video is this graph with three nodes swapped**, and the swap is named once here so
    // the class check and the wiring below cannot disagree about which graph is being written.
    let beyond = match &ask.beyond {
        None => None,
        Some(asked) => Some((
            asked,
            family(asked.family())
                .ok_or_else(|| Unreadable::Missing(vec![asked.family().to_owned()]))?,
        )),
    };
    // A mesh is its own graph. See `shape_graph` for why this is a branch and not three more
    // fields on the one below.
    if let Some((Beyond::Shape { from, surface, .. }, family)) = &beyond {
        return shape_graph(ask, *family, from, *surface, schema);
    }

    let mut core: Vec<&str> = vec!["KSampler"];
    match &beyond {
        Some((_, family)) => {
            core.push(family.encode);
            core.push(family.latent);
            core.push(family.decode);
            core.extend(family.conditioning);
            core.extend(family.keeping.nodes());
        }
        None => core.extend(["CLIPTextEncode", "VAEDecode", "SaveImage"]),
    }
    match assembled {
        Some((encoders, _)) => {
            core.extend(["UNETLoader", "VAELoader"]);
            core.push(clip_loader(encoders));
            if guided {
                core.push("FluxGuidance");
            }
            if beyond.is_none() {
                // A sixteen-channel latent, which is what every family that ships in parts uses.
                // An older four-channel one refuses at the door, naming the node — which is a
                // refusal somebody can act on, and better than a table of channel counts that
                // would be wrong the week something new arrives.
                core.push("EmptySD3LatentImage");
            }
        }
        None => {
            core.push("CheckpointLoaderSimple");
            if let Loading::Encoded { clip, .. } = &ask.loading {
                core.push(clip_loader(clip.len()));
            }
            if beyond.is_none() {
                core.push("EmptyLatentImage");
            }
        }
    }
    if !ask.loras.is_empty() {
        core.push("LoraLoader");
    }
    // Two nodes, and only when one was asked for. A server without them says so by name, which
    // is a refusal somebody can act on.
    if ask.upscale.is_some() {
        core.extend(["UpscaleModelLoader", "ImageUpscaleWithModel"]);
    }
    // Three more, and the same rule. `ControlNetApplyAdvanced` rather than `ControlNetApply`
    // because the graph has two conditionings and the simple node takes one: steering the
    // positive prompt and leaving the negative unsteered is not what a ControlNet means.
    if !ask.control.is_empty() {
        core.extend(["ControlNetLoader", "LoadImage", "ControlNetApplyAdvanced"]);
    }
    // And the two a picture drawn on top of another one needs. Named at the door for the same
    // reason as everything else here: a refusal now beats a stack trace four minutes in.
    if ask.from.is_some() {
        core.extend(["LoadImage", "VAEEncode"]);
    }
    // And whatever prepares each reference, which is only ever a class the server listed — but
    // checked here anyway, because a workflow kept from yesterday can name a node uninstalled
    // since. Refused by name at the door beats a stack trace four minutes into a render.
    for control in &ask.control {
        if let Some(class) = &control.prepare {
            core.push(class.as_str());
        }
    }
    let missing: Vec<String> = core
        .iter()
        .filter(|class| !schema.classes.contains_key(**class))
        .map(|class| (*class).to_owned())
        .collect();
    if !missing.is_empty() {
        return Err(Unreadable::Missing(missing));
    }

    let wire = |node: String, slot: i64| Input::From(vec![node.into(), slot.into()]);
    let value = Input::Value;
    let node = |class: &str, inputs: Vec<(&str, Input)>| Node {
        class_type: class.to_owned(),
        inputs: inputs
            .into_iter()
            .map(|(name, input)| (name.to_owned(), input))
            .collect(),
    };

    // Where the model, the CLIP and the VAE come from. Three sockets, filled by one node or by
    // three, and everything downstream is written the same either way.
    let mut nodes: Vec<(String, Node)> = Vec::new();
    let (mut model, mut clip, vae) = match &ask.loading {
        Loading::Checkpoint(file) => {
            nodes.push((
                "1".to_owned(),
                node(
                    "CheckpointLoaderSimple",
                    vec![("ckpt_name", value(file.clone().into()))],
                ),
            ));
            (
                ("1".to_owned(), 0),
                ("1".to_owned(), 1),
                ("1".to_owned(), 2),
            )
        }
        // The model and the VAE off the file; the CLIP off a loader beside it, because the
        // file's own is `None` and wiring it draws nothing but a refusal.
        Loading::Encoded {
            checkpoint,
            clip,
            clip_type,
        } => {
            nodes.push((
                "1".to_owned(),
                node(
                    "CheckpointLoaderSimple",
                    vec![("ckpt_name", value(checkpoint.clone().into()))],
                ),
            ));
            nodes.push(("8".to_owned(), encoders(clip, clip_type)));
            (
                ("1".to_owned(), 0),
                ("8".to_owned(), 0),
                ("1".to_owned(), 2),
            )
        }
        Loading::Assembled {
            unet,
            clip,
            clip_type,
            vae,
        } => {
            nodes.push((
                "1".to_owned(),
                node(
                    "UNETLoader",
                    vec![
                        ("unet_name", value(unet.clone().into())),
                        // The file decides its own precision. Asking for fp8 on weights that are
                        // already fp8 is a second opinion nobody asked for.
                        ("weight_dtype", value("default".into())),
                    ],
                ),
            ));
            nodes.push(("8".to_owned(), encoders(clip, clip_type)));
            nodes.push((
                "9".to_owned(),
                node("VAELoader", vec![("vae_name", value(vae.clone().into()))]),
            ));
            (
                ("1".to_owned(), 0),
                ("8".to_owned(), 0),
                ("9".to_owned(), 0),
            )
        }
    };
    // **Every node a request adds gets its id from here, and hand-numbering is why.**
    //
    // The fixed graph is `1`–`10`. Everything optional used to write its own numbers: LoRAs from
    // `11`, the upscale pair at `11` and `12`, the steerings from `13`. Two of those overlap and
    // one overlaps at three LoRAs — measured 2026-08-27 by drawing with a LoRA *and* an upscale,
    // which produced no picture and a refusal nobody could read: `11` became the
    // `UpscaleModelLoader` because it was pushed last, both `CLIPTextEncode`s were still wired to
    // its slot `1`, and ComfyUI answered `list index out of range` on a node that had one output.
    //
    // A counter cannot collide with itself. Three blocks of numbers can, and the arithmetic that
    // says whether they do is exactly the thing nobody re-checks when a fourth is added.
    let mut last_id = 10u32;
    let mut fresh = move || {
        last_id += 1;
        last_id.to_string()
    };
    for (file, strength) in &ask.loras {
        let id = fresh();
        nodes.push((
            id.clone(),
            node(
                "LoraLoader",
                vec![
                    ("lora_name", value(file.clone().into())),
                    // One number in the panel, two on the node. A separate control for the text
                    // encoder is a knob nobody asked for and a way for the two to disagree.
                    ("strength_model", value((*strength).into())),
                    ("strength_clip", value((*strength).into())),
                    ("model", wire(model.0.clone(), model.1)),
                    ("clip", wire(clip.0.clone(), clip.1)),
                ],
            ),
        ));
        model = (id.clone(), 0);
        clip = (id, 1);
    }

    // What turns words into a conditioning, and **which words that node takes**.
    //
    // `CLIPTextEncode` takes a `text`; `TextEncodeAceStepAudio` takes `tags` and `lyrics` —
    // measured, and not a rename: a music model is told a genre and a vocal separately, and its
    // `lyrics` is empty here because the panel asks for one prompt and inventing a second from
    // it would be Epoch writing words nobody typed.
    let encode = match &beyond {
        Some((_, family)) => family.encode,
        None => "CLIPTextEncode",
    };
    // What is **sung**, as opposed to what describes the song. Empty for everything that is not
    // a music model, and legitimately empty for a music model too — that is an instrumental.
    let (sung, how_long) = match &beyond {
        Some((
            Beyond::Sound {
                lyrics, seconds, ..
            },
            _,
        )) => (lyrics.clone(), *seconds),
        _ => (String::new(), 0.0),
    };
    let words = |said: &str| -> Vec<(String, Input)> {
        let named = |name: &str, it: Input| (name.to_owned(), it);
        if encode == "TextEncodeAceStepAudio" {
            vec![
                named("tags", value(said.to_owned().into())),
                named("lyrics", value(sung.clone().into())),
                named("lyrics_strength", value(1.0.into())),
                named("clip", wire(clip.0.clone(), clip.1)),
            ]
        } else if encode == "TextEncodeAceStepAudio1.5" {
            // **Three things Epoch knows, and everything else from the server's own default.**
            //
            // It knows the tags, the lyrics and how long the song is — `duration` here is the
            // same number the latent is given, because a graph that told them different numbers
            // would generate one length and describe another. The rest — a key, a time
            // signature, a tempo, the sampling knobs on the language model — are values a person
            // would have to be asked for, and the node declares one for each. Same discipline as
            // a preparation's thresholds: read them, never invent them.
            let mut inputs = vec![
                named("tags", value(said.to_owned().into())),
                named("lyrics", value(sung.clone().into())),
                named("duration", value(how_long.into())),
                named("seed", value(ask.seed.into())),
                named("clip", wire(clip.0.clone(), clip.1)),
            ];
            if let Some(widgets) = schema.classes.get(encode) {
                for widget in &widgets.order {
                    if inputs.iter().any(|(name, _)| *name == widget.name) {
                        continue;
                    }
                    if let Some(given) = widget.declared() {
                        inputs.push((widget.name.clone(), value(given)));
                    }
                }
            }
            inputs
        } else {
            vec![
                named("text", value(said.to_owned().into())),
                named("clip", wire(clip.0.clone(), clip.1)),
            ]
        }
    };
    let named_node = |class: &str, inputs: Vec<(String, Input)>| Node {
        class_type: class.to_owned(),
        inputs: inputs.into_iter().collect(),
    };
    nodes.push(("2".to_owned(), named_node(encode, words(&ask.prompt))));
    nodes.push(("3".to_owned(), named_node(encode, words(&ask.negative))));
    // Flux's positive prompt goes through its own dial on the way to the sampler.
    let positive = if guided {
        nodes.push((
            "10".to_owned(),
            node(
                "FluxGuidance",
                vec![
                    ("conditioning", wire("2".to_owned(), 0)),
                    ("guidance", value(ask.guidance.into())),
                ],
            ),
        ));
        ("10".to_owned(), 0)
    } else {
        ("2".to_owned(), 0)
    };

    // Where the sampler starts, and it is measured in whatever the medium is measured in.
    //
    // A picture's latent takes a width, a height and a batch. A video's takes a `length` in
    // frames as well. **Sound's takes neither a width nor a height** — it is `seconds` and a
    // batch, and sending pixels to it would be an unknown input, which ComfyUI refuses as firmly
    // as a wrong value.
    let latent = match &beyond {
        Some((Beyond::Sound { seconds, .. }, _)) => vec![
            ("seconds", value((*seconds).into())),
            ("batch_size", value(ask.batch.max(1).into())),
        ],
        other => {
            let mut inputs = vec![
                ("width", value(ask.width.into())),
                ("height", value(ask.height.into())),
                ("batch_size", value(ask.batch.max(1).into())),
            ];
            if let Some((Beyond::Moving { frames, .. }, _)) = other {
                inputs.push(("length", value((*frames).max(1).into())));
            }
            inputs
        }
    };
    nodes.push((
        "4".to_owned(),
        node(
            match &beyond {
                Some((_, family)) => family.latent,
                None if assembled.is_some() => "EmptySD3LatentImage",
                None => "EmptyLatentImage",
            },
            latent,
        ),
    ));
    // The reference picture steers both sides of the conditioning, on its way to the sampler.
    //
    // **Measured against the server** (2026-08-27, this ComfyUI's `/object_info`):
    // `ControlNetLoader` takes `control_net_name` and gives a `CONTROL_NET`; `LoadImage` takes
    // the name the server itself answered with and gives an `IMAGE`; `ControlNetApplyAdvanced`
    // takes both plus `positive`, `negative`, `strength`, `start_percent` and `end_percent`,
    // and gives **two** conditionings back.
    //
    // `vae` is wired because the node accepts one and the graph already has it — every family
    // that ships in parts needs it, and passing what is already there is not a guess. A
    // ControlNet that does not want it ignores it.
    // **They chain, because the node's shape says so.** Each apply takes a pair of conditionings
    // and answers a pair, so the second reads what the first produced and the sampler sees the
    // last one.
    let mut positive = positive;
    let mut negative = ("3".to_owned(), 0);
    for control in &ask.control {
        let loader = fresh();
        let picture = fresh();
        let apply = fresh();
        nodes.push((
            loader.clone(),
            node(
                "ControlNetLoader",
                vec![("control_net_name", value(control.model.clone().into()))],
            ),
        ));
        nodes.push((
            picture.clone(),
            node(
                "LoadImage",
                vec![("image", value(control.image.clone().into()))],
            ),
        ));
        // What the ControlNet actually reads. A canny one reads edges, and handing it the
        // photograph draws noise — measured, and the reason this step exists.
        //
        // **Every other value comes from the server's own default.** `Canny` takes two
        // thresholds and declares `0.4` and `0.8` for them; asking a person for numbers they
        // have no way to choose between is how a panel stops being a panel. A value the server
        // declares no default for is left out, so it refuses by name rather than by guess.
        let read_from = match &control.prepare {
            None => (picture, 0),
            Some(class) => {
                let prepared = fresh();
                let mut inputs = vec![("image".to_owned(), wire(picture, 0))];
                if let Some(widgets) = schema.classes.get(class) {
                    for widget in &widgets.order {
                        if widget.name == "image" {
                            continue;
                        }
                        if let Some(given) = widget.declared() {
                            inputs.push((widget.name.clone(), value(given)));
                        }
                    }
                }
                nodes.push((
                    prepared.clone(),
                    Node {
                        class_type: class.clone(),
                        inputs: inputs.into_iter().collect(),
                    },
                ));
                (prepared, 0)
            }
        };
        nodes.push((
            apply.clone(),
            node(
                "ControlNetApplyAdvanced",
                vec![
                    ("positive", wire(positive.0.clone(), positive.1)),
                    ("negative", wire(negative.0.clone(), negative.1)),
                    ("control_net", wire(loader, 0)),
                    ("image", wire(read_from.0, read_from.1)),
                    ("strength", value(control.strength.into())),
                    ("start_percent", value(control.start.into())),
                    ("end_percent", value(control.end.into())),
                    ("vae", wire(vae.0.clone(), vae.1)),
                ],
            ),
        ));
        positive = (apply.clone(), 0);
        negative = (apply, 1);
    }
    // The last thing between the conditionings and the sampler, where a video family has one.
    //
    // **After the ControlNet chain deliberately.** Each apply answers a pair of conditionings
    // and this takes a pair, so putting it last means the family node sees whatever steering
    // produced — and a family that carries a frame rate must carry it on the conditioning the
    // sampler actually reads, not on one that was replaced afterwards.
    let (positive, negative) = match &beyond {
        Some((
            asked,
            Family {
                conditioning: Some(class),
                ..
            },
        )) => {
            let id = fresh();
            let mut inputs = vec![
                ("positive".to_owned(), wire(positive.0.clone(), positive.1)),
                ("negative".to_owned(), wire(negative.0.clone(), negative.1)),
            ];
            // **Each family's own dial, and it is the medium's unit.** Measured against this
            // server: `LTXVConditioning` takes a `frame_rate`; `ConditioningStableAudio` takes a
            // `seconds_start` and a `seconds_total` — the same duration the latent was given,
            // because a graph that told them different numbers would generate one length and
            // describe another.
            match asked {
                Beyond::Moving { fps, .. } => {
                    inputs.push(("frame_rate".to_owned(), value((*fps).into())));
                }
                Beyond::Sound { seconds, .. } => {
                    inputs.push(("seconds_start".to_owned(), value(0.0.into())));
                    inputs.push(("seconds_total".to_owned(), value((*seconds).into())));
                }
                // Unreachable: a shape leaves through `shape_graph` above, before any of this.
                Beyond::Shape { .. } => {}
            }
            nodes.push((
                id.clone(),
                Node {
                    class_type: (*class).to_owned(),
                    inputs: inputs.into_iter().collect(),
                },
            ));
            ((id.clone(), 0), (id, 1))
        }
        _ => (positive, negative),
    };

    /*
        **A picture to draw on top of, encoded into the latent the sampler starts from.**

        Two nodes and no new mechanism: `LoadImage` is how a reference already arrives, and
        `VAEEncode` is its opposite number to the `VAEDecode` that ends every one of these
        graphs. The empty latent stays where there is nothing to start from, which is every
        other picture.

        The VAE is the same one the decode uses — there is exactly one in this graph, and
        encoding with a different one than you decode with is how a picture comes back with its
        colours shifted.
    */
    let drawn_on = match &ask.from {
        None => ("4".to_owned(), 0),
        Some((picture, _)) => {
            let loaded = fresh();
            let encoded = fresh();
            nodes.push((
                loaded.clone(),
                node("LoadImage", vec![("image", value(picture.clone().into()))]),
            ));
            nodes.push((
                encoded.clone(),
                node(
                    "VAEEncode",
                    vec![
                        ("pixels", wire(loaded, 0)),
                        ("vae", wire(vae.0.clone(), vae.1)),
                    ],
                ),
            ));
            (encoded, 0)
        }
    };

    nodes.push((
        "5".to_owned(),
        node(
            "KSampler",
            vec![
                ("seed", value(ask.seed.into())),
                ("steps", value(ask.steps.into())),
                // **Neither Flux nor Z-Image Turbo uses classifier-free guidance.** Flux is
                // trained without it and Turbo is distilled: cfg is 1.0 for both, and anything
                // else burns the image. Flux's `guidance` is the number that behaves like cfg;
                // Z-Image has none at all.
                (
                    "cfg",
                    value(if assembled.is_some() { 1.0 } else { ask.cfg }.into()),
                ),
                ("sampler_name", value("euler".into())),
                (
                    "scheduler",
                    value(
                        if assembled.is_some() {
                            "simple"
                        } else {
                            "normal"
                        }
                        .into(),
                    ),
                ),
                /*
                    **Where the picture starts, and how much of it survives.**

                    Every picture Epoch made until now began as noise: an empty latent and
                    `denoise: 1.0`. With a picture handed over, the latent is *that picture*
                    encoded, and the dial is how much the model may overwrite — ComfyUI's own
                    `denoise`, where 1.0 keeps nothing and a low number is a light touch.

                    It is deliberately the same node the ControlNet reference uses to arrive
                    (`LoadImage`, by the name the server answered with) and deliberately not the
                    same *thing*: a ControlNet carries outlines and reinvents the rest; this
                    carries the picture.
                */
                (
                    "denoise",
                    value(ask.from.as_ref().map_or(1.0, |(_, how)| *how).into()),
                ),
                ("model", wire(model.0.clone(), model.1)),
                ("positive", wire(positive.0.clone(), positive.1)),
                ("negative", wire(negative.0.clone(), negative.1)),
                ("latent_image", wire(drawn_on.0.clone(), drawn_on.1)),
            ],
        ),
    ));
    nodes.push((
        "6".to_owned(),
        node(
            match &beyond {
                Some((_, family)) => family.decode,
                None => "VAEDecode",
            },
            vec![
                ("samples", wire("5".to_owned(), 0)),
                ("vae", wire(vae.0.clone(), vae.1)),
            ],
        ),
    ));
    // The finished picture, enlarged before it is saved.
    //
    // **Measured against the server rather than remembered** (2026-08-25): `UpscaleModelLoader`
    // takes `model_name` and gives an `UPSCALE_MODEL`; `ImageUpscaleWithModel` takes that and an
    // `image`. Both were read from this ComfyUI's own `/object_info`, which is the same thing
    // every other class in this function is checked against.
    let saving = match &ask.upscale {
        None => ("6".to_owned(), 0),
        Some(file) => {
            let loader = fresh();
            let enlarge = fresh();
            nodes.push((
                loader.clone(),
                node(
                    "UpscaleModelLoader",
                    vec![("model_name", value(file.clone().into()))],
                ),
            ));
            nodes.push((
                enlarge.clone(),
                node(
                    "ImageUpscaleWithModel",
                    vec![
                        ("upscale_model", wire(loader, 0)),
                        ("image", wire("6".to_owned(), 0)),
                    ],
                ),
            ));
            (enlarge, 0)
        }
    };
    match &beyond {
        // **Sound goes straight to a file.** Measured: `SaveAudio` takes an `AUDIO` and a prefix
        // and writes FLAC, which every Chromium plays — and this window is one. `SaveAudioMP3`
        // and `SaveAudioOpus` exist and are not offered, because a format choice with no
        // measured reason behind it is a control nobody can answer.
        Some((_, family)) if family.keeping != Keeping::Video => {
            nodes.push((
                "7".to_owned(),
                node(
                    "SaveAudio",
                    vec![
                        ("filename_prefix", value("epoch".into())),
                        ("audio", wire(saving.0.clone(), saving.1)),
                    ],
                ),
            ));
        }
        // **Measured against this server rather than remembered** (2026-08-28): `CreateVideo`
        // takes `images` and an `fps` and answers a VIDEO; `SaveVideo` takes that, a prefix, a
        // `format` published as `[auto, mp4]` and a `codec` published as `[auto, h264]`. `auto`
        // for both, because choosing between one real option and the server's own default is a
        // control with nothing behind it.
        Some((asked, _)) => {
            let fps = match asked {
                Beyond::Moving { fps, .. } => *fps,
                Beyond::Sound { .. } | Beyond::Shape { .. } => {
                    unreachable!("sound is kept above and a shape never reaches here")
                }
            };
            let muxed = fresh();
            nodes.push((
                muxed.clone(),
                node(
                    "CreateVideo",
                    vec![
                        ("images", wire(saving.0.clone(), saving.1)),
                        ("fps", value(fps.into())),
                    ],
                ),
            ));
            nodes.push((
                "7".to_owned(),
                node(
                    "SaveVideo",
                    vec![
                        ("filename_prefix", value("epoch".into())),
                        ("format", value("auto".into())),
                        ("codec", value("auto".into())),
                        ("video", wire(muxed, 0)),
                    ],
                ),
            ));
        }
        None => nodes.push((
            "7".to_owned(),
            node(
                "SaveImage",
                vec![
                    ("filename_prefix", value("epoch".into())),
                    ("images", wire(saving.0.clone(), saving.1)),
                ],
            ),
        )),
    }

    let workflow = Workflow {
        nodes: nodes.into_iter().collect(),
    };
    // The nodes, not the wrapper: API format is a flat map of ids, which is what `starter` emits
    // and what the server executes. Serialising `Workflow` itself would nest everything under
    // `nodes` and be refused for having no node called `nodes`.
    Ok(serde_json::to_value(&workflow.nodes).unwrap_or_default())
}

pub fn starter(checkpoint: &str, schema: &Schema) -> Result<serde_json::Value, Unreadable> {
    const CORE: [&str; 6] = [
        "CheckpointLoaderSimple",
        "CLIPTextEncode",
        "EmptyLatentImage",
        "KSampler",
        "VAEDecode",
        "SaveImage",
    ];
    let missing: Vec<String> = CORE
        .iter()
        .filter(|class| !schema.classes.contains_key(**class))
        .map(|class| (*class).to_owned())
        .collect();
    if !missing.is_empty() {
        return Err(Unreadable::Missing(missing));
    }

    let (width, height) = trained_at(checkpoint);
    let wire = |node: &str, slot: i64| Input::From(vec![node.into(), slot.into()]);
    let value = |v: serde_json::Value| Input::Value(v);
    let node = |class: &str, inputs: Vec<(&str, Input)>| Node {
        class_type: class.to_owned(),
        inputs: inputs
            .into_iter()
            .map(|(name, input)| (name.to_owned(), input))
            .collect(),
    };

    let workflow = Workflow {
        nodes: [
            (
                "1".to_owned(),
                node(
                    "CheckpointLoaderSimple",
                    vec![("ckpt_name", value(checkpoint.into()))],
                ),
            ),
            // The two prompts. Epoch fills the positive one on every run — `say()` finds it by
            // being the one wired into the sampler's `positive` — and leaves the negative empty
            // rather than inventing a house opinion about what pictures should not contain.
            (
                "2".to_owned(),
                node(
                    "CLIPTextEncode",
                    vec![("text", value("".into())), ("clip", wire("1", 1))],
                ),
            ),
            (
                "3".to_owned(),
                node(
                    "CLIPTextEncode",
                    vec![("text", value("".into())), ("clip", wire("1", 1))],
                ),
            ),
            (
                "4".to_owned(),
                node(
                    "EmptyLatentImage",
                    vec![
                        ("width", value(width.into())),
                        ("height", value(height.into())),
                        ("batch_size", value(1.into())),
                    ],
                ),
            ),
            (
                "5".to_owned(),
                node(
                    "KSampler",
                    vec![
                        ("seed", value(0.into())),
                        // ComfyUI's own default for an ordinary checkpoint. `harder()` scales
                        // this rather than replacing it, so *fine* means more of whatever the
                        // workflow already asked for.
                        ("steps", value(20.into())),
                        ("cfg", value(7.0.into())),
                        ("sampler_name", value("euler".into())),
                        ("scheduler", value("normal".into())),
                        ("denoise", value(1.0.into())),
                        ("model", wire("1", 0)),
                        ("positive", wire("2", 0)),
                        ("negative", wire("3", 0)),
                        ("latent_image", wire("4", 0)),
                    ],
                ),
            ),
            (
                "6".to_owned(),
                node(
                    "VAEDecode",
                    vec![("samples", wire("5", 0)), ("vae", wire("1", 2))],
                ),
            ),
            (
                "7".to_owned(),
                node(
                    "SaveImage",
                    vec![
                        ("filename_prefix", value("epoch".into())),
                        ("images", wire("6", 0)),
                    ],
                ),
            ),
        ]
        .into_iter()
        .collect(),
    };

    // Emitted as API-format JSON so it goes in through the same door an exported file does, and
    // so the file kept as `source` is the one a person could open in ComfyUI themselves.
    Ok(serde_json::to_value(&workflow.nodes).unwrap_or_default())
}

/// The size a checkpoint was trained at, as far as its name says.
///
/// **A heuristic, and labelled one.** SDXL draws badly at 512 and SD 1.5 draws badly at 1024, so
/// a single number would be wrong half the time; the name is the only thing available without
/// opening a seven-gigabyte file. What would replace it is reading the safetensors header — SDXL
/// carries a second text encoder and says so in its tensor names — and that is a real
/// measurement rather than a guess, worth doing the day this is wrong for somebody.
///
/// It is not hidden either way: the built workflow's name carries the size, so a wrong guess is
/// visible on the deck rather than discovered in a bad picture.
fn trained_at(checkpoint: &str) -> (u32, u32) {
    let name = checkpoint.to_ascii_lowercase();
    let big = ["xl", "sd3", "sd35", "flux", "qwen", "chroma", "lumina"]
        .iter()
        .any(|marker| name.contains(marker));
    if big {
        (1024, 1024)
    } else {
        (512, 512)
    }
}

impl Imported {
    /// Take a workflow in, in whichever shape it arrived.
    pub fn of(source: serde_json::Value, schema: &Schema) -> Result<Self, Unreadable> {
        let compiled = read(&source, schema)?;
        let opening = compiled.opening();
        Ok(Self {
            source,
            compiled,
            opening,
        })
    }

    /// Compile it again — after a custom node was installed, or ComfyUI changed.
    ///
    /// This is the whole reason the source is kept. It also means a workflow that could not be
    /// read at all is still worth storing, so *install the node and press again* is possible.
    pub fn recompile(&mut self, schema: &Schema) -> Result<(), Unreadable> {
        self.compiled = read(&self.source, schema)?;
        self.opening = self.compiled.opening();
        Ok(())
    }
}

/// One value a node takes, as the server describes it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Widget {
    pub name: String,
    /// `INT`, `FLOAT`, `STRING`, `BOOLEAN`, or `CHOICE` when it is a list.
    pub kind: String,
    /// The options, when it is a list. What a surface would draw as a dropdown.
    pub choices: Vec<String>,
    pub default: Option<serde_json::Value>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    /// Whether it takes more than one line — the difference between a field and a box.
    pub multiline: bool,
    /// **The server's own sentence about it.** Written by whoever wrote the node, which makes
    /// it better than anything Epoch could invent, and it is never invented when absent.
    pub help: Option<String>,
    /// Choosing this value adds more values after it.
    ///
    /// **A dynamic combo is not one widget.** `CreateCameraInfo.mode` is
    /// `COMFY_DYNAMICCOMBO_V3`, and picking `orbit` grows `mode.yaw`, `mode.pitch` and
    /// `mode.distance` beside it — all of them saved into `widgets_values` in positions nothing
    /// in `input_order` accounts for. Measured 2026-08-22: the server answered
    /// `required_input_missing: mode.yaw` and `camera_type: 0 not in ['perspective',
    /// 'orthographic']` on a workflow that read perfectly.
    ///
    /// The server declares exactly what each choice adds, so this is read rather than refused:
    /// `options: [{ key: "scale dimensions", inputs: { required: { width, height, crop } } }]`,
    /// and the sub-values follow the chosen one in the saved list. They are named the way the
    /// server names them in its own errors — `mode.yaw`, `resize_type.crop`.
    pub expands: bool,
    /// What each choice grows, by the choice's own name.
    pub grows: BTreeMap<String, Vec<Widget>>,
    /// This value is followed in the editor by a `control_after_generate` selector that is not
    /// an input at all.
    ///
    /// **Declared, not guessed.** The first version matched on the name containing `seed`,
    /// which worked and was a heuristic sitting next to the fact: `/object_info` says
    /// `"control_after_generate": true` on exactly those inputs.
    pub then_a_control: bool,
}

impl Widget {
    /// The value this widget takes when nobody sets it — **the server's, never Epoch's**.
    ///
    /// ## A combo declares its answer as a list, not as a default
    ///
    /// Measured 2026-08-29, and it was a refusal first. `TextEncodeAceStepAudio1.5` declares
    /// `timesignature` and `keyscale` as `IO.Combo.Input(options=[...])` with **no `default`**,
    /// so a graph built from `default` alone omitted them and ComfyUI answered
    /// `required_input_missing: timesignature` after the form had been filled in perfectly.
    ///
    /// The first option *is* the declared answer — it is what ComfyUI's own editor puts in the
    /// box when a node is added. So this is still read rather than invented, and the alternative
    /// (a value typed here) would be wrong for the next node that publishes a different list.
    ///
    /// `None` stays `None` for a widget that declares neither, and the graph then refuses **by
    /// name** rather than by guess.
    pub fn declared(&self) -> Option<serde_json::Value> {
        self.default
            .clone()
            .or_else(|| self.choices.first().map(|it| it.clone().into()))
    }
}

/// What one node class takes, as far as reading a UI workflow needs to know.
#[derive(Debug, Clone, Default)]
pub struct Widgets {
    /// Widget inputs, in the order the editor lays them out.
    pub order: Vec<Widget>,
    /// The server published an order for this class.
    ///
    /// **The difference between *takes no values* and *will not say in what order*.** An empty
    /// `order` means both, and they are opposite situations: a node with nothing to set has
    /// nothing to misalign, while a node whose order is unknown would be aligned by guesswork.
    /// Conflating them refused 38 workflows over `PreviewAny`, whose only input is a wildcard
    /// socket the editor keeps display state beside.
    pub told: bool,
}

/// The server's node schema, reduced to what conversion needs.
///
/// Built from `/object_info`. Reduced rather than kept whole because that answer is several
/// megabytes and this needs one list per class.
#[derive(Debug, Clone, Default)]
pub struct Schema {
    pub classes: BTreeMap<String, Widgets>,
}

impl Schema {
    /// Read `/object_info` into the ordering conversion needs.
    ///
    /// A widget is an input whose type is a primitive or a list of choices; anything else is a
    /// socket another node plugs into. That distinction is the server's, not ours.
    ///
    /// ## The order comes from `input_order`, and it has to
    ///
    /// **Measured 2026-08-22, and it was a defect first.** The prototype walked the `input`
    /// object in document order and worked; the same code in Rust produced
    /// `cfg: 721897303308196` and the server refused it, listing every reason. `serde_json`'s
    /// map is a `BTreeMap` — it sorts keys alphabetically — so document order does not survive
    /// parsing at all, and `add_noise, cfg, end_at_step, …` is not the order anything is laid
    /// out in.
    ///
    /// ComfyUI publishes `input_order` beside `input` for exactly this. Asked rather than
    /// reconstructed: a server that says what it means is better than a parser that guesses,
    /// and this one says it.
    ///
    /// A class whose order the server does not publish gets **no widget order at all** rather
    /// than a plausible one. `from_ui` then refuses that workflow instead of aligning values
    /// against a guess — the difference between saying nothing and saying something wrong.
    pub fn read(object_info: &serde_json::Value) -> Self {
        let mut classes = BTreeMap::new();
        let Some(all) = object_info.as_object() else {
            return Self { classes };
        };

        // **A socket is a type something produces. Everything else is a value.**
        //
        // Measured 2026-08-22, and it replaced a list of names that was already wrong: reading
        // only `INT | FLOAT | STRING | BOOLEAN` and inline choice lists missed
        // `COMFY_DYNAMICCOMBO_V3`, which sits *first* on `CreateCameraInfo` — so every later
        // value shifted by one and the server was handed `target_x: "orbit"`, `fov: 0.0`,
        // `camera_type: 0`.
        //
        // Rather than add a name and wait for the next one, the question is asked of the whole
        // schema: 26 input types are produced by no node at all — `COMBO`, the dynamic combos,
        // `LOAD_3D`, `WEBCAM`, the `FILE_3D_*` unions — and a type nothing produces cannot be
        // on the end of a wire.
        let produced: std::collections::BTreeSet<&str> = all
            .values()
            .filter_map(|spec| spec.get("output").and_then(|o| o.as_array()))
            .flatten()
            .filter_map(|kind| kind.as_str())
            .collect();
        for (class, spec) in all {
            let mut order = Vec::new();
            for section in ["required", "optional"] {
                let Some(names) = spec
                    .get("input_order")
                    .and_then(|o| o.get(section))
                    .and_then(|s| s.as_array())
                else {
                    continue;
                };
                let inputs = spec.get("input").and_then(|i| i.get(section));
                for name in names.iter().filter_map(|n| n.as_str()) {
                    let Some(definition) = inputs.and_then(|i| i.get(name)) else {
                        continue;
                    };
                    if let Some(widget) = one_widget(name, definition, &produced) {
                        order.push(widget);
                    }
                }
            }
            let told = spec.get("input_order").is_some();
            classes.insert(class.clone(), Widgets { order, told });
        }
        Self { classes }
    }
}

/// What each choice of a growing setting adds, in the order the values are saved in.
///
/// Reachable only because `serde_json` keeps object order now: nothing declares an `input_order`
/// this far down, so `width, height, crop` sorted to `crop, height, width` would have been read
/// as three values in the wrong places — the same defect as the outer one, one level in.
fn grown(
    about: Option<&serde_json::Value>,
    produced: &std::collections::BTreeSet<&str>,
) -> BTreeMap<String, Vec<Widget>> {
    let mut grown = BTreeMap::new();
    let Some(options) = about
        .and_then(|a| a.get("options"))
        .and_then(|o| o.as_array())
    else {
        return grown;
    };
    for option in options {
        let Some(key) = option.get("key").and_then(|k| k.as_str()) else {
            continue;
        };
        let mut extra = Vec::new();
        for section in ["required", "optional"] {
            let Some(inputs) = option
                .get("inputs")
                .and_then(|i| i.get(section))
                .and_then(|s| s.as_object())
            else {
                continue;
            };
            for (name, definition) in inputs {
                if let Some(widget) = one_widget(name, definition, produced) {
                    extra.push(widget);
                }
            }
        }
        grown.insert(key.to_owned(), extra);
    }
    grown
}

/// Whether a type is something somebody sets rather than something they plug in.
///
/// A union — `"SAM3_TRACK_DATA,MASK"` — counts as a socket if **any** of its parts is produced
/// somewhere, because that is the part a wire would arrive on. Getting this backwards is worse
/// than leaving it: a socket mistaken for a value consumes a position nothing filled, and every
/// later value lands one place early.
fn is_a_value(kind: &str, produced: &std::collections::BTreeSet<&str>) -> bool {
    kind != "*" && !kind.split(',').any(|part| produced.contains(part.trim()))
}

/// Why a workflow could not be read.
#[derive(Debug, Clone, PartialEq)]
pub enum Unreadable {
    /// Not JSON, or JSON in neither shape.
    NotAWorkflow,
    /// It uses nodes this ComfyUI does not have — almost always uninstalled custom nodes.
    ///
    /// Named rather than counted: *"needs 2 custom nodes"* sends somebody hunting, and
    /// *"needs `IPAdapterApply`"* is something they can search for.
    Missing(Vec<String>),
    /// This node takes a value whose choice grows more values, and Epoch cannot line those up.
    ///
    /// Named separately from `Unaligned` because the fix is different: this one is work Epoch
    /// has not done, not a server that will not answer.
    Dynamic(String),
    /// The server did not say what order this node's inputs come in, and the workflow has
    /// values that would have to be aligned against them.
    ///
    /// Refusing beats guessing. A misaligned workflow does not fail — it runs with `cfg` where
    /// `steps` belongs, which is a picture nobody can explain.
    Unaligned(String),
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Unreadable::NotAWorkflow => {
                write!(f, "this file is not a ComfyUI workflow Epoch can read")
            }
            Unreadable::Missing(nodes) => write!(
                f,
                "this workflow needs nodes this ComfyUI does not have: {}",
                nodes.join(", ")
            ),
            Unreadable::Dynamic(class) => write!(
                f,
                "'{class}' has a setting that changes which other settings exist, and Epoch cannot line those up yet — so it will not guess at them"
            ),
            Unreadable::Unaligned(class) => write!(
                f,
                "this ComfyUI does not say what order '{class}' takes its settings in, so Epoch will not guess at them"
            ),
        }
    }
}

/// Read a workflow in whichever shape it arrived.
///
/// `schema` is only consulted for the UI format; an API workflow already says what it means.
pub fn read(raw: &serde_json::Value, schema: &Schema) -> Result<Workflow, Unreadable> {
    if raw.get("nodes").and_then(|n| n.as_array()).is_some() {
        from_ui(raw, schema)
    } else {
        from_api(raw)
    }
}

/// The shape the server executes, taken as it is.
pub fn from_api(raw: &serde_json::Value) -> Result<Workflow, Unreadable> {
    let Some(all) = raw.as_object() else {
        return Err(Unreadable::NotAWorkflow);
    };
    let mut nodes = BTreeMap::new();
    for (id, node) in all {
        let Some(class) = node.get("class_type").and_then(|c| c.as_str()) else {
            continue;
        };
        let mut inputs = BTreeMap::new();
        if let Some(given) = node.get("inputs").and_then(|i| i.as_object()) {
            for (name, value) in given {
                inputs.insert(name.clone(), input_of(value));
            }
        }
        nodes.insert(
            id.clone(),
            Node {
                class_type: class.to_owned(),
                inputs,
            },
        );
    }
    if nodes.is_empty() {
        return Err(Unreadable::NotAWorkflow);
    }
    Ok(Workflow { nodes })
}

/// A link is a two-element array of `[node id, slot]`; anything else is a literal.
fn input_of(value: &serde_json::Value) -> Input {
    match value.as_array() {
        Some(pair) if pair.len() == 2 && (pair[0].is_string() || pair[0].is_number()) => {
            Input::From(vec![
                serde_json::Value::String(as_id(&pair[0])),
                pair[1].clone(),
            ])
        }
        _ => Input::Value(value.clone()),
    }
}

fn as_id(value: &serde_json::Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string())
}

/// The shape people actually have, converted against this machine's ComfyUI.
pub fn from_ui(raw: &serde_json::Value, schema: &Schema) -> Result<Workflow, Unreadable> {
    // Subgraphs first: the server has never heard of one, and a third of real workflows have
    // them. What comes back is a graph of nodes it knows.
    let raw = &flatten(raw);
    let Some(nodes) = raw.get("nodes").and_then(|n| n.as_array()) else {
        return Err(Unreadable::NotAWorkflow);
    };

    let by_id: BTreeMap<String, &serde_json::Value> = nodes
        .iter()
        .filter_map(|node| node.get("id").map(|id| (as_id(id), node)))
        .collect();

    // `links` is `[link id, source node, source slot, target node, target slot, type]`.
    let mut wires: BTreeMap<String, (String, serde_json::Value)> = BTreeMap::new();
    if let Some(links) = raw.get("links").and_then(|l| l.as_array()) {
        for link in links.iter().filter_map(|l| l.as_array()) {
            if link.len() >= 3 {
                wires.insert(as_id(&link[0]), (as_id(&link[1]), link[2].clone()));
            }
        }
    }

    let mut missing = Vec::new();
    let mut out = BTreeMap::new();

    for node in nodes {
        let class = node
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or_default();
        if class.is_empty()
            || DECORATION.contains(&class)
            || PASSTHROUGH.contains(&class)
            || silenced(node.get("mode").and_then(|m| m.as_i64()))
        {
            continue;
        }
        let Some(widgets) = schema.classes.get(class) else {
            if !missing.contains(&class.to_owned()) {
                missing.push(class.to_owned());
            }
            continue;
        };

        let slots = node.get("inputs").and_then(|i| i.as_array());
        let linked: BTreeMap<String, serde_json::Value> = slots
            .map(|list| {
                list.iter()
                    .filter_map(|slot| {
                        slot.get("name")
                            .and_then(|n| n.as_str())
                            .map(|n| (n.to_owned(), slot.get("link").cloned().unwrap_or_default()))
                    })
                    .collect()
            })
            .unwrap_or_default();

        // **The node itself says which of its inputs are sockets, and it is the only thing that
        // can.** A widget the author converted to a link is listed here *with* a `widget` marker
        // and keeps its place among the values; a real socket is listed without one and never
        // had a value at all.
        //
        // Measured 2026-08-22: `RenderSplat` takes a `SPLAT`, and nothing in `/object_info`
        // produces one — it comes out of a subgraph. Judged globally it looked like a value,
        // consumed the first position, and pushed the last of nine settings off the end;
        // the server answered `required_input_missing: background`.
        let sockets: std::collections::BTreeSet<String> = slots
            .map(|list| {
                list.iter()
                    .filter(|slot| slot.get("widget").is_none())
                    .filter_map(|slot| slot.get("name").and_then(|n| n.as_str()).map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();

        let values: Vec<serde_json::Value> = node
            .get("widgets_values")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // **Misalignment needs somewhere to land.** Values with no published order cannot be
        // aligned and a wrong alignment runs — but a class the server says takes *no values at
        // all* has nothing to get wrong. `PreviewAny` takes one wildcard socket and the editor
        // keeps display state beside it; refusing those cost 38 workflows for no risk.
        if !values.is_empty() && widgets.order.is_empty() && !widgets.told {
            return Err(Unreadable::Unaligned(class.to_owned()));
        }
        // **Counted, not forbidden.** Refusing every node with a growing setting cost 317 of
        // 509 workflows — `SaveVideo` has one and almost never expands it. What actually breaks
        // alignment is a *mismatch*: when the file holds more values than the schema accounts
        // for, the extra ones came from an expansion, and everything after it is one place out.
        //
        // So the arithmetic decides. Same count, walk it; different count with something that
        // grows, refuse; different count with nothing that grows, walk it as before (a node
        // saved by an older version simply has fewer).

        let mut inputs = BTreeMap::new();
        let mut at = 0usize;
        for widget in &widgets.order {
            if sockets.contains(&widget.name) {
                // This instance draws it as a socket. It has no value to take.
                continue;
            }
            if at >= values.len() {
                break;
            }
            // Trap 1: a converted widget still occupies its slot. Take the value regardless; the
            // link below overwrites it.
            let chosen = values[at].clone();
            inputs.insert(widget.name.clone(), Input::Value(chosen.clone()));
            at += 1;
            // Trap 2: `control_after_generate` follows the value and is not an input. The server
            // declares which inputs have one, so this is read rather than guessed at.
            if widget.then_a_control && values.get(at).is_some_and(|v| v.is_string()) {
                at += 1;
            }
            // **A choice that grows takes the values it grew.** `resize_type: "scale
            // dimensions"` is followed by `width`, `height` and `crop`, in the order the server
            // declares them — and named the way the server names them in its own errors.
            if widget.expands {
                let grew = chosen
                    .as_str()
                    .and_then(|key| widget.grows.get(key))
                    .map(Vec::as_slice)
                    .unwrap_or_default();
                for extra in grew {
                    if at >= values.len() {
                        break;
                    }
                    inputs.insert(
                        format!("{}.{}", widget.name, extra.name),
                        Input::Value(values[at].clone()),
                    );
                    at += 1;
                    if extra.then_a_control && values.get(at).is_some_and(|v| v.is_string()) {
                        at += 1;
                    }
                }
            }
        }

        for (name, link) in &linked {
            if let Some(found) = follow(link, &wires, &by_id, 0) {
                inputs.insert(name.clone(), found);
            }
        }

        out.insert(
            as_id(node.get("id").unwrap_or(&serde_json::Value::Null)),
            Node {
                class_type: class.to_owned(),
                inputs,
            },
        );
    }

    if !missing.is_empty() {
        return Err(Unreadable::Missing(missing));
    }
    if out.is_empty() {
        return Err(Unreadable::NotAWorkflow);
    }
    Ok(Workflow { nodes: out })
}

/// Inline every subgraph, so what is left is nodes a server knows.
///
/// ## Why this is not optional
///
/// A subgraph is a node whose `type` is a UUID and whose graph lives in
/// `definitions.subgraphs`. ComfyUI's editor flattens them before executing; the server has
/// never heard of one. **Measured 2026-08-22: 181 of 509 shipped templates define at least one**
/// — and all 134 of the workflows carrying `proxyWidgets` are among them, because exposing a
/// value is what a subgraph is *for*. Refusing them would refuse a third of the world.
///
/// ## What the shape actually is, read rather than assumed
///
/// ```text
/// outer node 29   type 94961018-…            inputs [source_panorama -> link 43]
/// subgraph        inputNode.id -10           outputNode.id -20
///   inner link    { id: 57, origin_id: -10, origin_slot: 0, target_id: 31, … }
///   inner link    { id: 37, origin_id: 24, origin_slot: 0, target_id: -20, … }
/// ```
///
/// Three facts, each of which would have been guessed wrong:
///
/// 1. **Inner links are objects; outer links are arrays.** The same document uses both.
/// 2. **A boundary input the caller left unconnected is not an error** — it means the inner
///    node's own widget value stands. Half of the eleven inputs on the example are like that.
/// 3. **`origin_slot` on a boundary link indexes the boundary list**, not an output socket.
///
/// ## Ids are prefixed, not renumbered
///
/// An inner node `5` inside instance `29` becomes `29:5`. Renumbering would need a counter
/// threaded through recursion, and the prefix is reversible — which is what lets
/// `proxyWidgets`, written against inner ids, still point at something afterwards.
fn flatten(ui: &serde_json::Value) -> serde_json::Value {
    let mut flat = ui.clone();
    // Nested subgraphs are ordinary: a pass may expose more. Bounded, because each pass either
    // removes an instance or changes nothing.
    for _ in 0..16 {
        match inline_once(&flat) {
            Some(next) => flat = next,
            None => break,
        }
    }
    flat
}

/// One pass. `None` when there was nothing left to inline.
fn inline_once(ui: &serde_json::Value) -> Option<serde_json::Value> {
    let definitions: BTreeMap<String, &serde_json::Value> = ui
        .get("definitions")
        .and_then(|d| d.get("subgraphs"))
        .and_then(|s| s.as_array())
        .map(|all| {
            all.iter()
                .filter_map(|sub| {
                    sub.get("id")
                        .and_then(|i| i.as_str())
                        .map(|id| (id.to_owned(), sub))
                })
                .collect()
        })
        .unwrap_or_default();
    if definitions.is_empty() {
        return None;
    }

    let nodes = ui.get("nodes")?.as_array()?;
    let instances: Vec<&serde_json::Value> = nodes
        .iter()
        .filter(|node| {
            node.get("type")
                .and_then(|t| t.as_str())
                .is_some_and(|t| definitions.contains_key(t))
        })
        .collect();
    if instances.is_empty() {
        return None;
    }

    let mut out_nodes: Vec<serde_json::Value> = Vec::new();
    let mut out_links: Vec<serde_json::Value> = Vec::new();
    // Where an outer link starting at a subgraph instance should now start.
    let mut rerouted: BTreeMap<(String, i64), (String, serde_json::Value)> = BTreeMap::new();

    for node in nodes {
        let class = node
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or_default();
        let Some(definition) = definitions.get(class) else {
            out_nodes.push(node.clone());
            continue;
        };
        let instance = as_id(node.get("id").unwrap_or(&serde_json::Value::Null));
        let inner_boundary = boundary(definition, "inputNode");
        let outer_boundary = boundary(definition, "outputNode");

        // What the caller wired into each boundary input, by position.
        let given: Vec<Option<serde_json::Value>> = definition
            .get("inputs")
            .and_then(|i| i.as_array())
            .map(|boundaries| {
                boundaries
                    .iter()
                    .map(|edge| {
                        let name = edge.get("name")?.as_str()?;
                        node.get("inputs")?
                            .as_array()?
                            .iter()
                            .find(|slot| slot.get("name").and_then(|n| n.as_str()) == Some(name))?
                            .get("link")
                            .filter(|link| !link.is_null())
                            .cloned()
                    })
                    .collect()
            })
            .unwrap_or_default();

        // The inner nodes, carried out under prefixed ids.
        if let Some(inner) = definition.get("nodes").and_then(|n| n.as_array()) {
            for node in inner {
                let mut moved = node.clone();
                let id = as_id(node.get("id").unwrap_or(&serde_json::Value::Null));
                moved["id"] = serde_json::json!(format!("{instance}:{id}"));
                if let Some(slots) = moved.get_mut("inputs").and_then(|i| i.as_array_mut()) {
                    for slot in slots {
                        if let Some(link) = slot.get("link").filter(|l| !l.is_null()).cloned() {
                            slot["link"] =
                                serde_json::json!(format!("{instance}:{}", as_id(&link)));
                        }
                    }
                }
                out_nodes.push(moved);
            }
        }

        // The inner wires, with the two boundaries resolved.
        for link in inner_links(definition) {
            let (id, origin, origin_slot, target, target_slot, kind) = link;

            if Some(origin.as_str()) == inner_boundary.as_deref() {
                // From the outside in. The caller may have left it unwired, in which case the
                // inner node keeps whatever value it was saved with — which is a real answer,
                // not a gap.
                let Some(Some(outer_link)) = given.get(origin_slot as usize) else {
                    continue;
                };
                let Some((source, slot)) = source_of(ui, outer_link) else {
                    continue;
                };
                out_links.push(serde_json::json!([
                    format!("{instance}:{id}"),
                    source,
                    slot,
                    format!("{instance}:{target}"),
                    target_slot,
                    kind
                ]));
                continue;
            }

            if Some(target.as_str()) == outer_boundary.as_deref() {
                // From the inside out. Remember who really produces this, so the caller's own
                // wires can be pointed at them.
                rerouted.insert(
                    (instance.clone(), target_slot),
                    (
                        format!("{instance}:{origin}"),
                        serde_json::json!(origin_slot),
                    ),
                );
                continue;
            }

            out_links.push(serde_json::json!([
                format!("{instance}:{id}"),
                format!("{instance}:{origin}"),
                origin_slot,
                format!("{instance}:{target}"),
                target_slot,
                kind
            ]));
        }
    }

    // The caller's own wires, with anything leaving a subgraph re-pointed at the node inside it
    // that actually makes the thing.
    if let Some(links) = ui.get("links").and_then(|l| l.as_array()) {
        for link in links {
            let Some((id, origin, origin_slot, target, target_slot, kind)) = one_link(link) else {
                continue;
            };
            match rerouted.get(&(origin.clone(), origin_slot)) {
                Some((source, slot)) => out_links.push(serde_json::json!([
                    id,
                    source,
                    slot,
                    target,
                    target_slot,
                    kind
                ])),
                None => out_links.push(serde_json::json!([
                    id,
                    origin,
                    origin_slot,
                    target,
                    target_slot,
                    kind
                ])),
            }
        }
    }

    let mut next = ui.clone();
    next["nodes"] = serde_json::Value::Array(out_nodes);
    next["links"] = serde_json::Value::Array(out_links);
    Some(next)
}

/// The id of a subgraph's input or output pseudo-node — `-10` and `-20` in the wild.
///
/// Read rather than hardcoded: they are two numbers in one file, and a workflow written by a
/// different version is not a thing to gamble on.
fn boundary(definition: &serde_json::Value, which: &str) -> Option<String> {
    definition.get(which)?.get("id").map(as_id)
}

/// A subgraph's own wires, in the shape they are written in.
///
/// Objects here, arrays outside — the same document, two shapes, and both are read.
fn inner_links(
    definition: &serde_json::Value,
) -> Vec<(String, String, i64, String, i64, serde_json::Value)> {
    definition
        .get("links")
        .and_then(|l| l.as_array())
        .map(|all| all.iter().filter_map(one_link).collect())
        .unwrap_or_default()
}

/// One wire, whether it was written as an object or as an array.
fn one_link(
    link: &serde_json::Value,
) -> Option<(String, String, i64, String, i64, serde_json::Value)> {
    if let Some(fields) = link.as_array() {
        if fields.len() < 5 {
            return None;
        }
        return Some((
            as_id(&fields[0]),
            as_id(&fields[1]),
            fields[2].as_i64().unwrap_or_default(),
            as_id(&fields[3]),
            fields[4].as_i64().unwrap_or_default(),
            fields.get(5).cloned().unwrap_or(serde_json::Value::Null),
        ));
    }
    Some((
        as_id(link.get("id")?),
        as_id(link.get("origin_id")?),
        link.get("origin_slot")?.as_i64().unwrap_or_default(),
        as_id(link.get("target_id")?),
        link.get("target_slot")?.as_i64().unwrap_or_default(),
        link.get("type").cloned().unwrap_or(serde_json::Value::Null),
    ))
}

/// Who is on the far end of one of the caller's wires.
fn source_of(
    ui: &serde_json::Value,
    link_id: &serde_json::Value,
) -> Option<(String, serde_json::Value)> {
    let wanted = as_id(link_id);
    ui.get("links")?
        .as_array()?
        .iter()
        .filter_map(one_link)
        .find(|(id, ..)| *id == wanted)
        .map(|(_, origin, origin_slot, ..)| (origin, serde_json::json!(origin_slot)))
}

/// Follow one wire back to a node the server knows, inlining anything that is not one.
///
/// Trap 3: a `PrimitiveNode` holds a value for somebody else's widget and a `Reroute` is a bend
/// in a wire. Neither exists on the server, so both are resolved here — and a muted node is
/// passed straight through, which is what bypassing one means.
fn follow(
    link: &serde_json::Value,
    wires: &BTreeMap<String, (String, serde_json::Value)>,
    by_id: &BTreeMap<String, &serde_json::Value>,
    depth: usize,
) -> Option<Input> {
    if link.is_null() || depth > 16 {
        return None;
    }
    let (source_id, slot) = wires.get(&as_id(link))?;
    let source = by_id.get(source_id)?;
    let class = source
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or_default();

    let incoming = || -> Option<serde_json::Value> {
        source
            .get("inputs")
            .and_then(|i| i.as_array())
            .and_then(|list| {
                list.iter()
                    .filter_map(|s| s.get("link"))
                    .find(|l| !l.is_null())
                    .cloned()
            })
    };

    if PASSTHROUGH.contains(&class) {
        return match incoming() {
            Some(next) => follow(&next, wires, by_id, depth + 1),
            None => source
                .get("widgets_values")
                .and_then(|v| v.as_array())
                .and_then(|v| v.first())
                .cloned()
                .map(Input::Value),
        };
    }
    if silenced(source.get("mode").and_then(|m| m.as_i64())) {
        return incoming().and_then(|next| follow(&next, wires, by_id, depth + 1));
    }

    Some(Input::From(vec![
        serde_json::Value::String(source_id.clone()),
        slot.clone(),
    ]))
}

/// What a workflow leaves open, and what it can be asked for.
///
/// ## Derived, never declared
///
/// Nobody types this in. Whether a workflow can take a reference image is answered by whether
/// its graph loads one; where the prompt goes is answered by asking the sampler which of its
/// inputs is `positive`. A capability somebody wrote into a manifest is a capability that will
/// eventually lie (ADR-0005, ADR-0030).
///
/// ## Every positive, not the first one
///
/// Measured on ComfyUI's own SDXL template: it has **four** `CLIPTextEncode` nodes — a positive
/// and a negative for the base pass, and the same pair again for the refiner — and the author
/// wrote the *same sentence* in both positives. A prompt set on only the first would leave the
/// refiner painting something else.
///
/// So a prompt is written to every text node the samplers call positive, and the same for
/// negative. That also makes single-sampler workflows the ordinary case rather than a special
/// one.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opening {
    /// Node ids whose `text` is what the picture should contain.
    pub positive: Vec<String>,
    /// Node ids whose `text` is what it should avoid.
    pub negative: Vec<String>,
    /// Node id holding `width` and `height`, when the size is not fixed by the graph.
    pub size: Option<String>,
    /// Node ids with a seed, so *"again, differently"* is possible.
    pub seeds: Vec<String>,
    /// Node ids with a `steps` widget — how hard it works, in the workflow's own terms.
    pub steps: Vec<String>,
    /// Node ids that load a picture into the graph — a reference, or the thing being edited.
    pub takes_image: Vec<String>,
    /// What it can be asked to do.
    pub can: Abilities,
    /// The model files it names. What must be installed for it to run at all.
    pub needs: Vec<String>,
}

/// What a workflow is for, read off its graph.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Abilities {
    /// It turns words into a picture. True of anything with a sampler and a text prompt.
    pub from_words: bool,
    /// It takes a picture in — a reference, or the thing being edited.
    pub from_image: bool,
    /// It paints inside a mask.
    pub inpaint: bool,
    /// It enlarges.
    pub upscale: bool,
}

/// The classes that load a picture into a graph.
const LOADS_AN_IMAGE: [&str; 4] = [
    "LoadImage",
    "LoadImageMask",
    "LoadImageOutput",
    "ImageBatch",
];

/// The classes that mean *paint inside this shape*.
const MASKS: [&str; 4] = [
    "VAEEncodeForInpaint",
    "SetLatentNoiseMask",
    "InpaintModelConditioning",
    "LoadImageMask",
];

/// Widget names that name a file on disk.
const NAMES_A_FILE: [&str; 6] = [
    "ckpt_name",
    "lora_name",
    "vae_name",
    "unet_name",
    "clip_name",
    "model_name",
];

impl Workflow {
    /// Read what this workflow leaves open.
    pub fn opening(&self) -> Opening {
        let mut opening = Opening::default();

        for (id, node) in &self.nodes {
            // A sampler is anything that samples. `KSampler`, `KSamplerAdvanced`,
            // `SamplerCustom` and whatever the next one is called — matched on the word rather
            // than on a list, because that list would be out of date the week it was written.
            if node.class_type.contains("Sampler") {
                for (which, into) in [
                    ("positive", &mut opening.positive),
                    ("negative", &mut opening.negative),
                ] {
                    if let Some(Input::From(link)) = node.inputs.get(which) {
                        if let Some(text) = self.text_behind(link, 0) {
                            if !into.contains(&text) {
                                into.push(text);
                            }
                        }
                    }
                }
            }

            // Size, seed and steps are widgets wherever they are, and a workflow may have
            // several of each — the SDXL template has two samplers with a seed apiece.
            if node.inputs.contains_key("width") && node.inputs.contains_key("height") {
                opening.size.get_or_insert_with(|| id.clone());
            }
            for (name, value) in &node.inputs {
                if matches!(value, Input::Value(_)) {
                    if name == "seed" || name == "noise_seed" {
                        opening.seeds.push(id.clone());
                    }
                    if name == "steps" {
                        opening.steps.push(id.clone());
                    }
                    if NAMES_A_FILE.contains(&name.as_str()) {
                        if let Input::Value(file) = value {
                            if let Some(named) = file.as_str() {
                                if !opening.needs.contains(&named.to_owned()) {
                                    opening.needs.push(named.to_owned());
                                }
                            }
                        }
                    }
                }
            }

            if LOADS_AN_IMAGE.contains(&node.class_type.as_str()) {
                opening.can.from_image = true;
                opening.takes_image.push(id.clone());
            }
            if MASKS.contains(&node.class_type.as_str()) {
                opening.can.inpaint = true;
            }
            if node.class_type.contains("Upscale") {
                opening.can.upscale = true;
            }
        }

        // Words in, picture out — which needs somewhere to put the words.
        opening.can.from_words = !opening.positive.is_empty();
        opening.seeds.sort();
        opening.seeds.dedup();
        opening.steps.sort();
        opening.steps.dedup();
        opening.needs.sort();
        opening
    }

    /// Follow a conditioning wire back to the node holding the words.
    ///
    /// A prompt rarely reaches a sampler directly: it passes through `ConditioningCombine`,
    /// `ControlNetApply`, `ConditioningSetArea` and whatever else the author put in the way. So
    /// this walks back through anything until it finds text, rather than expecting the sampler's
    /// neighbour to be a `CLIPTextEncode`.
    fn text_behind(&self, link: &[serde_json::Value], depth: usize) -> Option<String> {
        if depth > 16 {
            return None;
        }
        let id = link.first()?.as_str()?;
        let node = self.nodes.get(id)?;
        if matches!(node.inputs.get("text"), Some(Input::Value(_))) {
            return Some(id.to_owned());
        }
        node.inputs
            .values()
            .filter_map(|input| match input {
                Input::From(next) => self.text_behind(next, depth + 1),
                Input::Value(_) => None,
            })
            .next()
    }

    /// Write a prompt into every place the workflow accepts one.
    ///
    /// Positive and negative both, because leaving the negative alone means keeping whatever the
    /// author was avoiding — which is usually right, and is the caller's decision rather than
    /// this function's.
    pub fn say(&mut self, describe: &str, avoid: Option<&str>) {
        let opening = self.opening();
        for id in &opening.positive {
            if let Some(node) = self.nodes.get_mut(id) {
                node.inputs
                    .insert("text".into(), Input::Value(serde_json::json!(describe)));
            }
        }
        if let Some(avoid) = avoid {
            for id in &opening.negative {
                if let Some(node) = self.nodes.get_mut(id) {
                    node.inputs
                        .insert("text".into(), Input::Value(serde_json::json!(avoid)));
                }
            }
        }
    }

    /// Set the picture's shape, when the workflow has one to set.
    pub fn shape(&mut self, width: u32, height: u32) {
        let Some(id) = self.opening().size else {
            return;
        };
        if let Some(node) = self.nodes.get_mut(&id) {
            node.inputs
                .insert("width".into(), Input::Value(serde_json::json!(width)));
            node.inputs
                .insert("height".into(), Input::Value(serde_json::json!(height)));
        }
    }

    /// Hand the workflow a picture to work from.
    ///
    /// The name is ComfyUI's, not the vault's: a workflow loads by the name in the server's own
    /// input directory, which is what [`crate::workflow`]'s caller gets back from handing the
    /// bytes over. Epoch's filenames mean nothing there.
    ///
    /// Every loader gets it, for the reason every prompt node does: a graph with two references
    /// is far more often the same picture twice than two different ones, and the alternative is
    /// asking somebody which `LoadImage` they meant.
    pub fn show(&mut self, name: &str) {
        for id in self.opening().takes_image {
            if let Some(node) = self.nodes.get_mut(&id) {
                node.inputs
                    .insert("image".into(), Input::Value(serde_json::json!(name)));
            }
        }
    }

    /// Give every sampler a seed, so *"again, differently"* means something.
    pub fn seed(&mut self, seed: u64) {
        for id in self.opening().seeds {
            if let Some(node) = self.nodes.get_mut(&id) {
                for name in ["seed", "noise_seed"] {
                    if node.inputs.contains_key(name) {
                        node.inputs
                            .insert(name.into(), Input::Value(serde_json::json!(seed)));
                    }
                }
            }
        }
    }
}

/// How much of a control a person should have to meet.
///
/// ## Where the line is, and it moved twice
///
/// The first version had three levels and the last one was **never drawn**. The owner corrected
/// it (2026-08-22): *the user chooses everything*. Folding something away is a place it lives;
/// not drawing it is a decision taken away from them.
///
/// Then it moved again, and this is the sharper version of the same point: **being asked is how
/// you choose.** A checkpoint sitting behind *Advanced* is a choice nobody made, even though
/// they could have. A value already filled in and visible is not a question put to somebody —
/// it is an answer they can read in one line and change if they want to.
///
/// So the line is not *how many fields fit*, it is:
///
/// > **What the picture is, is shown. How it is computed, is folded.**
///
/// The words, the shape, the model and the LoRA are the picture. The sampler and the scheduler
/// are arithmetic, and nobody picks one on purpose the first time.
///
/// **Finding a setting is not the same as showing it**, and ADR-0026 already settled the shape
/// this takes: *General / Advanced ▼ — the default is that the user touches nothing.* A workflow
/// has thirty knobs; importing one must not rebuild ComfyUI's interface inside Epoch.
///
/// Three, not four. `PRIMARY` beside `BASIC` was proposed and refused: it is a distinction
/// nobody applies the same way twice, and there is already a vocabulary in this product.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Tier {
    /// Shown. The prompt, the shape, the reference.
    General,
    /// Behind **Advanced ▾**. Real settings, for somebody who wants them.
    Advanced,
    /// Behind **Everything ▾**. The workflow's own internals — which sampler, which scheduler,
    /// which files.
    ///
    /// **Folded away, never withheld** (owner, 2026-08-22: *the user chooses everything*). An
    /// earlier version of this never drew them at all, which is a different thing from a default
    /// that asks nothing: one keeps the decision, the other takes it. ADR-0026's rule is that
    /// **touching nothing is the default**, not that there is nothing to touch.
    Everything,
}

/// One thing a surface could let somebody set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Control {
    pub node: String,
    pub input: String,
    /// What to call it. The node's own title when the author gave it one, because an author who
    /// renamed a node to *"Character description"* has said what it is better than we can.
    pub label: String,
    pub kind: String,
    pub choices: Vec<String>,
    pub value: serde_json::Value,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub step: Option<f64>,
    pub multiline: bool,
    /// The server's own sentence, never one Epoch made up.
    pub help: Option<String>,
    pub tier: Tier,
    /// The author of this workflow exposed it deliberately.
    pub exposed: bool,
}

/// Inputs Epoch understands well enough to place without being told.
///
/// Everything else is `Kept` unless the workflow's author says otherwise — which is the whole
/// ordering: **what the author exposed, then what Epoch knows, then nothing.**
fn known(input: &str) -> Option<Tier> {
    match input {
        "width" | "height" => Some(Tier::General),
        "seed" | "noise_seed" | "steps" | "cfg" | "denoise" | "batch_size" => Some(Tier::Advanced),
        // **Shown, because a choice you cannot see is not one you made** (owner, 2026-08-22:
        // *being asked is how you choose*). This went to Advanced first, on the reasoning that
        // the default should ask nothing — but a value already filled in is not a question being
        // put to somebody, it is an answer they can read and change. It costs one line on screen
        // and it is the most direct thing anybody can do to change how a picture looks.
        //
        // The plumbing below it stays folded: nobody chooses a scheduler on purpose the first
        // time. That is the line — what the picture *is* is shown, how it is computed is not.
        name if NAMES_A_FILE.contains(&name) => Some(Tier::General),
        _ => None,
    }
}

impl Imported {
    /// Every control this workflow could offer, in the order a surface should draw them.
    ///
    /// ## The author goes first, and it is not a courtesy
    ///
    /// A ComfyUI workflow may carry `proxyWidgets` — the author's own list of which values
    /// matter, `[["5", "resolution_level"], ["24", "texture"], …]`. **Measured 2026-08-22: 134
    /// of the 522 templates on this machine carry one.** A quarter of the corpus has already
    /// answered the question this function is otherwise guessing at, so it is read first and
    /// heuristics only fill the rest.
    ///
    /// Author intent lifts a value to **Advanced**, never to General. General is four things a
    /// person expects to see; an author who exposed eleven knobs meant *these are the knobs*,
    /// not *put eleven fields on the first screen*.
    pub fn controls(&self, schema: &Schema) -> Vec<Control> {
        let exposed = self.exposed();
        let opening = &self.opening;
        let mut controls = Vec::new();

        for (id, node) in &self.compiled.nodes {
            let Some(widgets) = schema.classes.get(&node.class_type) else {
                continue;
            };
            for widget in &widgets.order {
                let Some(Input::Value(value)) = node.inputs.get(&widget.name) else {
                    // Driven by another node. Not a value anybody sets here.
                    continue;
                };

                let author = exposed.contains(&(id.clone(), widget.name.clone()));
                let tier = if opening.positive.contains(id) && widget.name == "text" {
                    Tier::General
                } else if opening.negative.contains(id) && widget.name == "text" {
                    // What to avoid is a real setting and a rare one. It also usually holds
                    // something the author chose, which is worth keeping by default.
                    Tier::Advanced
                } else {
                    match (known(&widget.name), author) {
                        (Some(tier), _) => tier,
                        (None, true) => Tier::Advanced,
                        (None, false) => Tier::Everything,
                    }
                };

                controls.push(Control {
                    node: id.clone(),
                    input: widget.name.clone(),
                    label: self.label_for(id, &widget.name, opening),
                    kind: widget.kind.clone(),
                    choices: widget.choices.clone(),
                    value: value.clone(),
                    min: widget.min,
                    max: widget.max,
                    step: widget.step,
                    multiline: widget.multiline,
                    help: widget.help.clone(),
                    tier,
                    exposed: author,
                });
            }
        }

        // Within a tier, the prompt first. Node ids are the order somebody laid a graph out in,
        // which is not an order to show a person: it put *width* above *what it should contain*.
        controls.sort_by(|a, b| {
            let first = |c: &Control| u8::from(c.input != "text");
            a.tier
                .cmp(&b.tier)
                .then_with(|| first(a).cmp(&first(b)))
                .then_with(|| a.node.cmp(&b.node))
                .then_with(|| a.input.cmp(&b.input))
        });
        controls
    }

    /// What the author called it, or what it is.
    fn label_for(&self, id: &str, input: &str, opening: &Opening) -> String {
        if input == "text" {
            if opening.positive.contains(&id.to_owned()) {
                return "What it should contain".to_owned();
            }
            if opening.negative.contains(&id.to_owned()) {
                return "What it should avoid".to_owned();
            }
        }
        if let Some(title) = self.title_of(id) {
            return title;
        }
        // `noise_seed` -> `Noise seed`. Nothing clever: the server's names are already words.
        let mut plain = input.replace('_', " ");
        if let Some(first) = plain.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        plain
    }

    /// The title the author gave a node, when they gave it one.
    ///
    /// Read from the **source**, because a title is something a person wrote and the compiled
    /// form is only what the server needs. Measured: 383 of 522 templates title at least one
    /// node.
    fn title_of(&self, id: &str) -> Option<String> {
        self.source
            .get("nodes")?
            .as_array()?
            .iter()
            .find(|node| node.get("id").map(as_id).as_deref() == Some(id))?
            .get("title")?
            .as_str()
            .map(str::to_owned)
    }

    /// `(node id, input)` pairs the author deliberately exposed.
    ///
    /// From `properties.proxyWidgets` in the source. Absent from three quarters of workflows,
    /// which is why it decides rather than being required.
    fn exposed(&self) -> std::collections::BTreeSet<(String, String)> {
        let mut found = std::collections::BTreeSet::new();
        let Some(nodes) = self.source.get("nodes").and_then(|n| n.as_array()) else {
            return found;
        };
        for node in nodes {
            let Some(pairs) = node
                .get("properties")
                .and_then(|p| p.get("proxyWidgets"))
                .and_then(|w| w.as_array())
            else {
                continue;
            };
            for pair in pairs.iter().filter_map(|p| p.as_array()) {
                if let [node_id, input] = pair.as_slice() {
                    if let Some(name) = input.as_str() {
                        found.insert((as_id(node_id), name.to_owned()));
                    }
                }
            }
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Build a schema the way the server would report one, straight to the reduced form.
    fn schema_for(class: &str, order: &[(&str, &str)]) -> Schema {
        let mut classes = BTreeMap::new();
        classes.insert(
            class.to_owned(),
            Widgets {
                told: true,
                order: order
                    .iter()
                    .map(|(name, kind)| Widget {
                        name: (*name).to_owned(),
                        kind: (*kind).to_owned(),
                        // The one the traps turn on, and the fixtures name it the way the
                        // server does.
                        then_a_control: *kind == "INT" && name.contains("seed"),
                        ..Widget::default()
                    })
                    .collect(),
            },
        );
        Schema { classes }
    }

    #[test]
    fn an_api_workflow_is_taken_as_it_is() {
        let raw = json!({
            "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "a lighthouse", "clip": ["4", 1] } }
        });
        let workflow = from_api(&raw).expect("readable");
        let node = &workflow.nodes["6"];
        assert_eq!(node.class_type, "CLIPTextEncode");
        assert_eq!(node.inputs["clip"], Input::From(vec![json!("4"), json!(1)]));
        assert_eq!(node.inputs["text"], Input::Value(json!("a lighthouse")));
    }

    /// Trap 1, and it is the one that produces a plausible, wrong request.
    ///
    /// Measured 2026-08-22 on ComfyUI's own `sdxl_simple_example`: `steps` and `end_at_step` were
    /// converted to links, and skipping their widget slots gave `cfg: 25, sampler_name: 8,
    /// scheduler: "euler", start_at_step: "normal"` — every value one place late.
    #[test]
    fn a_widget_converted_to_a_link_keeps_its_place() {
        let schema = schema_for(
            "KSampler",
            &[
                ("steps", "INT"),
                ("cfg", "FLOAT"),
                ("sampler_name", "CHOICE"),
            ],
        );
        let ui = json!({
            "nodes": [
                { "id": 3, "type": "KSampler",
                  // A converted widget carries this marker in the wild — measured on
                  // `KSamplerAdvanced` in ComfyUI's own SDXL template. Without it the slot is a
                  // real socket, which never had a value to take.
                  "inputs": [{ "name": "steps", "widget": { "name": "steps" }, "link": 7 }],
                  "widgets_values": [25, 8.0, "euler"] },
                { "id": 9, "type": "PrimitiveNode", "widgets_values": [30, "fixed"] }
            ],
            "links": [[7, 9, 0, 3, 0, "INT"]]
        });

        let workflow = from_ui(&ui, &schema).expect("readable");
        let inputs = &workflow.nodes["3"].inputs;
        assert_eq!(inputs["cfg"], Input::Value(json!(8.0)));
        assert_eq!(inputs["sampler_name"], Input::Value(json!("euler")));
        // And the linked one took the primitive's value rather than a wire the server cannot
        // resolve, because `PrimitiveNode` does not exist on the server.
        assert_eq!(inputs["steps"], Input::Value(json!(30)));
    }

    /// Trap 2. `control_after_generate` sits in the values and is not an input.
    #[test]
    fn the_value_after_a_seed_belongs_to_the_editor() {
        let schema = schema_for("KSampler", &[("seed", "INT"), ("steps", "INT")]);
        let ui = json!({
            "nodes": [{ "id": 3, "type": "KSampler",
                        "widgets_values": [721897303308196i64, "randomize", 20] }],
            "links": []
        });
        let inputs = &from_ui(&ui, &schema).expect("readable").nodes["3"].inputs;
        assert_eq!(inputs["seed"], Input::Value(json!(721897303308196i64)));
        assert_eq!(
            inputs["steps"],
            Input::Value(json!(20)),
            "not \"randomize\""
        );
    }

    /// Trap 3. Text for people, and wires that bend.
    #[test]
    fn notes_and_reroutes_never_reach_the_server() {
        let schema = schema_for("VAEDecode", &[]);
        let ui = json!({
            "nodes": [
                { "id": 1, "type": "Note", "widgets_values": ["remember to breathe"] },
                { "id": 2, "type": "MarkdownNote", "widgets_values": ["# hello"] },
                { "id": 3, "type": "VAEDecode", "inputs": [{ "name": "samples", "link": 5 }] },
                { "id": 4, "type": "Reroute", "inputs": [{ "name": "", "link": 6 }] },
                { "id": 7, "type": "VAEDecode", "inputs": [] }
            ],
            "links": [[5, 4, 0, 3, 0, "LATENT"], [6, 7, 0, 4, 0, "LATENT"]]
        });

        let workflow = from_ui(&ui, &schema).expect("readable");
        assert_eq!(
            workflow.nodes.len(),
            2,
            "two decorations and one reroute left"
        );
        // The reroute was followed through to the node actually producing the value.
        assert_eq!(
            workflow.nodes["3"].inputs["samples"],
            Input::From(vec![json!("7"), json!(0)])
        );
    }

    /// The prompt is found by asking the sampler which input it calls positive.
    ///
    /// Not by looking for `CLIPTextEncode` and hoping: a graph may have four of them, and which
    /// is which is a fact about the wires rather than about the class.
    #[test]
    fn the_sampler_says_which_words_are_the_picture_and_which_are_the_warning() {
        let raw = json!({
            "3": { "class_type": "KSampler", "inputs": {
                "positive": ["6", 0], "negative": ["7", 0], "latent_image": ["5", 0] } },
            "5": { "class_type": "EmptyLatentImage", "inputs": { "width": 512, "height": 512 } },
            "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "a lighthouse" } },
            "7": { "class_type": "CLIPTextEncode", "inputs": { "text": "blurry" } },
            "4": { "class_type": "CheckpointLoaderSimple", "inputs": { "ckpt_name": "sdxl.safetensors" } }
        });
        let opening = from_api(&raw).unwrap().opening();
        assert_eq!(opening.positive, vec!["6"]);
        assert_eq!(opening.negative, vec!["7"]);
        assert_eq!(opening.size.as_deref(), Some("5"));
        assert_eq!(opening.needs, vec!["sdxl.safetensors"]);
        assert!(opening.can.from_words);
        assert!(!opening.can.from_image, "nothing loads a picture");
    }

    /// Two samplers, two positives, and the author wrote the same sentence in both.
    ///
    /// Measured on ComfyUI's own SDXL template: base and refiner each get their own prompt pair.
    /// Writing to only the first would leave the refiner painting something else.
    #[test]
    fn a_prompt_is_written_everywhere_the_workflow_takes_one() {
        let raw = json!({
            "10": { "class_type": "KSamplerAdvanced", "inputs": { "positive": ["6", 0], "negative": ["7", 0] } },
            "11": { "class_type": "KSamplerAdvanced", "inputs": { "positive": ["15", 0], "negative": ["16", 0] } },
            "6":  { "class_type": "CLIPTextEncode", "inputs": { "text": "a bottle" } },
            "15": { "class_type": "CLIPTextEncode", "inputs": { "text": "a bottle" } },
            "7":  { "class_type": "CLIPTextEncode", "inputs": { "text": "watermark" } },
            "16": { "class_type": "CLIPTextEncode", "inputs": { "text": "watermark" } }
        });
        let mut workflow = from_api(&raw).unwrap();
        // Node ids are strings, so "10" sorts before "11": the base sampler is read first.
        // Deterministic, which is what matters — nothing downstream depends on which is which.
        assert_eq!(workflow.opening().positive, vec!["6", "15"]);

        workflow.say("a lighthouse", Some("blurry"));
        for id in ["6", "15"] {
            assert_eq!(
                workflow.nodes[id].inputs["text"],
                Input::Value(json!("a lighthouse")),
                "node {id}"
            );
        }
        for id in ["7", "16"] {
            assert_eq!(
                workflow.nodes[id].inputs["text"],
                Input::Value(json!("blurry"))
            );
        }
    }

    /// A prompt rarely reaches the sampler directly.
    #[test]
    fn the_words_are_found_through_whatever_is_in_the_way() {
        let raw = json!({
            "3": { "class_type": "KSampler", "inputs": { "positive": ["8", 0] } },
            "8": { "class_type": "ControlNetApply", "inputs": { "conditioning": ["6", 0], "strength": 1.0 } },
            "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "a lighthouse" } }
        });
        assert_eq!(from_api(&raw).unwrap().opening().positive, vec!["6"]);
    }

    /// What it can do is read off the graph, never off a manifest.
    #[test]
    fn abilities_come_from_the_nodes_that_are_there() {
        let raw = json!({
            "3": { "class_type": "KSampler", "inputs": { "positive": ["6", 0] } },
            "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "hello" } },
            "1": { "class_type": "LoadImage", "inputs": { "image": "photo.png" } },
            "2": { "class_type": "VAEEncodeForInpaint", "inputs": {} },
            "9": { "class_type": "ImageUpscaleWithModel", "inputs": {} }
        });
        let can = from_api(&raw).unwrap().opening().can;
        assert!(can.from_words && can.from_image && can.inpaint && can.upscale);
    }

    /// Shape and seed are set wherever the workflow keeps them.
    #[test]
    fn every_sampler_gets_the_seed() {
        let raw = json!({
            "5": { "class_type": "EmptyLatentImage", "inputs": { "width": 512, "height": 512, "batch_size": 1 } },
            "10": { "class_type": "KSamplerAdvanced", "inputs": { "noise_seed": 1, "steps": 20 } },
            "11": { "class_type": "KSamplerAdvanced", "inputs": { "noise_seed": 2, "steps": 20 } }
        });
        let mut workflow = from_api(&raw).unwrap();
        workflow.shape(1024, 768);
        workflow.seed(99);
        assert_eq!(
            workflow.nodes["5"].inputs["width"],
            Input::Value(json!(1024))
        );
        assert_eq!(
            workflow.nodes["5"].inputs["height"],
            Input::Value(json!(768))
        );
        assert_eq!(
            workflow.nodes["10"].inputs["noise_seed"],
            Input::Value(json!(99))
        );
        assert_eq!(
            workflow.nodes["11"].inputs["noise_seed"],
            Input::Value(json!(99))
        );
        // Untouched: the workflow's own answer to how hard it works.
        assert_eq!(
            workflow.nodes["10"].inputs["steps"],
            Input::Value(json!(20))
        );
    }

    /// The source is kept, and that is what makes *install it and press again* possible.
    #[test]
    fn a_workflow_that_cannot_be_read_yet_is_still_worth_keeping() {
        let source = json!({
            "nodes": [
                { "id": 1, "type": "VAEDecode", "inputs": [] },
                { "id": 2, "type": "IPAdapterApply", "inputs": [] }
            ],
            "links": []
        });

        // Today this machine does not have that node.
        let thin = schema_for("VAEDecode", &[]);
        assert_eq!(
            Imported::of(source.clone(), &thin).map(|_| ()),
            Err(Unreadable::Missing(vec!["IPAdapterApply".to_owned()]))
        );

        // Somebody installs it. The same bytes now compile — which is only possible because the
        // original was never thrown away in favour of a conversion that did not happen.
        let mut wide = thin.clone();
        wide.classes
            .insert("IPAdapterApply".to_owned(), Widgets::default());
        let mut taken = Imported::of(source.clone(), &wide).expect("now it reads");
        assert_eq!(taken.compiled.nodes.len(), 2);
        assert_eq!(taken.source, source, "kept exactly as it arrived");

        // And it can be compiled again against a different server without the file.
        taken.compiled = Workflow::default();
        taken.recompile(&wide).expect("rebuilt from the source");
        assert_eq!(taken.compiled.nodes.len(), 2);
    }

    /// Finding thirty settings must not put thirty fields on a screen.
    #[test]
    fn most_of_a_workflow_is_kept_out_of_sight() {
        let mut schema = schema_for(
            "KSampler",
            &[
                ("seed", "INT"),
                ("steps", "INT"),
                ("cfg", "FLOAT"),
                ("sampler_name", "CHOICE"),
                ("scheduler", "CHOICE"),
            ],
        );
        schema.classes.insert(
            "EmptyLatentImage".to_owned(),
            Widgets {
                told: true,
                order: vec![
                    Widget {
                        name: "width".into(),
                        kind: "INT".into(),
                        ..Widget::default()
                    },
                    Widget {
                        name: "height".into(),
                        kind: "INT".into(),
                        ..Widget::default()
                    },
                ],
            },
        );
        schema.classes.insert(
            "CLIPTextEncode".to_owned(),
            Widgets {
                told: true,
                order: vec![Widget {
                    name: "text".into(),
                    kind: "STRING".into(),
                    multiline: true,
                    ..Widget::default()
                }],
            },
        );

        let source = json!({
            "3": { "class_type": "KSampler", "inputs": {
                "seed": 1, "steps": 20, "cfg": 8.0,
                "sampler_name": "euler", "scheduler": "normal",
                "positive": ["6", 0], "negative": ["7", 0] } },
            "5": { "class_type": "EmptyLatentImage", "inputs": { "width": 1024, "height": 1024 } },
            "6": { "class_type": "CLIPTextEncode", "inputs": { "text": "a lighthouse" } },
            "7": { "class_type": "CLIPTextEncode", "inputs": { "text": "blurry" } }
        });
        let taken = Imported::of(source, &schema).expect("readable");
        let controls = taken.controls(&schema);

        let at = |tier: Tier| -> Vec<String> {
            controls
                .iter()
                .filter(|c| c.tier == tier)
                .map(|c| c.input.clone())
                .collect()
        };

        // What a person expects to see, and nothing else.
        assert_eq!(
            at(Tier::General),
            vec!["text", "height", "width"],
            "{controls:#?}"
        );
        // The negative prompt leads Advanced for the same reason the positive leads General.
        assert_eq!(at(Tier::Advanced), vec!["text", "cfg", "seed", "steps"]);
        // Which sampler and which scheduler are the workflow's business — folded away, and
        // still reachable. Nothing is withheld.
        assert_eq!(at(Tier::Everything), vec!["sampler_name", "scheduler"]);

        // And the two prompts are named for what they are, not for their field.
        let positive = controls.iter().find(|c| c.node == "6").unwrap();
        assert_eq!(positive.label, "What it should contain");
        assert!(positive.multiline, "a prompt is a box, not a line");
        let negative = controls.iter().find(|c| c.node == "7").unwrap();
        assert_eq!(negative.label, "What it should avoid");
    }

    /// Nothing a workflow can be set to is unreachable.
    ///
    /// **The owner's rule, 2026-08-22: *the user chooses everything*.** Which is not the same
    /// as being asked about everything — ADR-0026's default still holds, and the default is that
    /// they touch nothing. So every setting has a tier and no setting has none: folded away is
    /// a place, withheld is not.
    #[test]
    fn every_setting_is_reachable_and_the_model_is_a_choice() {
        let mut schema = schema_for("KSampler", &[("sampler_name", "CHOICE")]);
        schema.classes.insert(
            "CheckpointLoaderSimple".to_owned(),
            Widgets {
                told: true,
                order: vec![Widget {
                    name: "ckpt_name".into(),
                    kind: "CHOICE".into(),
                    // What the server says is installed. A list, never a filename to type.
                    choices: vec!["sdxl.safetensors".into(), "flux.safetensors".into()],
                    ..Widget::default()
                }],
            },
        );

        let source = json!({
            "3": { "class_type": "KSampler", "inputs": { "sampler_name": "euler" } },
            "4": { "class_type": "CheckpointLoaderSimple",
                   "inputs": { "ckpt_name": "sdxl.safetensors" } }
        });
        let taken = Imported::of(source, &schema).expect("readable");
        let controls = taken.controls(&schema);

        // Every value the workflow holds is somewhere a person can get to.
        assert_eq!(controls.len(), 2, "{controls:#?}");

        // **Which model draws it is shown.** A value filled in is not a question being asked;
        // it is an answer somebody can read and change.
        let model = controls.iter().find(|c| c.input == "ckpt_name").unwrap();
        assert_eq!(model.tier, Tier::General);
        assert_eq!(model.choices.len(), 2, "the installed ones, as a list");

        // The sampler stays folded, because it is the workflow's own business — and it is still
        // there for whoever wants it.
        let sampler = controls.iter().find(|c| c.input == "sampler_name").unwrap();
        assert_eq!(sampler.tier, Tier::Everything);
    }

    /// The author of a workflow has already answered what matters, in a quarter of them.
    #[test]
    fn what_the_author_exposed_beats_the_heuristic() {
        let schema = schema_for("KSampler", &[("sampler_name", "CHOICE")]);
        let source = json!({
            "nodes": [
                { "id": 3, "type": "KSampler", "widgets_values": ["euler"], "inputs": [] },
                // `properties` sits on any node. In the wild the one carrying this is usually
                // a subgraph — see `a_subgraph_is_named_rather_than_guessed_at`.
                { "id": 9, "type": "KSampler", "widgets_values": ["euler"], "inputs": [],
                  "properties": { "proxyWidgets": [["3", "sampler_name"]] } }
            ],
            "links": []
        });
        let taken = Imported::of(source, &schema).expect("readable");
        let controls = taken.controls(&schema);
        let control = controls
            .iter()
            .find(|c| c.node == "3")
            .expect("the exposed one");

        // Left to Epoch, which sampler is internal. The author said otherwise.
        assert_eq!(control.tier, Tier::Advanced);
        assert!(control.exposed);
        // And the one nobody exposed stays out of sight.
        assert_eq!(
            controls.iter().find(|c| c.node == "9").unwrap().tier,
            Tier::Everything
        );
    }

    /// Exposing something lifts it to Advanced, never onto the first screen.
    #[test]
    fn an_author_who_exposed_eleven_knobs_did_not_ask_for_eleven_fields() {
        let schema = schema_for("Thing", &[("a", "INT"), ("b", "INT"), ("c", "INT")]);
        let source = json!({
            "nodes": [
                { "id": 3, "type": "Thing", "widgets_values": [1, 2, 3], "inputs": [] },
                { "id": 9, "type": "Thing", "widgets_values": [0, 0, 0], "inputs": [],
                  "properties": { "proxyWidgets": [["3", "a"], ["3", "b"], ["3", "c"]] } }
            ],
            "links": []
        });
        let taken = Imported::of(source, &schema).expect("readable");
        let controls = taken.controls(&schema);
        let exposed: Vec<&Control> = controls.iter().filter(|c| c.node == "3").collect();
        assert_eq!(exposed.len(), 3);
        assert!(
            exposed.iter().all(|c| c.tier == Tier::Advanced),
            "{exposed:#?}"
        );
        assert!(!controls.iter().any(|c| c.tier == Tier::General));
    }

    /// A node the author renamed says what it is better than we can.
    #[test]
    fn a_title_the_author_wrote_is_the_label() {
        let schema = schema_for("Thing", &[("value", "INT")]);
        let source = json!({
            "nodes": [{ "id": 3, "type": "Thing", "title": "How many frogs",
                        "widgets_values": [4], "inputs": [] }],
            "links": []
        });
        let taken = Imported::of(source, &schema).expect("readable");
        assert_eq!(taken.controls(&schema)[0].label, "How many frogs");
    }

    /// A muted node is passed through, which is what bypassing one means in the editor.
    #[test]
    fn a_bypassed_node_hands_on_what_it_was_given() {
        let schema = schema_for("VAEDecode", &[]);
        let ui = json!({
            "nodes": [
                { "id": 3, "type": "VAEDecode", "inputs": [{ "name": "samples", "link": 5 }] },
                { "id": 4, "type": "VAEDecode", "mode": 4, "inputs": [{ "name": "samples", "link": 6 }] },
                { "id": 7, "type": "VAEDecode", "inputs": [] }
            ],
            "links": [[5, 4, 0, 3, 0, "LATENT"], [6, 7, 0, 4, 0, "LATENT"]]
        });
        let workflow = from_ui(&ui, &schema).expect("readable");
        assert!(!workflow.nodes.contains_key("4"), "muted nodes do not run");
        assert_eq!(
            workflow.nodes["3"].inputs["samples"],
            Input::From(vec![json!("7"), json!(0)])
        );
    }

    /// A workflow needing something this machine has not installed is refused **by name**.
    ///
    /// "Needs 2 custom nodes" sends somebody hunting. `IPAdapterApply` is searchable.
    #[test]
    fn a_missing_custom_node_is_named() {
        let schema = schema_for("VAEDecode", &[]);
        let ui = json!({
            "nodes": [
                { "id": 1, "type": "VAEDecode", "inputs": [] },
                { "id": 2, "type": "IPAdapterApply", "inputs": [] }
            ],
            "links": []
        });
        assert_eq!(
            from_ui(&ui, &schema),
            Err(Unreadable::Missing(vec!["IPAdapterApply".to_owned()]))
        );
    }

    #[test]
    fn the_shape_decides_which_reader_runs() {
        let schema = schema_for("VAEDecode", &[]);
        let api = json!({ "1": { "class_type": "VAEDecode", "inputs": {} } });
        assert!(read(&api, &schema).is_ok());

        let ui = json!({ "nodes": [{ "id": 1, "type": "VAEDecode", "inputs": [] }], "links": [] });
        assert!(read(&ui, &schema).is_ok());

        assert_eq!(
            read(&json!({ "hello": "world" }), &schema),
            Err(Unreadable::NotAWorkflow)
        );
    }

    /// The schema is read from the server's own answer, in the server's own order.
    ///
    /// The shape is `KSamplerAdvanced`'s, copied from a live `/object_info` on 2026-08-22.
    #[test]
    fn a_widget_is_a_primitive_or_a_list_of_choices_in_the_order_the_server_gives() {
        let info = json!({
            // Something has to *produce* `MODEL` for `MODEL` to be a socket — which is the rule
            // now, and it is the server's own answer rather than a list of type names.
            "CheckpointLoaderSimple": {
                "input": { "required": { "ckpt_name": ["COMBO", { "options": ["sdxl.safetensors"] }] } },
                "input_order": { "required": ["ckpt_name"] },
                "output": ["MODEL", "CLIP", "VAE"]
            },
            "KSampler": {
                "input": { "required": {
                    "model": ["MODEL"],
                    "seed": ["INT", { "default": 0 }],
                    "sampler_name": [["euler", "heun"]],
                    "cfg": ["FLOAT", { "default": 8.0 }],
                    // Nothing produces this type, so it cannot be on the end of a wire. One of
                    // these sat *first* on a real node and shifted every value after it by one.
                    "mode": ["COMFY_DYNAMICCOMBO_V3", {}]
                }},
                // Not alphabetical, and that is the point: `serde_json` sorts the object above
                // into `cfg, mode, model, sampler_name, seed`, which is nothing's layout.
                "input_order": { "required": ["model", "mode", "seed", "cfg", "sampler_name"] }
            }
        });
        let schema = Schema::read(&info);
        let names: Vec<&str> = schema.classes["KSampler"]
            .order
            .iter()
            .map(|w| w.name.as_str())
            .collect();
        // `model` is a socket another node plugs into, not a value somebody types.
        assert_eq!(
            names,
            vec!["mode", "seed", "cfg", "sampler_name"],
            "in the server's order, and `mode` is a value because nothing produces its type"
        );
        // A COMBO's options are read from beside it, which is how newer nodes declare them.
        let ckpt = &schema.classes["CheckpointLoaderSimple"].order[0];
        assert_eq!(ckpt.choices, vec!["sdxl.safetensors"]);
    }

    /// A server that will not say the order gets no guess, and the workflow is refused.
    ///
    /// This is the cold-instrument rule where it costs something: refusing an import is
    /// annoying, and running a workflow with `cfg` where `steps` belongs produces a picture
    /// nobody can explain and no error anybody can find.
    #[test]
    fn without_a_published_order_a_workflow_is_refused_rather_than_guessed_at() {
        let info = json!({ "KSampler": { "input": { "required": { "seed": ["INT"] } } } });
        let schema = Schema::read(&info);
        assert!(schema.classes["KSampler"].order.is_empty(), "no guess");

        let ui = json!({
            "nodes": [{ "id": 3, "type": "KSampler", "widgets_values": [42] }],
            "links": []
        });
        assert_eq!(
            from_ui(&ui, &schema),
            Err(Unreadable::Unaligned("KSampler".to_owned()))
        );

        // A node with no values to align is unaffected: there is nothing to get wrong.
        let empty = json!({
            "nodes": [{ "id": 3, "type": "KSampler", "widgets_values": [] }],
            "links": []
        });
        assert!(from_ui(&empty, &schema).is_ok());
    }
}

#[cfg(test)]
mod against_a_real_comfyui {
    use super::*;

    /// The whole conversion, against ComfyUI's own SDXL template and a running server.
    ///
    /// `#[ignore]` because it needs one — run it with
    /// `cargo test -p epoch-models -- --ignored converts_and_runs`.
    ///
    /// It exists because the three traps above were each *plausible* when wrong: the workflow
    /// converted, the JSON looked right, and the server refused it — or worse, would have run it
    /// with `cfg` where `steps` belongs. Unit tests hold the rules; only a run proves them.
    ///
    /// The template asks for the SDXL refiner, which this machine does not have, so the refiner
    /// loader is pointed at the base model. That is a substitution for the *test*, not something
    /// Epoch ever does to somebody's workflow.
    #[test]
    #[ignore = "needs ComfyUI serving on 8188 with sd_xl_base_1.0"]
    fn converts_and_runs_comfyuis_own_template() {
        let host = "http://127.0.0.1:8188";
        let info: serde_json::Value = ureq::get(&format!("{host}/object_info"))
            .call()
            .expect("ComfyUI answers")
            .into_json()
            .expect("object_info is JSON");
        let schema = Schema::read(&info);
        assert!(
            schema.classes.len() > 100,
            "a real server knows hundreds of nodes, saw {}",
            schema.classes.len()
        );

        let template = std::path::Path::new(&std::env::var("LOCALAPPDATA").unwrap())
            .join("Comfy-Desktop/ComfyUI-Installs/ComfyUI/ComfyUI/.venv/Lib/site-packages")
            .join("comfyui_workflow_templates_json/templates/sdxl_simple_example.json");
        let Ok(raw) = std::fs::read_to_string(&template) else {
            eprintln!("no template at {}, nothing to convert", template.display());
            return;
        };

        let ui: serde_json::Value = serde_json::from_str(&raw).expect("the template is JSON");
        let mut workflow = read(&ui, &schema).expect("it converts");
        assert_eq!(
            workflow.nodes.len(),
            11,
            "25 nodes, of which 11 execute: {:?}",
            workflow.nodes.keys().collect::<Vec<_>>()
        );

        // **What the graph left open, read off the graph.** The template has four text nodes —
        // a pair for the base pass and a pair for the refiner — and the derivation finds both
        // positives rather than the first one.
        let opening = workflow.opening();
        assert_eq!(opening.positive.len(), 2, "{opening:?}");
        assert_eq!(opening.negative.len(), 2, "{opening:?}");
        assert!(opening.size.is_some(), "EmptyLatentImage: {opening:?}");
        assert_eq!(opening.seeds.len(), 2, "two samplers, a seed each");
        assert!(opening.can.from_words && !opening.can.from_image);
        assert!(
            opening.needs.iter().any(|n| n.contains("sd_xl_base")),
            "it names the files it needs: {:?}",
            opening.needs
        );

        // And Epoch's own words go in, which is the whole point of finding them.
        workflow.say(
            "a lighthouse on a cliff at dawn, storm clouds",
            Some("blurry, watermark"),
        );
        workflow.shape(1024, 1024);
        workflow.seed(4242);

        for node in workflow.nodes.values_mut() {
            if node.class_type == "CheckpointLoaderSimple" {
                if let Some(Input::Value(name)) = node.inputs.get("ckpt_name") {
                    if name.as_str().is_some_and(|n| n.contains("refiner")) {
                        node.inputs.insert(
                            "ckpt_name".into(),
                            Input::Value(serde_json::json!("sd_xl_base_1.0.safetensors")),
                        );
                    }
                }
            }
            if node.class_type == "SaveImage" {
                node.inputs.insert(
                    "filename_prefix".into(),
                    Input::Value(serde_json::json!("epoch-rust-converted")),
                );
            }
        }

        let answered = ureq::post(&format!("{host}/prompt")).send_json(serde_json::json!({
            "prompt": workflow.nodes,
            "client_id": "epoch-workflow-test",
        }));
        // The server's own words when it refuses. A 400 with the reason thrown away is a whole
        // afternoon of guessing.
        let queued: serde_json::Value = match answered {
            Ok(response) => response.into_json().expect("it answers with JSON"),
            Err(ureq::Error::Status(_, response)) => {
                panic!("refused: {}", response.into_string().unwrap_or_default())
            }
            Err(other) => panic!("{other}"),
        };
        let id = queued["prompt_id"]
            .as_str()
            .expect("a prompt id")
            .to_owned();

        let started = std::time::Instant::now();
        let images = loop {
            let history: serde_json::Value = ureq::get(&format!("{host}/history/{id}"))
                .call()
                .expect("history answers")
                .into_json()
                .expect("history is JSON");
            if let Some(entry) = history.get(&id) {
                assert_eq!(
                    entry["status"]["status_str"].as_str(),
                    Some("success"),
                    "{entry}"
                );
                break entry["outputs"].clone();
            }
            assert!(started.elapsed().as_secs() < 600, "it never finished");
            std::thread::sleep(std::time::Duration::from_secs(3));
        };

        let made: Vec<&serde_json::Value> = images
            .as_object()
            .map(|out| {
                out.values()
                    .filter_map(|o| o.get("images"))
                    .filter_map(|i| i.as_array())
                    .flatten()
                    .collect()
            })
            .unwrap_or_default();
        assert!(!made.is_empty(), "a picture came back: {images}");
        println!(
            "converted and ran in {:.1}s -> {}",
            started.elapsed().as_secs_f32(),
            made[0]["filename"].as_str().unwrap_or("?")
        );
    }
}

#[cfg(test)]
mod the_one_epoch_writes {
    use super::*;

    /// A schema holding exactly the seven core classes, shaped the way ComfyUI reports them.
    /// The classes [`core`] declares, plus whatever else a case needs.
    ///
    /// Built the same way `core` builds its own rather than by taking one apart: a `Schema` is
    /// what a server answered, and there is one way to read one.
    fn schema_plus(classes: &[&str]) -> Schema {
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentImage", vec!["width", "height", "batch_size"]),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecode", vec!["samples", "vae"]),
            ("SaveImage", vec!["images", "filename_prefix"]),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        for class in classes {
            info.insert(
                (*class).to_owned(),
                serde_json::json!({
                    "input": { "required": {} },
                    "input_order": { "required": [] },
                }),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    fn core() -> Schema {
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentImage", vec!["width", "height", "batch_size"]),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecode", vec!["samples", "vae"]),
            ("SaveImage", vec!["images", "filename_prefix"]),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    /// The two upscale classes, as this ComfyUI reports them.
    ///
    /// Measured 2026-08-25 against the live server rather than remembered:
    /// `UpscaleModelLoader` takes `model_name` and answers `UPSCALE_MODEL`;
    /// `ImageUpscaleWithModel` takes that and an `image`.
    fn core_with_upscaling() -> Schema {
        // Rebuilt rather than extended, because `Schema` keeps what it read and not the JSON it
        // read it from — the same shape `core()` builds, plus the two this test is about.
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentImage", vec!["width", "height", "batch_size"]),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecode", vec!["samples", "vae"]),
            ("SaveImage", vec!["images", "filename_prefix"]),
            ("UpscaleModelLoader", vec!["model_name"]),
            ("ImageUpscaleWithModel", vec!["upscale_model", "image"]),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    /// Steering, plus the one preparation this ComfyUI has.
    ///
    /// `Canny`'s two thresholds carry the defaults the server declares — `0.4` and `0.8`, read
    /// from its own `/object_info` — because a test that invents them would pass against a graph
    /// nobody could run.
    fn core_with_preparing() -> Schema {
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentImage", vec!["width", "height", "batch_size"]),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecode", vec!["samples", "vae"]),
            ("SaveImage", vec!["images", "filename_prefix"]),
            ("ControlNetLoader", vec!["control_net_name"]),
            ("LoadImage", vec!["image"]),
            (
                "ControlNetApplyAdvanced",
                vec![
                    "positive",
                    "negative",
                    "control_net",
                    "image",
                    "strength",
                    "start_percent",
                    "end_percent",
                ],
            ),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        info.insert(
            "Canny".to_owned(),
            serde_json::json!({
                "input": { "required": {
                    "image": ["IMAGE", {}],
                    "low_threshold": ["FLOAT", { "default": 0.4, "min": 0.01, "max": 0.99 }],
                    "high_threshold": ["FLOAT", { "default": 0.8, "min": 0.01, "max": 0.99 }],
                }},
                "input_order": { "required": ["image", "low_threshold", "high_threshold"] },
                "output": ["IMAGE"],
            }),
        );
        Schema::read(&serde_json::Value::Object(info))
    }

    /// Every class a composed graph can name, so a test may ask for all of them at once.
    ///
    /// Built the same way the others are — the server's own shape, reduced to what the composer
    /// is checked against — rather than by extending one of them, because `Schema` keeps what it
    /// read and not the JSON it read it from.
    fn everything() -> Schema {
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentImage", vec!["width", "height", "batch_size"]),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecode", vec!["samples", "vae"]),
            ("SaveImage", vec!["images", "filename_prefix"]),
            ("LoraLoader", vec!["model", "clip", "lora_name"]),
            ("UpscaleModelLoader", vec!["model_name"]),
            ("ImageUpscaleWithModel", vec!["upscale_model", "image"]),
            ("ControlNetLoader", vec!["control_net_name"]),
            ("LoadImage", vec!["image"]),
            (
                "ControlNetApplyAdvanced",
                vec![
                    "positive",
                    "negative",
                    "control_net",
                    "image",
                    "strength",
                    "start_percent",
                    "end_percent",
                ],
            ),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    /// The three ControlNet classes, as this ComfyUI reports them.
    ///
    /// **Read from the server, 2026-08-27**, the same way the upscale pair was:
    /// `ControlNetLoader` takes `control_net_name` and answers `CONTROL_NET`; `LoadImage` takes
    /// an `image` name and answers `IMAGE`; `ControlNetApplyAdvanced` takes `positive`,
    /// `negative`, `control_net`, `image`, `strength`, `start_percent` and `end_percent`, and
    /// answers **two** conditionings.
    fn core_with_steering() -> Schema {
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentImage", vec!["width", "height", "batch_size"]),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecode", vec!["samples", "vae"]),
            ("SaveImage", vec!["images", "filename_prefix"]),
            ("LoraLoader", vec!["model", "clip", "lora_name"]),
            ("ControlNetLoader", vec!["control_net_name"]),
            ("LoadImage", vec!["image"]),
            (
                "ControlNetApplyAdvanced",
                vec![
                    "positive",
                    "negative",
                    "control_net",
                    "image",
                    "strength",
                    "start_percent",
                    "end_percent",
                ],
            ),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    /// A schema with the video classes on it, as this ComfyUI publishes them.
    ///
    /// **Read from the server, 2026-08-28**, the same way the ControlNet and upscale classes
    /// were: `EmptyLTXVLatentVideo` takes a width, a height, a `length` and a batch;
    /// `LTXVConditioning` takes both conditionings and a `frame_rate` and answers **two**;
    /// `CreateVideo` takes `images` and an `fps` and answers a VIDEO; `SaveVideo` takes that
    /// video, a prefix, a `format` and a `codec`.
    fn core_that_moves() -> Schema {
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentImage", vec!["width", "height", "batch_size"]),
            (
                "EmptyLTXVLatentVideo",
                vec!["width", "height", "length", "batch_size"],
            ),
            (
                "LTXVConditioning",
                vec!["positive", "negative", "frame_rate"],
            ),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecode", vec!["samples", "vae"]),
            ("SaveImage", vec!["images", "filename_prefix"]),
            ("CreateVideo", vec!["images", "fps"]),
            (
                "SaveVideo",
                vec!["video", "filename_prefix", "format", "codec"],
            ),
            ("LoraLoader", vec!["model", "clip", "lora_name"]),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    fn moving(prompt: &str, frames: i64, fps: f64) -> Ask {
        let mut ask = Ask::of("ltxv-2b-0.9.6-distilled-04-25.safetensors", prompt);
        ask.beyond = Some(Beyond::Moving {
            family: "LTXV".to_owned(),
            frames,
            fps,
        });
        ask
    }

    /// **The whole difference between a picture and a video, asserted by following the wires.**
    ///
    /// Not by node id: the ids are what the LoRA/upscale collision was made of, and a test that
    /// names them proves the numbering rather than the graph. Every claim here is *what class is
    /// on the other end of this input*.
    ///
    /// Measured against the real server first (2026-08-28): this exact graph, 49 frames at
    /// 512x320 through `ltxv-2b-0.9.6-distilled`, drew in **10.1 s** and answered with an
    /// `epoch_00101_.mp4`.
    #[test]
    fn a_video_starts_from_a_video_latent_and_ends_in_a_muxer() {
        let graph = compose(
            &moving("a red car on a coast road", 49, 25.0),
            &core_that_moves(),
        )
        .expect("every video class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };
        let goes_to = |id: &str, input: &str| {
            nodes[id]["inputs"][input][0]
                .as_str()
                .expect("a wire, not a value")
                .to_owned()
        };

        // It starts from a latent that has a duration, and the duration is on it.
        assert_eq!(class(&goes_to("5", "latent_image")), "EmptyLTXVLatentVideo");
        assert_eq!(nodes[&goes_to("5", "latent_image")]["inputs"]["length"], 49);

        // The family's conditioning is the last thing before the sampler, on both sides, and it
        // carries the frame rate — which is how LTXV is told how fast the thing it makes moves.
        for side in ["positive", "negative"] {
            assert_eq!(class(&goes_to("5", side)), "LTXVConditioning");
        }
        assert_eq!(
            nodes[&goes_to("5", "positive")]["inputs"]["frame_rate"],
            25.0
        );
        // Both outputs of it, not the same one twice: the node answers a pair.
        assert_eq!(nodes["5"]["inputs"]["positive"][1], 0);
        assert_eq!(nodes["5"]["inputs"]["negative"][1], 1);

        // And the decode is muxed rather than saved, at the same rate it was conditioned with.
        assert_eq!(class(&goes_to("7", "video")), "CreateVideo");
        assert_eq!(
            class(&goes_to(&goes_to("7", "video"), "images")),
            "VAEDecode"
        );
        assert_eq!(nodes[&goes_to("7", "video")]["inputs"]["fps"], 25.0);
        assert_eq!(class("7"), "SaveVideo");

        // Nothing saves a picture, which is the half a `SaveImage` left behind would break
        // silently: the render would work and produce two outputs.
        assert!(
            !nodes.values().any(|n| n["class_type"] == "SaveImage"),
            "a video graph saves a video and nothing else: {graph}"
        );
    }

    /// A sound schema, as this ComfyUI publishes it.
    ///
    /// **Read from the server, 2026-08-28**: `EmptyLatentAudio` takes `seconds` and a batch and
    /// no pixels at all; `ConditioningStableAudio` takes both conditionings plus a
    /// `seconds_start` and a `seconds_total` and answers **two**; `VAEDecodeAudio` gives an
    /// `AUDIO`; `SaveAudio` writes it.
    /// A schema with ACE-Step 1.5's own classes, and its encoder's declared defaults.
    ///
    /// The defaults are the node's own, read from this ComfyUI's `comfy_extras/nodes_ace.py`
    /// (2026-08-29). A test that invented them would pass against a graph nobody could run.
    fn core_that_sings() -> Schema {
        let mut info = serde_json::Map::new();
        let plain: Vec<(&str, Vec<&str>)> = vec![
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("EmptyAceStep1.5LatentAudio", vec!["seconds", "batch_size"]),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecodeAudio", vec!["samples", "vae"]),
            ("SaveAudio", vec!["audio", "filename_prefix"]),
        ];
        for (class, required) in plain {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        // The encoder, with the widgets that carry defaults.
        let mut inputs = serde_json::Map::new();
        inputs.insert("clip".to_owned(), serde_json::json!([["CLIP"], {}]));
        inputs.insert("tags".to_owned(), serde_json::json!(["STRING", {}]));
        inputs.insert("lyrics".to_owned(), serde_json::json!(["STRING", {}]));
        inputs.insert(
            "seed".to_owned(),
            serde_json::json!(["INT", { "default": 0 }]),
        );
        inputs.insert(
            "bpm".to_owned(),
            serde_json::json!(["INT", { "default": 120 }]),
        );
        inputs.insert(
            "duration".to_owned(),
            serde_json::json!(["FLOAT", { "default": 120.0 }]),
        );
        // **No `default`, because the node declares none.** Measured on this ComfyUI: these
        // two are `IO.Combo.Input(options=[...])` and nothing else, and a graph built from
        // `default` alone omitted them — `required_input_missing: timesignature`, after the form
        // had been filled in perfectly.
        inputs.insert(
            "timesignature".to_owned(),
            serde_json::json!([["2", "3", "4", "6"], {}]),
        );
        inputs.insert(
            "language".to_owned(),
            serde_json::json!([["en", "es"], { "default": "en" }]),
        );
        inputs.insert(
            "keyscale".to_owned(),
            serde_json::json!([["C major", "A minor"], {}]),
        );
        inputs.insert(
            "generate_audio_codes".to_owned(),
            serde_json::json!(["BOOLEAN", { "default": true }]),
        );
        inputs.insert(
            "cfg_scale".to_owned(),
            serde_json::json!(["FLOAT", { "default": 2.0 }]),
        );
        inputs.insert(
            "temperature".to_owned(),
            serde_json::json!(["FLOAT", { "default": 0.85 }]),
        );
        inputs.insert(
            "top_p".to_owned(),
            serde_json::json!(["FLOAT", { "default": 0.9 }]),
        );
        inputs.insert(
            "top_k".to_owned(),
            serde_json::json!(["INT", { "default": 0 }]),
        );
        inputs.insert(
            "min_p".to_owned(),
            serde_json::json!(["FLOAT", { "default": 0.0 }]),
        );
        info.insert(
            "TextEncodeAceStepAudio1.5".to_owned(),
            serde_json::json!({
                "input": { "required": inputs },
                "input_order": { "required": [
                    "clip", "tags", "lyrics", "seed", "bpm", "duration", "timesignature",
                    "language", "keyscale", "generate_audio_codes", "cfg_scale", "temperature",
                    "top_p", "top_k", "min_p",
                ] },
            }),
        );
        Schema::read(&serde_json::Value::Object(info))
    }

    /// **A song is told two things, and one of them is what is sung.**
    ///
    /// The lyrics reached the encoder as an empty string from the day sound worked, with a
    /// comment saying the panel had nowhere to type them. This asserts the whole way through:
    /// the words land on `lyrics`, the description lands on `tags`, and they are not one field.
    #[test]
    fn what_is_sung_is_not_what_describes_the_song() {
        let mut ask = Ask::of("ace_step_1.5_turbo_aio.safetensors", "reggaeton, spanish");
        ask.beyond = Some(Beyond::Sound {
            family: "ACE-Step 1.5".to_owned(),
            seconds: 60.0,
            lyrics: "yo, me la paso pensando".to_owned(),
        });
        let graph = compose(&ask, &core_that_sings()).expect("every 1.5 class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let encode = nodes
            .values()
            .find(|n| n["class_type"] == "TextEncodeAceStepAudio1.5")
            .expect("its own encoder, not v1's");
        assert_eq!(encode["inputs"]["tags"], "reggaeton, spanish");
        assert_eq!(encode["inputs"]["lyrics"], "yo, me la paso pensando");

        // **The duration is one number in two places.** The encoder is told how long the song is
        // and the latent is sized for it; two numbers that could disagree would generate one
        // length and describe another.
        assert_eq!(encode["inputs"]["duration"], 60.0);
        let latent = nodes
            .values()
            .find(|n| n["class_type"] == "EmptyAceStep1.5LatentAudio")
            .expect("1.5's latent, not v1's");
        assert_eq!(latent["inputs"]["seconds"], 60.0);

        // **Everything Epoch does not know comes from the node's own default.** A key, a time
        // signature, a tempo and the language model's sampling knobs are values a person would
        // have to be asked for, and asking is how a panel stops being a panel.
        assert_eq!(encode["inputs"]["bpm"], 120);
        // **A combo with no declared default still has one: its first option**, which is what
        // the editor puts in the box. These two are exactly the inputs ComfyUI refused the graph
        // over, so the assertion is on the value rather than on its presence.
        assert_eq!(encode["inputs"]["timesignature"], "2");
        assert_eq!(encode["inputs"]["keyscale"], "C major");
        assert_eq!(encode["inputs"]["generate_audio_codes"], true);
        assert_eq!(encode["inputs"]["temperature"], 0.85);
    }

    /// And an instrumental is a song with no words, not a defect.
    #[test]
    fn no_lyrics_is_an_instrumental() {
        let mut ask = Ask::of("ace_step_1.5_turbo_aio.safetensors", "lofi piano");
        ask.beyond = Some(Beyond::Sound {
            family: "ACE-Step 1.5".to_owned(),
            seconds: 30.0,
            lyrics: String::new(),
        });
        let graph = compose(&ask, &core_that_sings()).expect("every class is there");
        let encode = graph
            .as_object()
            .expect("a flat map")
            .values()
            .find(|n| n["class_type"] == "TextEncodeAceStepAudio1.5")
            .expect("the encoder");
        assert_eq!(encode["inputs"]["lyrics"], "");
        assert_eq!(encode["inputs"]["tags"], "lofi piano");
    }

    fn core_that_sounds() -> Schema {
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("CLIPLoader", vec!["clip_name", "type"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentAudio", vec!["seconds", "batch_size"]),
            (
                "ConditioningStableAudio",
                vec!["positive", "negative", "seconds_start", "seconds_total"],
            ),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecodeAudio", vec!["samples", "vae"]),
            ("SaveAudio", vec!["audio", "filename_prefix"]),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    /// **Sound has no width and no height, and that is the whole of what is different.**
    ///
    /// Measured against the real server first (2026-08-28): this exact graph made ten seconds of
    /// audio in 6.1 s and three seconds in 4.1 s, and answered with an `epoch_00112.flac`
    /// reported under the `audio` key rather than `images`.
    #[test]
    fn a_sound_starts_from_a_duration_and_ends_in_a_sound_file() {
        let mut ask = Ask::of("stable-audio-open-1.0.safetensors", "a snes button blip");
        ask.beyond = Some(Beyond::Sound {
            family: "Stable Audio".to_owned(),
            seconds: 10.0,
            lyrics: String::new(),
        });
        let graph = compose(&ask, &core_that_sounds()).expect("every audio class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };
        let goes_to = |id: &str, input: &str| {
            nodes[id]["inputs"][input][0]
                .as_str()
                .expect("a wire, not a value")
                .to_owned()
        };

        // The latent is a duration and nothing else. **Sending pixels to it would be an unknown
        // input**, which ComfyUI refuses as firmly as a wrong value.
        let latent = goes_to("5", "latent_image");
        assert_eq!(class(&latent), "EmptyLatentAudio");
        assert_eq!(nodes[&latent]["inputs"]["seconds"], 10.0);
        for pixels in ["width", "height", "length"] {
            assert!(
                nodes[&latent]["inputs"].get(pixels).is_none(),
                "sound has no {pixels}: {graph}"
            );
        }

        // The family's conditioning carries **the same** duration into both sides. A graph that
        // told the latent one number and the conditioning another would generate one length and
        // describe a different one.
        for side in ["positive", "negative"] {
            assert_eq!(class(&goes_to("5", side)), "ConditioningStableAudio");
        }
        let carried = goes_to("5", "positive");
        assert_eq!(nodes[&carried]["inputs"]["seconds_total"], 10.0);
        assert_eq!(nodes[&carried]["inputs"]["seconds_start"], 0.0);
        assert_eq!(nodes["5"]["inputs"]["positive"][1], 0);
        assert_eq!(nodes["5"]["inputs"]["negative"][1], 1);

        // And it is decoded as sound and saved as sound — not through `VAEDecode`, which gives
        // frames, and not through `SaveImage`.
        assert_eq!(class(&goes_to("7", "audio")), "VAEDecodeAudio");
        assert_eq!(class("7"), "SaveAudio");
        for absent in ["VAEDecode", "SaveImage", "CreateVideo", "SaveVideo"] {
            assert!(
                !nodes.values().any(|n| n["class_type"] == absent),
                "{absent} has no business in a sound: {graph}"
            );
        }
    }

    /// A checkpoint that borrows an encoder still does, whichever medium it makes.
    #[test]
    fn a_sound_checkpoint_loads_its_encoder_beside_it() {
        let mut ask = Ask::of("stable-audio-open-1.0.safetensors", "waves");
        ask.loading = Loading::Encoded {
            checkpoint: "stable-audio-open-1.0.safetensors".to_owned(),
            clip: vec!["t5_base.safetensors".to_owned()],
            clip_type: "stable_audio".to_owned(),
        };
        ask.beyond = Some(Beyond::Sound {
            family: "Stable Audio".to_owned(),
            seconds: 5.0,
            lyrics: String::new(),
        });
        let graph = compose(&ask, &core_that_sounds()).expect("every class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };
        // The encoders come off a loader; the model and the VAE off the file.
        for encode in ["2", "3"] {
            assert_eq!(
                class(nodes[encode]["inputs"]["clip"][0].as_str().expect("a wire")),
                "CLIPLoader"
            );
        }
        assert_eq!(
            class(nodes["5"]["inputs"]["model"][0].as_str().expect("a wire")),
            "CheckpointLoaderSimple"
        );
        assert_eq!(
            class(nodes["6"]["inputs"]["vae"][0].as_str().expect("a wire")),
            "CheckpointLoaderSimple"
        );
    }

    /// Each family is offered only where every node it needs exists — including its own save.
    #[test]
    fn a_family_is_offered_only_where_its_whole_chain_exists() {
        assert_eq!(
            families(&core_that_sounds()),
            vec!["Stable Audio".to_owned()]
        );
        assert_eq!(families(&core_that_moves()), vec!["LTXV".to_owned()]);
        assert!(families(&everything()).is_empty());
    }

    /// A schema with the 3D classes, as this ComfyUI publishes them.
    fn core_that_shapes() -> Schema {
        let mut info = serde_json::Map::new();
        for (class, required) in [
            ("CheckpointLoaderSimple", vec!["ckpt_name"]),
            ("ImageOnlyCheckpointLoader", vec!["ckpt_name"]),
            ("CLIPTextEncode", vec!["text", "clip"]),
            ("EmptyLatentImage", vec!["width", "height", "batch_size"]),
            ("LoadImage", vec!["image"]),
            ("CLIPVisionEncode", vec!["clip_vision", "image", "crop"]),
            ("Hunyuan3Dv2Conditioning", vec!["clip_vision_output"]),
            ("Hunyuan3Dv2ConditioningMultiView", vec![]),
            ("EmptyLatentHunyuan3Dv2", vec!["resolution", "batch_size"]),
            (
                "KSampler",
                vec![
                    "model",
                    "seed",
                    "steps",
                    "cfg",
                    "sampler_name",
                    "scheduler",
                    "positive",
                    "negative",
                    "latent_image",
                    "denoise",
                ],
            ),
            ("VAEDecode", vec!["samples", "vae"]),
            (
                "VAEDecodeHunyuan3D",
                vec!["samples", "vae", "num_chunks", "octree_resolution"],
            ),
            ("VoxelToMesh", vec!["voxel", "algorithm", "threshold"]),
            ("SaveGLB", vec!["mesh", "filename_prefix"]),
        ] {
            let inputs: serde_json::Map<String, serde_json::Value> = required
                .iter()
                .map(|name| ((*name).to_owned(), serde_json::json!([["a"], {}])))
                .collect();
            info.insert(
                class.to_owned(),
                serde_json::json!({
                    "input": { "required": inputs },
                    "input_order": { "required": required },
                }),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    fn shaping(from: From3d) -> Ask {
        let mut ask = Ask::of("hunyuan3d-dit-v2_fp16.safetensors", "a cow");
        ask.beyond = Some(Beyond::Shape {
            family: "Hunyuan3D".to_owned(),
            from,
            surface: Surface::Smooth,
        });
        ask
    }

    /// **A model is conditioned on a picture, and there is no text encoder anywhere in it.**
    ///
    /// Measured against the real server first (2026-08-29): text through picture to mesh in one
    /// graph, 42 to 57 s, answering with a `.glb` under the `3d` key.
    #[test]
    fn a_model_is_built_from_a_picture_and_never_from_words() {
        let graph = compose(
            &shaping(From3d::Pictures([
                Some("epoch-ref-cow.png".to_owned()),
                None,
                None,
                None,
            ])),
            &core_that_shapes(),
        )
        .expect("every 3D class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };
        let goes_to = |id: &str, input: &str| {
            nodes[id]["inputs"][input][0]
                .as_str()
                .expect("a wire, not a value")
                .to_owned()
        };

        // Its own loader, which answers a **vision** encoder where the other answers a CLIP.
        assert_eq!(class(&goes_to("5", "model")), "ImageOnlyCheckpointLoader");
        // The conditioning comes from a picture, through the vision encoder.
        assert_eq!(class(&goes_to("5", "positive")), "Hunyuan3Dv2Conditioning");
        assert_eq!(
            class(&goes_to(&goes_to("5", "positive"), "clip_vision_output")),
            "CLIPVisionEncode"
        );
        // **No text encoder at all**, which is why the panel does not ask for one and why the
        // prompt is not required when the picture is handed over.
        assert!(
            !nodes.values().any(|n| n["class_type"] == "CLIPTextEncode"),
            "a handed-over picture reaches no words: {graph}"
        );
        // Voxels to a mesh to a file.
        assert_eq!(class(&goes_to("11", "voxel")), "VAEDecodeHunyuan3D");
        assert_eq!(class(&goes_to("7", "mesh")), "VoxelToMesh");
        assert_eq!(class("7"), "SaveGLB");
    }

    /// Words reach it by being drawn first — **one press, one graph, two models**.
    #[test]
    fn words_reach_a_model_by_being_drawn_into_a_picture() {
        let graph = compose(
            &shaping(From3d::Drawn {
                checkpoint: "sd_xl_base_1.0.safetensors".to_owned(),
                prompts: [Some("a cow".to_owned()), None, None, None],
                negative: "blurry".to_owned(),
                width: 1024,
                height: 1024,
                steps: 25,
                cfg: 7.0,
                seed: 5,
            }),
            &core_that_shapes(),
        )
        .expect("every class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };
        let goes_to = |id: &str, input: &str| {
            nodes[id]["inputs"][input][0]
                .as_str()
                .expect("a wire, not a value")
                .to_owned()
        };

        // The vision encoder reads a decode, which reads a sampler of its own.
        let seen = goes_to(&goes_to("5", "positive"), "clip_vision_output");
        assert_eq!(class(&seen), "CLIPVisionEncode");
        assert_eq!(class(&goes_to(&seen, "image")), "VAEDecode");
        // Two models: the picture's and the mesh's.
        assert_eq!(class(&goes_to("5", "model")), "ImageOnlyCheckpointLoader");
        assert!(
            nodes
                .values()
                .any(|n| n["class_type"] == "CheckpointLoaderSimple"),
            "the picture half loads its own: {graph}"
        );

        // **The mesh sampler's numbers are the family's, not the panel's.** 20 steps at cfg 7
        // produced a blob twice in the window; the panel's numbers go to the picture half.
        assert_eq!(nodes["5"]["inputs"]["steps"], 50);
        assert_eq!(nodes["5"]["inputs"]["cfg"], 5.0);
        let drew = goes_to(&goes_to(&seen, "image"), "samples");
        assert_eq!(nodes[&drew]["inputs"]["steps"], 25);
        assert_eq!(nodes[&drew]["inputs"]["cfg"], 7.0);
    }

    /// **Four named views, and the node follows the count rather than the family.**
    ///
    /// Measured 2026-08-29: `Hunyuan3Dv2ConditioningMultiView` has no required inputs and four
    /// optional ones. And the two Hunyuan3D checkpoints are structurally identical, so which
    /// graph to write is the person's choice rather than the file's.
    #[test]
    fn four_views_go_in_by_name_and_each_gets_its_own_seed() {
        let graph = compose(
            &shaping(From3d::Drawn {
                checkpoint: "sd_xl_base_1.0.safetensors".to_owned(),
                prompts: [
                    Some("a cow from the front".to_owned()),
                    Some("a cow from the left".to_owned()),
                    Some("a cow from behind".to_owned()),
                    Some("a cow from the right".to_owned()),
                ],
                negative: String::new(),
                width: 1024,
                height: 1024,
                steps: 25,
                cfg: 7.0,
                seed: 5,
            }),
            &core_that_shapes(),
        )
        .expect("every class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };
        let goes_to = |id: &str, input: &str| {
            nodes[id]["inputs"][input][0]
                .as_str()
                .expect("a wire, not a value")
                .to_owned()
        };

        let mv = goes_to("5", "positive");
        assert_eq!(class(&mv), "Hunyuan3Dv2ConditioningMultiView");
        // Every side is named and every one reads a vision encoder of its own.
        for side in SIDES {
            let seen = nodes[&mv]["inputs"][side][0]
                .as_str()
                .unwrap_or_else(|| panic!("{side} is wired: {graph}"));
            assert_eq!(class(seen), "CLIPVisionEncode");
        }
        // Both outputs of it, not the same one twice.
        assert_eq!(nodes["5"]["inputs"]["positive"][1], 0);
        assert_eq!(nodes["5"]["inputs"]["negative"][1], 1);

        // **A seed per side.** One seed for four views draws the same picture four times, which
        // is one view wearing four labels.
        let seeds: std::collections::BTreeSet<i64> = nodes
            .values()
            .filter(|n| n["class_type"] == "KSampler")
            .filter_map(|n| n["inputs"]["seed"].as_i64())
            .collect();
        assert!(seeds.len() >= 4, "four views need four seeds: {seeds:?}");
    }

    /// **Blocky is a look, not a defect**, and each is one named choice rather than two dials.
    ///
    /// Measured on this card, same picture and same seed: smooth gave 1.65M faces and a cow with
    /// a face, ears, horns and a tail; blocky gave 407k and a stepped silhouette. The owner asked
    /// why the models looked cubic — they were, and this is the control that says so.
    ///
    /// Fine is the third, and it is here because comparing the *animal* said it was pointless
    /// and comparing the *ground* said otherwise. See [`Surface`].
    #[test]
    fn a_surface_is_smooth_or_the_voxels_on_purpose() {
        let one = |surface| {
            let mut ask = Ask::of("hunyuan3d-dit-v2_fp16.safetensors", "a cow");
            ask.beyond = Some(Beyond::Shape {
                family: "Hunyuan3D".to_owned(),
                from: From3d::Pictures([Some("front.png".to_owned()), None, None, None]),
                surface,
            });
            let graph = compose(&ask, &core_that_shapes()).expect("every class is there");
            let nodes = graph.as_object().expect("a flat map of ids").clone();
            (
                nodes["11"]["inputs"]["algorithm"].clone(),
                nodes["6"]["inputs"]["octree_resolution"].clone(),
            )
        };
        // **Three looks, and the octree is the one that costs.** Measured through the panel:
        // 62 s at 256, 366 s at 512, same animal and a visibly cleaner base — which is a trade
        // with two real sides rather than a constant.
        assert_eq!(
            one(Surface::Smooth),
            (serde_json::json!("surface net"), serde_json::json!(256))
        );
        assert_eq!(
            one(Surface::Fine),
            (serde_json::json!("surface net"), serde_json::json!(512))
        );
        assert_eq!(
            one(Surface::Blocky),
            (serde_json::json!("basic"), serde_json::json!(256))
        );
        // And smooth is what a default means, because it is what *a cow* means.
        assert_eq!(Surface::default(), Surface::Smooth);
    }

    /// One view uses the single-view node, and does not name the other one at the door.
    #[test]
    fn one_view_composes_the_single_view_graph() {
        let graph = compose(
            &shaping(From3d::Pictures([
                Some("front.png".to_owned()),
                None,
                None,
                None,
            ])),
            &core_that_shapes(),
        )
        .expect("every class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        assert!(
            !nodes
                .values()
                .any(|n| n["class_type"] == "Hunyuan3Dv2ConditioningMultiView"),
            "one picture is the single-view graph: {graph}"
        );
    }

    /// **The picture graph is untouched**, which is the claim that made one function right
    /// instead of two.
    #[test]
    fn a_picture_is_composed_exactly_as_it_was_before_video_existed() {
        let graph = compose(
            &Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse"),
            &core_that_moves(),
        )
        .expect("the picture classes are there");
        let nodes = graph.as_object().expect("a flat map of ids");
        assert_eq!(nodes["4"]["class_type"], "EmptyLatentImage");
        assert_eq!(nodes["7"]["class_type"], "SaveImage");
        assert!(nodes["4"]["inputs"].get("length").is_none());
        for absent in ["CreateVideo", "SaveVideo", "LTXVConditioning"] {
            assert!(
                !nodes.values().any(|n| n["class_type"] == absent),
                "{absent} has no business in a picture: {graph}"
            );
        }
    }

    /// A LoRA still chains, and the video nodes take their ids off the same counter.
    ///
    /// The collision this guards against is the one that already happened once: three blocks of
    /// hand-written numbers, two of which overlapped. Asserted by class, never by id.
    #[test]
    fn a_video_with_a_lora_collides_with_nothing() {
        let mut ask = moving("a red car", 25, 24.0);
        ask.loras = vec![("motion.safetensors".to_owned(), 0.8)];
        let graph = compose(&ask, &core_that_moves()).expect("every class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };
        let goes_to = |id: &str, input: &str| {
            nodes[id]["inputs"][input][0]
                .as_str()
                .expect("a wire, not a value")
                .to_owned()
        };
        assert_eq!(class(&goes_to("5", "model")), "LoraLoader");
        for encode in ["2", "3"] {
            assert_eq!(class(&goes_to(encode, "clip")), "LoraLoader");
        }
        assert_eq!(class(&goes_to("5", "latent_image")), "EmptyLTXVLatentVideo");
        assert_eq!(class(&goes_to("7", "video")), "CreateVideo");
        // Every id is claimed once, which is the property the counter exists for.
        assert_eq!(nodes.len(), 10);
    }

    /// **A family nobody measured is refused by name**, at the door.
    ///
    /// Not silently drawn as a picture, and not composed from a guess: `FAMILIES` holds what has
    /// been measured, and a surface asking for something else has asked for something that does
    /// not exist yet.
    #[test]
    fn an_unmeasured_family_is_refused_and_says_which() {
        let mut ask = moving("a red car", 49, 25.0);
        ask.beyond = Some(Beyond::Moving {
            family: "Wan".to_owned(),
            frames: 49,
            fps: 25.0,
        });
        let why = compose(&ask, &core_that_moves()).expect_err("Wan is not measured here");
        assert!(format!("{why}").contains("Wan"), "{why}");
    }

    /// And a server without the video classes says which ones it is missing.
    #[test]
    fn a_server_that_cannot_make_a_video_names_the_nodes_it_lacks() {
        let why = compose(&moving("a red car", 49, 25.0), &everything())
            .expect_err("everything() is a picture server");
        let said = format!("{why}");
        for class in ["CreateVideo", "SaveVideo", "EmptyLTXVLatentVideo"] {
            assert!(said.contains(class), "{said}");
        }
    }

    /// **They chain, and the sampler sees the last one.** A pose from one picture and a depth
    /// from another is what chaining is for, and it is the node's own shape: each apply takes a
    /// pair of conditionings and answers a pair.
    #[test]
    fn two_controlnets_chain_and_the_second_reads_what_the_first_made() {
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.negative = "blurry".to_owned();
        ask.control = vec![
            Steering {
                model: "canny.pth".to_owned(),
                image: "outline.png".to_owned(),
                prepare: None,
                strength: 0.9,
                start: 0.0,
                end: 1.0,
            },
            Steering {
                model: "depth.pth".to_owned(),
                image: "depth.png".to_owned(),
                prepare: None,
                strength: 0.4,
                start: 0.0,
                end: 1.0,
            },
        ];

        let graph = compose(&ask, &core_with_steering()).expect("the three classes are there");
        let nodes = graph.as_object().expect("a flat map of ids");

        // Three nodes each, and the ids come off the one counter.
        assert_eq!(nodes["11"]["inputs"]["control_net_name"], "canny.pth");
        assert_eq!(nodes["12"]["inputs"]["image"], "outline.png");
        assert_eq!(nodes["14"]["inputs"]["control_net_name"], "depth.pth");
        assert_eq!(nodes["15"]["inputs"]["image"], "depth.png");

        // The first reads the encodes; the second reads the first, on both sides.
        assert_eq!(nodes["13"]["inputs"]["negative"][0], "3");
        assert_eq!(nodes["16"]["inputs"]["positive"][0], "13");
        assert_eq!(nodes["16"]["inputs"]["positive"][1], 0);
        assert_eq!(nodes["16"]["inputs"]["negative"][0], "13");
        assert_eq!(nodes["16"]["inputs"]["negative"][1], 1);

        // And the sampler sees the last of them, not the first.
        assert_eq!(nodes["5"]["inputs"]["positive"][0], "16");
        assert_eq!(nodes["5"]["inputs"]["negative"][0], "16");
        // Each keeps its own strength: stacking two is not averaging them.
        assert_eq!(nodes["13"]["inputs"]["strength"], 0.9);
        assert_eq!(nodes["16"]["inputs"]["strength"], 0.4);
    }

    #[test]
    fn a_controlnet_steers_both_conditionings_and_not_only_the_positive() {
        // **Why `Advanced` and not `ControlNetApply`.** The simple node takes one conditioning,
        // and this graph has two. Steering the positive prompt and leaving the negative
        // unsteered is not what a ControlNet means — measured, the advanced node answers two
        // conditionings for exactly this.
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.negative = "blurry".to_owned();
        ask.control = vec![Steering {
            model: "control_v11p_sd15_canny.pth".to_owned(),
            image: "epoch-reference.png".to_owned(),
            prepare: None,
            strength: 0.8,
            start: 0.0,
            end: 1.0,
        }];

        let graph = compose(&ask, &core_with_steering()).expect("the three classes are there");
        let nodes = graph.as_object().expect("a flat map of ids");

        assert_eq!(
            nodes["11"]["inputs"]["control_net_name"],
            "control_v11p_sd15_canny.pth"
        );
        // The name the server answered with, never a path from this machine.
        assert_eq!(nodes["12"]["inputs"]["image"], "epoch-reference.png");
        assert_eq!(nodes["13"]["inputs"]["control_net"][0], "11");
        assert_eq!(nodes["13"]["inputs"]["image"][0], "12");
        assert_eq!(nodes["13"]["inputs"]["strength"], 0.8);

        // Both sides of the sampler come off the apply node, and off its two different outputs.
        assert_eq!(nodes["5"]["inputs"]["positive"][0], "13");
        assert_eq!(nodes["5"]["inputs"]["positive"][1], 0);
        assert_eq!(nodes["5"]["inputs"]["negative"][0], "13");
        assert_eq!(nodes["5"]["inputs"]["negative"][1], 1);
    }

    /// Everything a request can add, at once, none of it standing on anything else's id.
    ///
    /// **This is the defect, and it needed all three at the same time to appear.** Each optional
    /// piece used to write its own numbers — LoRAs from `11`, the upscale pair at `11` and `12`,
    /// the steerings from `13` — so one LoRA and an upscale both claimed `11`, and three LoRAs
    /// and a steering both claimed `13`. The later push won, the earlier node vanished, and every
    /// wire into it pointed at a node of the wrong class.
    ///
    /// Found by drawing (2026-08-27, a LoRA plus 4x-UltraSharp): no picture, and a refusal
    /// nobody could act on — `list index out of range` raised while validating `CLIPTextEncode`,
    /// because its `clip` asked slot `1` of what had become a one-output `UpscaleModelLoader`.
    ///
    /// So the assertion is not about numbers. It is that **every wire lands on a node of the
    /// class that can answer it**, which is the property the ids exist to keep.
    #[test]
    fn a_lora_an_upscale_and_two_steerings_do_not_stand_on_each_other() {
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.loras = vec![
            ("pixel.safetensors".to_owned(), 0.8),
            ("film.safetensors".to_owned(), 0.5),
            ("grain.safetensors".to_owned(), 0.3),
        ];
        ask.upscale = Some("4x-UltraSharp.pth".to_owned());
        ask.control = vec![
            Steering {
                model: "canny.pth".to_owned(),
                image: "outline.png".to_owned(),
                prepare: None,
                strength: 1.0,
                start: 0.0,
                end: 1.0,
            },
            Steering {
                model: "depth.pth".to_owned(),
                image: "depth.png".to_owned(),
                prepare: None,
                strength: 1.0,
                start: 0.0,
                end: 1.0,
            },
        ];

        let graph = compose(&ask, &everything()).expect("every class is there");
        let nodes = graph.as_object().expect("a flat map of ids");

        // 7 fixed + 3 LoRAs + 2x3 steering + 2 upscale.
        assert_eq!(nodes.len(), 18);
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };
        let goes_to = |id: &str, input: &str| {
            nodes[id]["inputs"][input][0]
                .as_str()
                .expect("a wire, not a value")
                .to_owned()
        };

        // The text encoders take a CLIP, and only a loader has one to give.
        for encode in ["2", "3"] {
            assert_eq!(class(&goes_to(encode, "clip")), "LoraLoader");
            assert_eq!(nodes[encode]["inputs"]["clip"][1], 1);
        }
        // The sampler's model comes off the last LoRA, and its conditionings off the last apply.
        assert_eq!(class(&goes_to("5", "model")), "LoraLoader");
        for side in ["positive", "negative"] {
            assert_eq!(class(&goes_to("5", side)), "ControlNetApplyAdvanced");
        }
        // And the picture is saved after it was enlarged, not before.
        assert_eq!(class(&goes_to("7", "images")), "ImageUpscaleWithModel");
        assert_eq!(
            class(&goes_to(&goes_to("7", "images"), "upscale_model")),
            "UpscaleModelLoader"
        );
    }

    /// **A canny ControlNet reads edges, and a photograph is not one.**
    ///
    /// Measured 2026-08-27, one steering, same seed, one variable at a time: the raw reference
    /// drew unusable noise; the same graph without `vae` on the apply drew the identical noise,
    /// so the vae was never it; the same reference through `Canny` drew the reference's own
    /// composition. Epoch was handing the picture straight over.
    #[test]
    fn a_preparation_sits_between_the_reference_and_the_controlnet() {
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.control = vec![Steering {
            model: "canny.pth".to_owned(),
            image: "a-photograph.png".to_owned(),
            prepare: Some("Canny".to_owned()),
            strength: 1.0,
            start: 0.0,
            end: 1.0,
        }];

        let graph = compose(&ask, &core_with_preparing()).expect("every class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        let class = |id: &str| {
            nodes[id]["class_type"]
                .as_str()
                .unwrap_or("<gone>")
                .to_owned()
        };

        // The apply node no longer reads the picture; it reads what was made of it.
        let reads = nodes["13"]["inputs"]["image"][0].as_str().expect("a wire");
        assert_eq!(class(reads), "Canny");
        assert_eq!(nodes[reads]["inputs"]["image"][0], "12");
        assert_eq!(class("12"), "LoadImage");
        // Every other value is the server's own default, never a number Epoch invented.
        assert_eq!(nodes[reads]["inputs"]["low_threshold"], 0.4);
        assert_eq!(nodes[reads]["inputs"]["high_threshold"], 0.8);
    }

    #[test]
    fn an_already_prepared_reference_goes_straight_over() {
        // Somebody who drew the edge map themselves is not made to draw it twice.
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.control = vec![Steering {
            model: "canny.pth".to_owned(),
            image: "edges.png".to_owned(),
            prepare: None,
            strength: 1.0,
            start: 0.0,
            end: 1.0,
        }];

        let graph = compose(&ask, &core_with_preparing()).expect("every class is there");
        let nodes = graph.as_object().expect("a flat map of ids");
        assert_eq!(nodes["13"]["inputs"]["image"][0], "12");
        assert_eq!(nodes["12"]["class_type"], "LoadImage");
        assert!(!nodes.values().any(|it| it["class_type"] == "Canny"));
    }

    #[test]
    fn a_preparation_this_server_does_not_have_is_refused_by_name() {
        // A workflow kept from yesterday can name a node uninstalled since.
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.control = vec![Steering {
            model: "canny.pth".to_owned(),
            image: "a-photograph.png".to_owned(),
            prepare: Some("Canny".to_owned()),
            strength: 1.0,
            start: 0.0,
            end: 1.0,
        }];
        match compose(&ask, &core_with_steering()) {
            Err(Unreadable::Missing(missing)) => {
                assert_eq!(missing, vec!["Canny".to_owned()]);
            }
            other => panic!("a missing preparation is named: {other:?}"),
        }
    }

    /// **Only what the server has, and it is not a category.** Asked of this ComfyUI 2026-08-27,
    /// `Canny` lives in `image/filters` beside `ImageBlur` and `Morphology`, and no category on
    /// the machine contains the word *preprocessor* at all.
    #[test]
    fn only_the_preparations_this_server_actually_has_are_offered() {
        assert_eq!(
            preparations(&core_with_preparing()),
            vec!["Canny".to_owned()]
        );
        assert!(preparations(&core_with_steering()).is_empty());
    }

    #[test]
    fn no_controlnet_is_the_graph_exactly_as_it_was() {
        // The ordinary case costs nothing. An inventory item nothing consumes was the defect;
        // a node nobody asked for would be a second one.
        let ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        let graph = compose(&ask, &core_with_steering()).expect("the core is there");
        let nodes = graph.as_object().expect("a flat map of ids");

        assert_eq!(nodes.len(), 7);
        assert_eq!(
            nodes["5"]["inputs"]["negative"][0], "3",
            "straight off the encode"
        );
    }

    #[test]
    fn a_server_without_the_controlnet_nodes_is_refused_by_name() {
        // A name somebody can act on beats a graph the server rejects later — and this is the
        // ordinary state of a fresh ComfyUI, whose `control_net_name` list is empty.
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.control = vec![Steering {
            model: "canny.pth".to_owned(),
            image: "ref.png".to_owned(),
            prepare: None,
            strength: 1.0,
            start: 0.0,
            end: 1.0,
        }];
        match compose(&ask, &core()) {
            Err(Unreadable::Missing(missing)) => {
                assert!(
                    missing.iter().any(|it| it == "ControlNetLoader"),
                    "{missing:?}"
                );
                assert!(
                    missing.iter().any(|it| it == "ControlNetApplyAdvanced"),
                    "{missing:?}"
                );
            }
            other => panic!("a missing class is named: {other:?}"),
        }
    }

    #[test]
    fn an_upscaler_enlarges_the_picture_that_was_made_rather_than_making_another() {
        // A second sampler pass was the other shape and is not this: that changes what is *in*
        // the picture. Somebody who asked for theirs bigger did not ask for a different one.
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.upscale = Some("4x-UltraSharp.pth".to_owned());

        let graph = compose(&ask, &core_with_upscaling()).expect("the two classes are there");
        let nodes = graph.as_object().expect("a flat map of ids");

        assert_eq!(
            nodes["11"]["inputs"]["model_name"], "4x-UltraSharp.pth",
            "the file the server listed, named as it listed it"
        );
        // Between the decode and the save: the enlarging takes the finished picture.
        assert_eq!(nodes["12"]["inputs"]["image"][0], "6");
        assert_eq!(nodes["12"]["inputs"]["upscale_model"][0], "11");
        assert_eq!(
            nodes["7"]["inputs"]["images"][0], "12",
            "and what is saved is the enlarged one"
        );
    }

    #[test]
    fn no_upscaler_is_the_graph_exactly_as_it_was() {
        // The ordinary case must cost nothing: an inventory item nothing consumes was the defect,
        // and an extra node nobody asked for would be a second one.
        let ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        let graph = compose(&ask, &core()).expect("the core is there");
        let nodes = graph.as_object().expect("a flat map of ids");

        assert_eq!(nodes.len(), 7);
        assert_eq!(nodes["7"]["inputs"]["images"][0], "6");
    }

    #[test]
    fn a_server_without_the_upscale_nodes_is_refused_by_name() {
        // The same refusal every other class gets, and the same reason: a name somebody can act
        // on beats a graph the server rejects later.
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a lighthouse");
        ask.upscale = Some("4x-UltraSharp.pth".to_owned());

        match compose(&ask, &core()) {
            Err(Unreadable::Missing(missing)) => {
                assert!(
                    missing.iter().any(|it| it == "UpscaleModelLoader"),
                    "{missing:?}"
                );
                assert!(
                    missing.iter().any(|it| it == "ImageUpscaleWithModel"),
                    "{missing:?}"
                );
            }
            other => panic!("expected a refusal naming the nodes: {other:?}"),
        }
    }

    #[test]
    fn what_epoch_writes_compiles_against_a_plain_comfyui() {
        // The claim the whole button rests on. It goes in through the ordinary import door, so
        // if the graph were malformed this is where it would refuse — by name, not later.
        let written = starter("sd_xl_base_1.0.safetensors", &core()).expect("the core is there");
        let taken = Imported::of(written, &core()).expect("and it reads");
        assert_eq!(taken.compiled.nodes.len(), 7);
    }

    #[test]
    fn it_is_an_ordinary_workflow_afterwards() {
        // Nothing downstream may be able to tell a built one from an imported one — same
        // `Opening`, derived from the graph rather than declared.
        let written = starter("v1-5-pruned-emaonly.safetensors", &core()).expect("the core");
        let taken = Imported::of(written, &core()).expect("reads");
        assert!(taken.opening.can.from_words, "it draws from words");
        assert!(!taken.opening.can.from_image, "there is no LoadImage in it");
        assert_eq!(
            taken.opening.positive.len(),
            1,
            "one prompt, wired to positive"
        );
        assert_eq!(taken.opening.negative.len(), 1, "and one wired to negative");
        assert_eq!(taken.opening.seeds.len(), 1, "the seed was found");
        assert_eq!(
            taken.opening.steps.len(),
            1,
            "so `fine` has something to scale"
        );
        assert!(taken.opening.size.is_some(), "the size was found");
        assert!(
            taken
                .opening
                .needs
                .contains(&"v1-5-pruned-emaonly.safetensors".to_owned()),
            "it names the checkpoint it was built around, in needs: {:?}",
            taken.opening.needs
        );
    }

    #[test]
    fn a_comfyui_missing_a_core_node_is_refused_by_name() {
        // It cannot happen on a normal install, and *cannot happen* is not a reason to produce a
        // graph that fails halfway through a Quest instead.
        let mut thin = serde_json::Map::new();
        thin.insert(
            "CheckpointLoaderSimple".to_owned(),
            serde_json::json!({
                "input": { "required": { "ckpt_name": [["a"], {}] } },
                "input_order": { "required": ["ckpt_name"] },
            }),
        );
        let refused = starter(
            "anything.safetensors",
            &Schema::read(&serde_json::Value::Object(thin)),
        );
        let Err(Unreadable::Missing(missing)) = refused else {
            panic!("a workflow naming nodes this server lacks must be refused");
        };
        assert!(missing.contains(&"KSampler".to_owned()), "{missing:?}");
    }

    #[test]
    fn the_size_follows_the_checkpoints_name_and_says_so_by_being_visible() {
        // A heuristic, and the only one here. SDXL at 512 and SD 1.5 at 1024 both draw badly, so
        // one number would be wrong half the time.
        assert_eq!(trained_at("sd_xl_base_1.0.safetensors"), (1024, 1024));
        assert_eq!(trained_at("flux1-dev.safetensors"), (1024, 1024));
        assert_eq!(trained_at("v1-5-pruned-emaonly.safetensors"), (512, 512));
        assert_eq!(trained_at("dreamshaper_8.safetensors"), (512, 512));
    }

    #[test]
    fn the_negative_prompt_is_left_empty_rather_than_opinionated() {
        // A default negative prompt is a house opinion about what pictures should not contain,
        // arriving in a box nobody opened. The same rule that keeps a workflow from shipping.
        let built = starter("x.safetensors", &core()).expect("the core");
        let texts: Vec<&str> = built
            .as_object()
            .expect("an object")
            .values()
            .filter(|node| node["class_type"] == "CLIPTextEncode")
            .filter_map(|node| node["inputs"]["text"].as_str())
            .collect();
        assert_eq!(texts, vec!["", ""], "both prompts start empty");
    }
    /// A picture drawn on top of another one, followed by the wires rather than by the ids.
    ///
    /// **Every picture before this began as noise**: `latent_image` came from an empty latent
    /// and `denoise` was 1.0 on every sampler. Two nodes change that and nothing else does —
    /// which is the property worth holding, because the ids around them are assigned by a
    /// counter and a counter that collides has taken a `LoraLoader` out of this graph before.
    #[test]
    fn a_picture_can_be_drawn_on_top_of_another_one() {
        let plain = compose(&Ask::of("sd_xl_base_1.0.safetensors", "a city"), &core())
            .expect("every class is there");
        let plain = plain.as_object().expect("a flat map");
        let class = |g: &serde_json::Map<String, serde_json::Value>, id: &str| {
            g[id]["class_type"].as_str().unwrap_or("<gone>").to_owned()
        };
        let goes_to = |g: &serde_json::Map<String, serde_json::Value>, id: &str, input: &str| {
            g[id]["inputs"][input][0]
                .as_str()
                .expect("a wire, not a value")
                .to_owned()
        };

        // Unchanged where nothing was handed over: from nothing, keeping nothing.
        assert_eq!(
            class(plain, &goes_to(plain, "5", "latent_image")),
            "EmptyLatentImage"
        );
        assert_eq!(plain["5"]["inputs"]["denoise"], 1.0);

        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "a city");
        ask.from = Some(("photo.png".to_owned(), 0.45));
        // **A server that has the two.** `core()` does not, and the door refuses by name for it
        // — which is the guard working, and worth saying: a ComfyUI without `VAEEncode` is told
        // so at the door rather than four minutes into a render.
        let has_both = schema_plus(&["LoadImage", "VAEEncode"]);
        assert!(
            compose(&ask, &core()).is_err(),
            "a server without them is refused by name"
        );
        let on_top = compose(&ask, &has_both).expect("every class is there");
        let on_top = on_top.as_object().expect("a flat map");

        let encoded = goes_to(on_top, "5", "latent_image");
        assert_eq!(class(on_top, &encoded), "VAEEncode");
        assert_eq!(
            class(on_top, &goes_to(on_top, &encoded, "pixels")),
            "LoadImage",
            "the picture arrives the way a reference does"
        );
        // **The same VAE it decodes with.** Encoding with one and decoding with another is how a
        // picture comes back with its colours shifted.
        assert_eq!(
            goes_to(on_top, &encoded, "vae"),
            goes_to(on_top, "6", "vae")
        );
        assert_eq!(on_top["5"]["inputs"]["denoise"], 0.45);
        assert_eq!(
            on_top[&goes_to(on_top, &encoded, "pixels")]["inputs"]["image"],
            "photo.png"
        );
    }
}

#[cfg(test)]
mod the_built_one_against_a_real_server {
    use super::*;

    /// Offer what Epoch writes to a live ComfyUI, and read what it says.
    ///
    /// The claim is that this graph is one any ComfyUI can run. A hand-written test schema
    /// cannot check that — it only proves the code agrees with itself — so this asks the real
    /// server, which is how the nine parser defects were found.
    ///
    /// **It expects to be refused, and about exactly one thing.** A checkpoint has to be named
    /// before the graph exists, so on a machine that has none the server answers
    /// `ckpt_name: '…' not in []`. If that is the *only* complaint, every other node, wire and
    /// value was accepted — which is the whole question.
    ///
    /// `#[ignore]`: needs a running server.
    #[test]
    #[ignore = "needs ComfyUI serving on 8188"]
    fn a_real_comfyui_accepts_everything_except_the_model_it_does_not_have() {
        let host = "http://127.0.0.1:8188";
        let info: serde_json::Value = ureq::get(&format!("{host}/object_info"))
            .call()
            .expect("ComfyUI answers")
            .into_json()
            .expect("JSON");
        let schema = Schema::read(&info);

        let held: Vec<String> = info["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"]
            .get(0)
            .and_then(|list| list.as_array())
            .map(|list| {
                list.iter()
                    .filter_map(|n| n.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        // Whatever it has; otherwise a name it certainly does not, so the refusal is about the
        // model and nothing else.
        let checkpoint = held
            .first()
            .cloned()
            .unwrap_or_else(|| "sd_xl_base_1.0.safetensors".to_owned());
        println!("building around {checkpoint} (server holds {})", held.len());

        let built = starter(&checkpoint, &schema).expect("the core nodes are on a real server");
        let taken = Imported::of(built, &schema).expect("and it reads back");
        assert_eq!(taken.compiled.nodes.len(), 7);

        let answered = ureq::post(&format!("{host}/prompt")).send_json(serde_json::json!({
            "prompt": taken.compiled.nodes,
            "client_id": "epoch-built-one",
        }));

        match answered {
            Ok(_) => println!("accepted outright — this machine has a checkpoint"),
            Err(ureq::Error::Status(_, response)) => {
                let said = response.into_string().unwrap_or_default();
                println!("refused: {said}");
                assert!(
                    said.contains("ckpt_name"),
                    "the only thing it may complain about is the model: {said}"
                );
                // One complaint, not several. A second would mean a wire or a value was wrong.
                for other in [
                    "KSampler",
                    "CLIPTextEncode",
                    "VAEDecode",
                    "SaveImage",
                    "EmptyLatentImage",
                ] {
                    assert!(
                        !said.contains(other),
                        "it complained about {other} as well: {said}"
                    );
                }
            }
            Err(other) => panic!("could not reach ComfyUI: {other}"),
        }
    }
}

#[cfg(test)]
mod against_the_whole_corpus {
    use super::*;

    /// How much of the real world this can read, counted rather than hoped.
    ///
    /// ComfyUI ships 509 workflows and they are what the community's look like. Running the
    /// converter over all of them turns *"it works on the one I tried"* into a number, and the
    /// failures sort themselves into reasons worth acting on.
    ///
    /// `#[ignore]`: needs a running server for the schema.
    #[test]
    #[ignore = "needs ComfyUI serving on 8188"]
    fn how_much_of_the_corpus_reads() {
        let info: serde_json::Value = ureq::get("http://127.0.0.1:8188/object_info")
            .call()
            .expect("ComfyUI answers")
            .into_json()
            .expect("JSON");
        let schema = Schema::read(&info);

        let dir = std::path::Path::new(&std::env::var("LOCALAPPDATA").unwrap())
            .join("Comfy-Desktop/ComfyUI-Installs/ComfyUI/ComfyUI/.venv/Lib/site-packages")
            .join("comfyui_workflow_templates_json/templates");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            eprintln!("no corpus at {}", dir.display());
            return;
        };

        let (mut read_ok, mut missing, mut unaligned, mut not_one, mut dynamic) = (0, 0, 0, 0, 0);
        let mut absent: BTreeMap<String, usize> = BTreeMap::new();

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            if !raw.is_object() {
                continue;
            }

            match read(&raw, &schema) {
                Ok(_) => read_ok += 1,
                Err(Unreadable::Missing(nodes)) => {
                    // **Classified by the reason, not by what the file happens to hold.** These
                    // three were counted as *needs subgraphs* because they contain one; they
                    // actually want `AILab_QwenVL` and friends, which are somebody else's nodes
                    // and are not installed. Counting them as ours would have hidden that the
                    // flattener works.
                    missing += 1;
                    for node in nodes {
                        *absent.entry(node).or_default() += 1;
                    }
                }
                Err(Unreadable::Unaligned(class)) => {
                    unaligned += 1;
                    *absent.entry(format!("unaligned: {class}")).or_default() += 1;
                }
                Err(Unreadable::Dynamic(class)) => {
                    dynamic += 1;
                    *absent.entry(format!("dynamic: {class}")).or_default() += 1;
                }
                Err(Unreadable::NotAWorkflow) => {
                    not_one += 1;
                    *absent
                        .entry(format!(
                            "not a workflow: {}",
                            path.file_name().unwrap().to_string_lossy()
                        ))
                        .or_default() += 1;
                }
            }
        }

        let total = read_ok + missing + unaligned + not_one + dynamic;
        println!("corpus: {total}");
        println!("  read:                   {read_ok}");
        println!("  needs somebody's node:  {missing}");
        println!("  unaligned:              {unaligned}");
        println!("  not a workflow:         {not_one}");
        println!("  a setting that grows:   {dynamic}");
        let mut worst: Vec<(&String, &usize)> = absent.iter().collect();
        worst.sort_by(|a, b| b.1.cmp(a.1));
        for (node, count) in worst.iter().take(8) {
            println!("    {count:>3}  {node}");
        }
        assert!(total > 400, "the corpus is there");

        // **A floor, so this stays a regression test rather than a report.** Measured
        // 2026-08-22, in four passes that each moved it, and every one was a defect in this
        // reader rather than in the workflows:
        //
        //   214 read · 160 unaligned  -> a choice list may be `"COMBO"` with options beside it
        //   278 read ·  63 unaligned  -> `widgetType` says what a union-typed input really is
        //   325 read ·   0 unaligned  -> *takes no values* is not *will not say in what order*
        //   503 read                  -> subgraphs flattened (181 of these define one)
        //
        // Then the wiring test found four more, none of which reading could have caught:
        // `COMFY_DYNAMICCOMBO_V3` is a value nothing produces; `COMFY_AUTOGROW_V3` is a list of
        // sockets and not a value at all; a choice that grows takes the values it grew; and
        // `socketless: true` beats any guess made from a type name.
        //
        // The six that remain are not Epoch's failures: three need third-party nodes nobody
        // installed — `AILab_QwenVL`, `ImageRemoveAlpha+`, `GetImageSizeAndCount`, each named
        // so it can be searched for — and three are not workflows at all (`index.schema.json`,
        // `fuse_options.json`, `index_logo.json`).
        assert!(read_ok >= 503, "read {read_ok}, was 503");
        assert_eq!(dynamic, 0, "a growing setting is read, not refused");
        assert_eq!(unaligned, 0, "nothing is refused for alignment any more");
        assert!(
            missing <= 3,
            "{missing} want somebody else's nodes: {absent:?}"
        );
    }
}

#[cfg(test)]
mod a_flattened_subgraph {
    use super::*;

    /// Reading a subgraph is not the same as having wired it correctly.
    ///
    /// The server validates a whole graph before it runs anything, and it distinguishes the two
    /// kinds of complaint this needs to tell apart:
    ///
    /// - **wiring** — `required_input_missing`, `return_type_mismatch`, `bad_linked_input`,
    ///   `prompt_no_outputs`. Any of these means the flattener produced a graph that does not
    ///   hold together, which is the failure this test exists for.
    /// - **values** — a checkpoint that is not on this machine. Expected: these templates are
    ///   for models nobody downloaded, and 3D and video weights are enormous.
    ///
    /// So the assertion is not *it ran*. It is **nothing it complained about was structural**,
    /// which is exactly what can be proven without fetching fifty gigabytes.
    #[test]
    #[ignore = "needs ComfyUI serving on 8188"]
    fn is_wired_well_enough_for_the_server_to_accept() {
        let host = "http://127.0.0.1:8188";
        let info: serde_json::Value = ureq::get(&format!("{host}/object_info"))
            .call()
            .expect("ComfyUI answers")
            .into_json()
            .expect("JSON");
        let schema = Schema::read(&info);

        let dir = std::path::Path::new(&std::env::var("LOCALAPPDATA").unwrap())
            .join("Comfy-Desktop/ComfyUI-Installs/ComfyUI/ComfyUI/.venv/Lib/site-packages")
            .join("comfyui_workflow_templates_json/templates");
        let Ok(entries) = std::fs::read_dir(&dir) else {
            eprintln!("no corpus at {}", dir.display());
            return;
        };

        const STRUCTURAL: [&str; 5] = [
            "required_input_missing",
            "return_type_mismatch",
            "bad_linked_input",
            "prompt_no_outputs",
            "invalid_prompt",
        ];

        let (mut tried, mut clean, mut only_values) = (0, 0, 0);
        for entry in entries.flatten() {
            if tried >= 12 {
                break;
            }
            let path = entry.path();
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(raw) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            // Only the ones this test is about.
            let has_subgraphs = raw
                .get("definitions")
                .and_then(|d| d.get("subgraphs"))
                .and_then(|s| s.as_array())
                .is_some_and(|s| !s.is_empty());
            if !has_subgraphs {
                continue;
            }
            let Ok(workflow) = read(&raw, &schema) else {
                continue;
            };
            tried += 1;

            let answered = ureq::post(&format!("{host}/prompt")).send_json(serde_json::json!({
                "prompt": workflow.nodes,
                "client_id": "epoch-flatten-test",
            }));
            let said = match answered {
                Ok(_) => {
                    clean += 1;
                    continue;
                }
                Err(ureq::Error::Status(_, response)) => response.into_string().unwrap_or_default(),
                Err(other) => panic!("{other}"),
            };

            for bad in STRUCTURAL {
                assert!(
                    !said.contains(bad),
                    "{} is wired wrong: {bad}\n{said}",
                    path.file_name().unwrap().to_string_lossy()
                );
            }
            only_values += 1;
        }

        println!("subgraph templates offered to the server: {tried}");
        println!("  accepted outright:        {clean}");
        println!("  refused only over values: {only_values}");
        assert!(tried >= 8, "there are plenty to try, saw {tried}");
    }
}

/// One input, read into what a surface and an aligner both need.
///
/// Its own function because the same shape appears twice: at the top of a node, and nested
/// inside a growing setting's options. Two copies would drift, and the nested one is the one
/// nobody would remember to fix.
fn one_widget(
    name: &str,
    definition: &serde_json::Value,
    produced: &std::collections::BTreeSet<&str>,
) -> Option<Widget> {
    let kind = definition.get(0).unwrap_or(definition);
    let about = definition.get(1);
    let number = |key: &str| about.and_then(|a| a.get(key)).and_then(|v| v.as_f64());

    let mut widget = Widget {
        name: name.to_owned(),
        default: about.and_then(|a| a.get("default")).cloned(),
        min: number("min"),
        max: number("max"),
        step: number("step"),
        multiline: about
            .and_then(|a| a.get("multiline"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        help: about
            .and_then(|a| a.get("tooltip"))
            .and_then(|v| v.as_str())
            .map(str::to_owned),
        then_a_control: about
            .and_then(|a| a.get("control_after_generate"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        ..Widget::default()
    };

    // **Two shapes for the same thing, and the newer one is the common one.** A choice list used
    // to be written inline — `[["euler", "heun"], {…}]` — and newer nodes declare `"COMBO"` with
    // the options beside it. Measured 2026-08-22: reading only the old shape refused **160 of
    // 509** workflows, because their only widget looked like a socket and the node then had
    // values nothing could align.
    if let Some(options) = kind.as_array() {
        widget.kind = "CHOICE".to_owned();
        widget.choices = options
            .iter()
            .filter_map(|option| option.as_str().map(str::to_owned))
            .collect();
        return Some(widget);
    }

    let word = about
        .and_then(|a| a.get("widgetType"))
        .and_then(|w| w.as_str())
        .or_else(|| kind.as_str())?;

    if word == "COMBO" {
        widget.kind = "CHOICE".to_owned();
        widget.choices = about
            .and_then(|a| a.get("options"))
            .and_then(|o| o.as_array())
            .map(|options| {
                options
                    .iter()
                    .filter_map(|option| option.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        return Some(widget);
    }
    if matches!(word, "INT" | "FLOAT" | "STRING" | "BOOLEAN") {
        // **`widgetType` when the node declares one.** `Preview3D` types its input
        // `"STRING,FILE_3D_GLB,…"` — a union that is not a primitive — and says
        // `"widgetType": "STRING"` beside it. Asked, not parsed out of that list.
        widget.kind = word.to_owned();
        return Some(widget);
    }
    if word.contains("AUTOGROW") {
        // **A growing list of sockets, not a value.** `ComfyMathExpression` takes `expression`
        // and then `values`, and the values are wires other nodes plug into — they appear in the
        // node's own `inputs`. Counting it as a widget made 29 workflows look one value short.
        return None;
    }
    // **`socketless` is the server saying it can never be a wire.** `RenderSplat.background` is
    // typed `COLOR`, and something else produces `COLOR`, so judging by type alone made it a
    // socket — it took no value, the ninth value was left over, and the server answered
    // `required_input_missing: background`. Declared beats inferred, every time.
    let socketless = about
        .and_then(|a| a.get("socketless"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if !socketless && !is_a_value(word, produced) {
        // A socket another node plugs into.
        return None;
    }

    widget.kind = "CHOICE".to_owned();
    if word.contains("DYNAMICCOMBO") {
        widget.expands = true;
        widget.grows = grown(about, produced);
        widget.choices = widget.grows.keys().cloned().collect();
    }
    Some(widget)
}

#[cfg(test)]
mod what_a_panel_asks_for {
    use super::*;

    fn schema_with(classes: &[&str]) -> Schema {
        let mut info = serde_json::Map::new();
        for class in classes {
            info.insert(
                (*class).to_owned(),
                serde_json::json!({"input": {"required": {}}, "input_order": {"required": []}}),
            );
        }
        Schema::read(&serde_json::Value::Object(info))
    }

    const PLAIN: [&str; 6] = [
        "CheckpointLoaderSimple",
        "CLIPTextEncode",
        "EmptyLatentImage",
        "KSampler",
        "VAEDecode",
        "SaveImage",
    ];

    fn nodes_of(graph: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        graph.as_object().expect("an object").clone()
    }

    #[test]
    fn an_ask_with_no_loras_needs_no_lora_node() {
        // The property that keeps a plain ComfyUI working: `LoraLoader` is required only when
        // something actually uses one.
        let graph = compose(
            &Ask::of("sd_xl_base_1.0.safetensors", "a cat"),
            &schema_with(&PLAIN),
        )
        .expect("it composed");
        let nodes = nodes_of(&graph);
        assert!(!nodes
            .values()
            .any(|node| node["class_type"] == "LoraLoader"));
        assert_eq!(nodes["2"]["inputs"]["text"], "a cat");
        // Nothing is invented for the negative prompt.
        assert_eq!(nodes["3"]["inputs"]["text"], "");
    }

    #[test]
    fn two_loras_are_chained_so_that_both_of_them_apply() {
        // Wiring both from the checkpoint compiles perfectly and silently applies only the last,
        // which is the kind of failure that is impossible to see in a picture.
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "asuka");
        ask.loras = vec![
            ("pixel-art-xl.safetensors".to_owned(), 0.8),
            ("vhs.safetensors".to_owned(), 0.5),
        ];
        let mut classes = PLAIN.to_vec();
        classes.push("LoraLoader");
        let graph = compose(&ask, &schema_with(&classes)).expect("it composed");
        let nodes = nodes_of(&graph);

        assert_eq!(nodes["11"]["class_type"], "LoraLoader");
        assert_eq!(
            nodes["11"]["inputs"]["lora_name"],
            "pixel-art-xl.safetensors"
        );
        assert_eq!(nodes["11"]["inputs"]["strength_model"], 0.8);
        // The first takes the checkpoint.
        assert_eq!(nodes["11"]["inputs"]["model"], serde_json::json!(["1", 0]));
        // The second takes the first — model *and* clip.
        assert_eq!(nodes["12"]["inputs"]["model"], serde_json::json!(["11", 0]));
        assert_eq!(nodes["12"]["inputs"]["clip"], serde_json::json!(["11", 1]));
        // And the sampler and both prompts take the last one, not the checkpoint.
        assert_eq!(nodes["5"]["inputs"]["model"], serde_json::json!(["12", 0]));
        assert_eq!(nodes["2"]["inputs"]["clip"], serde_json::json!(["12", 1]));
        assert_eq!(nodes["3"]["inputs"]["clip"], serde_json::json!(["12", 1]));
    }

    #[test]
    fn a_comfyui_without_loraloader_is_refused_by_name_before_anything_runs() {
        let mut ask = Ask::of("sd_xl_base_1.0.safetensors", "asuka");
        ask.loras = vec![("anything.safetensors".to_owned(), 1.0)];
        match compose(&ask, &schema_with(&PLAIN)) {
            Err(Unreadable::Missing(missing)) => assert_eq!(missing, vec!["LoraLoader"]),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn every_number_the_person_chose_reaches_the_graph() {
        let mut ask = Ask::of("v1-5-pruned-emaonly.safetensors", "a castle");
        ask.negative = "blurry".to_owned();
        ask.width = 832;
        ask.height = 1216;
        ask.steps = 9;
        ask.cfg = 1.0;
        ask.seed = 12345;
        ask.batch = 2;
        let graph = compose(&ask, &schema_with(&PLAIN)).expect("it composed");
        let nodes = nodes_of(&graph);
        assert_eq!(nodes["3"]["inputs"]["text"], "blurry");
        assert_eq!(nodes["4"]["inputs"]["width"], 832);
        assert_eq!(nodes["4"]["inputs"]["height"], 1216);
        assert_eq!(nodes["4"]["inputs"]["batch_size"], 2);
        assert_eq!(nodes["5"]["inputs"]["steps"], 9);
        assert_eq!(nodes["5"]["inputs"]["cfg"], 1.0);
        assert_eq!(nodes["5"]["inputs"]["seed"], 12345);
    }

    #[test]
    fn a_batch_of_none_is_still_one_picture() {
        // A zero would be accepted by the graph and produce nothing, which reads as a failure
        // with no error attached.
        let mut ask = Ask::of("x.safetensors", "a cat");
        ask.batch = 0;
        let graph = compose(&ask, &schema_with(&PLAIN)).expect("it composed");
        assert_eq!(nodes_of(&graph)["4"]["inputs"]["batch_size"], 1);
    }

    #[test]
    fn a_flux_graph_loads_three_files_and_never_a_checkpoint() {
        let mut ask = Ask::of("unused.safetensors", "asuka");
        ask.loading = Loading::Assembled {
            unet: "flux1-dev.safetensors".to_owned(),
            clip: vec![
                "t5xxl_fp16.safetensors".to_owned(),
                "clip_l.safetensors".to_owned(),
            ],
            clip_type: "flux".to_owned(),
            vae: "ae.safetensors".to_owned(),
        };
        let mut classes = PLAIN.to_vec();
        classes.retain(|class| *class != "CheckpointLoaderSimple" && *class != "EmptyLatentImage");
        classes.extend([
            "UNETLoader",
            "DualCLIPLoader",
            "VAELoader",
            "FluxGuidance",
            "EmptySD3LatentImage",
        ]);
        let graph = compose(&ask, &schema_with(&classes)).expect("it composed");
        let nodes = nodes_of(&graph);

        assert!(!nodes
            .values()
            .any(|n| n["class_type"] == "CheckpointLoaderSimple"));
        assert_eq!(nodes["1"]["inputs"]["unet_name"], "flux1-dev.safetensors");
        assert_eq!(nodes["8"]["inputs"]["type"], "flux");
        assert_eq!(nodes["9"]["inputs"]["vae_name"], "ae.safetensors");
        // Sixteen channels, not four: `EmptyLatentImage` makes the wrong shape and the sampler
        // refuses it with a message about a node rather than about the mistake.
        assert_eq!(nodes["4"]["class_type"], "EmptySD3LatentImage");
        // Flux is trained without classifier-free guidance. cfg stays at 1.0 whatever the panel
        // says, and the dial that behaves like cfg is FluxGuidance.
        assert_eq!(nodes["5"]["inputs"]["cfg"], 1.0);
        assert_eq!(nodes["5"]["inputs"]["scheduler"], "simple");
        assert_eq!(nodes["10"]["class_type"], "FluxGuidance");
        assert_eq!(nodes["10"]["inputs"]["guidance"], 3.5);
        // And the sampler takes the guidance, not the raw prompt.
        assert_eq!(
            nodes["5"]["inputs"]["positive"],
            serde_json::json!(["10", 0])
        );
        // The VAE comes from the loader that was added for it.
        assert_eq!(nodes["6"]["inputs"]["vae"], serde_json::json!(["9", 0]));
    }

    #[test]
    fn a_z_image_graph_loads_one_encoder_and_asks_for_no_guidance() {
        // Measured in ComfyUI's own source: a Qwen3-4B encoder becomes Z-Image's for every
        // clip type except Flux's, so the file decides and the type stays at its default.
        let mut ask = Ask::of("unused.safetensors", "a castle");
        ask.loading = Loading::Assembled {
            unet: "z_image_turbo_int8_convrot.safetensors".to_owned(),
            clip: vec!["qwen_3_4b_fp8_mixed.safetensors".to_owned()],
            clip_type: "stable_diffusion".to_owned(),
            vae: "z_image_ae.safetensors".to_owned(),
        };
        let mut classes = PLAIN.to_vec();
        classes.retain(|class| *class != "CheckpointLoaderSimple" && *class != "EmptyLatentImage");
        classes.extend([
            "UNETLoader",
            "CLIPLoader",
            "VAELoader",
            "EmptySD3LatentImage",
        ]);
        let nodes = nodes_of(&compose(&ask, &schema_with(&classes)).expect("it composed"));

        assert_eq!(nodes["8"]["class_type"], "CLIPLoader");
        assert_eq!(nodes["8"]["inputs"]["type"], "stable_diffusion");
        // One encoder, not two: a `DualCLIPLoader` here would ask for a second file that does
        // not exist.
        assert!(!nodes.values().any(|n| n["class_type"] == "DualCLIPLoader"));
        // And no guidance node — Turbo is distilled, and cfg is simply 1.0.
        assert!(!nodes.values().any(|n| n["class_type"] == "FluxGuidance"));
        assert_eq!(nodes["5"]["inputs"]["cfg"], 1.0);
        assert_eq!(
            nodes["5"]["inputs"]["positive"],
            serde_json::json!(["2", 0])
        );
        // Sixteen channels, like Flux.
        assert_eq!(nodes["4"]["class_type"], "EmptySD3LatentImage");
    }

    #[test]
    fn a_lora_chains_onto_flux_exactly_as_it_does_onto_a_checkpoint() {
        // The reason the LoRA chain lives outside the branch: it is the same wiring, and a second
        // copy inside a Flux arm is a second place for it to be wrong.
        let mut ask = Ask::of("unused.safetensors", "asuka");
        ask.loading = Loading::Assembled {
            unet: "flux1-dev.safetensors".to_owned(),
            clip: vec![
                "t5xxl.safetensors".to_owned(),
                "clip_l.safetensors".to_owned(),
            ],
            clip_type: "flux".to_owned(),
            vae: "ae.safetensors".to_owned(),
        };
        ask.loras = vec![("japanese_vhs.safetensors".to_owned(), 0.9)];
        let mut classes = PLAIN.to_vec();
        classes.extend([
            "UNETLoader",
            "DualCLIPLoader",
            "VAELoader",
            "FluxGuidance",
            "EmptySD3LatentImage",
            "LoraLoader",
        ]);
        let nodes = nodes_of(&compose(&ask, &schema_with(&classes)).expect("it composed"));
        assert_eq!(nodes["11"]["inputs"]["model"], serde_json::json!(["1", 0]));
        assert_eq!(nodes["11"]["inputs"]["clip"], serde_json::json!(["8", 0]));
        assert_eq!(nodes["5"]["inputs"]["model"], serde_json::json!(["11", 0]));
        assert_eq!(nodes["2"]["inputs"]["clip"], serde_json::json!(["11", 1]));
    }

    #[test]
    fn a_comfyui_too_old_for_flux_is_refused_by_name() {
        let mut ask = Ask::of("unused.safetensors", "asuka");
        ask.loading = Loading::Assembled {
            unet: "flux1-dev.safetensors".to_owned(),
            clip: vec!["t5.safetensors".to_owned(), "clip_l.safetensors".to_owned()],
            clip_type: "flux".to_owned(),
            vae: "ae.safetensors".to_owned(),
        };
        match compose(&ask, &schema_with(&PLAIN)) {
            Err(Unreadable::Missing(missing)) => {
                assert!(missing.contains(&"UNETLoader".to_owned()), "{missing:?}");
                assert!(missing.contains(&"FluxGuidance".to_owned()), "{missing:?}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// Offered to a real ComfyUI, which is the only thing that proves a graph.
    ///
    /// Ignored because it needs one serving. It asks the server to *validate* rather than to
    /// draw: `/prompt` answers with what it refuses, and a refusal naming a node or an input is
    /// exactly the failure this whole function exists to move to the door.
    #[test]
    #[ignore = "needs ComfyUI serving on 8188"]
    fn what_the_panel_composes_is_accepted_by_a_real_comfyui() {
        let info: serde_json::Value = ureq::get("http://127.0.0.1:8188/object_info")
            .timeout(std::time::Duration::from_secs(30))
            .call()
            .expect("ComfyUI answered")
            .into_json()
            .expect("readable");
        let schema = Schema::read(&info);

        let checkpoint = info["CheckpointLoaderSimple"]["input"]["required"]["ckpt_name"][0][0]
            .as_str()
            .expect("this ComfyUI reports a checkpoint")
            .to_owned();
        println!("composing against {checkpoint}");

        let graph = compose(&Ask::of(&checkpoint, "a red apple"), &schema).expect("it composed");
        let answer = ureq::post("http://127.0.0.1:8188/prompt")
            .timeout(std::time::Duration::from_secs(30))
            .send_json(serde_json::json!({"prompt": graph}));

        match answer {
            Ok(said) => {
                let said: serde_json::Value = said.into_json().expect("readable");
                println!("queued: {said}");
                assert!(said.get("prompt_id").is_some(), "{said}");
            }
            Err(ureq::Error::Status(_, said)) => {
                panic!(
                    "ComfyUI refused it: {}",
                    said.into_string().unwrap_or_default()
                )
            }
            Err(why) => panic!("{why}"),
        }
    }
}
