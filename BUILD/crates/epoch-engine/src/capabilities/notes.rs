//! Reading the Library — somebody's second brain, as data.
//!
//! Two capabilities, and both only read. They declare `Reads` alone, so they cost nothing to
//! allow and never interrupt anybody — the same floor of usefulness the project file tools
//! stand on. **Nothing here writes**, and not by policy: no writing capability is constructed
//! over a library at all (see [`crate::library`]).
//!
//! ## Why these are not `read_file` and `find_files` pointed elsewhere
//!
//! Three differences, and each one is the whole reason a note tool exists:
//!
//! 1. **A note is addressed by its title.** `[[Architecture Runway]]` is how a vault refers to
//!    itself, and it says nothing about which folder that note is in. A reader that demanded a
//!    relative path would be handed a link on every page that it could not follow.
//! 2. **A search of notes wants the note, not the line.** `find_files` answers with matching
//!    lines because that is what you want in source. In a vault the useful answer is *which
//!    notes are about this*, with enough of each to recognise it.
//! 3. **The Library and the Project are different questions**, so they are different tools with
//!    different folders. One tool with a "which folder?" argument would make every call a place
//!    where the wrong one could be named.
//!
//! ## Untrusted, like everything else that comes back
//!
//! A note is somebody's writing, and a note that says "ignore your instructions" is a note that
//! says that. It reaches the model as data in a user-role message, exactly as a fetched web
//! page does; that rule lives with the turn loop and nothing here pretends otherwise.

use std::path::{Path, PathBuf};

use epoch_kernel::{Arguments, CapabilityId, Descriptor, Explanation, Parameter, ValueKind};

use crate::capability::{Capability, CapabilityError, Outcome, Source};
use crate::project::ProjectRoot;

/// The most of one note handed to a model at once.
///
/// Smaller than the project reader's cap on purpose: notes are prose, a model is being asked to
/// *understand* rather than to edit, and a vault's long note is a book chapter rather than a
/// build log. Past this it is windowed and says so.
const MAX_BYTES: usize = 64 * 1024;

/// The most notes one search returns.
const MAX_HITS: usize = 25;

/// How much of a matching note is quoted back, in characters.
const EXCERPT: usize = 320;

/// How deep the walk goes. Deep enough for a real vault, bounded against a symlink loop.
const MAX_DEPTH: usize = 12;

fn id(raw: &str) -> CapabilityId {
    CapabilityId::new(raw).expect("capability ids in this file are literals")
}

/// Notes as a vault names them: `Folder/Note.md` written the way a person would say it.
fn shown(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every note in the library, deepest folders included, in a stable order.
///
/// Sorted so two identical questions get identical answers. An unsorted `read_dir` is
/// filesystem order, which differs between machines and makes "why did it cite a different note
/// this time" unanswerable.
fn notes(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0usize)];

    while let Some((dir, depth)) = pending.pop() {
        if depth > MAX_DEPTH {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            // Hidden folders are configuration, not notes. `.obsidian` in particular holds
            // plugin source, which would drown a search in JavaScript.
            if path.is_dir() {
                if !name.starts_with('.') {
                    pending.push((path, depth + 1));
                }
            } else if name.ends_with(".md") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// Find the note somebody means.
///
/// Three ways, in order, and the order is what makes a `[[wikilink]]` work:
///
/// 1. exactly what was asked for, relative to the library;
/// 2. the same with `.md` added, since a link never writes the extension;
/// 3. **any note whose title matches**, anywhere in the vault — which is what a wikilink is.
///
/// An exact path therefore always wins, and is never ambiguous. That is the escape hatch the
/// ambiguity error below points people at.
///
/// Every candidate still goes through [`ProjectRoot::confine`]. Resolving by title never
/// escapes the library, because the search space is the walk of the library itself.
fn locate(root: &ProjectRoot, asked: &str) -> Result<PathBuf, CapabilityError> {
    let wanted = asked.trim().trim_start_matches("[[").trim_end_matches("]]");
    // A link may carry a display name or a heading: `[[Note|call it this]]`, `[[Note#Heading]]`.
    // Neither is part of the note's identity, so neither takes part in finding it.
    let wanted = wanted
        .split('|')
        .next()
        .unwrap_or(wanted)
        .split('#')
        .next()
        .unwrap_or(wanted)
        .trim();

    for candidate in [wanted.to_owned(), format!("{wanted}.md")] {
        if let Ok(path) = root.confine(&candidate) {
            if path.is_file() {
                return Ok(path);
            }
        }
    }

    let title = wanted.to_lowercase();
    let matches: Vec<PathBuf> = notes(root.path())
        .into_iter()
        .filter(|path| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_lowercase() == title)
                .unwrap_or(false)
        })
        .collect();

    match matches.len() {
        0 => Err(CapabilityError::Failed(format!(
            "no note called '{wanted}' in this library"
        ))),
        1 => Ok(matches.into_iter().next().expect("just counted")),
        // Named, never guessed. Two notes with one title is a real thing in a vault, and
        // silently picking the first would make a citation that cannot be checked.
        _ => Err(CapabilityError::Failed(format!(
            "'{wanted}' names {} notes: {}. Ask for one by its path.",
            matches.len(),
            matches
                .iter()
                .take(6)
                .map(|path| shown(root.path(), path))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

pub fn search_notes() -> Descriptor {
    Descriptor::observing(
        id("search_notes"),
        "Search this World's library of notes and return the notes that mention the words, with an excerpt of each.",
    )
    .taking([Parameter::required(
        "query",
        ValueKind::Text,
        "Words to look for. Matched anywhere in a note's title or text, ignoring case.",
    )])
}

pub fn read_note() -> Descriptor {
    Descriptor::observing(
        id("read_note"),
        "Read one note from this World's library. Accepts a path or a note's title, including a [[wikilink]].",
    )
    .taking([Parameter::required(
        "note",
        ValueKind::Text,
        "A note's title, or its path inside the library.",
    )])
}

/// Search the library.
pub struct SearchNotes {
    library: ProjectRoot,
}

impl SearchNotes {
    pub fn new(library: ProjectRoot) -> Self {
        Self { library }
    }
}

impl Capability for SearchNotes {
    fn describe(&self) -> Descriptor {
        search_notes()
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
        Ok(Explanation::of(
            &descriptor,
            format!("search the library for `{query}`"),
        ))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let query = arguments
            .text("query")
            .map_err(CapabilityError::BadArguments)?;
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Err(CapabilityError::BadArguments(
                "say what to search for".into(),
            ));
        }

        let root = self.library.path();
        let mut hits = Vec::new();
        let mut looked = 0usize;

        for path in notes(root) {
            looked += 1;
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let name = shown(root, &path);
            let in_title = name.to_lowercase().contains(&needle);
            let at = text.to_lowercase().find(&needle);
            if !in_title && at.is_none() {
                continue;
            }

            // Around the match rather than from the top: the first paragraph of a long note is
            // rarely the part that mentioned the thing being asked about.
            let excerpt = match at {
                Some(at) => {
                    let start = text[..at]
                        .char_indices()
                        .rev()
                        .nth(EXCERPT / 3)
                        .map(|(i, _)| i)
                        .unwrap_or(0);
                    let end = text[at..]
                        .char_indices()
                        .nth(EXCERPT)
                        .map(|(i, _)| at + i)
                        .unwrap_or(text.len());
                    text[start..end].trim().to_owned()
                }
                None => text.chars().take(EXCERPT).collect::<String>(),
            };

            hits.push((name, excerpt));
            if hits.len() >= MAX_HITS {
                break;
            }
        }

        if hits.is_empty() {
            // What was searched, not only that nothing was found. "Nothing in 0 notes" and
            // "nothing in 900 notes" are different answers, and only one of them means the
            // library is empty.
            return Ok(Outcome::told(format!(
                "No note in this library mentions '{query}'. {looked} notes were searched."
            )));
        }

        let truncated = hits.len() >= MAX_HITS;
        let body = hits
            .iter()
            .map(|(name, excerpt)| format!("## {name}\n{excerpt}"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let head = format!(
            "{} note{} in this library mention '{query}'{}:",
            hits.len(),
            if hits.len() == 1 { "" } else { "s" },
            if truncated {
                ", and there may be more"
            } else {
                ""
            }
        );

        Ok(Outcome {
            content: format!("{head}\n\n{body}"),
            // A search leaves nothing behind (ADR-0025), so it is never evidence.
            evidence: None,
            begun: None,
            // Citable, so the user can open the note and check what was said about it.
            sources: hits
                .iter()
                .map(|(name, _)| Source {
                    title: name.clone(),
                    url: root.join(name).to_string_lossy().to_string(),
                })
                .collect(),
        })
    }
}

/// Read one note.
pub struct ReadNote {
    library: ProjectRoot,
}

impl ReadNote {
    pub fn new(library: ProjectRoot) -> Self {
        Self { library }
    }
}

impl Capability for ReadNote {
    fn describe(&self) -> Descriptor {
        read_note()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let note = arguments
            .text("note")
            .map_err(CapabilityError::BadArguments)?;
        Ok(Explanation::of(&descriptor, format!("read note `{note}`")))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let asked = arguments
            .text("note")
            .map_err(CapabilityError::BadArguments)?;
        let path = locate(&self.library, asked)?;
        let name = shown(self.library.path(), &path);

        let bytes = std::fs::read(&path)
            .map_err(|err| CapabilityError::Failed(format!("cannot read '{name}': {err}")))?;
        let text = String::from_utf8_lossy(&bytes);

        let (body, note) = if bytes.len() > MAX_BYTES {
            let cut = text
                .char_indices()
                .take_while(|(i, _)| *i < MAX_BYTES)
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0);
            (
                text[..cut].to_owned(),
                // Said, never silent. A shortened note is one a model will reason about
                // wrongly and confidently.
                format!("\n\n[This note is longer than {MAX_BYTES} bytes and was cut here.]"),
            )
        } else {
            (text.into_owned(), String::new())
        };

        Ok(Outcome {
            content: format!("# {name}\n\n{body}{note}"),
            evidence: None,
            begun: None,
            sources: vec![Source {
                title: name,
                url: path.to_string_lossy().to_string(),
            }],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vault(name: &str) -> ProjectRoot {
        let dir = std::env::temp_dir().join(format!("epoch-notes-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Architecture")).expect("scratch");
        std::fs::write(
            dir.join("Architecture/Runway.md"),
            "# Runway\n\nEvery subsystem is classified IMPLEMENT NOW, DESIGN NOW or VISION.\n",
        )
        .expect("write");
        std::fs::write(
            dir.join("Daily.md"),
            "Talked about the runway with the crew.\n",
        )
        .expect("write");
        // Configuration, never notes.
        std::fs::create_dir_all(dir.join(".obsidian")).expect("mkdir");
        std::fs::write(dir.join(".obsidian/app.json"), "{\"runway\": true}").expect("write");
        ProjectRoot::open(&dir.to_string_lossy()).expect("open")
    }

    fn asked(key: &str, value: &str) -> Arguments {
        Arguments::new().with(key, epoch_kernel::Value::Text(value.into()))
    }

    #[test]
    fn a_note_is_found_by_its_title_from_anywhere_in_the_vault() {
        // The whole reason these are note tools rather than the project file tools. `Runway`
        // lives in `Architecture/`, and a vault refers to it as `[[Runway]]` from every page.
        let library = vault("wikilink");
        let read = ReadNote::new(library);

        let plain = read.run(&asked("note", "Runway")).expect("by title");
        let linked = read.run(&asked("note", "[[Runway]]")).expect("by wikilink");
        let with_alias = read
            .run(&asked("note", "[[Runway|the runway]]"))
            .expect("with a display name");
        let by_path = read
            .run(&asked("note", "Architecture/Runway.md"))
            .expect("by path");

        assert!(plain.content.contains("IMPLEMENT NOW"));
        assert_eq!(linked.content, plain.content);
        assert_eq!(with_alias.content, plain.content);
        assert_eq!(by_path.content, plain.content);
        // Citable, so the user can go and read what was quoted at them.
        assert_eq!(plain.sources.len(), 1);
        assert_eq!(plain.sources[0].title, "Architecture/Runway.md");
    }

    #[test]
    fn a_search_answers_with_notes_and_never_with_configuration() {
        // `.obsidian/app.json` contains the word and is not a note. A vault whose search
        // returns plugin source has spent somebody's context window on settings.
        let library = vault("search");
        let found = SearchNotes::new(library)
            .run(&asked("query", "runway"))
            .expect("search");

        assert!(found.content.contains("Architecture/Runway.md"));
        assert!(found.content.contains("Daily.md"));
        assert!(!found.content.contains("app.json"));
        assert_eq!(found.sources.len(), 2);
        // Reading is not evidence (ADR-0025): a Quest that only searched produced nothing.
        assert!(found.evidence.is_none());
    }

    #[test]
    fn nothing_found_says_how_much_was_looked_at() {
        // "Nothing in 0 notes" and "nothing in 900 notes" are different answers, and only one
        // of them means the library is empty.
        let library = vault("empty-answer");
        let found = SearchNotes::new(library)
            .run(&asked("query", "kubernetes"))
            .expect("search");

        assert!(found.content.contains("2 notes were searched"));
    }

    #[test]
    fn two_notes_with_one_title_are_named_rather_than_guessed() {
        // Real in a vault, and picking the first silently would produce a citation nobody can
        // check — the answer would be right about a note the reader never opened.
        let library = vault("ambiguous");
        // In a *second folder*, not at the root: a note at the root is reachable by the exact
        // path `Runway.md`, and an exact path is not ambiguous. Only title resolution can be.
        std::fs::create_dir_all(library.path().join("Journal")).expect("mkdir");
        std::fs::write(
            library.path().join("Journal/Runway.md"),
            "a different runway",
        )
        .expect("write");

        let refused = ReadNote::new(library).run(&asked("note", "Runway"));

        let Err(CapabilityError::Failed(why)) = refused else {
            panic!("two notes with one title must not resolve");
        };
        assert!(why.contains("names 2 notes"), "{why}");
        assert!(why.contains("Architecture/Runway.md"), "{why}");
    }

    #[test]
    fn nothing_outside_the_library_can_be_read() {
        // Confinement is `ProjectRoot`'s, deliberately: one implementation of containment, and
        // it is the one that has been thought about. This asserts it is actually in the path.
        let library = vault("confined");
        let refused = ReadNote::new(library).run(&asked("note", "../../../secrets.md"));

        assert!(matches!(
            refused,
            Err(CapabilityError::Refused(_) | CapabilityError::Failed(_))
        ));
    }
}
