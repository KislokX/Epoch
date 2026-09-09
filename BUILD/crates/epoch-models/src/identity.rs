//! Canonical identities for the things a benchmark record has to name exactly.
//!
//! ## The rule
//!
//! > **A display string is never an identity.** Persist the fields; build the sentence for a
//! > person from them. The moment a formatted string becomes a key, its formatting becomes a
//! > contract nobody wrote down — and the first person to change the wording breaks equality on
//! > records that are about the same thing.
//!
//! Measured 2026-09-01, both from the same file. `llama_build()` returns
//! `version: 0.3.0-dev (build 10622, commit 3737e4137)` because that is the whole line the
//! program prints; an older paused session had stored the same build as
//! `0.3.0-dev (build 10622, commit 3737e4137)` without the prefix. Two spellings of one fact, and
//! the prefix had leaked into a reference **id**.
//!
//! And a speculation setting was being persisted as `format!("{:?}", …)` — a `Debug` rendering,
//! which Rust makes no stability promise about and which changes the day a field is added. That
//! is a formatter deciding whether two benchmarks are comparable.

use serde::{Deserialize, Serialize};

/// What runtime, at what version, from which commit.
///
/// Fields rather than a sentence. [`Build::display`] makes the sentence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Build {
    /// `llama_cpp`. Canonical, never the pretty name.
    pub runtime: String,
    /// `0.3.0-dev`. `None` where the program would not say.
    pub version: Option<String>,
    /// `10622` — readable, and it repeats across forks, which is why the commit is kept too.
    pub number: Option<u64>,
    /// `3737e4137` — the exact answer.
    pub commit: Option<String>,
}

impl Build {
    /// Read what `llama-server --version` printed.
    ///
    /// **Parsed, never trusted as a whole string.** The line is
    /// `version: 0.3.0-dev (build 10622, commit 3737e4137)`; anything that does not match leaves
    /// the fields it could not read as `None`, which reads as *unrecorded* wherever it is
    /// compared — never as a guess, and never as an empty string that would compare equal to
    /// another empty string from a different failure.
    pub fn read(runtime: &str, said: &str) -> Self {
        let mut one = Build {
            runtime: runtime.to_owned(),
            ..Build::default()
        };
        let line = said
            .lines()
            .find(|it| it.contains("version:"))
            .unwrap_or("")
            .trim();
        let after = line.split("version:").nth(1).unwrap_or("").trim();
        let head = after.split('(').next().unwrap_or("").trim();
        if !head.is_empty() {
            one.version = Some(head.to_owned());
        }
        for part in after
            .split(['(', ')', ','])
            .map(str::trim)
            .filter(|it| !it.is_empty())
        {
            if let Some(n) = part.strip_prefix("build ") {
                one.number = n.trim().parse().ok();
            }
            if let Some(c) = part.strip_prefix("commit ") {
                let c = c.trim();
                if !c.is_empty() {
                    one.commit = Some(c.to_owned());
                }
            }
        }
        one
    }

    /// The sentence a person reads. **Built from the fields, never stored as identity.**
    pub fn display(&self) -> String {
        let mut said = self.version.clone().unwrap_or_else(|| "unknown".to_owned());
        let mut about: Vec<String> = Vec::new();
        if let Some(n) = self.number {
            about.push(format!("build {n}"));
        }
        if let Some(c) = &self.commit {
            about.push(format!("commit {c}"));
        }
        if !about.is_empty() {
            said.push_str(&format!(" ({})", about.join(", ")));
        }
        said
    }

    /// A short, stable token for an id. No spaces, no punctuation a path would object to.
    pub fn token(&self) -> String {
        match (self.number, &self.commit) {
            (Some(n), Some(c)) => format!("{}-b{n}-{c}", self.runtime),
            (Some(n), None) => format!("{}-b{n}", self.runtime),
            (None, Some(c)) => format!("{}-{c}", self.runtime),
            (None, None) => self.runtime.clone(),
        }
    }

    /// Whether anything was actually read.
    pub fn known(&self) -> bool {
        self.version.is_some() || self.number.is_some() || self.commit.is_some()
    }
}

/// Which speculative decoding was in force, as a stable id.
///
/// **Not a `Debug` rendering.** `format!("{:?}", speculation)` was being persisted, which makes
/// the `Debug` impl part of the record's meaning: add a field and every stored value silently
/// stops matching. The kinds are llama.cpp's own `--spec-type` values, which is a vocabulary the
/// program publishes rather than one invented here.
pub fn speculation_id(one: &crate::tuning::Speculation) -> String {
    let kind = one.kind.trim();
    if kind.is_empty() || kind == "none" {
        return "off".to_owned();
    }
    // The draft length changes what the technique *does*, so it belongs in the id — a run at
    // n_max 1 and one at n_max 8 are not the same configuration. The draft *file* does not: it
    // is named by the artefact fingerprint already.
    match one.n_max {
        Some(n) => format!("{kind}:n{n}"),
        None => kind.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = "version: 0.3.0-dev (build 10622, commit 3737e4137)\n\
                        built with Clang 20.1.8 for Windows x86_64";

    #[test]
    fn a_build_is_fields_and_the_sentence_is_made_from_them() {
        /*
            Measured 2026-09-01: one record stored `version: 0.3.0-dev (build 10622, commit
            3737e4137)` and another stored the same build without the `version: ` prefix. Two
            spellings of one fact, and the prefix had leaked into a reference **id**.
        */
        let one = Build::read("llama_cpp", REAL);
        assert_eq!(one.version.as_deref(), Some("0.3.0-dev"));
        assert_eq!(one.number, Some(10622));
        assert_eq!(one.commit.as_deref(), Some("3737e4137"));
        assert_eq!(one.display(), "0.3.0-dev (build 10622, commit 3737e4137)");
        assert!(!one.display().contains("version:"), "never in the sentence");
        assert_eq!(one.token(), "llama_cpp-b10622-3737e4137");
        assert!(one.known());

        // Both spellings of the same line read to the same identity, which is the whole point.
        let bare = Build::read(
            "llama_cpp",
            "version: 0.3.0-dev (build 10622, commit 3737e4137)",
        );
        assert_eq!(one, bare);
    }

    #[test]
    fn the_other_build_on_this_machine_is_a_different_identity() {
        // The winget package: Vulkan-only, and 4.5x slower on this card. Same model, same
        // hardware, a number that looks exactly like a fault to anything comparing them.
        let cuda = Build::read("llama_cpp", REAL);
        let vulkan = Build::read(
            "llama_cpp",
            "version: 0.1.2-dev (build 10507, commit 95c409c13)",
        );
        assert_ne!(cuda, vulkan);
        assert_ne!(cuda.token(), vulkan.token());
    }

    #[test]
    fn a_program_that_would_not_say_records_nothing_rather_than_an_empty_string() {
        // Unrecorded is a state a fingerprint handles. An empty string is a value, and two
        // different failures would compare equal.
        let nothing = Build::read("llama_cpp", "some unrelated output");
        assert_eq!(nothing.version, None);
        assert_eq!(nothing.number, None);
        assert_eq!(nothing.commit, None);
        assert!(!nothing.known());
        assert_eq!(nothing.token(), "llama_cpp");
        assert_eq!(nothing.display(), "unknown");
    }

    #[test]
    fn speculation_is_a_stable_id_and_never_a_debug_rendering() {
        /*
            `format!("{:?}", speculation)` was being persisted. That makes the `Debug` impl part
            of the record's meaning: add one field and every stored value stops matching, silently
            and everywhere at once.
        */
        use crate::tuning::Speculation;
        assert_eq!(speculation_id(&Speculation::default()), "off");
        assert_eq!(
            speculation_id(&Speculation {
                kind: "none".into(),
                ..Speculation::default()
            }),
            "off",
        );
        assert_eq!(
            speculation_id(&Speculation {
                kind: "ngram-simple".into(),
                ..Speculation::default()
            }),
            "ngram-simple",
        );
        // The draft length changes what the technique does, so two lengths are two
        // configurations. The draft *file* does not appear: the artefact fingerprint names it.
        assert_eq!(
            speculation_id(&Speculation {
                kind: "draft-mtp".into(),
                n_max: Some(3),
                draft: Some("somewhere.gguf".into()),
                ..Speculation::default()
            }),
            "draft-mtp:n3",
        );
        assert_ne!(
            speculation_id(&Speculation {
                kind: "draft-mtp".into(),
                n_max: Some(1),
                ..Speculation::default()
            }),
            speculation_id(&Speculation {
                kind: "draft-mtp".into(),
                n_max: Some(8),
                ..Speculation::default()
            }),
        );
    }
}
