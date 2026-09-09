//! Making a picture: the Studio Panel, the easel and the graph that is composed for it.
//!
//! Split out of `state.rs` unchanged. Everything here is the picture chain (ADR-0030, ADR-0033):
//! what the panel offers, what a family arrives in, which bench draws, and the one place a
//! composed graph is handed to a server.

use super::*;

/// The Studio Panel (ADR-0033): what this machine can draw with, for one chosen model.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelView {
    /// Checkpoints the server that will draw reported. Its answer, never a directory listing.
    pub models: Vec<PanelModel>,
    /// Every LoRA in the library, compatible or not — greyed with a reason, never hidden.
    pub loras: Vec<PanelLora>,
    /// Shapes offered for the chosen model, largest side first.
    pub shapes: Vec<PanelShape>,
    /// Text encoders this ComfyUI can load, for a model that arrives in parts — each named by
    /// what it *is*, when Epoch holds the file and could read it.
    pub encoders: Vec<PanelPart>,
    /// VAEs it can load, the same way.
    pub vaes: Vec<PanelPart>,
    /// The encoder families **ComfyUI itself publishes** — measured, never a list typed here.
    ///
    /// This is the whole of what separates Flux from Z-Image from Qwen-Image from SD 3: one
    /// string. One added to ComfyUI tomorrow appears here with no code at all.
    ///
    /// **One list per loader, because they are different lists.** Asked of a live server:
    /// `CLIPLoader` offers 28 (including `stable_diffusion`), `DualCLIPLoader` offers 12
    /// (including `flux` and `sdxl`, and *not* `stable_diffusion`), and `TripleCLIPLoader` has no
    /// such input at all. Offering one list for all three is how a picture came to be refused
    /// with *"'stable_diffusion' not in \['sdxl','sd3','flux',…\]"* — the reading was right and
    /// it was attributed to the wrong node.
    pub clip_types: Vec<String>,
    /// The same, for two encoders. Empty when the server did not answer.
    pub clip_types_two: Vec<String>,
    /// What a model of the chosen family is normally assembled from — in words, never in files.
    ///
    /// `None` when Epoch has nothing true to say: an unmeasured model, or a family it has no
    /// documented assembly for. Silence, rather than a guess dressed as help.
    pub assembly: Option<PanelAssembly>,
    /// Upscale models this server offers, as it lists them.
    ///
    /// **Inventory with a consumer at last.** `Shelf::Upscalers` and `Kind::Upscaler` have
    /// existed since ADR-0032: one is identified from its bytes, filed on the right shelf,
    /// installed and listed — and until now no composed graph had ever contained one.
    ///
    /// Empty is honest and common: nothing is installed, and the row says where one would land.
    #[serde(default)]
    pub upscalers: Vec<PanelPart>,
    /// The ControlNets this server lists, if it has the node at all.
    ///
    /// **The second shelf that fed nothing.** `Shelf::ControlNet` and `Kind::ControlNet` have
    /// existed since ADR-0032 — identified from bytes, filed, installed, listed — and no
    /// composed graph had ever contained one.
    ///
    /// Empty is the ordinary state of a fresh ComfyUI (measured: `control_net_name` is an empty
    /// combo), and it is said rather than hidden: a control with an empty list tells somebody
    /// the shelf is where one would land, and a missing control tells them Epoch cannot do it.
    #[serde(default)]
    pub controlnets: Vec<PanelPart>,
    /// The embeddings this server has, by the names it answers with — no extensions.
    ///
    /// **The last shelf that fed nothing.** `Shelf::Embeddings` has existed since ADR-0032 and no
    /// composed graph ever contained one — and it never will, because an embedding is not loaded
    /// by a node. Measured: nothing in `/object_info` takes one, and the only way to use one is
    /// to write `embedding:<name>` into the prompt.
    ///
    /// So this is not a control that draws; it is the panel saying what the person may name.
    /// Which matters more than it sounds: measured on this server, same seed, a name that is
    /// **not** installed is not refused — `embedding`, `:` and the misspelling are encoded as
    /// ordinary words and quietly change the picture. A list is the difference between choosing
    /// and guessing.
    #[serde(default)]
    pub embeddings: Vec<String>,
    /// What this server can turn a picture into, by node class. Empty is a real answer.
    ///
    /// **A named set, filtered by what the server has** — never a category. `Canny` lives in
    /// `image/filters` beside `ImageBlur` and `Morphology` (asked 2026-08-27), and a list derived
    /// from *takes an IMAGE, answers an IMAGE* is sixty classes wide and mostly hosted-API photo
    /// editors.
    #[serde(default)]
    pub preparations: Vec<String>,
    /// The video families this drawing machine can actually run, in the order they are offered.
    ///
    /// **Empty is the ordinary answer and it is a real one**: a ComfyUI without the video nodes
    /// cannot make a video, and a VIDEO tab that offered a family this server has never heard of
    /// would refuse after the form was filled in — the defect ADR-0033's encoder list already
    /// had once.
    ///
    /// One name, because one has been measured (`workflow::FAMILIES`). It grows the day another
    /// is drawn with here, not the day somebody remembers what Wan's nodes are called.
    #[serde(default)]
    pub motions: Vec<String>,
    /// The families this drawing machine can make **sound** with, in the order they are offered.
    ///
    /// Its own list rather than a medium tag on `motions`, because the two are read by two
    /// different tabs and a surface that filtered one list would be the second place deciding
    /// which family belongs where.
    #[serde(default)]
    pub sounds: Vec<String>,
    /// The families this drawing machine can make a **model** with.
    ///
    /// Its own list for the same reason `sounds` is: one tab reads one list, and a surface that
    /// filtered a shared one would be the second place deciding which family belongs where.
    ///
    /// `meshes` rather than `shapes` because `shapes` on this view is already the size presets —
    /// a word that had a job before this medium existed.
    #[serde(default)]
    pub meshes: Vec<String>,
    /// Which sound families are told words that are **sung**, beside words that describe.
    ///
    /// A subset of `sounds`, and its own field for the same reason `sounds` is its own list: the
    /// panel asks one question of it, and a flag computed on the surface would be a second place
    /// deciding what ACE-Step takes.
    pub lyrical: Vec<String>,
    /// Whether the drawing machine is answering **right now**.
    ///
    /// Opening the panel no longer starts it, so this is usually `false` on a fresh machine and
    /// that is not a problem to report — it is a fact that changes what the panel can know. The
    /// file lists are read off the shelves either way; a node's own vocabulary is not, and the
    /// panel says which is which instead of showing an empty dropdown.
    #[serde(default)]
    pub serving: bool,
    /// What this exact model has already been drawn with here, newest first.
    ///
    /// Empty when nobody has drawn with it, and **empty when nobody has measured its hash** —
    /// which is a different sentence and the honest one: a file Epoch has never read cannot be
    /// recognised, and hashing twelve gigabytes to open a panel is the wrong trade. It fills in
    /// the first time a picture is made with it.
    #[serde(default)]
    pub remembered: Vec<PanelRecipe>,
    /// Where the picture would be made, in words.
    pub where_at: String,
    /// Nothing can draw right now, and why. `None` means it can.
    pub problem: Option<String>,
}

/// One part a model that arrives in parts can be given: an encoder, or a VAE.
///
/// ## Why the row carries a description at all
///
/// Measured 2026-08-25: a part's *family* is unmeasurable, because it does not have one.
/// `clip_l.safetensors` is byte-for-byte the same file beside Flux and beside SDXL. So greying
/// the parts belonging to another family — the obvious help, and what a LoRA row already gets —
/// would have greyed nothing at all.
///
/// What *is* in the bytes is what the part is: 768 wide is a CLIP-L, `shared.weight` at 4096 is a
/// T5-XXL. The panel says the family wants "a CLIP-L and a T5-XXL" and the rows say which file is
/// which, so the two halves meet without Epoch ever claiming a file belongs to a family it cannot
/// see.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelPart {
    /// Exactly as the server listed it. What the graph will name.
    pub file: String,
    /// What it is, in the words a person matches against the guidance — `CLIP-L`, `T5-XXL`.
    ///
    /// `None` for a file Epoch does not hold, or one whose shape it does not recognise. Silence,
    /// which is the absence of a measurement rather than a claim that it will not work.
    pub says: Option<String>,
    /// **Which medium this part belongs to**, for the parts where that is readable — today, a
    /// VAE, from the rank of its convolutions (`asset::medium_of`).
    ///
    /// `None` is *unplaced*, and the surface must offer such a file under every medium rather
    /// than hiding it from the one it belongs to. Hiding somebody's file because Epoch failed to
    /// measure it is the worst outcome a filter has available: nothing they can do about it, and
    /// nothing telling them why.
    pub medium: Option<String>,
}

/// How a family arrives, for a model that arrives in parts.
///
/// **This is a statement about a family, not about a file.** `shapes_for` already makes the same
/// kind of statement — SD 1.5 was trained at 512 and SDXL at 1024 — and it is honest for the same
/// reason: it comes from what the family *is*, not from what a filename claims. Epoch measured
/// the model's family from its tensors; everything here follows from that one measurement.
///
/// It is guidance and never a choice. The panel still makes the user pick every file
/// (ADR-0033) — this only stops them picking between twelve names with nothing to go on.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelAssembly {
    /// The family, in the words a person uses. `Flux`, `SD 3`.
    pub family: String,
    /// How many text encoders this family is loaded with.
    ///
    /// `None` when the family could not be read from the model's tensors — which is the one thing
    /// that decides the count, so guessing it would be guessing the whole answer.
    pub encoders: Option<usize>,
    /// What those encoders are, named as what they are rather than as files on this machine.
    pub says: String,
    /// The `type` this family is in ComfyUI's own vocabulary — and **only when the loader that
    /// this many encoders selects genuinely offers it**. A family the server has never heard of
    /// is left unsaid rather than recommended into a refusal.
    pub clip_type: Option<String>,
    /// Which encoders this family wants, named the way `understand` names a file.
    ///
    /// **So the two halves can meet in the dropdown itself.** The guidance already said *a CLIP-L
    /// and a T5-XXL* and every row already said which file was which, and the owner still had to
    /// read one sentence, hold it, and match it against a list by eye — twice, once per field.
    /// These are the same strings `Encoder::plainly` produces, so a row can be marked by
    /// comparison rather than by anybody typing a filename.
    ///
    /// Empty where the family is unknown: what an unmeasured family wants is exactly what is not
    /// known, and a list invented here would be marked confidently and wrong.
    #[serde(default)]
    pub wants: Vec<String>,
}

/// One combination this machine has already run against this model.
///
/// **Evidence, never a claim about the architecture.** It says a graph ran, not that it is the
/// best one — and a second entry is a second thing that worked rather than a contradiction
/// (ADR-0032's amendment). Which is exactly the guidance the family reader can never give for a
/// model it has not heard of.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelRecipe {
    /// The encoders, as the server names them — because that is what a graph must name, and a
    /// hash cannot be put into a `CLIPLoader`.
    pub clip: Vec<String>,
    pub clip_type: String,
    pub vae: String,
    /// A picture came out.
    pub drew: bool,
    /// The server's own words, when it refused. Only kept for a refusal about the configuration:
    /// an out-of-memory is the card that day, and filing it would be a gauge that lies.
    pub said: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelModel {
    /// Exactly as the server listed it. What the graph will name.
    pub file: String,
    pub family: String,
    /// **What the site called its base, verbatim** — `Pony`, `Illustrious`, `Flux.1 D`.
    ///
    /// A finer grain than `family`, which has five names and calls Pony and Illustrious the same
    /// thing. Measured on the owner's files: `DiivesP1` says Pony, `DiivesIXL` says Illustrious,
    /// and Epoch reads both as SDXL — so the panel showed one word for two things that draw
    /// differently. Said and never enforced: it is the author's claim, not a measurement, and
    /// his best picture came from the pairing these words call mismatched.
    pub said_base: Option<String>,

    /// Read from the file when Epoch can find it; `null` for one it never saw.
    pub bytes: Option<u64>,
    /// `checkpoint` or `diffusion`. Which recipe the compiler will write — a checkpoint carries
    /// its own text encoder and VAE, and a diffusion model needs both handed to it.
    pub kind: String,
    /// What a diffusion model still needs before it can draw, named. Empty means it can.
    pub needs: Vec<String>,
    /// The text encoders and VAE Epoch would load beside it, so the choice is visible rather
    /// than silent.
    pub with: Vec<String>,
    /// Which medium this makes: `picture`, `video`, `sound`. **`null` is a real answer.**
    ///
    /// A model whose family Epoch measured belongs to one medium and appears under that tab. One
    /// it could not read belongs to none of them, so it appears under **every** tab — hiding a
    /// file the user owns because Epoch failed to read it is the worst outcome available: there
    /// is nothing they can do about it and nothing telling them why.
    ///
    /// A string rather than a boolean because there are three media now, and a boolean carrying
    /// three states is the field somebody reads the wrong way round.
    ///
    /// The measurement is the family's, from the tensors (`Base::makes`), never from a filename.
    #[serde(default)]
    pub makes: Option<String>,
    /// Whether this checkpoint carries its own text encoder, so the panel knows whether to ask
    /// for one.
    ///
    /// **Measured, not inferred from the medium.** The panel asked for an encoder beside every
    /// non-picture checkpoint, on a rule generalised from the two it had seen — and ACE-Step is
    /// all-in-one, so GENERATE sat grey waiting for a file that model has no use for.
    #[serde(default)]
    pub carries_encoder: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelLora {
    pub file: String,
    pub family: String,
    /// **What the site called its base, verbatim** — `Pony`, `Illustrious`, `Flux.1 D`.
    ///
    /// A finer grain than `family`, which has five names and calls Pony and Illustrious the same
    /// thing. Measured on the owner's files: `DiivesP1` says Pony, `DiivesIXL` says Illustrious,
    /// and Epoch reads both as SDXL — so the panel showed one word for two things that draw
    /// differently. Said and never enforced: it is the author's claim, not a measurement, and
    /// his best picture came from the pairing these words call mismatched.
    pub said_base: Option<String>,

    pub bytes: u64,
    /// Words its author says a prompt needs, when the file or its manifest carries them.
    pub triggers: Vec<String>,
    /// It can be used with the chosen model.
    pub fits: bool,
    /// Why not, in the words a person uses — never `mat1 and mat2 shapes cannot be multiplied`.
    pub why: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelShape {
    pub label: String,
    pub width: i64,
    pub height: i64,
    /// This is a size the chosen family was actually trained at.
    ///
    /// **Marked, never enforced.** Asking SD 1.5 for 1024 produces two heads and asking Flux for
    /// 4K produces an out-of-memory — both true, and neither is Epoch's decision to make for
    /// somebody who owns the card. The screen sizes below are offered because *"I want a
    /// wallpaper"* is a real thing to want; the mark is what stops it being a trap.
    pub native: bool,
}

/// What the panel asked for, arriving from the window.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelAsk {
    pub checkpoint: String,
    /// `checkpoint` or `diffusion`, as the panel row said.
    #[serde(default)]
    pub kind: String,
    /// Flux's own dial. Ignored by everything else.
    #[serde(default)]
    pub guidance: f64,
    /// For a model that arrives in parts: its encoders, their family, and its VAE.
    #[serde(default)]
    pub clip: Vec<String>,
    #[serde(default)]
    pub clip_type: String,
    #[serde(default)]
    pub vae: String,
    /// Each LoRA and its strength, in order.
    pub loras: Vec<(String, f64)>,
    /// An upscale model to enlarge the finished picture with, as the server lists it. Empty is
    /// none, and none is the ordinary case.
    #[serde(default)]
    pub upscale: String,
    /// Each ControlNet, its reference picture, and how hard it steers — in order.
    ///
    /// **The user's choice, and only the user's.** A ControlNet is a file and a reference
    /// picture is a file — ADR-0033 settled that files are chosen in the panel by the person
    /// who owns the machine. `draw_image` gains nothing: a character still asks for a mood.
    ///
    /// A list because they chain, which is the node's own shape rather than an idea: a pose
    /// from one picture and a depth from another. Empty is the ordinary case.
    #[serde(default)]
    pub controls: Vec<PanelSteer>,
    /// **A picture this one is drawn on top of**, by the name the server answered with when it
    /// was handed over. Empty means from nothing, which is every other picture.
    pub from: String,
    /// How much of that picture survives, as a fraction: `0` keeps all of it and `1` keeps none.
    ///
    /// Said this way round on the surface because that is the question somebody has — *how much
    /// of my photo is left* — and turned into ComfyUI's `denoise`, which counts the other way,
    /// exactly once, here.
    pub keep: f64,
    pub prompt: String,
    #[serde(default)]
    pub negative: String,
    pub width: i64,
    pub height: i64,
    pub steps: i64,
    pub cfg: f64,
    /// `0` means a different picture every time.
    #[serde(default)]
    pub seed: i64,
    #[serde(default)]
    pub batch: i64,
    /// Which family, by the name `workflow::families` offered. Empty is a picture, which is
    /// the ordinary case and changes the graph not at all.
    ///
    /// **The name and not a flag.** A boolean would say *this is not a picture* and leave the
    /// graph to guess which of several ways — and the guess would be Epoch inventing a family
    /// for a file it did not measure (ADR-0033: the panel is where the person says).
    #[serde(default)]
    pub motion: String,
    /// How many frames, and how fast they play. Ignored by a family that makes sound.
    #[serde(default)]
    pub frames: i64,
    #[serde(default)]
    pub fps: f64,
    /// How long, in seconds. Read only by a family that makes sound, which has no frames.
    #[serde(default)]
    pub seconds: f64,
    /// What is **sung**, which is not what describes the song.
    ///
    /// Read only by a family whose encoder takes it. Empty is an instrumental, and it is what
    /// every family that is not a music model gets.
    #[serde(default)]
    pub lyrics: String,
    /// For a shape: the pictures it is built from, one per side, in `workflow::SIDES` order —
    /// front, left, back, right. An empty string is a side nobody supplied.
    ///
    /// **Named views, not a bag.** Measured: `Hunyuan3Dv2ConditioningMultiView` takes four
    /// optional inputs by name, so a list would have to guess which is which.
    ///
    /// All empty means *draw them here*, from the prompts and the checkpoint below — both of
    /// which are the person's choice, exactly as everything else on this panel is.
    #[serde(default)]
    pub shape_from: Vec<String>,
    /// One prompt per side, when they are drawn here. Same order, same rule about empties.
    ///
    /// **A prompt per view rather than one with the side appended.** Epoch does not write words
    /// nobody typed — it refused to invent lyrics for a song and it refuses to invent *"…, seen
    /// from the left"* here.
    #[serde(default)]
    pub shape_prompts: Vec<String>,
    /// How the surface is pulled out of the voxels: `smooth`, `fine` or `blocky`.
    ///
    /// The two things that decide it — the conversion algorithm and the octree the shape is
    /// sampled on — are one named look here, because a person means *smooth* or *blocky* and not
    /// *surface net at 256*. Anything unrecognised is smooth, which is what most asks mean.
    #[serde(default)]
    pub surface: String,
    /// Which checkpoint draws that picture, when one is drawn. Empty is a refusal rather than a
    /// guess: Epoch does not pick a model.
    #[serde(default)]
    pub shape_with: String,
}

impl PanelAsk {
    /// What this panel is asking to make, when it is asking for something that is not a still.
    ///
    /// **Rounded here, because here is where the family is known.** LTXV samples in blocks of
    /// eight and takes `8n+1` frames; asking it for 50 gets a refusal about a number the person
    /// typed into a field that never said so. The clamp is the same courtesy the sizes get.
    ///
    /// Which of the two shapes is decided by the family's own `keeping`, not by which fields the
    /// surface happened to fill in: a panel that sent `seconds` for a video would otherwise get
    /// a graph built from a number nobody meant.
    fn beyond(&self) -> Option<epoch_engine::assets::workflow::Beyond> {
        use epoch_engine::assets::workflow::{Beyond, Keeping};
        let named = self.motion.trim();
        if named.is_empty() {
            return None;
        }
        let family = epoch_engine::assets::workflow::family(named)?;
        Some(match family.keeping {
            // **A shape is conditioned on a picture, and the panel says which.** A name means a
            // file the person handed over; nothing means one drawn here in the same press, by a
            // checkpoint they also chose. Neither is Epoch picking (ADR-0033).
            Keeping::Mesh => Beyond::Shape {
                family: named.to_owned(),
                // **Smooth unless somebody asked otherwise.** Blocky is not a defect to be
                // defaulted away from — somebody asking for a voxel model means it — and fine
                // costs six minutes against one, which is the person's call and not Epoch's.
                surface: match self.surface.trim() {
                    "fine" => epoch_engine::assets::workflow::Surface::Fine,
                    "blocky" => epoch_engine::assets::workflow::Surface::Blocky,
                    _ => epoch_engine::assets::workflow::Surface::Smooth,
                },
                from: {
                    let sides = |from: &[String]| -> epoch_engine::assets::workflow::Views<String> {
                        let mut out: epoch_engine::assets::workflow::Views<String> =
                            [None, None, None, None];
                        for (at, said) in from.iter().take(4).enumerate() {
                            let said = said.trim();
                            if !said.is_empty() {
                                out[at] = Some(said.to_owned());
                            }
                        }
                        out
                    };
                    let handed = sides(&self.shape_from);
                    if handed.iter().any(Option::is_some) {
                        epoch_engine::assets::workflow::From3d::Pictures(handed)
                    } else {
                        // The front prompt falls back to the panel's own, so the ordinary case —
                        // one view, one sentence — needs no second field filled in.
                        let mut prompts = sides(&self.shape_prompts);
                        if prompts[0].is_none() && !self.prompt.trim().is_empty() {
                            prompts[0] = Some(self.prompt.clone());
                        }
                        epoch_engine::assets::workflow::From3d::Drawn {
                            checkpoint: self.shape_with.trim().to_owned(),
                            prompts,
                            negative: self.negative.clone(),
                            // Square and centred: what a picture is *for* here is a subject a
                            // vision encoder will read, and it centre-crops. A wide one loses
                            // its edges.
                            width: 1024,
                            height: 1024,
                            steps: self.steps.clamp(1, 200),
                            cfg: self.cfg.clamp(0.0, 30.0),
                            seed: if self.seed == 0 {
                                seed_now() as i64
                            } else {
                                self.seed
                            },
                        }
                    }
                },
            },
            Keeping::Audio => Beyond::Sound {
                family: named.to_owned(),
                // **The family's own numbers, and both were asked of the server.** Its latent
                // node declares what it is usually given and the most it will take; a ceiling
                // typed in here would be a number nobody measured, and it would be wrong the
                // day a family with a different one arrives.
                //
                // Nothing is refused short of that ceiling: asking Stable Audio for 200 seconds
                // pads rather than fails, which is the user's call — the panel marks which
                // number is the trained one, the way the sizes are marked.
                seconds: if self.seconds <= 0.0 {
                    family.usual
                } else {
                    self.seconds.clamp(1.0, family.longest)
                },
                // Straight through, trimmed and never invented. A music model is told a genre
                // and a vocal separately, and splitting one box into two would be Epoch writing
                // words nobody typed.
                lyrics: self.lyrics.trim().to_owned(),
            },
            Keeping::Video => Beyond::Moving {
                family: named.to_owned(),
                // The nearest `8n+1` at or below what was asked for, never above: a frame more
                // than the person wanted is a frame more of video memory, and this runs on the
                // card that also holds the model.
                frames: ((self.frames.clamp(9, 481) - 1) / 8) * 8 + 1,
                fps: if self.fps <= 0.0 {
                    25.0
                } else {
                    self.fps.clamp(1.0, 60.0)
                },
            },
        })
    }
}

/// What pressing GENERATE produced: a finished picture, or work that has begun.
///
/// **Two shapes because there are two, and a nullable field would have been one shape lying.**
/// A picture is finished when the call returns; a video is not (ADR-0034). The surface renders
/// them differently — one shows the file, the other says a character is working on it — and a
/// `PanelDrawn` with an empty filename would make that the frontend's guess.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PanelMade {
    Drew(PanelDrawn),
    /// It started and will land later. The words are the Job's own, so what the panel says and
    /// what the crew card says come from one place.
    #[serde(rename_all = "camelCase")]
    Began {
        what: String,
        waiting_on: String,
    },
}

/// One picture the panel made.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelDrawn {
    /// The vault filename. The window reads its bytes through `shared_image`, as it does for
    /// every other picture — one path for pictures, not two.
    pub file: String,
    /// Where it was also written, when the World has a Project Root — the folder the crew works
    /// in (ADR-0025). `null` when the World has none, which is a World that still draws.
    ///
    /// Shown to the person who pressed the button. A capability's result deliberately carries no
    /// path at all: what a tool says back is prompt, and a filename in it is the one thing a
    /// model needs to claim a picture nobody made (ADR-0030's third amendment).
    pub at: Option<String>,
    pub seconds: f32,
}

/// Which kind a surface named, when it named one.
pub(crate) fn kind_named(said: &str) -> Option<epoch_engine::assets::asset::Kind> {
    use epoch_engine::assets::asset::Kind;
    match said {
        "checkpoint" => Some(Kind::Checkpoint),
        "lora" => Some(Kind::Lora),
        "vae" => Some(Kind::Vae),
        "controlnet" => Some(Kind::ControlNet),
        "embedding" => Some(Kind::Embedding),
        "upscaler" => Some(Kind::Upscaler),
        "voice" => Some(Kind::Voice),
        _ => None,
    }
}

/// The base models worth offering as a filter, in the sites' own words.
///
/// Read from `epoch-models` rather than typed here: the list was measured, and a second copy on
/// a screen would be the second answer that drifts.
pub fn bases_offered() -> Vec<String> {
    epoch_engine::models::civitai::BASES
        .iter()
        .map(|it| (*it).to_owned())
        .collect()
}

/// Which medium a family makes, as the window names it. `None` stays `None`.
pub(crate) fn makes_id(base: epoch_engine::assets::asset::Base) -> Option<String> {
    base.makes().map(|it| medium_id(it).to_owned())
}

/// A medium, as the window names it — the same word whether it was read from a family or from a
/// part's own tensors.
///
/// **One spelling, because two would agree by luck.** A model's medium comes from `Base::makes`
/// and a VAE's from its convolutions, and the panel compares them: written out twice, the day
/// one of them gained a word the other would silently stop matching. That defect has been paid
/// for once already, on `stableaudio` against `Stable Audio`.
pub(crate) const fn medium_id(makes: epoch_engine::assets::asset::Makes) -> &'static str {
    use epoch_engine::assets::asset::Makes;
    match makes {
        Makes::Picture => "picture",
        Makes::Video => "video",
        Makes::Sound => "sound",
        Makes::Model => "model",
    }
}

/// What a part decodes into, in the words a person reads on the row.
pub(crate) const fn medium_words(makes: epoch_engine::assets::asset::Makes) -> &'static str {
    use epoch_engine::assets::asset::Makes;
    match makes {
        Makes::Picture => "decodes a picture",
        Makes::Video => "decodes a video",
        Makes::Sound => "decodes a sound",
        Makes::Model => "decodes a model",
    }
}

/// A family, as the window names it. Machine-readable, so nothing compares display text.
pub(crate) const fn family_id(base: epoch_engine::assets::asset::Base) -> &'static str {
    use epoch_engine::assets::asset::Base;
    match base {
        Base::Sd15 => "sd15",
        Base::Sd2 => "sd2",
        Base::Sdxl => "sdxl",
        Base::Sd3 => "sd3",
        Base::Flux => "flux",
        Base::ZImage => "zimage",
        Base::Ltxv => "ltxv",
        Base::StableAudio => "stableaudio",
        Base::AceStep => "acestep",
        Base::AceStep15 => "acestep15",
        Base::Hunyuan3d => "hunyuan3d",
        Base::Unknown => "unknown",
    }
}

pub(crate) fn base_named(id: &str) -> epoch_engine::assets::asset::Base {
    use epoch_engine::assets::asset::Base;
    match id {
        "zimage" => Base::ZImage,
        "ltxv" => Base::Ltxv,
        "stableaudio" => Base::StableAudio,
        "acestep15" => Base::AceStep15,
        "acestep" => Base::AceStep,
        "hunyuan3d" => Base::Hunyuan3d,
        "sd15" => Base::Sd15,
        "sd2" => Base::Sd2,
        "sdxl" => Base::Sdxl,
        "sd3" => Base::Sd3,
        "flux" => Base::Flux,
        _ => Base::Unknown,
    }
}

/// Every LoRA on the shelf, judged against the chosen model.
///
/// **Nothing is hidden.** An incompatible one keeps its row, loses its light and carries the
/// reason — and it can still be used, because the file is theirs and Epoch only measured.
pub(crate) fn loras_for(
    shelves: &[std::path::PathBuf],
    model: epoch_engine::assets::asset::Base,
) -> Vec<PanelLora> {
    use epoch_engine::assets::asset::Base;

    let mut loras: Vec<PanelLora> = shelves
        .iter()
        .filter_map(|shelf| std::fs::read_dir(shelf).ok())
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| {
            let path = entry.path();
            // A manifest beside the file is skipped rather than offered as a LoRA.
            if path.extension().and_then(|it| it.to_str()) == Some("json") {
                return None;
            }
            let read = epoch_engine::assets::asset::understand(&path).ok()?;
            let fits = read.base.works_with(model);
            Some(PanelLora {
                file: entry.file_name().to_string_lossy().into_owned(),
                family: family_id(read.base).to_owned(),
                bytes: read.bytes,
                // **The manifest first, the header second.** A catalogue publishes trained
                // words and most files do not carry them — which `Asset::triggers` has said
                // since it existed, and this line asked the file anyway. Measured on
                // `DiivesP1.safetensors`: `["diives"]` beside it, nothing inside it.
                triggers: {
                    let said = epoch_engine::models::catalogue::triggers_beside(&path);
                    if said.is_empty() {
                        read.triggers
                    } else {
                        said
                    }
                },
                said_base: epoch_engine::models::catalogue::said_base_beside(&path),
                fits,
                why: if fits {
                    (read.base == Base::Unknown || model == Base::Unknown).then(|| {
                        "Nobody measured which family one of these belongs to, so Epoch is not \
                         refusing it. It may simply not work."
                            .to_owned()
                    })
                } else {
                    Some(format!(
                        "This is for {}, and the model you chose is {}. It will not load.",
                        read.base.plainly(),
                        model.plainly()
                    ))
                },
            })
        })
        .collect();
    loras.sort_by(|a, b| b.fits.cmp(&a.fits).then(a.file.cmp(&b.file)));
    loras
}

/// What a family arrives in, said from the family and never from a filename.
///
/// ## Why this exists
///
/// Measured 2026-08-25 against the files on this machine: `understand` reads the *kind* of every
/// part correctly — `clip_l.safetensors` is a text encoder, `flux-vae-bf16.safetensors` is a VAE —
/// and reads the *family* of almost none of them. Both encoders and the Flux VAE came back
/// `unknown`.
///
/// So the obvious help — grey the parts that belong to another family, the way a LoRA row is
/// greyed — would have greyed nothing and helped nobody. What Epoch does know is the *model's*
/// family, and what a model of that family is loaded with. That is one measurement and one fact
/// about the family, which is exactly the shape `shapes_for` already has.
///
/// It stays guidance. The user still picks every file (ADR-0033); this only stops them choosing
/// between twelve family names with nothing to go on, which is how a correctly filled panel came
/// to be refused by the server.
pub(crate) fn assembly_for(
    family: epoch_engine::assets::asset::Base,
    picked: &[Option<epoch_engine::assets::asset::Encoder>],
    offered_one: &[String],
    offered_two: &[String],
) -> PanelAssembly {
    use epoch_engine::assets::asset::Base;

    // **The same strings `Encoder::plainly` produces**, so the panel can mark the right row by
    // comparison rather than by anybody typing a filename. Written beside the sentence they
    // belong to, because a sentence and a list that disagree is worse than either alone.
    let Some((encoders, says, wanted, wants)) = (match family {
        Base::Flux => Some((
            2usize,
            "Flux is loaded with two text encoders (a CLIP-L and a T5-XXL) and its own VAE.",
            "flux",
            vec!["CLIP-L".to_owned(), "T5-XXL".to_owned()],
        )),
        Base::Sd3 => Some((
            3,
            "SD 3 is loaded with three text encoders (CLIP-L, CLIP-G and a T5) and its own VAE. Three encoders take no family at all.",
            "sd3",
            vec!["CLIP-L".to_owned(), "CLIP-G".to_owned(), "T5".to_owned()],
        )),
        // **Measured twice over.** The model is read as Z-Image from `cap_embedder.1.weight`
        // being 3840 wide — ComfyUI's own test — and the family string was found by drawing:
        // `stable_diffusion` in 12.2s, `qwen_image` in 10.1s, `flux2` refused. The encoder is a
        // Qwen3-4B, which `understand` names *a language model*, so the row marks itself.
        Base::ZImage => Some((
            1,
            "Z-Image is loaded with one text encoder — a language model, not a CLIP — and its own VAE. Measured: it draws as stable_diffusion and as qwen_image, and fails as flux2.",
            "stable_diffusion",
            vec!["a language model".to_owned()],
        )),
        // Everything else, including a family nobody measured.
        _ => None,
    }) else {
        // **Something is still said, because something is still known.**
        //
        // The count is `None` rather than a guess: how many encoders an unmeasured family takes
        // is exactly the thing that is not known. What *is* known comes from the kind — a bare
        // diffusion model carries no encoder and no VAE, whatever family it belongs to.
        // **And only here, the encoders answer instead.**
        //
        // Epoch reads the family of a *checkpoint* for five families and comes back `Unknown`
        // for Z-Image, Qwen-Image and everything newer — which is why a Z-Image panel offered
        // twenty-eight names and no help. It reads the *encoder* of every file on the shelf,
        // from the width of its embedding table (11.23), and — measured — that is what ComfyUI
        // actually keys on: `comfy/sd.py` picks the text-encoder implementation from the
        // detected model and consults `clip_type` only to tell a Flux/Klein setup apart.
        //
        // **After the checkpoint, never before it.** Written the other way round first, and it
        // was wrong in the window within a minute: a Flux model with one CLIP-L picked so far
        // was told *"a CLIP-L on its own is how Stable Diffusion is loaded"* and *"Epoch
        // measured this model as Stable Diffusion"* — a false sentence about the stronger
        // measurement, derived from a selection that was merely unfinished. A partial choice is
        // not evidence about the model; it is evidence about how far somebody has got.
        if let Some(from_parts) = by_encoder(picked, offered_one, offered_two) {
            return from_parts;
        }
        return PanelAssembly {
            family: family.plainly().to_owned(),
            encoders: None,
            says: "Epoch could not read which family this model is, so it cannot say how many text encoders it takes. Every model that arrives in parts needs at least one encoder and a VAE; each row below says what it is, and choosing one tells Epoch which family the loader wants."
                .to_owned(),
            clip_type: None,
            wants: Vec::new(),
        };
    };

    // **Offered, or unsaid.** The list belongs to the loader this many encoders selects, and the
    // two loaders speak different vocabularies. A family the server does not publish is one the
    // server would refuse, so it is left out rather than recommended.
    let published = if encoders >= 3 {
        &[][..]
    } else if encoders == 2 {
        offered_two
    } else {
        offered_one
    };

    PanelAssembly {
        family: family.plainly().to_owned(),
        encoders: Some(encoders),
        says: says.to_owned(),
        clip_type: published.iter().find(|it| it.as_str() == wanted).cloned(),
        wants,
    }
}

/// What the chosen encoder files say about how this model is put together.
///
/// **The measurement Epoch can always take.** A checkpoint's family is readable for five
/// families and unreadable for everything newer; an encoder is readable from the width of its
/// embedding table, always. And it is the one ComfyUI keys on — `comfy/sd.py` selects the text
/// encoder from the detected model and reads `clip_type` only to separate a Flux/Klein setup.
///
/// `None` where nothing was chosen or the combination is one nobody measured. Silence is the
/// honest answer to a question nobody asked; a family invented here would be refused by the
/// server after the panel was filled in perfectly.
///
/// # It describes the choice, never the model — and it marks nothing
///
/// **Because it would be circular, and it was.** Z-Image was measured only as *a diffusion model*
/// at the time, so the owner picking `clip_l` made this answer *"A CLIP-L on its own is how
/// Stable Diffusion is loaded. Epoch measured this model as Stable Diffusion"* — a claim about
/// the model derived from a wrong guess about it — and then marked `clip_l` as *one this model
/// needs*, because Stable Diffusion wants a CLIP-L. Epoch confidently confirmed the mistake it
/// had just been handed.
///
/// So every sentence here opens by saying the family could not be read, describes what **was
/// picked** rather than what the model is, and invites a change. And `wants` is always empty:
/// marking a row is a statement about the *model*, and the only thing entitled to make one is a
/// family read from the model's own tensors.
pub(crate) fn by_encoder(
    picked: &[Option<epoch_engine::assets::asset::Encoder>],
    offered_one: &[String],
    offered_two: &[String],
) -> Option<PanelAssembly> {
    use epoch_engine::assets::asset::Encoder;

    let known: Vec<Encoder> = picked.iter().flatten().copied().collect();
    if known.len() != picked.len() || known.is_empty() {
        return None;
    }
    let has = |want: Encoder| known.contains(&want);

    // Two encoders, and the pair that means Flux. `DualCLIPLoader`'s vocabulary.
    if known.len() == 2 && has(Encoder::ClipL) && (has(Encoder::T5Xxl) || has(Encoder::T5)) {
        return Some(PanelAssembly {
            family: "Flux".to_owned(),
            encoders: Some(2),
            says: "Epoch could not read which family this model is. What you have picked — a CLIP-L and a T5-XXL — is how Flux is loaded, so that is the family the double loader is being given. If this model asks for something else, change it.".to_owned(),
            clip_type: offered_two.iter().find(|it| it.as_str() == "flux").cloned(),
            // **Nothing is marked from this.** See `by_encoder`.
            wants: Vec::new(),
        });
    }

    // One language model — a Qwen3-4B and its relatives. Measured against a live server: a
    // Z-Image drew as `stable_diffusion` and as `qwen_image`, and failed as `flux2`, because the
    // encoder implementation is chosen from the file and the family only separates Flux/Klein.
    if known.len() == 1 && has(Encoder::Language) {
        return Some(PanelAssembly {
            family: "a language-model encoder".to_owned(),
            encoders: Some(1),
            says: "Epoch could not read which family this model is. The encoder you picked is a language model, which is how Z-Image and Qwen-Image are loaded — measured, that draws as stable_diffusion and as qwen_image, and fails as flux2. If this model asks for something else, change it.".to_owned(),
            clip_type: offered_one
                .iter()
                .find(|it| it.as_str() == "stable_diffusion")
                .cloned(),
            wants: Vec::new(),
        });
    }

    // One CLIP-L on its own is the original Stable Diffusion arrangement.
    if known.len() == 1 && has(Encoder::ClipL) {
        return Some(PanelAssembly {
            family: "Stable Diffusion".to_owned(),
            encoders: Some(1),
            says: "Epoch could not read which family this model is. A CLIP-L on its own is how Stable Diffusion is loaded, so that is the family the loader is being given. If this model asks for something else, change it.".to_owned(),
            clip_type: offered_one
                .iter()
                .find(|it| it.as_str() == "stable_diffusion")
                .cloned(),
            wants: Vec::new(),
        });
    }

    None
}

/// The shapes worth offering for a family, and the sizes people actually ask for.
///
/// **Two lists in one, and the difference is marked.** The first three come from what the family
/// was trained at — an SDXL panel offers 1024 and an SD 1.5 panel offers 512, because asking SD
/// 1.5 for 1024 produces two heads and asking SDXL for 512 produces mush. The rest are the sizes
/// a person names out loud: HD, Full HD, QHD, 4K, and each of them the other way up.
///
/// Offering only three ratios was the panel deciding that nobody wants a wallpaper. They do, and
/// a model asked for one usually manages; when it does not, `native` is what explains why. The
/// same arrangement as a LoRA from another family — Epoch measured and said, and the answer is
/// the user's (ADR-0033).
pub(crate) fn shapes_for(family: epoch_engine::assets::asset::Base) -> Vec<PanelShape> {
    use epoch_engine::assets::asset::Base;
    let side = match family {
        Base::Sd15 | Base::Sd2 => 512,
        // Unknown gets 1024: every family after SD 2 was trained there, so it is the better of
        // two guesses — and it is a shape somebody can change, not a claim about the file.
        _ => 1024,
    };
    let tall = (side * 2) / 3;
    let wide = side + side / 5;
    let mut shapes = vec![
        PanelShape {
            label: "2:3".to_owned(),
            width: round_to_eight(tall),
            height: round_to_eight(wide),
            native: true,
        },
        PanelShape {
            label: "1:1".to_owned(),
            width: side,
            height: side,
            native: true,
        },
        PanelShape {
            label: "3:2".to_owned(),
            width: round_to_eight(wide),
            height: round_to_eight(tall),
            native: true,
        },
    ];
    // Landscape then portrait, in the words a screen is sold in. Every one is already a multiple
    // of eight, which is what the sampler works in.
    for (label, w, h) in [
        ("HD", 1280, 720),
        ("Full HD", 1920, 1080),
        ("QHD", 2560, 1440),
        ("4K UHD", 3840, 2160),
    ] {
        shapes.push(PanelShape {
            label: label.to_owned(),
            width: w,
            height: h,
            native: false,
        });
        shapes.push(PanelShape {
            label: format!("{label} portrait"),
            width: h,
            height: w,
            native: false,
        });
    }
    // And the square sizes the families themselves are built around, for somebody who wants one
    // that is not the size their model happens to prefer.
    for square in [512, 768, 1024, 1536, 2048] {
        shapes.push(PanelShape {
            label: format!("{square} square"),
            width: square,
            height: square,
            native: square == side,
        });
    }
    shapes
}

/// Latent space works in multiples of eight; anything else is silently rounded by the sampler.
pub(crate) const fn round_to_eight(value: i64) -> i64 {
    (value / 8) * 8
}

/// Remember what the panel was filled in with.
pub(crate) fn remember_choice(chosen: PanelAsk) {
    if let Ok(mut held) = CHOSEN.lock() {
        *held = Some(chosen);
    }
}

/// What the panel was last filled in with, if anything.
pub(crate) fn choice_now() -> Option<PanelAsk> {
    CHOSEN.lock().ok().and_then(|held| held.clone())
}

/// A machine that can make a picture — this one, or one that lends its card.
///
/// Drawn from the same three facts as everything else on the deck: whether the program is there,
/// whether it is answering, and what it holds. A lent machine's are what **it** reported, because
/// the Host cannot measure them (ADR-0029) and guessing would be the invented gauge the Launcher
/// forbids.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchView {
    /// Empty for this machine; a paired machine's id otherwise.
    pub id: String,
    pub name: String,
    pub here: bool,
    pub serving: bool,
    /// Checkpoints it reported. Empty while serving is the interesting case: running, and
    /// cannot yet make a picture.
    pub models: Vec<String>,
}

/// One link of the picture chain, walked.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioCheck {
    /// What was tried, in the words of somebody reading down a list.
    pub step: String,
    pub ok: bool,
    /// What happened. On a failure this is the fix, or the server's own refusal.
    pub said: String,
}

impl StudioCheck {
    fn good(step: &str, said: impl Into<String>) -> Self {
        Self {
            step: step.to_owned(),
            ok: true,
            said: said.into(),
        }
    }

    fn bad(step: &str, said: impl Into<String>) -> Self {
        Self {
            step: step.to_owned(),
            ok: false,
            said: said.into(),
        }
    }
}

/// One Style, and whether anything here serves it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleView {
    pub name: String,
    /// Workflow names attached to it, best first.
    pub using: Vec<String>,
    /// Something here can draw it. `false` keeps the frame and loses the light.
    pub lit: bool,
}

/// One workflow this machine has.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowView {
    pub id: String,
    pub name: String,
    /// What it can be asked for, read off the graph rather than declared.
    pub can: Vec<String>,
    /// The model files it names. What the Workshop would have to fetch.
    pub needs: Vec<String>,
    /// Why it cannot run here — usually a custom node nobody installed, named.
    pub problem: Option<String>,
}

/// Where a picture actually gets made on this machine.
///
/// The other half of [`epoch_engine::capabilities::draw`], and it is where the three separate
/// things finally meet: the Styles this machine keeps, the ComfyUI that is running, and the
/// vault the picture has to land in.
///
/// ## Asked every time, like `Eyes`
///
/// Which Styles can be served is measured on each call rather than remembered. Somebody attaches
/// a workflow while Epoch is open, and a stored answer would be wrong in the direction that
/// matters — claiming a Style this machine cannot draw, which produces a confident failure
/// instead of a sentence.
pub(crate) struct Painter {
    /// Which World this is drawing for, when it is drawing for one.
    ///
    /// It is here so a finished picture can be written **where that World works** as well as
    /// into the vault. `None` for the callers that only *describe* what can be drawn — a list of
    /// Styles has no World in front of it, and reading somebody's Project Root to answer a
    /// question about a menu would be a lookup nobody asked for.
    world: Option<String>,
}

impl Painter {
    /// For a caller that is only asking what can be drawn.
    pub(crate) fn anywhere() -> Self {
        Self { world: None }
    }
}

impl Painter {
    /// The ComfyUI that is answering here, and the schema to compile against.
    ///
    /// Both or neither: a workflow compiled against a server that is not the one that will run
    /// it is a workflow compiled against a guess.
    fn serving() -> Option<(
        epoch_engine::comfy::Comfy,
        epoch_engine::assets::workflow::Schema,
    )> {
        let studio =
            epoch_engine::models::studio::look_for(epoch_engine::models::studio::Studio::ComfyUi);
        if !studio.serving {
            return None;
        }
        let comfy = epoch_engine::comfy::Comfy::at(&studio.endpoint);
        let schema = comfy.schema().ok()?;
        Some((comfy, schema))
    }
}

/// What the machine that will draw reports about itself.
///
/// The local `Easel` and a lent one answer the same four questions, so the walk asks them of
/// whichever machine `draw_on` names rather than always of this one. Measuring here and drawing
/// there would put four green links on the deck about a ComfyUI no picture goes near — true
/// about *something*, which is the most convincing form of a lying gauge.
pub(crate) struct Drawing {
    installed: bool,
    serving: bool,
    models: Vec<String>,
    /// Diffusion models on their own — Flux, Z-Image. Not checkpoints, and loaded by a different
    /// node, which is why they are a second list rather than more of the first.
    diffusion_models: Vec<String>,
    install: String,
    /// Where, in the words somebody reads on a deck.
    at: String,
    /// An address Epoch can ask directly. `None` for a lent machine — the Host cannot reach that
    /// ComfyUI, only the Bridge can, and pretending otherwise is the invented reading the
    /// Launcher forbids.
    endpoint: Option<String>,
}

/// Ask the machine this World draws on what it holds.
///
/// A lent machine is asked **now** rather than remembered: what a studio holds changes there,
/// and the roster keeps only whether it was serving. This is a deck somebody opened, so one
/// request is affordable and a stale list is not.
pub(crate) fn studio_that_draws(images: &epoch_engine::images::Images) -> Result<Drawing, String> {
    if images.draw_on.trim().is_empty() {
        let seen =
            epoch_engine::models::studio::look_for(epoch_engine::models::studio::Studio::ComfyUi);
        // Only of a server that is answering: `look_for` has just settled that, and asking
        // anyway is a round trip whose answer is already known.
        let diffusion_models = seen
            .serving
            .then(|| {
                epoch_engine::models::studio::offered_at(&seen.endpoint, "UNETLoader", "unet_name")
            })
            .flatten()
            .unwrap_or_default();
        return Ok(Drawing {
            installed: seen.installed,
            serving: seen.serving,
            models: seen.models,
            diffusion_models,
            install: seen.install.to_owned(),
            at: format!("at {}", seen.endpoint),
            endpoint: Some(seen.endpoint),
        });
    }

    let wanted = images.draw_on.trim();
    let vault = vault_dir();
    let pairings = epoch_engine::Pairings::load(&vault);
    let paired = pairings
        .all()
        .iter()
        .find(|machine| machine.id == wanted)
        .cloned()
        .ok_or_else(|| {
            format!("This World draws on a machine ({wanted}) that is no longer paired.")
        })?;
    let secrets = epoch_engine::secrets::Secrets::at(&vault);
    let have = epoch_engine::bridge::have_of(&paired, &secrets)
        .map_err(|why| format!("{} could not be reached: {why}", paired.name))?;
    let easel = have
        .easels
        .iter()
        .find(|easel| easel.id == "comfyui")
        .cloned()
        .ok_or_else(|| format!("{} does not report a ComfyUI at all.", paired.name))?;
    Ok(Drawing {
        installed: easel.installed,
        serving: easel.serving,
        // What **it** measured. The Host cannot see that machine's disk, and a guess here is
        // the invented reading the Launcher forbids.
        models: easel.models,
        // A lent machine reports its checkpoints and nothing else. Empty here is honest: Epoch
        // has not been told, and inventing a list would be inventing a reading.
        diffusion_models: Vec::new(),
        install: easel.install,
        at: format!("on {}", paired.name),
        endpoint: None,
    })
}

/// Every machine that could make a picture, this one first.
///
/// **Reported, never chosen.** The Workshop's rule about which model fits applies here too:
/// Epoch measures and says, and the person decides. A machine that stopped serving stays on the
/// list, dark, because *it is not there right now* and *you never paired one* are different
/// facts and only one of them is fixed by pairing.
pub(crate) fn benches_now(
    images: &epoch_engine::images::Images,
    here: &epoch_engine::models::studio::Easel,
) -> Vec<BenchView> {
    let mut all = vec![BenchView {
        id: String::new(),
        name: "THIS MACHINE".to_owned(),
        here: true,
        serving: here.serving,
        models: here.models.clone(),
    }];
    let _ = images;
    let secrets = epoch_engine::secrets::Secrets::at(&vault_dir());
    for paired in epoch_engine::Pairings::load(&vault_dir()).all() {
        if !paired.may(epoch_engine::Grant::Compute) {
            continue;
        }
        // **Serving is not enough, and this is where that was learned twice.** A ComfyUI running
        // with no checkpoint is running perfectly and cannot make a picture, which cost an
        // afternoon on this machine and would cost another on somebody else's. The roster keeps
        // only whether it was serving, so what it *holds* is asked now — a deck somebody
        // opened can afford one request, and a remembered list would age into a claim.
        let serving = paired.easels.iter().any(|id| id == "comfyui");
        let models = if serving {
            epoch_engine::bridge::have_of(paired, &secrets)
                .ok()
                .and_then(|have| {
                    have.easels
                        .into_iter()
                        .find(|easel| easel.id == "comfyui")
                        .map(|easel| easel.models)
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        all.push(BenchView {
            id: paired.id.clone(),
            name: paired.name.clone(),
            here: false,
            serving,
            models,
        });
    }
    all
}

/// Which machine makes the picture.
///
/// Two transports and one everything-else: the Style resolution, the compilation, the seed and
/// the confinement are identical wherever the graph runs, and this is the only thing that
/// differs. Written as a place rather than a flag so the failure can name the machine — *"the
/// MacBook could not be reached"* sends somebody to the right computer, and *"could not draw"*
/// does not.
pub(crate) enum Bench {
    Here(epoch_engine::comfy::Comfy),
    Lent(epoch_engine::bridge::LentEasel),
}

impl Bench {
    /// Where the work is happening, in the words a person reads.
    ///
    /// Not decoration: ADR-0034 names an unexplainable wait as its own risk, so a job says what
    /// it is waiting on. *The MacBook* and *ComfyUI here* send somebody to different computers.
    fn where_at(&self) -> String {
        match self {
            Bench::Here(_) => "ComfyUI on this machine".to_owned(),
            Bench::Lent(lent) => format!("ComfyUI on {}", lent.machine),
        }
    }

    fn schema(&self) -> Result<epoch_engine::assets::workflow::Schema, String> {
        match self {
            Bench::Here(comfy) => comfy.schema(),
            Bench::Lent(lent) => lent.schema(),
        }
    }

    fn draw_graph(&self, nodes: &impl serde::Serialize) -> Result<(Vec<u8>, String, f32), String> {
        let drawn = match self {
            Bench::Here(comfy) => comfy.draw_graph(nodes),
            Bench::Lent(lent) => lent.draw_graph(nodes),
        };
        // **The picture is finished, so the card is somebody else's again.**
        //
        // Here rather than in either caller: a Style and a Studio Panel are two ways of deciding
        // *what* to draw and one thing happens afterwards, so a second copy of this rule could
        // only ever disagree with the first.
        //
        // Measured 2026-08-25: a render left 10728 MB of system memory and 4369 MB of video
        // memory free; one `POST /free` returned them to 17832 and 10994. ComfyUI gives back
        // neither pool on its own, and the 7 GB is what the owner was looking at in Task Manager
        // long after the picture had appeared.
        //
        // Only on success. It used to be gated on `concurrentCrew` as well, and that gate is
        // gone — see `Bench::release`.
        if drawn.is_ok() {
            self.release();
        }
        drawn
    }

    /// Let the drawing machine go back to whatever else it was for.
    ///
    /// **Only this machine, and that is not an oversight.** A lent easel's memory belongs to the
    /// machine that lent it, the Bridge has no route that asks for it back, and inventing one to
    /// tidy a symmetry would be a request nobody measured. What is true is said and no more.
    /// ## The gate that was here, and why it is not (2026-08-28)
    ///
    /// This returned early when `Settings::keep_loaded()` was not `Never`, reasoning that
    /// somebody who wants their models resident wants the one that draws resident too. That read
    /// a **text-model** setting to answer a question about a **picture server**, and the day the
    /// setting stopped meaning *hold nothing* the answer silently inverted: ComfyUI stopped
    /// giving anything back, and the owner found 20.8 GB of Python in Task Manager after a
    /// finished render.
    ///
    /// > **A condition borrowed from another subsystem is a condition nobody re-reads when that
    /// > subsystem changes.** The gate was never wrong about ComfyUI; it was never about ComfyUI.
    ///
    /// So it frees, every time, whichever way the crew setting is set. The trade is stated
    /// rather than hidden: a second picture in a row reloads its checkpoint off disk — tens of
    /// seconds, visible, and paid by somebody who is watching — against gigabytes held
    /// indefinitely by a server nobody can see, which is what was actually reported.
    fn release(&self) {
        if let Bench::Here(comfy) = self {
            comfy.release();
        }
    }

    /// Where a shared picture is put so the graph can load it.
    ///
    /// **Only here, for now.** A reference on a lent machine means handing the bytes over as
    /// well, which is a second thing to carry and a second thing to get wrong; it is refused by
    /// name rather than silently ignored, because a picture that was ignored is exactly the
    /// failure `cannot_take_a_reference` exists to prevent.
    fn hand_over(&self, name: &str, bytes: &[u8]) -> Result<String, String> {
        match self {
            Bench::Here(comfy) => comfy.hand_over(name, bytes),
            Bench::Lent(lent) => Err(format!(
                "REFUSED. Nothing was drawn and no picture exists. Pictures are being made on {}, and a reference picture cannot be sent there yet. Tell the user, and that drawing on this machine would take one.",
                lent.machine
            )),
        }
    }
}

/// Resolve where this World draws, and say plainly when it cannot.
///
/// `draw_on` is the user's choice and is never reinterpreted: a machine that stopped serving is
/// **reported**, not quietly replaced with a working one. Drawing somewhere the user did not
/// choose is a different answer to a different question, which is the same rule that governs
/// Styles one layer up.
/// Hand a reference picture to the studio, and answer with the name it knows it by.
///
/// ## Why the name comes back rather than the path going in
///
/// A `LoadImage` node loads by a name **that server** holds, and a lent ComfyUI has its own
/// disk — measured and documented on `Comfy::hand_over` since 2026-08-22. So a path from this
/// machine is meaningless there, and the only thing worth keeping is what the server answered.
///
/// ## Why bytes rather than a path from the frontend
///
/// ADR-0024: the frontend never touches the filesystem. The picture arrives as the `data:` URI
/// the webview's own `<input type="file">` produced, and the only thing that reads a disk here
/// is the Engine.
///
/// The bytes are also **not** filed in the vault. This is not artwork the user is keeping — it
/// is a reference for one picture, and the studio already has it. Writing a copy Epoch would
/// then have to clean up is a second thing to get wrong.
pub fn reference_for_studio(data_uri: &str) -> Result<String, String> {
    let bytes = epoch_engine::import::decode_shared(data_uri)
        .map_err(|why| format!("that picture could not be read: {why}"))?;
    if bytes.is_empty() {
        return Err("that picture is empty".to_owned());
    }
    let bench = bench_for(&epoch_engine::images::Images::load(&vault_dir()))?;
    // Named from the bytes, exactly as everything else Epoch names is (ADR-0024). The upload
    // overwrites, so the same picture is the same file on the server rather than a second one.
    let name = format!(
        "epoch-ref-{:016x}.png",
        epoch_engine::import::fingerprint(&bytes)
    );
    bench.hand_over(&name, &bytes)
}

pub(crate) fn bench_for(images: &epoch_engine::images::Images) -> Result<Bench, String> {
    if images.draw_on.trim().is_empty() {
        return match Painter::serving() {
            Some((comfy, _)) => Ok(Bench::Here(comfy)),
            None => Err("REFUSED. Nothing was drawn and no picture exists. ComfyUI is not running on this machine. Tell the user, and that CREATIONS in the Launcher can start it."
                .to_owned()),
        };
    }

    let wanted = images.draw_on.trim();
    let pairings = epoch_engine::Pairings::load(&vault_dir());
    let paired = pairings
        .all()
        .iter()
        .find(|machine| machine.id == wanted)
        .cloned()
        .ok_or_else(|| {
            format!(
                "REFUSED. Nothing was drawn and no picture exists. This World draws on a machine ({wanted}) that is no longer paired. Tell the user, and that CREATIONS can choose another."
            )
        })?;
    if !paired.easels.iter().any(|id| id == "comfyui") {
        return Err(format!(
            "REFUSED. Nothing was drawn and no picture exists. {} is paired and was not serving ComfyUI when it was last asked. Tell the user to open it there.",
            paired.name
        ));
    }
    let secrets = epoch_engine::secrets::Secrets::at(&vault_dir());
    epoch_engine::bridge::LentEasel::of(&paired, &secrets).map(Bench::Lent)
}

/// How the graph should load what the panel was filled in with.
///
/// **Three shapes, derived from what was picked rather than from what the file is.** A model on
/// the `diffusion` shelf arrives in parts; a checkpoint with encoders chosen beside it carries
/// its own model and VAE and not its encoder; a checkpoint with nothing beside it carries
/// everything. Epoch never reads a checkpoint and decides it is missing something (ADR-0033) —
/// the person picking an encoder is what says so.
///
/// The third case is not hypothetical: `ltxv-2b-0.9.6-distilled` is a checkpoint, and ComfyUI
/// answers *"clip input is invalid: None"* when its own CLIP output is wired to a text encoder.
fn loading_for(ask: &PanelAsk) -> Result<epoch_engine::assets::workflow::Loading, String> {
    use epoch_engine::assets::workflow::Loading;
    if ask.kind == "diffusion" {
        // **The person chose the parts.** Nothing is inferred from a filename here: a model
        // that arrives in parts needs an encoder, a family and a VAE, and which ones is
        // exactly the question the panel exists to put.
        if ask.clip.is_empty() || ask.clip_type.trim().is_empty() || ask.vae.trim().is_empty() {
            return Err(
                "that model needs a text encoder, its family and a VAE chosen before it can draw"
                    .to_owned(),
            );
        }
        return Ok(Loading::Assembled {
            unet: ask.checkpoint.clone(),
            clip: ask.clip.clone(),
            clip_type: ask.clip_type.clone(),
            vae: ask.vae.clone(),
        });
    }
    if ask.clip.is_empty() {
        return Ok(Loading::Checkpoint(ask.checkpoint.clone()));
    }
    Ok(Loading::Encoded {
        checkpoint: ask.checkpoint.clone(),
        clip: ask.clip.clone(),
        clip_type: ask.clip_type.clone(),
    })
}

/// Draw what the character asked for, the way the person chose.
///
/// Its own function because it is a different *route to the same picture*: a Style resolves to a
/// workflow somebody imported, and this composes a graph from a panel. Both end at the same
/// `Drawn`, which is what keeps everything downstream unable to tell them apart.
pub(crate) fn draw_as_chosen(
    vault: &std::path::Path,
    bench: &Bench,
    schema: &epoch_engine::assets::workflow::Schema,
    chosen: &PanelAsk,
    describe: &str,
    // Where this World works, when it has somewhere. The picture is written there as well.
    root: Option<&std::path::Path>,
) -> Result<epoch_engine::capabilities::draw::Drawn, String> {
    let loading = loading_for(chosen).map_err(|_| {
        "REFUSED. Nothing was drawn and no picture exists. That model arrives in parts and the \
         panel was not told which encoder, family and VAE to load. Tell the user to choose them."
            .to_owned()
    })?;

    let ask = epoch_engine::assets::workflow::Ask {
        loading,
        loras: chosen.loras.clone(),
        // Whatever the panel was last set to, including no upscale at all.
        upscale: Some(chosen.upscale.trim().to_owned()).filter(|it| !it.is_empty()),
        control: steering(chosen),
        // Whatever the panel was last set to, exactly like the upscale above.
        from: Some(chosen.from.trim().to_owned())
            .filter(|it| !it.is_empty())
            .map(|picture| (picture, (1.0 - chosen.keep).clamp(0.0, 1.0))),
        beyond: chosen.beyond(),
        // The character's words, never the panel's. The panel chose *how*; this is *what*.
        prompt: describe.to_owned(),
        negative: chosen.negative.clone(),
        width: chosen.width.clamp(64, 4096),
        height: chosen.height.clamp(64, 4096),
        steps: chosen.steps.clamp(1, 200),
        cfg: chosen.cfg.clamp(0.0, 30.0),
        seed: if chosen.seed == 0 {
            seed_now() as i64
        } else {
            chosen.seed
        },
        batch: chosen.batch.clamp(1, 8),
        guidance: if chosen.guidance <= 0.0 {
            3.5
        } else {
            chosen.guidance.clamp(0.0, 20.0)
        },
    };

    let graph = epoch_engine::assets::workflow::compose(&ask, schema).map_err(|why| {
        format!(
            "REFUSED. Nothing was drawn and no picture exists. This ComfyUI cannot run that: \
             {why}. Tell the user."
        )
    })?;
    let (bytes, _name, seconds) = bench.draw_graph(&graph)?;
    // Kept where the work is as well as in the vault. **The path is deliberately not put in
    // what the model reads** — a result that names a file hands it the one thing it needs to
    // claim a picture that does not exist (ADR-0030's third amendment). The person is told by
    // the surface; the character is told a picture was made.
    let kept = epoch_engine::import::keep_made(vault, root, &bytes)
        .map_err(|why| format!("the picture was drawn and could not be kept: {why}"))?;
    Ok(epoch_engine::capabilities::draw::Drawn {
        file: kept.file,
        // What a screen calls this one. Not a Style — the person chose the parts themselves.
        style: "your own settings".to_owned(),
        seconds,
    })
}

/// **Epoch started the picture studio, so Epoch may stop it.**
///
/// The whole of the rule. Somebody who started ComfyUI themselves — in a terminal, from its own
/// launcher, to use its web page — is using it, and a picture finishing in another application is
/// not a reason to take it away from them. This is set only where Epoch spawns it, and it is the
/// only thing that licenses stopping it again.
///
/// A process rather than a World, so it lives here rather than in the World's state: both the
/// panel and a character's turn start and release the same one server, and a flag either of them
/// could not see would be two answers to one question.
static STUDIO_IS_OURS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Start the picture studio if it is here, idle and ours to start — and wait for it to answer.
///
/// **Called wherever something is about to draw.** Opening the panel is asking to draw, and so is
/// a character reaching for the brush; in both cases the alternative is a refusal that says
/// *nothing here draws* about a machine that has ComfyUI installed and switched off.
///
/// `wait` is the difference between the two callers. The panel returns immediately and asks
/// again while it fills in, so it waits for nothing; a capability answers inside its call and has
/// to hold. Measured on this machine: about thirty seconds from start to answering.
pub(crate) fn wake_studio(wait: std::time::Duration) -> bool {
    let studio = epoch_engine::models::studio::Studio::ComfyUi;
    let seen = epoch_engine::models::studio::look_for(studio);
    if seen.serving {
        return true;
    }
    if !seen.installed {
        return false;
    }
    // Drawing somewhere else: that machine's server is not ours to start or stop (ADR-0029).
    if !epoch_engine::images::Images::load(&vault_dir())
        .draw_on
        .trim()
        .is_empty()
    {
        return false;
    }
    let Some(command) = seen.start else {
        return false;
    };
    if epoch_engine::models::runtimes::open_in_terminal(
        &format!("Epoch - {}", studio.name()),
        &command,
    )
    .is_err()
    {
        return false;
    }
    STUDIO_IS_OURS.store(true, std::sync::atomic::Ordering::Relaxed);

    // **It was started. A caller who waits for nothing is asking whether one is coming.**
    //
    // Found in the window on 2026-08-30, on the first run of the panel starting the studio for
    // itself: the spawn succeeds, nothing is serving a moment later, and the loop below answered
    // `false` — so the panel drew *"Epoch could not start it"* over a ComfyUI it had just
    // started, which was answering on 8188 thirty seconds afterwards.
    //
    // `false` there meant *not serving yet* and the surface read it as *no*, which is the
    // inversion this codebase has paid for more than once. The two callers ask different
    // questions and now get different answers: the panel asks *is one on the way* and re-asks
    // until the lists fill in; a capability answers inside its own call, passes a real wait, and
    // still gets *is it serving*.
    if wait.is_zero() {
        return true;
    }

    let until = std::time::Instant::now() + wait;
    loop {
        if epoch_engine::models::studio::look_for(studio).serving {
            return true;
        }
        if std::time::Instant::now() >= until {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

/// The drawing is done: stop the studio **if Epoch started it** and nothing else is queued.
///
/// Two conditions and each is somebody's work. *If Epoch started it*, because a server somebody
/// launched themselves is one they are using. *If nothing is queued*, asked of the server rather
/// than tracked here — a picture can be asked for from the panel, from a character's turn and
/// from ComfyUI's own web page, and stopping mid-render is the one way this could take something
/// away from somebody who was waiting for it.
pub(crate) fn let_studio_go() -> Option<String> {
    if !STUDIO_IS_OURS.load(std::sync::atomic::Ordering::Relaxed) {
        return None;
    }
    let seen =
        epoch_engine::models::studio::look_for(epoch_engine::models::studio::Studio::ComfyUi);
    if !seen.serving {
        STUDIO_IS_OURS.store(false, std::sync::atomic::Ordering::Relaxed);
        return None;
    }
    if epoch_engine::models::studio::is_busy(&seen.endpoint) {
        // Still drawing. Left alone, and left ours — whatever asks next will find it idle.
        return None;
    }
    let said =
        epoch_engine::models::studio::stop_server(epoch_engine::models::studio::Studio::ComfyUi)
            .ok();
    STUDIO_IS_OURS.store(false, std::sync::atomic::Ordering::Relaxed);
    said
}

/// Read one model back onto the card, if there is one to read.
///
/// A free function because both callers are already off the World's lock and one of them is a
/// thread that outlives the turn that spawned it. Best-effort, like every release: a model that
/// did not come back is a slower next message, and the lamp says so because it reads the card.
fn warm_back(
    providers: &std::sync::RwLock<epoch_engine::ProviderRegistry>,
    brain: Option<&(String, String)>,
) {
    let Some((backend, model)) = brain else {
        return;
    };
    if let Some(provider) = providers.read().expect("providers lock").get(backend) {
        provider.warm(model);
    }
}

/// `a video` -> `A video`. For a sentence a character says.
fn capitalised(said: &str) -> String {
    let mut chars = said.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Where this World works, when it has a Project Root.
///
/// A picture belongs where the person is working (ADR-0025 — the Project Root is the World's
/// workspace), and a World without one still draws: the vault copy is what everything downstream
/// reads, and this is the extra.
fn where_the_work_is(world: Option<&String>) -> Option<std::path::PathBuf> {
    let world = world?;
    epoch_engine::ProjectRoots::load(&vault_dir())
        .open(world)
        .map(|root| root.path().to_path_buf())
}

/// Which shelf a model of this kind is looked for on.
fn shelf_of(kind: &str) -> epoch_engine::models::generative::Shelf {
    if kind == "diffusion" {
        epoch_engine::models::generative::Shelf::DiffusionModels
    } else {
        epoch_engine::models::generative::Shelf::Models
    }
}

/// This model's own hash, for a caller that may pay for it.
///
/// `measure` is the whole decision. **On a read path it is false**: hashing a twelve-gigabyte
/// file to open a panel is the wrong trade, and `None` there means *nobody has measured this
/// yet* rather than *it has none*. **After a picture is drawn it is true**: the render already
/// cost tens of seconds, the person is not waiting on a form, and it is paid once per file ever
/// — measured on this machine at 1.8 GB/s, so 3.4 s for a 6.2 GB model and about 7 s for Flux.
///
/// Measuring also writes the manifest beside the file, which is the same act ADR-0032 already
/// describes: a file is understood from its bytes, and what was read is kept next to it.
fn model_hash(file: &str, kind: &str, measure: bool) -> Option<String> {
    let path = epoch_engine::models::generative::file_behind(shelf_of(kind), file)?;
    if let Some(known) = epoch_engine::models::catalogue::hash_beside(&path) {
        return Some(known);
    }
    if !measure {
        return None;
    }
    // No catalogues: this is not a lookup, it is a measurement. Asking two websites about a file
    // somebody has already drawn with would be a network round trip for a fact the bytes answer.
    epoch_engine::models::catalogue::identify(&path, &[])
        .ok()
        .map(|known| known.sha256)
}

/// File what this combination did, against the model's hash.
///
/// Silent about everything it cannot do: a model whose file Epoch cannot find, or whose hash
/// could not be taken, simply is not remembered. A recipe is advice, and advice nobody can key
/// to a file is worse than none.
fn remember_recipe(ask: &PanelAsk, outcome: epoch_engine::models::recipes::Outcome) {
    // **Only a model that arrives in parts.** A checkpoint carries its own encoder and VAE, so
    // there is nothing to have got right and nothing to offer next time — a recipe for one would
    // be a record nobody could read and nobody could use.
    if ask.kind != "diffusion" {
        return;
    }
    let Some(model) = model_hash(&ask.checkpoint, &ask.kind, true) else {
        return;
    };
    let library = epoch_engine::models::generative::Library::here();
    let mut kept = epoch_engine::models::recipes::Recipes::load(library.root());
    kept.remember(epoch_engine::models::recipes::Recipe {
        model,
        kind: ask.kind.clone(),
        clip: ask.clip.clone(),
        clip_type: ask.clip_type.clone(),
        vae: ask.vae.clone(),
        outcome,
        at: epoch_engine::now_ms(),
    });
    let _ = kept.save(library.root());
}

impl epoch_engine::capabilities::draw::Easel for Painter {
    fn styles(&self) -> Vec<String> {
        epoch_engine::images::Images::load(&vault_dir())
            .lit()
            .into_iter()
            .filter(|(_, served)| *served)
            .map(|(name, _)| name)
            .collect()
    }

    fn draw(
        &self,
        asked: &epoch_engine::capabilities::draw::Asked,
    ) -> Result<epoch_engine::capabilities::draw::Drawn, String> {
        let vault = vault_dir();

        // **A character reaching for the brush is asking to draw**, exactly as opening the panel
        // is — so the same server is started, and here the call has to hold until it answers
        // because a capability answers inside its own call. Measured at about thirty seconds;
        // ninety is room for a slower machine and still a bounded wait.
        //
        // Without this, a picture asked for after the panel released the server would be refused
        // with *nothing here draws*, about a machine that has ComfyUI installed and switched off.
        wake_studio(std::time::Duration::from_secs(90));

        let images = epoch_engine::images::Images::load(&vault);
        // Where, before what: the graph is compiled against the schema of the server that will
        // actually run it, so this has to be settled first.
        let bench = bench_for(&images)?;
        let schema = bench.schema()?;

        // **What the person chose in the panel wins, and it is not an argument.** The character
        // said what to draw; the model, the LoRAs and the size came from the panel a moment ago.
        // That is what lets a character choose nothing about *how* and still draw exactly what
        // somebody asked for (ADR-0033).
        if let Some(chosen) = choice_now() {
            return draw_as_chosen(
                &vault,
                &bench,
                &schema,
                &chosen,
                &asked.describe,
                where_the_work_is(self.world.as_ref()).as_deref(),
            );
        }

        // Which Style, and never a substitute. `usually` when nobody said; the first one that
        // can serve when even that is empty.
        let wanted = asked
            .style
            .clone()
            .unwrap_or_else(|| images.usually.clone());
        let (style, kept) = match images.serving(&wanted) {
            Some(kept) => (wanted, kept),
            None if asked.style.is_none() => {
                // Nobody asked for a particular one, so the World's own default not being set up
                // is not a refusal — the first Style that can draw is what this World draws.
                let first = images
                    .lit()
                    .into_iter()
                    .find(|(_, served)| *served)
                    .map(|(name, _)| name)
                    .ok_or(
                        "REFUSED. Nothing was drawn and no picture exists. This World has no \
                         workflow attached to any Style yet. Tell the user, and that CREATIONS \
                         in the Launcher can build one.",
                    )?;
                let kept = images.serving(&first).ok_or("nothing serves that Style")?;
                (first, kept)
            }
            None => {
                return Err(format!(
                    "REFUSED. Nothing was drawn and no picture exists. No workflow here \
                     draws {wanted}. Tell the user, and say what this World does draw."
                ))
            }
        };

        // **Compiled now, from the source.** The stored summary is for a screen; a run compiles
        // again, because this ComfyUI may have gained a node since the import — which is the
        // entire reason the original is kept.
        let mut workflow = images
            .open(&vault, &kept.id, &schema)
            .map_err(|why| why.to_string())?;

        // **A reference only where a workflow can take one.** Refusing beats drawing something
        // that ignored the picture: a graph with no `LoadImage` runs perfectly and quietly
        // answers a different question.
        //
        // **And the refusal must not offer the thing it just refused.** It used to end *"or ask
        // without one"*, which contradicted the sentence above it: asking without the picture
        // *is* the different question. Measured 2026-08-23 — `gemma4:12b` took the offer three
        // times, rephrasing the description each round, and the turn ended with nothing drawn and
        // no explanation for the user. Every refusal on this path now opens with the outcome and
        // names what not to do next (ADR-0030, third amendment).
        if let Some(reference) = &asked.from_image {
            if !workflow.opening.can.from_image {
                return Err(cannot_take_a_reference(&style));
            }
            let bytes = std::fs::read(reference)
                .map_err(|err| format!("that picture could not be read ({err})"))?;
            let name = reference
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "reference.png".to_owned());
            // Handed over rather than pointed at: a lent ComfyUI has its own disk, and a path
            // from this machine means nothing there.
            let there = bench.hand_over(&name, &bytes)?;
            workflow.compiled.show(&there);
        }

        workflow.compiled.say(&asked.describe, None);
        // An edit keeps the picture's own size, so there is nothing to set — and setting one
        // would answer a question nobody asked.
        if let Some((width, height)) = asked.shape.pixels() {
            workflow.compiled.shape(width, height);
        }
        // A different picture every time it is asked, unless somebody pinned the workflow's own
        // seed on purpose — which they cannot yet, so this is simply *not the same picture*.
        workflow.compiled.seed(seed_now());
        harder(&mut workflow.compiled, asked.detail);

        let (bytes, _name, seconds) = bench.draw_graph(&workflow.compiled.nodes)?;

        // Into the vault, beside the pictures the user shared — which is the folder the
        // Chronicle already knows how to draw from. ComfyUI's own output directory belongs to
        // ComfyUI: it is cleaned, re-pointed and shared with everything else somebody runs.
        //
        // And into the World's Project Root, because that is where the person is working. The
        // path is deliberately kept out of what the model reads: a result that names a file
        // hands it the one thing it needs to claim a picture nobody made (ADR-0030's third
        // amendment).
        let kept = epoch_engine::import::keep_made(
            &vault,
            where_the_work_is(self.world.as_ref()).as_deref(),
            &bytes,
        )
        .map_err(|err| format!("the picture was drawn and could not be kept: {err}"))?;

        Ok(epoch_engine::capabilities::draw::Drawn {
            file: kept.file,
            style,
            seconds,
        })
    }
}

/// Scale what the workflow already asks for.
///
/// **Relative, never absolute.** Its author chose 20 steps or 8 for a reason — a turbo model
/// needs four — and a number of Epoch's own would make *fine* mean *broken* on half of them.
pub(crate) fn harder(
    workflow: &mut epoch_engine::assets::workflow::Workflow,
    detail: epoch_engine::capabilities::draw::Detail,
) {
    let times = detail.times();
    if (times - 1.0).abs() < f32::EPSILON {
        return;
    }
    let opening = workflow.opening();
    for id in opening.steps {
        let Some(node) = workflow.nodes.get_mut(&id) else {
            continue;
        };
        if let Some(epoch_engine::assets::workflow::Input::Value(value)) = node.inputs.get("steps")
        {
            if let Some(steps) = value.as_f64() {
                // At least one step, and never fewer than the author's own floor of sanity.
                let scaled = ((steps as f32) * times).round().max(1.0) as u64;
                node.inputs.insert(
                    "steps".into(),
                    epoch_engine::assets::workflow::Input::Value(serde_json::json!(scaled)),
                );
            }
        }
    }
}

/// A seed nobody chose.
///
/// The clock, because a picture asked for twice should not come back identical — and because a
/// counter would have to live somewhere, which is state for something nothing reads.
pub(crate) fn seed_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos() as u64)
        .unwrap_or(1)
}

/// What a character is told when the Style it resolved to cannot take a reference picture.
///
/// Its own function so a test can hold the words. This text is **prompt**, and the version
/// that ended *"or ask without one"* invited the model to drop the picture and try again —
/// which is a different question, and the one the refusal exists to prevent. Measured
/// 2026-08-23: three rounds, three rephrasings, nothing drawn, nothing explained.
///
/// The phrase is **avoided rather than negated**. Writing *"do not ask again"* puts the
/// words *ask again* in front of a model that reads instructions better than it reads
/// negations, and the test forbids the phrase in any framing for that reason.
pub(crate) fn cannot_take_a_reference(style: &str) -> String {
    format!(
        "REFUSED. Nothing was drawn and no picture exists. {style} draws from words only \
         and cannot use a reference picture. Tell the user that, and that a workflow \
         which takes one must be attached in CREATIONS. Dropping the picture would \
         answer a different question, so do not."
    )
}

/// Every local brain, asked for a picture, all the way to a file on disk.
///
/// ## Why this is here and not in `epoch-engine`
///
/// Because the easel is. `Painter` reads this World's Styles, picks the bench, compiles the
/// workflow against *that* ComfyUI's schema and posts it — and every one of those steps lives in
/// the shell. A test in the Engine can prove a model reaches for the brush; only a test here can
/// prove a picture exists afterwards.
///
/// ## And why it does not build its own request
///
/// `tests/who_reaches_for_the_brush.rs` declared the tools itself. It passed on three runtimes
/// while every character on two of them was being handed no tools at all, because it measured a
/// path Epoch does not take. This one asks each Provider what it declares, applies **the same
/// rule the turn loop applies** to decide whether to offer anything, and then calls the same
/// `take_turn` the window calls. If the offering rule breaks again, this goes red.
///
/// `#[ignore]` — needs the three runtimes serving and a ComfyUI with a Style attached. Start the
/// runtimes from Epoch's Connections deck; that is the path a user has.
/// `cargo test -p epoch-tauri -- --ignored --nocapture every_brain`
/// One row of the panel's steering: which ControlNet, which picture, how hard.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelSteer {
    /// The ControlNet, as the server lists it.
    #[serde(default)]
    pub file: String,
    /// The reference picture, **by the name ComfyUI answered with**. Never a path from here:
    /// a lent ComfyUI has its own disk, which is why `Comfy::hand_over` answers with a name.
    #[serde(default)]
    pub image: String,
    /// What turns the reference into something the ControlNet can read, by node class.
    ///
    /// Three states, and the empty one is not a default: `""` is **nobody has said yet** and
    /// `GENERATE` waits for it, `"already"` means the picture is already a control map, and
    /// anything else names the node that makes one. A canny ControlNet handed a photograph draws
    /// noise — measured — and neither Epoch nor the picture can say which was meant.
    #[serde(default)]
    pub prepare: String,
    /// How hard it steers. `0` is *never set* and reads as the server's own 1.0.
    #[serde(default)]
    pub strength: f64,
}

/// The panel's ControlNet, when it has both halves.
///
/// **Both, or neither.** A ControlNet with no reference picture steers by nothing, and a
/// reference picture with no ControlNet is a file the graph has no node to read. Either half
/// alone is a control that silently does nothing, which is the shape this codebase keeps
/// deleting — so the graph is left exactly as it was.
fn steering(chosen: &PanelAsk) -> Vec<epoch_engine::assets::workflow::Steering> {
    chosen
        .controls
        .iter()
        // **Both halves, per row.** A ControlNet with no picture steers by nothing and a picture
        // with no ControlNet is a file the graph has no node to read. A half-filled row is
        // dropped here *and* the surface refuses to draw with one, so neither can be the only
        // guard — the panel says why, and this makes sure a stale one cannot slip through.
        //
        // **And the preparation, for the same reason.** An unanswered one is not *already
        // prepared*: reading silence as an answer is what drew noise, so a row nobody has
        // finished is dropped rather than guessed at.
        .filter(|row| {
            !row.file.trim().is_empty()
                && !row.image.trim().is_empty()
                && !row.prepare.trim().is_empty()
        })
        .map(|row| epoch_engine::assets::workflow::Steering {
            model: row.file.trim().to_owned(),
            image: row.image.trim().to_owned(),
            // `already` is the person saying they drew the control map themselves.
            prepare: match row.prepare.trim() {
                "already" => None,
                class => Some(class.to_owned()),
            },
            // `0` is *never set*, not *no steering at all*: a row that has never been touched
            // means the server's own default, and a strength of zero would draw as if the
            // ControlNet were not there while showing that it is.
            strength: if row.strength > 0.0 {
                row.strength.clamp(0.0, 2.0)
            } else {
                1.0
            },
            // The whole of the sampling. Anything narrower is a decision nobody has asked to
            // make, and `ControlNetApplyAdvanced` requires both — measured, not optional.
            start: 0.0,
            end: 1.0,
        })
        .collect()
}

// **`every_brain_on_this_machine_can_make_a_picture` was here, and it measured a path that no
// longer exists** (removed 2026-08-28).
//
// It asked each local runtime whether a character could reach `draw_image` and end at a PNG, and
// it was the right test on the day it was written: 11.24 built it precisely because a harness
// that declares its own tools proves nothing about what Epoch would offer.
//
// Drawing is the panel's alone now. A character opens the Studio Panel and the person makes the
// picture, so there is no per-brain tool call left to measure — what a brain can do with a
// picture is `see_image`, which has its own measurements. Deleted rather than left `#[ignore]`d:
// a test nobody can run is a claim nobody can check.

#[cfg(test)]
mod a_family_is_one_fact {
    use super::{base_named, family_id};
    use epoch_engine::assets::asset::Base;

    /// Every family Epoch can read, in one list. **Both tests read this one**, so a family added
    /// to `asset.rs` and forgotten here fails the first test by name rather than passing the
    /// second one vacuously.
    const EVERY: [Base; 12] = [
        Base::Sd15,
        Base::Sd2,
        Base::Sdxl,
        Base::Sd3,
        Base::Flux,
        Base::ZImage,
        Base::Ltxv,
        Base::StableAudio,
        Base::AceStep,
        Base::AceStep15,
        Base::Hunyuan3d,
        Base::Unknown,
    ];

    /// **Every family in the graph table must be reachable from a model's id.**
    ///
    /// The AUDIO tab was dark on a machine holding both audio checkpoints, because the panel
    /// compared an *id* (`stableaudio`) with a *name* (`Stable Audio`) case-insensitively. That
    /// happens to be true for `ltxv` and `hunyuan3d` and false the moment a name carries a space
    /// or a hyphen — so the bug lived only in the two families nobody was watching, and the two
    /// that worked were working by luck.
    ///
    /// This asserts the round trip rather than the comparison: `id -> Base -> name` must land on
    /// the key `FAMILIES` is written with.
    #[test]
    fn every_graph_family_is_reachable_from_a_model_id() {
        for (name, _) in epoch_engine::assets::workflow::FAMILIES {
            let base = EVERY
                .into_iter()
                .find(|it| it.plainly() == name)
                .unwrap_or_else(|| {
                    panic!("no Base answers to the family key {name} — add one to `asset.rs`")
                });
            // The id is what a model carries on the view; it must come back to this same family.
            assert_eq!(base_named(family_id(base)).plainly(), name);
        }
    }

    /// And the id is round-trippable for every family Epoch can read, not only the graph ones.
    #[test]
    fn an_id_survives_the_trip_back() {
        for base in EVERY {
            assert_eq!(base_named(family_id(base)), base);
        }
    }
}

#[cfg(test)]
pub(crate) mod what_a_family_arrives_in {
    use super::{assembly_for, shapes_for};
    use epoch_engine::assets::asset::Base;

    fn dual() -> Vec<String> {
        ["sdxl", "sd3", "flux", "hunyuan_video"]
            .iter()
            .map(|it| (*it).to_owned())
            .collect()
    }
    fn single() -> Vec<String> {
        ["stable_diffusion", "qwen_image"]
            .iter()
            .map(|it| (*it).to_owned())
            .collect()
    }

    /// **The circle this closes.** Z-Image was measured only as *a diffusion model*, so picking a
    /// CLIP-L for it made `by_encoder` answer *"A CLIP-L on its own is how Stable Diffusion is
    /// loaded — Epoch measured this model as Stable Diffusion"* and then mark that same CLIP-L as
    /// *one this model needs*. Epoch confirmed the mistake it had just been handed.
    ///
    /// Reading the model closes it: `cap_embedder.1.weight` at 3840 wide is Z-Image (ComfyUI's own
    /// test), so the checkpoint answers first and the wrong encoder is never confirmed.
    #[test]
    fn z_image_says_it_wants_a_language_model_whatever_was_picked() {
        use epoch_engine::assets::asset::Encoder;

        let said = assembly_for(
            Base::ZImage,
            // The wrong one, chosen in good faith.
            &[Some(Encoder::ClipL)],
            &single(),
            &dual(),
        );
        assert_eq!(said.family, "Z-Image");
        assert_eq!(said.encoders, Some(1));
        assert_eq!(said.wants, vec!["a language model".to_owned()]);
        assert_eq!(said.clip_type.as_deref(), Some("stable_diffusion"));
        // The one that does not work is named, because that is the trap.
        assert!(said.says.contains("flux2"), "{}", said.says);
    }

    /// **A guess about the choice is never a claim about the model, and never marks a row.**
    ///
    /// `wants` drives the mark in the dropdown, and a mark is a statement about what the *model*
    /// needs. Only a family read from the model's own tensors is entitled to make one.
    #[test]
    fn an_encoder_derived_answer_claims_nothing_and_marks_nothing() {
        use epoch_engine::assets::asset::Encoder;

        let said = assembly_for(Base::Unknown, &[Some(Encoder::ClipL)], &single(), &dual());
        assert!(said.wants.is_empty(), "it must not mark a row");
        assert!(
            said.says
                .starts_with("Epoch could not read which family this model is"),
            "{}",
            said.says
        );
        assert!(said.says.contains("change it"), "{}", said.says);
    }

    /// **A partial selection is not evidence about the model.**
    ///
    /// Written the other way round first, and the window refuted it inside a minute: a Flux model
    /// with one CLIP-L picked so far was told *"a CLIP-L on its own is how Stable Diffusion is
    /// loaded"* and *"Epoch measured this model as Stable Diffusion"* — a false sentence about
    /// the stronger measurement, derived from a choice that was merely unfinished.
    #[test]
    fn a_measured_checkpoint_outranks_what_the_encoders_so_far_suggest() {
        use epoch_engine::assets::asset::Encoder;

        let said = assembly_for(
            Base::Flux,
            // Halfway through choosing Flux's two.
            &[Some(Encoder::ClipL)],
            &single(),
            &dual(),
        );
        assert_eq!(said.family, "Flux");
        assert_eq!(
            said.encoders,
            Some(2),
            "still two, so the panel keeps waiting"
        );
        assert_eq!(said.clip_type.as_deref(), Some("flux"));
    }

    /// The complaint this answers: a Z-Image panel offered twenty-eight families and no help.
    ///
    /// **Epoch reads a checkpoint's family for five families and an encoder for every file.** So
    /// the guidance is keyed on the encoder — which is also what ComfyUI keys on: `comfy/sd.py`
    /// picks the text-encoder implementation from the detected model and consults `clip_type`
    /// only to tell a Flux/Klein setup apart.
    ///
    /// Verified by drawing, not by reading the source: a Z-Image with a Qwen3-4B encoder drew in
    /// 12.2s as `stable_diffusion`, in 10.1s as `qwen_image`, and failed as `flux2`.
    #[test]
    fn a_language_model_encoder_answers_for_a_family_epoch_cannot_read() {
        use epoch_engine::assets::asset::Encoder;

        // The model itself is unreadable — which is the whole case.
        let said = assembly_for(
            Base::Unknown,
            &[Some(Encoder::Language)],
            &single(),
            &dual(),
        );
        assert_eq!(said.encoders, Some(1));
        assert_eq!(said.clip_type.as_deref(), Some("stable_diffusion"));
        assert!(said.says.contains("language model"), "{}", said.says);
        // The one that does not work is named, because that is the trap.
        assert!(said.says.contains("flux2"), "{}", said.says);
    }

    /// A CLIP-L and a T5-XXL are Flux however unreadable the checkpoint is.
    #[test]
    fn the_pair_that_means_flux_is_read_from_the_pair() {
        use epoch_engine::assets::asset::Encoder;

        let said = assembly_for(
            Base::Unknown,
            &[Some(Encoder::ClipL), Some(Encoder::T5Xxl)],
            &single(),
            &dual(),
        );
        assert_eq!(said.encoders, Some(2));
        assert_eq!(said.clip_type.as_deref(), Some("flux"));
    }

    /// **And silence where nothing was measured.** An encoder Epoch could not read is not an
    /// argument for guessing a family: the server would refuse it after the panel was filled in
    /// perfectly, which is the failure this whole path exists to prevent.
    #[test]
    fn an_unreadable_encoder_produces_no_family_at_all() {
        let said = assembly_for(Base::Unknown, &[None], &single(), &dual());
        assert_eq!(said.clip_type, None);
        assert_eq!(said.encoders, None);
    }

    /// The measured family still answers when nothing has been picked yet, so a Flux panel is
    /// useful from its first frame rather than only after two files are chosen.
    #[test]
    fn nothing_chosen_yet_falls_back_to_what_the_model_was_measured_as() {
        let said = assembly_for(Base::Flux, &[], &single(), &dual());
        assert_eq!(said.clip_type.as_deref(), Some("flux"));
    }

    /// The complaint this answers: a Flux model said only that it "arrives in parts".
    #[test]
    fn flux_says_how_many_encoders_and_which_family_it_is() {
        let said = assembly_for(Base::Flux, &[], &single(), &dual());
        assert_eq!(said.encoders, Some(2));
        assert_eq!(said.family, "Flux");
        assert_eq!(said.clip_type.as_deref(), Some("flux"));
        assert!(said.says.contains("two text encoders"), "{}", said.says);
    }

    /// **The list belongs to the loader.** Two encoders is a `DualCLIPLoader`, so the family is
    /// looked for in *its* vocabulary — never in `CLIPLoader`'s, which has never heard of `flux`.
    #[test]
    fn the_family_is_looked_for_in_the_list_of_the_loader_that_many_encoders_selects() {
        // The single loader publishes no `flux`, and Flux takes two encoders anyway. If the wrong
        // list were consulted this would find nothing.
        let said = assembly_for(Base::Flux, &[], &dual(), &single());
        assert_eq!(
            said.clip_type, None,
            "the double loader's list was passed as the single one's and must not have matched"
        );
    }

    /// Offered, or unsaid. A server that does not publish the family would refuse it, so
    /// recommending it would send somebody into exactly the refusal this is meant to prevent.
    #[test]
    fn a_family_this_server_does_not_publish_is_left_unsaid() {
        let said = assembly_for(Base::Flux, &[], &single(), &[]);
        assert_eq!(said.clip_type, None);
        // And the rest is still said: what the family is loaded with does not depend on ComfyUI.
        assert_eq!(said.encoders, Some(2));
    }

    /// Three encoders is a `TripleCLIPLoader`, which has no family input at all.
    #[test]
    fn three_encoders_take_no_family() {
        let said = assembly_for(Base::Sd3, &[], &single(), &dual());
        assert_eq!(said.encoders, Some(3));
        assert_eq!(said.clip_type, None);
    }

    /// A family with no account still gets the part of the answer that is known.
    ///
    /// **This used to return nothing at all**, and a Z-Image — measured as a diffusion model and
    /// not as a family — got a panel headed "THIS MODEL ARRIVES IN PARTS" followed by four empty
    /// dropdowns with no hint of what went in them. The count is genuinely unknown, so it stays
    /// `None`; that a bare diffusion model needs an encoder and a VAE is not a guess, it is what
    /// `Kind::DiffusionModel` means.
    #[test]
    fn a_family_with_no_account_says_what_is_known_and_no_more() {
        for family in [Base::Unknown, Base::Sdxl, Base::Sd15, Base::Sd2] {
            let said = assembly_for(family, &[], &single(), &dual());
            assert_eq!(
                said.encoders, None,
                "{family:?} invented a number of encoders"
            );
            assert_eq!(
                said.clip_type, None,
                "{family:?} recommended a family name it cannot know"
            );
            assert!(
                said.says.contains("at least one"),
                "{family:?} said nothing usable: {}",
                said.says
            );
        }
    }

    /// A prompt is code only a model executes, and this is text only a person reads — but the
    /// same blindness applies: nothing else in the suite would notice a mangled continuation.

    #[test]
    fn the_sentences_are_sentences() {
        for family in [Base::Flux, Base::Sd3, Base::Unknown] {
            let said = assembly_for(family, &[], &single(), &dual());
            assert!(
                !said.says.contains("  "),
                "{family:?} carries collapsed indentation: {:?}",
                said.says
            );
        }
    }

    #[test]
    fn the_panel_offers_the_sizes_people_name_out_loud_and_marks_only_the_trained_ones() {
        use epoch_engine::assets::asset::Base;

        // Three ratios were the whole offer, which quietly decided that nobody wants a wallpaper.
        let sdxl = shapes_for(Base::Sdxl);
        let named = |want: &str| sdxl.iter().find(|it| it.label == want).cloned();

        let fhd = named("Full HD").expect("Full HD is offered");
        assert_eq!((fhd.width, fhd.height), (1920, 1080));
        let tall = named("Full HD portrait").expect("and the other way up");
        assert_eq!((tall.width, tall.height), (1080, 1920));
        assert_eq!(
            named("4K UHD").map(|it| (it.width, it.height)),
            Some((3840, 2160))
        );

        // **Marked, never fenced off.** A screen resolution does not claim to be a size the
        // family was trained at, and the family's own sizes do — which is what stops the extra
        // choice becoming a trap. Asking Flux for 4K gives an out-of-memory; that is true, and
        // it is not Epoch's call to make for somebody who owns the card (ADR-0033).
        assert!(!fhd.native, "1920x1080 is not what SDXL was trained at");
        assert!(named("1:1").expect("its own square").native);
        assert!(
            named("1024 square").expect("offered").native,
            "SDXL's square is its own"
        );

        // And a family trained smaller says so about a different square.
        let sd15 = shapes_for(Base::Sd15);
        assert!(sd15.iter().any(|it| it.label == "512 square" && it.native));
        assert!(sd15
            .iter()
            .any(|it| it.label == "1024 square" && !it.native));

        // Every size the sampler is handed is a multiple of eight.
        for shape in sdxl.iter().chain(sd15.iter()) {
            assert_eq!(
                (shape.width % 8, shape.height % 8),
                (0, 0),
                "{} is not a multiple of eight",
                shape.label
            );
        }
    }
}

impl World {
    /// What this machine can draw with, and what it is missing.
    ///
    /// One call rather than three, because a screen showing Styles without their workflows shows
    /// six words and no information — and asking ComfyUI whether it is up is the same round trip
    /// either way.
    pub fn image_shelf(&self) -> ImagesView {
        let vault = vault_dir();
        let images = epoch_engine::images::Images::load(&vault);
        let studio =
            epoch_engine::models::studio::look_for(epoch_engine::models::studio::Studio::ComfyUi);

        ImagesView {
            styles: images
                .styles
                .iter()
                .map(|style| StyleView {
                    name: style.name.clone(),
                    // The workflows attached, in order — resolved to names, because an id is
                    // Epoch's business and a name is the person's.
                    using: style
                        .workflows
                        .iter()
                        .filter_map(|id| {
                            images
                                .workflows
                                .iter()
                                .find(|kept| &kept.id == id)
                                .map(|kept| kept.name.clone())
                        })
                        .collect(),
                    lit: images.serving(&style.name).is_some(),
                })
                .collect(),
            workflows: images
                .workflows
                .iter()
                .map(|kept| WorkflowView {
                    id: kept.id.clone(),
                    name: kept.name.clone(),
                    // Derived at import from the graph itself, never declared.
                    can: kept
                        .opening
                        .as_ref()
                        .map(|opening| {
                            let mut can = Vec::new();
                            if opening.can.from_words {
                                can.push("words".to_owned());
                            }
                            if opening.can.from_image {
                                can.push("a reference".to_owned());
                            }
                            if opening.can.inpaint {
                                can.push("inpaint".to_owned());
                            }
                            if opening.can.upscale {
                                can.push("upscale".to_owned());
                            }
                            can
                        })
                        .unwrap_or_default(),
                    // What it needs installed. The Workshop is where the missing ones come from.
                    needs: kept
                        .opening
                        .as_ref()
                        .map(|opening| opening.needs.clone())
                        .unwrap_or_default(),
                    problem: kept.problem.clone(),
                })
                .collect(),
            usually: images.usually.clone(),
            // Three facts about the studio, the same three the runtimes panel draws.
            installed: studio.installed,
            serving: studio.serving,
            benches: benches_now(&images, &studio),
            draw_on: images.draw_on.clone(),
            library: library_now(),
            problem: images.problem.clone(),
        }
    }

    /// Run the whole picture chain once, and report every link.
    ///
    /// ## Why a button rather than a test
    ///
    /// Every failure mode here is **silent**. A ComfyUI that is serving and holds no checkpoint
    /// looks identical to a working one until somebody asks for a picture; a workflow that
    /// compiled at import can stop compiling when its ComfyUI gains or loses a node; a graph
    /// with no `SaveImage` runs perfectly and produces nothing. None of that is visible on the
    /// deck, and all of it surfaces halfway through a Quest as the character apologising.
    ///
    /// So the chain is walked deliberately, and it is walked **through the real path** — the
    /// same [`Painter`] a character draws with, with a fixed description. A test that used its
    /// own shortcut would prove the shortcut works.
    ///
    /// ## It stops at the first broken link
    ///
    /// Everything after a break is noise: a compile cannot be judged against a server that is
    /// not answering. The last line is always the one to act on, and it carries the failure in
    /// **ComfyUI's own words** — it names the node, the input and the value it refused, which is
    /// the difference between a message somebody can fix and one that says something went wrong.
    pub fn test_studio(&self) -> Vec<StudioCheck> {
        use epoch_engine::capabilities::draw::Easel as _;

        let mut walked = Vec::new();

        // **The walk measures the machine that will draw**, which stopped being this one when a
        // World could choose (ADR-0029). Asking here and drawing there would report four green
        // links about a ComfyUI no picture goes near — the lying gauge the Launcher's whole
        // rule exists to prevent, in its most convincing form, because every reading would be
        // true about *something*.
        let images = epoch_engine::images::Images::load(&vault_dir());
        let studio = match studio_that_draws(&images) {
            Ok(seen) => seen,
            Err(why) => {
                walked.push(StudioCheck::bad("ComfyUI is answering", why));
                return walked;
            }
        };

        if !studio.serving {
            walked.push(StudioCheck::bad(
                "ComfyUI is answering",
                if studio.installed {
                    // The ordinary state, and the one that reads as broken. Measured: the
                    // desktop application opens on its dashboard and the server starts with an
                    // instance, so *reinstall it* would be the wrong fix.
                    "Not answering. It is on this machine — its server starts when you open an instance inside it, not when the application opens."
                        .to_owned()
                } else {
                    format!("Not on this machine. Install it with: {}", studio.install)
                },
            ));
            return walked;
        }
        walked.push(StudioCheck::good("ComfyUI is answering", studio.at.clone()));

        if studio.models.is_empty() {
            // **The link that would otherwise cost an afternoon.** Serving with nothing is a
            // machine running perfectly that cannot draw, and it is what this one was found
            // doing on 2026-08-22 with 11 GB of checkpoints sitting in its own model folder.
            walked.push(StudioCheck::bad(
                "It has something to load",
                "It reports no checkpoint at all. If you have downloaded one, its folder listing is stale — close that instance and open it again."
                    .to_owned(),
            ));
            return walked;
        }
        walked.push(StudioCheck::good(
            "It has something to load",
            studio.models.join(", "),
        ));

        let painter = Painter::anywhere();
        let lit = painter.styles();
        let Some(style) = lit.first().cloned() else {
            walked.push(StudioCheck::bad(
                "A Style has a workflow",
                "Nothing is attached yet. Import a workflow above, then press ATTACH on a Style."
                    .to_owned(),
            ));
            return walked;
        };
        walked.push(StudioCheck::good(
            "A Style has a workflow",
            format!("testing {style}"),
        ));

        // The real request a character makes, with the cheapest honest settings: the workflow's
        // own size, because overriding it would test a size nobody uses, and `quick`, because
        // this is a question about the chain and not about the picture.
        let asked = epoch_engine::capabilities::draw::Asked {
            describe: "a red circle on a plain white background".to_owned(),
            style: Some(style),
            shape: epoch_engine::capabilities::draw::Shape::Keep,
            detail: epoch_engine::capabilities::draw::Detail::Quick,
            from_image: None,
        };
        match painter.draw(&asked) {
            Ok(drawn) => walked.push(StudioCheck::good(
                "It drew, and the picture was kept",
                format!("{} in {:.1}s", drawn.file, drawn.seconds),
            )),
            // Whole, never summarised. ComfyUI says which node and which value it refused.
            Err(why) => walked.push(StudioCheck::bad("It drew, and the picture was kept", why)),
        }
        walked
    }

    /// Take a workflow in, from bytes the window read out of a file input.
    ///
    /// **The frontend never touches the filesystem** (ADR-0024): what crosses is base64, and only
    /// the Engine writes. The name is what the person will see; the file Epoch writes is named
    /// from the id it makes.
    pub fn import_workflow(&self, name: &str, base64_data: &str) -> Result<String, String> {
        let source = epoch_engine::images::from_base64(base64_data)?;

        // Compiled against the ComfyUI that is here — which is what makes a missing custom node
        // a sentence at import rather than a failure mid-Quest. Without a server there is no
        // schema, and an API-format workflow still reads; a UI one is kept with its reason.
        let schema = Painter::serving()
            .map(|(_, schema)| schema)
            .unwrap_or_default();

        let vault = vault_dir();
        let mut images = epoch_engine::images::Images::load(&vault);
        let id = images.take_in(&vault, name.trim(), &source, &schema)?;
        images.save(&vault)?;
        Ok(id)
    }

    /// Build a plain workflow from what this ComfyUI reports, and attach it.
    ///
    /// ## The seven steps this replaces
    ///
    /// Open ComfyUI · find a workflow that works · check its models are downloaded · File →
    /// Export · choose a folder · IMPORT A WORKFLOW… · ATTACH. Somebody who has never opened
    /// ComfyUI could not draw at all until they had learned ComfyUI, which is the opposite of
    /// the promise. Measured on the owner's own machine: they could not get through it.
    ///
    /// ## It is still not something that came in the box
    ///
    /// No `.json` ships. The graph is written here against the checkpoint **this server listed a
    /// moment ago**, compiled against **this server's schema**, and goes in through the ordinary
    /// import — so it is an ordinary workflow afterwards, with the same FORGET and the same
    /// recompile. A packaged file would name a checkpoint the user may not have and fail with
    /// nothing to fix.
    ///
    /// ## Attached, unlike an import
    ///
    /// Importing attaches nothing on purpose: a file called *SNES Pixel Art* is *probably* pixel
    /// art, and *probably* is not a thing to act on. There is no guess here — the plain graph is
    /// what **General** means — so it is attached to General and to nothing else. Asking for
    /// `pixel art` still refuses and still says so, because this cannot draw pixel art on
    /// purpose and claiming otherwise is the substitution the whole design refuses.
    pub fn build_workflow(&self) -> Result<String, String> {
        let studio =
            epoch_engine::models::studio::look_for(epoch_engine::models::studio::Studio::ComfyUi);
        let checkpoint = studio.models.first().ok_or_else(|| {
            if studio.serving {
                // The state this machine was actually in. Naming it beats a generic failure.
                "ComfyUI is answering and reports no checkpoint, so there is nothing to build a workflow around. Download one in ComfyUI first."
                    .to_owned()
            } else {
                "ComfyUI is not answering. Press START COMFYUI above, then open an instance inside it."
                    .to_owned()
            }
        })?;

        let (_, schema) = Painter::serving()
            .ok_or("ComfyUI stopped answering between one question and the next.")?;
        let source = epoch_engine::assets::workflow::starter(checkpoint, &schema)
            .map_err(|why| why.to_string())?;

        // The size is in the name because it is a guess from the checkpoint's name, and a guess
        // somebody can see is a guess somebody can correct.
        let short = checkpoint
            .rsplit([char::from(47), char::from(92)])
            .next()
            .unwrap_or(checkpoint)
            .trim_end_matches(".safetensors")
            .trim_end_matches(".ckpt");
        let name = format!("Basic - {short}");

        let vault = vault_dir();
        let mut images = epoch_engine::images::Images::load(&vault);
        let id = images.take_in(&vault, &name, &source, &schema)?;

        // `General` is the one Style Epoch may attach to on its own: it is the absence of a
        // style rather than a claim about how this graph draws (ADR-0030's second amendment).
        images.attach(epoch_engine::images::GENERAL, &id, true)?;
        images.save(&vault)?;
        Ok(name)
    }

    /// Point a Style at a workflow, or take it away.
    ///
    /// Attaching is what brings a Style into existence, and detaching the last workflow takes
    /// it away again (ADR-0030's second amendment). Nothing is attached automatically at import:
    /// a workflow called *SNES Pixel Art* is probably pixel art and *probably* is not a thing to
    /// act on — it would silently decide what somebody's `pixel art` means.
    pub fn attach_workflow(&self, style: &str, id: &str, attach: bool) -> Result<(), String> {
        let vault = vault_dir();
        let mut images = epoch_engine::images::Images::load(&vault);
        images.attach(style, id, attach)?;
        images.save(&vault)
    }

    /// Forget a workflow, and take it out of every Style that pointed at it.
    ///
    /// Leaving a dangling id would make a Style that looks attached and draws nothing, which is
    /// the one state this whole design exists to avoid.
    pub fn forget_workflow(&self, id: &str) -> Result<(), String> {
        let vault = vault_dir();
        let mut images = epoch_engine::images::Images::load(&vault);
        let Some(at) = images.workflows.iter().position(|kept| kept.id == id) else {
            return Ok(());
        };
        let kept = images.workflows.remove(at);
        for style in &mut images.styles {
            style.workflows.retain(|held| held != id);
        }
        let _ = std::fs::remove_file(epoch_engine::images::workflows_dir(&vault).join(&kept.file));
        images.save(&vault)
    }

    /// Which machine this World draws on. Empty is this one.
    ///
    /// Stored rather than resolved per picture, and **never reinterpreted**: a machine that
    /// stops serving is reported at the moment somebody asks for a picture, not quietly swapped
    /// for one that works. Drawing somewhere the user did not choose is a different answer to a
    /// different question — the same rule that governs Styles one layer up.
    pub fn draw_on(&self, machine: &str) -> Result<(), String> {
        let vault = vault_dir();
        let mut images = epoch_engine::images::Images::load(&vault);
        let wanted = machine.trim();
        if !wanted.is_empty() {
            let pairings = epoch_engine::Pairings::load(&vault);
            let known = pairings
                .all()
                .iter()
                .any(|paired| paired.id == wanted && paired.may(epoch_engine::Grant::Compute));
            if !known {
                return Err(format!(
                    "no machine paired for computation is called '{wanted}'"
                ));
            }
        }
        images.draw_on = wanted.to_owned();
        images.save(&vault)
    }

    /// Which Style this World draws with when nobody says.
    pub fn draw_usually(&self, style: &str) -> Result<(), String> {
        let vault = vault_dir();
        let mut images = epoch_engine::images::Images::load(&vault);
        if !images
            .styles
            .iter()
            .any(|known| known.name.eq_ignore_ascii_case(style.trim()))
        {
            return Err(format!("no Style here is called '{style}'"));
        }
        images.usually = style.trim().to_owned();
        images.save(&vault)
    }

    /// What the Studio Panel offers, for one chosen model (ADR-0033).
    ///
    /// **Asked of the machine that will actually draw**, which is the defect this repeats no
    /// more: a deck once measured the local ComfyUI while a lent one did the drawing, and every
    /// reading was true about something.
    ///
    /// Compatibility is decided here rather than in the window, so there is exactly one rule.
    /// `unknown` on either side is **not** an incompatibility: it is the absence of a
    /// measurement, and refusing on the strength of one nobody took is the same failure as a
    /// gauge nobody can explain.
    pub fn studio_panel(&self, checkpoint: &str, chosen_clip: &[String]) -> PanelView {
        use epoch_engine::assets::asset::Base;
        use epoch_engine::models::generative::Shelf;

        let images = epoch_engine::images::Images::load(&vault_dir());

        // **This function starts nothing, and the panel does** (2026-08-29, the owner's call,
        // reversing his own of the day before).
        //
        // Starting the studio was removed from here on 2026-08-28 because opening a *form* spun
        // up a process and a terminal window for somebody who may look and close it again. The
        // cost was real and it was measured in the wrong place: the twenty gigabytes are what
        // ComfyUI holds *after a render*, and freshly started holding nothing it is 954 MB.
        //
        // What the removal cost was the panel. Most of these lists are files and `names_on`
        // reads the same folders ComfyUI would — but a *node's vocabulary* is not a file, and a
        // model that arrives in parts cannot be drawn without one. So the panel opened with a
        // dead GENERATE and a sentence saying that pressing it once would fix it.
        //
        // The surface asks now, once, on open, through `wake_the_studio` — which is where the
        // rule about *whose* machine it is already lives (ADR-0029). This stays a read: a view
        // builder that started a process would start one for every re-ask, and the panel re-asks
        // every three seconds while it waits.
        //
        // Closing the panel still lets it go. That is the whole of the studio's life.

        // A bridge that will not answer is a reason to say so, not a reason to show nothing: the
        // library half of this panel is still true and still worth looking at.
        let drawing = studio_that_draws(&images);
        // **What is on the shelves, whether or not anything is running.**
        //
        // Only for the local bench: a lent easel's disk is that machine's and the Host cannot
        // see it, so an empty list there stays empty rather than being filled with this
        // machine's files (ADR-0029, and the invented reading the Launcher forbids).
        let here = images.draw_on.trim().is_empty();
        let on_shelf = |shelf| {
            if here {
                epoch_engine::models::generative::names_on(shelf)
            } else {
                Vec::new()
            }
        };
        let unreachable = drawing.as_ref().err().cloned();
        let mut drawing = drawing.unwrap_or(Drawing {
            installed: false,
            serving: false,
            models: Vec::new(),
            diffusion_models: Vec::new(),
            install: String::new(),
            at: "on a machine that did not answer".to_owned(),
            endpoint: None,
        });
        // Read off the shelves when the server has not been asked. Never *instead* of what a
        // running server said — that is the measurement, and this is the stand-in for it.
        if !drawing.serving {
            drawing.models = on_shelf(Shelf::Models);
            drawing.diffusion_models = on_shelf(Shelf::DiffusionModels);
        }
        // Read where the file really is, which for a checkpoint the user already had is
        // ComfyUI's own tree rather than Epoch's library. Reading is not adopting (ADR-0032),
        // and without it a 6.9 GB file sitting right there answered *nobody measured it*.
        let read_of = |shelf, file: &String| {
            epoch_engine::models::generative::file_behind(shelf, file)
                .and_then(|path| epoch_engine::assets::asset::understand(&path).ok())
        };

        let mut models: Vec<PanelModel> = drawing
            .models
            .iter()
            .map(|file| {
                let read = read_of(Shelf::Models, file);
                let family = read.as_ref().map(|it| it.base).unwrap_or(Base::Unknown);
                PanelModel {
                    file: file.clone(),
                    family: family_id(family).to_owned(),
                    bytes: read.as_ref().map(|it| it.bytes),
                    said_base: epoch_engine::models::catalogue::said_base_beside(
                        &epoch_engine::models::generative::file_behind(Shelf::Models, file)
                            .unwrap_or_default(),
                    ),
                    kind: "checkpoint".to_owned(),
                    needs: Vec::new(),
                    with: Vec::new(),
                    makes: makes_id(family),
                    carries_encoder: read.as_ref().is_some_and(|it| it.carries_encoder),
                }
            })
            .collect();

        // **And the diffusion models, which are not checkpoints.** Flux and Z-Image are normally
        // distributed as the diffusion half alone; leaving them out of this list is what made the
        // owner's Flux LoRA unreachable even once a Flux model was on the machine.
        models.extend(
            drawing
                .diffusion_models
                .iter()
                .map(|file| {
                    let read = read_of(Shelf::DiffusionModels, file);
                    let family = read.as_ref().map(|it| it.base).unwrap_or(Base::Unknown);
                    PanelModel {
                        file: file.clone(),
                        family: family_id(family).to_owned(),
                        bytes: read.as_ref().map(|it| it.bytes),
                        said_base: epoch_engine::models::catalogue::said_base_beside(
                            &epoch_engine::models::generative::file_behind(
                                Shelf::DiffusionModels,
                                file,
                            )
                            .unwrap_or_default(),
                        ),
                        kind: "diffusion".to_owned(),
                        // What it still needs is now a question for the panel rather than a
                        // guess made here: a model that arrives in parts needs parts chosen, and
                        // saying *which* was Epoch inferring from filenames.
                        needs: Vec::new(),
                        with: Vec::new(),
                        makes: makes_id(family),
                        // A diffusion model never carries one; it is the shape that arrives in
                        // parts, and the parts are what the panel already asks for.
                        carries_encoder: false,
                    }
                })
                .collect::<Vec<_>>(),
        );

        let chosen = models
            .iter()
            .find(|model| model.file == checkpoint)
            .or_else(|| models.first());
        let family = chosen
            .map(|model| base_named(&model.family))
            .unwrap_or(Base::Unknown);
        // Only a model that arrives in parts has an assembly. A checkpoint carries its own
        // encoder and VAE, so saying what it is "loaded with" would describe a decision nobody
        // makes.
        let in_parts = chosen.is_some_and(|model| model.kind == "diffusion");

        // **Nothing is asked of a server that is not answering** (measured 2026-08-28).
        //
        // One connection attempt to a closed port on this machine takes **21 seconds** — not the
        // instant refusal loopback is supposed to give, and `ask_node`'s own two-second timeout
        // does not cover it. The panel makes nine such calls, so with ComfyUI stopped it took
        // **190 s to open**: a form that was meant to be instant, waiting on nine questions
        // whose answer was already known.
        //
        // `serving` is that answer and it is already in hand. This is the same discipline as the
        // rest of the file — do not pay for a measurement whose result is already measured — and
        // the reason it matters is that the panel's first read happens *before* the studio it
        // just asked for has answered — so *not answering* is the ordinary state of the first
        // few of these, and it is re-asked every three seconds until it is not.
        let ask_at = drawing.serving.then(|| drawing.endpoint.clone()).flatten();
        let ask_at = ask_at.as_deref();

        // **Two nodes, asked once each.** The panel needs three answers and two of them come
        // from `CLIPLoader`; asking it twice cost a second round trip and a second timeout, and
        // the panel took about thirty seconds to open (measured 2026-08-24).
        let clip_loader =
            ask_at.and_then(|at| epoch_engine::models::studio::ask_node(at, "CLIPLoader"));
        // **And the double one, because it speaks a different language.** One more round trip,
        // bought by the picture it stops ComfyUI refusing.
        let dual_loader =
            ask_at.and_then(|at| epoch_engine::models::studio::ask_node(at, "DualCLIPLoader"));
        let vae_loader =
            ask_at.and_then(|at| epoch_engine::models::studio::ask_node(at, "VAELoader"));
        // One more round trip, and the reason it is worth it: a shelf that fills up and feeds
        // nothing is the gap this closes. A server that has never heard of the node answers
        // nothing and the control is simply absent, which is true.
        let upscale_loader =
            ask_at.and_then(|at| epoch_engine::models::studio::ask_node(at, "UpscaleModelLoader"));
        // And the same for the other shelf nothing consumed.
        let controlnet_loader =
            ask_at.and_then(|at| epoch_engine::models::studio::ask_node(at, "ControlNetLoader"));
        // What this ComfyUI can make a control map with. One round trip per name Epoch knows,
        // and Epoch knows one — so a server without it says so by offering nothing, which leaves
        // the person to prepare the picture themselves and say they did.
        let preparations: Vec<String> = epoch_engine::assets::workflow::PREPARATIONS
            .iter()
            .filter(|class| {
                ask_at
                    .and_then(|at| epoch_engine::models::studio::ask_node(at, class))
                    .is_some()
            })
            .map(|class| (*class).to_owned())
            .collect();
        // What this server could make something that moves with, asked the same way as the
        // preparations: one round trip per class Epoch knows, and Epoch knows four. A server
        // without them offers nothing, and the VIDEO tab says so rather than refusing after
        // somebody has filled the form in.
        let every_family = if drawing.serving {
            epoch_engine::assets::workflow::families_where(|class| {
                ask_at
                    .and_then(|at| epoch_engine::models::studio::ask_node(at, class))
                    .is_some()
            })
        } else {
            // **From the models on the shelves, when there is nothing to ask.**
            //
            // The server is the better answer and it is not available now that opening the panel
            // starts nothing — and *nothing to ask* must not become *no*, which is how the VIDEO
            // tab came to be dark on a machine with a video model sitting on it (`CLAUDE.md`:
            // a gate with nothing behind it must stay open).
            //
            // This is a real measurement rather than a softer one: Epoch read `LTXV` out of that
            // file's own tensors. What it cannot know until something is running is whether this
            // ComfyUI has the *nodes* — and `compose` refuses by name at the door for exactly
            // that, before a graph is ever queued.
            // **Both sides of this comparison come from `Base`, and they have to.** A model's
            // `family` is an *id* (`stableaudio`) and a `FAMILIES` key is a *name*
            // (`Stable Audio`); comparing them case-insensitively worked for `ltxv` and
            // `hunyuan3d` — whose id happens to be the name lowercased — and silently failed for
            // the two families whose name carries a separator. So the AUDIO tab was dark on a
            // machine holding both audio checkpoints, saying *this ComfyUI has no audio nodes*
            // about a ComfyUI it had not asked.
            //
            // `base_named` back to a `Base`, then that `Base`'s own name, is one measurement
            // read twice instead of two spellings agreeing by luck.
            epoch_engine::assets::workflow::FAMILIES
                .iter()
                .map(|(name, _)| (*name).to_owned())
                .filter(|name| {
                    models
                        .iter()
                        .any(|model| base_named(&model.family).plainly() == name)
                })
                .collect()
        };
        // Split by what each one makes, so a tab offers only what it can use. Derived from the
        // family table rather than from a second list, which is what keeps the two from
        // disagreeing about where a family belongs.
        let of = |keeping| {
            epoch_engine::assets::workflow::FAMILIES
                .iter()
                .filter(|(name, family)| {
                    family.keeping == keeping && every_family.iter().any(|it| it == name)
                })
                .map(|(name, _)| (*name).to_owned())
                .collect::<Vec<_>>()
        };
        let motions = of(epoch_engine::assets::workflow::Keeping::Video);
        let sounds = of(epoch_engine::assets::workflow::Keeping::Audio);
        let meshes = of(epoch_engine::assets::workflow::Keeping::Mesh);
        // Which of them are told **words that are sung**, so the panel offers a second box for
        // those and for nothing else. A property of the encoder node, read from the same table
        // the tabs are — a control that reaches nothing is worse than a control that is absent.
        let lyrical: Vec<String> = epoch_engine::assets::workflow::FAMILIES
            .iter()
            .filter(|(name, family)| family.lyrical && sounds.iter().any(|it| it == name))
            .map(|(name, _)| (*name).to_owned())
            .collect();
        // Its own endpoint rather than a node, because there is no node. One local round trip.
        let embeddings = ask_at
            .map(epoch_engine::models::studio::embeddings_here)
            .unwrap_or_default();
        // What a node offers — or, when nothing is running to ask, what is on the shelf that
        // node loads from.
        //
        // **The server's answer always wins.** This is the stand-in for a measurement that has
        // not been taken, not a second opinion about one that has: a running ComfyUI knows about
        // files in paths Epoch has never been told about, and it is the thing that will refuse
        // the graph.
        let serving = drawing.serving;
        // What a node said, and nothing else. For the answers no shelf can stand in for.
        let asked = |answer: &Option<serde_json::Value>, node: &str, input: &str| {
            answer
                .as_ref()
                .map(|it| epoch_engine::models::studio::read_offered(it, node, input))
                .unwrap_or_default()
        };
        let read = |answer: &Option<serde_json::Value>, node: &str, input: &str, shelf| {
            let offered = asked(answer, node, input);
            if offered.is_empty() && !serving {
                on_shelf(shelf)
            } else {
                offered
            }
        };

        PanelView {
            // Asked of the server that will draw, like everything else on this deck. A lent
            // machine cannot be asked directly (ADR-0029), so these come back empty and the
            // panel offers a checkpoint or nothing — which is true rather than convenient.
            // **Listed by the server, described by Epoch.** The list is ComfyUI's — a file Epoch
            // has never seen is still offered, with nothing said about it — and the description
            // comes from the bytes when the file is on a shelf Epoch knows.
            encoders: read(&clip_loader, "CLIPLoader", "clip_name", Shelf::TextEncoders)
                .into_iter()
                .map(|file| PanelPart {
                    says: read_of(Shelf::TextEncoders, &file)
                        .and_then(|it| it.encoder)
                        .map(|it| it.plainly().to_owned()),
                    // Not a VAE: nothing here decodes anything, so there is no medium to read.
                    medium: None,
                    file,
                })
                .collect(),
            vaes: read(&vae_loader, "VAELoader", "vae_name", Shelf::Vaes)
                .into_iter()
                // ComfyUI's own placeholder entry, which is not a file. Offering it would offer
                // a choice the server then refuses.
                .filter(|name| name != "pixel_space")
                .map(|file| {
                    // **What it decodes into, measured — and the claim only when nothing was.**
                    //
                    // This used to be the claim alone, on the reasoning that a VAE has no width
                    // to read and the only thing that can be said is what the file said about
                    // itself. The file lies: `z_image_ae.safetensors` declares `Flux.1-AE` and
                    // belongs to Z-Image, so the row said *Flux* beside the wrong file while
                    // MiniMax-H3's audio and video autoencoders sat in the same list saying
                    // nothing at all, offered to somebody drawing a picture.
                    //
                    // A VAE has no *family* and it does have a medium, written into the rank of
                    // every convolution it holds. See `asset::medium_of`.
                    let read = read_of(Shelf::Vaes, &file);
                    // `makes`, not `medium`: one place decides this, and it prefers a measured
                    // family over a part's own shape where a file has both.
                    let medium = read.as_ref().and_then(|it| it.makes());
                    PanelPart {
                        says: medium
                            .map(|it| medium_words(it).to_owned())
                            .or_else(|| read.and_then(|it| it.claimed)),
                        medium: medium.map(|it| medium_id(it).to_owned()),
                        file,
                    }
                })
                .collect(),
            upscalers: read(
                &upscale_loader,
                "UpscaleModelLoader",
                "model_name",
                Shelf::Upscalers,
            )
            .into_iter()
            .map(|file| PanelPart {
                // What the bytes say, where Epoch has seen the file. An upscaler carries no
                // family and no width to read, so this is usually nothing — and nothing is
                // what is shown rather than a guess from the name.
                says: read_of(Shelf::Upscalers, &file).and_then(|it| it.claimed),
                // Not a VAE: nothing here decodes anything, so there is no medium to read.
                medium: None,
                file,
            })
            .collect(),
            controlnets: read(
                &controlnet_loader,
                "ControlNetLoader",
                "control_net_name",
                Shelf::ControlNet,
            )
            .into_iter()
            .map(|file| PanelPart {
                // A ControlNet carries no width and no family Epoch can read, so this is
                // usually nothing — and nothing is shown rather than a guess from a name
                // like `control_v11p_sd15_canny`, which is a claim rather than a fact.
                says: read_of(Shelf::ControlNet, &file).and_then(|it| it.claimed),
                // Not a VAE: nothing here decodes anything, so there is no medium to read.
                medium: None,
                file,
            })
            .collect(),
            preparations,
            motions,
            sounds,
            lyrical,
            meshes,
            serving,
            embeddings,
            // **A node's vocabulary, and there is no shelf behind it.** These are the one thing
            // a stopped server genuinely cannot answer, so they use `asked` and not `read`: an
            // empty list here means *nobody has been asked*, and the panel says that rather than
            // showing an empty dropdown that reads as *this model has no family*.
            //
            // Deliberately not routed through a shelf that happens to hold nothing loadable.
            // A fallback that is empty by accident is one that stops being empty the day
            // somebody drops a file in the wrong folder.
            clip_types: asked(&clip_loader, "CLIPLoader", "type"),
            clip_types_two: asked(&dual_loader, "DualCLIPLoader", "type"),
            assembly: in_parts.then(|| {
                // The encoders the person has actually picked, named by what each one *is* —
                // which is the measurement that decides this, and the one Epoch can always take.
                let picked: Vec<_> = chosen_clip
                    .iter()
                    .map(|file| read_of(Shelf::TextEncoders, file).and_then(|it| it.encoder))
                    .collect();
                assembly_for(
                    family,
                    &picked,
                    &asked(&clip_loader, "CLIPLoader", "type"),
                    &asked(&dual_loader, "DualCLIPLoader", "type"),
                )
            }),
            // **What this model has already been drawn with here.** Read by hash, and only from
            // a hash somebody already paid for — a panel that hashed on open would cost seconds
            // every time it was looked at, to answer a question about advice.
            remembered: chosen
                .and_then(|model| model_hash(&model.file, &model.kind, false))
                .map(|hash| {
                    let library = epoch_engine::models::generative::Library::here();
                    epoch_engine::models::recipes::Recipes::load(library.root())
                        .about(&hash)
                        .into_iter()
                        .map(|kept| PanelRecipe {
                            clip: kept.clip.clone(),
                            clip_type: kept.clip_type.clone(),
                            vae: kept.vae.clone(),
                            drew: kept.outcome == epoch_engine::models::recipes::Outcome::Drew,
                            said: match &kept.outcome {
                                epoch_engine::models::recipes::Outcome::Drew => None,
                                epoch_engine::models::recipes::Outcome::Refused(said) => {
                                    Some(said.clone())
                                }
                            },
                        })
                        .collect()
                })
                .unwrap_or_default(),
            loras: loras_for(
                &epoch_engine::models::generative::search_paths(Shelf::Loras),
                family,
            ),
            shapes: shapes_for(family),
            where_at: drawing.at.clone(),
            problem: if let Some(why) = unreachable {
                Some(why)
            } else if !drawing.installed {
                Some("Nothing that draws is on this machine yet.".to_owned())
            } else if !drawing.serving {
                Some(format!("ComfyUI is not answering {}.", drawing.at))
            } else if models.is_empty() {
                Some(
                    "It is running and reports no model at all. Install one from the library, or \
                     download one through its own Manager."
                        .to_owned(),
                )
            } else {
                None
            },
            models,
        }
    }

    /// The panel closed with nothing drawn. Let the studio go if it is ours and idle.
    ///
    /// The ordinary ending is the other one — a picture is made and [`Self::draw_from_panel`]
    /// releases it there, the moment the drawing is done. This is for the panel that is opened
    /// and closed again, which started a server for a picture nobody asked for in the end.
    pub fn close_studio(&self) -> Option<String> {
        let_studio_go()
    }

    /// Make the picture one [`PanelAsk`] describes.
    ///
    /// The graph is composed here and compiled against the schema of the bench that will run it
    /// — not a shipped `.json`, and not the local server when a lent one is drawing.
    pub fn draw_from_panel(&self, ask: PanelAsk) -> Result<PanelMade, String> {
        // **The card first, the picture second** (ADR-0034, and the owner's own reading of it).
        //
        // This is the path that matters: a character offers the panel before it ever draws, so
        // almost every picture is made from here. It draws on the caller's thread — somebody
        // pressed a button and is waiting for it, which is right — and that is exactly why the
        // model has to let go: nothing ends a turn here, so whatever was left resident sits on
        // the card while ComfyUI needs all of it.
        //
        // Measured on this machine: a 7.5 GB model beside an 11.9 GB checkpoint on a 12 GB card
        // turns a 34 s render into 117 s.
        self.let_the_models_go();

        // **GENERATE is what starts the picture studio** (2026-08-28, the owner's call).
        //
        // Opening the panel used to. It no longer does, so this is the moment: somebody has
        // filled the form in and pressed the button, which is the first point at which a
        // twenty-gigabyte process is something they asked for rather than something that
        // happened to them.
        //
        // It waits, unlike the panel's old call, because there is nothing else to do with the
        // time — the person is waiting for a picture. Measured on this machine at about thirty
        // seconds from start to answering; three minutes is room for a slower one and still a
        // bounded wait.
        wake_studio(std::time::Duration::from_secs(180));

        let vault = vault_dir();
        let images = epoch_engine::images::Images::load(&vault);
        let bench = bench_for(&images)?;
        let schema = bench.schema()?;

        // Kept before the ask is taken apart: what was chosen is the thing being remembered.
        let chosen = ask.clone();

        // Read before `loading` takes the model's parts out of `ask`.
        let control = steering(&ask);
        let beyond = ask.beyond();
        let loading = loading_for(&ask)?;

        let asked = epoch_engine::assets::workflow::Ask {
            loading,
            beyond,
            // Empty means none, which is the graph exactly as it was before an upscale existed.
            upscale: Some(ask.upscale.trim().to_owned()).filter(|it| !it.is_empty()),
            control,
            // Only when there is one, and turned from *how much survives* into ComfyUI's
            // `denoise`, which is how much is overwritten. One place does that arithmetic.
            from: Some(ask.from.trim().to_owned())
                .filter(|it| !it.is_empty())
                .map(|picture| (picture, (1.0 - ask.keep).clamp(0.0, 1.0))),
            // Flux's own dial; a zero from a surface that did not send one becomes its default
            // rather than no guidance at all.
            guidance: if ask.guidance <= 0.0 {
                3.5
            } else {
                ask.guidance.clamp(0.0, 20.0)
            },
            loras: ask.loras,
            prompt: ask.prompt,
            negative: ask.negative,
            width: ask.width.clamp(64, 4096),
            height: ask.height.clamp(64, 4096),
            steps: ask.steps.clamp(1, 200),
            cfg: ask.cfg.clamp(0.0, 30.0),
            // Zero means *a different picture every time*, which is what somebody who has not
            // touched the seed field wants. A pinned zero would repeat one picture forever.
            seed: if ask.seed == 0 {
                seed_now() as i64
            } else {
                ask.seed
            },
            batch: ask.batch.clamp(1, 8),
        };
        // **Unless it is built from a picture**, which reaches no text encoder at all: the
        // prompt would have nowhere to go, and refusing over it is refusing a graph that is
        // complete.
        let words_matter = !matches!(
            &asked.beyond,
            Some(epoch_engine::assets::workflow::Beyond::Shape {
                from: epoch_engine::assets::workflow::From3d::Pictures(_),
                ..
            })
        );
        if words_matter && asked.prompt.trim().is_empty() {
            return Err("there is nothing to draw — the prompt is empty".to_owned());
        }

        let graph = epoch_engine::assets::workflow::compose(&asked, &schema)
            .map_err(|why| format!("this ComfyUI cannot run that: {why}"))?;

        // **Something that moves outlives the button that asked for it** (ADR-0034).
        //
        // A picture is drawn on this thread because it takes seconds and the person is standing
        // there — measured, 12 to 35 s on this machine. A video is the case ADR-0034 was written
        // for and the first real producer of a Job: it runs on its own thread, the character
        // shows as *waiting* in the World while it does, and the evidence lands on the Quest that
        // asked rather than on whichever one is open when it finishes.
        //
        // **Composed first, deliberately.** Everything that can be refused — a missing node, an
        // unknown family, an empty prompt — is refused on this thread, where it is an answer to
        // a button press. A refusal that arrived minutes later as a message from a character
        // would be the same defect `draw_image` had, where a `refusal()` written to run
        // asynchronously turned *you asked for a style nobody has* into silence.
        if let Some(beyond) = &asked.beyond {
            // **What it is, in the words a person reads.** It goes on the crew card and into the
            // Chronicle, so *making a video* on a sound is a small lie in two places at once.
            let what = match beyond {
                epoch_engine::assets::workflow::Beyond::Moving { .. } => "making a video",
                epoch_engine::assets::workflow::Beyond::Sound { .. } => "making a sound",
                epoch_engine::assets::workflow::Beyond::Shape { .. } => "making a model",
            };
            return self.begin_long_work(bench, graph, chosen, vault, what);
        }

        let drawn = bench.draw_graph(&graph);

        // **The measurement the panel could not take, taken for free.** Epoch reads a checkpoint's
        // family for six families and will never cover an open set — but a graph that ran is a
        // fact about this exact model, and it already happened (ADR-0032's amendment).
        match &drawn {
            Ok(_) => {
                remember_recipe(&chosen, epoch_engine::models::recipes::Outcome::Drew);
                // **And that somebody has now chosen.** `Studio::chosen()` reads this, and it is
                // what stops `draw_image` offering the panel again — ADR-0033's rule is *ask
                // before assuming twelve decisions*, not *ask forever*.
                //
                // It was missing, and the effect was total rather than cosmetic: 11.28 moved
                // GENERATE from handing the prompt to the character over to drawing directly, and
                // the recording of the choice stayed behind with the old path. Nothing in the UI
                // calls `choose_recipe` either, so the flag could never become true — measured by
                // asking a character to draw and watching it open the panel for the third time.
                remember_choice(chosen.clone());
            }
            Err(said) => {
                // Only a refusal about the configuration. An out-of-memory is the card that day
                // and would be false the next time nothing else is resident.
                if epoch_engine::models::recipes::is_about_the_configuration(said) {
                    remember_recipe(
                        &chosen,
                        epoch_engine::models::recipes::Outcome::Refused(said.clone()),
                    );
                }
            }
        }

        let (bytes, _name, seconds) = drawn?;
        // **Where this World works, as well as in the vault.** The person pressed GENERATE, so
        // they are told where it landed — unlike the capability's result, which a model reads.
        let world = self.lock().world_id.clone();
        let kept = epoch_engine::import::keep_made(
            &vault,
            where_the_work_is(world.as_ref()).as_deref(),
            &bytes,
        )
        .map_err(|why| format!("the picture was drawn and could not be kept: {why}"))?;
        self.file_as_evidence(&kept.file, seconds);

        // **The picture exists, so the studio can go.** This is the order the owner asked for:
        // open the panel, it starts; press GENERATE, the panel closes; the picture is made; the
        // studio stops. Released here rather than when the panel unmounts, because the panel
        // closes the moment GENERATE is pressed and the render is still running then — a stop
        // at that point would kill the thing it was waiting for.
        let _ = let_studio_go();
        // **And the brain comes back**, which is the other half of the mechanic the owner asked
        // for: the model gets out of the way for the render and returns when the card is free,
        // so the next message does not pay the load.
        //
        // On its own thread, always: reading a model back takes 25 s here, and the person who
        // pressed GENERATE is waiting for a picture rather than for a model.
        self.warm_the_crew_back();
        Ok(PanelMade::Drew(PanelDrawn {
            file: kept.file,
            // **Plainly, because it is being handed to a person.** `canonicalize` returns
            // Windows' extended-length form — `\\?\C:\Users\…` — which is correct, is
            // what lets a path exceed 260 characters, and is not what anybody wants to read
            // or paste. The rule and the function already existed for the Library, written
            // the day Obsidian answered *Vault not found* for a vault sitting right there.
            at: kept
                .beside_the_work
                .map(|at| epoch_engine::library::plainly(&at.display().to_string())),
            seconds,
        }))
    }

    /// Whose model to put back on the card when the drawing is done, if anybody's.
    ///
    /// `None` for an agent brain (there is no model of the user's), for a hosted one (there is
    /// nothing of theirs to hold), and for a character whose KEEP is off — which is somebody
    /// saying *let it go*, and putting it back would be the switch not working.
    fn brain_to_warm(&self, who: &epoch_kernel::CharacterId) -> Option<(String, String)> {
        let inner = self.lock();
        if inner.keep_loaded_for(who) == epoch_engine::KeepLoaded::Never {
            return None;
        }
        let mind = inner.registry.character(who)?.mind.clone()?;
        Some((mind.provider()?.to_owned(), mind.model().to_owned()))
    }

    /// Put the crew's brains back after a picture, on a thread of their own.
    ///
    /// Only whoever is talking here — not every character, which would load several models to
    /// tidy a symmetry nobody asked for and is the exact cost `concurrentCrew` exists to refuse.
    fn warm_the_crew_back(&self) {
        let who = {
            let inner = self.lock();
            inner.world_id.clone().and_then(|world| {
                inner
                    .quests
                    .active(&world)
                    .map(|q| q.inaugurated_by.clone())
            })
        };
        let Some(who) = who else { return };
        let brain = self.brain_to_warm(&who);
        let providers = std::sync::Arc::clone(&self.providers);
        std::thread::spawn(move || warm_back(&providers, brain.as_ref()));
    }

    /// Start a video, and answer that it started.
    ///
    /// ## Why the thread does not touch `self`
    ///
    /// It cannot: `Capability::run` has the same constraint and for the same reason (ADR-0008).
    /// What lands is *an id and how it ended* — the half that knows whose Quest it was files it,
    /// on the heartbeat, from `collect_finished_work`. So this thread needs the bench, the graph,
    /// the vault and where the work is, all read before it starts, and nothing else.
    ///
    /// That is also what makes the Quest right. `Jobs::begin` takes the Quest and the character
    /// **now**, and a four-minute render is exactly the case where the open conversation has
    /// moved on by the time it lands — the defect ADR-0025 recorded, arriving as the normal case
    /// rather than as a race.
    fn begin_long_work(
        &self,
        bench: Bench,
        graph: serde_json::Value,
        chosen: PanelAsk,
        vault: std::path::PathBuf,
        what: &'static str,
    ) -> Result<PanelMade, String> {
        let waiting_on = bench.where_at();
        let (quest, character) = {
            let inner = self.lock();
            let world = inner
                .world_id
                .clone()
                .ok_or("there is no World open to make this for")?;
            let quest = inner
                .quests
                .active(&world)
                .ok_or("there is no conversation for this to belong to")?;
            (quest.id.clone(), quest.inaugurated_by.clone())
        };

        let id = format!("work-{:016x}", seed_now());
        let job = self
            .lock()
            .jobs
            .begin(&id, &quest, &character, what, &waiting_on)?;

        // The World says so, because it is true: this character asked for something and is
        // waiting on a machine. Idle would be a lie and *working* would be a different one —
        // ComfyUI is working, and they are not (`simulation::Effort::Waiting`).
        if let Some(instance) = self.lock().simulation.get_mut(&character) {
            instance.began_waiting(format!("waiting on {waiting_on}"));
        }

        let root = where_the_work_is(self.lock().world_id.as_ref());
        // Read now, on this thread: the character's brain and whether they hold it. Reading it
        // inside the spawned one would need the World, and nothing reached from a thread that
        // outlives a turn may take that lock (`CLAUDE.md`, 11.18).
        let brain = self.brain_to_warm(&character);
        let providers = std::sync::Arc::clone(&self.providers);
        std::thread::spawn(move || {
            let landed = match bench.draw_graph(&graph) {
                Err(why) => {
                    if epoch_engine::models::recipes::is_about_the_configuration(&why) {
                        remember_recipe(
                            &chosen,
                            epoch_engine::models::recipes::Outcome::Refused(why.clone()),
                        );
                    }
                    epoch_engine::jobs::State::Failed(why)
                }
                Ok((bytes, _name, seconds)) => {
                    remember_recipe(&chosen, epoch_engine::models::recipes::Outcome::Drew);
                    remember_choice(chosen.clone());
                    // `making a video` -> `a video`. One string, so the card and the Chronicle
                    // cannot come to disagree about what was made.
                    let it = what.trim_start_matches("making ");
                    match epoch_engine::import::keep_made(&vault, root.as_deref(), &bytes) {
                        Err(why) => epoch_engine::jobs::State::Failed(format!(
                            "{it} was made and could not be kept: {why}"
                        )),
                        Ok(kept) => {
                            // **A mesh cannot be shown, so it is turned into something that can.**
                            // Named after the mesh (`<stem>.gif`), which is what lets the Chronicle
                            // find it without a second field on the artifact — see `turntable`.
                            //
                            // On this thread, after the mesh is safely kept: a preview that fails is
                            // a row without a picture, never a model that was made and lost.
                            if kept.file.to_ascii_lowercase().ends_with(".glb") {
                                let folder = epoch_engine::import::shared_images(&vault);
                                let _ =
                                    super::turntable::preview(&folder.join(&kept.file), &folder);
                            }
                            epoch_engine::jobs::State::Made {
                                made: epoch_engine::capability::Made {
                                    reference: kept.file,
                                    summary: format!("Made {it} from the panel in {seconds:.1}s."),
                                },
                                // **Two sentences, deliberately different.** A landed job writes
                                // both an `Answered` and a `Produced` (`file_against`), and the
                                // Chronicle prints both — so the same words twice reads as a
                                // stutter. Measured in the window before this was split: *"Made a
                                // video in 55.2s. Made a video in 55.2s."*
                                //
                                // This one is the character saying it arrived; the summary above is
                                // the evidence label. **No filename in either**: a result that names
                                // the file it wrote is what let a model forge a Markdown image once
                                // already (ADR-0030's third amendment).
                                said: format!("{it} is ready.", it = capitalised(it)),
                            }
                        }
                    }
                }
            };
            // The studio can go now, whichever way it ended. Here rather than in the caller,
            // because the caller returned minutes ago.
            let _ = let_studio_go();
            epoch_engine::jobs::land(&id, landed);
            // This thread *is* the background thread, so the model is read back on it — there is
            // nobody waiting on it and the next thing to happen is somebody typing.
            warm_back(&providers, brain.as_ref());
        });

        Ok(PanelMade::Began {
            what: job.what,
            waiting_on: job.waiting_on,
        })
    }
}
