//! Searching the web.
//!
//! ## Why there is a trait for exactly one backend
//!
//! Because the backend is the part most likely to be wrong. DuckDuckGo's HTML endpoint needs no
//! key, no account and no Docker — it works the minute Epoch is installed, which is the only
//! reason it is the default. It is also a page rather than an API, so the day it changes shape
//! this stops working and something else has to take over.
//!
//! `SearchBackend` is one method wide. That is not future-proofing for its own sake (Earn
//! Complexity would forbid that); it is the seam at the exact place the replacement will happen,
//! and it costs one trait.
//!
//! Deliberately **not** built: SearxNG and Brave. SearxNG means asking the user to run Docker,
//! which the project constitution rules out, and Brave means an API key, which means the secret
//! store that does not exist yet. Both become small once those problems are somebody's job.
//!
//! ## Results are somebody else's words
//!
//! A search result is a title and a snippet written by a stranger, chosen by a ranking nobody
//! here controls. It reaches the model wrapped as untrusted tool output like everything else —
//! there is no path by which it can instruct anyone (ADR-0012).

use std::time::Duration;

use epoch_kernel::{
    Arguments, CapabilityId, Descriptor, Effect, Explanation, Parameter, Reversal, ValueKind,
};

use crate::capability::{Capability, CapabilityError, Outcome, Source};

/// How long to wait. Shorter than a page fetch: a search that is slow is a search nobody wants
/// any more, and the character can always try again with better words.
const TIMEOUT: Duration = Duration::from_secs(12);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(1500);

/// How many results come back when nobody says.
const RESULTS: usize = 6;
/// The most anybody may ask for. Past this it is a scrape, not a search.
const MOST: usize = 12;

/// A found page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Somewhere searches go.
pub trait SearchBackend: Send + Sync {
    /// What this backend is called, for the sentence the user approves.
    fn name(&self) -> &'static str;

    /// Run the search. Errors are the user's to read, so they say what was tried.
    fn look(&self, query: &str, want: usize) -> Result<Vec<Found>, String>;
}

/// DuckDuckGo's HTML endpoint.
///
/// No key, no account, no quota to run out of on a Sunday. In exchange it is a web page being
/// read by a program, which is a bargain that has to be stated plainly rather than discovered:
/// if the markup changes this breaks, and it breaks visibly — zero results with the reason,
/// never a quiet empty answer.
pub struct DuckDuckGo;

impl SearchBackend for DuckDuckGo {
    fn name(&self) -> &'static str {
        "DuckDuckGo"
    }

    fn look(&self, query: &str, want: usize) -> Result<Vec<Found>, String> {
        let response = ureq::builder()
            .timeout_connect(CONNECT_TIMEOUT)
            .timeout(TIMEOUT)
            // Sent because the endpoint answers differently to a client that admits it is a
            // program. Not a disguise: it names Epoch.
            .user_agent("Epoch/0.1 (+https://github.com/epoch)")
            .build()
            .post("https://html.duckduckgo.com/html/")
            .send_form(&[("q", query)])
            .map_err(|err| format!("could not reach DuckDuckGo: {err}"))?;

        let body = response
            .into_string()
            .map_err(|err| format!("DuckDuckGo answered with something unreadable: {err}"))?;

        Ok(scrape(&body, want))
    }
}

/// Pull results out of the page.
///
/// Hand-written rather than a DOM library: the shape needed is three fields, and taking a
/// parser dependency to read one page would be a build-system decision made by a scraper.
fn scrape(html: &str, want: usize) -> Vec<Found> {
    let mut found = Vec::new();
    // Every result link, in order. The class is DuckDuckGo's own and is the thing that will
    // change one day; when it does, this returns nothing rather than nonsense.
    for chunk in html.split("result__a").skip(1) {
        if found.len() >= want {
            break;
        }
        let Some(href) = attribute(chunk, "href=\"") else {
            continue;
        };
        let Some(url) = real_url(&href) else { continue };
        let title = text_of(chunk);
        if title.is_empty() {
            continue;
        }
        let snippet = chunk
            .split_once("result__snippet")
            .map(|(_, rest)| text_of(rest))
            .unwrap_or_default();
        found.push(Found {
            title,
            url,
            snippet,
        });
    }
    found
}

/// The value of an attribute that begins just before this chunk.
///
/// The chunk starts *inside* the anchor tag, so `href` is a few characters behind it — hence
/// looking backwards is impossible and the split is done on the tag itself upstream.
fn attribute(chunk: &str, key: &str) -> Option<String> {
    let start = chunk.find(key)? + key.len();
    let rest = &chunk[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_owned())
}

/// Unwrap DuckDuckGo's redirect.
///
/// Results are given as `//duckduckgo.com/l/?uddg=<percent-encoded>`. Handing that to the model
/// would cite the search engine as the source of everything, which is exactly the kind of
/// almost-true that makes a citation worthless.
fn real_url(href: &str) -> Option<String> {
    let direct =
        || (href.starts_with("http://") || href.starts_with("https://")).then(|| href.to_owned());
    let Some((_, query)) = href.split_once("uddg=") else {
        return direct();
    };
    let encoded = query.split('&').next().unwrap_or(query);
    let decoded = percent_decode(encoded);
    (decoded.starts_with("http://") || decoded.starts_with("https://")).then_some(decoded)
}

/// Percent-decoding, for the one place it is needed.
fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The words in a fragment of HTML, tags removed.
fn text_of(chunk: &str) -> String {
    let body = chunk.split_once('>').map(|(_, rest)| rest).unwrap_or(chunk);
    let end = body.find('<').unwrap_or(body.len());
    let mut text = body[..end].trim().to_owned();
    for (entity, replacement) in [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#x27;", "'"),
    ] {
        text = text.replace(entity, replacement);
    }
    text
}

fn id(raw: &str) -> CapabilityId {
    CapabilityId::new(raw).expect("capability ids in this file are literals")
}

/// Search the web.
pub struct WebSearch {
    backend: Box<dyn SearchBackend>,
}

impl Default for WebSearch {
    fn default() -> Self {
        Self::new(Box::new(DuckDuckGo))
    }
}

impl WebSearch {
    pub fn new(backend: Box<dyn SearchBackend>) -> Self {
        Self { backend }
    }

    fn wanted(arguments: &Arguments) -> usize {
        match arguments.get("count") {
            Some(epoch_kernel::Value::Integer(n)) => (*n).clamp(1, MOST as i64) as usize,
            _ => RESULTS,
        }
    }
}

impl Capability for WebSearch {
    fn describe(&self) -> Descriptor {
        web_search()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let query = arguments
            .text("query")
            .map_err(CapabilityError::BadArguments)?;
        // Which engine, by name. The user is approving a query leaving their machine, and
        // *where it goes* is half of what they are agreeing to.
        Ok(Explanation::of(
            &descriptor,
            format!("search {} for `{query}`", self.backend.name()),
        )
        .expecting("titles, addresses and a line of description for each"))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let query = arguments
            .text("query")
            .map_err(CapabilityError::BadArguments)?;
        if query.trim().is_empty() {
            return Err(CapabilityError::BadArguments("a query is required".into()));
        }

        let found = self
            .backend
            .look(query, Self::wanted(arguments))
            .map_err(CapabilityError::Failed)?;

        if found.is_empty() {
            // Said plainly rather than returned as an empty list the model has to interpret. A
            // character told "no results" can say so; one handed nothing invents something.
            return Ok(Outcome::told(format!(
                "{} returned no results for `{query}`. Either there are none, or the search \
                 page has changed shape and Epoch can no longer read it.",
                self.backend.name()
            )));
        }

        let mut body = format!("{} results for `{query}`:\n\n", found.len());
        for (n, result) in found.iter().enumerate() {
            body.push_str(&format!("{}. {}\n   {}\n", n + 1, result.title, result.url));
            if !result.snippet.is_empty() {
                body.push_str(&format!("   {}\n", result.snippet));
            }
        }

        // Sources, not evidence. A search leaves nothing behind — a Quest that only searched
        // still produced nothing (ADR-0025) — but the answer can now be checked.
        let sources = found
            .into_iter()
            .map(|f| Source {
                title: f.title,
                url: f.url,
            })
            .collect();
        Ok(Outcome::told(body).from(sources))
    }
}

/// What this capability is, without needing one to exist.
///
/// **Separate from `describe`, so the catalogue can be read without a Project Root.** A
/// descriptor is static text and parameters; it never depended on the root a capability was
/// constructed with. Keeping it reachable only through an instance meant the one list a surface
/// needs - what this build can be asked for - had to be a hand-written constant somewhere else,
/// and that constant could not grow when a capability did.
pub fn web_search() -> Descriptor {
    Descriptor::acting(
        id("web_search"),
        "Search the web and get back titles, addresses and short descriptions. Use it to \
         find pages; use fetch_url to read one.",
        // Reads and Network — never an observation, because a request is something that
        // happened to somebody else's machine and what comes back is unvetted.
        [Effect::Reads, Effect::Network],
        // Permanent, and the descriptor validator was right to insist. A search feels like
        // looking at something, but the query has left the machine and been logged by a
        // stranger; there is no undoing that. `NothingToUndo` is for observations only, and
        // this is not one.
        Reversal::Permanent,
    )
    .taking([
        Parameter::required(
            "query",
            ValueKind::Text,
            "What to search for, in plain words.",
        ),
        Parameter::optional(
            "count",
            ValueKind::Integer,
            "How many results to return. Between 1 and 12; 6 if omitted.",
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    use epoch_kernel::{Risk, Value};

    /// A backend that answers from memory, so the parsing and the shaping are testable without
    /// asking DuckDuckGo anything.
    struct Canned(Vec<Found>);

    impl SearchBackend for Canned {
        fn name(&self) -> &'static str {
            "a test"
        }
        fn look(&self, _query: &str, want: usize) -> Result<Vec<Found>, String> {
            Ok(self.0.iter().take(want).cloned().collect())
        }
    }

    fn asking(query: &str) -> Arguments {
        Arguments::new().with("query", Value::Text(query.into()))
    }

    fn some(n: usize) -> Vec<Found> {
        (0..n)
            .map(|i| Found {
                title: format!("Result {i}"),
                url: format!("https://example.invalid/{i}"),
                snippet: "a line about it".into(),
            })
            .collect()
    }

    #[test]
    fn searching_is_never_an_observation() {
        // "It only reads" is the sentence this catches. The query leaves the machine, so the
        // user gets asked in every mode below Auto.
        let d = WebSearch::default().describe();
        assert!(!d.is_observation());
        assert_eq!(d.risk(), Risk::Medium);
        assert!(d.problems().is_empty(), "{:?}", d.problems());
    }

    #[test]
    fn the_approval_names_the_engine_and_the_words() {
        // Where a query goes is half of what is being agreed to.
        let search = WebSearch::new(Box::new(Canned(some(1))));
        let explained = search.explain(&asking("rust ownership")).unwrap();
        assert!(explained.what.contains("a test"), "{}", explained.what);
        assert!(explained.what.contains("rust ownership"));
    }

    #[test]
    fn results_come_back_as_sources_and_never_as_evidence() {
        // Searching leaves nothing behind. A History that counted searches as achievements
        // would be counting intentions (ADR-0025).
        let search = WebSearch::new(Box::new(Canned(some(3))));
        let outcome = search.run(&asking("anything")).unwrap();
        assert_eq!(outcome.evidence, None);
        assert_eq!(outcome.sources.len(), 3);
        assert_eq!(outcome.sources[0].url, "https://example.invalid/0");
        assert!(outcome.content.contains("Result 0"));
    }

    #[test]
    fn asking_for_too_many_gets_the_most_there_is() {
        let search = WebSearch::new(Box::new(Canned(some(50))));
        let outcome = search
            .run(&asking("anything").with("count", Value::Integer(999)))
            .unwrap();
        assert_eq!(outcome.sources.len(), MOST);
    }

    #[test]
    fn nothing_found_is_said_rather_than_returned_empty() {
        // A character handed an empty list invents something; one told "no results" says so.
        let search = WebSearch::new(Box::new(Canned(Vec::new())));
        let outcome = search.run(&asking("nothing at all")).unwrap();
        assert!(outcome.sources.is_empty());
        assert!(outcome.content.contains("no results"));
        // And it names the other possibility, because a scraper that silently stops working
        // looks exactly like a topic nobody has written about.
        assert!(outcome.content.contains("changed shape"));
    }

    #[test]
    fn a_result_cites_the_page_rather_than_the_search_engine() {
        // DuckDuckGo hands back a redirect. Citing that would make every source in Epoch a
        // DuckDuckGo link — almost true, and useless.
        let page = r##"
            <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Fdoc.rust-lang.org%2Fbook%2F&amp;rut=x">The Rust Book</a>
            <a class="result__snippet" href="#">Everything about ownership</a>
        "##;
        let found = scrape(page, 5);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].url, "https://doc.rust-lang.org/book/");
        assert_eq!(found[0].title, "The Rust Book");
    }

    #[test]
    fn a_page_that_changed_shape_returns_nothing_rather_than_nonsense() {
        // The bargain of reading a page instead of an API, stated as a test: when it breaks it
        // has to break visibly.
        assert!(scrape("<html><body>redesigned</body></html>", 5).is_empty());
    }
}
