//! Where assets come from (ADR-0031).
//!
//! ## One shape, however many sites
//!
//! A catalogue is a place that can be asked *what exists* and *what is this file I already have*.
//! Hugging Face is one, Civitai is another, and the Engine may never branch on which one
//! answered — that is the whole content of ADR-0031, and the failure it prevents is three
//! searchers, three ways of asking whether 17.7 GB fits on a 17.2 GB machine, and three screens
//! that drift apart.
//!
//! ## A refusal is not an empty answer
//!
//! Measured, and it is why [`Refusal`] exists rather than `Result<_, String>` with prose in it:
//! on 2026-08-23 Civitai's free-text search answered **503 `Model search is temporarily
//! overloaded — please retry`** six times over several minutes and then worked perfectly. A
//! client that folded that into *nothing matched* would tell somebody their style does not exist
//! because a server was busy — the same failure as a cold instrument reading zero.
//!
//! So *busy*, *unreachable*, *unreadable* and *needs a key* are four different sentences with four
//! different fixes, and none of them is "no results".
//!
//! ## What a source says, and what Epoch measured
//!
//! An [`Asset`] carries both. `said_base` is the catalogue's own words — `Krea 2`, `Pony`,
//! `Flux.1 D` — kept verbatim because a person recognises them and because a closed enum will
//! always be behind the world. `family` is what Epoch can actually decide compatibility with, and
//! it is `Unknown` whenever the words do not map to something the compiler knows a graph for.
//!
//! Keeping one and dropping the other is the mistake in both directions: only the string and
//! nothing can check compatibility; only the enum and `Krea 2` becomes a blank.

use epoch_assets::asset::{Base, Kind};
use serde::{Deserialize, Serialize};

/// A place that can be asked what exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    HuggingFace,
    Civitai,
}

impl Source {
    pub const fn name(self) -> &'static str {
        match self {
            Source::HuggingFace => "Hugging Face",
            Source::Civitai => "Civitai",
        }
    }
}

/// Why a catalogue did not answer.
///
/// Four, because they have four different fixes. Collapsing them is how somebody ends up
/// reinstalling something over a busy server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "refusal", content = "said", rename_all = "camelCase")]
pub enum Refusal {
    /// The source is up and asked for a retry. Not an empty result.
    Busy(String),
    /// Nothing answered at all — offline, DNS, a timeout.
    Unreachable(String),
    /// It answered in a shape this cannot read. Worth saying: it means the site changed.
    Unreadable(String),
    /// It answered *no* because nobody is signed in. The fix is the user's own key.
    NeedsKey(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::Busy(said) => write!(out, "{said}"),
            Refusal::Unreachable(said) => write!(out, "{said}"),
            Refusal::Unreadable(said) => write!(out, "{said}"),
            Refusal::NeedsKey(said) => write!(out, "{said}"),
        }
    }
}

/// What a source says about reusing what it hosts.
///
/// Carried rather than judged. `CONTENT_PHILOSOPHY`'s hard rule governs what Epoch
/// **distributes**, not what somebody keeps in their own vault — so this exists to be shown, and
/// to be checked again at export.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Licence {
    /// The author may be left uncredited.
    pub no_credit_needed: bool,
    /// The ways commercial use is allowed, in the source's own words. Empty means none stated.
    pub commercial: Vec<String>,
    pub derivatives: bool,
    pub relicensing: bool,
}

/// One file a catalogue would hand over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct File {
    pub name: String,
    pub bytes: u64,
    /// Lowercase hex. What [`Catalogue::by_hash`] is asked with, and what a finished download is
    /// checked against.
    pub sha256: Option<String>,
    pub url: String,
    /// The download refuses without the user's own key. Measured per source, never assumed.
    pub needs_key: bool,
}

/// One published version of an asset.
///
/// **Not a detail; a different file for a different engine.** The LoRA the owner asked about is
/// published twice — 170 MB for Z-Image and 18 MB for Flux — and taking the largest fetched the
/// wrong one for a Flux graph. A version carries its own base and its own bytes, so a person
/// choosing one is choosing what will actually load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    /// Qualifies an [`Asset`] id: `civitai:2851202#3219757`.
    pub id: String,
    pub name: String,
    /// The source's own words for this version's base.
    pub said_base: String,
    /// What Epoch can build with, for this version.
    pub family: Base,
    pub bytes: u64,
}

/// Something a catalogue holds, in the one shape the Engine knows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    /// Unique across sources: `civitai:3219757`. Nothing keys on the name.
    pub id: String,
    pub name: String,
    pub source: Source,
    pub kind: Kind,
    /// What Epoch can decide compatibility with. `Unknown` is unmeasured, never incompatible.
    pub family: Base,
    /// What the source called it, verbatim — `Krea 2`, `Pony`, `Flux.1 D`.
    pub said_base: String,
    pub by: Option<String>,
    pub downloads: u64,
    /// The source marked it adult. Filtered by default and **said rather than hidden**.
    pub adult: bool,
    /// Words the author says a prompt needs. The reason this beats reading the file: most files
    /// do not carry them and every catalogue does.
    pub triggers: Vec<String>,
    pub licence: Licence,
    pub files: Vec<File>,
    /// Every version this asset was published in, newest first.
    ///
    /// Empty or one means there is nothing to choose. A source that publishes one tree per
    /// repository — Hugging Face — leaves this empty rather than inventing a version of one,
    /// which would be a control that changes nothing.
    #[serde(default)]
    pub versions: Vec<Version>,
    /// **Which medium the source placed this in**, and `None` where it placed it in none.
    ///
    /// ## A weaker measurement than a shelf's, and it has to say so
    ///
    /// A file on a shelf is read from its own bytes. A search result has not been downloaded and
    /// cannot be, so this is what the *source* said: Civitai publishes a base model, and where
    /// that maps to a family Epoch knows the medium follows exactly (`Base::makes`); Hugging
    /// Face publishes pipeline tags — `text-to-video`, `text-to-audio`, `text-to-3d` — which
    /// name the medium directly and are the better answer where they exist.
    ///
    /// `None` is **unplaced**, never *not this one*: a filtered list keeps showing it, exactly as
    /// the shelves keep showing a file Epoch could not read. That rule has been paid for once
    /// and is not being relearned here.
    pub makes: Option<epoch_assets::asset::Makes>,
    /// **A picture of what this makes**, as the source published it. `None` where it published
    /// none — Hugging Face mostly, which answers with a card and not with artwork.
    ///
    /// The URL and not the bytes: a page of twenty results must not be twenty downloads before
    /// anything is drawn. Each surface fetches the one it is about to show, through its own
    /// door, and neither hands this address to a browser (ADR-0024 §2b).
    pub preview: Option<String>,
    /// **A recording of what this sounds like**, as the source published it.
    ///
    /// Its own field and not `preview`, which is a *picture*. A voice has none, and putting an
    /// address to an mp3 in a field every surface draws as an `<img>` would be a real reading of
    /// the wrong quantity — the most convincing way a gauge lies.
    ///
    /// `None` where the source published nothing, which is most of them. A play button with
    /// nothing behind it is a control that does nothing.
    #[serde(default)]
    pub sample: Option<String>,
    /// The page a person can go and look at. Never fetched by Epoch — offered.
    pub page: String,
}

impl Asset {
    /// The file a download would take, when the choice is obvious.
    ///
    /// The largest, because a version's extra files are configs, previews and pruned variants;
    /// `None` when there are none, which a caller must handle rather than unwrap.
    pub fn principal(&self) -> Option<&File> {
        self.files.iter().max_by_key(|file| file.bytes)
    }

    /// Every file that has to land for this asset to be usable.
    ///
    /// ## Why this is not just `principal()`
    ///
    /// For a checkpoint or a LoRA the extra files really are configs, previews and pruned
    /// variants, and taking the largest is right. A **voice is not like that**: Piper reads the
    /// `.onnx.json` beside the model for its phoneme table, so the sidecar is not a detail — an
    /// `.onnx` on its own is a download that cannot speak.
    ///
    /// Deciding it here rather than in the installer keeps one answer to *what does installing
    /// this mean*. The installer branching on `Kind` would be the second place deciding, and two
    /// places deciding is how they come to disagree the first time a third kind arrives.
    pub fn parts(&self) -> Vec<&File> {
        match self.kind {
            // Both, in the order they are listed: the model, then the sidecar.
            Kind::Voice => self.files.iter().collect(),
            _ => self.principal().into_iter().collect(),
        }
    }
}

/// In what order a source should answer.
///
/// **Only what both sources genuinely do.** Civitai sorts by `Most Downloaded` and `Newest`;
/// Hugging Face by `downloads` and `createdAt`. Alphabetical is offered by neither, and a control
/// that could only reorder the page on screen would be a sort that lies about its scope.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    #[default]
    MostDownloaded,
    Newest,
}

/// What to ask a catalogue for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Query {
    pub words: String,
    /// Narrow to one kind, when the caller knows. `None` asks for everything.
    pub kind: Option<Kind>,
    pub order: Order,
    /// Narrow to one base model, **in the source's own words** — `Pony`, `Illustrious`,
    /// `Flux.1 D`. Empty asks for all of them.
    ///
    /// ## Why the site's vocabulary and not Epoch's
    ///
    /// Measured 2026-08-24 across 400 of Civitai's most-downloaded models: **38 distinct base
    /// labels**. Epoch knows five families, because five is how many *architectures* it can write
    /// a graph for — and those five cover about 92% of what was sampled, since Pony,
    /// Illustrious, NoobAI, Lightning and Hyper are all SDXL underneath.
    ///
    /// Those are two different questions. *What can Epoch draw with* is answered from the bytes,
    /// later, by `understand`. *What am I looking for* is answered by the word the site prints,
    /// and offering somebody five words for a shelf that has thirty-eight would hide most of it.
    pub base: String,
    /// Include what the source marked adult.
    pub adult: bool,
    /// Continue a previous listing. Opaque, and passed back exactly as it arrived.
    pub more: Option<String>,
}

/// How far one source got.
///
/// Opaque, and handed back exactly as it arrived: Civitai's is a whole next-page URL and Hugging
/// Face's is an offset into a fixed sort. Anything that took either apart would be guessing at
/// somebody else's private format.
///
/// Here rather than on a surface because **both** surfaces page — that is the whole content of
/// ADR-0031, and a second definition would be the second way of asking the same question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoreRow {
    pub source: String,
    pub cursor: String,
}

/// One page of an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    pub assets: Vec<Asset>,
    /// The cursor for the next page, when there is one. Opaque on purpose — it belongs to
    /// somebody else's API and anything that took it apart would be guessing.
    pub more: Option<String>,
}

/// A place assets come from.
///
/// Deliberately two questions. *What exists* is how somebody finds something; *what is this*
/// is how a file already on disk stops being anonymous — and the second is the one that makes
/// [`epoch_assets::asset::fingerprint`] worth paying for.
pub trait Catalogue {
    fn source(&self) -> Source;

    /// Whether this source can be asked to narrow by base model.
    ///
    /// **A real asymmetry, surfaced rather than faked.** Civitai takes `baseModels`; Hugging Face
    /// publishes no such field, and filtering its answers on a word it never prints would empty
    /// it. So a caller narrowing by base asks only the sources that can answer — and the screen
    /// says which those are, rather than showing a filter that half-works.
    fn narrows_by_base(&self) -> bool {
        false
    }

    /// What this source has, for that question.
    fn search(&self, query: &Query) -> Result<Listing, Refusal>;

    /// The files one asset would hand over.
    ///
    /// Its own question because sources weigh differently: Civitai's listing already carries
    /// every file with its size and hash, and Hugging Face's carries none of them (measured) —
    /// one extra request per row would make a shelf take a minute to open. So the list is cheap
    /// and the weighing happens when somebody points at one thing.
    ///
    /// The default answers with nothing, which is right for a source whose listing is already
    /// complete: the files are on the [`Asset`] and there is nothing to go and fetch.
    fn files_of(&self, _id: &str) -> Result<Vec<File>, Refusal> {
        Ok(Vec::new())
    }

    /// One asset, by the id a listing gave.
    ///
    /// Its own question because a shelf shows a page and a download needs one thing completely:
    /// the files, their sizes, their hashes. Passing an [`Asset`] back down from a surface would
    /// mean trusting a URL that came from outside the Engine.
    fn asset(&self, id: &str) -> Result<Option<Asset>, Refusal>;

    /// What this source knows about a file with that SHA-256.
    ///
    /// `Ok(None)` is a real answer — *this is not ours* — and different from every [`Refusal`].
    fn by_hash(&self, sha256: &str) -> Result<Option<Asset>, Refusal>;
}

/// Ask several catalogues one question.
///
/// **The caller never learns which one answered.** A source that refuses contributes its refusal
/// and the others still answer, because one busy site must not empty a screen — the same reason
/// the Workshop's two brain sources were never allowed to fail together.
pub fn ask_all(
    catalogues: &[&dyn Catalogue],
    query: &Query,
) -> (Vec<Asset>, Vec<(Source, Refusal)>) {
    let mut found = Vec::new();
    let mut refused = Vec::new();
    for catalogue in catalogues {
        match catalogue.search(query) {
            Ok(listing) => found.extend(listing.assets),
            Err(why) => refused.push((catalogue.source(), why)),
        }
    }
    // Most downloaded first, across every source at once. Sorting per source and concatenating
    // would rank the second site's best below the first site's worst.
    found.sort_by_key(|one| std::cmp::Reverse(one.downloads));
    (found, refused)
}

/// Everything known about one file on this machine.
///
/// The union of the two ways of knowing: what its own bytes say (always available, offline, and
/// impossible to fake) and what a catalogue says about that hash (a name, trigger words, a
/// licence — none of which a file carries reliably).
///
/// **The bytes outrank the catalogue on anything the bytes can answer.** A catalogue is right
/// about what a thing is *called*; the tensors are right about what it *is*, and only one of
/// those decides whether a render succeeds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Known {
    /// Lowercase hex, of this exact file.
    pub sha256: String,
    pub bytes: u64,
    /// Measured from the header. Never from the filename.
    pub kind: Kind,
    pub family: Base,
    /// What a catalogue calls it, when one recognised the hash.
    pub name: Option<String>,
    pub said_base: Option<String>,
    pub triggers: Vec<String>,
    pub page: Option<String>,
    pub licence: Option<Licence>,
    /// Which catalogue answered. `None` means nobody was asked, or nobody knew it.
    pub found_in: Option<Source>,
}

/// Where the answer is kept, so a 6.9 GB file is hashed once.
///
/// Beside the file rather than in a database: it survives a reinstall, it is obvious to a person
/// looking at the folder, and deleting the asset deletes what was known about it. A manifest for
/// a file that changed is detected by size, so an interrupted download that resumes is re-read
/// rather than trusted.
pub fn manifest_for(asset: &std::path::Path) -> std::path::PathBuf {
    let mut name = asset.file_name().unwrap_or_default().to_os_string();
    name.push(".epoch.json");
    asset.with_file_name(name)
}

/// Say everything that can be said about one file.
///
/// Reads the header first because it is free, then hashes, then asks — and stops asking the
/// moment a catalogue recognises the hash. A refusal is *not* recorded as "nobody knew it": the
/// manifest is written either way, but `found_in` stays `None` and the next call asks again.
pub fn identify(
    path: &std::path::Path,
    catalogues: &[&dyn Catalogue],
) -> Result<Known, epoch_assets::asset::Unreadable> {
    let read = epoch_assets::asset::understand(path)?;

    if let Some(kept) = kept_beside(path) {
        if kept.bytes == read.bytes && kept.found_in.is_some() {
            return Ok(kept);
        }
    }

    let sha256 = epoch_assets::asset::fingerprint(path)
        .map_err(|why| epoch_assets::asset::Unreadable::Refused(why.to_string()))?;

    let mut known = Known {
        sha256: sha256.clone(),
        bytes: read.bytes,
        kind: read.kind,
        family: read.base,
        name: None,
        // What the file itself claimed, until a catalogue says better.
        said_base: read.claimed.clone(),
        triggers: read.triggers.clone(),
        page: None,
        licence: None,
        found_in: None,
    };

    for catalogue in catalogues {
        if let Ok(Some(asset)) = catalogue.by_hash(&sha256) {
            known.name = Some(asset.name);
            known.said_base = Some(asset.said_base);
            known.page = Some(asset.page);
            known.licence = Some(asset.licence);
            known.found_in = Some(asset.source);
            if !asset.triggers.is_empty() {
                known.triggers = asset.triggers;
            }
            // The bytes keep the last word on the family: a catalogue that files a Flux LoRA
            // under an SDXL heading is wrong about the only thing that decides whether it loads.
            if known.family == Base::Unknown {
                known.family = asset.family;
            }
            break;
        }
    }

    if let Ok(written) = serde_json::to_vec_pretty(&known) {
        let _ = std::fs::write(manifest_for(path), written);
    }
    Ok(known)
}

/// What was written beside this file last time, when it is still readable.
/// The hash of a file Epoch has already read, **without reading it again**.
///
/// For a caller on a read path — a panel opening, a list being drawn — where hashing twelve
/// gigabytes to answer a question about advice would be the wrong trade. `None` means *nobody has
/// measured this file yet*, never *it has no hash*: the same distinction the whole surface keeps.
///
/// The size is checked because a manifest describes the bytes that were there when it was
/// written, and an interrupted download that resumed is a different file under the same name.
/// What a catalogue said a prompt needs for this file, from the manifest written beside it.
///
/// ## Why not the file's own header
///
/// `Asset::triggers` has said it since it existed: *most files do not carry them and every
/// catalogue does*. Epoch reads the trained words at install and writes them next to the bytes —
/// and the Studio Panel asked `understand()`, which reads `modelspec.trigger_phrase` out of the
/// safetensors header. Measured on the owner's `DiivesP1.safetensors`: the manifest holds
/// `["diives"]` and the header holds nothing, so the panel showed no trigger, the prompt went
/// without it, and a style LoRA with its trained word missing does very little — which is
/// exactly the picture he got.
///
/// Empty for a file nobody downloaded through Epoch, which is honest: nothing was ever said
/// about it.
/// What a catalogue called this file's base, verbatim, from the manifest written beside it.
///
/// ## A finer grain than Epoch measures, and it is the grain that matters here
///
/// `Base` is five families and `Pony`, `Illustrious` and `NoobAI` are all `Sdxl` in it — which
/// is right for what `Base` decides (a LoRA of one loads against a model of another) and useless
/// for choosing between them. Measured on the owner's own files: `DiivesP1` says **Pony**,
/// `DiivesIXL` says **Illustrious**, and Epoch reads both as SDXL, so the panel showed one word
/// for two things that draw differently.
///
/// **Said, never enforced.** This is the site's claim and not a measurement, so nothing greys
/// and nothing refuses — the picture is the evidence, and his best result came from the pairing
/// these words call mismatched.
pub fn said_base_beside(path: &std::path::Path) -> Option<String> {
    kept_beside(path)?.said_base
}

pub fn triggers_beside(path: &std::path::Path) -> Vec<String> {
    kept_beside(path)
        .map(|known| known.triggers)
        .unwrap_or_default()
}

pub fn hash_beside(path: &std::path::Path) -> Option<String> {
    let kept = kept_beside(path)?;
    let now = std::fs::metadata(path).ok()?.len();
    (kept.bytes == now).then_some(kept.sha256)
}

fn kept_beside(path: &std::path::Path) -> Option<Known> {
    let said = std::fs::read_to_string(manifest_for(path)).ok()?;
    serde_json::from_str(&said).ok()
}

/// Every preview a search has answered with, by the token the surfaces address it as.
///
/// ## Why a token and not the address
///
/// **The window never holds a URL to a third party.** ADR-0024 §2b: the page hands over a *name*
/// and the Engine decides what it resolves to — which is what keeps `img-src` at `'self' data:
/// epoch:` and keeps every request to Civitai coming from the Engine rather than from the
/// user's browser context. A row carrying `https://image.civitai.com/…` would be one `<img>`
/// away from that guarantee being spent.
///
/// So a search remembers what it was told, hands out sixteen hex digits, and resolves them back
/// here. In memory and for this run only: a token from a search nobody ran is a token that
/// resolves to nothing, which is the honest answer.
static PREVIEWS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<String, String>>> =
    std::sync::LazyLock::new(Default::default);

/// Remember one preview address and answer with the token that stands for it.
pub fn remember_preview(url: &str) -> String {
    use sha2::{Digest as _, Sha256};
    let token = Sha256::digest(url.as_bytes())
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    if let Ok(mut held) = PREVIEWS.lock() {
        held.insert(token.clone(), url.to_owned());
    }
    token
}

/// The bytes of one remembered preview, fetched once and kept.
///
/// **Kept on disk under the token**, which is a hash of the address, so a search run twice costs
/// one download and a scroll back up costs none. The same bargain the picture scheme already
/// makes, and the reason both name a file after something that cannot mean two pictures.
///
/// `None` for a token nobody remembered, a source that refused, and anything that did not come
/// back looking like an image — measured from the bytes, never from the address, because a
/// redirect can land anywhere (ADR-0024).
pub fn preview_bytes(token: &str, into: &std::path::Path) -> Option<Vec<u8>> {
    if token.len() != 16 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let kept = into.join(format!("{token}.preview"));
    if let Ok(bytes) = std::fs::read(&kept) {
        return Some(bytes);
    }
    let url = PREVIEWS.lock().ok()?.get(token)?.clone();
    let answer = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .ok()?;
    let mut bytes = Vec::new();
    {
        use std::io::Read as _;
        // A cap, because this is somebody else's server answering: a preview is tens of
        // kilobytes and anything claiming to be sixteen megabytes of one is not a preview.
        std::io::copy(&mut answer.into_reader().take(16 << 20), &mut bytes).ok()?;
    }
    /*
        **Is this a picture at all**, from its own first bytes rather than from the address it
        came from — a redirect can land anywhere, and a source that answers with an error page
        answers with two hundred and some HTML.

        Deliberately *not* `import::ImageFormat`, which asks a bigger question and lives in the
        Engine: `epoch-models` is the crate both surfaces share and ADR-0029 keeps the Engine out
        of EpochServices. This is the smaller question — four magic numbers, no format returned,
        and nothing downstream branches on the answer.
    */
    let looks_like_a_picture = bytes.starts_with(&[0x89, b'P', b'N', b'G'])
        || bytes.starts_with(&[0xff, 0xd8, 0xff])
        || bytes.starts_with(b"GIF8")
        || (bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP");
    if !looks_like_a_picture {
        return None;
    }
    let _ = std::fs::create_dir_all(into);
    let _ = std::fs::write(&kept, &bytes);
    Some(bytes)
}

/// The bytes of one remembered **recording**, fetched once and kept.
///
/// A sibling of [`preview_bytes`] rather than a flag on it, because the two answer different
/// questions about what came back: *is this a picture* and *is this a sound* are different
/// checks, and one function taking a boolean would be one place deciding two things.
///
/// The check itself is the same discipline: **read from the first bytes, never from the
/// address**. A redirect can land anywhere, and a server answering with an error page answers
/// with two hundred and some HTML — which would arrive as a play button that produces silence.
pub fn sample_bytes(token: &str, into: &std::path::Path) -> Option<Vec<u8>> {
    if token.len() != 16 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let kept = into.join(format!("{token}.sample"));
    if let Ok(bytes) = std::fs::read(&kept) {
        return Some(bytes);
    }
    let url = PREVIEWS.lock().ok()?.get(token)?.clone();
    let answer = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .ok()?;
    let mut bytes = Vec::new();
    {
        use std::io::Read as _;
        // A cap, for the reason the picture one has: a sample is a couple of hundred kilobytes,
        // measured, and anything claiming to be thirty-two megabytes of one is not a sample.
        std::io::copy(&mut answer.into_reader().take(32 << 20), &mut bytes).ok()?;
    }
    // MP3 with or without an ID3 tag, WAV, OGG, FLAC. Four magic numbers, nothing returned, and
    // nothing downstream branches on which it was.
    let sounds_like_audio = bytes.starts_with(b"ID3")
        || (bytes.len() > 2 && bytes[0] == 0xff && (bytes[1] & 0xe0) == 0xe0)
        || (bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE")
        || bytes.starts_with(b"OggS")
        || bytes.starts_with(b"fLaC");
    if !sounds_like_audio {
        return None;
    }
    let _ = std::fs::create_dir_all(into);
    let _ = std::fs::write(&kept, &bytes);
    Some(bytes)
}

/// Bring one file onto this machine.
///
/// ## Streamed, and named by Epoch
///
/// Written a megabyte at a time, so a seven-gigabyte checkpoint costs time and not memory. The
/// filename comes from the catalogue's own `name` with every path separator refused — ADR-0024's
/// rule, one subsystem over: a name that arrives from outside may not decide where a file lands.
///
/// ## Verified, when the source said what to verify against
///
/// A finished download is checked against the hash the catalogue published, and a mismatch
/// **deletes the file**. A corrupt 6 GB checkpoint that stays on the shelf is worse than a failed
/// download: it will be selected, it will fail inside a render, and nothing will say why.
///
/// A source that published no hash gets no check and says so by returning `None` for it — never a
/// silent pass.
///
/// ## It writes beside itself, then moves
///
/// A partial file with the real name is a file the shelf would list and the panel would offer.
/// So it lands as `<name>.part` and is renamed only once it is whole.
pub fn fetch(
    file: &File,
    into: &std::path::Path,
    key: Option<&str>,
    // How far it has got, called as the body arrives: bytes so far, and the total when the
    // source stated one. See [`fetch`]'s own note on why the second is an `Option`.
    watching: &dyn Fn(u64, Option<u64>),
) -> Result<std::path::PathBuf, Refusal> {
    let name = file
        .name
        .rsplit(['/', BACKSLASH])
        .next()
        .unwrap_or_default()
        .trim();
    if name.is_empty() || name.starts_with('.') {
        return Err(Refusal::Unreadable(format!(
            "{:?} is not a filename Epoch will write.",
            file.name
        )));
    }

    let mut request = ureq::get(&file.url).timeout(std::time::Duration::from_secs(60 * 60));
    if let Some(key) = key {
        request = request.set("Authorization", &format!("Bearer {key}"));
    }
    let answer = match request.call() {
        Ok(answer) => answer,
        Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => {
            return Err(Refusal::NeedsKey(
                "That site will not hand the file over without your own API key. Settings → \
                 Catalogue keys."
                    .to_owned(),
            ))
        }
        Err(why) => {
            return Err(Refusal::Unreachable(format!(
                "the download did not start: {why}"
            )))
        }
    };

    std::fs::create_dir_all(into)
        .map_err(|why| Refusal::Unreadable(format!("the shelf could not be made: {why}")))?;
    let landing = into.join(format!("{name}.part"));
    let final_path = into.join(name);

    /*
        **What the source said it is sending, when it said anything.**

        `Content-Length` is a claim by the server and most of them make it; a redirect chain, a
        chunked response or a CDN that streams may not. `None` there is *no total*, and every
        surface must show bytes arrived rather than a bar guessing at a denominator — a
        percentage invented from nothing is the gauge this codebase keeps deleting.

        Read before the body, because reading it afterwards means reading it from a response
        that has already been consumed.
    */
    let expected = answer
        .header("Content-Length")
        .and_then(|said| said.parse::<u64>().ok())
        // A source that states a size in its own catalogue is a second opinion, and the better
        // one where the transport says nothing: it is what the row already shows.
        .or(if file.bytes > 0 {
            Some(file.bytes)
        } else {
            None
        });

    let outcome = (|| -> std::io::Result<String> {
        use sha2::{Digest as _, Sha256};
        use std::io::{Read as _, Write as _};

        let mut reading = answer.into_reader();
        let mut writing = std::io::BufWriter::new(std::fs::File::create(&landing)?);
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 1 << 20];
        let mut so_far = 0u64;
        // **Said as it goes, and not on every chunk.** A megabyte at a time on a fast line is
        // hundreds of events a second, all of them redrawing the same bar; a quarter of a second
        // is faster than anybody reads and cheap enough to ignore.
        let mut last = std::time::Instant::now();
        watching(0, expected);
        loop {
            let read = reading.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            writing.write_all(&buffer[..read])?;
            so_far += read as u64;
            if last.elapsed() >= std::time::Duration::from_millis(250) {
                last = std::time::Instant::now();
                watching(so_far, expected);
            }
        }
        writing.flush()?;
        // The last one is unconditional, so a finished download does not sit at 98% because its
        // final chunk arrived inside the quarter second.
        watching(so_far, expected);
        Ok(hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    })();

    let arrived = match outcome {
        Ok(hash) => hash,
        Err(why) => {
            let _ = std::fs::remove_file(&landing);
            return Err(Refusal::Unreachable(format!("the download stopped: {why}")));
        }
    };

    if let Some(expected) = &file.sha256 {
        if !expected.eq_ignore_ascii_case(&arrived) {
            let _ = std::fs::remove_file(&landing);
            return Err(Refusal::Unreadable(
                "What arrived is not the file that was published — its hash does not match. It \
                 has been deleted rather than left on the shelf."
                    .to_owned(),
            ));
        }
    }

    std::fs::rename(&landing, &final_path).map_err(|why| {
        let _ = std::fs::remove_file(&landing);
        Refusal::Unreadable(format!(
            "it downloaded and could not be put in place: {why}"
        ))
    })?;
    Ok(final_path)
}

/// A path separator, spelled without one so this file has no escape in it.
const BACKSLASH: char = '\\';

#[cfg(test)]
mod tests {
    use super::*;

    struct Silent(Source, Option<Refusal>, Vec<Asset>);

    impl Catalogue for Silent {
        fn source(&self) -> Source {
            self.0
        }
        fn search(&self, _query: &Query) -> Result<Listing, Refusal> {
            match &self.1 {
                Some(why) => Err(why.clone()),
                None => Ok(Listing {
                    assets: self.2.clone(),
                    more: None,
                }),
            }
        }
        fn asset(&self, _id: &str) -> Result<Option<Asset>, Refusal> {
            Ok(self.2.first().cloned())
        }
        fn by_hash(&self, _sha256: &str) -> Result<Option<Asset>, Refusal> {
            Ok(None)
        }
    }

    fn asset(id: &str, source: Source, downloads: u64) -> Asset {
        Asset {
            preview: None,
            makes: None,
            id: id.to_owned(),
            name: id.to_owned(),
            source,
            kind: Kind::Lora,
            family: Base::Unknown,
            said_base: "whatever".to_owned(),
            by: None,
            downloads,
            adult: false,
            triggers: Vec::new(),
            licence: Licence::default(),
            files: Vec::new(),
            versions: Vec::new(),
            sample: None,
            page: String::new(),
        }
    }

    #[test]
    fn one_busy_source_does_not_empty_the_answer() {
        let up = Silent(
            Source::HuggingFace,
            None,
            vec![asset("a", Source::HuggingFace, 10)],
        );
        let down = Silent(
            Source::Civitai,
            Some(Refusal::Busy("please retry".to_owned())),
            Vec::new(),
        );
        let (found, refused) = ask_all(&[&up, &down], &Query::default());
        assert_eq!(found.len(), 1);
        // And the refusal is reported rather than swallowed: somebody must be able to learn that
        // half the world was not asked.
        assert_eq!(refused.len(), 1);
        assert!(matches!(refused[0], (Source::Civitai, Refusal::Busy(_))));
    }

    #[test]
    fn the_best_of_both_sources_outranks_the_worst_of_either() {
        let hf = Silent(
            Source::HuggingFace,
            None,
            vec![asset("hf-small", Source::HuggingFace, 5)],
        );
        let civitai = Silent(
            Source::Civitai,
            None,
            vec![asset("civitai-big", Source::Civitai, 900)],
        );
        let (found, _) = ask_all(&[&hf, &civitai], &Query::default());
        assert_eq!(found[0].id, "civitai-big");
    }

    #[test]
    fn the_principal_file_is_the_one_worth_downloading() {
        let mut it = asset("x", Source::Civitai, 0);
        it.files = vec![
            File {
                name: "preview.png".to_owned(),
                bytes: 900_000,
                sha256: None,
                url: String::new(),
                needs_key: false,
            },
            File {
                name: "model.safetensors".to_owned(),
                bytes: 228_449_016,
                sha256: None,
                url: String::new(),
                needs_key: true,
            },
        ];
        assert_eq!(it.principal().unwrap().name, "model.safetensors");
        // A version with no files at all is a real state on Civitai (early or withdrawn), and
        // the caller has to be able to say so rather than crash.
        assert_eq!(asset("y", Source::Civitai, 0).principal(), None);
    }

    #[test]
    fn a_manifest_sits_beside_its_file_and_names_it() {
        let kept = manifest_for(std::path::Path::new("/library/loras/pixel.safetensors"));
        assert!(kept.ends_with("pixel.safetensors.epoch.json"), "{kept:?}");
        // The extension is kept rather than replaced: two files differing only in extension are
        // two assets, and `with_extension` would have given them one manifest.
        let other = manifest_for(std::path::Path::new("/library/loras/pixel.ckpt"));
        assert_ne!(kept.file_name(), other.file_name());
    }

    #[test]
    fn what_the_bytes_say_survives_what_a_catalogue_says() {
        // A catalogue filing a Flux LoRA under SDXL is wrong about the one thing that decides
        // whether it loads, so the header wins and the catalogue only fills in the blanks.
        let dir = std::env::temp_dir().join("epoch-identify-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("anything.safetensors");
        let head = serde_json::to_vec(&serde_json::json!({
            "transformer.single_transformer_blocks.0.attn.to_q.lora_A.weight":
                {"dtype": "F16", "shape": [1, 1], "data_offsets": [0, 4]},
        }))
        .unwrap();
        let mut bytes = (head.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(&head);
        bytes.extend_from_slice(&[0u8; 4]);
        std::fs::write(&path, &bytes).unwrap();

        let mut lying = asset("civitai:1", Source::Civitai, 3);
        lying.family = Base::Sdxl;
        lying.said_base = "Pony".to_owned();
        lying.name = "Something".to_owned();
        lying.triggers = vec!["a word".to_owned()];
        let liar = Wrong(lying);

        let known = identify(&path, &[&liar]).unwrap();
        assert_eq!(known.family, Base::Flux, "the tensors decide the family");
        assert_eq!(
            known.name.as_deref(),
            Some("Something"),
            "the catalogue names it"
        );
        assert_eq!(known.said_base.as_deref(), Some("Pony"), "in its own words");
        assert_eq!(known.triggers, vec!["a word"]);
        assert!(manifest_for(&path).is_file(), "and it is written down");

        // Second time: the manifest answers, so nothing is hashed and nothing is asked.
        let again = identify(&path, &[&Silent(Source::Civitai, None, Vec::new())]).unwrap();
        assert_eq!(again, known);

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A catalogue that recognises everything and is wrong about the family.
    struct Wrong(Asset);

    impl Catalogue for Wrong {
        fn source(&self) -> Source {
            self.0.source
        }
        fn search(&self, _query: &Query) -> Result<Listing, Refusal> {
            Ok(Listing {
                assets: vec![self.0.clone()],
                more: None,
            })
        }
        fn asset(&self, _id: &str) -> Result<Option<Asset>, Refusal> {
            Ok(Some(self.0.clone()))
        }
        fn by_hash(&self, _sha256: &str) -> Result<Option<Asset>, Refusal> {
            Ok(Some(self.0.clone()))
        }
    }

    #[test]
    fn a_filename_from_outside_cannot_choose_where_a_file_lands() {
        // ADR-0024's rule, one subsystem over. Neither of these reaches the parent directory.
        let hostile = File {
            name: "../../windows/system32/evil.safetensors".to_owned(),
            bytes: 1,
            sha256: None,
            url: "http://127.0.0.1:1/never".to_owned(),
            needs_key: false,
        };
        let dir = std::env::temp_dir().join("epoch-fetch-test");
        // It never gets as far as the network: the name is refused, or it resolves to the leaf.
        match fetch(&hostile, &dir, None, &|_, _| {}) {
            Err(Refusal::Unreachable(_)) => {
                // The name was reduced to `evil.safetensors` and the *URL* is what failed, which
                // is the correct order: nothing was written outside `dir`.
                assert!(!dir.join("..").join("evil.safetensors").exists());
            }
            Err(other) => panic!("{other:?}"),
            Ok(landed) => panic!("nothing should have arrived: {landed:?}"),
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_name_that_is_only_a_path_is_refused_outright() {
        let nameless = File {
            name: "some/dir/".to_owned(),
            bytes: 1,
            sha256: None,
            url: "http://127.0.0.1:1/never".to_owned(),
            needs_key: false,
        };
        let why = fetch(&nameless, &std::env::temp_dir(), None, &|_, _| {}).unwrap_err();
        assert!(matches!(why, Refusal::Unreadable(_)), "{why:?}");
    }
}
