//! Hugging Face, as a [`Catalogue`] (Phase 11.3).
//!
//! ## Why this is not `hf.rs`
//!
//! `hf.rs` drives the **CLI** to fetch a GGUF for a brain. This reads the **API** to find weights
//! that draw. Same company, different question — and ADR-0031's amendment is exactly about not
//! putting `gemma4:12b` and `sd_xl_base_1.0.safetensors` in one list because they are both large
//! files.
//!
//! ## Measured, 2026-08-23
//!
//! ```text
//! GET /api/models?filter=lora&sort=downloads&limit=2&full=true → 200, no key
//! GET /api/models/<id>/tree/main?recursive=true                → 200: size, and lfs.oid
//! ```
//!
//! Two things follow.
//!
//! **The listing has no sizes.** `full=true` names the files and weighs none of them, so a shelf
//! that showed sizes would need one extra request per row. That is what [`Catalogue::files_of`]
//! is for: the list is cheap, and the weighing happens when somebody points at one thing.
//!
//! **`lfs.oid` is the SHA-256.** git-lfs addresses by content, so the hash a download must be
//! checked against is already in the tree — no second source of truth, and no hashing on this end
//! to find out whether a download arrived intact.
//!
//! **There is no lookup by hash.** Hugging Face indexes repositories, not file digests, so
//! [`Catalogue::by_hash`] answers `Ok(None)` — *not ours to say* — rather than pretending to have
//! asked. Civitai is the source that can answer that question, and one source answering it is
//! enough for a file on disk to stop being anonymous.

use epoch_assets::asset::{Base, Kind};

use crate::catalogue::{Asset, Catalogue, File, Licence, Listing, Query, Refusal, Source};

const API: &str = "https://huggingface.co/api";
const HOW_LONG: std::time::Duration = std::time::Duration::from_secs(20);
const PAGE: usize = 24;

/// The site, asked over HTTP. Nothing here needs an account.
#[derive(Debug, Clone, Default)]
pub struct HuggingFace;

impl HuggingFace {
    pub fn new() -> Self {
        Self
    }

    fn get(&self, url: &str) -> Result<serde_json::Value, Refusal> {
        match ureq::get(url).timeout(HOW_LONG).call() {
            Ok(answer) => answer.into_json().map_err(|why| {
                Refusal::Unreadable(format!(
                    "Hugging Face answered something this cannot read: {why}"
                ))
            }),
            Err(ureq::Error::Status(429 | 503, _)) => Err(Refusal::Busy(
                "Hugging Face is rate-limiting right now. This is not an empty result.".to_owned(),
            )),
            Err(ureq::Error::Status(401 | 403, _)) => Err(Refusal::NeedsKey(
                "Hugging Face refused without an account — that repository is gated.".to_owned(),
            )),
            Err(ureq::Error::Status(404, _)) => Err(Refusal::Unreadable(
                "Hugging Face has nothing at that address.".to_owned(),
            )),
            Err(ureq::Error::Status(code, _)) => Err(Refusal::Unreachable(format!(
                "Hugging Face answered {code}."
            ))),
            Err(why) => Err(Refusal::Unreachable(format!(
                "Hugging Face did not answer: {why}"
            ))),
        }
    }
}

impl Catalogue for HuggingFace {
    fn source(&self) -> Source {
        Source::HuggingFace
    }

    fn search(&self, query: &Query) -> Result<Listing, Refusal> {
        // **A page number, not a cursor.** Hugging Face pages a sorted list by `skip`, and the
        // sort is fixed here — most downloaded, descending. That makes an offset stable enough
        // to page with, which a `Link` header this client never sees would not be.
        let page: usize = query
            .more
            .as_deref()
            .and_then(|at| at.parse().ok())
            .unwrap_or(0);
        let mut url = format!(
            "{API}/models?sort={}&direction=-1&limit={PAGE}&skip={}&full=true",
            match query.order {
                crate::catalogue::Order::MostDownloaded => "downloads",
                crate::catalogue::Order::Newest => "createdAt",
            },
            page * PAGE
        );
        url.push_str("&filter=");
        url.push_str(filter_for(query.kind));
        let words = query.words.trim();
        if !words.is_empty() {
            url.push_str("&search=");
            url.push_str(&urlencode(words));
        }

        let answer = self.get(&url)?;
        Ok(Listing {
            assets: answer
                .as_array()
                .map(|models| models.iter().filter_map(read_model).collect())
                .unwrap_or_default(),
            // One more page is offered only while this one came back full. A short page is the
            // end of the list, and offering NEXT there would be a control that answers nothing.
            more: (answer.as_array().map(|it| it.len()).unwrap_or(0) == PAGE)
                .then(|| (page + 1).to_string()),
        })
    }

    fn files_of(&self, id: &str) -> Result<Vec<File>, Refusal> {
        let repo = id.strip_prefix("huggingface:").unwrap_or(id);
        let tree = self.get(&format!("{API}/models/{repo}/tree/main?recursive=true"))?;
        Ok(tree
            .as_array()
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| entry.get("type").and_then(|it| it.as_str()) == Some("file"))
                    .filter_map(|entry| {
                        let path = entry.get("path")?.as_str()?;
                        // Only what something could load. A repository is full of configs,
                        // READMEs and preview images, and offering one as a download is offering
                        // a download that cannot be used.
                        if !path.ends_with(".safetensors") && !path.ends_with(".sft") {
                            return None;
                        }
                        Some(File {
                            name: path.to_owned(),
                            bytes: entry.get("size")?.as_u64()?,
                            // git-lfs addresses by content, so this *is* the SHA-256.
                            sha256: entry
                                .get("lfs")
                                .and_then(|it| it.get("oid"))
                                .and_then(|it| it.as_str())
                                .map(|oid| oid.to_ascii_lowercase()),
                            url: format!("https://huggingface.co/{repo}/resolve/main/{path}"),
                            // Measured: a public repository downloads without one. A gated repo
                            // answers 401, and that arrives as `NeedsKey` at the moment it does.
                            needs_key: false,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    fn asset(&self, id: &str) -> Result<Option<Asset>, Refusal> {
        let repo = id.strip_prefix("huggingface:").unwrap_or(id);
        let model = match self.get(&format!("{API}/models/{repo}")) {
            Ok(model) => model,
            Err(Refusal::Unreadable(said)) if said.contains("nothing at that address") => {
                return Ok(None)
            }
            Err(why) => return Err(why),
        };
        let Some(mut asset) = read_model(&model) else {
            return Ok(None);
        };
        // Weighed here and not in the listing: this is the one row somebody pointed at.
        asset.files = self.files_of(id)?;
        Ok(Some(asset))
    }

    /// Hugging Face indexes repositories, not digests.
    ///
    /// `Ok(None)` is the honest answer — *this source cannot recognise a file by its hash* — and
    /// it is deliberately the same answer as *we do not have it*, because to a caller trying to
    /// name an anonymous file the two are the same outcome and neither is a fault.
    fn by_hash(&self, _sha256: &str) -> Result<Option<Asset>, Refusal> {
        Ok(None)
    }
}

/// The Hugging Face filter that comes closest to a kind.
///
/// `diffusers` for everything unnarrowed, because that is the library tag every model that draws
/// carries and it is the only thing keeping language models out of a picture shelf.
const fn filter_for(kind: Option<Kind>) -> &'static str {
    match kind {
        Some(Kind::Lora) => "lora",
        Some(Kind::ControlNet) => "controlnet",
        _ => "diffusers",
    }
}

/// Which medium a repository's own tags place it in.
///
/// **Read from what Hugging Face publishes, and it publishes it directly.** A pipeline tag is
/// the medium in as many words — `text-to-video`, `text-to-audio`, `text-to-3d` — which makes it
/// a better answer than a base model, where the medium is only implied. Measured against the
/// live API on 2026-08-30: the tag list of a video repository carries `text-to-video` and
/// `audio-text-to-video` beside the plain word `video`.
///
/// **The suffix and not the whole tag**, because the front half is what it takes and the back
/// half is what it makes: `image-to-video`, `audio-text-to-video` and `text-to-video` all make a
/// video, and enumerating every prefix somebody might publish is a list that goes stale.
///
/// `None` where nothing says: an unplaced result is shown under every medium rather than hidden
/// from the one it belongs to.
fn makes_from(tags: &[&str]) -> Option<epoch_assets::asset::Makes> {
    use epoch_assets::asset::Makes;
    // Order matters only in that the first tag to answer wins, and a repository that claims two
    // media has told us one thing about itself either way.
    tags.iter().find_map(|tag| {
        let made = tag.rsplit_once("-to-").map(|(_, made)| made)?;
        match made {
            "video" => Some(Makes::Video),
            "audio" | "speech" => Some(Makes::Sound),
            "3d" => Some(Makes::Model),
            "image" => Some(Makes::Picture),
            _ => None,
        }
    })
}

fn read_model(model: &serde_json::Value) -> Option<Asset> {
    let id = model.get("id")?.as_str()?.to_owned();
    let tags: Vec<&str> = model
        .get("tags")
        .and_then(|it| it.as_array())
        .map(|tags| tags.iter().filter_map(|tag| tag.as_str()).collect())
        .unwrap_or_default();

    // `base_model:adapter:Comfy-Org/MiniMax-H3` and `base_model:Comfy-Org/MiniMax-H3` both
    // appear, measured. The last colon-separated part is the repository either way.
    let said_base = tags
        .iter()
        .find(|tag| tag.starts_with("base_model:"))
        .and_then(|tag| tag.rsplit(':').next())
        .unwrap_or("")
        .to_owned();

    Some(Asset {
        // **What Hugging Face said it does**, which is the better answer where it exists: a
        // pipeline tag names the medium directly, and a base model only implies it.
        makes: makes_from(&tags),
        // **None, and it stays None until it is measured.** Hugging Face answers with a model
        // card, and a card is Markdown that may or may not open with an image hosted anywhere.
        // Guessing a thumbnail URL from a repository name is exactly the invented reading this
        // codebase deletes; a row with no picture says so by having none.
        preview: None,
        name: id.clone(),
        id: format!("huggingface:{id}"),
        source: Source::HuggingFace,
        kind: kind_from(&tags),
        family: family_from(&said_base, &tags),
        said_base,
        by: id.split('/').next().map(str::to_owned),
        downloads: model
            .get("downloads")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        // Hugging Face has a `not-for-all-audiences` tag and marks nothing else. Reading only
        // what it states keeps this from implying a judgement nobody made.
        adult: tags.contains(&"not-for-all-audiences"),
        // Trigger words are not something a repository states in a place this can read. Empty is
        // *not declared*, and the file's own metadata may still carry them (ADR-0032).
        triggers: Vec::new(),
        licence: Licence::default(),
        // The listing weighs nothing. `files_of` is how a shelf gets sizes for one row.
        files: Vec::new(),
        // One repository, one tree. Inventing a version of one would be a control that changes
        // nothing, which is worse than no control.
        versions: Vec::new(),
        sample: None,
        page: format!("https://huggingface.co/{id}"),
    })
}

fn kind_from(tags: &[&str]) -> Kind {
    let has = |needle: &str| tags.iter().any(|tag| tag.eq_ignore_ascii_case(needle));
    if has("lora") {
        return Kind::Lora;
    }
    if has("controlnet") {
        return Kind::ControlNet;
    }
    if has("textual_inversion") || has("textual-inversion") {
        return Kind::Embedding;
    }
    if has("text-to-image") || has("diffusers") {
        return Kind::Checkpoint;
    }
    Kind::Unknown
}

/// The family, from what a repository declares.
///
/// The base-model tag first, because it is the one thing stated rather than inferred; then the
/// pipeline tags, which name the architecture in the same words Civitai does.
fn family_from(said_base: &str, tags: &[&str]) -> Base {
    let all = format!("{said_base} {}", tags.join(" ")).to_ascii_lowercase();
    if all.contains("flux") {
        return Base::Flux;
    }
    if all.contains("stable-diffusion-3") || all.contains("sd3") {
        return Base::Sd3;
    }
    if all.contains("xl") || all.contains("pony") || all.contains("illustrious") {
        return Base::Sdxl;
    }
    if all.contains("stable-diffusion-2") {
        return Base::Sd2;
    }
    if all.contains("stable-diffusion-v1") || all.contains("sd-v1") || all.contains("sd15") {
        return Base::Sd15;
    }
    Base::Unknown
}

fn urlencode(what: &str) -> String {
    what.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Trimmed from what the live API really returned, so a change in its shape breaks a test.
    fn a_real_row() -> serde_json::Value {
        json!({
            "id": "lightx2v/Qwen-Image-Lightning",
            "downloads": 439014,
            "tags": ["diffusers", "Qwen-Image", "distillation", "LoRA", "lora", "text-to-image"]
        })
    }

    #[test]
    fn a_repository_reads_as_one_asset_with_nothing_invented() {
        let read = read_model(&a_real_row()).unwrap();
        assert_eq!(read.id, "huggingface:lightx2v/Qwen-Image-Lightning");
        assert_eq!(read.kind, Kind::Lora, "the lora tag outranks text-to-image");
        assert_eq!(read.by.as_deref(), Some("lightx2v"));
        assert_eq!(read.downloads, 439_014);
        // No base-model tag on this one, and nothing is guessed from `Qwen-Image`.
        assert_eq!(read.family, Base::Unknown);
        // The listing weighs nothing, so nothing is weighed. A zero here would read as an empty
        // file rather than as a question nobody asked yet.
        assert!(read.files.is_empty());
        assert!(read.triggers.is_empty(), "not declared is not none");
    }

    #[test]
    fn the_base_model_tag_is_read_in_both_shapes_it_comes_in() {
        let both = json!({
            "id": "larryvrh/MiniMax-H3-Turbo-Lora",
            "downloads": 606_557,
            "tags": ["lora", "base_model:Comfy-Org/MiniMax-H3",
                     "base_model:adapter:Comfy-Org/MiniMax-H3"]
        });
        let read = read_model(&both).unwrap();
        assert_eq!(read.said_base, "Comfy-Org/MiniMax-H3");
        assert_eq!(
            read.family,
            Base::Unknown,
            "nothing here draws with that yet"
        );
    }

    #[test]
    fn a_source_that_cannot_answer_by_hash_says_so_by_answering_nothing() {
        // Not a refusal: to somebody trying to name an anonymous file, *we do not index that*
        // and *we do not have it* are the same outcome, and neither is a fault.
        assert_eq!(HuggingFace::new().by_hash(&"a".repeat(64)), Ok(None));
    }

    #[test]
    #[ignore = "reaches Hugging Face"]
    fn what_hugging_face_really_answers() {
        let hf = HuggingFace::new();
        let listing = hf
            .search(&Query {
                words: "pixel art".to_owned(),
                kind: Some(Kind::Lora),
                ..Query::default()
            })
            .expect("it answered");
        for asset in listing.assets.iter().take(5) {
            println!(
                "  {:<52} {:<10} {:>9} downloads",
                asset.name,
                asset.family.plainly(),
                asset.downloads
            );
        }
        assert!(!listing.assets.is_empty());

        // And the weighing is a second question, asked about one row.
        let files = hf
            .files_of(&listing.assets[0].id)
            .expect("the tree answered");
        for file in files.iter().take(3) {
            println!(
                "     {:<60} {:>6} MB  sha256 {}",
                file.name,
                file.bytes / 1_000_000,
                file.sha256.as_deref().unwrap_or("-")
            );
        }
    }

    /// The medium a repository's own tags place it in, read from the suffix.
    ///
    /// Tags taken from the live API on 2026-08-30: a video repository carries `text-to-video`
    /// and `audio-text-to-video` beside the plain word `video`, and the plain word is not a
    /// claim about what it *makes* — a model that reads video carries it too.
    #[test]
    fn a_pipeline_tag_names_the_medium_and_a_bare_word_does_not() {
        use epoch_assets::asset::Makes;

        assert_eq!(
            makes_from(&["diffusers", "text-to-video"]),
            Some(Makes::Video)
        );
        // The front half is what it takes; the back half is what it makes.
        assert_eq!(makes_from(&["audio-text-to-video"]), Some(Makes::Video));
        assert_eq!(makes_from(&["image-to-video"]), Some(Makes::Video));
        assert_eq!(makes_from(&["text-to-audio"]), Some(Makes::Sound));
        assert_eq!(makes_from(&["text-to-speech"]), Some(Makes::Sound));
        assert_eq!(makes_from(&["image-to-3d"]), Some(Makes::Model));
        assert_eq!(makes_from(&["text-to-image"]), Some(Makes::Picture));

        // **`video` on its own is not a medium claim.** A model that *reads* video is tagged
        // with it, and placing that under VIDEO would put a feature extractor in a list of
        // things that make one.
        assert_eq!(makes_from(&["transformers", "video", "safetensors"]), None);
        assert_eq!(makes_from(&[]), None);
    }
}
