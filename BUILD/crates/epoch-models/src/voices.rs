//! Where voices come from — a shelf, not a sibling (ADR-0031, Phase 15).
//!
//! ## Why this is a `Catalogue` and not something new
//!
//! Everything the Workshop already does is here: ask a source what exists, hand one thing to the
//! same `fetch`, file it on a shelf by what it is. A second install path for voices would be the
//! third searcher ADR-0031 was written to prevent, arriving in a medium instead of on a site.
//!
//! ## A repository qualifies from its files, never from a tag
//!
//! Measured 2026-09-03: **Hugging Face has no tag for *a Piper voice*.** `rhasspy/piper-voices`
//! declares no library; its only tags are `onnx` and `license:mit`, which a hundred unrelated
//! repositories also carry. So there is nothing to filter on and this must not pretend otherwise.
//!
//! The rule is the one that already decides whether a workflow can do img2img — *derived, never
//! declared*:
//!
//! > A file is a voice when an `.onnx` has an `.onnx.json` **beside it**.
//!
//! It is applied to the repository's own tree, so it works identically for a community repository
//! nobody has catalogued. **A capability typed into a manifest is one that will eventually lie**,
//! and here there is no manifest to lie in.
//!
//! ## One repository today, because one has been measured
//!
//! `rhasspy/piper-voices`, MIT, **175 voices across 56 locales** — counted by the rule above
//! rather than taken from its README, and the count came out at exactly what the README claims,
//! which is the only reason that is worth mentioning.
//!
//! The community repositories the roadmap names (`HirCoir/*`, `AIHeaven/piper_unofficial_voices`)
//! are not here yet for the reason `PREPARATIONS` gives: **a named set grows the day somebody
//! measures one, not the day somebody remembers one.** Adding the next one is a line in
//! [`REPOSITORIES`], because the expansion rule is the file rule and not a per-site parser.
//!
//! ## What a listing may say, and what it may not
//!
//! The tree gives a path and a size and nothing else. So:
//!
//! - **Identity** is the path. That is the source's own identifier, exactly as Civitai's numeric
//!   id is — not a claim about content, and nothing downstream keys on the words in it.
//! - **What it sounds like** — language, quality, how many speakers — lives in the sidecar, which
//!   is read when somebody points at one voice and again after it lands. That is the split the
//!   `Catalogue` trait already draws: *the list is cheap and the weighing happens when somebody
//!   points at one thing.*
//! - **Downloads is not reported.** Hugging Face publishes a count per repository and never per
//!   file. Printing the repository's number on each of its 175 rows would be a real reading of
//!   the wrong quantity — the most convincing way a gauge can lie.

use std::collections::BTreeMap;
use std::sync::Mutex;

use epoch_assets::asset::{Base, Kind};

use crate::catalogue::{Asset, Catalogue, File, Licence, Listing, Query, Refusal, Source};

const API: &str = "https://huggingface.co/api";
const HOW_LONG: std::time::Duration = std::time::Duration::from_secs(20);

/// How many voices one page of the shelf carries.
const PAGE: usize = 24;

/// Every repository measured to hold Piper voices, with the licence its page declares.
///
/// The licence travels with the asset rather than with the search (ADR-0016): `rhasspy` is MIT,
/// some of HirCoir's are Apache-2.0, and AIHeaven declares none at all. A shelf that printed one
/// licence for everything would be wrong the moment the second entry arrives.
const REPOSITORIES: &[Repository] = &[Repository {
    id: "rhasspy/piper-voices",
    by: "rhasspy",
    licence: "MIT",
}];

struct Repository {
    id: &'static str,
    by: &'static str,
    licence: &'static str,
}

/// The voices shelf.
pub struct Voices;

impl Voices {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Voices {
    fn default() -> Self {
        Self::new()
    }
}

/// One voice, as the pairing rule found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// `rhasspy/piper-voices`.
    pub repo: String,
    /// `es/es_ES/davefx/medium/es_ES-davefx-medium.onnx`, inside that repository.
    pub onnx: String,
    /// Its sidecar. Present by construction — that is what made it a voice.
    pub sidecar: String,
    pub bytes: u64,
    pub sidecar_bytes: u64,
    /// git-lfs addresses by content, so this *is* the SHA-256, when the tree stated one.
    pub sha256: Option<String>,
    /// A recording of this voice, when the repository publishes one.
    ///
    /// **Derived from the tree, never assumed.** `rhasspy/piper-voices` puts a
    /// `samples/speaker_0.mp3` beside each voice — 191 KB, measured — and a community
    /// repository may not. A play button that appears whether or not there is anything behind
    /// it is a control that does nothing, which is the failure this codebase keeps deleting.
    pub sample: Option<String>,
    /// What the repository says this voice is, when it publishes an index.
    ///
    /// **A description, never the qualification.** Whether this is a voice at all was decided by
    /// the pairing rule against the tree; an index only says what it *is*. So an entry the index
    /// names but the tree does not hold is ignored, and a paired file the index never mentions is
    /// still on the shelf, undescribed. The files outrank the index, which is the same order the
    /// bytes and a catalogue already stand in.
    pub spoken: Option<Spoken>,
}

impl Found {
    /// What to call it: the last path segment, without the extension.
    ///
    /// The source's own identifier, not a reading of the file. What the voice actually **is** —
    /// language, quality, speakers — comes from the sidecar, measured.
    pub fn name(&self) -> &str {
        let last = self.onnx.rsplit('/').next().unwrap_or(&self.onnx);
        last.strip_suffix(".onnx").unwrap_or(last)
    }

    /// `piper:rhasspy/piper-voices:es/es_ES/davefx/medium/es_ES-davefx-medium.onnx`
    pub fn id(&self) -> String {
        format!("piper:{}:{}", self.repo, self.onnx)
    }

    fn asset(&self, repo: &Repository) -> Asset {
        let url = |path: &str| format!("https://huggingface.co/{}/resolve/main/{path}", self.repo);
        Asset {
            id: self.id(),
            name: self.name().to_owned(),
            source: Source::HuggingFace,
            kind: Kind::Voice,
            // A voice belongs to no diffusion family and never will. `Unknown` is the honest
            // value and it is already what every surface reads as *unmeasured, not incompatible*.
            family: Base::Unknown,
            // **What this voice is, in the row's own words** — `Español (Spain) · medium ·
            // 22050 Hz`. `said_base` is *what the source called it*, and for a picture model
            // that is a base model; for a voice it is the language. Empty where no index
            // described it, which reads as unmeasured rather than as a blank field.
            said_base: self
                .spoken
                .as_ref()
                .map(|it| it.plainly())
                .unwrap_or_default(),
            by: Some(repo.by.to_owned()),
            // Not reported. See this module's header: Hugging Face counts repositories, and
            // this shelf must not print one repository's number on 175 rows.
            downloads: 0,
            adult: false,
            triggers: Vec::new(),
            licence: Licence {
                no_credit_needed: repo.licence == "MIT",
                commercial: vec![repo.licence.to_owned()],
                derivatives: true,
                relicensing: false,
            },
            // **Both files, in the order they must land.** A voice is the one asset here whose
            // second file is not optional: Piper reads the sidecar for its phoneme table, and an
            // `.onnx` on its own is a download that cannot speak.
            files: vec![
                File {
                    name: self.onnx.clone(),
                    bytes: self.bytes,
                    sha256: self.sha256.clone(),
                    url: url(&self.onnx),
                    needs_key: false,
                },
                File {
                    name: self.sidecar.clone(),
                    bytes: self.sidecar_bytes,
                    sha256: None,
                    url: url(&self.sidecar),
                    needs_key: false,
                },
            ],
            versions: Vec::new(),
            // A voice makes sound, and `Makes` is the picture chain's word for which medium a
            // *generative model* draws in. Leaving it unplaced is right: nothing filters this
            // shelf by medium, and claiming one would put voices in the Studio Panel's tabs.
            makes: None,
            // Hugging Face publishes samples for these as `.mp3` beside the model, and playing
            // one is step 2 of the phase rather than this step. `preview` is a *picture*, and a
            // voice has none.
            preview: None,
            sample: self.sample.as_ref().map(|it| url(it)),
            page: format!("https://huggingface.co/{}/tree/main/{}", self.repo, {
                let path = &self.onnx;
                path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
            }),
        }
    }
}

/// Every voice in every measured repository.
///
/// Cached for the life of the process. Four requests and 2.4 s measured against
/// `rhasspy/piper-voices`, which is cheap once and rude on every keystroke.
fn all() -> Result<Vec<(usize, Found)>, Refusal> {
    static KNOWN: Mutex<Option<Vec<(usize, Found)>>> = Mutex::new(None);
    if let Ok(seen) = KNOWN.lock() {
        if let Some(found) = seen.as_ref() {
            return Ok(found.clone());
        }
    }

    let mut found = Vec::new();
    for (which, repo) in REPOSITORIES.iter().enumerate() {
        for voice in voices_in(repo.id)? {
            found.push((which, voice));
        }
    }
    if let Ok(mut seen) = KNOWN.lock() {
        *seen = Some(found.clone());
    }
    Ok(found)
}

/// What a repository's own index says about each of its voices, keyed by the `.onnx` path.
///
/// ## Why an index at all, when the sidecars are right there
///
/// Measured 2026-09-04: reading all 176 sidecars takes **14.2 s with eight threads**, and the
/// shelf paints in 1.5 s. `voices.json` carries the same fields for every voice in **one request,
/// 245 KB, 0.68 s** — so the description is free and arrives before the first row is drawn.
///
/// **Best effort, by construction.** A repository that publishes no index simply has none of
/// this, and its voices appear on the shelf with their names and their sizes. Nothing here can
/// make a voice fail to be found.
fn index_of(repo: &str) -> BTreeMap<String, Spoken> {
    let Ok(answer) = ureq::get(&format!(
        "https://huggingface.co/{repo}/resolve/main/voices.json"
    ))
    .timeout(HOW_LONG)
    .call() else {
        return BTreeMap::new();
    };
    let Ok(json) = answer.into_json::<serde_json::Value>() else {
        return BTreeMap::new();
    };
    let Some(entries) = json.as_object() else {
        return BTreeMap::new();
    };

    let mut said = BTreeMap::new();
    for entry in entries.values() {
        let Some(spoken) = Spoken::of(entry) else {
            continue;
        };
        // Keyed by the path of the `.onnx` the entry lists, so the join is against the same
        // string the pairing rule found — never against a name either side had to parse.
        let files = entry.get("files").and_then(|it| it.as_object());
        for path in files.into_iter().flat_map(|it| it.keys()) {
            if path.ends_with(".onnx") {
                said.insert(path.clone(), spoken.clone());
            }
        }
    }
    said
}

/// Ask one repository what it holds, and apply the pairing rule to the answer.
///
/// The tree is paginated at 1000 entries and the cursor arrives in a `Link` header, so this
/// follows it rather than reading the first page and calling it the repository. Measured: four
/// pages, 3903 entries, 175 voices — a first-page-only reader would have found 28.
pub fn voices_in(repo: &str) -> Result<Vec<Found>, Refusal> {
    let mut url = format!("{API}/models/{repo}/tree/main?recursive=true");
    let mut entries: Vec<serde_json::Value> = Vec::new();
    // A bound, so a repository that answers with a cursor loop cannot hang the shelf.
    for _ in 0..24 {
        let answer = ureq::get(&url).timeout(HOW_LONG).call().map_err(describe)?;
        let next = answer.header("Link").and_then(next_page).map(str::to_owned);
        let page: serde_json::Value = answer
            .into_json()
            .map_err(|why| Refusal::Unreadable(format!("Hugging Face answered oddly: {why}")))?;
        match page {
            serde_json::Value::Array(rows) => entries.extend(rows),
            _ => {
                return Err(Refusal::Unreadable(
                    "Hugging Face answered with something that was not a list of files.".into(),
                ))
            }
        }
        match next {
            Some(link) => url = link,
            None => break,
        }
    }

    // Everything the repository holds, by path, so the pairing is a lookup rather than a scan
    // per candidate.
    let mut sizes: BTreeMap<&str, u64> = BTreeMap::new();
    for entry in &entries {
        if entry.get("type").and_then(|it| it.as_str()) != Some("file") {
            continue;
        }
        if let Some(path) = entry.get("path").and_then(|it| it.as_str()) {
            sizes.insert(
                path,
                entry.get("size").and_then(|it| it.as_u64()).unwrap_or(0),
            );
        }
    }

    let index = index_of(repo);
    let mut found = Vec::new();
    for entry in &entries {
        let Some(path) = entry.get("path").and_then(|it| it.as_str()) else {
            continue;
        };
        if !path.ends_with(".onnx") {
            continue;
        }
        let sidecar = format!("{path}.json");
        // The whole rule. No tag, no filename pattern, no README.
        let Some(&sidecar_bytes) = sizes.get(sidecar.as_str()) else {
            continue;
        };
        found.push(Found {
            repo: repo.to_owned(),
            onnx: path.to_owned(),
            sidecar,
            bytes: sizes.get(path).copied().unwrap_or(0),
            sidecar_bytes,
            sha256: entry
                .get("lfs")
                .and_then(|it| it.get("oid"))
                .and_then(|it| it.as_str())
                .map(|oid| oid.to_ascii_lowercase()),
            spoken: index.get(path).cloned(),
            // The recording that sits beside this voice, if the tree holds one. One speaker
            // today; a multi-speaker voice would publish several and this takes the first,
            // which is what *listen to this voice* means when nobody has picked a person yet.
            sample: {
                let folder = path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
                let wanted = format!("{folder}/samples/speaker_0.mp3");
                sizes.contains_key(wanted.as_str()).then_some(wanted)
            },
        });
    }
    Ok(found)
}

/// The `rel="next"` URL out of a `Link` header, when there is one.
fn next_page(link: &str) -> Option<&str> {
    link.split(',').find_map(|part| {
        part.contains("rel=\"next\"")
            .then(|| part.split_once('<')?.1.split_once('>').map(|(url, _)| url))
            .flatten()
    })
}

fn describe(why: ureq::Error) -> Refusal {
    match why {
        ureq::Error::Status(429 | 503, _) => {
            Refusal::Busy("Hugging Face is busy and asked to be tried again in a moment.".into())
        }
        ureq::Error::Status(401 | 403, _) => Refusal::NeedsKey(
            "That repository hands its files over only to an account. Settings → Catalogue keys."
                .into(),
        ),
        ureq::Error::Status(code, _) => {
            Refusal::Unreadable(format!("Hugging Face answered {code}."))
        }
        ureq::Error::Transport(what) => {
            Refusal::Unreachable(format!("Hugging Face could not be reached: {what}"))
        }
    }
}

impl Catalogue for Voices {
    fn source(&self) -> Source {
        Source::HuggingFace
    }

    /// **The words filter locally, and that is the honest arrangement rather than a shortcut.**
    ///
    /// Hugging Face can search *repositories*; there are 175 voices inside one repository and it
    /// has no way to be asked about them. So the whole set is fetched once and narrowed here —
    /// which also means the shelf answers instantly after the first open, and paging is exact
    /// instead of a cursor into somebody else's sort.
    fn search(&self, query: &Query) -> Result<Listing, Refusal> {
        if matches!(query.kind, Some(kind) if kind != Kind::Voice) {
            return Ok(Listing {
                assets: Vec::new(),
                more: None,
            });
        }

        let wanted = query.words.trim().to_lowercase();
        let mut matching: Vec<Asset> = all()?
            .into_iter()
            .filter(|(_, voice)| {
                wanted.is_empty()
                    || voice.onnx.to_lowercase().contains(&wanted)
                    || voice.name().to_lowercase().contains(&wanted)
                    // **The words a person uses.** The path is `es/es_ES/davefx/medium`, so a
                    // search for `spanish` matched nothing on a shelf holding nine Spanish
                    // voices — a silence read as *there are none*. The language name, the native
                    // name and the country were in the file the whole time.
                    || voice
                        .spoken
                        .as_ref()
                        .is_some_and(|it| it.words().contains(&wanted))
            })
            .map(|(which, voice)| voice.asset(&REPOSITORIES[which]))
            .collect();
        matching.sort_by(|a, b| a.id.cmp(&b.id));

        let from: usize = query
            .more
            .as_deref()
            .and_then(|more| more.parse().ok())
            .unwrap_or(0);
        let page: Vec<Asset> = matching.iter().skip(from).take(PAGE).cloned().collect();
        let shown = from + page.len();
        Ok(Listing {
            assets: page,
            // Exact, because the whole list is in hand: NEXT appears when there really is more,
            // never merely because this page came back full.
            more: (shown < matching.len()).then(|| shown.to_string()),
        })
    }

    fn asset(&self, id: &str) -> Result<Option<Asset>, Refusal> {
        Ok(all()?
            .into_iter()
            .find(|(_, voice)| voice.id() == id)
            .map(|(which, voice)| voice.asset(&REPOSITORIES[which])))
    }

    fn files_of(&self, id: &str) -> Result<Vec<File>, Refusal> {
        Ok(self.asset(id)?.map(|it| it.files).unwrap_or_default())
    }

    /// A voice's `.onnx` is git-lfs, so its hash *is* addressable — but only within the
    /// repositories this knows, and answering `None` for everything else is the same honest
    /// shape `HuggingFace::by_hash` already has.
    fn by_hash(&self, sha256: &str) -> Result<Option<Asset>, Refusal> {
        let wanted = sha256.to_ascii_lowercase();
        Ok(all()?
            .into_iter()
            .find(|(_, voice)| voice.sha256.as_deref() == Some(wanted.as_str()))
            .map(|(which, voice)| voice.asset(&REPOSITORIES[which])))
    }
}

/// What a voice on this machine actually is, read from the sidecar beside it.
///
/// **This is the measurement, and the filename is not.** `es_ES-davefx-medium.onnx` is a claim;
/// `{"language":{"code":"es_ES","name_native":"Español"},"audio":{"quality":"medium"}}` is the
/// file saying it (ADR-0024). Every field is optional because a community sidecar may carry
/// fewer of them, and a missing one is *unasked* rather than a default.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Spoken {
    /// `es_ES`.
    pub code: Option<String>,
    /// `Español`, in its own language — which is what somebody scanning a list of 56 locales
    /// actually recognises.
    pub native: Option<String>,
    /// `Spanish`.
    pub english: Option<String>,
    /// `Spain`, `Mexico`, `Argentina` — the half of the answer a code does not give. Three
    /// voices all say `Spanish`, and which country is the thing a person is choosing between.
    pub country: Option<String>,
    /// `x_low` · `low` · `medium` · `high`.
    pub quality: Option<String>,
    pub sample_rate: Option<u64>,
    /// How many people this one file can be. Most are 1; a few are dozens.
    pub speakers: Option<u64>,
    /// The version of Piper that trained it — and **the field that makes a sidecar a Piper
    /// sidecar**, which is what qualifies a repository nobody has catalogued.
    pub piper_version: Option<String>,
}

impl Spoken {
    /// Read the sidecar that belongs to an `.onnx` on disk.
    ///
    /// `None` when there is none or it will not parse: a voice Epoch cannot describe still
    /// speaks, so this may never be the thing that makes one unusable.
    pub fn beside(onnx: &std::path::Path) -> Option<Self> {
        let mut sidecar = onnx.as_os_str().to_owned();
        sidecar.push(".json");
        let text = std::fs::read_to_string(std::path::PathBuf::from(sidecar)).ok()?;
        Self::read(&text)
    }

    pub fn read(sidecar: &str) -> Option<Self> {
        Self::of(&serde_json::from_str(sidecar).ok()?)
    }

    /// Read from either shape the same publisher writes.
    ///
    /// **One reader, deliberately.** A sidecar puts the quality under `audio`; the repository's
    /// own index puts it at the top level. They are the same fields written twice by the same
    /// people, and two readers would be two answers that drift the first time one of them learns
    /// something.
    pub fn of(json: &serde_json::Value) -> Option<Self> {
        let word = |at: &serde_json::Value, key: &str| {
            at.get(key)
                .and_then(|it| it.as_str())
                .map(|it| it.trim().to_owned())
                .filter(|it| !it.is_empty())
        };
        let language = json.get("language").cloned().unwrap_or_default();
        let audio = json.get("audio").cloned().unwrap_or_default();
        Some(Self {
            code: word(&language, "code"),
            native: word(&language, "name_native"),
            english: word(&language, "name_english"),
            country: word(&language, "country_english"),
            quality: word(&audio, "quality").or_else(|| word(json, "quality")),
            sample_rate: audio.get("sample_rate").and_then(|it| it.as_u64()),
            speakers: json.get("num_speakers").and_then(|it| it.as_u64()),
            piper_version: word(json, "piper_version"),
        })
    }

    /// Every word somebody might type looking for this voice.
    ///
    /// **The reason this exists**: the shelf matched on the path, and the path is
    /// `es/es_ES/davefx/medium/…`. Somebody who typed `spanish` — which is the word a person
    /// uses — was told *Nothing here matched that*, about a shelf holding nine Spanish voices.
    /// The English name, the native name and the country are all in the file; they were simply
    /// not being searched.
    pub fn words(&self) -> String {
        [
            self.code.as_deref(),
            self.native.as_deref(),
            self.english.as_deref(),
            self.country.as_deref(),
            self.quality.as_deref(),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
    }

    /// One line for a person: *Español (España) · medium · 22050 Hz*.
    ///
    /// Built from what was read, so a sidecar carrying less says less rather than showing gaps.
    pub fn plainly(&self) -> String {
        let mut said: Vec<String> = Vec::new();
        if let Some(name) = self.native.as_ref().or(self.english.as_ref()) {
            // The country where the file says one, and the locale code where it does not. Three
            // voices reading `Spanish` are told apart by *Spain* and *Mexico*, not by `es_ES`.
            said.push(match self.country.as_deref().or(self.code.as_deref()) {
                Some(which) => format!("{name} ({which})"),
                None => name.clone(),
            });
        }
        if let Some(quality) = &self.quality {
            said.push(quality.clone());
        }
        if let Some(rate) = self.sample_rate {
            said.push(format!("{rate} Hz"));
        }
        if matches!(self.speakers, Some(many) if many > 1) {
            said.push(format!("{} speakers", self.speakers.unwrap_or_default()));
        }
        said.join(" · ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(rows: &[(&str, u64)]) -> Vec<serde_json::Value> {
        rows.iter()
            .map(|(path, size)| serde_json::json!({ "type": "file", "path": path, "size": size }))
            .collect()
    }

    /// The pairing rule, with nothing else consulted.
    fn pair(rows: &[(&str, u64)]) -> Vec<String> {
        let entries = tree(rows);
        let sizes: BTreeMap<&str, u64> = entries
            .iter()
            .map(|e| (e["path"].as_str().unwrap(), e["size"].as_u64().unwrap_or(0)))
            .collect();
        entries
            .iter()
            .filter_map(|e| {
                let path = e["path"].as_str()?;
                path.ends_with(".onnx")
                    .then(|| sizes.contains_key(format!("{path}.json").as_str()))
                    .filter(|has| *has)
                    .map(|_| path.to_owned())
            })
            .collect()
    }

    #[test]
    fn a_voice_is_an_onnx_with_a_sidecar_beside_it() {
        let found = pair(&[
            (
                "es/es_ES/davefx/medium/es_ES-davefx-medium.onnx",
                63_201_294,
            ),
            (
                "es/es_ES/davefx/medium/es_ES-davefx-medium.onnx.json",
                4_817,
            ),
        ]);
        assert_eq!(found, ["es/es_ES/davefx/medium/es_ES-davefx-medium.onnx"]);
    }

    #[test]
    fn an_onnx_with_no_sidecar_is_not_a_voice() {
        // This is the whole reason the rule is a pairing rather than an extension check: a
        // repository full of ONNX models that are not voices must come back empty, not full.
        assert!(pair(&[("some/detector.onnx", 1_000)]).is_empty());
    }

    #[test]
    fn a_sidecar_with_no_model_is_not_a_voice_either() {
        assert!(pair(&[("es/x.onnx.json", 10)]).is_empty());
    }

    #[test]
    fn the_name_is_the_source_identifier_and_the_id_carries_its_repository() {
        let voice = Found {
            repo: "rhasspy/piper-voices".into(),
            onnx: "es/es_ES/davefx/medium/es_ES-davefx-medium.onnx".into(),
            sidecar: "es/es_ES/davefx/medium/es_ES-davefx-medium.onnx.json".into(),
            bytes: 63_201_294,
            sidecar_bytes: 4_817,
            sha256: None,
            spoken: None,
            sample: None,
        };
        assert_eq!(voice.name(), "es_ES-davefx-medium");
        assert_eq!(
            voice.id(),
            "piper:rhasspy/piper-voices:es/es_ES/davefx/medium/es_ES-davefx-medium.onnx"
        );
    }

    #[test]
    fn a_voice_offers_both_of_its_files_and_the_model_first() {
        let voice = Found {
            repo: "rhasspy/piper-voices".into(),
            onnx: "es/a.onnx".into(),
            sidecar: "es/a.onnx.json".into(),
            bytes: 100,
            sidecar_bytes: 4,
            sha256: Some("ABC".into()),
            spoken: None,
            sample: None,
        };
        let asset = voice.asset(&REPOSITORIES[0]);
        assert_eq!(asset.kind, Kind::Voice);
        let names: Vec<&str> = asset.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, ["es/a.onnx", "es/a.onnx.json"]);
        // The sidecar is not optional, so a caller taking only `principal()` would install
        // something that cannot speak. `parts()` is what the installer uses.
        assert_eq!(
            asset.principal().map(|f| f.name.as_str()),
            Some("es/a.onnx")
        );
    }

    #[test]
    fn downloads_are_not_reported_because_hugging_face_counts_repositories() {
        let voice = Found {
            repo: "rhasspy/piper-voices".into(),
            onnx: "es/a.onnx".into(),
            sidecar: "es/a.onnx.json".into(),
            bytes: 1,
            sidecar_bytes: 1,
            sha256: None,
            spoken: None,
            sample: None,
        };
        assert_eq!(voice.asset(&REPOSITORIES[0]).downloads, 0);
    }

    #[test]
    fn the_next_page_is_read_out_of_the_link_header() {
        let header = "<https://huggingface.co/api/models/x/tree/main?cursor=abc>; rel=\"next\"";
        assert_eq!(
            next_page(header),
            Some("https://huggingface.co/api/models/x/tree/main?cursor=abc")
        );
        assert_eq!(next_page("<https://x>; rel=\"prev\""), None);
        assert_eq!(next_page(""), None);
    }

    /// The real sidecar, from the file measured on 2026-09-04.
    #[test]
    fn a_sidecar_says_what_a_filename_only_claims() {
        let spoken = Spoken::read(
            r#"{
                "audio": { "sample_rate": 22050, "quality": "medium" },
                "espeak": { "voice": "es" },
                "language": {
                    "code": "es_ES", "family": "es", "region": "ES",
                    "name_native": "Español", "name_english": "Spanish",
                    "country_english": "Spain"
                },
                "num_speakers": 1,
                "piper_version": "1.0.0"
            }"#,
        )
        .expect("a Piper sidecar reads");
        assert_eq!(spoken.native.as_deref(), Some("Español"));
        assert_eq!(spoken.quality.as_deref(), Some("medium"));
        assert_eq!(spoken.sample_rate, Some(22050));
        assert_eq!(spoken.piper_version.as_deref(), Some("1.0.0"));
        assert_eq!(spoken.country.as_deref(), Some("Spain"));
        // The country, not the code: three voices all read `Spanish`, and *Spain* against
        // *Mexico* is the thing somebody is actually choosing between.
        assert_eq!(spoken.plainly(), "Español (Spain) · medium · 22050 Hz");
        // And the words somebody would type, all of them, lowercased.
        for word in ["spanish", "español", "spain", "es_es", "medium"] {
            assert!(spoken.words().contains(word), "{word} is not searchable");
        }
        // One speaker is not worth saying; it is what nearly every voice is.
        assert!(!spoken.plainly().contains("speakers"));
    }

    #[test]
    fn a_sidecar_that_carries_less_says_less_rather_than_guessing() {
        let spoken = Spoken::read(r#"{ "audio": { "quality": "high" } }"#).expect("still reads");
        assert_eq!(spoken.plainly(), "high");
        assert_eq!(spoken.native, None);
        assert_eq!(spoken.piper_version, None);
        // And something that is not a sidecar at all is `None`, not an empty description.
        assert_eq!(Spoken::read("not json"), None);
    }

    /// The shelf reads a voice from its sidecar, and never calls it unreadable.
    ///
    /// `understand` answers *its own bytes could not be read* about an `.onnx`, which is true
    /// about tensors and false about the file. This is the one that would have shipped a shelf
    /// of `unreadable · 0 bytes` rows for files that speak perfectly.
    #[test]
    fn a_voice_on_a_shelf_is_described_rather_than_called_unreadable() {
        let dir = std::env::temp_dir().join(format!("epoch-voice-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a shelf to read");
        let onnx = dir.join("es_ES-davefx-medium.onnx");
        std::fs::write(&onnx, b"not really a model, and it does not matter here").unwrap();
        std::fs::write(
            dir.join("es_ES-davefx-medium.onnx.json"),
            br#"{"audio":{"sample_rate":22050,"quality":"medium"},
                "language":{"code":"es_ES","name_native":"Espanol"},"num_speakers":1}"#,
        )
        .unwrap();

        let held = crate::generative::held_on(&dir);
        assert_eq!(
            held.len(),
            1,
            "the sidecar is not a second thing on the shelf"
        );
        assert_eq!(held[0].file, "es_ES-davefx-medium.onnx");
        assert_eq!(held[0].kind, "a voice");
        assert_eq!(held[0].base, "Espanol (es_ES) · medium · 22050 Hz");
        assert_eq!(held[0].medium.as_deref(), Some("sound"));
        assert_eq!(held[0].unread, None);
        assert!(held[0].bytes > 0, "a real size, read from the file");

        // And a voice whose sidecar never arrived says *that*, which is the failure Piper would
        // otherwise report minutes later and somewhere else.
        std::fs::remove_file(dir.join("es_ES-davefx-medium.onnx.json")).unwrap();
        let held = crate::generative::held_on(&dir);
        assert_eq!(held[0].base, "unknown");
        assert!(held[0].unread.is_some());

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The real repository, and the number is the point.
    ///
    /// A first-page-only reader finds 28 of the 175 and looks like it worked. So this asserts a
    /// figure only a paged reader can reach.
    #[test]
    #[ignore = "network"]
    fn the_official_repository_holds_the_voices_it_claims() {
        let found = voices_in("rhasspy/piper-voices").expect("Hugging Face answered");
        assert!(
            found.len() >= 175,
            "expected at least 175 voices, paired {}",
            found.len()
        );
        assert!(found
            .iter()
            .any(|voice| voice.name() == "es_ES-davefx-medium"));
        let spanish = found.iter().filter(|v| v.onnx.starts_with("es/")).count();
        assert!(spanish >= 9, "expected the Spanish voices, found {spanish}");

        // **The word a person actually types.** Reported from the window on 2026-09-04:
        // searching `spanish` answered *Nothing here matched that* on a shelf holding nine
        // Spanish voices, because the only thing being matched was the path — `es/es_ES/…`.
        let by_word = Voices::new()
            .search(&Query {
                words: "spanish".into(),
                ..Query::default()
            })
            .expect("the shelf answers");
        assert!(
            by_word.assets.len() >= 9,
            "searching `spanish` found {}",
            by_word.assets.len()
        );
        // And the row says what it is before anybody installs it.
        assert!(by_word
            .assets
            .iter()
            .all(|it| it.said_base.contains("Espa")));

        // A country is a word somebody types too, and it is the half a language code does not
        // give: three locales all read `Spanish`.
        let by_country = Voices::new()
            .search(&Query {
                words: "mexico".into(),
                ..Query::default()
            })
            .expect("the shelf answers");
        assert!(
            by_country.assets.iter().all(|it| it.id.contains("/es_MX/")),
            "`mexico` reached something that is not Mexican"
        );
        assert!(!by_country.assets.is_empty(), "`mexico` found nothing");
    }
}
