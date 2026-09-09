//! Civitai, as a [`Catalogue`] (Phase 11.4).
//!
//! ## Measured before it was built, 2026-08-23
//!
//! The roadmap's own rule, and the Ollama cloud listing is what made it a rule: it was written,
//! measured, found to be lying, and deleted. So every claim below was asked of the live API.
//!
//! ```text
//! GET /api/v1/models/2851202                    → 200, no key
//! GET /api/v1/models?types=LORA&limit=3         → 200, no key
//! GET /api/v1/models?tag=watercolor&…           → 200, no key
//! GET /api/v1/models?query=watercolor&…         → 503 six times, then 200
//! GET /api/v1/model-versions/by-hash/<sha256>   → 200 with the full hash; 404 with a short one
//! GET /api/download/models/3219757?fileId=…     → 401 Unauthorized
//! ```
//!
//! Four of those shape the design.
//!
//! **Free-text search is the flaky endpoint.** `Model search is temporarily overloaded — please
//! retry`, six times over several minutes, then perfect. That is a [`Refusal::Busy`] and never an
//! empty result; `tag=` and a plain filtered listing answered throughout.
//!
//! **The hash lookup needs the whole SHA-256.** A truncated one 404s, which would have read as
//! *not ours*. This is the endpoint that makes hashing a 6.9 GB file worth the time: it turns a
//! file somebody already had into a named asset with its trigger words and its licence.
//!
//! **Downloading needs the user's own key.** Anonymous download is `401`. Epoch never asks for a
//! password and never types one — it says which key is missing and where it goes, and the user
//! pastes their own into the same secrets store every other credential lives in. Search does not
//! need one, so the shelf is fully usable before anybody signs in anywhere.
//!
//! **The base vocabulary is open and Epoch's is not.** The very model the owner asked about is
//! `Krea 2`, which no compiler here has a graph for. So both are carried: `said_base` verbatim,
//! because a person recognises it, and `family` as whatever Epoch can actually build with —
//! `Unknown` when the words map to nothing, which is honest rather than a blank.

use crate::catalogue::{Asset, Catalogue, File, Licence, Listing, Query, Refusal, Source, Version};
use epoch_assets::asset::{Base, Kind};

const API: &str = "https://civitai.com/api/v1";
const HOW_LONG: std::time::Duration = std::time::Duration::from_secs(20);

/// How many models one request asks for.
const PAGE: usize = 24;

/// The site, asked over HTTP.
#[derive(Debug, Clone, Default)]
pub struct Civitai {
    /// The user's own API key, when they have given Epoch one. Only downloads need it.
    key: Option<String>,
}

impl Civitai {
    pub fn new() -> Self {
        Self::default()
    }

    /// With the user's key, for the half that needs one.
    pub fn with_key(key: impl Into<String>) -> Self {
        Self {
            key: Some(key.into()),
        }
    }

    pub fn has_key(&self) -> bool {
        self.key.is_some()
    }

    fn get(&self, url: &str) -> Result<serde_json::Value, Refusal> {
        let mut request = ureq::get(url).timeout(HOW_LONG);
        if let Some(key) = &self.key {
            request = request.set("Authorization", &format!("Bearer {key}"));
        }
        match request.call() {
            Ok(answer) => answer.into_json().map_err(|why| {
                Refusal::Unreadable(format!(
                    "Civitai answered something this cannot read: {why}"
                ))
            }),
            Err(ureq::Error::Status(503, _)) => Err(Refusal::Busy(
                "Civitai's search is overloaded right now. It asks for a retry — this is not an \
                 empty result."
                    .to_owned(),
            )),
            Err(ureq::Error::Status(401, _)) | Err(ureq::Error::Status(403, _)) => {
                Err(Refusal::NeedsKey(
                    "Civitai refused without an account. Downloads need your own Civitai API key; \
                     searching does not."
                        .to_owned(),
                ))
            }
            Err(ureq::Error::Status(404, _)) => Err(Refusal::Unreadable(
                "Civitai has nothing at that address.".to_owned(),
            )),
            Err(ureq::Error::Status(code, _)) => {
                Err(Refusal::Unreachable(format!("Civitai answered {code}.")))
            }
            Err(why) => Err(Refusal::Unreachable(format!(
                "Civitai did not answer: {why}"
            ))),
        }
    }
}

impl Catalogue for Civitai {
    fn source(&self) -> Source {
        Source::Civitai
    }

    fn narrows_by_base(&self) -> bool {
        true
    }

    fn search(&self, query: &Query) -> Result<Listing, Refusal> {
        // A cursor arrives complete. Rebuilding the query around it is how a second page ends up
        // being a different search.
        let url = match &query.more {
            Some(cursor) => cursor.clone(),
            None => {
                let mut url = format!(
                    "{API}/models?limit={PAGE}&sort={}",
                    match query.order {
                        crate::catalogue::Order::MostDownloaded => "Most%20Downloaded",
                        crate::catalogue::Order::Newest => "Newest",
                    }
                );
                // Asked of the site rather than filtered afterwards: a page of twenty-four
                // filtered down to two is a page that looks empty for the wrong reason. The word
                // travels exactly as the site prints it — Epoch has no opinion about what
                // `Illustrious` means here.
                if !query.base.trim().is_empty() {
                    url.push_str("&baseModels=");
                    url.push_str(&urlencode(query.base.trim()));
                }
                if let Some(kind) = query.kind.and_then(civitai_type) {
                    url.push_str("&types=");
                    url.push_str(kind);
                }
                let words = query.words.trim();
                if !words.is_empty() {
                    url.push_str("&query=");
                    url.push_str(&urlencode(words));
                }
                if !query.adult {
                    url.push_str("&nsfw=false");
                }
                url
            }
        };

        let answer = self.get(&url)?;
        Ok(Listing {
            assets: answer
                .get("items")
                .and_then(|it| it.as_array())
                .map(|items| items.iter().filter_map(read_model).collect())
                .unwrap_or_default(),
            // Civitai hands out a whole next-page URL. Passed back exactly as it arrived: it
            // carries the query, the sort and the position, and anything that took it apart
            // would be guessing at somebody else's private format.
            more: answer
                .get("metadata")
                .and_then(|it| it.get("nextPage"))
                .and_then(|it| it.as_str())
                .map(str::to_owned),
        })
    }

    fn asset(&self, id: &str) -> Result<Option<Asset>, Refusal> {
        // `civitai:2851202#3219757` — a model, and which of its versions. Without the second
        // half the newest is taken, which is what a listing shows.
        let (id, wanted) = match id.split_once('#') {
            Some((id, version)) => (id, Some(version)),
            None => (id, None),
        };
        let number = id.strip_prefix("civitai:").unwrap_or(id);
        match self.get(&format!("{API}/models/{number}")) {
            Ok(model) => Ok(read_model_version(&model, wanted)),
            Err(Refusal::Unreadable(said)) if said.contains("nothing at that address") => Ok(None),
            Err(why) => Err(why),
        }
    }

    fn by_hash(&self, sha256: &str) -> Result<Option<Asset>, Refusal> {
        // Measured: a truncated hash 404s, which would read as *not ours*. Refusing here means a
        // caller can never accidentally ask a question that cannot be answered correctly.
        if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(Refusal::Unreadable(
                "Civitai's lookup takes a whole SHA-256; a shortened one answers 'not found' \
                 whatever the file is."
                    .to_owned(),
            ));
        }
        match self.get(&format!("{API}/model-versions/by-hash/{sha256}")) {
            Ok(version) => Ok(read_version_alone(&version)),
            // The one place a 404 is an answer rather than a fault: Civitai does not have it.
            Err(Refusal::Unreadable(said)) if said.contains("nothing at that address") => Ok(None),
            Err(why) => Err(why),
        }
    }
}

/// What Civitai calls a kind of asset.
///
/// `None` for the kinds it does not file separately, which asks for everything rather than
/// inventing a filter that would silently exclude.
const fn civitai_type(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Checkpoint => Some("Checkpoint"),
        Kind::Lora => Some("LORA"),
        Kind::Vae => Some("VAE"),
        Kind::ControlNet => Some("Controlnet"),
        Kind::Embedding => Some("TextualInversion"),
        Kind::Upscaler => Some("Upscaler"),
        // Civitai files a bare diffusion model under Checkpoint, and does not file text
        // encoders at all. Asking for what it does not have would return nothing and read as
        // *none exist*.
        Kind::DiffusionModel => Some("Checkpoint"),
        // Civitai has no type for a Piper voice, so there is no filter to send it — the same
        // situation as a text encoder, and it falls to the same `None`. The Voices shelf never
        // asks this source at all, which is what actually keeps checkpoints out of it.
        Kind::TextEncoder | Kind::Voice | Kind::Unknown => None,
    }
}

/// The base models worth offering as a filter, in Civitai's own words.
///
/// **Measured, not listed from memory.** Sampled 2026-08-24 across 400 of the most-downloaded
/// checkpoints and LoRAs: 38 distinct labels appeared. These are the ones that appeared often
/// enough to be worth a row; the long tail — one or two models each — is reachable by searching
/// for it, and a dropdown of thirty-eight is a dropdown nobody reads.
///
/// Ordered by how often they turned up, which is the order somebody scanning for theirs will
/// find it fastest.
pub const BASES: [&str; 16] = [
    "SD 1.5",
    "Illustrious",
    "Anima",
    "Krea 2",
    "Pony",
    "SDXL 1.0",
    "ZImageTurbo",
    "Flux.1 D",
    "NoobAI",
    "SDXL Lightning",
    "ZImageBase",
    "Qwen",
    "Flux.1 S",
    "Flux.2 D",
    "SD 2.1 768",
    "HiDream",
];

fn read_kind(said: &str) -> Kind {
    match said {
        "Checkpoint" => Kind::Checkpoint,
        "LORA" | "LoCon" | "DoRA" => Kind::Lora,
        "VAE" => Kind::Vae,
        "Controlnet" => Kind::ControlNet,
        "TextualInversion" => Kind::Embedding,
        "Upscaler" => Kind::Upscaler,
        _ => Kind::Unknown,
    }
}

/// Civitai's base vocabulary, mapped to what Epoch can build a graph for.
///
/// **Pony, Illustrious and NoobAI are SDXL**, and saying so is a measurement rather than a
/// courtesy: they are SDXL-architecture fine-tunes, so an adapter for one loads against the
/// other. What differs is taste, not tensor shapes — and tensor shapes are the only thing this
/// enum is allowed to decide.
///
/// Everything unrecognised is `Unknown`, which is shown as unmeasured and never as incompatible.
/// `Krea 2` — the model the owner asked about — is one of those, and that is the honest answer.
pub fn family_of(said: &str) -> Base {
    let said = said.to_ascii_lowercase();
    if said.contains("flux") {
        return Base::Flux;
    }
    if said.starts_with("sd 3") || said.starts_with("sd3") {
        return Base::Sd3;
    }
    if said.contains("xl")
        || said.contains("pony")
        || said.contains("illustrious")
        || said.contains("noobai")
    {
        return Base::Sdxl;
    }
    if said.starts_with("sd 2") || said.starts_with("sd2") {
        return Base::Sd2;
    }
    if said.starts_with("sd 1") || said.starts_with("sd1") {
        return Base::Sd15;
    }
    Base::Unknown
}

/// One model, with its newest published version — which is the one a shelf offers.
fn read_model(model: &serde_json::Value) -> Option<Asset> {
    read_model_version(model, None)
}

/// The picture a version leads with, at a size worth drawing.
///
/// ## Three readings, all from what Civitai answered
///
/// **A preview may be a video.** `images[]` carries `type`, and it is `image` or `video`; a
/// `.mp4` in an `<img>` draws nothing at all, so the first *image* is taken rather than the
/// first entry.
///
/// **A preview carries its own rating.** `nsfwLevel` is per image and not per model, so the
/// gentlest one is chosen rather than the first: an asset that is not itself marked adult can
/// still lead with a picture somebody did not ask to see. Where the asset *is* marked adult the
/// caller decides — that is the filter that already exists and already says so.
///
/// **And the size is a URL segment.** `original=true` is what the API answers with; measured
/// 2026-08-30, the same picture at `width=384` is 23 KB against 39 KB, and both follow a 301.
/// A row is 384 pixels at most and there is no reason to move the other 16 KB.
fn read_preview(version: &serde_json::Value) -> Option<String> {
    let images = version.get("images")?.as_array()?;
    let gentlest = images
        .iter()
        .filter(|one| {
            one.get("type")
                .and_then(|it| it.as_str())
                .is_none_or(|kind| kind == "image")
        })
        .min_by_key(|one| {
            one.get("nsfwLevel")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(u64::MAX)
        })?;
    let url = gentlest.get("url")?.as_str()?;
    // Only where the segment is the one Civitai puts there. A URL shaped differently is left
    // exactly as it came rather than rewritten on a guess.
    Some(url.replace("/original=true/", "/width=384/"))
}

/// One model, at the version somebody chose.
///
/// `None` takes the newest, which is what a listing shows. A named version that no longer exists
/// yields `None` rather than quietly falling back — downloading a different file from the one
/// somebody picked is the failure this whole parameter exists to prevent.
fn read_model_version(model: &serde_json::Value, wanted: Option<&str>) -> Option<Asset> {
    let id = model.get("id")?.as_u64()?;
    let all = model.get("modelVersions")?.as_array()?;
    let version = match wanted {
        Some(wanted) => all.iter().find(|version| {
            version
                .get("id")
                .and_then(serde_json::Value::as_u64)
                .map(|it| it.to_string())
                .as_deref()
                == Some(wanted)
        })?,
        None => all.first()?,
    };
    let said_base = version
        .get("baseModel")
        .and_then(|it| it.as_str())
        .unwrap_or("")
        .to_owned();

    Some(Asset {
        id: format!("civitai:{id}"),
        name: model.get("name")?.as_str()?.to_owned(),
        preview: read_preview(version),
        // **What the base model it was trained for makes.** Civitai has no notion of a medium,
        // and it does publish a base — so where that maps to a family Epoch measured, the medium
        // follows exactly. Everything else is unplaced and stays visible under every one.
        makes: family_of(&said_base).makes(),
        source: Source::Civitai,
        kind: read_kind(model.get("type").and_then(|it| it.as_str()).unwrap_or("")),
        family: family_of(&said_base),
        said_base,
        by: model
            .get("creator")
            .and_then(|it| it.get("username"))
            .and_then(|it| it.as_str())
            .map(str::to_owned),
        downloads: model
            .get("stats")
            .and_then(|it| it.get("downloadCount"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        adult: model
            .get("nsfw")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        triggers: read_triggers(version),
        licence: read_licence(model),
        files: read_files(version),
        versions: all
            .iter()
            .filter_map(|version| {
                let said_base = version
                    .get("baseModel")
                    .and_then(|it| it.as_str())
                    .unwrap_or("")
                    .to_owned();
                Some(Version {
                    id: format!(
                        "civitai:{id}#{}",
                        version.get("id").and_then(serde_json::Value::as_u64)?
                    ),
                    name: version
                        .get("name")
                        .and_then(|it| it.as_str())
                        .unwrap_or("a version")
                        .to_owned(),
                    family: family_of(&said_base),
                    said_base,
                    bytes: read_files(version)
                        .iter()
                        .map(|file| file.bytes)
                        .max()
                        .unwrap_or(0),
                })
            })
            .collect(),
        sample: None,
        page: format!("https://civitai.com/models/{id}"),
    })
}

/// A version fetched on its own, by hash — it carries its model's name inside it.
fn read_version_alone(version: &serde_json::Value) -> Option<Asset> {
    let model_id = version.get("modelId")?.as_u64()?;
    let model = version.get("model");
    let said_base = version
        .get("baseModel")
        .and_then(|it| it.as_str())
        .unwrap_or("")
        .to_owned();

    Some(Asset {
        id: format!("civitai:{model_id}"),
        name: model
            .and_then(|it| it.get("name"))
            .and_then(|it| it.as_str())
            .unwrap_or("something on Civitai")
            .to_owned(),
        preview: read_preview(version),
        // **What the base model it was trained for makes.** Civitai has no notion of a medium,
        // and it does publish a base — so where that maps to a family Epoch measured, the medium
        // follows exactly. Everything else is unplaced and stays visible under every one.
        makes: family_of(&said_base).makes(),
        source: Source::Civitai,
        kind: read_kind(
            model
                .and_then(|it| it.get("type"))
                .and_then(|it| it.as_str())
                .unwrap_or(""),
        ),
        family: family_of(&said_base),
        said_base,
        by: None,
        downloads: version
            .get("stats")
            .and_then(|it| it.get("downloadCount"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        adult: model
            .and_then(|it| it.get("nsfw"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        triggers: read_triggers(version),
        licence: Licence::default(),
        files: read_files(version),
        // Reached by hash, which names one version already. Offering the others here would be a
        // choice about a file somebody already has.
        versions: Vec::new(),
        sample: None,
        page: format!("https://civitai.com/models/{model_id}"),
    })
}

fn read_triggers(version: &serde_json::Value) -> Vec<String> {
    version
        .get("trainedWords")
        .and_then(|it| it.as_array())
        .map(|words| {
            words
                .iter()
                .filter_map(|word| word.as_str())
                // Measured on the owner's own link: `"a digital low poly pixel art,"` — the
                // trailing comma is in the data, and pasting it into a prompt is somebody else's
                // punctuation leaking into their sentence.
                .map(|word| word.trim().trim_end_matches(',').trim().to_owned())
                .filter(|word| !word.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn read_licence(model: &serde_json::Value) -> Licence {
    let flag = |key: &str| {
        model
            .get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    Licence {
        no_credit_needed: flag("allowNoCredit"),
        commercial: model
            .get("allowCommercialUse")
            .and_then(|it| it.as_array())
            .map(|ways| {
                ways.iter()
                    .filter_map(|way| way.as_str())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        derivatives: flag("allowDerivatives"),
        relicensing: flag("allowDifferentLicense"),
    }
}

fn read_files(version: &serde_json::Value) -> Vec<File> {
    version
        .get("files")
        .and_then(|it| it.as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|file| {
                    Some(File {
                        name: file.get("name")?.as_str()?.to_owned(),
                        // `sizeKB` is a float in the answer — `223231.3671875`, measured.
                        bytes: (file.get("sizeKB")?.as_f64()? * 1024.0) as u64,
                        sha256: file
                            .get("hashes")
                            .and_then(|it| it.get("SHA256"))
                            .and_then(|it| it.as_str())
                            .map(|hash| hash.to_ascii_lowercase()),
                        url: file.get("downloadUrl")?.as_str()?.to_owned(),
                        // Measured: anonymous download is 401, for every file.
                        needs_key: true,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Percent-encode a query.
///
/// Its own three lines rather than a dependency: the alphabet that survives is small and the rule
/// is one line, and a crate for it is a crate to keep updated.
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

    /// The answer the live API really gave for the model the owner asked about, trimmed to the
    /// fields this reads. Kept verbatim so a change in their shape breaks a test rather than a
    /// screen.
    fn the_owners_lora() -> serde_json::Value {
        json!({
            "id": 2851202,
            "name": "Low Poly Pixel Art - Krea2 style",
            "type": "LORA",
            "nsfw": false,
            "allowNoCredit": true,
            "allowCommercialUse": ["Image", "RentCivit"],
            "allowDerivatives": false,
            "allowDifferentLicense": true,
            "stats": {"downloadCount": 256},
            "creator": {"username": "Fatbuns"},
            "modelVersions": [{
                "id": 3219757,
                "name": "v1.0",
                "baseModel": "Krea 2",
                "trainedWords": ["a digital low poly pixel art,"],
                "files": [{
                    "name": "Pixelart_LowPoly.safetensors",
                    "sizeKB": 223231.3671875,
                    "downloadUrl": "https://civitai.com/api/download/models/3219757?fileId=3101651",
                    "hashes": {"SHA256": "E41DB4F45D70DAF244A7B557BEE0CA102B5499B2CDFC479F9CAE72469D09116B"}
                }]
            }]
        })
    }

    #[test]
    fn the_model_the_owner_asked_about_reads_completely() {
        let read = read_model(&the_owners_lora()).unwrap();
        assert_eq!(read.id, "civitai:2851202");
        assert_eq!(read.kind, Kind::Lora);
        assert_eq!(read.by.as_deref(), Some("Fatbuns"));
        assert_eq!(read.downloads, 256);
        assert_eq!(read.page, "https://civitai.com/models/2851202");

        // The point of carrying both: Epoch cannot build for `Krea 2`, and a person recognises
        // the words. Showing a blank would have thrown away the half that is useful.
        assert_eq!(read.said_base, "Krea 2");
        assert_eq!(read.family, Base::Unknown);

        // Somebody else's punctuation does not leak into the user's prompt.
        assert_eq!(read.triggers, vec!["a digital low poly pixel art"]);

        let file = read.principal().unwrap();
        assert_eq!(file.bytes, 228_588_920);
        assert!(file.needs_key, "measured: anonymous download is 401");
        assert_eq!(
            file.sha256.as_deref(),
            Some("e41db4f45d70daf244a7b557bee0ca102b5499b2cdfc479f9cae72469d09116b"),
            "lowercase, because that is what a fingerprint of the file on disk will be"
        );

        assert_eq!(read.licence.commercial, vec!["Image", "RentCivit"]);
        assert!(!read.licence.derivatives);
    }

    #[test]
    fn every_version_is_offered_and_each_one_names_its_own_base() {
        // The defect this fixes, measured 2026-08-24: the owner's LoRA is published twice —
        // 170 MB for Z-Image and 18 MB for Flux — and Epoch fetched the largest, which is the
        // wrong file for a Flux graph.
        let mut model = the_owners_lora();
        model["modelVersions"] = serde_json::json!([
            {
                "id": 2744046,
                "name": "ZImageTurbo",
                "baseModel": "ZImageTurbo",
                "files": [{"name": "z.safetensors", "sizeKB": 166000.0, "downloadUrl": "u"}]
            },
            {
                "id": 2142473,
                "name": "Flux",
                "baseModel": "Flux.1 D",
                "files": [{"name": "f.safetensors", "sizeKB": 18000.0, "downloadUrl": "u"}]
            }
        ]);

        let read = read_model(&model).unwrap();
        assert_eq!(read.versions.len(), 2);
        assert_eq!(read.versions[1].family, Base::Flux);
        assert_eq!(read.versions[1].id, "civitai:2851202#2142473");
        // Unqualified takes the newest, which is what a listing shows.
        assert_eq!(read.said_base, "ZImageTurbo");

        // And asking for one takes that one.
        let flux = read_model_version(&model, Some("2142473")).unwrap();
        assert_eq!(flux.family, Base::Flux);
        assert_eq!(flux.principal().unwrap().name, "f.safetensors");
    }

    #[test]
    fn a_version_that_no_longer_exists_is_not_quietly_replaced() {
        // Downloading a different file from the one somebody picked is the failure the whole
        // parameter exists to prevent.
        assert!(read_model_version(&the_owners_lora(), Some("999")).is_none());
    }

    #[test]
    fn a_fine_tune_of_sdxl_is_sdxl_because_that_is_what_decides_whether_it_loads() {
        for said in ["Pony", "Illustrious", "NoobAI", "SDXL 1.0", "SDXL Turbo"] {
            assert_eq!(family_of(said), Base::Sdxl, "{said}");
        }
        assert_eq!(family_of("SD 1.5"), Base::Sd15);
        assert_eq!(family_of("SD 2.1"), Base::Sd2);
        assert_eq!(family_of("SD 3.5"), Base::Sd3);
        assert_eq!(family_of("Flux.1 D"), Base::Flux);
        // The one that matters most: a name nobody here has a graph for stays unknown rather
        // than being filed as the nearest thing.
        assert_eq!(family_of("Krea 2"), Base::Unknown);
        assert_eq!(family_of("Hunyuan Video"), Base::Unknown);
    }

    #[test]
    fn a_short_hash_is_refused_rather_than_asked() {
        // Measured: Civitai answers 404 to a truncated hash, which would read as *not ours* —
        // so the question is never sent.
        let why = Civitai::new().by_hash("E41DB4F45D70DAF244A7").unwrap_err();
        assert!(matches!(why, Refusal::Unreadable(_)), "{why:?}");
    }

    #[test]
    fn a_query_reaches_the_api_and_cannot_reach_out_of_it() {
        assert_eq!(urlencode("pixel art"), "pixel%20art");
        assert_eq!(urlencode("a&b=c"), "a%26b%3Dc");
        assert_eq!(urlencode("水彩"), "%E6%B0%B4%E5%BD%A9");
    }

    /// Against the live site. Ignored, like every other network measurement here.
    #[test]
    #[ignore = "reaches Civitai"]
    fn what_civitai_really_answers() {
        let civitai = Civitai::new();

        let by_hash = civitai
            .by_hash("e41db4f45d70daf244a7b557bee0ca102b5499b2cdfc479f9cae72469d09116b")
            .expect("the lookup answered");
        println!("by hash: {:#?}", by_hash.as_ref().map(|it| &it.name));
        assert!(by_hash.is_some(), "the owner's LoRA is on Civitai");

        // A hash of nothing. Not an error — Civitai simply does not have it, and the caller must
        // be able to tell that from a refusal.
        let unknown = civitai.by_hash(&"0".repeat(64));
        println!("unknown hash: {unknown:?}");
        assert!(matches!(unknown, Ok(None)), "{unknown:?}");

        match civitai.search(&Query {
            words: "watercolor".to_owned(),
            kind: Some(Kind::Lora),
            ..Query::default()
        }) {
            Ok(listing) => {
                for asset in listing.assets.iter().take(5) {
                    println!(
                        "  {:<44} {:<12} {:<8} {:>9} downloads",
                        asset.name,
                        asset.said_base,
                        asset.family.plainly(),
                        asset.downloads
                    );
                }
                assert!(!listing.assets.is_empty());
            }
            // The measured reality: this endpoint is genuinely flaky, and a run that hits it
            // must not fail the build — it must report which of the four things happened.
            Err(why) => println!("search refused: {why:?}"),
        }
    }

    /// The preview a version leads with, and the three things read off Civitai's own answer.
    ///
    /// Shapes taken from a live response on 2026-08-30: `images[]` carries `url`, `type` and a
    /// per-image `nsfwLevel`, and the URL's size is a path segment.
    #[test]
    fn a_preview_is_the_gentlest_still_picture_at_a_size_worth_drawing() {
        let version = serde_json::json!({
            "images": [
                // First, and a video: an `<img>` draws nothing at all from an mp4.
                { "type": "video", "nsfwLevel": 1, "url": "https://image.civitai.com/a/b/original=true/1.mp4" },
                // Second, and the harshest.
                { "type": "image", "nsfwLevel": 8, "url": "https://image.civitai.com/a/b/original=true/2.jpeg" },
                // Third, and the one to show.
                { "type": "image", "nsfwLevel": 1, "url": "https://image.civitai.com/a/b/original=true/3.jpeg" },
            ]
        });
        assert_eq!(
            read_preview(&version).as_deref(),
            Some("https://image.civitai.com/a/b/width=384/3.jpeg"),
            "a still, the gentlest one, and 384 wide"
        );

        // Nothing to show is `None`, and never a guessed address.
        assert_eq!(read_preview(&serde_json::json!({})), None);
        assert_eq!(read_preview(&serde_json::json!({ "images": [] })), None);
        // A URL shaped differently is left exactly as it came rather than rewritten on a guess.
        let odd = serde_json::json!({
            "images": [{ "type": "image", "nsfwLevel": 1, "url": "https://example.test/a.png" }]
        });
        assert_eq!(
            read_preview(&odd).as_deref(),
            Some("https://example.test/a.png")
        );
    }
}
