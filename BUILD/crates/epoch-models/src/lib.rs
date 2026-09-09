//! The Models Workshop — what this machine has, what it could have, and what will fit.
//!
//! ## What was measured before this was written
//!
//! Same discipline as the MCP Workshop next door: ask the sources what they actually answer,
//! then write against that rather than against a memory of an API.
//!
//! | asked | answer |
//! |---|---|
//! | `localhost:11434/api/tags` | what is installed, with real sizes and capabilities |
//! | `ollama.com/api/tags` | machine readable, **and not the library** — see below |
//! | `registry.ollama.ai/v2/library/<name>/tags/list` | 404 — no tag index |
//! | `registry.ollama.ai/v2/library/<name>/manifests/<tag>` | 200, with **per-layer sizes** |
//!
//! ## What `ollama.com/api/tags` is, and what it is not
//!
//! It looked like the library and it is not one. Measured: **19 entries**, of mixed shape —
//! some are families (`glm-5.1`, `nemotron-3-super`), some are tags (`gpt-oss:20b`,
//! `gemma4:31b`) — and it does **not** contain `gemma4:12b`, which the machine this was written
//! on has installed.
//!
//! So it is a **featured list**, and calling it a catalogue would be the kind of label that
//! makes somebody believe those nineteen are everything they could run. What makes the Workshop
//! useful anyway is the other half: [`weigh`] takes **any** name, because the manifest endpoint
//! does. Somebody who knows they want `qwen3:14b` types it and gets a real size and a real
//! answer about whether it fits.
//!
//! ## The catalogue's size is not the model's size
//!
//! This is the finding that decides the whole module. `ollama.com/api/tags` reports
//! `nemotron-3-super` at 230 GB and `deepseek-v4-pro` at 892 GB — those are not downloads
//! anybody makes on a desktop. The manifest for one *tag* reports what is really pulled:
//! `gemma4:12b` is a 7.38 GB model layer, which matches the 7.56 GB the local install reports
//! for the same thing.
//!
//! So the catalogue supplies **names** and the manifest supplies **size**, and the size is only
//! ever shown for a tag somebody actually pointed at. Showing the catalogue's number beside a
//! model would be an invented reading with a plausible unit on it.
//!
//! ## Hugging Face, and what a filter is allowed to mean
//!
//! Ollama's featured list is nineteen entries. Hugging Face's `/api/models?filter=gguf` is the
//! rest of the world, sorted by downloads, and Ollama pulls straight from it — so it is the
//! second source rather than a nicer one.
//!
//! But the two describe themselves differently, and **that decides what a filter may claim**:
//!
//! | source | what it declares |
//! |---|---|
//! | installed (local `/api/tags`) | `capabilities` — the exact words `vision`, `tools`, `thinking`, `audio` |
//! | Ollama featured | **nothing.** `details` come back as empty strings |
//! | Hugging Face | `pipeline_tag` and `tags` — vision, image and audio, never tools or thinking |
//!
//! So a filter narrows to what a source **said**, and anything that declared nothing is reported
//! as *not described* rather than quietly dropped. A filter that hid the featured list because
//! Ollama does not annotate it would be a filter that lies by omission — the same failure the
//! MCP Workshop's chips are labelled to avoid next door.
//!
//! **`cloud` is not a facet of a model.** It turned out to be a property of the **Service** it
//! runs on, which Epoch already measures (`ProviderStatus::local`). A model is not cloudy; the
//! machine it runs on is somewhere.
//!
//! ## Recommending is not choosing
//!
//! The Workshop measures this machine ([`machine::Machine`]) and says which models suit it. The
//! user may assign something their card cannot hold, and Epoch respects that — it told them,
//! which is the whole of its job here (ROADMAP, Phase 9).

pub mod accepts;
pub mod artifact;
pub mod catalogue;
pub mod civitai;
pub mod comfy;
pub mod deck;
pub mod engines;
pub mod generative;
pub mod gguf;
pub mod health;
pub mod hf;
pub mod hugging_face;
pub mod hygiene;
pub mod identity;
pub mod load;
pub mod loadout;
pub mod machine;
pub mod ollama_library;
pub mod quiet;
pub mod recipes;
pub mod runtimes;
pub mod speeds;
/// The programs that make pictures. Not runtimes — see the module's own note (ADR-0030).
pub mod studio;
pub mod tuning;
pub mod voices;

pub use machine::Machine;

use serde::{Deserialize, Serialize};

/// What Ollama is featuring. **Not the library** — nineteen entries when this was measured.
const FEATURED: &str = "https://ollama.com/api/tags";

/// Where a tag's real size is written.
const REGISTRY: &str = "https://registry.ollama.ai/v2/library";

/// Everything else. GGUF only, because that is what Ollama can pull.
const HUGGING_FACE: &str = "https://huggingface.co/api/models";

/// What a model says it can do.
///
/// The words are Ollama's own, because Ollama is the source that actually declares them — and a
/// second vocabulary translated from the first is two things to keep in step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Facet {
    Vision,
    Tools,
    Thinking,
    Audio,
    /// It makes pictures rather than reading them. Hugging Face's `text-to-image` — Ollama has
    /// no word for this because it does not host one.
    Image,
}

impl Facet {
    pub const ALL: [Facet; 5] = [
        Facet::Vision,
        Facet::Tools,
        Facet::Thinking,
        Facet::Audio,
        Facet::Image,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Facet::Vision => "vision",
            Facet::Tools => "tools",
            Facet::Thinking => "thinking",
            Facet::Audio => "audio",
            Facet::Image => "image",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.id() == id)
    }
}

/// One model, as a shelf shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Offer {
    /// What to pull, exactly as it is typed — `gemma4:12b`.
    pub name: String,
    /// Whether this machine already has it. Read from the Provider, never from a list.
    pub installed: bool,
    /// Real bytes, from the manifest of this exact tag. `None` until somebody asks about it —
    /// one HTTP call per model would be a shelf that takes a minute to open.
    pub bytes: Option<u64>,
    /// Whether it fits in free video memory. `None` when either number is unknown, which is
    /// **not** the same as no.
    pub fits: Option<bool>,
    /// Where this came from: `"featured"` or `"huggingface"`.
    pub source: &'static str,
    /// What it says it can do, in the source's own words.
    ///
    /// Empty means one of two very different things, which is why [`Offer::described`] exists
    /// beside it: nothing declared, or nothing declared *that Epoch knows a word for*.
    pub facets: Vec<Facet>,
    /// Whether the source describes this model at all.
    ///
    /// **The honest half of a filter.** Ollama's featured list annotates nothing, so filtering
    /// it by `vision` would hide every entry and imply none of them can see. `false` here lets a
    /// surface say *not described* instead of pretending it asked and got a no.
    pub described: bool,
    /// What to type to get it. `gemma4:12b`, or `hf.co/user/repo` for Hugging Face.
    pub pull: String,
    /// Whether Ollama can fetch this one at all.
    ///
    /// **Measured against Ollama's own refusal**, never guessed: asking for a sharded tag
    /// answers `400: This tag is a sharded GGUF. Ollama does not yet support pulling sharded
    /// GGUF via the registry`. The file is still perfectly fetchable with `hf`, so this closes
    /// one destination rather than hiding the model.
    /// **No `#[serde(default)]` here, because it never fired.** The attribute read
    /// `default = "yes"`, and `Offer` derives `Serialize` only — a default applies when
    /// something is *read*, and nothing reads one of these. Clippy found the helper as dead
    /// code, which is the only reason anybody checked what the attribute was doing.
    pub pullable: bool,
}

/// One entry of a `/api/tags` answer, local or remote. The two speak the same shape.
#[derive(Debug, Deserialize)]
struct Tagged {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Tags {
    #[serde(default)]
    models: Vec<Tagged>,
}

/// What Ollama is featuring right now, with what this machine already has marked.
///
/// Sizes are absent here on purpose: see the module note. A shelf that fetched a manifest per
/// row would spend a minute to show numbers nobody asked for yet.
pub fn featured(installed: &[String]) -> Result<Vec<Offer>, String> {
    let body: Tags = ureq::get(FEATURED)
        .timeout(std::time::Duration::from_secs(12))
        .call()
        .map_err(|err| format!("could not read the model library: {err}"))?
        .into_json()
        .map_err(|err| format!("the model library answered something unreadable: {err}"))?;

    let mut offers: Vec<Offer> = body
        .models
        .into_iter()
        .map(|model| Offer {
            // An Ollama registry name is one artefact its own registry serves.
            pullable: true,
            // **By family, because the two lists speak differently.** The featured list mixes
            // families and tags; a local install is always a tag. Compared literally,
            // `gemma4:12b` on disk never matches `gemma4` on the shelf, and the Workshop offers
            // somebody a download they already have — which is how this was found.
            installed: installed
                .iter()
                .any(|have| family(have) == family(&model.name)),
            pull: model.name.clone(),
            name: model.name,
            bytes: None,
            fits: None,
            source: "featured",
            facets: Vec::new(),
            // Measured: `details` come back as empty strings and there is no capabilities key.
            // Nothing is declared here, and saying so is what stops a filter from lying.
            described: false,
        })
        .collect();
    // What is already here, first. It is what somebody is most likely looking for, and it is
    // the half of the shelf that is a measurement rather than an offer.
    offers.sort_by(|a, b| b.installed.cmp(&a.installed).then(a.name.cmp(&b.name)));
    Ok(offers)
}

/// Search Ollama's own shelf, as [`featured`] lists the front of it.
///
/// ## Why the shelf needed searching at all
///
/// `featured` reads `ollama.com/api/tags` — nineteen entries, the ones on the front page. Every
/// other model Ollama publishes was reachable only by typing its exact name into the weigher,
/// which is a search box for people who already know the answer.
///
/// Hugging Face covers the rest of the world and is searched properly (`search`). What was
/// missing is the middle: `qwen3-coder`, `qwen3-vl`, `qwen2.5-coder` — curated names somebody
/// who has used Ollama before types first, and which resolve against a registry that genuinely
/// answers.
///
/// ## Weighed here, exactly as the featured shelf is not
///
/// A name is all the search page gives, so `bytes` and `fits` stay `None` and the Workshop asks
/// about a specific one when somebody picks it — the same shape `featured` produces, for the
/// same reason: weighing nineteen models to draw a list would be nineteen round trips before
/// the first row appears.
pub fn search_shelf(query: &str, installed: &[String]) -> Result<Vec<Offer>, String> {
    let found = crate::ollama_library::search(query)?;
    if found.unreadable {
        return Err(
            "Ollama's library page could not be read. That is a change at their end rather than a result — try Hugging Face, which has an API."
                .to_owned(),
        );
    }
    let mut offers: Vec<Offer> = found
        .names
        .into_iter()
        .map(|name| Offer {
            pullable: true,
            // By family, for the reason `featured` states: the shelf lists families and a local
            // install is always a tag, so `gemma4:12b` on disk never matches `gemma4` literally.
            installed: installed.iter().any(|have| family(have) == family(&name)),
            pull: name.clone(),
            name,
            bytes: None,
            fits: None,
            source: "ollama",
            facets: Vec::new(),
            // The search page declares nothing about a model, and saying so is what stops a
            // filter from lying about what it filtered.
            described: false,
        })
        .collect();
    offers.sort_by(|a, b| b.installed.cmp(&a.installed).then(a.name.cmp(&b.name)));
    Ok(offers)
}

/// What one model really costs here, and whether it fits.
///
/// ## Two registries, and the name says which
///
/// `ollama pull` takes names from two places and they are not the same service. `gemma4:12b`
/// resolves against Ollama's own registry; `hf.co/unsloth/Qwen3-8B-GGUF` resolves against
/// Hugging Face. Sending the second to the first produced
/// `registry.ollama.ai/v2/library/hf.co/unsloth/...: 404` -- a URL that could never have worked,
/// so every Hugging Face result the Workshop offered could be searched and never weighed.
///
/// ## A Hugging Face repository has no size
///
/// It has one **per quantisation**: `unsloth/Qwen3-8B-GGUF` holds 25 GGUF files between 3.3 GB
/// and 16.4 GB (measured). "Will it run here?" therefore has no answer for a repository, only
/// for a file -- so when nobody names one, this picks **the largest that fits this machine** and
/// reports which. That is the question the person was asking. When none fit, the smallest is
/// reported with `fits: false`: the honest answer is the cheapest option, and that it is still
/// too big.
///
/// `pull` always carries the exact quantisation weighed, so what gets downloaded is what was
/// measured rather than whatever a default would have chosen.
pub fn weigh(name: &str, machine: &machine::Machine) -> Result<Offer, String> {
    match repository(name) {
        Some((repo, wanted)) => weigh_on_hugging_face(name, &repo, wanted.as_deref(), machine),
        None => weigh_in_the_ollama_registry(name, machine),
    }
}

/// A Hugging Face repository and the quantisation asked for, when the name is one.
///
/// Both prefixes, because both work at the command line and somebody will paste either.
fn repository(name: &str) -> Option<(String, Option<String>)> {
    let rest = name
        .strip_prefix("hf.co/")
        .or_else(|| name.strip_prefix("huggingface.co/"))?;
    // The quantisation follows the last colon, and a repository path has none.
    match rest.rsplit_once(':') {
        Some((repo, quant)) => Some((repo.to_owned(), Some(quant.to_owned()))),
        None => Some((rest.to_owned(), None)),
    }
}

/// One file in a repository's tree.
#[derive(Debug, Deserialize)]
struct Entry {
    path: String,
    #[serde(default)]
    size: u64,
}

fn weigh_on_hugging_face(
    name: &str,
    repo: &str,
    wanted: Option<&str>,
    machine: &machine::Machine,
) -> Result<Offer, String> {
    let url = format!("{HUGGING_FACE}/{repo}/tree/main?recursive=true");
    let tree: Vec<Entry> = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|err| format!("Hugging Face has no repository '{repo}': {err}"))?
        .into_json()
        .map_err(|err| format!("the file list for '{repo}' is unreadable: {err}"))?;

    let mut files: Vec<(String, u64)> = tree
        .into_iter()
        .filter(|entry| entry.path.to_lowercase().ends_with(".gguf"))
        .filter(|entry| entry.size > 0)
        .map(|entry| (entry.path, entry.size))
        .collect();
    if files.is_empty() {
        return Err(format!(
            "'{repo}' has no GGUF files, so Ollama cannot pull it"
        ));
    }
    files.sort_by_key(|(_, size)| *size);

    let chosen = choose(&files, wanted, machine)?;
    let (path, _) = chosen;
    /*
        **The same reader `variants` uses, because there were two.**

        This one used to take everything after the last dash of the filename. On a sharded
        variant that is the shard number: `BF16/Qwen3.8-27B-BF16-00001-of-00002.gguf` weighed as
        `00002`, so the download asked Ollama for `hf.co/…:00002` and got *File does not exist*.
        It also dropped the `UD-` prefix that unsloth's names carry.

        Two functions answering "which quantisation is this file" is the same shape as every
        other defect found today: they agreed until a filename neither had been tested against.
    */
    let (quant, tag) = read_quant(path)
        .ok_or_else(|| format!("'{path}' does not name a quantisation Epoch can read"))?;

    /*
        **A variant is all of its parts, and weighing found one of them.**

        `choose` returns a file; a large quantisation is published as several. Weighing `BF16`
        answered with the size of shard one — 30 GB against the 55.6 GB the variant list showed
        for the same thing — so the verdict was about a file nobody can run on its own, and the
        two screens disagreed about one model.

        Summed here the same way `variants` sums them, so both readings come from one rule.
    */
    let (bytes, parts) = files
        .iter()
        .filter(|(p, _)| read_quant(p).as_ref() == Some(&(quant.clone(), tag.clone())))
        .fold((0u64, 0usize), |(sum, n), (_, size)| (sum + size, n + 1));

    Ok(Offer {
        installed: false,
        bytes: Some(bytes),
        // Several files is a shard set, and Ollama refuses those by name.
        pullable: parts <= 1,
        fits: machine.fits(bytes),
        source: "huggingface",
        facets: Vec::new(),
        described: false,
        // The exact file, never the repository: what is downloaded must be what was weighed.
        pull: format!("hf.co/{repo}:{quant}"),
        name: format!("{name} - {quant}"),
    })
}

/// Which file to weigh out of a repository's many.
///
/// Separated from the request so the rule is testable without a network: a named quantisation is
/// honoured exactly, and an unnamed one becomes "the largest that runs here".
fn choose<'a>(
    files: &'a [(String, u64)],
    wanted: Option<&str>,
    machine: &machine::Machine,
) -> Result<&'a (String, u64), String> {
    match wanted {
        // Named, so it is weighed whether or not it fits. Refusing would be Epoch deciding for
        // somebody who already decided.
        Some(quant) => {
            /*
                **Matched on what the file *is*, not on the letters in its path.**

                A substring over a size-sorted list picked the wrong file twice over. Asking for
                `Q4_0` in a repository that publishes both `Qwen3.8-27B-Q4_0.gguf` and
                `MTP/mtp-Qwen3.8-27B-Q4_0.gguf` found the MTP module first, because it is a tenth
                the size and the list is smallest-first — so somebody choosing a 17 GB model was
                handed a 1.4 GB module that is not the model at all.

                Reading each file the same way the variant list reads it makes the two agree by
                construction. A tagged extra is only ever chosen when it is asked for by name.
            */
            let wanted = quant.trim().to_lowercase();
            files
                .iter()
                .find(|(path, _)| match read_quant(path) {
                    // The model itself first: anything carrying a tag is published *beside*
                    // the model rather than as a version of it.
                    Some((found, tag)) => tag.is_none() && found.to_lowercase() == wanted,
                    None => false,
                })
                // Then by tag, so `MTP` still reaches the module somebody deliberately named.
                .or_else(|| {
                    files.iter().find(|(path, _)| {
                        read_quant(path).is_some_and(|(found, tag)| {
                            tag.is_some_and(|t| t.to_lowercase() == wanted)
                                || found.to_lowercase() == wanted
                        })
                    })
                })
                .ok_or_else(|| {
                    format!(
                        "no '{quant}' here. This repository has {} files.",
                        files.len()
                    )
                })
        }
        // Nobody named one, so answer the question actually asked: the largest that runs here.
        // Falling back to the smallest keeps the answer useful when none do.
        None => Ok(files
            .iter()
            .rfind(|(_, size)| machine.fits(*size) == Some(true))
            .unwrap_or(&files[0])),
    }
}

/// What one tag really costs, from its manifest.
///
/// **The sum of the layers**, which is what a pull downloads -- not the catalogue's number for
/// the model family, which is the size of everything that family has ever published.
fn weigh_in_the_ollama_registry(name: &str, machine: &machine::Machine) -> Result<Offer, String> {
    let (model, tag) = name.split_once(':').unwrap_or((name, "latest"));
    let url = format!("{REGISTRY}/{model}/manifests/{tag}");

    let manifest: Manifest = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(12))
        .call()
        .map_err(|err| match err {
            // **Measured, and worth saying properly.** Ollama's featured list mixes models you
            // can download with models it serves in its own cloud: `gemma4:31b` and `gpt-oss:20b`
            // have manifests, while `deepseek-v4-flash`, `glm-5.1`, `kimi-k2.6` and
            // `qwen3.5:397b` return 404 with no tag list at all. A bare "404" reads as a broken
            // Workshop; this reads as what it is.
            ureq::Error::Status(404, _) => NOT_DOWNLOADABLE.replace("{name}", name),
            other => format!("no manifest for '{name}': {other}"),
        })?
        .into_json()
        .map_err(|err| format!("the manifest for '{name}' is unreadable: {err}"))?;

    let bytes: u64 = manifest.layers.iter().map(|layer| layer.size).sum();
    Ok(Offer {
        installed: false,
        // A name in Ollama's own registry, served by Ollama. There is nothing to refuse.
        pullable: true,
        bytes: Some(bytes),
        fits: machine.fits(bytes),
        source: "featured",
        facets: Vec::new(),
        described: false,
        pull: name.to_owned(),
        name: name.to_owned(),
    })
}

/// Said when Ollama's registry has no manifest at all for a name.
const NOT_DOWNLOADABLE: &str = "'{name}' has nothing to download. Ollama's featured list also names models it runs in its cloud, and those have no manifest to weigh -- check the spelling, or pick one the shelf marks as already here.";

/// How a download is going, as the runtime itself reports it.
///
/// Measured against a real `POST /api/pull`: a line is `{"status":"..."}`, optionally with
/// `total` and `completed` when it is moving bytes, and `{"error":"..."}` when it is not going
/// to work. Nothing here is inferred -- a percentage Epoch calculated from a guess would be the
/// invented reading this project removes everywhere else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Fetching {
    /// The runtime's own words: "pulling manifest", "verifying sha256 digest", "writing
    /// manifest". Shown as they arrive, because they are a truer account of what is happening
    /// than any sentence Epoch could write in advance.
    pub status: String,
    /// Bytes for this layer, when this line is about bytes. `None` for the steps that are not.
    pub total: Option<u64>,
    pub completed: Option<u64>,
}

/// Download a model into the local runtime, reporting progress as it goes.
///
/// ## Only Ollama, and that is a measurement rather than a preference
///
/// Ollama has an endpoint that fetches a model and files it where the runtime will find it.
/// llama.cpp and LM Studio have no such thing -- there is no request Epoch could send them that
/// would result in a model being installed. So this does what can be done, and
/// [`file_url`] answers the other half honestly instead of pretending the button exists
/// everywhere.
///
/// ## Blocking, one line at a time
///
/// The answer is NDJSON and arrives over minutes. `sink` is called for every line, in order, so
/// a surface can show the runtime's own account of what it is doing rather than a spinner.
pub fn pull(
    endpoint: &str,
    model: &str,
    stopped: &dyn Fn() -> bool,
    sink: &mut dyn FnMut(Fetching),
) -> Result<(), String> {
    use std::io::{BufRead, BufReader};

    let answer = ureq::post(&format!("{}/api/pull", endpoint.trim_end_matches('/')))
        // **No timeout at all**, deliberately. `ureq`'s timeout covers the whole exchange, and
        // a download is minutes — setting one would abort a model that was arriving perfectly
        // well. Stopping is the user's business, through `stopped`.
        .send_json(serde_json::json!({ "model": model, "stream": true }))
        .map_err(|err| format!("the local runtime would not start the download: {err}"))?;

    #[derive(Deserialize)]
    struct Line {
        #[serde(default)]
        status: String,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        total: Option<u64>,
        #[serde(default)]
        completed: Option<u64>,
    }

    for line in BufReader::new(answer.into_reader()).lines() {
        if stopped() {
            // The connection closes, and Ollama stops. What it already wrote stays -- a partial
            // pull resumes rather than starting again, which is its behaviour and not ours.
            return Err("the download was stopped".to_owned());
        }
        let Ok(text) = line else { break };
        let Ok(read) = serde_json::from_str::<Line>(&text) else {
            continue;
        };
        // **The runtime's own sentence, not ours.** Measured: a missing model answers
        // `{"error":"pull model manifest: file does not exist"}` and nothing else -- there is no
        // status line to make sense of, and inventing one would hide the only information there.
        if let Some(why) = read.error {
            return Err(why);
        }
        sink(Fetching {
            status: read.status,
            total: read.total,
            completed: read.completed,
        });
    }
    Ok(())
}

/// Where the actual file lives, for a runtime that cannot be told to fetch it.
///
/// llama.cpp and LM Studio both take a GGUF from disk, and neither has an endpoint that installs
/// one. So the useful thing Epoch can give them is the address of the exact file it weighed --
/// measured: `huggingface.co/{repo}/resolve/main/{file}` answers 200 and redirects to the CDN.
///
/// `None` for anything that is not a Hugging Face offer: Ollama's own registry serves layers by
/// digest rather than a file somebody can download and open, so there is no honest URL to give.
pub fn file_url(offer: &Offer) -> Option<String> {
    let rest = offer.pull.strip_prefix("hf.co/")?;
    let (repo, quant) = rest.rsplit_once(':')?;
    // The filename is not derivable from the quantisation -- repositories name their files
    // differently -- so the tree is asked again rather than guessed at.
    let tree: Vec<Entry> = ureq::get(&format!("{HUGGING_FACE}/{repo}/tree/main?recursive=true"))
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .ok()?
        .into_json()
        .ok()?;
    let needle = quant.to_lowercase();
    let file = tree.into_iter().find(|entry| {
        let path = entry.path.to_lowercase();
        path.ends_with(".gguf") && path.contains(&needle)
    })?;
    Some(format!(
        "https://huggingface.co/{repo}/resolve/main/{}",
        file.path
    ))
}

/// One quantisation of a repository, and what it costs here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    /// How the repository spells it: `UD-Q4_K_XL`, `Q8_0`, `BF16`.
    pub quant: String,
    /// Bytes, **summed across shards**. A large quantisation is published in parts, and each
    /// part on its own is not something anybody can run.
    pub bytes: u64,
    /// How many files it is in. Shown when it is more than one, because a 55 GB download that
    /// arrives as three files is worth knowing about before it starts.
    pub files: usize,
    /// What to type to get exactly this one.
    pub pull: String,
    /// A label the repository attached rather than a quantisation of the model — `MTP` is the
    /// multi-token-prediction module, published alongside and much smaller. Naming it as though
    /// it were a 1.4 GB version of a 27B model would be the list lying about what it offers.
    pub tag: Option<String>,
    /// Whether this is the same quantisation **with the model's own prediction head in it**.
    ///
    /// ## Its own field, because `tag` already means the opposite
    ///
    /// `tag: MTP` marks a *module* — 1.37 GB beside a 27B, not a version of the model. What this
    /// marks is a whole model that happens to carry the head: `unsloth/X-MTP-GGUF` publishes the
    /// same quantisations as `unsloth/X-GGUF`, each one the plain file plus one block. Measured
    /// on `UD-IQ4_XS`: 17,730,509,792 bytes against 18,209,036,576, and 733 tensors against 753.
    ///
    /// Reusing `tag` would have described an 18 GB model as *the MTP module, not a version of
    /// it* — one word with two meanings, which this codebase has paid for more than once.
    ///
    /// ## Why it is worth offering
    ///
    /// The head is weights, so it cannot be switched on later: pick the plain file and
    /// `--spec-type draft-mtp` is unavailable to that model for ever, with nothing on the screen
    /// saying so. Measured on this machine, same card, same prompt:
    ///
    /// | | tok/s |
    /// |---|---|
    /// | plain file, speculation off | 45.8 |
    /// | this file, speculation off | 44.1 |
    /// | this file, `draft-mtp n=1` | **47.6** |
    ///
    /// So it costs 3.7% to carry unused and pays 3.9% over the plain file when used — on a
    /// runtime that can use it. llama.cpp can; Ollama and LM Studio expose no `--spec-type`, and
    /// there it is 0.48 GB and 3.7% for nothing. The trade is stated and the choice is the
    /// user's.
    ///
    /// **And the repository name is a claim, not evidence.** Nothing here reads the head; what
    /// does is `Identity::has_mtp`, from the file's own `nextn_predict_layers`, and that is what
    /// decides whether `draft-mtp` is ever offered. A repository that says MTP and ships files
    /// without one simply never gets the option — no separate check needed, and no chance of a
    /// name outvoting the bytes.
    #[serde(default)]
    pub own_head: bool,
    /// Whether Ollama can fetch this one at all.
    ///
    /// **Measured against Ollama's own refusal**, not guessed: asking for a sharded tag answers
    /// `400: This tag is a sharded GGUF. Ollama does not yet support pulling sharded GGUF via
    /// the registry`. Every multi-file variant is one, so a DOWNLOAD button beside it is a
    /// control that cannot do what it says.
    ///
    /// The file itself is still perfectly fetchable — `hf` handles shards — so this disables one
    /// destination rather than hiding the variant.
    pub pullable: bool,
}

/// Every quantisation a repository publishes, grouped the way it is read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// `1`, `4`, `16`. Zero when the name says nothing about width.
    pub bits: u8,
    pub variants: Vec<Variant>,
}

/// What a repository actually offers, measured.
///
/// ## Why a repository is a list rather than a size
///
/// `unsloth/Qwen3.8-27B-GGUF` publishes 27 of these, from 6.19 GB to 55.6 GB. "Will it run
/// here?" has no answer for the repository and a different answer for each of them — so this
/// returns all of them and lets the caller ask about each against the machine it cares about.
///
/// ## Shards are summed
///
/// Measured on that repository: `BF16` arrives as `-00001-of-00002` and `-00002-of-00002`.
/// Treating either as a variant would offer a download that cannot be run, and reporting the
/// larger one as the size would be wrong by 5 GB.
///
/// ## One request
///
/// `?blobs=true` returns every file with its size in the model's own record, so this asks once
/// rather than walking a tree.
/// The repository that publishes the same quantisations with the prediction head in them.
///
/// **A naming convention, and therefore a claim.** `unsloth/Qwen3.6-35B-A3B-GGUF` has
/// `unsloth/Qwen3.6-35B-A3B-MTP-GGUF` beside it. Whether it exists is *asked*; whether a file
/// from it really carries the head is read from that file's own header afterwards, by the code
/// that decides whether `draft-mtp` may be offered at all.
///
/// `None` for a repository that already is one, so the list never offers itself twice.
pub fn mtp_sibling(repo: &str) -> Option<String> {
    let upper = repo.to_uppercase();
    if upper.contains("-MTP-") || upper.ends_with("-MTP") {
        return None;
    }
    // Only where the name ends in the suffix the convention attaches to. A repository called
    // something else is not one this can guess at, and guessing would produce a request for a
    // repository nobody published.
    let stem = repo
        .strip_suffix("-GGUF")
        .or_else(|| repo.strip_suffix("-gguf"))?;
    Some(format!("{stem}-MTP-GGUF"))
}

/// Every quantisation a repository publishes, and the same ones with the prediction head where a
/// sibling publishes them.
///
/// ## Why both are one list
///
/// The head is weights. Pick the plain file and `--spec-type draft-mtp` is unavailable to that
/// model for ever — not a setting anybody can turn on later, and nothing on the screen said so.
/// Two repositories with identical filenames is Hugging Face's arrangement, not a choice Epoch
/// can make on somebody's behalf; what Epoch can do is put them in front of the person choosing,
/// with what each costs.
///
/// **A missing sibling is silence, never a failure.** Most repositories have none, and asking is
/// one request that is allowed to come back empty.
pub fn variants_with_head(repo: &str) -> Result<Vec<Group>, String> {
    let mut groups = variants(repo)?;
    let Some(sibling) = mtp_sibling(repo) else {
        return Ok(groups);
    };
    let Ok(theirs) = variants(&sibling) else {
        return Ok(groups);
    };
    for group in theirs {
        let bits = group.bits;
        let mut carrying: Vec<Variant> = group
            .variants
            .into_iter()
            // The module inside that repository is still a module; only whole quantisations are
            // an alternative to a whole quantisation.
            .filter(|it| it.tag.is_none())
            .map(|it| Variant {
                own_head: true,
                ..it
            })
            .collect();
        if carrying.is_empty() {
            continue;
        }
        match groups.iter_mut().find(|it| it.bits == bits) {
            Some(here) => here.variants.append(&mut carrying),
            None => groups.push(Group {
                bits,
                variants: carrying,
            }),
        }
    }
    groups.sort_by_key(|it| it.bits);
    Ok(groups)
}

pub fn variants(repo: &str) -> Result<Vec<Group>, String> {
    #[derive(Deserialize)]
    struct Sibling {
        rfilename: String,
        #[serde(default)]
        size: u64,
    }
    #[derive(Deserialize)]
    struct Record {
        #[serde(default)]
        siblings: Vec<Sibling>,
    }

    let record: Record = ureq::get(&format!("{HUGGING_FACE}/{repo}?blobs=true"))
        .timeout(std::time::Duration::from_secs(20))
        .call()
        .map_err(|err| format!("Hugging Face has no repository '{repo}': {err}"))?
        .into_json()
        .map_err(|err| format!("the record for '{repo}' is unreadable: {err}"))?;

    // Keyed by tag and name together, so the MTP module and the ordinary Q4_0 do not merge.
    let mut found: std::collections::BTreeMap<(Option<String>, String), (u64, usize)> =
        std::collections::BTreeMap::new();

    for file in record.siblings {
        if !file.rfilename.to_lowercase().ends_with(".gguf") {
            continue;
        }
        let Some((quant, tag)) = read_quant(&file.rfilename) else {
            continue;
        };
        let entry = found.entry((tag, quant)).or_insert((0, 0));
        entry.0 += file.size;
        entry.1 += 1;
    }

    if found.is_empty() {
        return Err(format!(
            "'{repo}' publishes no GGUF files, so Ollama cannot pull it"
        ));
    }

    let mut groups: std::collections::BTreeMap<u8, Vec<Variant>> =
        std::collections::BTreeMap::new();
    for ((tag, quant), (bytes, files)) in found {
        groups.entry(bits_of(&quant)).or_default().push(Variant {
            pull: format!("hf.co/{repo}:{quant}"),
            quant,
            bytes,
            // One file is one thing Ollama can take; several is a shard set, and it refuses
            // those by name.
            pullable: files == 1,
            // Set by `variants_with_head` for the rows it merges in; a repository asked about
            // directly answers for itself.
            own_head: false,
            files,
            tag,
        });
    }

    Ok(groups
        .into_iter()
        .map(|(bits, mut variants)| {
            // Smallest first inside a row: it is the one most likely to fit, and a row read
            // left to right then goes from cheapest to dearest.
            variants.sort_by_key(|v| v.bytes);
            Group { bits, variants }
        })
        .collect())
}

/// The quantisation a filename names, and whether it is a tagged extra rather than the model.
///
/// Read off the name rather than matched against a known set: this vocabulary grows (`IQ4_NL`,
/// `UD-Q4_K_XL`), and a list of the ones that existed when this was written would go quietly
/// wrong the first time somebody published a new one.
fn read_quant(path: &str) -> Option<(String, Option<String>)> {
    let (dir, file) = match path.rsplit_once('/') {
        Some((dir, file)) => (Some(dir), file),
        None => (None, path),
    };
    let stem = file
        .strip_suffix(".gguf")
        .or_else(|| file.strip_suffix(".GGUF"))?;
    // Shards: `-00001-of-00002`. Dropped so the parts of one variant add up to one variant.
    let stem = match stem.rsplit_once("-of-") {
        Some((head, tail)) if tail.chars().all(|c| c.is_ascii_digit()) => {
            head.rsplit_once('-').map(|(h, _)| h).unwrap_or(head)
        }
        _ => stem,
    };

    // The quantisation is the last dash-separated run that looks like one.
    let last = stem.rsplit('-').next()?;
    let quant = if is_a_width(last) {
        // `UD-Q4_K_XL` and `mtp-Q4_0` carry a prefix that belongs to the name.
        match stem.rsplit_once('-') {
            Some((head, _)) if head.ends_with("UD") => format!("UD-{last}"),
            _ => last.to_owned(),
        }
    } else {
        return None;
    };

    // MTP is published beside the model, in its own folder or with its own prefix. It is a
    // module rather than a version of the model, and it is a tenth of the size — offering it as
    // one would be the list lying about what a 1.4 GB download gets you.
    let tag = (dir.is_some_and(|d| d.eq_ignore_ascii_case("MTP"))
        || stem.to_lowercase().contains("mtp-"))
    .then(|| "MTP".to_owned());

    Some((quant, tag))
}

/// Whether a word names a quantisation width.
fn is_a_width(word: &str) -> bool {
    let upper = word.to_uppercase();
    if upper == "BF16" || upper == "F16" || upper == "F32" {
        return true;
    }
    let rest = upper
        .strip_prefix("IQ")
        .or_else(|| upper.strip_prefix('Q'))
        .unwrap_or("");
    rest.starts_with(|c: char| c.is_ascii_digit())
}

/// How many bits a quantisation name claims. `0` when it claims nothing.
fn bits_of(quant: &str) -> u8 {
    let upper = quant.to_uppercase();
    if upper.ends_with("BF16") || upper.ends_with("F16") {
        return 16;
    }
    if upper.ends_with("F32") {
        return 32;
    }
    let after = upper
        .rsplit_once("IQ")
        .map(|(_, rest)| rest)
        .or_else(|| upper.rsplit_once('Q').map(|(_, rest)| rest))
        .unwrap_or("");
    after
        .chars()
        .next()
        .and_then(|c| c.to_digit(10))
        .unwrap_or(0) as u8
}

/// One model that **fits this machine**, with the largest quantisation that does.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suited {
    /// Its place in this list, from 1. The list's order, said out loud so a surface does not have
    /// to count — and so what the number means can be written next to it.
    pub rank: usize,
    /// The repository, as Hugging Face spells it — or the model's own name when it is already
    /// here.
    pub repo: String,
    /// Which quantisation fits: `UD-Q4_K_XL`, `Q5_K_M`. Empty for one already installed, whose
    /// quantisation is whatever it was pulled at.
    pub quant: String,
    pub bytes: u64,
    /// What to type to get exactly this one. Empty for one that is already here.
    pub pull: String,
    /// What the repository declared it can do, in its own words.
    pub facets: Vec<Facet>,
    /// The runtime this is already installed on, when it is. `None` means it would be downloaded.
    pub here: Option<String>,
    /// Tokens per second: **measured** when `measured_on` says where, and an estimate otherwise.
    ///
    /// `None` is the honest state on a machine that has measured nothing: there is no way to say
    /// how fast a model would answer here without one real answer to divide by.
    pub tokens_per_second: Option<f64>,
    /// Which runtime the number came from. `None` means nobody ran it and the number above is an
    /// estimate from this machine's own measured throughput.
    pub measured_on: Option<String>,
}

/// A model this machine already holds, as the runtime that holds it reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    pub name: String,
    /// `ollama`, `llama_cpp`, `lm_studio`.
    pub runtime: String,
    pub bytes: u64,
}

/// How much video memory is left for the conversation rather than the weights.
///
/// A model that fills the card exactly leaves nowhere for the context, and a turn that cannot
/// allocate its KV cache does not run slowly — it fails. An allowance rather than a measurement,
/// and small enough to say so: 1.5 GB is a few thousand tokens on a model this size.
const FOR_THE_CONVERSATION: u64 = 1_500_000_000;

/// What this machine can actually run, most used first.
///
/// ## What is measured and what is borrowed
///
/// **Epoch measures whether it fits**: the card's free memory against the real byte count of a
/// quantisation, summed across shards. That half is a fact.
///
/// **The order is Hugging Face's own** — its `sort=downloads`. Epoch has no way to measure
/// whether a model is *good*, and a list ordered by something it invented would be the gauge
/// nobody can explain, with a download button under it. So the ordering is borrowed, named as
/// borrowed, and never called a ranking of quality.
///
/// ## The largest that fits, not the smallest
///
/// For one model a bigger quantisation is closer to the original, so the best answer for a card
/// is the largest one it can hold. That is a rule about numbers rather than a preference.
///
/// ## Why it costs a moment
///
/// A repository is not a size: `unsloth/Qwen3.8-27B-GGUF` publishes 27 quantisations from 6.19 GB
/// to 55.6 GB, and only asking about each one answers *will it run here*. So this reads the
/// download list once and then asks about each repository in turn, which is one request each —
/// deliberately behind a button rather than on a deck that opens.
pub fn suited_to(
    free_vram: u64,
    // What this machine already holds. These are listed too, and never offered for download —
    // the question is *which model should I use*, and half the answer is usually already here.
    installed: &[Installed],
    // What has been timed on this machine, which is what turns a size into a speed.
    speeds: &crate::speeds::Speeds,
    want: usize,
    look_at: usize,
) -> Result<Vec<Suited>, String> {
    let budget = free_vram.saturating_sub(FOR_THE_CONVERSATION);
    if budget == 0 {
        return Ok(Vec::new());
    }

    // **Asked of the source, not filtered afterwards.** `mxbai-embed-large-v1` is one of the most
    // downloaded GGUF repositories there is, it fits any card, and it cannot hold a conversation
    // — it turns text into vectors. Hugging Face publishes what a model is *for*, so the question
    // is put to it rather than guessed at from a name.
    let found = a_page(&format!(
        "{HUGGING_FACE}?filter=gguf&pipeline_tag=text-generation&sort=downloads&direction=-1&limit={PAGE}"
    ))?;
    let mut suited = Vec::new();

    // **What is already here comes first**, and not because Epoch judges it better.
    //
    // The question is *which model should I use on this machine*, and for somebody who has
    // already downloaded five, most of the answer is on their disk. A list that only offered
    // downloads would be answering a different question — and the fastest thing here is often
    // something they already have.
    //
    // Largest first among them, for the same reason the largest quantisation wins below: more
    // model on the same card.
    let mut here: Vec<&Installed> = installed
        .iter()
        .filter(|one| one.bytes > 0 && one.bytes <= budget)
        .collect();
    here.sort_by_key(|one| std::cmp::Reverse(one.bytes));

    // **One model, however many shelves it is on.** IMPORT hands Ollama a GGUF the Workshop
    // saved, and Ollama files its own copy — so the same weights are on the disk twice, under
    // two names, and this list showed them as two candidates. It answers *which model should I
    // use*, and the same model is not two answers.
    //
    // Same byte count **and** the same name to begin with: the size alone is a coincidence
    // waiting to happen, and the prefix alone would fold two quantisations of one model into
    // one row when they are genuinely different things to choose between.
    let mut folded: Vec<(&Installed, Vec<String>)> = Vec::new();
    for one in here {
        match folded.iter_mut().find(|(kept, _)| {
            kept.bytes == one.bytes && crate::runtimes::arrived_together(&kept.name, &one.name)
        }) {
            Some((_, shelves)) => {
                if !shelves.contains(&one.runtime) {
                    shelves.push(one.runtime.clone());
                }
            }
            None => folded.push((one, vec![one.runtime.clone()])),
        }
    }

    for (one, shelves) in folded {
        let timed = speeds.about(&one.name).into_iter().next();
        suited.push(Suited {
            rank: 0,
            repo: one.name.clone(),
            quant: String::new(),
            bytes: one.bytes,
            pull: String::new(),
            facets: Vec::new(),
            // Every shelf it is on, because *where* it is decides what can open it — and two
            // shelves is also the only thing on screen that says the disk is holding it twice.
            here: Some(shelves.join(" + ")),
            tokens_per_second: timed
                .map(|it| it.tokens_per_second)
                .or_else(|| speeds.estimate(one.bytes, true)),
            measured_on: timed.map(|it| it.runtime.clone()),
        });
    }
    for offer in found.offers.into_iter().take(look_at) {
        if suited.len() >= want {
            break;
        }
        let Ok(groups) = variants(&offer.name) else {
            // A repository that will not answer is skipped rather than guessed at. It is one row
            // of a list, and inventing a size for it is the one thing this must not do.
            continue;
        };
        let all: Vec<Variant> = groups
            .into_iter()
            .flat_map(|group| group.variants)
            .collect();

        // **What the biggest file in this repository weighs**, which is what makes the next rule
        // a comparison rather than a guess.
        let heaviest = all.iter().map(|variant| variant.bytes).max().unwrap_or(0);

        let best = all
            .into_iter()
            // A label the repository attached rather than a quantisation of the model — the MTP
            // module and friends. Offering one as though it were a small version of a 27B model
            // is the list lying about what it holds.
            .filter(|variant| variant.tag.is_none())
            .filter(|variant| variant.pullable)
            // **And the same lie without a label on it.** Watched in the first run of this list:
            // a 27B repository offered `F32 · 1.8 GB`, which is not a small quantisation of that
            // model — it is a piece of it, published beside the model under a name Epoch's tag
            // detection does not recognise.
            //
            // Decided by comparison inside the repository rather than by reading its name
            // (ADR-0024). A real quantisation ladder runs from full precision to about two bits,
            // which is roughly eight-fold; anything under a tenth of the largest file here is
            // not the same model at a smaller width.
            .filter(|variant| heaviest == 0 || variant.bytes * 10 >= heaviest)
            .filter(|variant| variant.bytes > 0 && variant.bytes <= budget)
            .max_by_key(|variant| variant.bytes);

        if let Some(variant) = best {
            // Already here under another name is still already here: an installed model and its
            // repository are the same weights, and offering a download for something on the disk
            // would be the list not knowing what the machine holds.
            if already_here(installed, &offer.name) {
                continue;
            }
            suited.push(Suited {
                rank: 0,
                repo: offer.name.clone(),
                bytes: variant.bytes,
                // Nobody has run this one here, so anything said about its speed is this
                // machine's own throughput divided by its size — and `None` when nothing has
                // been measured yet.
                tokens_per_second: speeds.estimate(variant.bytes, true),
                measured_on: None,
                quant: variant.quant,
                pull: variant.pull,
                facets: offer.facets,
                here: None,
            });
        }
    }

    // The place in the list, said rather than counted by whoever renders it.
    for (at, one) in suited.iter_mut().enumerate() {
        one.rank = at + 1;
    }
    Ok(suited)
}

/// Search Hugging Face for models Ollama can actually pull.
///
/// **GGUF only**, because that is what `ollama pull hf.co/…` takes. Offering a repository of
/// safetensors would be offering a download that cannot be run, which is worse than offering
/// nothing.
///
/// `wanted` narrows to what a model **declares**. Hugging Face describes vision, image and audio
/// through `pipeline_tag` and says nothing about tools or reasoning — so asking for those here
/// returns nothing rather than guessing, and the surface says which source can answer them.
/// One page of results, and how to ask for the next.
///
/// Hugging Face hosts tens of thousands of GGUF repositories and returns them a page at a time.
/// A single fixed page was showing forty and implying that was all there is -- which is the same
/// failure as a cold instrument reading zero: not wrong about what it shows, wrong about what it
/// suggests.
pub struct Found {
    pub offers: Vec<Offer>,
    /// The opaque cursor for the next page, when there is one.
    ///
    /// **Hugging Face pages by cursor, not by number** (measured: the answer carries
    /// `Link: <...&cursor=...>; rel="next"`), so a page cannot be jumped to and "page 7" is not a
    /// thing that exists. Passed back verbatim rather than parsed -- it encodes a sort position
    /// and a search token, and anything that took it apart would be guessing at somebody else's
    /// private format.
    pub more: Option<String>,
}

/// Search Hugging Face for models Ollama can actually pull.
///
/// **GGUF only**, because that is what `ollama pull hf.co/...` takes. Offering a repository of
/// safetensors would be offering a download that cannot be run, which is worse than offering
/// nothing.
///
/// `wanted` narrows to what a model **declares**. Hugging Face describes vision, image and audio
/// through `pipeline_tag` and says nothing about tools or reasoning -- so asking for those here
/// returns nothing rather than guessing, and the surface says which source can answer them.
/// One page of Hugging Face's own listing, with no filters applied to it.
///
/// Split out so [`suited_to`] can ask a question the facet filters cannot express — *what is this
/// model for* — without a second way of talking to the same endpoint.
fn a_page(url: &str) -> Result<Found, String> {
    read_a_page(url, &[], &[])
}

pub fn search(query: &str, wanted: &[Facet], installed: &[String]) -> Result<Found, String> {
    let query = query.trim();
    let mut url = format!("{HUGGING_FACE}?filter=gguf&sort=downloads&direction=-1&limit={PAGE}");
    if !query.is_empty() {
        url.push_str("&search=");
        url.push_str(&urlencode(query));
    }
    // The one facet Hugging Face can be *asked* about rather than filtered on afterwards.
    if let Some(pipeline) = wanted.iter().find_map(pipeline_for) {
        url.push_str("&pipeline_tag=");
        url.push_str(pipeline);
    }
    read_a_page(&url, wanted, installed)
}

/// The next page of a search already run.
///
/// The cursor carries the query, the sort and the position, so nothing else needs repeating --
/// which is also why the filters are passed again: they are applied *here*, to what a model
/// declared, and the cursor knows nothing about them.
pub fn search_more(cursor: &str, wanted: &[Facet], installed: &[String]) -> Result<Found, String> {
    read_a_page(cursor, wanted, installed)
}

/// How many repositories one request asks for.
///
/// Larger than a screenful on purpose: the filters below are applied after the answer arrives,
/// so a page of ten could come back empty and look like "nothing matches" when it means "not on
/// this page".
const PAGE: usize = 60;

fn read_a_page(url: &str, wanted: &[Facet], installed: &[String]) -> Result<Found, String> {
    let answer = ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|err| format!("could not search Hugging Face: {err}"))?;

    // Read before the body, which consumes the response.
    let more = next_page(answer.header("link"));

    let found: Vec<HfModel> = answer
        .into_json()
        .map_err(|err| format!("Hugging Face answered something unreadable: {err}"))?;

    Ok(Found {
        offers: found
            .into_iter()
            .map(|model| {
                let facets = facets_of(&model);
                Offer {
                    // A search result is a repository rather than one file. Which of its
                    // variants Ollama can take is answered when one is chosen, not here.
                    pullable: true,
                    installed: installed
                        .iter()
                        .any(|have| family(have) == family(&model.id)),
                    // What `ollama pull` takes for a Hugging Face repository.
                    pull: format!("hf.co/{}", model.id),
                    name: model.id,
                    bytes: None,
                    fits: None,
                    source: "huggingface",
                    described: !facets.is_empty(),
                    facets,
                }
            })
            // Everything asked for, and only what the model itself declared.
            .filter(|offer| wanted.iter().all(|want| offer.facets.contains(want)))
            .collect(),
        more,
    })
}

/// The `rel="next"` URL out of a `Link` header, if there is one.
///
/// Written rather than pulled in as a dependency: this is one header with one relation, and the
/// general parser handles quoting rules and multiple relations that this endpoint never sends.
fn next_page(header: Option<&str>) -> Option<String> {
    let header = header?;
    for part in header.split(',') {
        let (link, rel) = part.split_once(';')?;
        if !rel.contains("next") {
            continue;
        }
        let link = link.trim().trim_start_matches('<').trim_end_matches('>');
        if !link.is_empty() {
            return Some(link.to_owned());
        }
    }
    None
}

/// What a Hugging Face model says about itself, in Epoch's words.
///
/// Only what is declared. A `text-generation` model is not marked as *tools* because Hugging
/// Face does not say so, and this is not the place to decide it.
fn facets_of(model: &HfModel) -> Vec<Facet> {
    let mut facets = Vec::new();
    match model.pipeline_tag.as_deref().unwrap_or_default() {
        "image-text-to-text" | "visual-question-answering" | "image-to-text" => {
            facets.push(Facet::Vision)
        }
        "text-to-image" | "image-to-image" => facets.push(Facet::Image),
        "automatic-speech-recognition" | "text-to-speech" | "audio-text-to-text" => {
            facets.push(Facet::Audio)
        }
        _ => {}
    }
    facets
}

/// The pipeline Hugging Face can be asked for directly, when one is wanted.
fn pipeline_for(facet: &Facet) -> Option<&'static str> {
    match facet {
        Facet::Vision => Some("image-text-to-text"),
        Facet::Image => Some("text-to-image"),
        Facet::Audio => Some("automatic-speech-recognition"),
        // Hugging Face has no field for either, so asking would narrow to nothing and read as
        // "no model does this".
        Facet::Tools | Facet::Thinking => None,
    }
}

/// Percent-encode a query. Small on purpose: this escapes a search box, not a URL library.
fn urlencode(raw: &str) -> String {
    raw.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (b as char).to_string()
            }
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct HfModel {
    id: String,
    #[serde(default)]
    pipeline_tag: Option<String>,
}

/// The part before the tag. `gemma4:12b` and `gemma4` are the same family.
fn family(name: &str) -> &str {
    name.split_once(':').map(|(head, _)| head).unwrap_or(name)
}

/// Whether something on the disk is the model a repository is offering.
///
/// ## Why a tag comparison was not enough
///
/// [`family`] strips an Ollama tag, so `qwen3:14b` and `qwen3` are one thing. That covers every
/// model Ollama pulled and nothing else — and a GGUF the Workshop saved is a *file*, named after
/// what `hf download` fetched rather than after the repository it came from:
///
/// ```text
/// on the disk   Qwen3.8-27B-Uncensored-IQ2_M
/// the offer     JonathanColetti/Qwen3.8-27B-Uncensored-GGUF
/// ```
///
/// Those never match, so the list offered a DOWNLOAD for ten gigabytes already on the machine —
/// reported by the owner, looking at exactly that row.
///
/// ## The comparison, and which way it is allowed to be wrong
///
/// The repository's own half after the slash, against the file's name, by shared prefix — the
/// same rule the vision projector uses, so there is one spelling of *these arrived as a set*.
///
/// A filename is a claim (ADR-0024) and this is a place where a claim is the right tool: it is
/// deciding whether to *hide a download button*, not what a model is. Wrong one way costs a row
/// somebody can still find by searching; wrong the other way is what is on screen now.
fn already_here(installed: &[Installed], offer: &str) -> bool {
    let repo = offer.rsplit('/').next().unwrap_or(offer);
    installed.iter().any(|had| {
        family(&had.name) == family(offer) || crate::runtimes::arrived_together(&had.name, repo)
    })
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default)]
    layers: Vec<Layer>,
}

#[derive(Debug, Deserialize)]
struct Layer {
    size: u64,
}

#[cfg(test)]
mod paging {
    use super::*;

    /// A model saved as a file is still a model this machine has.
    ///
    /// Reported by the owner from the real list: row 5 offered a DOWNLOAD for
    /// `Qwen3.8-27B-Uncensored-GGUF` with 10.6 GB of it already on the disk. Two things were
    /// wrong — Ollama was the only shelf being asked, and the comparison could only ever match
    /// an Ollama tag.
    #[test]
    fn a_saved_gguf_is_not_offered_back_as_a_download() {
        let held = vec![Installed {
            name: "Qwen3.8-27B-Uncensored-IQ2_M".to_owned(),
            runtime: "Saved here".to_owned(),
            bytes: 10_624_771_968,
        }];
        assert!(
            already_here(&held, "JonathanColetti/Qwen3.8-27B-Uncensored-GGUF"),
            "the file and the repository are the same weights"
        );
    }

    /// IMPORT puts the same weights on two shelves, and this list must still hold one answer.
    ///
    /// Confirmed with the owner: they pressed IMPORT, so Ollama filed its own copy of a GGUF
    /// already in the vault. Both are real, both are usable, and *which model should I use* has
    /// one answer either way.
    #[test]
    fn one_model_on_two_shelves_is_one_row_that_names_both() {
        let held = [
            Installed {
                name: "Qwen3.8-27B-Uncensored-IQ2-M:latest".to_owned(),
                runtime: "Ollama".to_owned(),
                bytes: 10_624_771_968,
            },
            Installed {
                name: "Qwen3.8-27B-Uncensored-IQ2_M".to_owned(),
                runtime: "Saved here".to_owned(),
                bytes: 10_624_771_968,
            },
        ];
        let one = &held[0];
        let other = &held[1];
        assert_eq!(one.bytes, other.bytes);
        assert!(
            crate::runtimes::arrived_together(&one.name, &other.name),
            "the same weights under two names"
        );

        // A different quantisation of the same model stays two answers: it is genuinely a
        // choice, which is why the size has to agree as well as the name.
        let smaller = Installed {
            name: "Qwen3.8-27B-Uncensored-IQ4_XS".to_owned(),
            runtime: "Saved here".to_owned(),
            bytes: 15_310_000_000,
        };
        assert_ne!(smaller.bytes, one.bytes);
    }

    /// The Ollama case it always handled, still handled.
    #[test]
    fn a_pulled_model_is_recognised_under_its_tag() {
        let held = vec![Installed {
            name: "qwen3:14b".to_owned(),
            runtime: "Ollama".to_owned(),
            bytes: 9_300_000_000,
        }];
        assert!(already_here(&held, "qwen3"));
    }

    /// And it does not hide a row for something this machine does not have.
    ///
    /// The matching is by shared prefix, so the case worth holding still is two models whose
    /// names begin the same way and are not the same model.
    #[test]
    fn a_different_model_is_still_offered() {
        let held = vec![Installed {
            name: "Qwen3.8-27B-Uncensored-IQ2_M".to_owned(),
            runtime: "Saved here".to_owned(),
            bytes: 10_624_771_968,
        }];
        assert!(
            !already_here(&held, "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"),
            "a different model, whatever its name starts with"
        );
        assert!(!already_here(&held, "google/gemma-4-12b-GGUF"));
        assert!(!already_here(&[], "anything/at-all-GGUF"));
    }

    #[test]
    fn the_next_page_is_read_from_the_link_header() {
        // Measured: Hugging Face answers with `Link: <...&cursor=...>; rel="next"`. It pages by
        // cursor rather than by number, which is why there is a MORE button and not a page seven.
        let header =
            "<https://huggingface.co/api/models?filter=gguf&cursor=eyJhIjoxfQ>; rel=\"next\"";
        assert_eq!(
            next_page(Some(header)).as_deref(),
            Some("https://huggingface.co/api/models?filter=gguf&cursor=eyJhIjoxfQ")
        );
    }

    #[test]
    fn the_last_page_says_so_rather_than_looping() {
        // No header at all is the end of the list, and it is a different fact from "we asked for
        // forty and stopped" — which is the whole reason the button knows when to disappear.
        assert_eq!(next_page(None), None);
        assert_eq!(next_page(Some("")), None);
        // A `Link` that only offers something else is also not a next page.
        assert_eq!(
            next_page(Some("<https://example.com/first>; rel=\"first\"")),
            None
        );
    }

    #[test]
    fn the_cursor_is_passed_back_whole_rather_than_taken_apart() {
        // It encodes a sort position and a search token that belong to Hugging Face. Anything
        // here that parsed it would be guessing at somebody else's private format, and would
        // break the day they change it — silently, by returning the same page forever.
        let cursor = "https://huggingface.co/api/models?filter=gguf&cursor=eyIkb3IiOlt7ImRvd25s";
        assert_eq!(
            next_page(Some(&format!("<{cursor}>; rel=\"next\""))).as_deref(),
            Some(cursor)
        );
    }

    #[test]
    #[ignore = "reaches Hugging Face"]
    fn there_is_always_more_than_one_page() {
        // The defect this closes: forty results were shown as though they were the library.
        let first = search("qwen", &[], &[]).expect("searches");
        assert!(!first.offers.is_empty());
        let cursor = first
            .more
            .expect("there is more than one page of GGUF qwen models");

        let second = search_more(&cursor, &[], &[]).expect("continues");
        assert!(!second.offers.is_empty(), "the second page has results");
        // And it is genuinely a different page rather than the same one again.
        let repeated = second
            .offers
            .iter()
            .any(|offer| first.offers.iter().any(|had| had.name == offer.name));
        assert!(!repeated, "the second page repeats the first");
    }

    #[test]
    #[ignore = "reaches Hugging Face"]
    fn a_weighed_repository_can_be_downloaded_by_hand() {
        // llama.cpp and LM Studio take a GGUF from disk and have no endpoint that installs one,
        // so the address of the exact file Epoch weighed is the useful thing to hand them.
        let offer = Offer {
            pullable: true,
            name: "unsloth/Qwen3-8B-GGUF".into(),
            pull: "hf.co/unsloth/Qwen3-8B-GGUF:Q4_K_M".into(),
            installed: false,
            bytes: None,
            fits: None,
            source: "huggingface",
            facets: Vec::new(),
            described: false,
        };
        let url = file_url(&offer).expect("the file exists");
        assert!(
            url.starts_with("https://huggingface.co/unsloth/Qwen3-8B-GGUF/resolve/main/"),
            "{url}"
        );
        assert!(url.to_lowercase().ends_with(".gguf"), "{url}");
    }

    #[test]
    fn an_ollama_name_has_no_file_to_hand_anybody() {
        // Ollama's registry serves layers by digest rather than a file somebody can download and
        // open, so there is no honest URL — and inventing one would send a person to a 404.
        let offer = Offer {
            pullable: true,
            name: "gemma4:12b".into(),
            pull: "gemma4:12b".into(),
            installed: false,
            bytes: None,
            fits: None,
            source: "featured",
            facets: Vec::new(),
            described: false,
        };
        assert_eq!(file_url(&offer), None);
    }
}

#[cfg(test)]
mod variant_tests {
    use super::*;

    #[test]
    fn the_width_is_read_off_the_name() {
        assert_eq!(bits_of("UD-IQ1_S"), 1);
        assert_eq!(bits_of("UD-Q2_K_XL"), 2);
        assert_eq!(bits_of("Q4_0"), 4);
        assert_eq!(bits_of("UD-Q4_K_XL"), 4);
        assert_eq!(bits_of("Q8_0"), 8);
        assert_eq!(bits_of("BF16"), 16);
        assert_eq!(bits_of("F16"), 16);
        // A name this build has never seen claims nothing rather than guessing.
        assert_eq!(bits_of("SOMETHING_NEW"), 0);
    }

    #[test]
    fn a_shard_is_part_of_a_variant_rather_than_one() {
        // Measured: `BF16` is published as `-00001-of-00002` and `-00002-of-00002`. Either on
        // its own is a download nobody can run, and the larger one is wrong by 5 GB.
        let (quant, tag) = read_quant("BF16/Qwen3.8-27B-BF16-00001-of-00002.gguf").expect("reads");
        assert_eq!(quant, "BF16");
        assert_eq!(tag, None);
        let (again, _) = read_quant("BF16/Qwen3.8-27B-BF16-00002-of-00002.gguf").expect("reads");
        assert_eq!(
            again, quant,
            "both parts name the same variant, so they add up"
        );
    }

    #[test]
    fn the_mtp_module_is_tagged_rather_than_offered_as_a_tiny_model() {
        // It is 1.37 GB beside a 27B model. Listing it as a version of that model would tell
        // somebody a 27B fits in a gigabyte and a half.
        let (quant, tag) = read_quant("MTP/mtp-Qwen3.8-27B-Q4_0.gguf").expect("reads");
        assert_eq!(quant, "Q4_0");
        assert_eq!(tag.as_deref(), Some("MTP"));

        // And the ordinary Q4_0 beside it is not tagged, so the two do not merge.
        let (plain, none) = read_quant("Qwen3.8-27B-Q4_0.gguf").expect("reads");
        assert_eq!(plain, "Q4_0");
        assert_eq!(none, None);
    }

    #[test]
    fn a_prefixed_name_keeps_its_prefix() {
        // `UD-Q4_K_XL` and `Q4_K_XL` are different files with different sizes.
        assert_eq!(
            read_quant("Qwen3.8-27B-UD-Q4_K_XL.gguf").unwrap().0,
            "UD-Q4_K_XL"
        );
        assert_eq!(
            read_quant("Qwen3.8-27B-UD-IQ1_M.gguf").unwrap().0,
            "UD-IQ1_M"
        );
    }

    #[test]
    fn something_that_is_not_a_quantisation_is_left_out() {
        assert_eq!(read_quant("README.md"), None);
        assert_eq!(read_quant("Qwen3.8-27B-mmproj.gguf"), None);
    }

    #[test]
    #[ignore = "reaches Hugging Face"]
    fn a_real_repository_lists_what_it_really_publishes() {
        // Against the repository in the owner's screenshot. The expected numbers are the ones
        // Hugging Face's own page shows when the list is expanded.
        let groups = variants("unsloth/Qwen3.8-27B-GGUF").expect("the repository exists");
        let by_bits: std::collections::BTreeMap<u8, usize> =
            groups.iter().map(|g| (g.bits, g.variants.len())).collect();
        println!("{by_bits:?}");

        assert!(by_bits.contains_key(&1), "it publishes 1-bit quantisations");
        assert!(by_bits.contains_key(&16), "and a 16-bit one");

        // The MTP module is published as a `Q4_0`, beside the real `Q4_0`, and it is a tenth of
        // the size. Both appear — Hugging Face shows both — and the tag is what keeps somebody
        // from reading 1.4 GB as the cheapest way to run a 27B model.
        //
        // (This assertion first said the MTP file was the *smallest* in the repository. It is
        // not: there is an `F16` of 0.93 GB, which is also not the model and which Hugging Face
        // also lists untagged. The code matched their page; the assertion was the guess.)
        let four_bit = groups.iter().find(|g| g.bits == 4).expect("4-bit exists");
        let tagged: Vec<&Variant> = four_bit
            .variants
            .iter()
            .filter(|v| v.tag.as_deref() == Some("MTP"))
            .collect();
        assert_eq!(tagged.len(), 1, "{four_bit:?}");
        assert!(
            four_bit
                .variants
                .iter()
                .any(|v| v.quant == "Q4_0" && v.tag.is_none()),
            "the real Q4_0 is there too, and did not merge with the module"
        );

        let bf16 = groups
            .iter()
            .flat_map(|g| &g.variants)
            .find(|v| v.quant == "BF16")
            .expect("BF16 is published");
        assert!(bf16.files > 1, "it arrives in parts: {bf16:?}");
        assert!(
            (50_000_000_000..60_000_000_000).contains(&bf16.bytes),
            "the parts are summed: {} bytes",
            bf16.bytes
        );
    }
}

#[cfg(test)]
mod what_fits_here {
    use super::*;

    /// The one thing this must never do: offer something that does not fit.
    ///
    /// Not `#[ignore]`d, and it reaches the network on purpose — the rule under test is about
    /// **real** file sizes against a **real** card, and a fixture would be testing a fixture. It
    /// passes on a machine with no network too: an unreachable Hugging Face is an error, and an
    /// error is not an offer.
    #[test]
    fn nothing_offered_is_bigger_than_the_card() {
        // Twelve gigabytes, which is the card this was written against.
        let card = 12_000_000_000;
        let Ok(found) = suited_to(card, &[], &crate::speeds::Speeds::default(), 5, 8) else {
            return;
        };
        let budget = card - FOR_THE_CONVERSATION;
        for one in &found {
            assert!(
                one.bytes <= budget,
                "{} at {} bytes was offered for a {budget} byte budget",
                one.repo,
                one.bytes
            );
            assert!(
                one.bytes > 0,
                "{} was offered with no measured size",
                one.repo
            );
        }
    }

    /// What is already here is listed, and never offered for download.
    #[test]
    fn a_model_on_the_disk_is_a_row_rather_than_a_download() {
        // The question is *which model should I use on this machine*, and for somebody who has
        // already downloaded five, most of the answer is on their disk.
        let card = 12_000_000_000;
        let mine = vec![
            Installed {
                name: "gemma4-12b".into(),
                runtime: "llama_cpp".into(),
                bytes: 7_400_000_000,
            },
            // Too big for this card: it is not offered at all rather than offered as a bad idea.
            Installed {
                name: "qwen3.8".into(),
                runtime: "ollama".into(),
                bytes: 17_700_000_000,
            },
        ];
        let mut speeds = crate::speeds::Speeds::default();
        speeds.remember(crate::speeds::Measured {
            model: "gemma4-12b".into(),
            runtime: "llama_cpp".into(),
            bytes: 7_400_000_000,
            tokens_per_second: 46.3,
            fitted: true,
            at: 1,
        });

        let Ok(found) = suited_to(card, &mine, &speeds, 4, 6) else {
            return;
        };
        let first = found.first().expect("something fits");
        assert_eq!(first.rank, 1, "the place in the list is said, not counted");
        assert_eq!(first.repo, "gemma4-12b");
        assert_eq!(first.here.as_deref(), Some("llama_cpp"));
        assert!(first.pull.is_empty(), "nothing on the disk is a download");

        // **Measured, and it says where from.** An estimate would carry no runtime.
        assert_eq!(first.measured_on.as_deref(), Some("llama_cpp"));
        assert_eq!(first.tokens_per_second, Some(46.3));

        // The one that does not fit is absent, not listed with a warning.
        assert!(!found.iter().any(|one| one.repo == "qwen3.8"));

        // And everything else got an estimate from this machine's own answer, with no runtime
        // attached — which is what tells the two apart.
        for one in found.iter().filter(|one| one.here.is_none()) {
            assert!(one.measured_on.is_none());
            assert!(
                one.tokens_per_second.is_some(),
                "{} has no estimate",
                one.repo
            );
        }
    }

    /// A card with nothing spare offers nothing, rather than offering the smallest thing there is.
    #[test]
    fn a_card_with_no_room_is_told_so_rather_than_sold_something() {
        let nothing = crate::speeds::Speeds::default();
        assert_eq!(
            suited_to(FOR_THE_CONVERSATION, &[], &nothing, 5, 5).unwrap(),
            Vec::new()
        );
        assert_eq!(suited_to(0, &[], &nothing, 5, 5).unwrap(), Vec::new());
    }
}

#[cfg(test)]
mod weighing {
    use super::*;

    fn card(free_bytes: u64) -> machine::Machine {
        machine::Machine {
            gpu: Some("test card".into()),
            vram_total: Some(free_bytes),
            vram_free: Some(free_bytes),
            ram_total: Some(free_bytes * 2),
            unified: false,
        }
    }

    #[test]
    fn a_hugging_face_name_is_not_sent_to_ollamas_registry() {
        // The defect: every Hugging Face result could be searched and never weighed, because the
        // name was pasted into `registry.ollama.ai/v2/library/<name>` -- producing
        // `.../library/hf.co/unsloth/...`, a URL that could never have worked.
        assert_eq!(
            repository("hf.co/unsloth/Qwen3-8B-GGUF"),
            Some(("unsloth/Qwen3-8B-GGUF".to_owned(), None))
        );
        assert_eq!(
            repository("huggingface.co/unsloth/Qwen3-8B-GGUF:Q4_K_M"),
            Some((
                "unsloth/Qwen3-8B-GGUF".to_owned(),
                Some("Q4_K_M".to_owned())
            ))
        );
        // And an ordinary Ollama name still is.
        assert_eq!(repository("gemma4:12b"), None);
        assert_eq!(repository("qwen3"), None);
    }

    #[test]
    fn without_a_quantisation_it_answers_the_question_that_was_asked() {
        // A repository has no size -- it has one per quantisation, 25 of them in the measured
        // case, between 3.3 GB and 16.4 GB. "Will it run here?" only has an answer for a file,
        // so the largest that runs here is the answer, and it is named.
        let files = vec![
            ("Qwen3-8B-Q2_K.gguf".to_owned(), 3_281_733_440),
            ("Qwen3-8B-Q4_K_M.gguf".to_owned(), 5_027_782_656),
            ("Qwen3-8B-Q8_0.gguf".to_owned(), 8_710_000_000),
            ("Qwen3-8B-BF16.gguf".to_owned(), 16_388_044_384),
        ];
        let machine = card(6_000_000_000);
        let (path, _) = choose(&files, None, &machine).expect("something fits");
        assert_eq!(path, "Qwen3-8B-Q4_K_M.gguf", "the largest that runs here");
    }

    #[test]
    fn when_nothing_fits_the_smallest_is_reported_as_not_fitting() {
        // The useful answer is the cheapest option *and* that it is still too big. Returning an
        // error instead would leave somebody with no number to reason about.
        let files = vec![
            ("m-Q4_K_M.gguf".to_owned(), 5_000_000_000),
            ("m-Q8_0.gguf".to_owned(), 9_000_000_000),
        ];
        let machine = card(2_000_000_000);
        let (path, bytes) = choose(&files, None, &machine).expect("still answers");
        assert_eq!(path, "m-Q4_K_M.gguf");
        assert_eq!(machine.fits(*bytes), Some(false));
    }

    #[test]
    fn a_named_quantisation_is_weighed_even_when_it_does_not_fit() {
        // Refusing would be Epoch deciding for somebody who already decided.
        let files = vec![
            ("m-Q4_K_M.gguf".to_owned(), 5_000_000_000),
            ("m-BF16.gguf".to_owned(), 16_000_000_000),
        ];
        let machine = card(6_000_000_000);
        let (path, _) = choose(&files, Some("bf16"), &machine).expect("named is honoured");
        assert_eq!(path, "m-BF16.gguf", "and case does not matter");
    }

    #[test]
    fn a_quantisation_nobody_published_says_so_rather_than_picking_another() {
        let files = vec![("m-Q4_K_M.gguf".to_owned(), 5_000_000_000)];
        let why = choose(&files, Some("Q9_MAX"), &card(6_000_000_000)).unwrap_err();
        assert!(why.contains("Q9_MAX"), "{why}");
    }

    #[test]
    fn the_quantisation_is_read_off_the_filename_rather_than_matched_to_a_list() {
        // This vocabulary grows. A list of the ones that existed when this was written would go
        // quietly wrong the first time somebody published a new one.
        //
        // Now asserted against `read_quant`, which is the only reader left. The naive one this
        // used to check dropped `UD-` prefixes and read shard numbers as quantisations.
        let q = |p: &str| read_quant(p).map(|(quant, _)| quant);
        assert_eq!(q("Qwen3-8B-Q4_K_M.gguf").as_deref(), Some("Q4_K_M"));
        assert_eq!(q("Qwen3-8B-IQ4_NL.gguf").as_deref(), Some("IQ4_NL"));
        assert_eq!(q("dir/Qwen3-8B-BF16.gguf").as_deref(), Some("BF16"));
        // A file whose name says nothing about width is not a quantisation, and answering
        // `model` for it produced a pull string nothing could resolve.
        assert_eq!(q("model.gguf"), None);
    }

    #[test]
    fn a_missing_manifest_says_what_it_measured_rather_than_404() {
        // Measured against the real registry: `gemma4:31b` and `gpt-oss:20b` have manifests,
        // while `deepseek-v4-flash:0731`, `glm-5.1`, `kimi-k2.6` and `qwen3.5:397b` return 404
        // with no tag list at all -- Ollama's featured list names models it serves in its cloud.
        // A bare 404 reads as a broken Workshop.
        let said = NOT_DOWNLOADABLE.replace("{name}", "glm-5.1");
        assert!(said.contains("glm-5.1"));
        assert!(said.contains("cloud"));
        assert!(!said.contains("404"));
    }

    #[test]
    #[ignore = "reaches Hugging Face"]
    fn a_real_repository_weighs_a_real_file() {
        // The end-to-end claim, against the service itself. Ignored by default because a test
        // that needs the network is a test that fails on a train.
        let offer = weigh("hf.co/unsloth/Qwen3-8B-GGUF", &card(6_000_000_000))
            .expect("the repository exists");
        assert!(
            offer.bytes.unwrap_or(0) > 1_000_000_000,
            "{:?}",
            offer.bytes
        );
        assert_eq!(offer.source, "huggingface");
        // What gets pulled must be what was weighed: a repository, plus the exact quantisation.
        assert!(
            offer.pull.starts_with("hf.co/unsloth/Qwen3-8B-GGUF:"),
            "{}",
            offer.pull
        );
    }

    #[test]
    #[ignore = "reaches Ollama's registry"]
    fn a_cloud_only_featured_name_is_explained_rather_than_dumped() {
        let why = weigh("glm-5.1", &card(6_000_000_000)).unwrap_err();
        assert!(why.contains("cloud"), "{why}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repository that publishes the same quantisations with the prediction head.
    mod with_the_head {
        use super::*;

        #[test]
        fn the_sibling_is_the_name_the_convention_uses() {
            // Both of these exist. The plain one was on this machine and the MTP one was
            // downloaded beside it on 2026-08-31.
            assert_eq!(
                mtp_sibling("unsloth/Qwen3.6-35B-A3B-GGUF").as_deref(),
                Some("unsloth/Qwen3.6-35B-A3B-MTP-GGUF"),
            );
        }

        #[test]
        fn a_repository_that_already_is_one_is_not_offered_itself() {
            // Otherwise the list shows the same files twice, the second copy claiming to be an
            // upgrade on the first.
            assert_eq!(mtp_sibling("unsloth/Qwen3.6-35B-A3B-MTP-GGUF"), None);
            assert_eq!(mtp_sibling("someone/thing-MTP"), None);
        }

        #[test]
        fn a_name_the_convention_does_not_cover_is_left_alone() {
            /*
                **Silence rather than a guess.** The convention attaches to a `-GGUF` suffix, and
                a repository named anything else is one this cannot work out. Inventing a name
                would produce a request for a repository nobody published — and worse, the
                absence of an answer would then read as *this model has no MTP variant*, which is
                a claim nothing measured.
            */
            assert_eq!(mtp_sibling("bartowski/Some-Model"), None);
            assert_eq!(mtp_sibling("ggml-org/gemma-3-270m"), None);
        }

        #[test]
        fn the_case_the_repositories_actually_use_is_matched() {
            // `-gguf` lowercase happens. The suffix is matched both ways; the tag Epoch writes is
            // the upper-case one the convention publishes.
            assert_eq!(
                mtp_sibling("someone/model-gguf").as_deref(),
                Some("someone/model-MTP-GGUF"),
            );
        }
    }

    #[test]
    fn a_tag_on_disk_is_the_same_model_as_a_family_on_the_shelf() {
        // The two lists speak differently: the featured list mixes families and tags, and an
        // install is always a tag. Compared literally, somebody is offered a download they
        // already have.
        assert_eq!(family("gemma4:12b"), family("gemma4"));
        assert_eq!(family("gpt-oss:20b"), "gpt-oss");
        assert_ne!(family("gemma4"), family("gemma3"));
    }

    #[test]
    fn what_is_here_is_listed_first() {
        // The half of a shelf that is a measurement rather than an offer.
        let offers = vec![
            Offer {
                pullable: true,
                name: "zebra".into(),
                pull: "zebra".into(),
                installed: false,
                bytes: None,
                fits: None,
                source: "featured",
                facets: Vec::new(),
                described: false,
            },
            Offer {
                pullable: true,
                name: "gemma4:12b".into(),
                pull: "gemma4:12b".into(),
                installed: true,
                bytes: None,
                fits: None,
                source: "featured",
                facets: Vec::new(),
                described: false,
            },
        ];
        let mut sorted = offers;
        sorted.sort_by(|a, b| b.installed.cmp(&a.installed).then(a.name.cmp(&b.name)));
        assert_eq!(sorted[0].name, "gemma4:12b");
    }

    #[test]
    fn a_filter_narrows_to_what_was_declared_and_never_guesses() {
        // Hugging Face describes vision, image and audio through `pipeline_tag` and says nothing
        // about tools or reasoning. Asking it for those must return nothing rather than a guess
        // — and the surface says which source can answer them instead.
        assert_eq!(pipeline_for(&Facet::Vision), Some("image-text-to-text"));
        assert_eq!(pipeline_for(&Facet::Image), Some("text-to-image"));
        assert_eq!(pipeline_for(&Facet::Tools), None);
        assert_eq!(pipeline_for(&Facet::Thinking), None);
    }

    #[test]
    fn a_search_box_cannot_reach_out_of_its_query() {
        // It is a query going into a URL. Small on purpose, and tested because "small" is how
        // an escaping bug gets written.
        assert_eq!(urlencode("qwen3 coder"), "qwen3%20coder");
        assert_eq!(urlencode("a&b=c"), "a%26b%3Dc");
        assert_eq!(urlencode("Qwen3-Coder_30B.v2~x"), "Qwen3-Coder_30B.v2~x");
    }

    #[test]
    #[ignore = "reaches the network and this machine's hardware"]
    fn the_library_answers_and_a_tag_weighs_what_it_really_weighs() {
        // The measurement this module was written from, kept runnable. `gemma4:12b` is about
        // 7.4 GB in the manifest, and the local install reports 7.56 GB for the same thing —
        // while the catalogue reports its family in hundreds of gigabytes.
        let shelf = featured(&["gemma4:31b".to_string()]).expect("the featured list answers");
        println!("{} featured", shelf.len());
        assert!(!shelf.is_empty());
        assert!(
            shelf[0].installed,
            "a family this machine already has comes first"
        );

        let machine = machine::Machine::measure();
        let weighed = weigh("gemma4:12b", &machine).expect("a manifest");
        println!(
            "{} = {:?} bytes, fits {:?}",
            weighed.name, weighed.bytes, weighed.fits
        );
        let bytes = weighed.bytes.expect("a real size");
        assert!(
            (6..9).contains(&(bytes / 1_000_000_000)),
            "about 7 GB, not the family's hundreds: {bytes}"
        );
    }

    #[test]
    #[ignore = "reaches Hugging Face"]
    fn hugging_face_answers_with_models_ollama_can_actually_pull() {
        let found = search("qwen", &[], &[])
            .expect("Hugging Face answers")
            .offers;
        println!("{} results", found.len());
        assert!(!found.is_empty());
        // GGUF only — offering a repository of safetensors would be offering a download that
        // cannot be run.
        assert!(found.iter().all(|o| o.pull.starts_with("hf.co/")));
        for offer in found.iter().take(5) {
            println!("  {} {:?}", offer.pull, offer.facets);
        }

        // And a facet Hugging Face *can* be asked about narrows it rather than emptying it.
        let seeing = search("", &[Facet::Vision], &[])
            .expect("Hugging Face answers")
            .offers;
        println!("{} that declare vision", seeing.len());
        assert!(seeing.iter().all(|o| o.facets.contains(&Facet::Vision)));
    }
}

#[cfg(test)]
mod pull_strings {
    use super::*;

    /// Real filenames from `unsloth/Qwen3.8-27B-GGUF`, which is the repository the defect was
    /// reported against.
    fn published() -> Vec<(String, u64)> {
        [
            ("MTP/mtp-Qwen3.8-27B-Q4_0.gguf", 1_370_000_000u64),
            ("Qwen3.8-27B-UD-IQ1_S.gguf", 6_190_000_000),
            ("Qwen3.8-27B-UD-IQ3_S.gguf", 13_300_000_000),
            ("Qwen3.8-27B-Q4_0.gguf", 16_400_000_000),
            ("BF16/Qwen3.8-27B-BF16-00001-of-00002.gguf", 30_000_000_000),
            ("BF16/Qwen3.8-27B-BF16-00002-of-00002.gguf", 25_600_000_000),
        ]
        .into_iter()
        .map(|(p, s)| (p.to_owned(), s))
        .collect()
    }

    #[test]
    fn a_shard_is_never_mistaken_for_a_quantisation() {
        // The reported defect. Everything after the last dash of
        // `Qwen3.8-27B-BF16-00001-of-00002.gguf` is `00002`, so the download asked Ollama for
        // `hf.co/…:00002` and got *File does not exist*.
        let (quant, _) = read_quant("BF16/Qwen3.8-27B-BF16-00001-of-00002.gguf").expect("reads");
        assert_eq!(quant, "BF16");
    }

    #[test]
    fn the_prefix_that_belongs_to_the_name_is_kept() {
        // unsloth's dynamic quants are `UD-IQ3_S`, not `IQ3_S`. Weighing dropped it.
        let (quant, _) = read_quant("Qwen3.8-27B-UD-IQ3_S.gguf").expect("reads");
        assert_eq!(quant, "UD-IQ3_S");
    }

    #[test]
    fn asking_for_a_quantisation_never_lands_on_the_module_beside_it() {
        // A substring over a size-sorted list found `MTP/mtp-…-Q4_0.gguf` first, because it is
        // a tenth of the size — so somebody choosing a 16 GB model got a 1.4 GB module that is
        // not the model.
        let files = published();
        let machine = machine::Machine::default();
        let (path, bytes) = choose(&files, Some("Q4_0"), &machine).expect("it is published");
        assert_eq!(path, "Qwen3.8-27B-Q4_0.gguf");
        assert_eq!(*bytes, 16_400_000_000);

        // And the module is still reachable by the name it is published under.
        let (path, _) = choose(&files, Some("MTP"), &machine).expect("also published");
        assert!(path.contains("mtp-"), "{path}");
    }

    #[test]
    fn every_variant_offered_is_addressed_the_way_it_would_be_weighed() {
        // The two readers agreed until a filename neither had been tested against. This holds
        // them together: whatever the variant list offers, weighing that exact string must find
        // the same file back.
        let files = published();
        let machine = machine::Machine::default();
        for (path, _) in &files {
            let Some((quant, tag)) = read_quant(path) else {
                continue;
            };
            let asked = tag.as_deref().unwrap_or(&quant);
            let (found, _) = choose(&files, Some(asked), &machine)
                .unwrap_or_else(|why| panic!("'{asked}' came from {path}: {why}"));
            assert!(
                read_quant(found).is_some(),
                "{asked} resolved to something unreadable: {found}"
            );
        }
    }
}
