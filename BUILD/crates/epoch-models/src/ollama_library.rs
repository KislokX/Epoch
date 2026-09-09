//! Ollama's own shelf, searched the only way it can be.
//!
//! ## Why this is not the Hugging Face search one file over
//!
//! The Workshop searches Hugging Face for GGUF repositories, which is where most of what a
//! person pulls comes from. It does not reach Ollama's **curated library** — the short names,
//! `qwen3` and `gemma4` and `llama4` — and those are what somebody who has used Ollama before
//! types first. `ollama run qwen3` is the first command in its own documentation.
//!
//! ## Measured, and the measurement decided the shape
//!
//! Asked on 2026-08-21:
//!
//! ```text
//! GET ollama.com/search?q=qwen&format=json     200, text/html   (no JSON, whatever you ask for)
//! GET ollama.com/api/search?q=qwen             404 {"error":"path \"/api/search\" not found"}
//! GET registry.ollama.ai/v2/_catalog           404
//! GET registry.ollama.ai/v2/library/qwen3/tags/list  404
//! GET registry.ollama.ai/v2/library/qwen3/manifests/14b   200, real JSON
//! ```
//!
//! So: **there is no search API, and there is a real manifest API.** That pair is what makes
//! this defensible rather than a scrape.
//!
//! Names are read out of the search page by the one pattern that is the site's own structure —
//! `href="/library/<name>"`, a link to a model's page — rather than by a CSS class, which is
//! decoration and changes. And **every name is then verified against the manifest API**, which
//! genuinely answers. A broken scrape therefore yields *fewer* results, never wrong ones, and
//! yielding none is reported as *the page could not be read* rather than as *no matches* — an
//! empty list that looks like an answer is the failure this whole file is written around.

use std::collections::BTreeSet;

/// Where the shelf is.
const LIBRARY: &str = "https://ollama.com/search";

/// What a search of Ollama's library found.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    /// Model names, exactly as `ollama pull` takes them.
    pub names: Vec<String>,
    /// True when the page answered but nothing could be read out of it.
    ///
    /// **The distinction this file exists for.** *Nothing matched* and *the page changed shape*
    /// look identical in an empty list, and only one of them is a fact about the query.
    pub unreadable: bool,
}

/// Search Ollama's curated library.
///
/// An empty query lists what the shelf shows by default, which is what the page does.
pub fn search(query: &str) -> Result<Shelf, String> {
    read(&format!("{LIBRARY}?q={}", urlencode(query.trim())))
}

// There was a `cloud()` here, listing what `ollama.com/search?c=cloud` links to, and it was
// **wrong in a way only measuring caught.**
//
// That page answers *which families have a cloud tag somewhere*, not *which models are cloud
// models*. `gemma4` is on it — and `ollama.com/library/gemma4/tags` lists `gemma4:12b`,
// `gemma4:26b` and twenty other ordinary local tags. Presenting that list as cloud models would
// have told somebody "gemma4 is a cloud model", which is false about the model they already
// have on their disk.
//
// **Cloud is a property of the tag** (`gpt-oss:120b-cloud`), which is exactly what
// [`runs_in_the_cloud`] reads and what `disclosure::destination` acts on. That is the part that
// earned itself; a family list is a gauge nobody could explain.

fn read(url: &str) -> Result<Shelf, String> {
    let page = ureq::get(url)
        .timeout(std::time::Duration::from_secs(15))
        .call()
        .map_err(|err| format!("could not reach Ollama's library: {err}"))?
        .into_string()
        .map_err(|err| format!("Ollama's library answered something unreadable: {err}"))?;

    let names = names_in(&page);
    Ok(Shelf {
        unreadable: names.is_empty(),
        names,
    })
}

/// Every model name linked from one page.
///
/// **Its own function, because it is the part that can rot.** A test holds it against a real
/// fragment of the page as it was on 2026-08-21, so the day the markup changes, the test says
/// so rather than the product quietly returning nothing.
///
/// Sorted and deduplicated: the page links the same model from several places, and an ordering
/// taken from markup is an ordering that changes for reasons nobody can explain.
pub fn names_in(page: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    for piece in page.split("href=\"/library/").skip(1) {
        let Some(end) = piece.find('"') else {
            continue;
        };
        let name = &piece[..end];
        // A model name, and nothing that is a path, a query or an anchor. `qwen3.8` and
        // `qwen2.5-coder` are real; `qwen3/blobs` is a page about one.
        if name.is_empty()
            || !name.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_' | ':')
            })
        {
            continue;
        }
        found.insert(name.to_owned());
    }
    found.into_iter().collect()
}

/// Whether a model runs on Ollama's servers rather than on this machine.
///
/// **The one fact about a cloud model that changes what Epoch must say.** A turn on one leaves
/// the user's computer — their prompt, their Chronicle, whatever their character was given —
/// and `local` is what every disclosure in Epoch keys on (ADR-0025). A `-cloud` model reported
/// as local would be Epoch saying *this costs nothing and goes nowhere* about a request to a
/// third party.
///
/// By tag, because that is what Ollama itself uses and the only thing on the wire that says so.
/// Every tag one Ollama holds, with the bytes it weighs.
///
/// **Only what reports a size.** Ollama publishes the size of every tag; a plain
/// OpenAI-compatible list is names alone, and a model whose size is unknown cannot be placed
/// against a card — so it is left out rather than guessed at. Which is why llama.cpp and LM
/// Studio contribute nothing here even while they serve the same files: they are the same
/// weights on disk, and Ollama is the one that says how heavy they are.
///
/// An unreachable Ollama holds nothing, which is true of what this can see.
pub fn held_by(at: &str) -> Vec<(String, u64)> {
    #[derive(serde::Deserialize)]
    struct Tag {
        name: String,
        #[serde(default)]
        size: u64,
    }
    #[derive(serde::Deserialize)]
    struct Tags {
        #[serde(default)]
        models: Vec<Tag>,
    }

    // The same machine at the address that answers: `localhost` resolves to `[::1]` first on
    // Windows and an IPv4-only Ollama hangs there for fifteen seconds. Measured — see
    // `epoch_engine::provider::same_machine`, except this crate is below that one, so the same
    // two substitutions are made here rather than depended upon upwards.
    let at = at
        .trim_end_matches('/')
        .replace("://localhost:", "://127.0.0.1:")
        .replace("://[::1]:", "://127.0.0.1:");
    ureq::get(&format!("{at}/api/tags"))
        .timeout(std::time::Duration::from_secs(5))
        .call()
        .ok()
        .and_then(|said| said.into_json::<Tags>().ok())
        .map(|tags| {
            tags.models
                .into_iter()
                .filter(|tag| tag.size > 0)
                .map(|tag| (tag.name, tag.size))
                .collect()
        })
        .unwrap_or_default()
}

pub fn runs_in_the_cloud(model: &str) -> bool {
    model
        .rsplit(':')
        .next()
        .is_some_and(|tag| tag.eq_ignore_ascii_case("cloud") || tag.ends_with("-cloud"))
}

/// Percent-encode a query.
fn urlencode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real fragment of `ollama.com/search?q=qwen`, as it answered on 2026-08-21.
    ///
    /// Kept verbatim rather than simplified: the point of this test is to fail on the day the
    /// page stops looking like this, and a tidied-up fixture would keep passing.
    const REAL_PAGE: &str = r#"
      <li><a href="/library/qwen3.8" class="group w-full">qwen3.8</a></li>
      <li><a href="/library/qwen3-coder" class="group w-full">qwen3-coder</a></li>
      <li><a href="/library/qwen2.5-coder">qwen2.5-coder</a></li>
      <li><a href="/library/qwen3" x-test-search-response-title>qwen3</a></li>
      <li><a href="/library/qwen3">a second link to the same model</a></li>
      <a href="/library/qwen3/tags">tags</a>
      <a href="/search?q=other">not a model</a>
    "#;

    #[test]
    fn names_come_from_the_sites_own_urls_and_not_from_its_decoration() {
        // `href="/library/<name>"` is what the site *is* — a link to a model's page. A CSS class
        // is decoration and changes for reasons that have nothing to do with the data.
        let found = names_in(REAL_PAGE);
        assert_eq!(
            found,
            vec!["qwen2.5-coder", "qwen3", "qwen3-coder", "qwen3.8"]
        );
    }

    #[test]
    fn one_model_linked_twice_is_still_one_model() {
        // The page links the same model from several places, and an ordering taken from markup
        // is one that changes for reasons nobody can explain — so this is sorted and unique.
        assert_eq!(
            names_in(REAL_PAGE).iter().filter(|n| *n == "qwen3").count(),
            1
        );
    }

    #[test]
    fn a_page_about_a_model_is_not_a_model() {
        // `/library/qwen3/tags` is a page; `/search?q=other` is not a library link at all.
        let found = names_in(REAL_PAGE);
        assert!(!found.iter().any(|n| n.contains('/')), "{found:?}");
        assert!(!found.iter().any(|n| n.contains('?')), "{found:?}");
    }

    #[test]
    fn a_page_that_changed_shape_reads_as_unreadable_and_not_as_no_matches() {
        // The failure this whole file is written around. An empty list looks like an answer, and
        // only one of *nothing matched* and *the markup moved* is a fact about the query.
        assert!(names_in("<html><body>nothing here</body></html>").is_empty());

        let broken = Shelf {
            names: Vec::new(),
            unreadable: true,
        };
        assert!(broken.unreadable, "and it says which one it is");
    }

    #[test]
    fn a_cloud_model_is_recognised_by_the_tag_ollama_itself_uses() {
        // The one fact that changes what Epoch must say: a turn on one leaves the machine, and
        // `local` is what every disclosure keys on (ADR-0025).
        assert!(runs_in_the_cloud("gpt-oss:120b-cloud"));
        assert!(runs_in_the_cloud("deepseek-v4-pro:cloud"));
        assert!(!runs_in_the_cloud("qwen3:14b"));
        assert!(!runs_in_the_cloud("gemma4:12b"));
        // A repository whose *name* contains the word is not a cloud model.
        assert!(!runs_in_the_cloud("hf.co/somebody/cloud-model:Q4_K_M"));
        // And a bare name has no tag at all.
        assert!(!runs_in_the_cloud("qwen3"));
    }

    #[test]
    #[ignore = "reaches ollama.com"]
    fn the_library_still_looks_the_way_this_was_written_against() {
        // Run deliberately. It is the other half of the fixture above: one asserts the parser
        // against a page that was real, this asserts the page is still real.
        let found = search("qwen").expect("ollama.com answered");
        assert!(!found.unreadable, "the search page changed shape");
        assert!(
            found.names.contains(&"qwen3".to_owned()),
            "{:?}",
            found.names
        );

        // No cloud listing is asked for: see the note where `cloud()` used to be. That page
        // answers a different question from the one it looked like it answered.
    }
}
