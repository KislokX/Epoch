//! The Generative Library and the two catalogues that fill it (ADR-0031, ADR-0032).
//!
//! Split out of `state.rs` unchanged. What is on the shelves, what a search found, what fits on
//! this machine, and installing it — never what draws with it.

use super::*;

/// The library, and what ComfyUI knows about it.
///
/// **The handshake runs here, when somebody opens the panel.** It compares before it writes
/// (`generative::offer_library_to_comfyui`), so opening the panel twice writes once and asks for
/// one restart, ever — which is what makes running it on a screen rather than at startup honest
/// rather than a side effect hiding in a getter.
///
/// Every file is read from its own bytes, never its name (ADR-0032). That costs a header per
/// file — a few hundred kilobytes against a 6.9 GB checkpoint — so a full shelf reads in the time
/// the panel takes to open.
pub(crate) fn library_now() -> LibraryView {
    use epoch_engine::models::generative::{self, Library, Link, Shelf};

    let library = Library::here();
    let handshake = generative::offer_library_to_comfyui(&library);

    let (note, restart) = match &handshake.link {
        Link::Current => ("ComfyUI has been told where this is.".to_owned(), false),
        Link::Written => (
            "ComfyUI has just been told where this is. Restart it once and everything here becomes visible to it — after that, anything that arrives shows up without a restart."
                .to_owned(),
            true,
        ),
        Link::NoStudio => (
            "No ComfyUI was found on this machine, so nothing has been told about this yet."
                .to_owned(),
            false,
        ),
        Link::Refused(why) => (
            format!("ComfyUI was found and its config could not be written: {why}"),
            false,
        ),
    };

    LibraryView {
        root: library.root().display().to_string(),
        note,
        restart,
        studio_at: handshake.studio_at.clone(),
        shelves: Shelf::ALL
            .into_iter()
            .map(|shelf| ShelfView {
                id: shelf.folder().to_owned(),
                name: shelf_name(shelf).to_owned(),
                held: held_on(&library.shelf(shelf)),
            })
            .collect(),
    }
}

/// What a shelf is called on a screen.
///
/// Here rather than on `Shelf`, because `epoch-models` answers what a machine holds and a
/// capitalised English label is this surface's business (ADR-0003).
pub(crate) fn shelf_name(shelf: epoch_engine::models::generative::Shelf) -> &'static str {
    use epoch_engine::models::generative::Shelf;
    match shelf {
        Shelf::Models => "MODELS",
        Shelf::DiffusionModels => "DIFFUSION MODELS",
        Shelf::TextEncoders => "TEXT ENCODERS",
        Shelf::Loras => "LORAS",
        Shelf::Vaes => "VAES",
        Shelf::Upscalers => "UPSCALERS",
        Shelf::ControlNet => "CONTROLNET",
        Shelf::Embeddings => "EMBEDDINGS",
        Shelf::Workflows => "WORKFLOWS",
        Shelf::Voices => "VOICES",
        Shelf::Timbres => "TIMBRES",
    }
}

/// Everything on one shelf — **the shared reading**, not a second one.
///
/// This used to live here, and EpochServices needed the same list off the same disk. Two copies
/// of *what is this file* agree until one of them learns something: `medium` was added here on
/// 2026-08-30 and would have been the first thing to diverge. `epoch-models` is the crate both
/// surfaces share (ADR-0029 forbids EpochServices from linking the Engine), so the reading lives
/// there and this is the name the Host already calls it by.
pub(crate) use epoch_engine::models::generative::held_on;

/// One thing a catalogue holds, as a shelf shows it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetView {
    pub id: String,
    pub name: String,
    /// Which site answered. **For a person to recognise**, and it is not what a download is
    /// routed by.
    pub source: String,
    /// Which reader to ask when this row is installed.
    ///
    /// ## Why this is not `source`
    ///
    /// Two shelves read Hugging Face and ask it different questions: CREATION asks *what
    /// repositories exist*, VOICES asks *what is inside one*. Both answer `Hugging Face`, because
    /// that is the true answer to *who published this* — and routing an install on it would send
    /// a voice to the reader that cannot find one.
    ///
    /// The same separation as an agent account's id and its kind: **one identifies, the other
    /// classifies**, and nothing may take the first apart to recover the second.
    pub catalogue: String,
    pub kind: String,
    /// What the source called the base — `Krea 2`, `Pony`, `Flux.1 D`. Verbatim.
    pub said_base: String,
    /// What Epoch can build with. `unknown` is unmeasured, never incompatible.
    pub family: String,
    pub by: Option<String>,
    pub downloads: u64,
    pub adult: bool,
    pub bytes: u64,
    pub triggers: Vec<String>,
    pub page: String,
    /// **A token, never the address.** `epoch://preview/<token>` is what a row asks for, and the
    /// Engine decides what that resolves to — so the window never holds a URL to a third party
    /// and `img-src` is not widened by a single host (ADR-0024 §2b).
    ///
    /// `None` where the source published no picture, which is most of Hugging Face.
    pub preview: Option<String>,
    /// **A recording of what this sounds like**, as a token — `epoch://sample-<token>`.
    ///
    /// A token and not the address, for the reason `preview` is one: the window talks to Epoch
    /// and to nothing else. `None` where the source published none, which is a play button that
    /// is simply absent rather than one that does nothing.
    pub sample: Option<String>,
    /// **Which medium the source placed it in** — `picture`, `video`, `sound`, `model`.
    ///
    /// A weaker reading than a shelf's, and the surface says so: a file on a shelf is read from
    /// its own bytes, and this has not been downloaded. `None` is *unplaced*, never *not this
    /// one*, and stays visible under every medium.
    pub makes: Option<String>,
    /// Downloading needs the user's own key and there is none stored. Said before it is pressed.
    pub needs_key: bool,
    /// Every version this was published in, when there is more than one to choose between.
    ///
    /// A LoRA published for two engines is two different files, and taking the largest fetched
    /// the wrong one for a Flux graph. Empty means there is nothing to choose.
    pub versions: Vec<VersionRow>,
    /// Whether it fits in this card's free memory. `None` is *nobody measured*, never *no*.
    pub fits: Option<bool>,
}

/// Which reader answers for a kind of asset.
///
/// One place, so a listing and a download can never disagree about who to ask — the same reason
/// `Shelf::for_kind` exists one crate over.
/// Always a usable key, never an empty string a caller has to know to fall back from — a field
/// that is sometimes blank is a second rule living in whoever reads it.
pub(crate) fn catalogue_for(
    kind: epoch_engine::assets::asset::Kind,
    source: epoch_engine::models::catalogue::Source,
) -> &'static str {
    match kind {
        epoch_engine::assets::asset::Kind::Voice => "voices",
        // Everything else came from whichever picture catalogue answered, and those two route by
        // the site's own name because for them the site *is* the reader.
        _ => source.name(),
    }
}

/// One version of an asset, as a card offers it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionRow {
    /// The id to install: `civitai:2851202#2142473`.
    pub id: String,
    pub name: String,
    pub said_base: String,
    pub family: String,
    pub bytes: u64,
}

/// A search across every catalogue at once.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetsFound {
    pub assets: Vec<AssetView>,
    /// Where each source got to, so NEXT continues rather than searching again.
    ///
    /// One per source that has more, because they page differently and one of them running out
    /// is not the end of the other. Empty means every source reached its end.
    pub more: Vec<MoreRow>,
    /// Which sources did not answer, and why — in their own words. Never folded into an empty
    /// result: a busy server telling somebody their style does not exist is the failure the
    /// `Refusal` type exists to prevent.
    pub refused: Vec<String>,
    /// This machine's card, in one line. Empty when there is nothing to report.
    pub card: String,
}

/// The id a surface uses for a source, and the one a secret is named after.
pub(crate) const fn source_id(source: epoch_engine::models::catalogue::Source) -> &'static str {
    use epoch_engine::models::catalogue::Source;
    match source {
        Source::Civitai => "civitai",
        Source::HuggingFace => "huggingface",
    }
}

/// One asset catalogue's key, as a screen shows it.
///
/// **No field a value could go in.** ADR-0026's guarantee, applied to a surface: the panel is
/// structurally incapable of displaying a token, rather than careful about it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogueKeyView {
    pub id: String,
    pub name: String,
    /// A key is stored. Never what it is.
    pub held: bool,
    /// What stops working without one — measured against the site, not guessed.
    pub needed_for: String,
    /// Where the person goes to get one.
    pub found_at: String,
    /// What the key can do beyond what Epoch uses it for.
    pub caution: String,
}

/// The Generative Library, as a screen shows it (ADR-0032).
///
/// Machine-scoped, not World-scoped, and the view says so by being one block rather than one per
/// World: a checkpoint is a fact about this computer.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryView {
    /// Where it is, so an empty library is a place somebody can go and look at.
    pub root: String,
    /// What ComfyUI was told, and whether it has heard it yet.
    pub note: String,
    /// True only when ComfyUI has to be restarted before it can see the library. Measured, once:
    /// a new search path needs a restart; a new file inside one does not.
    pub restart: bool,
    /// The ComfyUI whose config Epoch wrote, when one was found.
    pub studio_at: Option<String>,
    pub shelves: Vec<ShelfView>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfView {
    pub id: String,
    pub name: String,
    pub held: Vec<HeldView>,
}

/// One file on a shelf, understood from its own bytes.
/// One runtime that is answering, and what it can time.
///
/// One runtime a curve can be mapped on, and how much of the curve it can be told.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasurableOn {
    pub id: String,
    pub name: String,
    /// Which axes this one can vary, in words: *context and cache*, or *context only*.
    ///
    /// Said on the button, because eight rows on one runtime and four on another is honest and
    /// silently different is not.
    pub maps: String,
    pub models: Vec<String>,
}

/// What one runtime's GPU backend is here, and what has been chosen for it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnginesView {
    pub id: String,
    pub name: String,
    /// A choice, a single fixed one, or nothing that could be read — never "CPU".
    pub engines: epoch_engine::models::engines::Engines,
    /// What the user picked. `None` is the program's own detection, not a missing answer.
    pub chose: Option<String>,
    pub compressed_cache: bool,
    /// Whether a compressed cache is something this runtime can be told at all.
    pub can_compress: bool,
}

/// Named by **id** and by name, because a surface must not take an id apart to work out which
/// program it is (`CLAUDE.md`: an id identifies; a kind classifies) and a person needs the name.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeableOn {
    pub id: String,
    pub name: String,
    /// **Which computer this one is on.** `None` is this machine.
    ///
    /// Without it the picker offered `llama.cpp`, `Ollama`, `llama.cpp` — measured, with a
    /// router running here and another on the MacBook. Both entries were right and neither said
    /// where it was, which is the gauge that identifies nobody: two correct readings of the
    /// right quantity with nothing naming what they are readings *of*.
    pub machine: Option<String>,
    /// The deck's own names, not the server's. What a row matches itself against.
    pub models: Vec<String>,
}

/// One file on a shelf. The shared type, for the reason `held_on` is shared.
pub(crate) use epoch_engine::models::generative::Held as HeldView;

/// One machine's answer about the Hugging Face CLI.
///
/// `reached` is separate from `installed` for the reason every readiness reading in Epoch keeps
/// its facts apart: a machine that is switched off and a machine without `hf` have different
/// fixes, and a surface that showed them the same way would send somebody to install something
/// on a computer that is asleep.
#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HuggingFaceSomewhere {
    pub machine: String,
    /// The machine Epoch is running on. Always first, and always answers.
    pub local: bool,
    pub reached: bool,
    pub installed: bool,
    pub version: Option<String>,
    pub found_at: Option<String>,
    pub user: Option<String>,
}

/// What the machine is doing, for somebody about to spend half an hour measuring it.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preflight {
    /// Whether a benchmark would start straight away rather than waiting.
    pub quiet: bool,
    /// One sentence naming what is in the way, or that nothing is.
    pub says: String,
    pub cpu: Option<f64>,
    pub gpu: Option<f64>,
    pub vram_held: Option<u64>,
    /// Other servers holding a model, each named. Empty is a measurement.
    pub neighbours: Vec<String>,
    /// The reading itself, for anything that wants more than the sentence.
    pub busy: epoch_engine::models::hygiene::Busy,
}

impl World {
    /// What Ollama is featuring, with what this machine already has marked.
    ///
    /// **Featured, not a catalogue** — nineteen entries when it was measured, and it does not
    /// contain everything a person can run. The panel says so, because a list labelled as the
    /// library would make somebody believe those are their options.
    pub fn featured_models(&self) -> Result<Vec<epoch_engine::Offer>, String> {
        let installed: Vec<String> = reading(&self.providers)
            .survey()
            .into_iter()
            .filter(|p| p.online)
            .flat_map(|p| p.models)
            .collect();
        epoch_engine::models::featured(&installed)
    }

    /// Search Hugging Face for models this machine could pull.
    ///
    /// The other source. Ollama's featured list is nineteen entries; this is the rest of the
    /// world, GGUF only because that is what `ollama pull hf.co/…` takes.
    pub fn search_models(
        &self,
        query: &str,
        facets: &[String],
        // The cursor from a previous answer, when the user asked for more. Opaque, and passed
        // straight back: it encodes a sort position and a search token that belong to Hugging
        // Face, and anything here that took it apart would be guessing at somebody's private
        // format.
        more: Option<&str>,
    ) -> Result<epoch_engine::models::Found, String> {
        let wanted: Vec<epoch_engine::models::Facet> = facets
            .iter()
            .filter_map(|id| epoch_engine::models::Facet::from_id(id))
            .collect();
        let installed: Vec<String> = reading(&self.providers)
            .survey()
            .into_iter()
            .filter(|p| p.online)
            .flat_map(|p| p.models)
            .collect();
        match more {
            Some(cursor) => epoch_engine::models::search_more(cursor, &wanted, &installed),
            None => epoch_engine::models::search(query, &wanted, &installed),
        }
    }

    /// Search Ollama's own shelf.
    ///
    /// **The third source, and the one that was missing.** `featured_models` reads the front
    /// page — nineteen entries — and `search_models` reaches Hugging Face. Everything Ollama
    /// publishes in between was reachable only by typing its exact name, which is a search box
    /// for people who already know the answer.
    ///
    /// Measured before it was built: `ollama.com` has no search API (`/api/search` answers 404),
    /// and its registry manifest endpoint genuinely does. So names are read from the page and
    /// weighed against a registry that answers — a page that changes shape yields fewer results
    /// and says so, never wrong ones.
    pub fn search_ollama(&self, query: &str) -> Result<Vec<epoch_engine::Offer>, String> {
        let installed: Vec<String> = reading(&self.providers)
            .survey()
            .into_iter()
            .filter(|p| p.online)
            .flat_map(|p| p.models)
            .collect();
        epoch_engine::models::search_shelf(query, &installed)
    }

    /// Download a model into the local runtime, reporting the runtime's own account of it.
    ///
    /// **Where it goes is measured, not chosen.** The endpoint comes from the configured Ollama
    /// backend rather than a constant: somebody running it on another port would otherwise watch
    /// this fail against an address nothing is listening on.
    pub fn pull_model(
        &self,
        model: &str,
        sink: &mut dyn FnMut(epoch_engine::models::Fetching),
    ) -> Result<(), String> {
        let endpoint = epoch_engine::backends::Backends::load(&vault_dir())
            .backends
            .iter()
            .find(|backend| backend.kind == epoch_engine::backends::Kind::Ollama && backend.enabled)
            .map(|backend| backend.endpoint.clone())
            .ok_or_else(|| {
                "No local runtime is configured to download into. Add Ollama under Connections."
                    .to_owned()
            })?;

        self.allow_running();
        epoch_engine::models::pull(&endpoint, model, &|| self.stopped(), sink)
    }

    /// Every quantisation a repository publishes, with what each costs **here**.
    ///
    /// The verdict is attached per variant rather than left to the caller: a list of sizes is
    /// Hugging Face's own page, and the reason to have one inside Epoch is that it can say which
    /// of them this machine can actually run.
    pub fn model_variants(&self, repo: &str) -> Result<Vec<epoch_engine::models::Group>, String> {
        let repo = repo
            .trim()
            .trim_start_matches("hf.co/")
            .trim_start_matches("huggingface.co/");
        // **And the same quantisations with the prediction head, where a sibling publishes them.**
        // The head is weights: pick the plain file and `--spec-type draft-mtp` is unavailable to
        // that model for ever, with nothing on the screen having said so.
        epoch_engine::models::variants_with_head(repo)
    }

    /// The other runtimes on this machine: llama.cpp and LM Studio.
    ///
    /// Measured, never remembered — installed, serving, and where. Three facts because they
    /// have three fixes, and telling somebody to install what they already have is the failure
    /// that shape exists to prevent (ADR-0027, one layer across).
    pub fn local_runtimes(&self) -> Vec<epoch_engine::models::runtimes::Available> {
        epoch_engine::models::runtimes::survey()
    }

    /// Open the user's own terminal on the command that installs one.
    ///
    /// **Epoch opens the door; it never holds the key.** An installer asks for elevation, shows
    /// a licence and sometimes asks questions, all of which belong to the person at the machine.
    /// Nothing here reports success: a terminal opened is all this can honestly claim, and
    /// whether it worked is answered by surveying again.
    pub fn install_runtime(&self, id: &str) -> Result<(), String> {
        let runtime = runtime_named(id)?;
        epoch_engine::models::runtimes::open_in_terminal(
            &format!("Epoch - install {}", runtime.name()),
            runtime.install_command(),
        )
    }

    /// What can make a picture on this machine.
    ///
    /// Separate from `local_runtimes` and deliberately so (ADR-0030): everything in that list
    /// becomes a Provider a character can be assigned to, and a diffusion server in the Brain
    /// dropdown would be a brain nobody can hold a conversation with.
    pub fn image_studios(&self) -> Vec<epoch_engine::models::studio::Easel> {
        epoch_engine::models::studio::survey()
    }

    /// What can speak here.
    ///
    /// Its own question and not a row on the runtimes deck: Piper has no server and no port, so
    /// a `SERVING` lamp beside it would be an instrument that can never move.
    pub fn voice_engines(&self) -> Vec<epoch_engine::speech::Mouth> {
        epoch_engine::speech::survey()
    }

    /// Fetch one, because there is nothing to ask a package manager for.
    ///
    /// **The only program on the deck Epoch downloads itself**, and it is measured rather than
    /// assumed: `winget search piper` answers `npiperelay` and `PhotoPiper`, and Homebrew answers
    /// 404 as formula and as cask. Everything else here has a command Epoch prints and runs in
    /// the user's own terminal.
    pub fn install_voice_engine(
        &self,
        id: &str,
        watching: &dyn Fn(u64, Option<u64>),
    ) -> Result<String, String> {
        let which = epoch_engine::speech::Voicebox::ALL
            .into_iter()
            .find(|it| it.id() == id)
            .ok_or_else(|| format!("Epoch has no voice engine called {id:?}"))?;
        epoch_engine::speech::install(which, watching)
    }

    /// Say one line with one voice, and hand back the sound itself.
    ///
    /// **The deck's own check**, and it ends at audio rather than at an exit code. A voice that
    /// installed perfectly and cannot speak is exactly what a row claiming `INSTALLED` would
    /// otherwise hide.
    ///
    /// The sound crosses as a `data:` URI, which is how a World's own sounds already reach the
    /// window (ADR-0020) — the presentation layer is handed bytes and never a path. A sample
    /// sentence is about 150 KB, so this needs no scheme of its own; the day something long is
    /// spoken, it takes the `epoch://` route pictures took.
    pub fn try_voice(
        &self,
        voice: &str,
        say: &str,
        sounds_like: Option<epoch_kernel::Timbre>,
    ) -> Result<Spoken, String> {
        let mouth = epoch_engine::speech::look_for(epoch_engine::speech::Voicebox::Piper);
        let exe = mouth
            .at
            .ok_or("Piper is not installed here yet — install it on this deck first.")?;
        // **Resolved by the one function that knows how**, never by joining a path here.
        //
        // This built `<shelf>/<name>` and the file is `<name>.onnx`, so every voice was *not on
        // the shelf* — with the missing extension invisible in the message, which named a path
        // instead of a voice. Two places deciding where a voice lives is how they disagree, and
        // `find_voice` is the place that decides.
        let path = epoch_engine::speech::find_voice(voice).ok_or_else(|| {
            format!("'{voice}' is not installed here. WORKSHOP → VOICES WORKSHOP has it.")
        })?;

        let into = std::env::temp_dir().join(format!("epoch-said-{voice}.wav"));
        // **`speak_as`, never `speak`.** A caller that spoke and forgot the timbre would answer
        // as the wrong person and sound perfectly fine doing it, which is why there is one
        // function rather than two steps somebody has to remember to take in order.
        let (took, but) = epoch_engine::speech::speak_as(
            std::path::Path::new(&exe),
            &path,
            sounds_like
                .as_ref()
                .map(|it| (it.voice.as_str(), it.semitones, it.speaker)),
            say,
            &into,
        )?;
        let bytes = std::fs::read(&into)
            .map_err(|why| format!("it spoke and the sound could not be read back: {why}"))?;
        let _ = std::fs::remove_file(&into);

        // **A name, never the bytes.** A `data:` URI was the first answer here and it never made
        // a sound: this build's `media-src` is `epoch: http://epoch.localhost`, so the browser
        // refused every one — silently, because a blocked source and a broken file raise the
        // same error.
        let name = epoch_engine::speech::keep_spoken(&crate::state::vault_dir(), &bytes)
            .map_err(|why| format!("it spoke and the sound could not be kept: {why}"))?;

        Ok(Spoken {
            // **The name, not a URL.** Measured in the window: `epoch://spoken-<name>` is
            // refused and `http://epoch.localhost/spoken-<name>` plays — the custom scheme is
            // served under that origin on Windows, which is what `convertFileSrc` knows and the
            // Engine does not. So the surface builds the address, exactly as it does for a
            // preview, and this says only which file.
            sound: name,
            millis: took.as_millis() as u64,
            bytes: bytes.len() as u64,
            // Said, not raised. A timbre is the optional last step: the answer is already on
            // screen, and the plain voice is the right thing to fall back to -- but silently
            // sounding like somebody else than the character was set to is a gauge that lies.
            but,
        })
    }

    /// What can listen here.
    pub fn voice_ears(&self) -> Vec<epoch_engine::hearing::Listening> {
        epoch_engine::hearing::survey()
    }

    /// Fetch the ear's program, or one of its models.
    ///
    /// One command for both because they are one decision to a person — *let Epoch listen* — and
    /// two rows that must be pressed in the right order is a shape that teaches somebody the
    /// order by failing at them.
    pub fn install_ear(
        &self,
        what: &str,
        watching: &dyn Fn(u64, Option<u64>),
    ) -> Result<String, String> {
        use epoch_engine::hearing::{Ear, Hearing};
        if what == Ear::WhisperCpp.id() {
            return epoch_engine::hearing::install(Ear::WhisperCpp, watching);
        }
        let model = Hearing::ALL
            .into_iter()
            .find(|it| it.id() == what)
            .ok_or_else(|| format!("Epoch has no ear or model called {what:?}"))?;
        epoch_engine::hearing::install_model(model, watching)
    }

    /// Turn a recording into words.
    ///
    /// **The crew's names travel with it.** Epoch knows who lives in the World and whisper takes
    /// an initial prompt; measured, that is the difference between hearing `Mage` and hearing
    /// *imagen*. Withholding a fact Epoch holds is the shape of every defect in `CLAUDE.md`.
    pub fn listen(
        &self,
        wav_base64: &str,
        language: Option<&str>,
    ) -> Result<epoch_engine::hearing::Heard, String> {
        let ear = epoch_engine::hearing::look_for(epoch_engine::hearing::Ear::WhisperCpp);
        let exe = ear
            .at
            .ok_or("Nothing here can listen yet — install the ear on the Creations deck.")?;
        let model = epoch_engine::hearing::best_model()
            .ok_or("The ear is installed and has no model yet.")?;

        let bytes = epoch_engine::import::decode_shared(wav_base64)
            .map_err(|why| format!("that recording could not be read: {why}"))?;
        // Its own file, thrown away afterwards: a recording is a means, not a record. Keeping it
        // would make evidence of somebody clearing their throat.
        let into = std::env::temp_dir().join(format!("epoch-heard-{}.wav", std::process::id()));
        std::fs::write(&into, &bytes).map_err(|why| format!("could not keep it to read: {why}"))?;

        // Who lives in this World, so whisper expects their names rather than guessing at them.
        // Bounded, because an initial prompt is a sentence whisper reads and not a lookup table:
        // a hundred names would crowd out the audio it is meant to be listening to.
        let crew: Vec<String> = {
            let inner = self.lock();
            match inner.world_id.clone() {
                Some(world) => inner
                    .registry
                    .living_in(&world)
                    .map(|c| c.name.clone())
                    .take(12)
                    .collect(),
                None => Vec::new(),
            }
        };
        /*
            **What the user said they speak, when a caller says nothing.**

            The surface does not carry this: it is a machine setting, and a window that read it
            and passed it down would be a second place holding one fact. Detecting is the
            fallback and it is a real choice — `None` here means *decide each time*, and the
            answer says which language it decided on.
        */
        let chosen = self.settings().hearing_language;
        let language = language.or(chosen.as_deref());

        let heard = epoch_engine::hearing::transcribe(
            std::path::Path::new(&exe),
            &model,
            &into,
            language,
            &crew,
        );
        let _ = std::fs::remove_file(&into);
        heard
    }

    /// Open the user's own terminal on the command that installs one.
    pub fn install_studio(&self, id: &str) -> Result<(), String> {
        let studio = studio_named(id)?;
        epoch_engine::models::runtimes::open_in_terminal(
            &format!("Epoch - install {}", studio.name()),
            studio.install_command(),
        )
    }

    /// Start it, and say nothing about what happens next.
    ///
    /// **It may open a wizard, and Epoch does not answer wizards.** ComfyUI's first run asks
    /// where to keep models and to accept its own licence; both are the user's decisions, and a
    /// licence accepted on somebody's behalf is not accepted. The panel says so before the press
    /// rather than after it.
    /// Stop the server a studio runs, and say what happened.
    ///
    /// **Measured, which is why the button exists**: idle, ComfyUI holds 2.6 GB of system RAM
    /// with 0.03 GB reserved on the card, and `POST /free` returns none of it — 2609 MB before
    /// and after. What stays is the interpreter, torch's CUDA context and the node modules, and
    /// none of that goes while the process lives.
    ///
    /// Epoch knew how to start one and not how to stop it, which made *"it is running and using
    /// memory"* a thing the user had to go and fix in Task Manager.
    /// What this machine can actually run, most used first.
    ///
    /// The card's free memory is measured here rather than passed in, for the reason every
    /// reading on this deck is: a number the surface sent could be one it read a minute ago.
    pub fn suited_models(&self) -> Result<Vec<epoch_engine::models::Suited>, String> {
        epoch_engine::models::deck::what_fits(&self.shared_weights())
    }

    /// Time one model on a runtime that is serving it, and remember the answer.
    ///
    /// **A measurement rather than an estimate**, which is the whole point. A card's published
    /// bandwidth times a per-quantisation efficiency factor gets the order of magnitude — checked
    /// against `whichllm`'s model on this machine, 19% low for one model and three times out for
    /// another. This machine's own answer is exact for this machine, and one of them turns every
    /// other size on the list into a speed.
    ///
    /// It costs a load and two short answers, so it is a button somebody presses.
    /// Which runtimes are answering, and which of this deck's models each one can time.
    ///
    /// **One probe for the whole deck**, rather than one per row: `models_and_loadouts` is
    /// deliberately a read with no probing in it, and this is the question that genuinely needs
    /// the network. Asked once when the deck opens and again after anything changes.
    ///
    /// The translation between the two spellings happens here, so no surface has to know that
    /// llama.cpp calls `gemma4:12b` something else.
    pub fn who_can_time(&self) -> Vec<TimeableOn> {
        let held: Vec<String> = self
            .shared_weights()
            .into_iter()
            .map(|one| one.name)
            .collect();
        self.providers()
            .into_iter()
            .filter(|it| it.online)
            .map(|it| TimeableOn {
                models: held
                    .iter()
                    .filter(|name| {
                        epoch_engine::models::runtimes::offered_as(name, &it.models).is_some()
                    })
                    .cloned()
                    .collect(),
                id: it.id,
                name: it.name,
                machine: it.machine,
            })
            .filter(|it| !it.models.is_empty())
            .collect()
    }

    /// Which runtimes a curve can be mapped on, and how much of it each one maps.
    ///
    /// **Not every runtime that answers.** A curve is only worth running where its result is
    /// consumed, and the three differ (measured 2026-08-30):
    ///
    /// - **llama.cpp** — context and cache, eight rows, applied through `presets.ini`.
    /// - **Ollama** — context only, four rows, against whatever cache the server was started
    ///   with; applied as a ceiling on the window every turn sizes for itself.
    /// - **LM Studio** — absent. It takes a context only at load, through its own CLI, and Epoch
    ///   does not load through it, so a curve would end in a setting nothing applies. That is the
    ///   dead `Manual` mode of ADR-0027, and the honest thing is to not offer the button.
    pub fn who_can_measure(&self) -> Vec<MeasurableOn> {
        let held: Vec<String> = self
            .shared_weights()
            .into_iter()
            .map(|one| one.name)
            .collect();
        epoch_engine::models::runtimes::Runtime::ALL
            .into_iter()
            .filter_map(|runtime| {
                let maps = match runtime {
                    epoch_engine::models::runtimes::Runtime::LlamaCpp => "context and cache",
                    epoch_engine::models::runtimes::Runtime::Ollama => "context only",
                    // Nothing consumes it, so it is not offered.
                    epoch_engine::models::runtimes::Runtime::LmStudio => return None,
                };
                let seen = epoch_engine::models::runtimes::look_for(runtime);
                // llama.cpp is spawned per setting, so it need only be installed. Ollama's curve
                // goes through the server that is up, so it must be answering.
                let ready = match runtime {
                    epoch_engine::models::runtimes::Runtime::Ollama => seen.serving,
                    _ => seen.installed,
                };
                if !ready {
                    return None;
                }
                let models = match runtime {
                    epoch_engine::models::runtimes::Runtime::Ollama => held
                        .iter()
                        .filter(|name| {
                            epoch_engine::models::runtimes::offered_as(name, &seen.models).is_some()
                        })
                        .cloned()
                        .collect(),
                    // Any file on the shelf: Epoch opens it itself.
                    _ => held.clone(),
                };
                (!models.is_empty()).then(|| MeasurableOn {
                    id: runtime.id().to_owned(),
                    name: runtime.name().to_owned(),
                    maps: maps.to_owned(),
                    models,
                })
            })
            .collect()
    }

    pub fn time_model(&self, model: &str, on: &str) -> Result<String, String> {
        // **The runtime is the caller's, and it used to be whichever answered first.**
        //
        // `find` over the providers in configured order meant Ollama won every model it had, and
        // it has every model on this machine's shelf — so TIME IT could not measure llama.cpp or
        // LM Studio at all, while naming Ollama in an answer that read like a fact about the
        // model. 11.19 measured 8.5 s, 10.5 s and 19.8 s for one short answer across the three;
        // which runtime is the interesting half of the number, and nobody could ask for it.
        let serving = self
            .providers()
            .into_iter()
            .find(|it| it.id == on)
            .ok_or_else(|| format!("there is no runtime called '{on}' here"))?;
        if !serving.online {
            return Err(format!("{} is not answering.", serving.name));
        }
        // **And the name it is asked for is the name that server uses.** A deck row is
        // `gemma4:12b` and llama.cpp's router calls the same weights `gemma4-12b`, because Epoch
        // wrote that link itself.
        let named = epoch_engine::models::runtimes::offered_as(model, &serving.models)
            .ok_or_else(|| format!("{} is not serving '{model}'.", serving.name))?;
        // The program behind the id, so the release afterwards speaks that program's language.
        // A provider that is not one of the three local runtimes is somebody else's machine and
        // has nothing here to release.
        let runtime = epoch_engine::models::runtimes::Runtime::ALL
            .into_iter()
            .find(|it| it.id() == serving.id);

        let held = epoch_engine::models::deck::installed(&self.shared_weights())
            .into_iter()
            .find(|it| it.name == model)
            .map(|it| it.bytes)
            .unwrap_or(0);
        let machine = epoch_engine::models::machine::Machine::measure();
        // Whether it fitted decides whether this answer may calibrate anything else: a model that
        // spilled across PCIe describes that model on this card and not the card.
        let fitted = machine
            .vram_free
            .is_some_and(|free| held > 0 && held <= free.saturating_sub(1_500_000_000));

        let ollama = serving.endpoint.contains("11434");

        /*
            **A measurement leaves the card as it found it.**

            Reported by the owner: after a TIME IT the model is still resident and REACTOR LOAD
            reads 12.6 of 12.9 GB. None of the rules that release a model reach this, and they
            are all about *turns* — a different character speaking, a picture needing the card,
            KEEP switched off above a conversation. TIME IT is none of those: nobody is talking
            to this model, and the person who pressed it is on a deck reading numbers.

            **Only if this was what loaded it**, which is the same rule the picture studio keeps:
            Epoch started it, so Epoch may stop it. A model a character is mid-conversation with
            was already resident, and taking it away to tidy up after a measurement would cost
            that conversation a reload it did not ask for.

            Read from the runtime's own word for what is on the card (`resident`), never from
            what it *offers* — 11.24, and the reason that field exists.
        */
        let was_resident = runtime.is_some_and(|runtime| {
            epoch_engine::models::runtimes::look_for(runtime)
                .resident
                .iter()
                .any(|it| it == &named)
        });

        let rate = epoch_engine::models::speeds::time_it(&serving.endpoint, &named, ollama)?;

        if let (false, Some(runtime)) = (was_resident, runtime) {
            epoch_engine::models::runtimes::let_go_of(runtime, &named);
        }

        let library = epoch_engine::models::generative::Library::here();
        let mut speeds = epoch_engine::models::speeds::Speeds::load(library.root());
        speeds.remember(epoch_engine::models::speeds::Measured {
            model: model.to_owned(),
            runtime: serving.name.clone(),
            bytes: held,
            tokens_per_second: rate,
            fitted,
            at: epoch_engine::now_ms(),
        });
        let _ = speeds.save(library.root());

        Ok(format!(
            "{rate:.1} tokens per second on {}, measured here.",
            serving.name
        ))
    }

    pub fn stop_studio(&self, id: &str) -> Result<String, String> {
        epoch_engine::models::studio::stop_server(studio_named(id)?)
    }

    pub fn start_studio(&self, id: &str) -> Result<(), String> {
        let studio = studio_named(id)?;
        let seen = epoch_engine::models::studio::look_for(studio);
        let command = seen.start.ok_or_else(|| {
            format!(
                "{} is not on this machine, so there is nothing to start.",
                seen.name
            )
        })?;
        epoch_engine::models::runtimes::open_in_terminal(
            &format!("Epoch - {}", studio.name()),
            &command,
        )
    }

    /// Start a runtime's server, in the user's own terminal.
    ///
    /// The same door as installing. A server Epoch spawned and owned would die when Epoch does,
    /// and its output would go somewhere nobody can read.
    pub fn start_runtime(&self, id: &str) -> Result<(), String> {
        let runtime = runtime_named(id)?;
        let seen = epoch_engine::models::runtimes::look_for(runtime);
        if seen.start.is_none() {
            return Err(format!(
                "{} is not on this machine, so there is nothing to start.",
                seen.name
            ));
        }
        // **What the user chose is applied here and nowhere else**, because a backend and a cache
        // type are read by these programs when they start. Adding them to `look_for`'s `start`
        // would put a preference inside a *probe*, and every surface that only wanted to know
        // whether a runtime exists would carry one.
        /*
            **llama.cpp's row starts the *router*, with the shelf, or it starts nothing useful.**

            `start_command` for llama.cpp is the bare `llama-server.exe`: no `--models-dir`, no
            `--models-preset`, no port. Measured on the owner's machine — START, then
            `/v1/models` listing **one** model, a cached `gemma-3-270m` that had nothing to do
            with his shelf — and a character on `Qwen3.6-35B` answered
            `400: model 'Qwen3.6-35B-A3B-UD-IQ2-M---MTP' not found`. Read as a problem with his
            machine at the time; it was this button.

            Every other path that starts llama.cpp — a download, a benchmark, applying a profile
            — goes through `serve_everything`, which passes the shelf and the presets. The deck's
            own START was the one that did not, so it started a program that could not answer for
            anything Epoch holds.

            **Starting the router is also the moment it reads the presets, so it is the moment
            they have to be right.**

            `Restocking the shelf is not telling the router about it` is already written down one
            function over. This is the same sentence the other way round: *starting the router
            without restocking starts it with yesterday's answers.*

            Measured on the owner's machine, 2026-09-06. `loadouts.json` held twelve finished
            searches under this card and this build — his Qwen3.6-35B chose **32,768 · q8_0** —
            MODELS drew every one of them as `measured`, and `presets.ini` said `ctx-size =
            16384` for all nine models on the shelf. So llama.cpp had been started with the
            conservative floor every time, and `--ctx-size 16384` in its log was the only place
            that disagreed with the deck.

            **Nothing was broken except the order.** The file is rewritten by choosing a profile,
            tuning a model, downloading one and removing one — every gesture except the one that
            makes a program read it. A search that finishes and is never applied is a benchmark
            somebody ran for nothing.
        */
        if runtime == epoch_engine::models::runtimes::Runtime::LlamaCpp {
            // The presets are rewritten inside this, from what has actually been measured, and
            // then the router is started with the shelf and that file. One path, so the deck's
            // button and a finished benchmark cannot start two different programs.
            self.tell_llama_cpp();
            return self.restart_router_for("", None);
        }

        let pref = self
            .settings()
            .runtimes
            .get(id)
            .cloned()
            .unwrap_or_default();
        let command = epoch_engine::models::runtimes::start_command_with(runtime, &pref)
            .ok_or_else(|| format!("{} could not be found to start.", seen.name))?;
        epoch_engine::models::runtimes::open_in_terminal(
            &format!("Epoch - {}", runtime.name()),
            &command,
        )
    }

    /// What each runtime's GPU backend is here, and what the user has chosen.
    ///
    /// Read every time rather than kept: somebody who installs a CUDA build while the deck is
    /// open should see it, and this is a directory listing and one CLI call.
    pub fn runtime_engines(&self) -> Vec<EnginesView> {
        let settings = self.settings();
        epoch_engine::models::runtimes::Runtime::ALL
            .into_iter()
            .map(|runtime| {
                let pref = settings
                    .runtimes
                    .get(runtime.id())
                    .cloned()
                    .unwrap_or_default();
                EnginesView {
                    id: runtime.id().to_owned(),
                    name: runtime.name().to_owned(),
                    engines: epoch_engine::models::engines::engines(runtime),
                    chose: pref.engine,
                    compressed_cache: pref.compressed_cache,
                    // Only Ollama's cache can be set at start (measured 2026-08-30): LM Studio
                    // has no flag and no load route, and llama.cpp's is chosen per model by the
                    // curve that measured it. A switch on either would change nothing.
                    can_compress: runtime == epoch_engine::models::runtimes::Runtime::Ollama,
                }
            })
            .collect()
    }

    /// Choose a backend for a runtime.
    ///
    /// **Two programs, two mechanisms, and the difference is said out loud.** LM Studio is told
    /// now and keeps the choice itself; Ollama is told when Epoch next starts it, so the answer
    /// says so rather than letting somebody believe a running server changed.
    pub fn choose_engine(&self, id: &str, engine: Option<String>) -> Result<String, String> {
        let runtime = runtime_named(id)?;
        // Under the lock, with the write: two controls on one row are set in one gesture, and a
        // read-modify-write from each puts the other one back. See `World::change_settings`.
        self.change_settings(|settings| {
            settings.runtimes.entry(id.to_owned()).or_default().engine = engine.clone();
        })?;

        if runtime == epoch_engine::models::runtimes::Runtime::LmStudio {
            return match engine {
                Some(engine) => epoch_engine::models::engines::choose(&engine),
                // Nothing to un-choose: LM Studio always has one engine selected, and asking it
                // to have none would be inventing a state the program does not have.
                None => Ok("LM Studio keeps the engine it has.".to_owned()),
            };
        }
        Ok(match engine.as_deref() {
            Some(_) => format!(
                "{} will use it the next time it starts — CONNECTIONS, {}, START.",
                runtime.name(),
                runtime.name()
            ),
            None => format!("{} will choose for itself again.", runtime.name()),
        })
    }

    /// Turn the compressed attention cache on or off, and — when turning it on — prove it.
    ///
    /// **Switched on, proved, and turned back off if the proof fails.** A quantized V cache
    /// requires flash attention, which belongs to the *backend*, and without it nothing loads at
    /// all: measured, `quantized V cache requires flash_attn to be enabled`, arriving as a 500
    /// with that sentence in the body. A setting kept on the strength of being switched on would
    /// leave a machine where every model fails, explained by a message naming llama.cpp.
    ///
    /// The proof needs the server running, so it is only attempted when one is answering. With
    /// nothing to ask, the setting is stored and reported as **unproved** — never as working.
    pub fn compress_cache(&self, id: &str, on: bool) -> Result<String, String> {
        let runtime = runtime_named(id)?;
        self.change_settings(|settings| {
            settings
                .runtimes
                .entry(id.to_owned())
                .or_default()
                .compressed_cache = on;
        })?;
        if !on {
            return Ok(format!(
                "{} will hold the cache at full precision the next time it starts.",
                runtime.name()
            ));
        }

        let seen = epoch_engine::models::runtimes::look_for(runtime);
        let Some(model) = seen.models.first().cloned() else {
            return Ok(format!(
                "Saved. {} is not answering, so nothing has been proved yet — it is applied, and \
                 checked, the next time it starts.",
                runtime.name()
            ));
        };
        Ok(format!(
            "Saved, and it takes effect the next time {} starts — CONNECTIONS, {}, START. \
             The load is checked then; on a backend without flash attention a compressed cache \
             stops models loading entirely, and Epoch will say so and switch this back off. \
             (There is a {model} here to check it with.)",
            runtime.name(),
            runtime.name()
        ))
    }

    /// Wait for a runtime that was just started, and prove the compressed cache actually loads.
    ///
    /// **Against the server that was started with the setting, never the one before it.** Proving
    /// it on the process already running — started without the variable — would be measuring a
    /// path Epoch does not take, which is the mistake 11.18 is named after.
    ///
    /// Answers `Ok(None)` when there was nothing to prove: the switch is off, or nothing is
    /// serving, or this runtime holds no model to try. Nothing proved is not the same as
    /// something failing, and the two must not share a sentence.
    pub fn prove_runtime(&self, id: &str) -> Result<Option<String>, String> {
        let runtime = runtime_named(id)?;
        let settings = self.settings();
        let wanted = settings
            .runtimes
            .get(id)
            .is_some_and(|pref| pref.compressed_cache);
        if !wanted {
            return Ok(None);
        }

        // A start opens a terminal and returns; the server is a few seconds behind it.
        let mut seen = epoch_engine::models::runtimes::look_for(runtime);
        for _ in 0..30 {
            if seen.serving {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
            seen = epoch_engine::models::runtimes::look_for(runtime);
        }
        if !seen.serving {
            return Ok(None);
        }
        let Some(model) = seen.models.first().cloned() else {
            return Ok(None);
        };

        match epoch_engine::models::runtimes::prove_the_cache(&seen.endpoint, &model) {
            Ok(()) => Ok(Some(format!(
                "{} loaded {model} with the compressed cache.",
                runtime.name()
            ))),
            Err(said) => {
                // **Switched off on the way out.** Leaving it on would leave a machine where
                // every model fails to load, explained by a message that names llama.cpp.
                self.change_settings(|settings| {
                    settings
                        .runtimes
                        .entry(id.to_owned())
                        .or_default()
                        .compressed_cache = false;
                })?;
                Err(format!(
                    "The compressed cache has been switched back off. {} would not load {model} \
                     with it, and said: {said}",
                    runtime.name()
                ))
            }
        }
    }

    /// Start a runtime serving one model, in the user's own terminal.
    ///
    /// The same door. A server that Epoch spawned and owned would be a server that dies when
    /// Epoch does, and one the user cannot see the output of.
    pub fn serve_model(&self, id: &str, pull: &str) -> Result<(), String> {
        let runtime = runtime_named(id)?;
        let command =
            epoch_engine::models::runtimes::serve_command(runtime, pull).ok_or_else(|| {
                format!(
                    "{} cannot be told to serve a model from here",
                    runtime.name()
                )
            })?;
        epoch_engine::models::runtimes::open_in_terminal(
            &format!("Epoch - {} serving", runtime.name()),
            &command,
        )
    }

    /// Every model Ollama already has on disk, as files another runtime can open.
    ///
    /// **Measured, and the measurement is the whole point.** Ollama stores unmodified GGUF: the
    /// blob a manifest marks as the model begins `GGUF`, and `llama-server -m <blob>` loaded
    /// `qwen3:14b` — 14.7B parameters, `Q4_K - Medium` — and answered in 2.4 s on the card.
    /// Nothing is converted or copied.
    ///
    /// So a second runtime costs no disk and no download. The alternative was pulling the same
    /// 9 GB twice on a machine that then has to hold both copies.
    pub fn shared_weights(&self) -> Vec<epoch_engine::models::runtimes::Weights> {
        /*
            **Ollama's store, plus wherever SAVE THE FILE writes.**

            `PULL` hands a model to Ollama, which files it and can use it at once. `SAVE THE
            FILE` runs `hf download` into the vault — and that produced a GGUF *nothing was
            looking at*: not Ollama, which knows only its own store, and not the other two,
            which were only ever offered Ollama's blobs.

            A download that lands somewhere nothing can reach is a download that did not happen.

            The directory is this surface's answer; what counts as a model is `epoch-models`',
            so a lent machine with a different directory still counts the same things.
        */
        epoch_engine::models::runtimes::everything_here(&[vault_dir().join("models")])
    }

    /// Every model here, with what is known about how it runs.
    ///
    /// **One read, no hashing, no probing.** Everything on a row is either on the disk already
    /// (name, shelf, size, whether it has eyes) or in a file Epoch wrote when it measured
    /// something. Opening this deck must not cost a graphics card.
    pub fn models_and_loadouts(&self) -> Vec<ModelHere> {
        epoch_engine::models::deck::about(self.shared_weights())
    }

    /// Which runtime still has something on the card, in its own words.
    ///
    /// **Named, because a name is what somebody can act on.** The sentence used to say only *close
    /// what is using it*, which sends a person to the Task Manager — where the process they find is
    /// as likely as not called `llama-server.exe` and belongs to LM Studio, whose inference runtime
    /// *is* llama.cpp. That happened: 7.6 GB on a process Epoch was blamed for while Epoch's own
    /// router sat at 8 MB with nothing loaded.
    ///
    /// **Asked of the runtimes, not of the driver.** `nvidia-smi --query-compute-apps` was the
    /// obvious source and it is the wrong one here: on this consumer card it answers `[N/A]` for
    /// every process's memory and lists every window on the desktop, so it would have reported
    /// `explorer.exe` as holding a graphics card. A real reading of the wrong quantity. Each
    /// runtime already answers *what is resident* exactly, and that is the thing to close.
    ///
    /// Empty when nothing local admits to holding anything — which is honest and still leaves the
    /// number above, because something is using the card whether or not it will say so.
    fn still_holding() -> String {
        let held: Vec<String> = epoch_engine::models::runtimes::survey()
            .into_iter()
            .filter(|one| !one.resident.is_empty())
            .map(|one| format!("{} has {}", one.name, one.resident.join(", ")))
            .collect();
        if held.is_empty() {
            String::new()
        } else {
            format!(" {}.", held.join("; "))
        }
    }

    /// Map what one model can do on this card. Minutes, and it is asked for.
    /// Map a model's curve on one runtime.
    ///
    /// **The runtime is the caller's**, never whichever answers first: a curve is about a model
    /// *on a runtime*, and the same GGUF answers at three different rates through the three
    /// programs on one card.
    pub fn search_loadout(
        &self,
        path: &str,
        on: &str,
        // Called once per setting tried, with how many have been tried and the bound. The World
        // draws a bar from it — see `loadout::how_many` for why the bound is honest.
        step: &dyn Fn(usize, usize),
    ) -> Result<String, String> {
        let one = self
            .shared_weights()
            .into_iter()
            .find(|held| held.path == path)
            .ok_or("that is not a model this machine reported holding")?;

        let file = std::path::PathBuf::from(&one.path);
        // A model is never asked for more context than it was trained for: it buys nothing, and
        // the probe that proves it costs a load.
        let ceiling = epoch_engine::models::gguf::read(&file)
            .ok()
            .and_then(|header| header.trained_context())
            .and_then(|n| u32::try_from(n).ok())
            .map(|n| n.min(epoch_engine::models::loadout::CEILING))
            .unwrap_or(epoch_engine::models::loadout::CEILING);

        // **The card has to be the search's alone.** Measured through the window: run right
        // after a TIME IT, with `gemma4-12b` still resident on Ollama's five-minute timer, the
        // 27B mapped a complete and plausible curve of wrong numbers — 19.7 tok/s where the same
        // model on a free card answers at 33.7 — and picked the *wrong cache*, because with less
        // room the trade between them runs the other way. Then it saved that as the loadout.
        //
        // A wrong answer chosen silently, beside a table of readings that all agree with each
        // other, is worse than refusing.
        epoch_engine::models::runtimes::free_the_card();

        // And the side effect is read back rather than assumed. Nothing above is required to
        // answer, and a runtime that ignored the ask would otherwise be invisible.
        let before = epoch_engine::models::machine::Machine::measure();
        if let (Some(free), Some(total)) = (before.vram_free, before.vram_total) {
            /*
                **Room for *this model*, not an idle card.**

                This asked for the card to be nearly empty — a tenth of it plus a twentieth of
                the model — and a card is never empty. The owner's, with nothing open but Epoch
                and Task Manager, holds **1.7 GB** for the desktop and the two windows, so
                MEASURE refused a machine that was working perfectly and told him to close what
                was using it. There is nothing to close. A guard whose condition the user cannot
                satisfy is worse than no guard: it teaches them the button is broken.

                The defect it exists for is real and measured — with `gemma4-12b` still resident,
                a 27B mapped a complete, plausible, wrong curve and saved it — but what ruins a
                search is **not having room for the weights**, not the card being untidy. So the
                comparison is against the model: 10.6 GB of weights in 10.3 GB free spills
                whatever is or is not running, and 7.4 GB in the same 10.3 does not.

                The measured case still refuses, and now says something a person can act on: a
                resident 7.4 GB model leaves 4.6 GB free, and a 10.6 GB model does not fit in
                that either.
            */
            let headroom = 512_000_000;
            if one.bytes + headroom > free {
                /*
                    **What happens next depends on the machine, and only one of the two is a
                    wall.**

                    On a card of its own, a model that does not fit RUNS ANYWAY — the rest goes
                    to system memory and it is slow. Nothing fails, which is exactly why this is
                    worth refusing: a curve mapped across the PCIe bus is a measurement of the
                    bus, it looks like every other curve, and it is then saved and chosen for
                    every turn afterwards. Saying "there is not enough" describes a wall that is
                    not there.

                    On a unified machine there is nothing to spill INTO. The same sentence would
                    describe a second pool that does not exist, and this codebase has already
                    paid once for a true number worded for the wrong architecture.
                */
                return Err(if before.unified {
                    format!(
                        "{} is {:.1} GB and there are {:.1} GB free of {:.1} GB. Here the \
                         graphics memory is the system memory, so it will not load rather than \
                         run slowly.{} Free some and measure again.",
                        one.name,
                        one.bytes as f64 / 1e9,
                        free as f64 / 1e9,
                        total as f64 / 1e9,
                        Self::still_holding(),
                    )
                } else {
                    format!(
                        "{} is {:.1} GB and there are {:.1} GB free on a {:.1} GB card. It would \
                         still run — the rest goes to system memory — and that is why this is \
                         worth stopping: every number the search took would be about the spill, \
                         and it would be saved as this model's settings.{} Epoch asked every \
                         local runtime to let go first.",
                        one.name,
                        one.bytes as f64 / 1e9,
                        free as f64 / 1e9,
                        total as f64 / 1e9,
                        Self::still_holding(),
                    )
                });
            }
        }

        /*
            **Two ways to run a curve, and which axes each one can map.**

            llama.cpp: Epoch spawns `llama-server` itself at every setting, so the cache type is
            a flag on the child and both can be compared - eight rows.

            Ollama: the curve goes through the server that is already answering, because
            `options.num_ctx` is a per-request thing there. The cache is not - it is read once
            when the server starts, and a per-request `cache_type_k` is ignored silently
            (measured: 8.39 GB with it and 8.39 GB without, to the byte). So the ladder runs
            under whatever Epoch started that server with, and the answer says which.

            LM Studio is deliberately absent, and it says why rather than offering a button that
            produces a row nothing applies.
        */
        let runtime = runtime_named(on)?;
        let (mut probe, caches, build): (
            Box<dyn epoch_engine::models::loadout::Probe>,
            epoch_engine::models::loadout::Caches,
            String,
        ) = match runtime {
            epoch_engine::models::runtimes::Runtime::LlamaCpp => (
                Box::new(epoch_engine::models::runtimes::Bench::new(&file).ok_or(
                    "llama.cpp is not on this machine, so there is nothing to measure with",
                )?),
                epoch_engine::models::loadout::Caches::Either,
                epoch_engine::models::runtimes::llama_build(),
            ),
            epoch_engine::models::runtimes::Runtime::Ollama => {
                let seen = epoch_engine::models::runtimes::look_for(runtime);
                if !seen.serving {
                    return Err(
 "Ollama is not answering, and this curve runs through the server that is already up. Start it from CONNECTIONS and measure again."
                            .to_owned(),
                    );
                }
                let named = epoch_engine::models::runtimes::offered_as(&one.name, &seen.models)
                    .ok_or_else(|| format!("Ollama is not serving '{}'.", one.name))?;
                let pref = self.settings().runtimes.get(on).cloned().unwrap_or_default();
                (
                    Box::new(epoch_engine::models::runtimes::Served {
                        endpoint: seen.endpoint.clone(),
                        model: named,
                    }),
                    epoch_engine::models::loadout::Caches::Only(
                        epoch_engine::models::runtimes::cache_asked_for(&pref),
                    ),
                    "Ollama".to_owned(),
                )
            }
            epoch_engine::models::runtimes::Runtime::LmStudio => {
                return Err(
 "LM Studio takes a context only when a model is loaded, through its own CLI, and Epoch does not load through it yet - so a curve here would end in a setting nothing applies. Measure on llama.cpp or Ollama."
                        .to_owned(),
                )
            }
        };

        let total = epoch_engine::models::loadout::how_many(ceiling, caches);
        let mut done = 0usize;
        // Reported before the probe rather than after it, so the row says *what it is doing now*
        // rather than what it has finished. A four-minute job that only speaks on completion is
        // one somebody kills at three.
        let mut watched = Watched {
            inner: probe.as_mut(),
            seen: &mut done,
            total,
            say: step,
        };
        let map = epoch_engine::models::loadout::explore(ceiling, caches, &mut watched);
        if !map.loaded() {
            return Err(format!(
 "{} did not load at any setting on {}. The Models deck says which models this build can open.",
                one.name,
                runtime.name()
            ));
        }
        let chose = map
            .recommended()
            .ok_or("nothing that loaded can hold a real turn")?;
        let rate = map.recommended_rate().unwrap_or_default();

        let machine = epoch_engine::models::machine::Machine::measure();
        let library = epoch_engine::models::generative::Library::here();
        let mut held = epoch_engine::models::loadout::Loadouts::load(library.root());
        held.remember(epoch_engine::models::loadout::Best {
            model: epoch_engine::models::loadout::names(&file)
                .ok_or("the file went away while it was being measured")?,
            name: one.name.clone(),
            card: machine.gpu.unwrap_or_default(),
            build,
            runtime: on.to_owned(),
            most: map.most_that_loaded(),
            chose,
            tokens_per_second: rate,
            tried: map
                .readings
                .iter()
                .map(|r| epoch_engine::models::loadout::Tried {
                    loadout: r.loadout,
                    tokens_per_second: r.tokens_per_second,
                })
                .collect(),
            at: epoch_engine::now_ms(),
        });
        held.save(library.root())?;
        /*
            **Told to the program the curve was taken on, and only that one.**

            `tell_llama_cpp` used to run at the end of every search, which was correct while
            llama.cpp was the only thing that could be measured and became a real defect the
            moment it was not: an Ollama curve was written into llama.cpp's `presets.ini` — a
            measurement of one program applied to another, which is the shape of every wrong
            gauge in this codebase.

            Without it for llama.cpp the curve says one thing and the running server keeps doing
            another, which is what made TIME IT and MEASURE disagree by a third on one row.
        */
        let told = match runtime {
            epoch_engine::models::runtimes::Runtime::LlamaCpp => {
                self.tell_llama_cpp();
 "llama.cpp has been told, and loads it this way the next time it starts — CONNECTIONS, llama.cpp, START."
                    .to_owned()
            }
            // Ollama sizes every turn for itself (11.16), which is better than a fixed value.
            // What a curve gives it is the wall: the largest window measured to load here.
            epoch_engine::models::runtimes::Runtime::Ollama => format!(
 "Turns on Ollama size their own window and will not now ask for more than {} tokens, which is the largest that loaded here.",
                map.most_that_loaded()
            ),
            epoch_engine::models::runtimes::Runtime::LmStudio => String::new(),
        };
        Ok(format!(
            "{} on {}: {} tokens of context on the {}, {rate:.1} tok/s. {told}",
            one.name,
            runtime.name(),
            chose.context,
            chose.cache.plainly(),
        ))
    }

    /// Take the user's pick from the curve.
    ///
    /// **Only a loadout that was measured**, so a choice is always a row somebody can see a
    /// number beside. Inventing a settable field that nothing ever ran would be a control with
    /// nothing behind it.
    pub fn choose_loadout(&self, path: &str, context: u32) -> Result<String, String> {
        let said = epoch_engine::models::deck::choose(&self.shared_weights(), path, context)?;
        self.tell_llama_cpp();
        Ok(said)
    }

    /// Write the chosen loadouts into llama.cpp's presets, and let go of what it is holding.
    ///
    /// ## The two places one fact was living in
    ///
    /// A chosen loadout lives in `loadouts.json`, and llama.cpp learns it from `presets.ini` —
    /// which was written **only when the router starts**. So measuring a model, or choosing a
    /// different row of its curve, changed the first and not the second: the deck said
    /// *loads with 16,384 tokens · full cache · measured* while the running server went on
    /// loading the same model with `cache-type-k = q8_0` from before.
    ///
    /// Found by the owner comparing TIME IT against MEASURE and getting 14.0 against 20.6 for
    /// what the screen said was one setting. Read off his machine: the curve had just chosen
    /// *full*, and `presets.ini` still said `q8_0`.
    ///
    /// **Rewriting is not enough on its own, and releasing is not either.** llama.cpp's router
    /// reads a preset when it *spawns* the child that holds the model, and `/models/unload`
    /// frees that child's memory without ending it — measured 2026-08-30: with the preset
    /// rewritten to `ctx-size = 65536` and the model unloaded, the next request woke the same
    /// child, still carrying `--ctx-size 16384 --cache-type-k q8_0` from the argv it was born
    /// with. Restarting the router spawned one with `--ctx-size 65536` and no cache flags.
    ///
    /// So the file is written and the memory is released — which is worth doing on its own — and
    /// what the surface says is what is true: it loads this way the next time llama.cpp starts.
    /// Nothing is started here: a router that is not running has nothing to unload and nothing
    /// to be wrong about.
    fn tell_llama_cpp(&self) {
        let held = self.shared_weights();
        let how = self.how_to_load();
        let told = Self::how_to_tune();
        epoch_engine::models::runtimes::stock_shelf(&shelf_dir(), &held, &how, &told);
        epoch_engine::models::runtimes::free_the_card();
    }

    /// How each model on the shelf should be loaded.
    ///
    /// **Measured where it has been measured, conservative where it has not.** A model nobody
    /// has run through [`epoch_engine::models::loadout`] gets the safe half of every trade —
    /// the compressed cache costs a model that did not need it three percent, and the
    /// full-precision one costs a model that did need it two thirds — so an unmeasured model is
    /// merely not optimal rather than slow.
    ///
    /// Keyed by hash, card and llama.cpp build, so none of the three can go stale silently. The
    /// hash is read from beside the file rather than computed here: this runs every time the
    /// shelf is stocked, and hashing twenty models on a read path would cost a minute of disk.
    /// What each model is **told**, as opposed to what was measured about it.
    ///
    /// Read from the library every time the shelf is stocked, which is a small JSON and not a
    /// probe. Absent is the ordinary state: nothing tuned, and llama.cpp deciding for itself.
    fn how_to_tune(
    ) -> impl Fn(&epoch_engine::models::runtimes::Weights) -> epoch_engine::models::tuning::Tuning
    {
        let library = epoch_engine::models::generative::Library::here();
        let held = epoch_engine::models::tuning::Tunings::load(library.root());
        move |model| held.of(&model.name)
    }

    fn how_to_load(
        &self,
    ) -> impl Fn(&epoch_engine::models::runtimes::Weights) -> epoch_engine::models::loadout::Loadout
    {
        let library = epoch_engine::models::generative::Library::here();
        let held = epoch_engine::models::loadout::Loadouts::load(library.root());
        let machine = epoch_engine::models::machine::Machine::measure();
        let card = machine.gpu.unwrap_or_default();
        let build = epoch_engine::models::runtimes::llama_build();
        let floor = epoch_engine::models::loadout::A_REAL_TURN
            + epoch_engine::models::loadout::ROOM_TO_ANSWER;
        move |model| {
            /*
                **Asked by the key it was written under, which it was not.**

                A search stores its answer under `loadout::names` — `"{bytes}:{stem}"`, readable
                from any file — and this read it back by `hash_beside`, which needs the manifest
                Epoch writes when it *downloads* something. An Ollama blob has none, and neither
                does a GGUF saved before manifests existed, so the lookup missed every time and
                every model was handed the conservative default.

                Nothing said so. The deck reads the same file by `names` and drew the measured
                curve correctly, marking a row *in use* that llama.cpp had never been told about
                — so MEASURE looked like it worked and changed nothing about how a model loads.
                Found by the owner comparing TIME IT against MEASURE: 14.0 tok/s against 20.6 for
                what the screen said was one setting, because the server was still on the
                conservative `q8_0` and the screen was showing the measured `full`.

                Two keys for one fact agree by luck, and these two could not agree at all.
            */
            epoch_engine::models::loadout::names(std::path::Path::new(&model.path))
                .and_then(|names| held.about(&names, &card, &build).map(|best| best.chose))
                .unwrap_or_else(|| {
                    epoch_engine::models::loadout::conservative(floor.next_power_of_two())
                })
        }
    }

    /// Say how one model should run: flash attention, and speculative decoding.
    ///
    /// **A choice, written down beside the measurements and never inside them.** What a curve
    /// found is a reading; this is what somebody decided. Applied the same way a chosen loadout
    /// is — llama.cpp is told at once, and loads it that way the next time it starts.
    ///
    /// Refuses before the server would. A speculation with a missing draft file, or a kind this
    /// build does not offer, fails minutes later as a load in the middle of a benchmark — and a
    /// run that dies on a typo has cost more than it measured.
    pub fn tune_model(
        &self,
        model: &str,
        tuning: epoch_engine::models::tuning::Tuning,
    ) -> Result<String, String> {
        if let Some(quarrel) = tuning.speculation.quarrel() {
            // A leftover draft file after switching kinds is worth saying and is not a refusal,
            // so only the ones that would stop a load are.
            if !quarrel.contains("ignored") {
                return Err(quarrel);
            }
        }
        let library = epoch_engine::models::generative::Library::here();
        let mut held = epoch_engine::models::tuning::Tunings::load(library.root());
        held.set(model, tuning.clone());
        held.save(library.root())?;
        self.tell_llama_cpp();

        let mut said = vec![format!("{model}:")];
        said.push(match tuning.flash_attn {
            Some(true) => "flash attention on".to_owned(),
            Some(false) => "flash attention off".to_owned(),
            None => "flash attention as llama.cpp decides".to_owned(),
        });
        if tuning.speculation.on() {
            said.push(format!("speculating with {}", tuning.speculation.kind));
        } else {
            said.push("no speculation".to_owned());
        }
        Ok(format!(
            "{}. llama.cpp has been told, and loads it this way the next time it starts.",
            said.join(", ")
        ))
    }

    /// Everything measured about one model on this machine, and the profiles it produces.
    pub fn optimization(&self, model: &str) -> Option<epoch_engine::profiles::Optimized> {
        let library = epoch_engine::models::generative::Library::here();
        let held = epoch_engine::profiles::Every::load(library.root());
        let one = held.of(model)?.clone();
        /*
            **A result from another card is not a fact about this one.** It is kept — a machine
            gets its card back and the measurements are still true of it — and it is not offered
            as a profile here. The most convincing kind of invented gauge is a real reading of a
            different thing.
        */
        let machine = epoch_engine::models::machine::Machine::measure();
        one.taken_here(
            &machine.gpu.unwrap_or_default(),
            &epoch_engine::models::runtimes::llama_build(),
            "llama_cpp",
        )
        .then_some(one)
    }

    /// A short reading of the configuration this model has right now.
    ///
    /// **Not the search.** This answers *how does what I have behave* in under a minute; the
    /// search answers *what should I have* in twenty. One button that did both would make the
    /// cheap question cost the expensive one's time.
    pub fn quick_test(&self, model: &str) -> Result<epoch_engine::optimize::Quick, String> {
        let (endpoint, named) = self.llama_cpp_serving(model)?;
        // The context this model is actually loaded with, which is what a reading of *the
        // configuration I have right now* has to use. Falling back to the standard one would
        // measure a configuration nobody is running.
        let library = epoch_engine::models::generative::Library::here();
        let held = epoch_engine::models::loadout::Loadouts::load(library.root());
        let machine = epoch_engine::models::machine::Machine::measure();
        let context = held
            .about(
                model,
                &machine.card(),
                &epoch_engine::models::runtimes::llama_build(),
            )
            .map(|it| it.chose.context)
            .unwrap_or(epoch_engine::suite::STANDARD_CONTEXT);

        let ask = epoch_engine::bench::Ask {
            context,
            ..epoch_engine::bench::Ask::default()
        };
        let watch = epoch_engine::models::load::Watch::begin();
        let measured = epoch_engine::bench::measure(&endpoint, &named, &ask, &|_, _| {});
        let load = watch.stop();
        let (finished, asked) = measured.stability();

        Ok(epoch_engine::optimize::Quick {
            model: model.to_owned(),
            generation: measured.generation(),
            prompt: measured.prompt(),
            first_token_ms: measured.first_token_ms(),
            context,
            vram_used: load.vram_used.map(|it| it.peak as u64),
            ram_used: load.ram_used.map(|it| it.peak as u64),
            gpu_percent: load.gpu_percent.map(|it| it.mean),
            cpu_percent: load.cpu_percent.map(|it| it.mean),
            healthy: finished == asked && measured.generation().is_some(),
            failures: measured.failures(),
        })
    }

    /// Whether this model has an MTP head inside it.
    ///
    /// **Read from the file, never from its name.** Measured 2026-08-31: an MTP artefact carries
    /// `<arch>.nextn_predict_layers` and a `blk.N.nextn.*` block, and two repositories publish
    /// files with identical names where only one of them does.
    ///
    /// It is what decides whether `draft-mtp` needs a second file. Without it, Epoch would refuse
    /// the technique to exactly the models built for it, for want of a file they do not need.
    fn carries_its_own_mtp(&self, model: &str) -> bool {
        self.shared_weights()
            .into_iter()
            .find(|it| it.name == model)
            .map(|it| {
                epoch_engine::models::artifact::Identity::read(std::path::Path::new(&it.path))
                    .has_mtp()
            })
            .unwrap_or(false)
    }

    /// Where llama.cpp is serving one model, and what it calls it there.
    fn llama_cpp_serving(&self, model: &str) -> Result<(String, String), String> {
        let seen = epoch_engine::models::runtimes::look_for(
            epoch_engine::models::runtimes::Runtime::LlamaCpp,
        );
        if !seen.serving {
            return Err("llama.cpp is not answering.".to_owned());
        }
        let named = epoch_engine::models::runtimes::offered_as(model, &seen.models)
            .ok_or_else(|| format!("llama.cpp is not serving '{model}'."))?;
        Ok((seen.endpoint, named))
    }

    /// What a control run is a measurement *of*.
    ///
    /// ## Why a bare number was not enough
    ///
    /// Measured 2026-08-31: the machine's remembered healthy figure was `46.0` and a timestamp.
    /// Nothing on it said which artefact, which build, which context, which cache or what
    /// workload, so comparing a fresh `40.1` against it was not science — it was a memory with a
    /// decimal point. Worse, the two llama.cpp builds on this machine differ by 4.5x because one
    /// is CUDA and one is Vulkan, so *the same model on the same card* can honestly produce two
    /// numbers that look like a fault.
    ///
    /// **Everything here is read, never assumed**, and anything that cannot be read stays `None`
    /// — which `Fingerprint` treats as *unrecorded*, never as *the same as yours*.
    fn fingerprint(
        &self,
        model: &str,
        loadout: &epoch_engine::models::loadout::Loadout,
        tuning: &epoch_engine::models::tuning::Tuning,
        protocol: &epoch_engine::protocol::Protocol,
    ) -> epoch_engine::reference::Fingerprint {
        let machine = epoch_engine::models::machine::Machine::measure();
        /*
            **The file itself, not its name.** Two repositories publish
            `Qwen3.6-35B-A3B-UD-IQ4_XS.gguf` and the two files are 17,730,509,792 and
            18,209,036,576 bytes — one carries an MTP head and one does not. Named alike, they are
            two models, and a reference taken on one may not gate the other.

            `artifactBytes` is therefore required for comparability: the size is the cheap half of
            *is this the same file* (`loadout::names`' reasoning, and the reason a hash is not
            taken on a path somebody is waiting on). `None` where the shelf has no row for it,
            which reads as unrecorded and refuses the comparison rather than guessing at it.
        */
        let known = self
            .shared_weights()
            .into_iter()
            .find(|it| {
                epoch_engine::models::runtimes::shelf_name(&it.name)
                    == epoch_engine::models::runtimes::shelf_name(model)
            })
            .map(|it| {
                (
                    it.bytes,
                    epoch_engine::models::artifact::Identity::read(std::path::Path::new(&it.path)),
                )
            });
        epoch_engine::reference::Fingerprint {
            artifact: Some(model.to_owned()),
            artifact_bytes: known.as_ref().map(|(bytes, _)| *bytes),
            logical_model: known.as_ref().map(|(_, id)| id.logical().to_owned()),
            mtp: known.as_ref().map(|(_, id)| id.has_mtp()),
            runtime: Some("llama_cpp".to_owned()),
            // Fields, and a sentence built from them. The raw `--version` line had been stored
            // as identity, `version: ` prefix and all, and a second record stored the same build
            // without it — two spellings of one fact, in a reference id.
            build: {
                let one = epoch_engine::models::identity::Build::read(
                    "llama_cpp",
                    &epoch_engine::models::runtimes::llama_build(),
                );
                one.known().then(|| one.display())
            },
            // Which GPU backend this build carries — the 4.5x above. `None` where the program
            // would not say, because a guess here turns a package difference into a fault.
            backend: epoch_engine::models::runtimes::llama_backend(),
            gpu: machine.gpu.clone(),
            driver: epoch_engine::models::machine::driver(),
            os: Some(std::env::consts::OS.to_owned()),
            context: Some(loadout.context),
            cache: Some(loadout.cache.id().to_owned()),
            // The program's own third state. `auto` is what it does when nobody said, and it is
            // a real answer rather than a missing one.
            flash_attention: Some(match tuning.flash_attn {
                Some(true) => "on".to_owned(),
                Some(false) => "off".to_owned(),
                None => "auto".to_owned(),
            }),
            // A stable id, never a `Debug` rendering — that made the formatter part of the
            // record's meaning, so adding one field would silently stop every stored value
            // matching.
            speculation: Some(epoch_engine::models::identity::speculation_id(
                &tuning.speculation,
            )),
            offload: tuning.placement.override_tensor.clone(),
            gpu_layers: tuning.placement.gpu_layers,
            // The workload, named and versioned, so a changed benchmark cannot masquerade as a
            // changed machine.
            workload: Some(format!(
                "{} prompts × {} tokens",
                epoch_engine::bench::Ask::default().prompts.len(),
                epoch_engine::bench::Ask::default().tokens
            )),
            // **How it was measured**, and not a boolean: two different instruments are both
            // "instrumented", and the hidden variable comes straight back under a name that
            // looks like it was handled.
            measurement_protocol: Some(protocol.id()),
            // **Which experiment this governs**, named rather than inferred. A figure for the
            // 32K standard control must not later gate a 64K run, an MTP artefact or an IQ2
            // quantisation — each of which already differs on a field above, so this is the
            // half that survives somebody adding a field and forgetting to require it.
            control: Some(epoch_engine::optimize::STANDARD_CONTROL.to_owned()),
            benchmark_version: Some(epoch_engine::bench::VERSION),
            // llama.cpp's own defaults, which Epoch does not set for a control run. Named rather
            // than left blank: *the server decided* is a fact, and it is the same fact each time.
            sampler: Some("server default".to_owned()),
            ..Default::default()
        }
    }

    /// Hand the card back when a measurement is over.
    ///
    /// ## A benchmark is not a turn
    ///
    /// `KEEP` holds the last speaker resident so the next message is instant, and it is right:
    /// the alternative is about nineteen seconds on every single message. **A benchmark is none
    /// of that.** Nobody is talking to this model, the person who pressed the button is reading
    /// numbers, and the run is over — so 12 GB stays on the card for a conversation that is not
    /// going to happen.
    ///
    /// It is also the state the *next* measurement starts from, which is the whole subject of
    /// this session: a control taken on a card that is already holding the previous candidate is
    /// not a control of the same machine.
    ///
    /// The rule and its wording are `time_model`'s, one function up, arrived at for the same
    /// reason: **only if this was what loaded it.** A model a character is mid-conversation with
    /// was already resident, and taking it away to tidy up after a measurement would cost that
    /// conversation a reload it did not ask for.
    ///
    /// Which is why `was_resident` is asked **before** the run and passed in. Asked afterwards it
    /// is always `true` — the run just loaded it — so the guard would read as a check while
    /// answering a question with only one possible answer. Read from the runtime's own word for
    /// what is on the card (`resident`), never from what it *offers*.
    fn holding_now(named: &str) -> bool {
        epoch_engine::models::runtimes::look_for(epoch_engine::models::runtimes::Runtime::LlamaCpp)
            .resident
            .iter()
            .any(|it| it == named)
    }

    fn stop_holding(&self, named: &str, was_resident: bool) {
        if !was_resident && Self::holding_now(named) {
            epoch_engine::models::runtimes::let_go_of(
                epoch_engine::models::runtimes::Runtime::LlamaCpp,
                named,
            );
        }
    }

    /// Run the search: try what this build offers, and record what each one did.
    ///
    /*
        **BENCHMARK & OPTIMIZE.** Twenty-odd configurations, each of them a preset written, a
        router restarted and a model loaded, so it is long and it says what it is doing —
        `Progress` carries the words, the flags and the best so far, because a job of this length
        that speaks only at the end is one somebody kills at the halfway mark.

        Only llama.cpp. Ollama exposes no speculation and takes a cache type only when the server
        starts; LM Studio takes neither. A search there would end in settings nothing reads —
        ADR-0027's dead `Manual` mode.

        **It puts back exactly what the model had.** A measurement is Epoch's and the choice is
        the user's (ADR-0033); a search that left the last thing it tried in place would be
        choosing by the order of a loop.
    */
    /// What the machine is doing right now, before somebody commits half an hour to a benchmark.
    ///
    /// ## Advice, and never a gate
    ///
    /// The search already waits for a quiet machine and refuses to record a run taken through a
    /// busy one — that half is `hygiene`, and it works. What it cannot do is tell somebody
    /// *before* they walk away, and a benchmark that spends twenty minutes waiting for a
    /// compilation nobody knew about is twenty minutes of somebody's afternoon.
    ///
    /// **Measured, never a lecture.** *Close what you are not using* is advice anybody could
    /// write; *the CPU is at 34% and LM Studio is holding a model* is a reading, and it is the
    /// one somebody can act on. Where the machine is quiet this says so and gets out of the way.
    ///
    /// And *quiet* is not *empty*. The thresholds come from this machine measured in both states
    /// — 13% CPU while serving a model against 32% while compiling — so the ordinary background
    /// of a desktop is below them by design. A reference is minted on a machine with a browser
    /// and a wallpaper and a chat client running, because that is the machine.
    pub fn before_benchmarking(&self) -> Preflight {
        let busy = epoch_engine::models::hygiene::look();
        Preflight {
            quiet: !busy.too_busy(),
            says: busy.why(),
            cpu: busy.cpu,
            gpu: busy.gpu,
            vram_held: busy.vram_held,
            // Named individually, because *LM Studio is holding a model* is a thing somebody can
            // do something about and *the machine is busy* is not.
            neighbours: epoch_engine::models::hygiene::other_servers()
                .into_iter()
                .filter(|it| it.conflict())
                .map(|it| it.say())
                .collect(),
            busy,
        }
    }
    /// The second instrument: **how much conversation is this model worth giving?**
    ///
    /// ## Why it is a separate button
    ///
    /// `BENCHMARK & OPTIMIZE` answers *how should this model run* in about twenty-five minutes.
    /// This answers a different question, costs about three quarters of an hour, and is asked far
    /// less often. Folding them together would double the price of the question people actually
    /// ask.
    ///
    /// ## And why it could not exist until the workload did
    ///
    /// Measured 2026-09-01 on `gemma4-12b`, one 64K window: a short question answers at 48.5
    /// tok/s and 62K of prompt at 41.9. Every context reading Epoch had was the first kind, so
    /// the curve reported that a 64K window costs nothing — a correct reading of what it costs to
    /// *hold* one, offered as an answer about *using* one. Each rung here is measured with the
    /// window about nine tenths full, and what a row keeps is the server's own `prompt_n`.
    ///
    /// ## Resumable, because it is long
    ///
    /// Every rung is written to the model's record as it finishes, and a rung already measured is
    /// skipped. A ladder stopped by drift at 128K comes back and starts at 128K — the rows below
    /// it were each closed by a control that held, and a later collapse says nothing about a
    /// measurement that was already bracketed.
    pub fn scale_context(
        &self,
        model: &str,
        stop: &dyn Fn() -> bool,
        watching: &dyn Fn(&epoch_engine::optimize::Progress),
    ) -> Result<epoch_engine::profiles::Optimized, String> {
        /*
            **Wait for the router before asking it anything.**

            A tuning search ends by stopping the router and starting it again, so the ladder that
            now follows it asks a port that has not bound yet. Measured 2026-09-02 on
            `gemma-3-270m`: eight rows, session complete, **zero rungs** — refused with
            *llama.cpp is not answering* about a llama.cpp seconds from answering.

            Here rather than in the caller, because the dependency is this function's: anything
            that needs a serving router should wait for one rather than rely on whoever called it
            remembering to. `put_into_effect` already does exactly this after its own restart.

            Returns nothing on failure — `llama_cpp_serving` below says the same thing in its own
            words, and it says it about the model as well as the port.
        */
        let _ = wait_for_llama_cpp(std::time::Duration::from_secs(90));
        let (endpoint, named) = self.llama_cpp_serving(model)?;
        let library = epoch_engine::models::generative::Library::here();
        let machine = epoch_engine::models::machine::Machine::measure();

        let weights = self
            .shared_weights()
            .into_iter()
            .find(|it| it.name == model)
            .ok_or_else(|| format!("{model} is not on this machine."))?;
        let path = std::path::PathBuf::from(&weights.path);
        let trained = epoch_engine::models::gguf::read(&path)
            .ok()
            .and_then(|header| header.trained_context())
            .and_then(|it| u32::try_from(it).ok())
            .unwrap_or(epoch_engine::suite::STANDARD_CONTEXT);

        /*
            **A ladder scales the winner, not the default.** Which configuration to carry up is
            what BENCHMARK & OPTIMIZE answered, and measuring the ladder on an unconfigured model
            would produce a curve about a machine nobody runs.
        */
        let mut found = self.optimization(model).ok_or_else(|| {
            format!(
                "Nothing has been measured for {model} yet. BENCHMARK & OPTIMIZE first: this \
                 scales the configuration that search found, and there is not one."
            )
        })?;
        let carry = found
            .recommended(epoch_engine::suite::STANDARD_CONTEXT)
            .ok_or_else(|| {
                format!(
                    "The last search for {model} produced no configuration it could stand \
                     behind, so there is nothing to scale."
                )
            })?
            .clone();

        /*
            **Asked before measuring.** A rung the fitter refuses costs one model load to skip and
            several minutes to discover by failing — and *256K does not fit here* is worth as much
            on a screen as a number would be.
        */
        watching(&epoch_engine::optimize::Progress {
            stage: epoch_engine::optimize::Stage::CheckingHardware,
            model: model.to_owned(),
            machine: machine.card(),
            done: 0,
            total: 0,
            about: "Asking what this card can hold".to_owned(),
            technical: String::new(),
            best: None,
        });
        let rungs = epoch_engine::optimize::feasible(&path, trained);
        let worth: Vec<epoch_engine::optimize::Rung> = rungs
            .iter()
            .filter(|it| it.worth_measuring())
            .cloned()
            .collect();
        if worth.is_empty() {
            return Err(format!(
                "This card cannot hold {model} at any window it was trained for."
            ));
        }

        let mut session = epoch_engine::session::Session {
            artifact: model.to_owned(),
            gpu: machine.gpu.clone().unwrap_or_default(),
            build: epoch_engine::models::runtimes::llama_build(),
            runtime: "llama_cpp".to_owned(),
            ..epoch_engine::session::Session::default()
        };

        // Borrowed for the length of the ladder and given back at the end, however it ends.
        let was = self.how_it_is_set(model);

        let total = worth.len();
        let mut opening: Option<String> = None;
        for (n, rung) in worth.iter().enumerate() {
            if stop() {
                break;
            }
            /*
                **A rung already measured is not measured again.** This is the whole of resuming:
                the record is the state, and a row carrying a filled reading at this window was
                closed by its own control when it was taken.
            */
            if found
                .tried
                .iter()
                .any(|it| it.loadout.context == rung.context && it.filled.is_some())
            {
                continue;
            }

            let step = epoch_engine::optimize::Step {
                about: format!("Context {} {}K, filled", '\u{00b7}', rung.context / 1024),
                technical: format!("-c {}", rung.context),
                loadout: epoch_engine::models::loadout::Loadout {
                    context: rung.context,
                    cache: carry.loadout.cache,
                },
                tuning: carry.tuning.clone(),
                offload: carry.offload.clone(),
                gpu_layers: carry.gpu_layers,
            };

            // The control before a rung is the one that closed the last, so the chain reads
            // straight through and no control is taken twice.
            if opening.is_none() {
                watching(&epoch_engine::optimize::Progress {
                    stage: epoch_engine::optimize::Stage::EstablishingBaseline,
                    model: model.to_owned(),
                    machine: machine.card(),
                    done: n,
                    total,
                    about: "Control: establishing what a healthy run looks like".to_owned(),
                    technical: String::new(),
                    best: None,
                });
                let control = self.standard_control_step();
                let seen = self.controlled(model, &endpoint, &named, &control, "openingControl");
                /*
                    **Judged against whatever governs, exactly as the search is.**

                    This passed `None` and took the band from its own opening control, which is
                    the honest fallback where nothing governs and the wrong answer where something
                    does: a ladder for a model with a reference on file would have judged its
                    rungs against one afternoon's control instead of against three fresh
                    calibrations.
                */
                let shape = self.fingerprint(
                    model,
                    &control.loadout,
                    &control.tuning,
                    &epoch_engine::protocol::STANDARD_CONTROL_V2,
                );
                let governing = epoch_engine::sessions::Kept::load(library.root())
                    .governing_reference_for(&shape.key())
                    .and_then(|it| it.band().map(|(band, _)| band));
                session.open(&seen.rates, governing, epoch_engine::now_ms());
                opening = Some(seen.id);
            }

            watching(&epoch_engine::optimize::Progress {
                stage: epoch_engine::optimize::Stage::TestingConfigurations,
                model: model.to_owned(),
                machine: machine.card(),
                done: n,
                total,
                about: step.about.clone(),
                technical: step.technical.clone(),
                best: None,
            });
            self.put_into_effect(model, &step)?;

            /*
                **Nine tenths, and the tenth is the answer.** A window filled to the brim leaves
                no room for the model to write, and llama.cpp refuses rather than truncating —
                measured, a 72K prompt in a 65,536 window answered `400`. What is left is the
                reply.
            */
            let ask = epoch_engine::bench::Ask {
                context: rung.context,
                fill: Some(rung.context * 9 / 10),
                ..epoch_engine::bench::Ask::default()
            };
            let watch = epoch_engine::models::load::Watch::begin();
            let measured = epoch_engine::bench::measure(&endpoint, &named, &ask, &|_, _| {});
            let load = watch.stop();
            let mut one = epoch_engine::optimize::recorded(
                &step,
                &measured,
                &load,
                None,
                0,
                0,
                None,
                // The ladder is the one workload that fills the window on purpose.
                true,
                epoch_engine::now_ms(),
            );

            watching(&epoch_engine::optimize::Progress {
                stage: epoch_engine::optimize::Stage::ValidatingStability,
                model: model.to_owned(),
                machine: machine.card(),
                done: n + 1,
                total,
                about: format!("Control: did the machine survive {}", step.about),
                technical: String::new(),
                best: None,
            });
            let control = self.standard_control_step();
            let closing = self.controlled(model, &endpoint, &named, &control, "sentinel");
            let rate = epoch_engine::models::health::median(&closing.rates);
            let held = if session.check(&step.about, rate, epoch_engine::now_ms()) {
                epoch_engine::profiles::Held::Reproduced
            } else {
                epoch_engine::profiles::Held::Failed
            };
            one.bracket = Some(epoch_engine::profiles::Bracket {
                before: opening.clone(),
                before_held: epoch_engine::profiles::Held::Reproduced,
                after: Some(closing.id.clone()),
                after_held: held,
            });
            opening = Some(closing.id);

            /*
                **Written as it finishes.** Three quarters of an hour of work that only lands at
                the end is three quarters of an hour a failed sentinel throws away.
            */
            found.tried.push(one);
            let mut every = epoch_engine::profiles::Every::load(library.root());
            every.set(model, found.clone());
            let _ = every.save(library.root());

            if !held.ok() {
                session.drifted("A control stopped reproducing partway up the ladder.");
                break;
            }
        }

        self.set_it_back(model, was);
        /*
            **A record may not hold two verdicts about one press.**

            The ladder replaces `session` with its own — right, because it is the more recent run
            and the one whose note explains anything that went wrong. It left `usable` alone,
            which was harmless while this was a separate button and is not now: a ladder whose
            control stopped reproducing would leave the record saying *these rows may become
            profiles* over a session saying *the machine stopped being itself*.

            The rows themselves were never at risk — each rung carries its own `Bracket` and a
            contaminated one is excluded by it. This is the belt to that braces, and it is one
            line: a press is usable if **both** halves were.
        */
        found.usable = found.usable && session.results_are_usable();
        found.session = Some(session);
        // A record of a refusal must not outlive the refusal. Reaching here means the ladder ran,
        // so a note from a previous attempt is now a sentence about a hole that is filled.
        found.ladder = None;
        let mut every = epoch_engine::profiles::Every::load(library.root());
        every.set(model, found.clone());
        every.save(library.root())?;
        Ok(found)
    }

    /// The one step every control is taken under: `-c` and nothing else.
    ///
    /// **Its own function because three places build it.** Two of them building it slightly
    /// differently is how a reference and a control come to be 11% apart and about to be
    /// compared, which this codebase has already paid for once.
    fn standard_control_step(&self) -> epoch_engine::optimize::Step {
        epoch_engine::optimize::plan(
            epoch_engine::optimize::Phase::Standard,
            epoch_engine::suite::STANDARD_CONTEXT,
            epoch_engine::suite::STANDARD_CONTEXT,
            &[epoch_engine::models::loadout::Cache::F16],
            &[],
            None,
            // Nothing is asked because nothing is needed: the control writes `-c` alone.
            &epoch_engine::models::accepts::Accepts::unasked(),
            epoch_engine::models::runtimes::FitClass::Unknown,
        )
        .remove(0)
    }

    pub fn optimize(
        &self,
        model: &str,
        phase: epoch_engine::optimize::Phase,
        stop: &dyn Fn() -> bool,
        watching: &dyn Fn(&epoch_engine::optimize::Progress),
    ) -> Result<epoch_engine::profiles::Optimized, String> {
        use epoch_engine::models::loadout::Cache;

        let (endpoint, named) = self.llama_cpp_serving(model)?;
        let library = epoch_engine::models::generative::Library::here();
        let machine = epoch_engine::models::machine::Machine::measure();
        let build = epoch_engine::models::runtimes::llama_build();

        let weights = self
            .shared_weights()
            .into_iter()
            .find(|it| it.name == model)
            .ok_or_else(|| format!("{model} is not on this machine."))?;
        let trained = epoch_engine::models::gguf::read(std::path::Path::new(&weights.path))
            .ok()
            .and_then(|header| header.trained_context())
            // A window is a `u32` everywhere it is used, and a header reporting something that
            // does not fit in one is a header nobody should be sized against.
            .and_then(|it| u32::try_from(it).ok())
            .unwrap_or(epoch_engine::suite::STANDARD_CONTEXT);

        // What llama.cpp itself says fits. Asked once, at the standard context, because it costs
        // a model load and the answer is about the model rather than about the search.
        /*
            **One run of the fitter, two answers**: how this model sits on this card, and the
            arguments that make it sit that way. Asked once, because it costs a model load.
        */
        let sits = epoch_engine::models::runtimes::fitting(
            std::path::Path::new(&weights.path),
            epoch_engine::suite::STANDARD_CONTEXT,
        );
        let fitted = sits.args.clone();

        let offered: Vec<String> = epoch_engine::models::tuning::SPEC_TYPES
            .iter()
            .map(|it| (*it).to_owned())
            .collect();
        let draft = epoch_engine::models::runtimes::draft_beside(&shelf_dir(), model);
        let carries_mtp = self.carries_its_own_mtp(model);
        /*
            One length per kind rather than the whole ladder: the ladder belongs to SWEEP
            SPECULATION, which is the tool for that question. Here the axis is *which kinds help
            at all*, and 2 is what ggml-org's own MTP README recommends starting at.

            **`every_kind`, not `at_length`.** The latter skips the first kind, because in the
            sweep it has already been measured across the ladder — and borrowing it here meant the
            search of an MTP artefact ran ten configurations without ever trying `draft-mtp`.
        */
        let speculations =
            epoch_engine::spec::every_kind(&offered, draft.as_deref(), carries_mtp, 2);

        /*
            **What this build accepts, asked once, from the build.**

            The two cache types were written here as a literal and the planner wrote
            `--flash-attn` unconditionally — true of the llama.cpp on this machine the day they
            were written, and not a measurement. This machine has two llama.cpp builds that differ
            (CUDA in `~/.llama/bin`, Vulkan from winget), which is the argument in one line: a
            candidate assembled from what is conceivable eventually names a flag the program
            rejects, two minutes into a search, as a failure that reads like a finding.

            `--help` starts no model and touches no card. `unasked` where it could not be run,
            and everything Epoch knows how to measure is offered — because *nobody asked* is not
            evidence that a flag is missing.
        */
        let accepts = epoch_engine::models::accepts::of_llama_cpp()
            .unwrap_or_else(epoch_engine::models::accepts::Accepts::unasked);
        let not_offered: Vec<String> = epoch_engine::optimize::unavailable(&accepts)
            .into_iter()
            .map(|one| format!("Not offered: {one}."))
            .collect();

        let steps = epoch_engine::optimize::plan(
            phase,
            epoch_engine::suite::STANDARD_CONTEXT,
            trained,
            &epoch_engine::optimize::caches_here(&accepts),
            &speculations,
            /*
                **The fitter already answered this.** A model that needed no arranging produces
                no arguments at all — `-ngl -1` and no `-ot`, measured on `gemma4-12b` — so there
                is nothing here to plan and nothing to filter out. Where it did arrange
                something, `FitClass` decides whether the candidate goes first or last.
            */
            fitted
                .as_ref()
                .map(|it| (it.override_tensor.clone(), it.gpu_layers)),
            &accepts,
            sits.class,
        );
        /*
            **The split llama.cpp itself worked out**, recorded against the step that asked for
            it and against no other. Every other step leaves it `None`, which is *nobody asked* —
            reporting the fitted split beside a configuration that was never given the fitted
            arguments would be the most convincing kind of wrong reading.
        */
        let placement = fitted
            .as_ref()
            .and_then(|it| it.model_on_gpu.zip(it.model_on_host));

        let was_tuned = epoch_engine::models::tuning::Tunings::load(library.root()).of(model);
        // The window and cache a candidate borrows are given back when the search ends, the same
        // as the tuning above: what the last candidate happened to be is not a choice anybody
        // made.
        let was_loaded = self.how_it_is_set(model);
        let mut found: Vec<epoch_engine::profiles::Configuration> = Vec::new();
        let mut baseline: Option<epoch_engine::profiles::Configuration> = None;
        let mut prints: Vec<String> = not_offered;

        let total = steps.len();
        /*
            **The control, taken before anything and again after every candidate.** Its readings
            are what the tolerance is derived from, so a slower machine is judged as itself.
        */
        let first = steps.first().cloned().unwrap_or_else(|| {
            epoch_engine::optimize::plan(
                phase,
                epoch_engine::suite::STANDARD_CONTEXT,
                trained,
                &[Cache::F16],
                &[],
                None,
                &accepts,
                sits.class,
            )
            .remove(0)
        });
        let mut session = epoch_engine::session::Session {
            artifact: model.to_owned(),
            gpu: machine.gpu.clone().unwrap_or_default(),
            build: build.clone(),
            runtime: "llama_cpp".to_owned(),
            ..epoch_engine::session::Session::default()
        };

        /*
            **The search does not begin on a machine that no longer reproduces itself.**

            Measured 2026-08-31: a control opened at 28.0 tok/s where this machine's healthy figure
            is about 46, and twenty configurations were then measured against a state that had
            already gone. Starting is the expensive mistake — every row after it is a measurement
            of the damage.

            The reference is what *this* machine produced while it was known healthy, per artefact
            and build. Nothing ever measured here allows the search: the first one is what
            establishes the reference, and refusing it for want of the thing it produces would make
            Epoch unable to start at all.
        */
        let kept_sessions = epoch_engine::sessions::Kept::load(library.root());
        // What this control is a measurement *of*, so a later one can be compared against it
        // rather than against a bare number and a date.
        let shape = self.fingerprint(
            model,
            &first.loadout,
            &first.tuning,
            &epoch_engine::protocol::STANDARD_CONTROL_V2,
        );
        /*
            **The button works out what it needs.** A user does not choose between calibrating and
            checking; the answer is a property of the record — is there a reference, does it
            govern, is it about this experiment — and Epoch reads it.

            `governing_reference_for` keys on the fingerprint, so a changed driver, build or backend
            simply finds nothing and the search calibrates instead of comparing across two
            machines. That is the same lookup a person would have had to do by hand, and the same
            one they would have got wrong the first time a build changed under them.
        */
        /*
            **The authority, and separately the newest.** These are two questions and this used to
            ask one: it took the newest and filtered it, so a retired record sitting above a good
            one meant *no governing reference exists*.

            Measured 2026-09-01: `R-002` governing with a superseded `R-003` above it sent a
            search off to calibrate from scratch against an authority one row down. Which
            reference governs is decided by the reference lifecycle — standing, provenance,
            comparability — and never by the order of a vector.

            `told` is still the newest one, governing or not, because it is the subject of a
            *sentence*: with the only reference retired, saying *nothing has been measured for
            this model* about a machine with fifteen recorded runs would be false. `what_is_needed`
            reads it and names which of the reasons it is.
        */
        let reference0 = kept_sessions.governing_reference_for(&shape.key()).cloned();
        let mut reference = reference0;
        let told = kept_sessions
            .history_of(&shape.key())
            .first()
            .map(|it| (*it).clone());

        /*
            **And it takes the baseline itself.** Announcing that a calibration is needed and then
            measuring candidates anyway would be the old behaviour with a caption.

            `REPRODUCTION_POLICY_V1` says how many fresh calibrations a reference is built from,
            and the count is the policy's rather than this function's: a series is what answers
            *does a fresh process reproduce a state*, and a single calibration cannot answer the
            question it is asking.

            **Nothing is minted from a series that cannot support one.** A process that could not
            hold still is a problem below the level a band describes, and `Series::mint` refuses
            it — leaving the observations on record and the search without a reference, which is
            `ReferenceRequired` rather than a guess.
        */
        const POLICY: &epoch_engine::reference::ReproductionPolicy =
            &epoch_engine::reference::REPRODUCTION_POLICY_V1;
        if let epoch_engine::orchestrate::Needs::Calibration { because } =
            epoch_engine::orchestrate::what_is_needed(told.as_ref(), &shape)
        {
            let mut took: Vec<String> = Vec::new();
            for n in 0..POLICY.calibrations {
                if stop() {
                    // A part-finished series cannot mint: `Series::mint` wants the policy's own
                    // count, so stopping here leaves the observations on record and no reference.
                    break;
                }
                watching(&epoch_engine::optimize::Progress {
                    stage: epoch_engine::optimize::Stage::EstablishingBaseline,
                    model: model.to_owned(),
                    machine: machine.card(),
                    done: 0,
                    total,
                    about: format!(
                        "Establishing a baseline — {because}. Calibration {} of {}.",
                        n + 1,
                        POLICY.calibrations,
                    ),
                    technical: String::new(),
                    best: None,
                });
                took.push(
                    self.controlled(model, &endpoint, &named, &first, "calibration")
                        .id,
                );
            }

            /*
                **Exactly the ones this run took**, by id.

                `observations_of` answers *everything ever recorded for this kind of experiment* —
                right for a history, wrong for a series. Measured 2026-09-01: three fresh
                calibrations agreed to 0.9% (48.17, 47.99, 47.73) and minting refused, because the
                set it was handed also held an observation from two hours earlier whose runs spanned
                5.24%. The fingerprint says what kind of experiment; it does not say when.
            */
            let mut kept = epoch_engine::sessions::Kept::load(library.root());
            let seen = kept.these(&took);
            let series = epoch_engine::reference::Series::of(&seen);
            let id = kept.next_reference_id();
            match series.mint(&id, &seen, POLICY, POLICY.calibrations) {
                Ok(one) => {
                    reference = Some(one.clone());
                    kept.remember(one);
                    let _ = kept.save(library.root());
                }
                Err(why) => {
                    watching(&epoch_engine::optimize::Progress {
                        stage: epoch_engine::optimize::Stage::EstablishingBaseline,
                        model: model.to_owned(),
                        machine: machine.card(),
                        done: 0,
                        total,
                        about: format!("No baseline could be established: {}.", why.why()),
                        technical: String::new(),
                        best: None,
                    });
                }
            }
            // **The reload that was here did nothing and is gone.** It re-read the session
            // store after the calibration series and nothing below ever read the result, so it
            // was a refresh for a consumer that had moved. Recorded rather than silently
            // dropped: if a later reader needs fresh sessions, it has to load them itself.
        }

        /*
            **Calibrate first, then open against it.**

            The opening control used to run before the calibration series, which is backwards:
            a control is a comparison, and there was nothing to compare it with yet. Measured
            2026-09-01, the run took its opening control at 51.06 and *then* three calibrations
            at 48.17, 47.99 and 47.73 — so the number the session was opened on was the one
            reading in the set that the reference does not describe.
        */
        watching(&epoch_engine::optimize::Progress {
            stage: epoch_engine::optimize::Stage::EstablishingBaseline,
            model: model.to_owned(),
            machine: machine.card(),
            done: 0,
            total,
            about: "Control · establishing what a healthy run looks like".to_owned(),
            technical: first.technical.clone(),
            best: None,
        });
        let opened = self.controlled(model, &endpoint, &named, &first, "openingControl");
        let opening = opened.rates.clone();
        session.open(
            &opening,
            reference
                .as_ref()
                .and_then(|it| it.band().map(|(band, _)| band)),
            epoch_engine::now_ms(),
        );
        if session.state() == epoch_engine::session::State::Invalid {
            return Err(
                "the control produced no reading, so nothing could be compared \
                        against it."
                    .to_owned(),
            );
        }
        let control_now = epoch_engine::models::health::median(&opening);
        let verdict = epoch_engine::sessions::may_begin(control_now, reference.as_ref(), &shape);

        /*
            **Nothing to compare against is a reason to stop, not a reason to carry on.**

            The defect the first golden test found, and the only one it was looking for. `mint`
            refused, `may_begin` answered `Ok(Verdict::NoReference)`, and this checked `if let
            Err` — so a candidate was measured with no governing reference. Epoch used evidence it
            had no right to use, which is the single thing that run was supposed to catch.

            `Ok` was never a synonym for *may proceed*. `Verdict::may_compare` is, and it says so:
            only `Clean`.
        */
        /*
            **The reference labels the run; it no longer decides whether one happens.**

            It used to refuse: a control outside the reference's band paused the search and
            nothing was measured. That is right about one thing and wrong about the question.

            Measured 2026-09-01 on `gemma4:12b`: five controls inside twenty minutes at 51.4-51.9,
            reproducing a reference of 51.35 — then, after a ten-minute release build, 46.5 and
            49.1. Two searches refused, and the owner still had no profile. His argument, and it
            is the right one: a desktop is never in one state. A Windows update, an indexer, a
            browser reclaiming memory — none of it is static, and a gate that waits for the state
            a reference was minted in waits for something that does not come back.

            **A comparison needs the same machine, not a fast one.** If today's machine runs at
            46.5 and holds 46.5 through every candidate, each comparison in that run is valid and
            the winner is the right winner. The absolute figure being 9% below last week is a fact
            about the day, not about the configuration — so it is *recorded*, and the profile says
            what state it was measured in.

            **What still stops a run is unchanged, and it is the half that was carrying the
            weight.** Controls that stop reproducing *each other* invalidate the comparisons
            around them, and the search stops on the first one. The run this whole mechanism was
            built after — 2026-08-31, opening at 28.0 on a machine that gives 46 — was caught by
            exactly that: 28.0 falling to 25.5 against its own opening. The reference gate was not
            what saved it.

            The one risk this accepts, stated rather than hidden: a degraded state can change the
            *ranking* as well as the numbers — a machine slow because it is spilling to host
            memory favours whatever relieves the pressure. Blocking does not fix that; it only
            guarantees there is no answer. Saying so does.
        */
        let standing = match &verdict {
            Ok(one) => one.say(control_now.unwrap_or_default()),
            Err(why) => why.clone(),
        };
        if !verdict
            .as_ref()
            .is_ok_and(epoch_engine::sessions::Verdict::may_compare)
        {
            session.measured_in_an_unusual_state(&standing);
            watching(&epoch_engine::optimize::Progress {
                stage: epoch_engine::optimize::Stage::EstablishingBaseline,
                model: model.to_owned(),
                machine: machine.card(),
                done: 0,
                total,
                about: format!("Measuring in this state: {standing}"),
                technical: String::new(),
                best: None,
            });
        }
        /*
            This control is what later ones are judged against — **and it is written with the
            whole fingerprint**, not as a rate and a date.

            The one it replaces knew the artefact, the card, the build and the runtime and nothing
            about the context, the cache, the flash-attention state or the workload. Four of the
            things that decide what a throughput number means were missing, which is how a
            remembered `46.0` came to be compared against a fresh `40.1` with no way to tell
            whether the two were even about the same configuration.
        */
        /*
            **And it is not minted here.** This used to promote the opening control to a
            `PerformanceReference` the moment it validated, which is *a run becoming an authority
            by having run* — the one thing this subsystem exists to prevent.

            Measured 2026-09-01: a search calibrated three times, minted `R-002` at 47.52 from
            them, and then minted `R-003` at 48.61 from the single opening control — no sources,
            no policy, five runs in one process, and newer, so it governed everything afterwards.
            The three calibrations decided nothing.

            `Observation::validate` still exists and still refuses: it answers *is this reading
            good enough to be believed*, which the calibration series asks of each of its members.
            What it may not do on its own is confer authority. `Series::mint` does that, from as
            many fresh processes as `REPRODUCTION_POLICY_V1` requires.

            The observation is on record either way — `controlled` wrote it before this ran.
        */
        // Every control this session takes, in order. The sequence is what drift reads; a single
        // reading cannot show a slope.
        let mut seen_controls: Vec<f64> = epoch_engine::models::health::median(&opening)
            .into_iter()
            .collect();
        /*
            **Which control opens the next candidate.** The opening control of the search opens
            the first one; after that it is whichever control closed the one before, which is what
            makes the chain a chain rather than a list of pairs.
        */
        let mut opening_id: Option<String> = Some(opened.id.clone());

        // The baseline's healthy behaviour, filled in by the first step and read by every one
        // after it. `None` until then, and nothing can collapse against nothing.
        let mut healthy_rate: Option<f64> = None;
        let mut healthy_gpu: Option<f64> = None;

        // Whether the person stopped it, so the end can tell a cancelled search from a finished
        // one. `session.complete()` would otherwise turn `Clean` into `Complete` and quietly
        // claim a search that never ran its last candidates had run them.
        let mut was_cancelled = false;
        for (n, step) in steps.iter().enumerate() {
            if stop() {
                was_cancelled = true;
                break;
            }
            watching(&epoch_engine::optimize::Progress {
                stage: epoch_engine::optimize::Stage::TestingConfigurations,
                model: model.to_owned(),
                machine: machine.card(),
                done: n,
                total,
                about: step.about.clone(),
                technical: step.technical.clone(),
                best: best_so_far(&found),
            });

            /*
                **A run taken on a busy machine is not a run.** Measured 2026-08-31: a sequence
                taken while the same machine compiled Rust read 18.0 tok/s for a model that
                measures 45.8, and every other row in it was wrong by an unknown amount. The only
                reason that one was caught is that 18 is absurd.

                Checked before **every** step rather than once at the start: a twenty-minute
                search is exactly long enough for somebody to start a build in the middle of it.
                It waits rather than refusing, because a compilation finishes.

                **The router is stopped first, and that ordering is load-bearing.** The check
                asks whether somebody else is holding the card — and with the previous step's
                model still resident, the somebody else was Epoch. It refused the second step of
                its own search with *12.4 GB of the card is already in use*, which was the model
                under test.

                > A guard placed where it can see its own subject will eventually block the thing
                > it was written to protect. It has to run where the answer is only about
                > everything else.
            */
            let _ = epoch_engine::models::runtimes::stop_router(&shelf_dir());
            if let Err(busy) = epoch_engine::models::hygiene::wait_until_quiet(
                std::time::Duration::from_secs(300),
                &|seen| {
                    watching(&epoch_engine::optimize::Progress {
                        stage: epoch_engine::optimize::Stage::CheckingHardware,
                        model: model.to_owned(),
                        machine: machine.card(),
                        done: n,
                        total,
                        about: format!(
                            "System busy — waiting for a clean benchmark ({})",
                            seen.why()
                        ),
                        technical: String::new(),
                        best: best_so_far(&found),
                    });
                },
            ) {
                return Err(format!(
                    "the machine did not go quiet: {}. Nothing was recorded — a measurement \
                     taken through that is a measurement of it.",
                    busy.why()
                ));
            }

            /*
                **Postflight, and the reason it exists is that preflight cannot see this.**

                Measured 2026-08-31: one resident model answered at 45–46 tok/s for four cells and
                at 26–28 for the five after it, with no recovery. The machine was quiet the whole
                time. What changed was inside the *instance* — the driver evicted about 70 MiB of
                it into shared memory when another program claimed the card, and never migrated it
                back.

                So a run is valid only if it also **ended** in the state it began in, and a
                collapse is answered by throwing the instance away rather than the number:
                unload, let the card settle, load again, warm up, and take the configuration
                again. Retrying against the degraded instance would produce three consistent
                readings of something broken.

                After `ATTEMPTS`, the configuration is recorded as what it is — residency
                sensitive — rather than retried forever.
            */
            const ATTEMPTS: usize = 3;
            let mut discarded = 0usize;
            let mut collapses = 0usize;
            let mut one = None;

            for attempt in 0..ATTEMPTS {
                if attempt > 0 {
                    watching(&epoch_engine::optimize::Progress {
                        stage: epoch_engine::optimize::Stage::TestingConfigurations,
                        model: model.to_owned(),
                        machine: machine.card(),
                        done: n,
                        total,
                        about: format!(
                            "{} — model instance degraded, reloading (attempt {} of {})",
                            step.about,
                            attempt + 1,
                            ATTEMPTS
                        ),
                        technical: step.technical.clone(),
                        best: best_so_far(&found),
                    });
                    // Throw the instance away, and give the driver a moment to take its memory
                    // back before the next one asks for it.
                    let _ = epoch_engine::models::runtimes::stop_router(&shelf_dir());
                    std::thread::sleep(std::time::Duration::from_secs(8));
                }

                let ask = epoch_engine::bench::Ask {
                    context: step.loadout.context,
                    ..epoch_engine::bench::Ask::default()
                };
                let Ok(()) = self.put_into_effect(model, step) else {
                    let mut failed = epoch_engine::optimize::recorded(
                        step,
                        &epoch_engine::bench::Measured::default(),
                        &epoch_engine::models::load::Load::default(),
                        None,
                        discarded,
                        collapses,
                        None,
                        // A question, not a filled window: the short workload.
                        false,
                        epoch_engine::now_ms(),
                    );
                    failed.stable = false;
                    one = Some(failed);
                    break;
                };

                let watch = epoch_engine::models::load::Watch::begin();
                let measured = epoch_engine::bench::measure(&endpoint, &named, &ask, &|_, _| {});
                let load = watch.stop();

                let around = epoch_engine::models::health::Around {
                    gpu_mean: load.gpu_percent.map(|it| it.mean),
                    shared_before: load.shared_before,
                    shared_after: load.shared_after,
                    vram_before: load.vram_before,
                    vram_peak: load.vram_used.map(|it| it.peak as u64),
                };

                /*
                    Judged against **the baseline's own healthy behaviour**, never an absolute
                    number: 26 tok/s is a collapse on this card and a fine result on a laptop.
                    Nothing to compare against — the first step — can never be a collapse.
                */
                let degraded = epoch_engine::models::health::collapsed(
                    measured.generation(),
                    &around,
                    healthy_rate,
                    healthy_gpu,
                );
                if degraded && attempt + 1 < ATTEMPTS {
                    discarded += 1;
                    collapses += 1;
                    continue;
                }
                if degraded {
                    collapses += 1;
                }

                /*
                    The first step is the baseline, so its digests are what everything after is
                    compared against. A configuration at a *different context* legitimately writes
                    different text — the window changed, not the model — so only the ones sharing
                    the baseline's context are judged on it, and the rest report `None`, which is
                    unproven rather than wrong.
                */
                let same = if n == 0 {
                    prints = measured.fingerprints();
                    None
                } else if step.loadout.context != steps[0].loadout.context {
                    None
                } else {
                    let mine = measured.fingerprints();
                    (!prints.is_empty() && prints.len() == mine.len()).then(|| prints == mine)
                };

                one = Some(epoch_engine::optimize::recorded(
                    step,
                    &measured,
                    &load,
                    step.offload.is_some().then_some(placement).flatten(),
                    discarded,
                    collapses,
                    same,
                    // A question, not a filled window: the short workload.
                    false,
                    epoch_engine::now_ms(),
                ));
                break;
            }

            let Some(mut one) = one else { continue };
            if n == 0 {
                /*
                    **What every later step is judged against.** Not a constant: the baseline's own
                    median and the card utilisation it ran at. A configuration that later answers
                    at 59% of this while the card works a quarter harder has changed state.
                */
                healthy_rate = one.verdict.median.or(Some(one.generation));
                healthy_gpu = one.around.gpu_mean;
                baseline = Some(one.clone());
            }
            let about = step.about.clone();

            /*
                **The closing control, and the candidate is only kept once it has held.**

                It used to be `found.push(one)` and *then* the sentinel — so a candidate whose
                closing control failed was already in the results, unbracketed and eligible.
                Nothing downstream could see it: `usable()` filters on the verdict, on stability
                and on whether the answers changed, and none of those knows what happened to the
                machine afterwards.

                > **Measurement is not eligibility.** A reading can be perfect and still not be a
                > thing to recommend until something shows the machine was the same afterwards.

                **Every candidate, including the last.** The gate here was `if n + 1 < total`, so
                the final configuration was never closed at all — bracketed on one side by
                construction. There is no separate "final control"; the last candidate's closing
                control *is* it, which is one fewer special path to get wrong.
            */
            watching(&epoch_engine::optimize::Progress {
                stage: epoch_engine::optimize::Stage::ValidatingStability,
                model: model.to_owned(),
                machine: machine.card(),
                done: n + 1,
                total,
                about: format!("Control · did the machine survive {about}"),
                technical: first.technical.clone(),
                best: best_so_far(&found),
            });
            let closing = self.controlled(model, &endpoint, &named, &first, "sentinel");
            let rate = epoch_engine::models::health::median(&closing.rates);
            if let Some(one) = rate {
                seen_controls.push(one);
            }

            let slipping =
                epoch_engine::orchestrate::drift(&seen_controls, session.allowed_share());
            let held = if slipping.stops_the_search() {
                epoch_engine::profiles::Held::Failed
            } else if session.check(&about, rate, epoch_engine::now_ms()) {
                epoch_engine::profiles::Held::Reproduced
            } else {
                epoch_engine::profiles::Held::Failed
            };

            /*
                **The closing control of one candidate is the opening control of the next.** So
                the id appears twice in a session, and that repetition is the chain: History can
                walk control, candidate, control, candidate without joining anything.
            */
            one.bracket = Some(epoch_engine::profiles::Bracket {
                before: opening_id.clone(),
                before_held: epoch_engine::profiles::Held::Reproduced,
                after: Some(closing.id.clone()),
                after_held: held,
            });
            opening_id = Some(closing.id.clone());
            found.push(one);

            if held.ok() {
                continue;
            }

            // The candidate above keeps its measurement and loses its eligibility; every earlier
            // one was closed by a control that held and is untouched by this.
            if slipping.stops_the_search() {
                watching(&epoch_engine::optimize::Progress {
                    stage: epoch_engine::optimize::Stage::ValidatingStability,
                    model: model.to_owned(),
                    machine: machine.card(),
                    done: n + 1,
                    total,
                    about: slipping.say(&seen_controls).join(" "),
                    technical: String::new(),
                    best: best_so_far(&found),
                });
                session.drifted(&slipping.say(&seen_controls).join("\n"));
                break;
            }

            watching(&epoch_engine::optimize::Progress {
                stage: epoch_engine::optimize::Stage::ValidatingStability,
                model: model.to_owned(),
                machine: machine.card(),
                done: n + 1,
                total,
                about: "Recovering GPU state — the control stopped reproducing".to_owned(),
                technical: String::new(),
                best: best_so_far(&found),
            });
            let again = self.controlled(model, &endpoint, &named, &first, "recovery");
            let rate = epoch_engine::models::health::median(&again.rates);
            if session.check(&about, rate, epoch_engine::now_ms()) {
                session.recovered();
                // Recovery makes the *next* candidate's opening control this one, and leaves the
                // contaminated candidate contaminated: what came back is not evidence about what
                // happened during it.
                opening_id = Some(again.id.clone());
            } else {
                session.recovery_failed();
                break;
            }
        }

        // Back to what it had, whatever happened above — including a stop.
        let mut held = epoch_engine::models::tuning::Tunings::load(library.root());
        held.set(model, was_tuned);
        let _ = held.save(library.root());
        let _ = epoch_engine::models::runtimes::stop_router(&shelf_dir());
        let _ = self.start_router();

        /*
            **Cancelled is not complete.** `complete()` promotes `Clean` to `Complete`, so a
            search somebody stopped after two of twenty configurations was recorded as having
            finished — and `Complete` is one of the states whose rows may become profiles.

            Nothing is lost by cancelling: every candidate that ran was closed by its own control,
            and each one's `Bracket` already decides whether it may be recommended. What changes
            is only that the session stops claiming it got to the end.
        */
        if was_cancelled {
            session.cancelled();
        } else {
            session.complete();
        }
        /*
            **A search that reached the end is the strongest evidence a machine is well.**

            An older pause for this artefact was left standing while a fresh search ran eleven
            sentinels past it, every one reproducing, and the deck went on saying the machine
            needed a clean GPU state and possibly a reboot. That is a gauge contradicted by the
            product's own measurements taken minutes earlier — and it is the reading somebody
            acts on.

            Only a completed, clean session clears it: cancelled sessions and degraded ones fall
            through to the branch below, which pauses on what they actually found.
        */
        if !was_cancelled && session.state() == epoch_engine::session::State::Complete {
            let mut kept_sessions = epoch_engine::sessions::Kept::load(library.root());
            if kept_sessions.paused_for(model).is_some() {
                kept_sessions.cleared(model);
                let _ = kept_sessions.save(library.root());
            }
        }
        // A search that ended needing recovery is kept the same way one that never started is:
        // whole, with what the control read and what it was compared against.
        if session.state() == epoch_engine::session::State::RecoveryRequired {
            let card = epoch_engine::models::machine::Machine::measure();
            let mut kept_sessions = epoch_engine::sessions::Kept::load(library.root());
            kept_sessions.pause(epoch_engine::sessions::Paused {
                session: session.clone(),
                last_control: session.controls.last().and_then(|it| it.rate),
                clean_reference_id: reference.as_ref().map(|it| it.id.clone()),
                clean_reference_seen: reference.as_ref().map(|it| it.median),
                vram_used: card.vram_total.zip(card.vram_free).map(|(t, f)| t - f),
                shared_used: None,
                at: epoch_engine::now_ms(),
            });
            let _ = kept_sessions.save(library.root());
        }
        /*
            **A session that ended degraded produces no profiles.** Its rows are kept — they are
            the diagnosis, and the one that broke the machine is named — but nothing measured after
            the control stopped reproducing may become something somebody runs for hours.
        */
        self.set_it_back(model, was_loaded);
        let usable = session.results_are_usable();
        let optimized = epoch_engine::profiles::Optimized {
            model: model.to_owned(),
            gpu: machine.gpu.clone().unwrap_or_default(),
            build,
            runtime: "llama_cpp".to_owned(),
            baseline,
            tried: found,
            // A custom profile is the user's and survives a re-measurement.
            custom: self.optimization(model).and_then(|it| it.custom),
            // A fresh search has not attempted a ladder yet. The caller fills this in if it
            // tries and cannot.
            ladder: None,
            chosen: None,
            session: Some(session),
            usable,
        };
        let mut every = epoch_engine::profiles::Every::load(library.root());
        every.set(model, optimized.clone());
        every.save(library.root())?;
        Ok(optimized)
    }

    /// Whether an interrupted benchmark is waiting, and what it is waiting for.
    pub fn paused_benchmark(&self) -> Option<epoch_engine::sessions::Paused> {
        let library = epoch_engine::models::generative::Library::here();
        epoch_engine::sessions::Kept::load(library.root())
            .anything_paused()
            .cloned()
    }

    /// Throw away a paused benchmark. **The record, not the evidence.**
    ///
    /// Every control that pause was built from is an Observation and stays exactly where it was.
    /// What goes is the session and its banner — and it may go, because it gates nothing any
    /// more: a search measures in whatever state it finds and labels the result.
    pub fn dismiss_pause(&self, artifact: &str) -> Result<(), String> {
        let library = epoch_engine::models::generative::Library::here();
        let mut kept = epoch_engine::sessions::Kept::load(library.root());
        kept.cleared(artifact);
        kept.save(library.root())
    }

    /// Run **only the control**, and say whether this machine is itself again.
    ///
    /*
        **It does not resume anything.** The rows measured before the break and the rows after it
        came from two different machines, and joining them would produce exactly the table the
        control exists to prevent. A clean answer clears the pause; the next search starts from the
        beginning.
    */
    /// Measure this machine's healthy figure, and write down what it is a figure about.
    ///
    /// ## Calibration, which is not a search and not a check
    ///
    /// A clean-state check *compares*; this *establishes*. It is the only thing `ReferenceRequired`
    /// permits, and it is deliberately a separate press: Epoch measures and the user decides
    /// (ADR-0033), so a check that quietly minted the reference it had just failed to find would
    /// be deciding for them — and would make the very next check pass by construction.
    ///
    /// **The whole fingerprint or nothing.** A calibration that could not read its own build,
    /// card or backend would write another anecdote, and the point of this function is to stop
    /// producing those.
    pub fn calibrate(&self, model: &str) -> Result<String, String> {
        use epoch_engine::protocol::STANDARD_CONTROL_V2 as PROTOCOL;

        let (endpoint, named) = self.llama_cpp_serving(model)?;
        let library = epoch_engine::models::generative::Library::here();

        // The machine has to be quiet, exactly as a search's opening control does. A calibration
        // taken beside something else using the card is a number that will be compared with
        // others taken when the card was free.
        let busy = epoch_engine::models::hygiene::look();
        if busy.too_busy() {
            return Err(format!(
                "Not calibrating: {}. Every calibration in a series has to meet the machine in \
                 the same state.",
                busy.why(),
            ));
        }

        let first = epoch_engine::optimize::plan(
            epoch_engine::optimize::Phase::Standard,
            epoch_engine::suite::STANDARD_CONTEXT,
            epoch_engine::suite::STANDARD_CONTEXT,
            &[epoch_engine::models::loadout::Cache::F16],
            &[],
            None,
            // **Nothing is asked because nothing is needed.** This builds the standard control
            // alone: `-c` and nothing else, which every build that exists accepts. Reading a
            // help text to write no flag would be a measurement taken for its own sake.
            &epoch_engine::models::accepts::Accepts::unasked(),
            // And nothing is placed, for the same reason: the control is `-c` alone.
            epoch_engine::models::runtimes::FitClass::Unknown,
        )
        .remove(0);

        // **The same function a clean-state check and every sentinel use.** There is no
        // calibration-shaped variant: that difference is what made a reference and a control 11%
        // apart and about to be compared.
        // The same recording path as every other control. A calibration is not a special source
        // of truth; it is a standard control taken on purpose.
        let seen = self.controlled(model, &endpoint, &named, &first, "calibration");
        let id = seen.id.clone();

        /*
            **Recorded, and nothing is minted.**

            A calibration is an observation: *this is what happened this time*. Whether a fresh
            process reproduces a state at all is a question about a **series** of them, and it
            cannot be answered by the run that is asking it. Minting a reference here would make
            the very next check pass by construction — and it would make the first calibration the
            authority on the variability the second one exists to measure.
        */
        let fid = seen.fingerprint_id.clone();
        let kept = epoch_engine::sessions::Kept::load(library.root());

        let mut report = vec![
            format!("Observation: {id}"),
            format!("Protocol: {}", PROTOCOL.id()),
            format!("Fingerprint: {fid}"),
        ];
        if let Some(one) = seen.spread() {
            report.push(format!(
                "Median: {:.2} tok/s over {} runs",
                one.median, one.runs
            ));
            report.push(format!(
                "Runs: {}",
                seen.rates
                    .iter()
                    .map(|it| format!("{it:.2}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
            // Four quantities, four names. These three describe *these* runs; a tolerance is a
            // rule about a later one and is not computed here at all.
            report.push(format!(
                "Observed range: {:.2} tok/s ({:.2}%)",
                one.observed_range_tps, one.observed_range_pct,
            ));
            report.push(format!(
                "Max deviation: {:.2} tok/s ({:.2}%)",
                one.max_deviation_tps, one.max_deviation_pct,
            ));
            report.push(format!(
                "MAD: {:.3} tok/s ({:.2}%)",
                one.mad_tps, one.mad_pct
            ));
        } else {
            report.push("No reading at all.".to_owned());
        }

        let s = &seen.sequence;
        report.push(format!("Status: {}", seen.status().label()));
        report.extend(s.execution.describe());
        report.push(format!(
            "Lifecycle: {} \u{00b7} warmup runs {} \u{00b7} requests before measurement {}",
            s.lifecycle.clone().unwrap_or_else(|| "\u{2014}".to_owned()),
            s.warmup_runs.unwrap_or(0),
            s.requests_before_measurement.unwrap_or(0),
        ));
        if !s.warmup_rates.is_empty() {
            // Discarded from the median and kept as evidence: a warmup far from the measured runs
            // is the interesting case, and lifecycle is exactly what is under investigation.
            report.push(format!(
                "Warmup (discarded): {}",
                s.warmup_rates
                    .iter()
                    .map(|it| format!("{it:.2}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            ));
        }
        report.push(format!(
            "Model load: {}",
            s.router_age_ms
                .map(|it| format!("{:.1} s to first warmup", it as f64 / 1000.0))
                .unwrap_or_else(|| "\u{2014}".to_owned()),
        ));

        let o = &seen.observed;
        let gb = |bytes: Option<u64>| {
            bytes
                .map(|it| format!("{:.2} GB", it as f64 / 1e9))
                .unwrap_or_else(|| "\u{2014}".to_owned())
        };
        let show = |one: Option<f64>, unit: &str| {
            one.map(|it| format!("{it:.1}{unit}"))
                .unwrap_or_else(|| "\u{2014}".to_owned())
        };
        report.push(format!(
            "Prompt: {} \u{00b7} first token {}",
            show(o.prompt, " tok/s"),
            show(o.first_token_ms, " ms"),
        ));
        report.push(format!(
            "Dedicated VRAM peak: {} \u{00b7} shared GPU {} \u{2192} {} \u{00b7} RAM {}",
            gb(o.vram_used),
            gb(o.shared_before),
            gb(o.shared_after),
            gb(o.ram_used),
        ));
        report.push(format!(
            "GPU {} \u{00b7} CPU {}",
            show(o.gpu_percent, "%"),
            show(o.cpu_percent, "%"),
        ));

        // The series so far: within-process and between-process, kept apart.
        report.push(String::new());
        report.extend(kept.series_of(&fid).describe());
        report.push(String::new());
        report.push(
            "Nothing was minted and nothing was cleared. A reference is built from a series that \
             has been looked at."
                .to_owned(),
        );
        Ok(report.join("\n"))
    }

    /// Build a reference from the calibrations already taken. **A person's decision.**
    ///
    /// ## Why this is not the end of a calibration
    ///
    /// A calibration cannot answer the question it is asking — whether a fresh process reproduces
    /// a state is a question about a *series* of them. So calibrating records an observation and
    /// stops, and this is the separate act of saying *those agree; make them the authority*.
    ///
    /// The band comes from `REPRODUCTION_POLICY_V1`, named and versioned because a constant that
    /// decides whether hours of work may be trusted must be datable. It was chosen from three
    /// calibrations, and when there are dozens it should be replaced by that evidence.
    pub fn mint_reference(&self, model: &str, wanted: usize) -> Result<String, String> {
        use epoch_engine::protocol::STANDARD_CONTROL_V2 as PROTOCOL;

        let library = epoch_engine::models::generative::Library::here();
        let first = epoch_engine::optimize::plan(
            epoch_engine::optimize::Phase::Standard,
            epoch_engine::suite::STANDARD_CONTEXT,
            epoch_engine::suite::STANDARD_CONTEXT,
            &[epoch_engine::models::loadout::Cache::F16],
            &[],
            None,
            // **Nothing is asked because nothing is needed.** This builds the standard control
            // alone: `-c` and nothing else, which every build that exists accepts. Reading a
            // help text to write no flag would be a measurement taken for its own sake.
            &epoch_engine::models::accepts::Accepts::unasked(),
            // And nothing is placed, for the same reason: the control is `-c` alone.
            epoch_engine::models::runtimes::FitClass::Unknown,
        )
        .remove(0);
        let shape = self.fingerprint(model, &first.loadout, &first.tuning, &PROTOCOL);
        let fid = shape.key();

        let mut kept = epoch_engine::sessions::Kept::load(library.root());
        let observations = kept.observations_of(&fid);
        let series = epoch_engine::reference::Series::of(&observations);
        let id = kept.next_reference_id();

        let one = series
            .mint(
                &id,
                &observations,
                &epoch_engine::reference::REPRODUCTION_POLICY_V1,
                wanted,
            )
            .map_err(|why| {
                format!(
                    "No reference minted: {}.\n\n{}",
                    why.why(),
                    series.describe().join("\n"),
                )
            })?;

        let mut report = series.describe();
        report.push(String::new());
        report.push(format!("Reference: {} \u{00b7} fingerprint {fid}", one.id));
        report.push(format!(
            "Policy: {}",
            one.policy.clone().unwrap_or_default()
        ));
        report.push(format!("Sources: {}", one.sources.join(", ")));
        report.push(format!("Centre: {:.2} tok/s", one.median));
        if let Some((band, whence)) = one.band() {
            report.push(format!(
                "Reproduction band: {:.2} \u{2013} {:.2} tok/s (\u{00b1}{:.2}, {:.2}%)",
                band.middle - band.allowed,
                band.middle + band.allowed,
                band.allowed,
                100.0 * band.allowed / band.middle,
            ));
            report.push(format!("Band from: {}", whence.about()));
        }
        report.push(format!(
            "Evidence kept: {} rates from {} calibrations",
            one.rates.len(),
            one.sources.len(),
        ));
        kept.remember(one);
        kept.save(library.root())?;
        report.push(String::new());
        report.push(
            "Nothing was cleared. Run the clean-state check to compare a fresh control against it."
                .to_owned(),
        );
        Ok(report.join("\n"))
    }

    pub fn clean_state_check(&self, model: &str) -> Result<String, String> {
        use epoch_engine::protocol::STANDARD_CONTROL_V2 as PROTOCOL;
        use epoch_engine::reference::Reproduction;

        let (endpoint, named) = self.llama_cpp_serving(model)?;
        let library = epoch_engine::models::generative::Library::here();

        let first = epoch_engine::optimize::plan(
            epoch_engine::optimize::Phase::Standard,
            epoch_engine::suite::STANDARD_CONTEXT,
            epoch_engine::suite::STANDARD_CONTEXT,
            &[epoch_engine::models::loadout::Cache::F16],
            &[],
            None,
            // **Nothing is asked because nothing is needed.** This builds the standard control
            // alone: `-c` and nothing else, which every build that exists accepts. Reading a
            // help text to write no flag would be a measurement taken for its own sake.
            &epoch_engine::models::accepts::Accepts::unasked(),
            // And nothing is placed, for the same reason: the control is `-c` alone.
            epoch_engine::models::runtimes::FitClass::Unknown,
        )
        .remove(0);

        // **The same function the calibration ran.** One protocol, one instrument, one lifecycle
        // — which is the only reason the two numbers below are about the same thing.
        let seen = self.controlled(model, &endpoint, &named, &first, "retry");
        let shape = seen.fingerprint.clone();
        let Some(spread) = seen.spread() else {
            return Err("The control produced no reading at all.".to_owned());
        };
        let rate = spread.median;

        let mut kept = epoch_engine::sessions::Kept::load(library.root());
        let fid = shape.key();
        let history = kept.history_of(&fid);
        let reference = kept.governing_reference_for(&fid).cloned();

        let mut report = vec![
            format!("Current control: {rate:.2} tok/s over {} runs", spread.runs),
            format!("Protocol: {}", PROTOCOL.id()),
            format!("Fingerprint: {fid}"),
            format!(
                "Observed range: {:.2}% \u{00b7} max deviation: {:.2}% \u{00b7} MAD: {:.2}%",
                spread.observed_range_pct, spread.max_deviation_pct, spread.mad_pct,
            ),
        ];
        match &reference {
            Some(one) => {
                report.push(format!(
                    "Reference: {} \u{00b7} {:.2} tok/s",
                    one.id, one.median
                ));
                report.push(format!(
                    "Comparable: {}",
                    one.fingerprint.comparable_to(&shape).why()
                ));
                if let Some((band, whence)) = one.band() {
                    report.push(format!(
                        "Gate tolerance: \u{00b1}{:.2} tok/s ({:.2}%) \u{2014} {}",
                        band.allowed,
                        100.0 * band.allowed / band.middle,
                        whence.about(),
                    ));
                    report.push(format!(
                        "Reproducibility band: {:.2} \u{2013} {:.2} tok/s",
                        band.middle - band.allowed,
                        band.middle + band.allowed,
                    ));
                }
            }
            None => report.push("Reference: none usable for this experiment".to_owned()),
        }
        report.push(format!(
            "History for this experiment: {} measurement(s)",
            history.len(),
        ));
        for one in epoch_engine::models::hygiene::other_servers() {
            report.push(one.say());
        }

        /*
            **Two-sided.** Reproduction is equivalence, not *at least as fast*. A control 18%
            above its reference has not reproduced it — it has produced a different state, and
            comparing twenty configurations against a state that is not happening any more is the
            mistake this whole subsystem exists to prevent.
        */
        let verdict = reference
            .as_ref()
            .map(|one| one.reproduced_by(rate, &shape))
            .unwrap_or(Reproduction::Incomplete);
        report.insert(0, format!("Verdict: {}", verdict.label()));

        // Only a real reproduction clears the pause. Everything else keeps the record and moves
        // it to what it is actually waiting for — and *faster than the reference* is not
        // evidence of damage, so it is never `RecoveryRequired`.
        if verdict.yes() {
            kept.cleared(model);
        } else if let Some(waiting) = kept.paused_for(model).cloned() {
            // **What this check decides, said where it decides it.** The comparison itself no
            // longer carries a consequence: a search now measures in whatever state it finds and
            // labels the result, so a sentence about pausing belongs to the one thing that still
            // pauses.
            report.push(
                "The interrupted benchmark stays paused. A new search may still run                  — it will measure in this state and say so."
                    .to_owned(),
            );
            let mut moved = waiting;
            if verdict.degraded() {
                moved.session.recovery_failed();
            } else {
                moved.session.needs_a_reference(verdict.label());
            }
            moved.last_control = Some(rate);
            moved.at = epoch_engine::now_ms();
            kept.pause(moved);
        }
        kept.save(library.root())?;
        Ok(report.join("\n"))
    }

    /// Take the control: run the baseline configuration and say what it answered.
    ///
    /*
        **The sentinel.** It runs before the first candidate and again after every one of them,
        because a candidate can change the driver's residency decisions and leave the environment
        degraded for everything measured afterwards — measured 2026-08-31, and a search that only
        bracketed itself at both ends would have spent nine configurations learning that row two
        was the last honest one.

        The same configuration every time, so the only thing a difference can be about is the
        machine.
    */
    /// The workload a control run asks for.
    ///
    /// **Named once and read twice**: by the run, and by the fingerprint that records what the
    /// run was a measurement of. Two spellings of one workload would agree by luck until the day
    /// somebody changed one of them, and the fingerprint would then describe a benchmark that
    /// did not happen.
    fn control_ask(baseline: &epoch_engine::optimize::Step) -> epoch_engine::bench::Ask {
        epoch_engine::bench::Ask {
            context: baseline.loadout.context,
            ..epoch_engine::bench::Ask::default()
        }
    }

    /// **The one way a standard control is taken.**
    ///
    /// ## Why this exists
    ///
    /// There were three of these. A calibration, a clean-state check and a search's sentinels each
    /// had a path that did *approximately* the same thing, and the differences between them were
    /// nobody's decision — they were whatever each call site happened to have been written with.
    ///
    /// It cost a real answer. Measured 2026-09-01, one machine, one afternoon, one nominal
    /// configuration: a clean-state control read **46.7 tok/s** and a calibration of the same
    /// thing read **41.9**, because the calibration ran a load sampler and the check did not. The
    /// fingerprint recorded neither fact, so the two were about to be compared.
    ///
    /// So: one function, one [`protocol::Protocol`], and the protocol travels on the fingerprint.
    /// A caller may not vary the instrument, the warmup, the run count or the lifecycle; it may
    /// only say which model and which configuration.
    ///
    /// The lifecycle is [`Lifecycle::Fresh`] and it is obeyed rather than described: the router is
    /// stopped, the configuration is written, the model is loaded, warmups run and are discarded,
    /// the measured runs happen, and the model is let go. What the process had done before is
    /// recorded on the [`Sequence`] so *did these two follow the same steps* is answerable after
    /// the fact instead of by recollection.
    fn measure_standard_control(
        &self,
        model: &str,
        endpoint: &str,
        named: &str,
        baseline: &epoch_engine::optimize::Step,
        protocol: &epoch_engine::protocol::Protocol,
        source: &str,
    ) -> epoch_engine::reference::Observation {
        use epoch_engine::protocol::Lifecycle;

        let empty = |sequence| epoch_engine::reference::Observation {
            id: String::new(),
            fingerprint_id: String::new(),
            fingerprint: self.fingerprint(model, &baseline.loadout, &baseline.tuning, protocol),
            protocol: protocol.id(),
            source: source.to_owned(),
            sequence,
            rates: Vec::new(),
            observed: Default::default(),
            at: epoch_engine::now_ms(),
        };

        // **Fresh means fresh.** A reused process carries however many requests it happened to
        // have served, and that number is in nobody's notes — which is the variable A and B
        // differed on when their run sets failed to overlap.
        if protocol.lifecycle == Lifecycle::Fresh {
            let _ = epoch_engine::models::runtimes::stop_router(&shelf_dir());
        }
        if self.put_into_effect(model, baseline).is_err() {
            return empty(Default::default());
        }
        let started_at = epoch_engine::now_ms();
        let ask = Self::control_ask(baseline);

        /*
            Warmups, run and thrown away — **and kept as evidence, not averaged in.**

            `A first load is not a latency` is already a rule here; this is the half of it that
            was never implemented. A warmup that came back far from the measured runs is the
            interesting case, so its rate is recorded beside them rather than discarded twice.
        */
        let shared_before = epoch_engine::models::load::shared();
        let held_it = Self::holding_now(named);
        let watch = epoch_engine::models::load::Watch::begin();
        // **One call, and the counts are the protocol's.** They used to be this function's
        // business and `bench` had its own private three plus a warm-up of its own, so a protocol
        // saying `warmup=1;runs=5` produced 1+3 and then 1+3 — an id describing a run that did
        // not happen, which is the failure the protocol exists to prevent.
        let (warmed, measured) = epoch_engine::bench::measure_times(
            endpoint,
            named,
            &ask,
            protocol.measured_runs as usize,
            protocol.warmup_runs as usize,
            &|_, _| {},
        );
        let load = watch.stop();
        let warmup_rates: Vec<f64> = warmed
            .runs
            .iter()
            .filter(|r| r.ok())
            .filter_map(|r| r.generation)
            .collect();
        let shared_after = epoch_engine::models::load::shared();
        // A benchmark is not a turn: the card goes back, and the next control starts from the
        // same place this one did.
        self.stop_holding(named, held_it);

        let rates: Vec<f64> = measured
            .runs
            .iter()
            .filter(|r| r.ok())
            .filter_map(|r| r.generation)
            .collect();

        epoch_engine::reference::Observation {
            id: String::new(),
            fingerprint_id: String::new(),
            fingerprint: self.fingerprint(model, &baseline.loadout, &baseline.tuning, protocol),
            protocol: protocol.id(),
            source: source.to_owned(),
            sequence: epoch_engine::protocol::Sequence {
                // **Counted, not assumed.** Asking for V2 is not the same as V2 having happened,
                // and until 2026-09-01 nothing checked: a protocol saying `warmup=1;runs=5` had
                // produced 1+3 and then 1+3, with the id travelling on the fingerprint as a
                // description of the run.
                execution: epoch_engine::protocol::Execution {
                    warmups_expected: protocol.warmup_runs,
                    warmups_observed: warmed.runs.len() as u32,
                    runs_expected: protocol.measured_runs,
                    runs_observed: measured.runs.len() as u32,
                },
                lifecycle: Some(protocol.lifecycle.id().to_owned()),
                router_started_at: Some(started_at),
                router_age_ms: Some(epoch_engine::now_ms().saturating_sub(started_at)),
                // What the process had served before the first *measured* run. The protocol's
                // warm-ups and nothing else, now that nothing else runs any of its own.
                requests_before_measurement: Some(protocol.warmup_runs),
                warmup_runs: Some(protocol.warmup_runs),
                warmup_rates,
            },
            rates,
            observed: epoch_engine::reference::Observed {
                prompt: measured.prompt(),
                first_token_ms: measured.first_token_ms(),
                vram_used: load.vram_used.map(|it| it.peak as u64),
                shared_before,
                shared_after,
                ram_used: load.ram_used.map(|it| it.peak as u64),
                gpu_percent: load.gpu_percent.map(|it| it.mean),
                cpu_percent: load.cpu_percent.map(|it| it.mean),
                sampling_overhead: Some(load.sampling.overhead()),
            },
            at: epoch_engine::now_ms(),
        }
    }

    /// The rates a standard control produced. Every caller that only wants the numbers.
    /// A standard control, **recorded**, whatever errand asked for it.
    ///
    /// **Every valid standard control produces an observation.** The route by which a measurement
    /// was taken must not decide whether it survives — a control read 48.04 tok/s with five runs,
    /// a MATCH and a comparable fingerprint, and existed only as a sentence on a screen because
    /// the code path that took it happened to be a comparison rather than a calibration.
    ///
    /// A refusal to record is never silent and never fatal: this is measurement, and losing the
    /// store is not a reason to lose the reading the caller is waiting for.
    fn controlled(
        &self,
        model: &str,
        endpoint: &str,
        named: &str,
        baseline: &epoch_engine::optimize::Step,
        source: &str,
    ) -> epoch_engine::reference::Observation {
        let library = epoch_engine::models::generative::Library::here();
        let mut kept = epoch_engine::sessions::Kept::load(library.root());
        let id = kept.next_observation_id();
        let seen = self
            .measure_standard_control(
                model,
                endpoint,
                named,
                baseline,
                &epoch_engine::protocol::STANDARD_CONTROL_V2,
                source,
            )
            .named(&id);
        // Kept whether or not it is usable: an observation that failed its own protocol is the
        // evidence that something is wrong with the instrument, and it is exactly the record that
        // would otherwise be thrown away for being inconvenient.
        kept.observed(seen.clone());
        let _ = kept.save(library.root());
        seen
    }

    /// The rates a standard control produced. Every caller that only wants the numbers.
    ///
    /// **Nothing calls this, or `try_to_recover` below** — found by clippy, not by reading.
    /// Both are the recovery path a degraded machine is supposed to take, so their being
    /// unreachable is a fact about that path rather than about these functions, and it is worth
    /// tracing before either is deleted. Allowed rather than removed for that reason.
    #[allow(dead_code)]
    fn take_control(
        &self,
        model: &str,
        endpoint: &str,
        named: &str,
        baseline: &epoch_engine::optimize::Step,
        source: &str,
    ) -> Vec<f64> {
        self.controlled(model, endpoint, named, baseline, source)
            .rates
    }

    /// What is safe and cheap to try when the control stops reproducing.
    ///
    /*
        **Deliberately timid.** What actually restores a degraded environment is not known, and
        what *is* known is that unloading and reloading the model does not — every retry after the
        first collapse collapsed again. So this ends the instance, waits for the driver to take its
        memory back, and takes the control once more.

        No driver reset and no restart. Those are actions on somebody else's machine, and Epoch
        does not take them because a benchmark would be more convenient if it had.
    */
    #[allow(dead_code)]
    fn try_to_recover(
        &self,
        model: &str,
        endpoint: &str,
        named: &str,
        baseline: &epoch_engine::optimize::Step,
    ) -> Vec<f64> {
        let _ = epoch_engine::models::runtimes::stop_router(&shelf_dir());
        // Long enough for the driver to reclaim, and short enough that somebody is still watching.
        std::thread::sleep(std::time::Duration::from_secs(20));
        self.take_control(model, endpoint, named, baseline, "recovery")
    }

    /// Put one step's configuration into effect and wait for llama.cpp to be ready for it.
    /// What this model is set to right now, so a measurement can put it back.
    ///
    /// **A measurement leaves the machine as it found it.** A search walks through twenty
    /// configurations and a ladder through five, and each one is written into effect for as long
    /// as it is being measured. Whatever the last one happened to be is not a choice anybody
    /// made — leaving a model on a 256K window because that was the top rung would be the search
    /// deciding by side effect what `APPLY` exists to decide on purpose.
    fn how_it_is_set(&self, model: &str) -> Option<epoch_engine::models::loadout::Loadout> {
        let library = epoch_engine::models::generative::Library::here();
        let weights = self
            .shared_weights()
            .into_iter()
            .find(|it| it.name == model)?;
        let names = epoch_engine::models::loadout::names(std::path::Path::new(&weights.path))?;
        let machine = epoch_engine::models::machine::Machine::measure();
        epoch_engine::models::loadout::Loadouts::load(library.root())
            .about(
                &names,
                &machine.gpu.unwrap_or_default(),
                &epoch_engine::models::runtimes::llama_build(),
            )
            .map(|it| it.chose)
    }

    /// Put back the window and cache a measurement borrowed.
    fn set_it_back(&self, model: &str, was: Option<epoch_engine::models::loadout::Loadout>) {
        let Some(was) = was else {
            return;
        };
        let step = epoch_engine::optimize::Step {
            about: String::new(),
            technical: String::new(),
            loadout: was,
            tuning: epoch_engine::models::tuning::Tunings::load(
                epoch_engine::models::generative::Library::here().root(),
            )
            .of(model),
            offload: None,
            gpu_layers: None,
        };
        let _ = self.put_into_effect(model, &step);
    }

    /// Stop the router, write the shelf, start it again, and wait for it to answer.
    ///
    /// ## Why writing the preset is not applying it
    ///
    /// Measured 2026-09-02, from the process table rather than from the file:
    ///
    /// | | |
    /// |---|---|
    /// | router started | 11:17:22 |
    /// | `presets.ini` rewritten by APPLY | 11:46:47 |
    /// | the model's own server spawned | 11:51:28 |
    ///
    /// The child was spawned half an hour *after* the preset was rewritten and still came up
    /// `--ctx-size 16384` with no cache flags — the values the file held when the router
    /// started. **The router reads the preset once and never again**, so a character kept a
    /// 16,384 window while the deck reported the 32,768 the user had chosen, and `use_profile`
    /// said *"is set to … measured here"* about a configuration nothing was running.
    ///
    /// It survives Epoch, too: the Epoch process asking the question had started an hour after
    /// the router it was asking about. So *restart Epoch* is not the workaround either.
    ///
    /// **One function, because two would disagree.** The search has stopped the router before
    /// each candidate since the morning of the same day; the user's own APPLY did not, and the
    /// two paths write the same file for the same purpose.
    fn restart_router_for(
        &self,
        model: &str,
        window: Option<epoch_engine::models::loadout::Loadout>,
    ) -> Result<(), String> {
        let _ = epoch_engine::models::runtimes::stop_router(&shelf_dir());
        let mine = model.to_owned();
        let otherwise = self.how_to_load();
        let how = move |it: &epoch_engine::models::runtimes::Weights| match window {
            // One start with one window, rather than a stored preference read back through a
            // lookup that can match the wrong row. Every other model keeps what it had.
            Some(one) if it.name == mine => one,
            _ => otherwise(it),
        };
        epoch_engine::models::runtimes::serve_everything(
            &shelf_dir(),
            &self.shared_weights(),
            &how,
            &Self::how_to_tune(),
            "Epoch - llama.cpp serving",
        )?;
        wait_for_llama_cpp(std::time::Duration::from_secs(90))
    }

    fn put_into_effect(
        &self,
        model: &str,
        step: &epoch_engine::optimize::Step,
    ) -> Result<(), String> {
        let library = epoch_engine::models::generative::Library::here();
        let mut held = epoch_engine::models::tuning::Tunings::load(library.root());
        held.set(model, step.tuning.clone());
        held.save(library.root())?;

        /*
            **And the half of a step that is not tuning, written straight into the preset.**

            `Tunings` carries flash attention, speculation and placement. The context and the KV
            cache reach llama.cpp through the shelf's preset, and this wrote only the first — so
            `step.loadout` died in the function named for putting a step into effect. Two things
            were measured that never ran: `KV cache · compressed` reported 51.69 tok/s for a
            configuration the server never had, and a ladder asking for 64K was answered by a
            server still holding 32,768, its filled prompt coming back with nothing at all.

            **Directly, rather than through `Loadouts` — and that is the fix rather than a
            detail.** The first attempt wrote the step into that store and let the shelf writer
            read it back. It is keyed by file, card, build and runtime, read back by three of
            those four, and holds a row per runtime for one model — so a step's window travelled
            through a lookup that can match the wrong row, no row, or a row a later restore has
            already overwritten, and the preset stayed at 32,768 through a ladder climbing to
            256K.

            A step is not a stored preference. What it needs is for *this* server start to use
            *this* window, once, and the shelf writer already takes a function that answers
            exactly that. Every other model on the shelf keeps whatever it had.

            **And the router is stopped first, which it was not.**

            This said *"not stopped here — the caller stopped it before the preflight ran"*. One
            caller does, on its retry path. Every other candidate and every rung did not, and
            `serve_everything` stops nothing: it writes the preset and opens a new terminal
            running a new router. The new one cannot take the port, so **the old router keeps
            answering, with the preset it read when it started**.

            Measured 2026-09-02: three `llama-server.exe` and four consoles alive at once, and a
            ladder whose preset read `ctx-size=65536` at the exact second the screen said
            `Context - 64K, filled` — while the requests went to a router still holding 32,768,
            and the rung came back with nothing. The window reached the file and the file reached
            nobody.

            It is also the terminals the owner reported piling up. Making `stop_router` close its
            console was right and could not have helped: nothing was calling it.
        */
        self.restart_router_for(model, Some(step.loadout))
    }

    /// What a character on this Brain inherits from MODELS: the window, and how it behaves.
    ///
    /// ## Read-only, and it says which of three things it read
    ///
    /// ADR-0026's amendment moved the window out of the character and into MODELS, which leaves a
    /// character panel with a number it did not choose and cannot change. That number is only
    /// worth printing if it says whose it is:
    ///
    /// | source | who decided it | speed shown |
    /// |---|---|---|
    /// | `Profile` | a search that ran on this card, applied by the user | yes |
    /// | `Loadout` | what llama.cpp will next be started with | no |
    /// | `Reported` | the backend's own answer about its model | no |
    /// | `Nothing` | nobody — *Not configured in MODELS* | no |
    ///
    /// **Only a profile carries a speed.** A model's last timing was taken at whatever loadout was
    /// current that afternoon; printing it beside a window chosen afterwards would be a real
    /// reading of a different configuration, which is the most convincing way a gauge can lie.
    ///
    /// **Nothing is invented at the bottom.** A Brain nobody has configured answers `None`, and the
    /// panel says so rather than reaching for a plausible constant.
    pub fn inherited_runtime(
        &self,
        provider: &str,
        model: &str,
    ) -> epoch_engine::inherits::Inherited {
        use epoch_engine::inherits::{Inherited, Source};
        use epoch_engine::models::runtimes::shelf_name;

        let mut it = Inherited {
            model: model.to_owned(),
            ..Inherited::default()
        };

        /*
            **One model, two spellings, and they agree by luck.** A character on llama.cpp carries
            the name the router serves it under — `gemma4-12b` — while the Models deck and the
            profile store both key on the shelf's own name, `gemma4:12b`. Every name without a
            colon matches either way, which is exactly how this survived: found by opening the
            panel on the one model here whose name has one, and reading `Not configured in MODELS`
            for a model with a 64K loadout sitting on the deck. Worse than the reading — the
            *profile* lookup misses the same way, so a character would never have seen a profile
            applied to its own Brain.

            `shelf_name` produces the second spelling from the first, so both sides are compared
            through it: one measurement read twice, rather than two spellings hoping to match.

            **Only for llama.cpp**, because that is the only runtime a profile is measured on and
            the only one a loadout is written for. Showing an Ollama character the window
            llama.cpp would use is a real reading of a different program.
        */
        let want = shelf_name(model);
        let mine = provider == "llama_cpp";

        /*
            **Epoch's own file first, and the shelves only if it has nothing to say.**

            Measured 2026-08-31 (`vault/measurements/shelfread.out`): the deck read is 15.7 s in
            the running window, because `pair_up` reads every GGUF header and `deck::about` reads
            them all again. That is the Models deck's own cost and it is a page somebody opens on
            purpose; a character panel is not, and it had inherited the whole of it.

            `optimized.json` answers the same question for any model that has been measured, in
            four milliseconds — and it is the half that carries a speed, so the reading that says
            the most is now the one that costs the least.
        */
        let library = epoch_engine::models::generative::Library::here();
        let measured_as = mine
            .then(|| {
                epoch_engine::profiles::Every::load(library.root())
                    .models()
                    .find(|known| shelf_name(known) == want)
                    .map(str::to_owned)
            })
            .flatten();

        let applied = measured_as.as_ref().and_then(|known| {
            let one = self.optimization(known)?;
            let intent = one.chosen?;
            let cfg = one
                .resolve(intent, epoch_engine::suite::STANDARD_CONTEXT)?
                .clone();
            // `optimization` already refuses a result taken on another card, and `usable` already
            // refuses a session whose control stopped reproducing — so a degraded search cannot
            // reach a character panel as a recommendation.
            one.usable.then_some((known.clone(), intent, cfg))
        });

        if let Some((known, intent, cfg)) = applied {
            it.model = known;
            it.window = Some(cfg.context());
            it.source = Source::Profile;
            it.profile = Some(intent.name().to_owned());
            // The median, not the last reading: a configuration is what it does repeatedly.
            it.generation = Some(cfg.verdict.median.unwrap_or(cfg.generation));
            it.stable = cfg.verdict.state().trustworthy();
        } else if mine {
            // Nothing measured, so the shelves are the only place the loadout lives — a real
            // choice made in MODELS even though nothing searched for it.
            if let Some(row) = self
                .models_and_loadouts()
                .into_iter()
                .find(|row| shelf_name(&row.name) == want)
            {
                it.window = Some(row.loadout.context);
                it.source = Source::Loadout;
                // The name that addresses MODELS, which is all `model` is used for: the panel's
                // door sends it to the deck to open a row on, and the deck drops an errand
                // naming a model it does not hold.
                it.model = row.name;
            }
        }

        // What the backend says about its own model. Last, because it is the only one of the
        // three that nothing in MODELS decided — and still a measurement, so it beats silence.
        if it.window.is_none() {
            if let Some(window) = self.measured_window(provider, model) {
                it.window = Some(window);
                it.source = Source::Reported;
            }
        }

        it
    }

    /// Write one profile's whole configuration out, so the model runs that way from now on.
    ///
    /// **Everything at once**, because the point of a profile is that nobody has to know which
    /// five settings it is made of. What is written is exactly what was measured — the same
    /// loadout and the same tuning that produced the number on the button.
    /// Put one **specific measured configuration** into effect, named by when it ran.
    ///
    /// ## Why a profile is not enough
    ///
    /// Epoch offers what it is willing to recommend, and the owner's correction of 2026-09-02 is
    /// that those are not the same list. A configuration whose runs overlapped a steadier one is
    /// a real gamble — `ngram-mod` measured 48.8 to 72.2 on `gemma4:12b` — and it is the owner's
    /// card, the owner's model and the owner's gamble. Epoch measured and said; the choice is
    /// theirs (ADR-0033, one subsystem over).
    ///
    /// **Addressed by `at`, never by an index into the list.** The rows are reordered by every
    /// rule that reads them, and an index would quietly start applying somebody else's
    /// measurement the first time one changed.
    ///
    /// It lands in `custom`, which already exists and already resolves — so nothing downstream
    /// learns a new concept, and a later search that re-measures the model leaves the pin alone.
    pub fn use_measured(&self, model: &str, at: u64) -> Result<String, String> {
        let library = epoch_engine::models::generative::Library::here();
        let optimized = self
            .optimization(model)
            .ok_or_else(|| format!("nothing has been measured for {model} on this machine."))?;
        let one = optimized
            .tried
            .iter()
            .find(|it| it.at == at)
            .ok_or_else(|| {
                format!("that configuration is not among what was measured for {model}.")
            })?
            .clone();

        /*
            **A collapse is still refused, and this is the one place it has to be said out loud.**

            Everything else set aside is a judgement about risk the owner may overrule. A
            collapsed set is not: its surviving runs came from an instance that broke, so the
            number does not describe a configuration at all. Offering it would not be respecting
            a choice, it would be handing over a reading of something that did not happen.
        */
        if one.verdict.collapses > 0 {
            return Err("that run collapsed partway through, so its speed is not a reading of a                  configuration — it is a reading of an instance that broke. Nothing here can be                  put into effect from it.".to_string());
        }

        let mut kept = optimized.clone();
        kept.custom = Some(one.clone());
        let mut every = epoch_engine::profiles::Every::load(library.root());
        every.set(model, kept);
        every.save(library.root())?;
        self.use_profile(model, epoch_engine::profiles::Intent::Custom)
    }

    pub fn use_profile(
        &self,
        model: &str,
        intent: epoch_engine::profiles::Intent,
    ) -> Result<String, String> {
        let library = epoch_engine::models::generative::Library::here();
        let optimized = self
            .optimization(model)
            .ok_or_else(|| format!("nothing has been measured for {model} on this machine."))?;
        let one = optimized
            .resolve(intent, epoch_engine::suite::STANDARD_CONTEXT)
            .ok_or_else(|| {
                format!(
                    "no measured configuration satisfies {} for {model}.",
                    intent.name()
                )
            })?
            .clone();

        let mut tuned = epoch_engine::models::tuning::Tunings::load(library.root());
        tuned.set(model, one.tuning.clone());
        tuned.save(library.root())?;

        /*
            **And the half of the configuration that is not tuning.**

            `Tunings` carries flash attention, speculation and placement; the context and the KV
            cache live in `Loadouts`, which is what `how_to_load` reads when the shelf is stocked.
            This wrote only the first, so applying a profile whose winning row was
            `KV cache · compressed` told llama.cpp nothing about the cache — and then said
            *"set to BALANCED — 51.0 tok/s at 32K, measured here"* about a model still running
            whatever a loadout curve had left behind, or the conservative default if nothing had.

            A recommendation the product does not put into effect is worse than no recommendation:
            it is a number quoted for a machine that is not the one running. Both halves are
            written here, under the key `how_to_load` reads them back by — `names`, card, build,
            runtime — because two keys for one fact agree by luck, and this file has paid for that
            once already.
        */
        let weights = self
            .shared_weights()
            .into_iter()
            .find(|it| it.name == model)
            .ok_or_else(|| format!("{model} is not on the shelf any more."))?;
        let names = epoch_engine::models::loadout::names(std::path::Path::new(&weights.path))
            .ok_or_else(|| format!("{model}'s file went away."))?;
        let machine = epoch_engine::models::machine::Machine::measure();
        let mut loadouts = epoch_engine::models::loadout::Loadouts::load(library.root());
        // What the curve already knew about this file, so applying a profile does not throw away
        // the measured ceiling. `most` is *the largest context that loaded* — a different fact
        // from the one being chosen here, and inventing a zero would uncap a model that was
        // measured to have a wall.
        let known = loadouts
            .about(&names, &optimized.gpu, &optimized.build)
            .cloned();
        loadouts.remember(epoch_engine::models::loadout::Best {
            model: names,
            name: model.to_owned(),
            card: machine.gpu.clone().unwrap_or_default(),
            build: epoch_engine::models::runtimes::llama_build(),
            runtime: "llama_cpp".to_owned(),
            chose: one.loadout,
            most: known.as_ref().map(|it| it.most).unwrap_or_default(),
            tokens_per_second: one.verdict.median.unwrap_or(one.generation),
            tried: known.map(|it| it.tried).unwrap_or_default(),
            at: epoch_engine::now_ms(),
        });
        loadouts.save(library.root())?;

        let mut kept = optimized.clone();
        kept.chosen = Some(intent);
        // Choosing a named profile retires whatever row was pinned by hand: two answers to
        // *what is this model set to* would agree only until one of them changed.
        if intent != epoch_engine::profiles::Intent::Custom {
            kept.custom = None;
        }
        let mut every = epoch_engine::profiles::Every::load(library.root());
        every.set(model, kept);
        every.save(library.root())?;

        self.tell_llama_cpp();

        /*
            **Written is not applied, and this said it was.**

            `tell_llama_cpp` rewrites the shelf preset and asks the router to let go of what it
            holds. Neither reaches a router that is already running: it reads the preset once, at
            startup. Measured from the process table on 2026-09-02 \u2014 preset rewritten 11:46:47,
            the model\'s own server spawned 11:51:28, and it came up with the values the file held
            at 11:17. The sentence below claimed a configuration was in force while a character
            went on answering from a 16,384 window nobody had chosen.

            So APPLY restarts the router, which the search has done per candidate since the same
            morning. The cost is real and is named on the button rather than discovered: about a
            minute, and a turn in flight is interrupted.
        */
        let restarted = self.restart_router_for(model, Some(one.loadout));

        // **Named for the runtime it governs.** A search runs on llama.cpp and writes a llama.cpp
        // preset; the same weights are often also served by Ollama, and a character on that Brain
        // is answered by a program none of this configured. "Measured here" was true and said
        // nothing about which program will use it.
        let set = format!(
            "{model} is set to {} on llama.cpp \u{2014} {:.1} tok/s at {}K, measured here.",
            intent.name(),
            one.generation,
            one.loadout.context / 1024,
        );
        Ok(match restarted {
            Ok(()) => format!("{set} llama.cpp restarted and is serving it this way."),
            // **The half that worked is still true.** The preset is on disk and the next start
            // will use it; what failed is only the part that would have made it true now, and
            // saying so beats both a refusal and a claim.
            Err(why) => format!(
                "{set} llama.cpp did not come back on its own ({why}). The setting is written \
                 and will take effect the next time it starts — CONNECTIONS, llama.cpp, START."
            ),
        })
    }

    /// Forget what was measured for one model.
    pub fn forget_optimization(&self, model: &str) -> Result<String, String> {
        let library = epoch_engine::models::generative::Library::here();
        let mut every = epoch_engine::profiles::Every::load(library.root());
        every.forget(model);
        every.save(library.root())?;
        Ok(format!("{model}'s measured configurations are gone."))
    }

    /// Find the fastest speculative decoding for one model, by running every candidate.
    ///
    /*
        **Long, and there is no shorter honest version.** Each configuration is a preset written,
        a router restarted and a model loaded, then a warm-up and three timed runs. For a large
        model that is minutes each, and the answer is worth it precisely because nobody else's
        table knows about this card.

        `apply` is the part that differs by runtime and it is a closure for that reason. llama.cpp
        reads speculation flags when it *spawns the child that holds the model* — measured
        2026-08-30, rewriting the preset and unloading is not enough, the child keeps the argv it
        was born with — so putting a configuration into effect means restarting the router and
        waiting for it to answer again.

        Only llama.cpp. Ollama exposes no speculation flags at all and LM Studio takes none Epoch
        can set, so offering the sweep there would be the dead `Manual` mode of ADR-0027: a
        control that changes nothing.
    */
    pub fn sweep_speculation(
        &self,
        model: &str,
        watching: &dyn Fn(&str, usize, usize),
    ) -> Result<epoch_engine::spec::Sweep, String> {
        use epoch_engine::models::runtimes::Runtime;

        let seen = epoch_engine::models::runtimes::look_for(Runtime::LlamaCpp);
        if !seen.serving {
            return Err(
                "llama.cpp is not answering. A sweep restarts it many times, so it \
                        has to be running to begin with."
                    .to_owned(),
            );
        }
        let named = epoch_engine::models::runtimes::offered_as(model, &seen.models)
            .ok_or_else(|| format!("llama.cpp is not serving '{model}'."))?;

        // The build's own vocabulary, asked rather than remembered. A release that grows a
        // twelfth type is swept without this file changing.
        let offered: Vec<String> = epoch_engine::models::tuning::SPEC_TYPES
            .iter()
            .map(|it| (*it).to_owned())
            .collect();

        // A draft file only if this model has one beside it. Nothing here knows which models ship
        // one — it is a question about a file, and `draft_beside` answers it by looking.
        let library = epoch_engine::models::generative::Library::here();
        let draft = epoch_engine::models::runtimes::draft_beside(&shelf_dir(), model);

        let carries_mtp = self.carries_its_own_mtp(model);
        let every = epoch_engine::spec::candidates(&offered, draft.as_deref(), carries_mtp);
        if every.is_empty() {
            return Err("this build offers no speculative decoding that can run here.".to_owned());
        }

        let held = epoch_engine::models::tuning::Tunings::load(library.root());
        let was = held.of(model);

        let apply = |tuning: &epoch_engine::models::tuning::Tuning| -> Result<(), String> {
            let mut held = epoch_engine::models::tuning::Tunings::load(library.root());
            held.set(model, tuning.clone());
            held.save(library.root())?;
            // Stopped before started: `serve_everything` opens a terminal rather than replacing
            // one, and a second terminal on a bound port fails. Only the router serving *this*
            // shelf — LM Studio's runtime is `llama-server.exe` too.
            epoch_engine::models::runtimes::stop_router(&shelf_dir())?;
            self.start_router()?;
            // The router answers before its children exist, so what is waited for is the model
            // list rather than the port. Sixty seconds: a 17.7 GB model on this machine takes
            // about seventy to load, and that load happens on the first request rather than here.
            wait_for_llama_cpp(std::time::Duration::from_secs(60))
        };

        let ask = epoch_engine::bench::Ask {
            context: epoch_engine::suite::STANDARD_CONTEXT,
            ..epoch_engine::bench::Ask::default()
        };
        let mut sweep =
            epoch_engine::spec::sweep(&seen.endpoint, &named, &ask, &every, &apply, watching);
        sweep.model = model.to_owned();

        /*
            **Put back what the user had.** A sweep is a measurement, not a decision: leaving the
            last candidate in place would mean the slowest thing tried becomes what the model runs
            with, chosen by nothing but the order of a loop. Applying the *winner* would be a
            decision Epoch made, and ADR-0033's line is that the measurement is Epoch's and the
            choice is the user's.
        */
        let mut held = epoch_engine::models::tuning::Tunings::load(library.root());
        held.set(model, was);
        let _ = held.save(library.root());
        let _ = epoch_engine::models::runtimes::stop_router(&shelf_dir());
        let _ = self.start_router();

        Ok(sweep)
    }

    /// Put one model through the Standard benchmark, on a runtime the caller names.
    ///
    /// **The conditions are read, never typed.** A card that said *45 tok/s* without which card,
    /// which build, which runtime, which context and which suite would be a fact about nothing —
    /// and every one of those is asked of the machine here rather than assumed from a form.
    ///
    /// Minutes. `watching` is called with what is happening because a job that speaks only at the
    /// end is one somebody kills in the middle.
    pub fn benchmark(
        &self,
        model: &str,
        on: &str,
        watching: &dyn Fn(&str, usize, usize),
    ) -> Result<String, String> {
        let runtime = runtime_named(on)?;
        let seen = epoch_engine::models::runtimes::look_for(runtime);
        if !seen.serving {
            return Err(format!(
                "{} is not answering. A benchmark runs through a server that is already up.",
                seen.name
            ));
        }
        // The name that server uses, which is not always the name on the deck.
        let named = epoch_engine::models::runtimes::offered_as(model, &seen.models)
            .ok_or_else(|| format!("{} is not serving '{model}'.", seen.name))?;

        let library = epoch_engine::models::generative::Library::here();
        let conditions = epoch_engine::card::Conditions {
            suite: epoch_engine::suite::VERSION,
            gpu: epoch_engine::models::machine::Machine::measure()
                .gpu
                .unwrap_or_default(),
            build: match runtime {
                epoch_engine::models::runtimes::Runtime::LlamaCpp => {
                    epoch_engine::models::runtimes::llama_build()
                }
                other => other.name().to_owned(),
            },
            runtime: on.to_owned(),
            context: epoch_engine::suite::STANDARD_CONTEXT,
            tuning: epoch_engine::models::tuning::Tunings::load(library.root()).of(model),
            at: epoch_engine::now_ms(),
        };

        let card =
            epoch_engine::card::standard(&seen.endpoint, &named, model, conditions, watching);
        let scored = |kind| {
            /*
                **Three numbers, because they are three facts.** A model that answered four of
                five reasoning trials and got all four right was accurate on everything it said
                and finished four fifths of what it was given, and one figure can only tell you
                one of those. The counts come too: the same rate over five trials and over fifty
                is not the same evidence.
            */
            let column = card.column(kind);
            if column.asked == 0 {
                return "not asked".to_owned();
            }
            let percent = |it: Option<f64>| {
                it.map(|it| format!("{:.1}%", it * 100.0))
                    .unwrap_or_else(|| "—".to_owned())
            };
            // A tool call is judged in parts, so a tally can be fractional. Shown as one only
            // when it really is one.
            let tally = |what: f64| {
                if (what - what.round()).abs() < 0.05 {
                    format!("{what:.0}")
                } else {
                    format!("{what:.1}")
                }
            };
            format!(
                "correct {} ({}/{}) · completed {} ({}/{}) · effective {} ({}/{})",
                percent(column.correctness),
                tally(column.correct),
                column.answered,
                percent(column.completion),
                column.answered,
                column.asked,
                percent(column.effective),
                tally(column.correct),
                column.asked,
            )
        };
        let said = format!(
            "{model}: {} tok/s
  reasoning  {}
  coding     {}
  following  {}
  tools      {}",
            card.speed
                .generation()
                .map(|it| format!("{it:.1}"))
                .unwrap_or_else(|| "no answer".to_owned()),
            scored(epoch_engine::card::Kind::Reasoning),
            scored(epoch_engine::card::Kind::Coding),
            scored(epoch_engine::card::Kind::Following),
            scored(epoch_engine::card::Kind::Tools),
        );

        let mut kept = epoch_engine::kept::Kept::load(library.root());
        kept.remember(card);
        kept.save(library.root())?;
        Ok(said)
    }

    /// Every benchmark result this machine has taken, with its columns already worked out.
    ///
    /*
        **The scoring is done once, here, and never again in the window.**

        The frontend used to compute the same four percentages from the same answers, and the two
        spellings disagreed in public on the very first run — the row read `follow 0%` beside a
        sentence reading `following 8%`, because a passing constraint serialises as `{"Ok": null}`
        and one of the two implementations had guessed otherwise.

        Patching the copy would have fixed that instance and left the arrangement that produced
        it. So the copy is gone: the Engine says what a card scored and how much of it was
        answered, and the window renders the sentence rather than recomputing it.
    */
    pub fn benchmarks(&self) -> Vec<Scored> {
        let library = epoch_engine::models::generative::Library::here();
        epoch_engine::kept::Kept::load(library.root())
            .all()
            .iter()
            .map(|card| Scored {
                columns: card.columns(),
                card: card.clone(),
            })
            .collect()
    }

    /// Forget one model's results.
    pub fn forget_benchmarks(&self, model: &str) -> Result<String, String> {
        let library = epoch_engine::models::generative::Library::here();
        let mut kept = epoch_engine::kept::Kept::load(library.root());
        kept.forget(model);
        kept.save(library.root())?;
        Ok(format!("{model}'s results are gone."))
    }

    /// Take one model off this machine.
    ///
    /// **Only what this machine reported holding.** The path is matched against
    /// `shared_weights` rather than trusted, so the one destructive thing the Workshop can do
    /// can only be pointed at something the Workshop itself listed. A delete that accepts an
    /// arbitrary path is a delete somebody can point at anything.
    pub fn remove_model(&self, path: &str) -> Result<String, String> {
        let held = self
            .shared_weights()
            .into_iter()
            .find(|held| held.path == path)
            .ok_or("that is not a model this machine reported holding")?;
        let said = epoch_engine::models::runtimes::remove(&held, &shelf_dir())?;
        /*
            **And llama.cpp is told, because its presets name files that are now gone.**

            `presets.ini` is written from the shelf every time it is written, so it heals itself
            — but only when something writes it, and a removal never did. Measured on the owner's
            machine after he deleted four models: the file still carried four sections pointing
            at GGUFs in `vault/shelf` that no longer exist, so the router would offer five models
            and fail to load four of them.

            (And the glob that named those files is not written here on purpose: Rust's block
            comments nest, so a slash-star inside one opens a level that never closes. The
            compiler catches it; it is still a minute nobody needs to spend.)

            This is the same shape as the defect one function over, which is why it is fixed in
            the same pass: a removal cleaned one of the things it had made and left the rest.
        */
        self.tell_llama_cpp();
        Ok(said)
    }

    /// Give Ollama a GGUF it did not download.
    ///
    /// **Only a file, never one of Ollama's own blobs.** Importing what it already holds under
    /// another name would spend three and a half minutes hashing to produce a second name for
    /// one model â€” measured, and the manifest is all it would add.
    ///
    /// Opens a terminal rather than blocking, because it *is* three and a half minutes for
    /// nine gigabytes: the same door as installing and starting, and for the same reason.
    pub fn import_weights(&self, path: &str) -> Result<(), String> {
        let held = self
            .shared_weights()
            .into_iter()
            .find(|held| held.path == path)
            .ok_or("that is not a model this machine reported holding")?;
        if held.from == "Ollama" {
            return Err("Ollama already has that one".to_owned());
        }
        let command =
            epoch_engine::models::runtimes::ollama_import_command(&held.name, &held.path)?;
        epoch_engine::models::runtimes::open_in_terminal(
            &format!("Epoch - importing {}", held.name),
            &command,
        )
    }

    /// Start llama.cpp knowing about **every** model on this machine.
    ///
    /// ## Why this replaced picking one
    ///
    /// It was started *holding* a model chosen from a dropdown, which is a question nobody wants
    /// to answer before they know what they are going to ask — and it made llama.cpp the one
    /// runtime you had to configure per use.
    ///
    /// Measured 2026-08-21: `llama-server --models-dir` is a **router**. It reports every model
    /// in the directory, loads one only when a request names it, and leaves the rest available:
    /// two models listed, `"Blue"` in 21.2 s from the one that was asked for, the other still
    /// merely on offer.
    ///
    /// The shelf is stocked first, so what it serves is what this machine has *now* rather than
    /// whatever was there the last time somebody pressed anything.
    pub fn start_router(&self) -> Result<String, String> {
        let held = self.shared_weights();
        let how = self.how_to_load();
        let told = Self::how_to_tune();
        epoch_engine::models::runtimes::serve_everything(
            &shelf_dir(),
            &held,
            &how,
            &told,
            "Epoch - llama.cpp serving",
        )
    }

    /// Put every model this machine has on LM Studio's shelf.
    ///
    /// **The wording and the linking live in `epoch-models`, not here.** A machine lending its
    /// graphics card runs EpochServices and offers these same two buttons, and two copies of
    /// *what counts as a model this machine has* is how the Host and a lent machine come to
    /// disagree about it.
    pub fn lend_all_to_lm_studio(&self) -> Result<String, String> {
        epoch_engine::models::runtimes::lend_everything(&self.shared_weights())
    }

    /// Whether this machine has the Hugging Face CLI, and who it is signed in as.
    pub fn hugging_face(&self) -> epoch_engine::models::hf::Cli {
        epoch_engine::models::hf::cli()
    }

    /// The Hugging Face CLI on every machine this World can use, measured on each.
    ///
    /// **Not one reading with a machine name on it.** `hf` on this computer says nothing about
    /// the machine lending a graphics card, and it is *that* machine a model would be
    /// downloaded onto — so each is asked where it lives and each answers for itself.
    ///
    /// A remote that does not answer is reported as unreachable rather than as *not installed*:
    /// an answer nobody can read is not an answer, and the two have different fixes.
    pub fn hugging_face_everywhere(&self) -> Vec<HuggingFaceSomewhere> {
        let here = self.hugging_face();
        let mut rows = vec![HuggingFaceSomewhere {
            machine: "This machine".to_owned(),
            local: true,
            reached: true,
            installed: here.installed,
            version: here.version,
            found_at: here.found_at,
            user: here.user,
        }];

        let secrets = self.secrets();
        for paired in epoch_engine::Pairings::load(&vault_dir()).all() {
            // Asked live rather than read from the roster: `hf` can be installed on that machine
            // while Epoch is open, and a reading taken at pairing time would say otherwise for
            // as long as the pairing lasts.
            let asked = epoch_engine::bridge::have_of(paired, &secrets);
            rows.push(match asked {
                Err(_) => HuggingFaceSomewhere {
                    machine: paired.name.clone(),
                    local: false,
                    reached: false,
                    ..Default::default()
                },
                Ok(have) => {
                    let read = have.hf.unwrap_or_default();
                    HuggingFaceSomewhere {
                        machine: paired.name.clone(),
                        local: false,
                        reached: true,
                        installed: read.installed,
                        version: read.version,
                        found_at: read.found_at,
                        user: read.user,
                    }
                }
            });
        }
        rows
    }

    /// What a download would fetch, without fetching it.
    pub fn plan_download(
        &self,
        repo: &str,
        quant: &str,
    ) -> Result<Vec<epoch_engine::models::hf::Planned>, String> {
        epoch_engine::models::hf::plan(bare(repo), quant)
    }

    /// Fetch one quantisation onto this machine, for a runtime that takes a file.
    ///
    /// **Into the vault, under `models/`.** The frontend never chooses a path — it never touches
    /// the filesystem at all (ADR-0024) — and a download that landed wherever the process
    /// happened to be running would be a file nobody could find twice.
    /// Fetch one quantisation and make it available everywhere that costs nothing.
    ///
    /// ## Why this is one press and Ollama is another
    ///
    /// The owner, 2026-09-02: *"se complica mucho las descargas"*. Three buttons stood for one
    /// intent — `DOWNLOAD TO OLLAMA`, `SAVE THE FILE`, `SERVE WITH LLAMA.CPP` — and two tools
    /// for one intent is a decision the user has to make about plumbing rather than about models
    /// (ADR-0030’s fourth amendment, applied to a person instead of a model).
    ///
    /// **Two, not one, because the third destination is not free.** Measured:
    ///
    /// | destination | what it takes |
    /// |---|---|
    /// | llama.cpp | nothing — the vault holds it and the router already serves the vault |
    /// | LM Studio | nothing — a hard link, no second copy of the bytes |
    /// | Ollama | a full copy plus a hash, minutes per model, the file on disk twice |
    ///
    /// Ollama has no search path: it insists on its own blob store, so serving through it means
    /// duplicating the weights — which ADR-0032 refuses to do silently. Folding it into this
    /// button would spend gigabytes without saying so; leaving it a separate press with its cost
    /// written on it is the honest shape.
    ///
    /// **The lend is best-effort and never fails the download.** LM Studio not being installed
    /// is not a reason for a fetch that already succeeded to report failure, and the sentence
    /// says which of the two happened.
    pub fn download_file(&self, repo: &str, quant: &str) -> Result<String, String> {
        let repo = bare(repo);
        let into = vault_dir().join("models").join(repo.replace('/', "-"));
        std::fs::create_dir_all(&into).map_err(|err| format!("cannot create {into:?}: {err}"))?;
        let at = epoch_engine::models::hf::fetch(repo, quant, &into)?;

        /*
            **Restocking the shelf is not telling the router about it.**

            `tell_llama_cpp` rewrites `presets.ini` and asks the router to let go of what it
            holds; it does not restart it. And a running router reads that file once, at startup
            — the same fact that made APPLY write a setting nothing used. Measured 2026-09-02:
            four models downloaded, all four on the shelf, all four `NOT SERVING` and none of
            them offering a benchmark, because the router was still listing the six it had found
            when it started.

            Same fix as APPLY, through the same function: a download that cannot be used until
            somebody restarts a program by hand is a download that is not finished.
        */
        self.tell_llama_cpp();
        let _ = self.restart_router_for(&at.display().to_string(), None);
        let lent = epoch_engine::models::runtimes::lend_everything(&self.shared_weights());
        Ok(match lent {
            Ok(_) => format!("{} — llama.cpp and LM Studio both have it.", at.display()),
            Err(why) => format!(
                "{} — llama.cpp has it. LM Studio was not given it: {why}",
                at.display()
            ),
        })
    }

    /// What one model really costs, and whether it fits here.
    ///
    /// Takes any name, because the manifest endpoint does — somebody who knows they want
    /// `qwen3:14b` types it rather than hunting for it in a list that may not hold it.
    pub fn weigh_model(&self, name: &str) -> Result<epoch_engine::Offer, String> {
        epoch_engine::models::weigh(name.trim(), &epoch_engine::Machine::measure())
    }

    /// Search every catalogue at once (ADR-0031).
    ///
    /// **The caller never learns which one answered.** Sources that refuse contribute their
    /// refusal and the others still answer, because one busy site must not empty a screen.
    pub fn find_assets(
        &self,
        words: &str,
        kind: &str,
        adult: bool,
        order: &str,
        base: &str,
        more: &[MoreRow],
    ) -> AssetsFound {
        use epoch_engine::models::catalogue::{Catalogue, Order, Query};

        let civitai = epoch_engine::models::civitai::Civitai::new();
        let hugging_face = epoch_engine::models::hugging_face::HuggingFace::new();
        let voices = epoch_engine::models::voices::Voices::new();

        // **The question decides who is asked.** A voice lives inside one repository and neither
        // picture catalogue has a way to be asked about it; asking them anyway would fill a
        // voices shelf with checkpoints. This is the same routing `narrows_by_base` already does
        // three lines down — only the sources that can answer the question that was put.
        let picture: [&dyn Catalogue; 2] = [&civitai, &hugging_face];
        let sources: &[&dyn Catalogue] =
            if kind_named(kind) == Some(epoch_engine::assets::asset::Kind::Voice) {
                &[&voices]
            } else {
                &picture
            };

        // **Only the sources that have more, once paging has started.** One site running out is
        // not the end of the other, and re-asking a finished source for page four would put its
        // first page back on the screen.
        let mut found = Vec::new();
        let mut refused = Vec::new();
        let mut carry_on = Vec::new();
        for source in sources {
            let id = source_id(source.source());
            // **Only the sources that can answer the question that was put.** Civitai takes a
            // base model; Hugging Face publishes none, and filtering its answers on a word it
            // never prints would empty it. Showing it unfiltered beside a filtered list would be
            // worse still — a filter that half-works.
            if !base.trim().is_empty() && !source.narrows_by_base() {
                continue;
            }
            let cursor = more.iter().find(|row| row.source == id);
            if !more.is_empty() && cursor.is_none() {
                continue;
            }
            match source.search(&Query {
                words: words.to_owned(),
                kind: kind_named(kind),
                adult,
                order: match order {
                    "newest" => Order::Newest,
                    _ => Order::MostDownloaded,
                },
                base: base.trim().to_owned(),
                more: cursor.map(|row| row.cursor.clone()),
            }) {
                Ok(listing) => {
                    found.extend(listing.assets);
                    if let Some(next) = listing.more {
                        carry_on.push(MoreRow {
                            source: id.to_owned(),
                            cursor: next,
                        });
                    }
                }
                Err(why) => refused.push((source.source(), why)),
            }
        }
        // Merged in the order that was asked for, across every source at once. Sorting per
        // source and concatenating would rank the second site's best below the first site's
        // worst — and for *newest* there is nothing local to sort on, so the sites' own order is
        // interleaved as it arrived rather than re-sorted by a number that means something else.
        if order != "newest" {
            found.sort_by_key(|one| std::cmp::Reverse(one.downloads));
        }

        let held = self.secrets();
        // Measured once for the page rather than once per row: `nvidia-smi` is a process, and a
        // page of twenty-four would start twenty-four of them.
        let card = epoch_engine::models::machine::Machine::measure();
        AssetsFound {
            assets: found
                .into_iter()
                .map(|asset| {
                    let needs_key = asset.files.iter().any(|file| file.needs_key)
                        && !held.holds(&epoch_kernel::SecretName::for_catalogue(source_id(
                            asset.source,
                        )));
                    let bytes = asset.principal().map(|file| file.bytes).unwrap_or(0);
                    AssetView {
                        // Weighed against what is *free*, and `None` when there is no card to
                        // weigh against. A model that does not fit is still offered: it is their
                        // disk and their decision, and Epoch measured and said.
                        fits: (bytes > 0).then(|| card.fits(bytes)).flatten(),
                        bytes,
                        id: asset.id,
                        name: asset.name,
                        source: asset.source.name().to_owned(),
                        // Decided from the kind, which the catalogue measured — never from the
                        // id, and never from the site's name.
                        catalogue: catalogue_for(asset.kind, asset.source).to_owned(),
                        kind: asset.kind.plainly().to_owned(),
                        said_base: asset.said_base,
                        family: family_id(asset.family).to_owned(),
                        by: asset.by,
                        downloads: asset.downloads,
                        adult: asset.adult,
                        triggers: asset.triggers,
                        page: asset.page,
                        // Remembered here rather than fetched: a page of twenty results must
                        // not be twenty downloads before anything is drawn. The row asks for
                        // its own when it is on screen.
                        preview: asset
                            .preview
                            .as_deref()
                            .map(epoch_engine::models::catalogue::remember_preview),
                        // The same token store: it maps a token to an address and has no
                        // opinion about what is at the other end. What each is *checked to be*
                        // is decided where the bytes arrive.
                        sample: asset
                            .sample
                            .as_deref()
                            .map(epoch_engine::models::catalogue::remember_preview),
                        makes: asset
                            .makes
                            .map(|it| crate::state::studio::medium_id(it).to_owned()),
                        needs_key,
                        versions: if asset.versions.len() > 1 {
                            asset
                                .versions
                                .into_iter()
                                .map(|version| VersionRow {
                                    id: version.id,
                                    name: version.name,
                                    said_base: version.said_base,
                                    family: family_id(version.family).to_owned(),
                                    bytes: version.bytes,
                                })
                                .collect()
                        } else {
                            // One version is not a choice. A dropdown with a single entry is a
                            // control that changes nothing.
                            Vec::new()
                        },
                    }
                })
                .collect(),
            refused: refused
                .into_iter()
                .map(|(source, why)| format!("{}: {why}", source.name()))
                .collect(),
            more: carry_on,
            card: card.card(),
        }
    }

    /// Bring one asset into the library.
    ///
    /// **The Engine asks the catalogue again rather than trusting a URL from the window.** What a
    /// surface holds is an id; the address, the size and the hash come from the source at the
    /// moment of the download. A URL that crossed the IPC boundary is a URL somebody could have
    /// changed.
    ///
    /// Blocking, and honest about it: this is gigabytes. The window is told what landed, and the
    /// shelf it landed on decides itself from what the bytes turn out to be.
    pub fn install_asset(
        &self,
        source: &str,
        id: &str,
        // How far the download has got: bytes so far, and the total when the source stated one.
        // Called from the thread doing the reading, so the surface must not block in it.
        watching: &dyn Fn(u64, Option<u64>),
    ) -> Result<String, String> {
        use epoch_engine::models::catalogue::{self, Catalogue};
        use epoch_engine::models::generative::{Library, Shelf};

        let civitai = epoch_engine::models::civitai::Civitai::new();
        let hugging_face = epoch_engine::models::hugging_face::HuggingFace::new();
        let voices = epoch_engine::models::voices::Voices::new();
        let catalogue: &dyn Catalogue = match source {
            "civitai" | "Civitai" => &civitai,
            "huggingface" | "Hugging Face" => &hugging_face,
            // Its own name because it is its own question. The Voices shelf and the Creation
            // shelf both read Hugging Face, and they read different things out of it: one asks
            // what repositories exist, the other asks what is inside one.
            "voices" | "Voices" => &voices,
            other => return Err(format!("Epoch has no catalogue called {other:?}")),
        };

        let asset = catalogue
            .asset(id)
            .map_err(|why| why.to_string())?
            .ok_or_else(|| format!("{source} no longer has {id}"))?;
        let parts: Vec<epoch_engine::models::catalogue::File> =
            asset.parts().into_iter().cloned().collect();
        let file = parts
            .first()
            .ok_or("that has no file to download — it may have been withdrawn")?
            .clone();

        let key = self
            .secrets()
            .get(&epoch_kernel::SecretName::for_catalogue(source_id(
                asset.source,
            )))
            .filter(|key| !key.is_empty());
        if file.needs_key && key.is_none() {
            return Err(format!(
                "{} hands files over only to an account. Settings → Catalogue keys.",
                asset.source.name()
            ));
        }

        let library = Library::here();
        // The shelf follows what the catalogue says it is. What the *bytes* say is settled after
        // it arrives, and `identify` writes that down beside the file.
        let shelf = library.shelf(Shelf::for_kind(asset.kind));
        let landed = catalogue::fetch(
            &file,
            &shelf,
            key.as_ref().map(|key| key.expose()),
            watching,
        )
        .map_err(|why| why.to_string())?;

        // The rest of what has to land, when there is any. A sidecar is four kilobytes, so it
        // reports no progress of its own — but it is **not** best-effort: a voice whose sidecar
        // failed is a file that will refuse at the moment somebody tries to speak, minutes later
        // and somewhere else, so the failure belongs here where it can be read.
        for part in parts.iter().skip(1) {
            catalogue::fetch(
                part,
                &shelf,
                key.as_ref().map(|key| key.expose()),
                &|_, _| {},
            )
            .map_err(|why| {
                format!(
                    "{} arrived, and the file it needs beside it did not: {why}",
                    landed
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default()
                )
            })?;
        }

        let name = landed
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        // A voice is described by the file that arrived beside it, not by hashing the model and
        // asking a catalogue what it was. `understand` reads tensors to place a picture model on
        // a shelf; an `.onnx` carries none of what it looks for, so it would answer *its own
        // bytes could not be read* about a file that is perfectly fine. The sidecar is the
        // measurement here — and reading it now also proves it really landed.
        if asset.kind == epoch_engine::assets::asset::Kind::Voice {
            return Ok(
                match epoch_engine::models::voices::Spoken::beside(&landed) {
                    Some(spoken) if !spoken.plainly().is_empty() => {
                        format!("{name} — {}", spoken.plainly())
                    }
                    // It is installed and it will speak; Epoch simply cannot describe it.
                    // Saying that is better than inventing a language from the filename.
                    _ => format!("{name} — a voice, and its sidecar did not describe it"),
                },
            );
        }

        // Read now rather than on the next panel open, so a file that turns out to be something
        // else says so while somebody is still looking at it.
        let known = catalogue::identify(&landed, &[catalogue]);
        Ok(match known {
            Ok(known) => format!(
                "{name} — {}, {}",
                known.kind.plainly(),
                known.family.plainly()
            ),
            Err(why) => format!("{name} arrived, and its own bytes could not be read: {why}"),
        })
    }
}

/// One model, as this deck draws it.
///
/// **Defined in `epoch-models`, not here.** It moved the day the companion needed the same rows
/// and could not reach them: a second copy would have been two answers to one question, and the
/// question is which row of a measured curve Epoch marks.
pub use epoch_engine::models::deck::ModelHere;

/// A probe that says where it has got to, and changes nothing else.
///
/// **A wrapper rather than a field on `Bench`.** The measurement and the reporting of it are two
/// jobs, and putting the second inside the first would mean every future caller of `Bench` — a
/// test, the companion, a script — carried a callback it does not want. The count comes from the
/// only place that can be right about it: the call itself.
struct Watched<'a> {
    // Any probe, because a curve now runs through a child Epoch spawns (llama.cpp) or through a
    // server that is already answering (Ollama), and counting the rows is the same job either
    // way.
    inner: &'a mut dyn epoch_engine::models::loadout::Probe,
    seen: &'a mut usize,
    total: usize,
    say: &'a dyn Fn(usize, usize),
}

impl epoch_engine::models::loadout::Probe for Watched<'_> {
    fn run(
        &mut self,
        loadout: epoch_engine::models::loadout::Loadout,
    ) -> Option<epoch_engine::models::loadout::Run> {
        *self.seen += 1;
        (self.say)(*self.seen, self.total);
        self.inner.run(loadout)
    }
}

/// The best configuration found so far, for the progress line.
///
/// **Never a failed one.** A panel reporting a crashed configuration as the current best is
/// showing a number nobody can use, and somebody watching would stop the search on it.
fn best_so_far(
    found: &[epoch_engine::profiles::Configuration],
) -> Option<epoch_engine::profiles::Configuration> {
    found
        .iter()
        .filter(|it| it.stable)
        .max_by(|a, b| {
            a.generation
                .partial_cmp(&b.generation)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned()
}

/// Wait until llama.cpp's router lists something, or give up saying so.
///
/// **What is waited for is the model list, not the port.** The router binds and answers long
/// before it has read its presets, so a port check reports ready and the next request is refused
/// by a server that is not. The same lesson as ComfyUI's `/free` answering an empty 200: verify
/// the side effect by looking at the side effect.
fn wait_for_llama_cpp(patience: std::time::Duration) -> Result<(), String> {
    let began = std::time::Instant::now();
    while began.elapsed() < patience {
        let seen = epoch_engine::models::runtimes::look_for(
            epoch_engine::models::runtimes::Runtime::LlamaCpp,
        );
        if seen.serving && !seen.models.is_empty() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    }
    Err(format!(
        "llama.cpp did not come back within {}s of being restarted",
        patience.as_secs()
    ))
}

/// A card with its four columns already scored.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Scored {
    #[serde(flatten)]
    pub card: epoch_engine::card::Card,
    /// Three rates and their counts, per column. **Never collapsed on the way out** — the whole
    /// reason they are three is that one number could not say both how accurate a model was and
    /// how much of the work it finished.
    pub columns: Vec<epoch_engine::card::Column>,
}

/// One line, spoken.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spoken {
    /// Which kept file this is — `e5eba3ab800c4937.wav`, and nothing else.
    ///
    /// **A name, never a path and never the bytes** (ADR-0024 §2b). The address is the surface's
    /// to build, because how a custom scheme is spelled is a property of the platform the window
    /// runs on: on Windows it is served under `http://epoch.localhost`, and an Engine that
    /// hardcoded `epoch://` produced a URL nothing could play.
    pub sound: String,
    /// How long the engine took. **A measurement of this machine**, and the only honest way to
    /// answer *will speech keep up with a model*: it is 21x faster than real time here and
    /// nobody should assume that elsewhere.
    pub millis: u64,
    pub bytes: u64,
    /// What went wrong with the *optional* half, if anything: a timbre that is not converted
    /// here, or one that refused.
    ///
    /// `None` is nothing to say. It is on the answer rather than raised because the sentence
    /// was still spoken and the alternative -- failing the whole errand -- would silence
    /// somebody over a colour. **And it is not silent either**: falling back without a word
    /// would leave a character sounding like the wrong person with nothing on screen saying so.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub but: Option<String>,
}
