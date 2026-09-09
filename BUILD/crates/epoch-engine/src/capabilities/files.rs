//! Reading the project — the first capabilities that touch the user's machine.
//!
//! All three only look. They declare `Reads` alone, so `Descriptor::risk()` computes `None`,
//! so they are the only things Explorer mode allows — and none of them ever prompts. Reading
//! is the floor of usefulness: a crew that has to ask permission to look at a file is a crew
//! nobody would keep.
//!
//! ## Every path goes through the Project Root
//!
//! Not by convention — structurally. These types cannot be constructed without a
//! [`ProjectRoot`], and the only way they turn a model's string into a path is
//! [`ProjectRoot::confine`]. There is no second route to the filesystem in this file.
//!
//! ## Limits exist because the caller is a model
//!
//! A model asked to "read the config" will happily read a 400 MB log, and the cost lands in
//! the context window rather than on the disk. So: a byte cap, a line window, a listing cap,
//! and a result cap on searches. Each one **says** when it truncated, because a silently
//! shortened file is a file the model will reason about wrongly and confidently.
//!
//! ## What comes back is not trusted
//!
//! Everything here returns somebody else's text. It reaches the model as data, wrapped, in a
//! user-role message — never as an instruction. That rule lives with the turn loop; this file
//! only makes sure nothing pretends otherwise.

use std::path::Path;

use epoch_kernel::{Arguments, CapabilityId, Descriptor, Explanation, Parameter, ValueKind};

use crate::capability::{Capability, CapabilityError, Outcome};
use crate::project::ProjectRoot;

use super::glob;

/// The most of one file that will be handed to a model at once.
///
/// Generous for source, firmly finite for a log. Past this the read is windowed and says so.
const MAX_BYTES: usize = 192 * 1024;

/// The most entries one listing returns.
const MAX_ENTRIES: usize = 400;

/// The most matches one search returns.
const MAX_MATCHES: usize = 200;

/// How deep a search walks. Deep enough for a real repository, bounded against a symlink loop.
const MAX_DEPTH: usize = 12;

/// Folders never walked into, and never listed.
///
/// Not a security boundary — confinement is that. These are here because a model given
/// `node_modules` will spend its entire context on somebody else's code, and because `.git`
/// contains the repository's whole history as unreadable blobs.
const SKIP: [&str; 7] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "__pycache__",
];

fn id(raw: &str) -> CapabilityId {
    // Every id here is a literal written above; a failure would be a typo caught by the tests
    // at the bottom of this file rather than something a user can reach.
    CapabilityId::new(raw).expect("capability ids in this file are literals")
}

/// Whether a file looks like text.
///
/// A NUL byte in the first few kilobytes is the same heuristic every editor uses, and it is
/// right about source code every time. Handing a model a binary is not dangerous — it is
/// useless, and it costs a whole context window to find out.
fn looks_binary(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|b| *b == 0)
}

fn skipped(name: &str) -> bool {
    SKIP.iter().any(|s| s.eq_ignore_ascii_case(name))
}

/// A path relative to the root, forward-slashed, for display and for glob matching.
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---------------------------------------------------------------------------
// read_file
// ---------------------------------------------------------------------------

/// Read a text file from the project.
pub struct ReadFile {
    root: ProjectRoot,
}

impl ReadFile {
    pub fn new(root: ProjectRoot) -> Self {
        Self { root }
    }
}

impl Capability for ReadFile {
    fn describe(&self) -> Descriptor {
        read_file()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let path = arguments
            .text("path")
            .map_err(CapabilityError::BadArguments)?;
        Ok(Explanation::of(&descriptor, format!("read `{path}`")))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let asked = arguments
            .text("path")
            .map_err(CapabilityError::BadArguments)?;
        let path = self.root.confine(asked).map_err(CapabilityError::Refused)?;

        if path.is_dir() {
            return Err(CapabilityError::Failed(format!(
                "'{asked}' is a folder; use list_files to see what is in it"
            )));
        }

        let bytes = std::fs::read(&path)
            .map_err(|err| CapabilityError::Failed(format!("cannot read '{asked}': {err}")))?;

        if looks_binary(&bytes) {
            return Err(CapabilityError::Failed(format!(
                "'{asked}' is not a text file"
            )));
        }

        let text = String::from_utf8_lossy(&bytes);
        let all: Vec<&str> = text.lines().collect();
        let total = all.len();

        // Counting from 1, because every editor and every error message does. A model that
        // saw line 40 in a compiler error must be able to ask for line 40.
        let from = match arguments.get("from_line") {
            Some(epoch_kernel::Value::Integer(n)) if *n > 0 => (*n as usize) - 1,
            _ => 0,
        };
        let count = match arguments.get("lines") {
            Some(epoch_kernel::Value::Integer(n)) if *n > 0 => *n as usize,
            _ => total,
        };

        if from >= total && total > 0 {
            return Err(CapabilityError::Failed(format!(
                "'{asked}' has {total} lines; line {} is past the end",
                from + 1
            )));
        }

        let window: Vec<&str> = all.into_iter().skip(from).take(count).collect();
        let shown = window.len();
        let mut body = window.join("\n");

        // Truncation is announced. A silently shortened file is one a model reasons about
        // wrongly and confidently — it will conclude a function does not exist.
        let mut notes = Vec::new();
        if body.len() > MAX_BYTES {
            body.truncate(MAX_BYTES);
            notes.push(format!("truncated at {MAX_BYTES} bytes"));
        }
        if from > 0 || from + shown < total {
            notes.push(format!("lines {}–{} of {total}", from + 1, from + shown));
        }

        let header = if notes.is_empty() {
            format!("{}\n", relative(self.root.path(), &path))
        } else {
            format!(
                "{} ({})\n",
                relative(self.root.path(), &path),
                notes.join(", ")
            )
        };

        // Reading is not evidence. A Quest that only read things produced nothing (ADR-0025).
        Ok(Outcome::told(format!("{header}{body}")))
    }
}

// ---------------------------------------------------------------------------
// list_files
// ---------------------------------------------------------------------------

/// List one folder of the project.
pub struct ListFiles {
    root: ProjectRoot,
}

impl ListFiles {
    pub fn new(root: ProjectRoot) -> Self {
        Self { root }
    }
}

impl Capability for ListFiles {
    fn describe(&self) -> Descriptor {
        list_files()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let what = match arguments.optional_text("path") {
            Some(value) => format!("list `{value}`"),
            None => "list the project root".to_owned(),
        };
        Ok(Explanation::of(&descriptor, what))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        // Blank is how a model omits an optional argument — see `Arguments::optional_text`.
        let asked = arguments.optional_text("path").unwrap_or(".");
        let path = self.root.confine(asked).map_err(CapabilityError::Refused)?;

        if !path.is_dir() {
            return Err(CapabilityError::Failed(format!(
                "'{asked}' is not a folder"
            )));
        }

        let entries = std::fs::read_dir(&path)
            .map_err(|err| CapabilityError::Failed(format!("cannot read '{asked}': {err}")))?;

        let mut listed: Vec<String> = Vec::new();
        let mut hidden = 0usize;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if skipped(&name) {
                hidden += 1;
                continue;
            }
            let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
            listed.push(if is_dir { format!("{name}/") } else { name });
        }
        // Folders first, then alphabetical — the order a person reads a directory in, and
        // stable across machines, which `read_dir` alone is not.
        listed.sort_by_key(|n| (!n.ends_with('/'), n.to_lowercase()));

        let total = listed.len();
        listed.truncate(MAX_ENTRIES);

        let mut notes = Vec::new();
        if total > listed.len() {
            notes.push(format!("{} of {total} shown", listed.len()));
        }
        if hidden > 0 {
            notes.push(format!(
                "{hidden} skipped (build output, dependencies, .git)"
            ));
        }

        let where_ = relative(self.root.path(), &path);
        let head = if notes.is_empty() {
            format!("{where_}/\n")
        } else {
            format!("{where_}/ ({})\n", notes.join(", "))
        };
        Ok(Outcome::told(format!("{head}{}", listed.join("\n"))))
    }
}

// ---------------------------------------------------------------------------
// Changing files
// ---------------------------------------------------------------------------

/// Where a capability records what it replaced, so it can be put back.
///
/// Held as a path pair rather than a shared journal, because a capability is `&self` and the
/// journal is a file: reading it, appending and writing it back is the whole operation, and
/// two capabilities writing at once is not a case this product has.
#[derive(Debug, Clone)]
pub struct Undo {
    vault: std::path::PathBuf,
    world: String,
}

impl Undo {
    pub fn new(vault: impl Into<std::path::PathBuf>, world: impl Into<String>) -> Self {
        Self {
            vault: vault.into(),
            world: world.into(),
        }
    }

    /// Remember a change. Best effort: a journal that cannot be written is a lost undo, not a
    /// lost edit, and failing the edit over it would be the tail wagging the dog.
    fn record(&self, change: crate::journal::Change) {
        let mut journal = crate::journal::Journal::load(&self.vault, &self.world);
        journal.record(change);
        let _ = journal.save(&self.vault, &self.world);
    }
}

/// The unified-diff-ish lines for one exact replacement.
///
/// Not a diff algorithm: an exact-string edit already knows precisely what goes and what
/// arrives, so there is nothing to infer. That is a property worth having — a computed diff can
/// be *plausible*, and this one is the change itself.
fn diff(before: &str, after: &str) -> String {
    let mut lines = Vec::new();
    for line in before.lines() {
        lines.push(format!("- {line}"));
    }
    for line in after.lines() {
        lines.push(format!("+ {line}"));
    }
    // A preview nobody reads is not a preview. Past this it is a wall of text and the useful
    // signal — *which* lines — is already at the top.
    if lines.len() > 40 {
        lines.truncate(40);
        lines.push(format!(
            "… and {} more",
            before.lines().count() + after.lines().count() - 40
        ));
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// write_file
// ---------------------------------------------------------------------------

/// Write a text file into the project.
///
/// The whole contents at once. Use [`EditFile`] to change part of an existing file.
///
/// This shipped declaring `Reversal::Permanent`, with a note saying it would become `Undoable`
/// the day an undo store existed. That day arrived: every write records what it replaced
/// ([`crate::journal`]), so the reversal is now a fact rather than a claim — which is the only
/// condition under which it may be declared at all.
pub struct WriteFile {
    root: ProjectRoot,
    undo: Undo,
}

impl WriteFile {
    pub fn new(root: ProjectRoot, undo: Undo) -> Self {
        Self { root, undo }
    }
}

impl Capability for WriteFile {
    fn describe(&self) -> Descriptor {
        write_file()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let asked = arguments
            .text("path")
            .map_err(CapabilityError::BadArguments)?;
        let content = arguments
            .text("content")
            .map_err(CapabilityError::BadArguments)?;

        // The difference the user actually needs before deciding: is something being lost?
        //
        // Answered against the real path rather than guessed from the string, because
        // "create" and "replace 400 lines" are not the same decision and the whole point of
        // explaining first is that the sentence is true.
        let what = match self.root.confine(asked) {
            Ok(path) if path.exists() => {
                let existing = std::fs::read_to_string(&path).unwrap_or_default();
                format!(
                    "replace `{asked}` — {} lines become {}",
                    existing.lines().count(),
                    content.lines().count()
                )
            }
            // Confinement failing is not reported here: judging happens before running, and a
            // path outside the project is refused by `run` with its own message. Saying
            // "create" is the honest reading of a path that does not exist.
            _ => format!("create `{asked}` — {} lines", content.lines().count()),
        };

        Ok(Explanation::of(&descriptor, what).expecting("the file on disk afterwards"))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let asked = arguments
            .text("path")
            .map_err(CapabilityError::BadArguments)?;
        let content = arguments
            .text("content")
            .map_err(CapabilityError::BadArguments)?;
        let path = self.root.confine(asked).map_err(CapabilityError::Refused)?;

        if path.is_dir() {
            return Err(CapabilityError::Failed(format!("'{asked}' is a folder")));
        }
        if content.len() > MAX_BYTES {
            return Err(CapabilityError::Failed(format!(
                "that is {} bytes; the limit for one write is {MAX_BYTES}",
                content.len()
            )));
        }

        let before = std::fs::read_to_string(&path).ok();
        let existed = before.is_some();

        // Parent folders are created. A model writing `src/thing/mod.rs` into a project that
        // has no `thing` folder is doing ordinary work, and refusing would send it round the
        // loop to ask for something Epoch does not offer.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                CapabilityError::Failed(format!("cannot make the folder for '{asked}': {err}"))
            })?;
        }

        std::fs::write(&path, content)
            .map_err(|err| CapabilityError::Failed(format!("cannot write '{asked}': {err}")))?;

        let rel = relative(self.root.path(), &path);
        let verb = if existed { "replaced" } else { "created" };

        self.undo.record(crate::journal::Change {
            path: rel.clone(),
            before,
            after: content.to_owned(),
            summary: format!("{verb} {rel}"),
        });
        // Evidence, because something real exists now that did not before. This is what makes
        // a Quest's History non-fictional (ADR-0025).
        Ok(Outcome::made(
            format!("{verb} {rel} ({} lines)", content.lines().count()),
            rel.clone(),
            format!("{verb} {rel}"),
        ))
    }
}

// ---------------------------------------------------------------------------
// edit_file
// ---------------------------------------------------------------------------

/// Change part of an existing file, by exact replacement.
///
/// ## Why this is not a nicer `write_file`
///
/// `write_file` asks a model to reproduce a whole file. On four hundred lines it will drop a
/// function, reorder an import or lose a comment, and the cost is paid twice — once in tokens
/// to produce it, once in trust when the diff turns out to be enormous.
///
/// An exact replacement is **verifiable**: the old text either appears exactly once or the edit
/// does not happen. That is a property, not an optimisation. A match that is ambiguous is
/// refused too, because "the first one" is a guess about which line somebody meant.
///
/// ## And it is the reason the preview is real
///
/// The change is known before it is made — precisely, not approximately — so the approval can
/// show the actual lines rather than a description of them. A computed diff can be *plausible*;
/// this one is the change itself.
pub struct EditFile {
    root: ProjectRoot,
    undo: Undo,
}

impl EditFile {
    pub fn new(root: ProjectRoot, undo: Undo) -> Self {
        Self { root, undo }
    }

    /// Resolve the file and find the one occurrence, or say why not.
    ///
    /// Shared by `explain` and `run` so that what the user was shown and what happens are
    /// decided by the same code. Two implementations of "which occurrence" is how an approval
    /// ends up describing a different edit than the one that runs.
    fn locate(
        &self,
        arguments: &Arguments,
    ) -> Result<(std::path::PathBuf, String), CapabilityError> {
        let asked = arguments
            .text("path")
            .map_err(CapabilityError::BadArguments)?;
        let old = arguments
            .text("old")
            .map_err(CapabilityError::BadArguments)?;
        if old.is_empty() {
            return Err(CapabilityError::BadArguments(
                "'old' is empty; use write_file to replace a whole file".into(),
            ));
        }

        let path = self.root.confine(asked).map_err(CapabilityError::Refused)?;
        let bytes = std::fs::read(&path)
            .map_err(|err| CapabilityError::Failed(format!("cannot read '{asked}': {err}")))?;
        if looks_binary(&bytes) {
            return Err(CapabilityError::Failed(format!(
                "'{asked}' is not a text file"
            )));
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();

        match text.matches(old).count() {
            1 => Ok((path, text)),
            0 => Err(CapabilityError::Failed(format!(
                "that text does not appear in '{asked}'. Read the file and copy the lines \
                 exactly, including indentation."
            ))),
            n => Err(CapabilityError::Failed(format!(
                "that text appears {n} times in '{asked}'. Include more of the surrounding \
                 lines so it matches exactly once."
            ))),
        }
    }
}

impl Capability for EditFile {
    fn describe(&self) -> Descriptor {
        edit_file()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let asked = arguments
            .text("path")
            .map_err(CapabilityError::BadArguments)?;
        let old = arguments
            .text("old")
            .map_err(CapabilityError::BadArguments)?;
        let new = arguments
            .text("new")
            .map_err(CapabilityError::BadArguments)?;

        // Located rather than assumed: if the text is not there, or is there twice, the user is
        // told that instead of being asked to approve an edit that could not happen.
        self.locate(arguments)?;

        Ok(Explanation::of(&descriptor, format!("edit `{asked}`")).showing(diff(old, new)))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let old = arguments
            .text("old")
            .map_err(CapabilityError::BadArguments)?;
        let new = arguments
            .text("new")
            .map_err(CapabilityError::BadArguments)?;
        let (path, before) = self.locate(arguments)?;

        let after = before.replacen(old, new, 1);
        std::fs::write(&path, &after).map_err(|err| {
            let asked = arguments.text("path").unwrap_or_default();
            CapabilityError::Failed(format!("cannot write '{asked}': {err}"))
        })?;

        let rel = relative(self.root.path(), &path);
        self.undo.record(crate::journal::Change {
            path: rel.clone(),
            before: Some(before),
            after,
            summary: format!("edited {rel}"),
        });

        Ok(Outcome::made(
            format!(
                "edited {rel} ({} lines replaced by {})",
                old.lines().count(),
                new.lines().count()
            ),
            rel.clone(),
            format!("edited {rel}"),
        ))
    }
}

// ---------------------------------------------------------------------------
// find_files
// ---------------------------------------------------------------------------

/// Find files in the project by name pattern.
pub struct FindFiles {
    root: ProjectRoot,
}

impl FindFiles {
    pub fn new(root: ProjectRoot) -> Self {
        Self { root }
    }

    /// Walk the project, collecting relative paths that match.
    ///
    /// Iterative rather than recursive, with an explicit depth bound: a symlinked directory
    /// pointing at its own parent is a real thing that exists, and a recursive walk meets it
    /// by exhausting the stack.
    fn walk(&self, pattern: &str) -> (Vec<String>, bool) {
        let root = self.root.path();
        let mut found = Vec::new();
        let mut stack = vec![(root.to_path_buf(), 0usize)];
        let mut hit_limit = false;

        while let Some((dir, depth)) = stack.pop() {
            if depth > MAX_DEPTH {
                hit_limit = true;
                continue;
            }
            let Ok(entries) = std::fs::read_dir(&dir) else {
                // An unreadable folder is skipped rather than failing the search. A permission
                // error two levels down should not lose the matches already found.
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if skipped(&name) {
                    continue;
                }
                let path = entry.path();
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                if is_dir {
                    stack.push((path, depth + 1));
                    continue;
                }
                let rel = relative(root, &path);
                if glob::matches(pattern, &rel) {
                    if found.len() >= MAX_MATCHES {
                        hit_limit = true;
                        continue;
                    }
                    found.push(rel);
                }
            }
        }
        // `read_dir` order is the filesystem's, which differs between machines. Sorted so the
        // same project gives the same answer twice.
        found.sort();
        (found, hit_limit)
    }
}

impl Capability for FindFiles {
    fn describe(&self) -> Descriptor {
        find_files()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let pattern = arguments
            .text("pattern")
            .map_err(CapabilityError::BadArguments)?;
        Ok(Explanation::of(
            &descriptor,
            format!("find files matching `{pattern}`"),
        ))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let pattern = arguments
            .text("pattern")
            .map_err(CapabilityError::BadArguments)?;
        if pattern.trim().is_empty() {
            return Err(CapabilityError::BadArguments(
                "a pattern is required".into(),
            ));
        }

        let (found, truncated) = self.walk(pattern);
        if found.is_empty() {
            // A plain sentence rather than an empty string. "Nothing matched" is an answer,
            // and a model handed "" concludes the tool is broken.
            return Ok(Outcome::told(format!("no files match `{pattern}`")));
        }

        let head = if truncated {
            format!(
                "{} files match `{pattern}` (first {})\n",
                found.len(),
                MAX_MATCHES
            )
        } else {
            format!("{} files match `{pattern}`\n", found.len())
        };
        Ok(Outcome::told(format!("{head}{}", found.join("\n"))))
    }
}

/// What this capability is, without needing one to exist.
///
/// **Separate from `describe`, so the catalogue can be read without a Project Root.** A
/// descriptor is static text and parameters; it never depended on the root a capability was
/// constructed with. Keeping it reachable only through an instance meant the one list a surface
/// needs - what this build can be asked for - had to be a hand-written constant somewhere else,
/// and that constant could not grow when a capability did.
pub fn read_file() -> Descriptor {
    Descriptor::observing(
        id("read_file"),
        "Read a text file from the project. Optionally a window of lines.",
    )
    .taking([
        Parameter::required(
            "path",
            ValueKind::Text,
            "Path relative to the project root.",
        ),
        Parameter::optional(
            "from_line",
            ValueKind::Integer,
            "First line to return, counting from 1. Omit to start at the beginning.",
        ),
        Parameter::optional(
            "lines",
            ValueKind::Integer,
            "How many lines to return. Omit to read to the end.",
        ),
    ])
}

/// What this capability is, without needing one to exist.
///
/// **Separate from `describe`, so the catalogue can be read without a Project Root.** A
/// descriptor is static text and parameters; it never depended on the root a capability was
/// constructed with. Keeping it reachable only through an instance meant the one list a surface
/// needs - what this build can be asked for - had to be a hand-written constant somewhere else,
/// and that constant could not grow when a capability did.
pub fn list_files() -> Descriptor {
    Descriptor::observing(
        id("list_files"),
        "List the files and folders in one folder of the project.",
    )
    .taking([Parameter::optional(
        "path",
        ValueKind::Text,
        "Folder relative to the project root. Omit for the root itself.",
    )])
}

/// What this capability is, without needing one to exist.
///
/// **Separate from `describe`, so the catalogue can be read without a Project Root.** A
/// descriptor is static text and parameters; it never depended on the root a capability was
/// constructed with. Keeping it reachable only through an instance meant the one list a surface
/// needs - what this build can be asked for - had to be a hand-written constant somewhere else,
/// and that constant could not grow when a capability did.
pub fn write_file() -> Descriptor {
    Descriptor::acting(
        id("write_file"),
        "Write a text file in the project, creating it or replacing the whole thing. To change part of an existing file, use edit_file instead.",
        [epoch_kernel::Effect::Writes],
        epoch_kernel::Reversal::Undoable("Epoch keeps what it replaced".into()),
    )
    .taking([
        Parameter::required("path", ValueKind::Text, "Path relative to the project root."),
        Parameter::required("content", ValueKind::Text, "The whole new contents of the file."),
    ])
}

/// What this capability is, without needing one to exist.
///
/// **Separate from `describe`, so the catalogue can be read without a Project Root.** A
/// descriptor is static text and parameters; it never depended on the root a capability was
/// constructed with. Keeping it reachable only through an instance meant the one list a surface
/// needs - what this build can be asked for - had to be a hand-written constant somewhere else,
/// and that constant could not grow when a capability did.
pub fn edit_file() -> Descriptor {
    Descriptor::acting(
        id("edit_file"),
        "Replace an exact piece of text in a file. The old text must appear exactly once.",
        [epoch_kernel::Effect::Writes],
        epoch_kernel::Reversal::Undoable("Epoch keeps what it replaced".into()),
    )
    .taking([
        Parameter::required(
            "path",
            ValueKind::Text,
            "Path relative to the project root.",
        ),
        Parameter::required(
            "old",
            ValueKind::Text,
            "The exact text to replace, copied from the file including indentation. It \
             must appear exactly once.",
        ),
        Parameter::required("new", ValueKind::Text, "What to put there instead."),
    ])
}

/// What this capability is, without needing one to exist.
///
/// **Separate from `describe`, so the catalogue can be read without a Project Root.** A
/// descriptor is static text and parameters; it never depended on the root a capability was
/// constructed with. Keeping it reachable only through an instance meant the one list a surface
/// needs - what this build can be asked for - had to be a hand-written constant somewhere else,
/// and that constant could not grow when a capability did.
pub fn find_files() -> Descriptor {
    Descriptor::observing(
        id("find_files"),
        "Find files in the project by name pattern, e.g. `**/*.rs` or `src/*.toml`.",
    )
    .taking([Parameter::required(
        "pattern",
        ValueKind::Text,
        "A glob. `*` matches within a folder, `**` across folders, `?` one character.",
    )])
}

#[cfg(test)]
mod tests {
    use super::*;

    use epoch_kernel::{Risk, Value};

    struct Tree(std::path::PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("epoch-files-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(d.join("src")).unwrap();
            std::fs::create_dir_all(d.join("node_modules/left-pad")).unwrap();
            std::fs::write(d.join("README.md"), "# A project\n").unwrap();
            std::fs::write(d.join("src/main.rs"), "one\ntwo\nthree\nfour\nfive\n").unwrap();
            std::fs::write(d.join("src/lib.rs"), "pub fn go() {}\n").unwrap();
            std::fs::write(d.join("node_modules/left-pad/index.js"), "//\n").unwrap();
            Self(d)
        }
        /// A journal beside the project rather than inside it, exactly as the shell does it.
        fn undo(&self) -> Undo {
            Undo::new(self.0.join("_vault"), "archipelago")
        }
        fn journal(&self) -> crate::journal::Journal {
            crate::journal::Journal::load(&self.0.join("_vault"), "archipelago")
        }
        fn root(&self) -> ProjectRoot {
            ProjectRoot::open(self.0.to_str().unwrap()).unwrap()
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn args(pairs: &[(&str, Value)]) -> Arguments {
        pairs
            .iter()
            .fold(Arguments::new(), |a, (k, v)| a.with(k, v.clone()))
    }

    fn text(v: &str) -> Value {
        Value::Text(v.into())
    }

    #[test]
    fn reading_is_the_one_thing_that_never_asks() {
        // All three declare Reads alone, so risk computes to None, so Explorer allows them and
        // nothing prompts. A crew that must ask permission to look is a crew nobody keeps.
        let t = Tree::new("risk");
        for descriptor in [
            ReadFile::new(t.root()).describe(),
            ListFiles::new(t.root()).describe(),
            FindFiles::new(t.root()).describe(),
        ] {
            assert_eq!(descriptor.risk(), Risk::None, "{}", descriptor.id);
            assert!(descriptor.is_observation());
            assert!(
                descriptor.problems().is_empty(),
                "{:?}",
                descriptor.problems()
            );
        }
    }

    #[test]
    fn a_file_is_read_with_its_path_named() {
        let t = Tree::new("read");
        let out = ReadFile::new(t.root())
            .run(&args(&[("path", text("src/main.rs"))]))
            .unwrap();
        assert!(out.content.starts_with("src/main.rs\n"), "{}", out.content);
        assert!(out.content.contains("three"));
        // Reading leaves nothing behind, so it is not evidence (ADR-0025).
        assert_eq!(out.evidence, None);
    }

    #[test]
    fn a_line_window_counts_from_one_and_says_what_it_showed() {
        // Counting from 1 because every compiler error does. A model that saw "line 3" must be
        // able to ask for line 3.
        let t = Tree::new("window");
        let out = ReadFile::new(t.root())
            .run(&args(&[
                ("path", text("src/main.rs")),
                ("from_line", Value::Integer(2)),
                ("lines", Value::Integer(2)),
            ]))
            .unwrap();
        assert!(out.content.contains("lines 2–3 of 5"), "{}", out.content);
        assert!(out.content.contains("two\nthree"));
        assert!(!out.content.contains("five"));
    }

    #[test]
    fn asking_past_the_end_says_so_rather_than_returning_nothing() {
        let t = Tree::new("past");
        let err = ReadFile::new(t.root())
            .run(&args(&[
                ("path", text("src/main.rs")),
                ("from_line", Value::Integer(99)),
            ]))
            .unwrap_err();
        assert!(matches!(err, CapabilityError::Failed(ref why) if why.contains("past the end")));
    }

    #[test]
    fn a_path_outside_the_project_is_refused_not_merely_failed() {
        // Refused is its own error: nothing was attempted, and the model must learn that
        // rather than concluding the machine is broken and retrying.
        let t = Tree::new("escape");
        let err = ReadFile::new(t.root())
            .run(&args(&[("path", text("../../../secrets.txt"))]))
            .unwrap_err();
        assert!(matches!(err, CapabilityError::Refused(ref why) if why.contains("outside")));
    }

    #[test]
    fn a_binary_file_is_refused_before_it_costs_a_context_window() {
        let t = Tree::new("binary");
        std::fs::write(t.0.join("thing.bin"), [0u8, 1, 2, 3, 0]).unwrap();
        let err = ReadFile::new(t.root())
            .run(&args(&[("path", text("thing.bin"))]))
            .unwrap_err();
        assert!(matches!(err, CapabilityError::Failed(ref why) if why.contains("not a text file")));
    }

    #[test]
    fn reading_a_folder_points_at_the_tool_that_does_that() {
        // An error a model can act on beats one it can only report.
        let t = Tree::new("isdir");
        let err = ReadFile::new(t.root())
            .run(&args(&[("path", text("src"))]))
            .unwrap_err();
        assert!(matches!(err, CapabilityError::Failed(ref why) if why.contains("list_files")));
    }

    #[test]
    fn a_listing_is_ordered_and_hides_what_nobody_wants_to_read() {
        let t = Tree::new("list");
        let out = ListFiles::new(t.root()).run(&Arguments::new()).unwrap();
        // Folders first, then alphabetical — a person's reading order, and stable across
        // machines, which read_dir alone is not.
        let lines: Vec<&str> = out.content.lines().skip(1).collect();
        assert_eq!(lines, vec!["src/", "README.md"]);
        assert!(out.content.contains("1 skipped"), "{}", out.content);
        assert!(!out.content.contains("node_modules"));
    }

    #[test]
    fn a_blank_path_lists_the_project_root_rather_than_refusing() {
        // Measured in a real turn: the model called `list_files` with `path: ""`, Epoch
        // answered "a path is required", and the message was spent on the refusal. The
        // schema names the field, so blank is the only way a model has to omit it.
        let t = Tree::new("blank");
        let listing = ListFiles::new(t.root());
        let root = listing.run(&Arguments::new()).unwrap().content;
        for said in ["", "  "] {
            let out = listing.run(&args(&[("path", text(said))])).unwrap();
            assert_eq!(out.content, root, "for {said:?}");
        }
        // And the sentence the user approves says where it is going.
        let what = listing.explain(&args(&[("path", text(""))])).unwrap().what;
        assert!(what.contains("project root"), "{what}");
    }

    #[test]
    fn finding_uses_the_pattern_people_actually_type() {
        let t = Tree::new("find");
        let finder = FindFiles::new(t.root());

        let out = finder.run(&args(&[("pattern", text("**/*.rs"))])).unwrap();
        assert!(out.content.contains("src/lib.rs"));
        assert!(out.content.contains("src/main.rs"));
        assert!(out.content.starts_with("2 files match"), "{}", out.content);

        // Dependencies are not searched: a model handed node_modules spends its whole context
        // on somebody else's code.
        assert!(!finder
            .run(&args(&[("pattern", text("**/*.js"))]))
            .unwrap()
            .content
            .contains("left-pad"));
    }

    #[test]
    fn nothing_matching_is_an_answer_not_an_empty_string() {
        // A model handed "" concludes the tool is broken and tries something else.
        let t = Tree::new("nomatch");
        let out = FindFiles::new(t.root())
            .run(&args(&[("pattern", text("**/*.zig"))]))
            .unwrap();
        assert_eq!(out.content, "no files match `**/*.zig`");
    }

    #[test]
    fn changing_a_file_is_undoable_now_that_something_actually_keeps_the_old_version() {
        // These shipped as `Permanent` with a note saying it was a statement about Epoch, not
        // about filesystems. The journal made it false, so the declaration changed — the fact
        // moved first. A reversal may only be declared when it is true; `Descriptor::problems`
        // is what stops it being declared when it is not.
        let t = Tree::new("writerisk");
        for d in [
            WriteFile::new(t.root(), t.undo()).describe(),
            EditFile::new(t.root(), t.undo()).describe(),
        ] {
            assert!(!d.is_observation(), "{}", d.id);
            assert!(!d.reversal.is_permanent(), "{}", d.id);
            // Low rather than Medium, and this is what makes "Accept edits" reasonable instead
            // of an act of faith.
            assert_eq!(d.risk(), Risk::Low, "{}", d.id);
            assert!(d.problems().is_empty(), "{:?}", d.problems());
        }
    }

    #[test]
    fn an_exact_edit_replaces_only_what_was_named() {
        let t = Tree::new("edit");
        let out = EditFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text("src/main.rs")),
                ("old", text("three")),
                ("new", text("THREE")),
            ]))
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(t.0.join("src/main.rs")).unwrap(),
            "one\ntwo\nTHREE\nfour\nfive\n"
        );
        let made = out.evidence.expect("editing leaves something behind");
        assert_eq!(made.summary, "edited src/main.rs");
        // The file itself, not only a sentence about it.
        assert_eq!(made.reference, "src/main.rs");
    }

    #[test]
    fn an_ambiguous_edit_is_refused_rather_than_guessed() {
        // "The first one" is a guess about which line somebody meant. Refusing and saying how
        // to disambiguate is something the model can act on.
        let t = Tree::new("ambiguous");
        std::fs::write(t.0.join("src/dup.rs"), "x = 1\ny = 2\nx = 1\n").unwrap();
        let err = EditFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text("src/dup.rs")),
                ("old", text("x = 1")),
                ("new", text("x = 9")),
            ]))
            .unwrap_err();
        assert!(matches!(err, CapabilityError::Failed(ref why) if why.contains("appears 2 times")));
        // And nothing happened.
        assert_eq!(
            std::fs::read_to_string(t.0.join("src/dup.rs")).unwrap(),
            "x = 1\ny = 2\nx = 1\n"
        );
    }

    #[test]
    fn text_that_is_not_there_says_how_to_get_it_right() {
        let t = Tree::new("missing");
        let err = EditFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text("src/main.rs")),
                ("old", text("nowhere in the file")),
                ("new", text("x")),
            ]))
            .unwrap_err();
        assert!(matches!(err, CapabilityError::Failed(ref why) if why.contains("copy the lines")));
    }

    #[test]
    fn the_approval_shows_the_change_rather_than_describing_it() {
        // The difference between a description and evidence: only one lets somebody notice
        // that the wrong thing is about to happen.
        let t = Tree::new("preview");
        let explained = EditFile::new(t.root(), t.undo())
            .explain(&args(&[
                ("path", text("src/main.rs")),
                ("old", text("three")),
                ("new", text("THREE")),
            ]))
            .unwrap();

        assert_eq!(explained.what, "edit `src/main.rs`");
        assert_eq!(explained.preview.as_deref(), Some("- three\n+ THREE"));
    }

    #[test]
    fn an_edit_that_could_not_happen_is_never_offered_for_approval() {
        // Nobody should be asked to approve something that would fail. Located during
        // `explain`, by the same code `run` uses, so the two cannot describe different edits.
        let t = Tree::new("noapproval");
        let err = EditFile::new(t.root(), t.undo())
            .explain(&args(&[
                ("path", text("src/main.rs")),
                ("old", text("not present")),
                ("new", text("x")),
            ]))
            .unwrap_err();
        assert!(matches!(err, CapabilityError::Failed(_)));
    }

    #[test]
    fn both_ways_of_changing_a_file_leave_something_to_undo_with() {
        let t = Tree::new("journalled");
        WriteFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text("new.md")),
                ("content", text("hello\n")),
            ]))
            .unwrap();
        EditFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text("src/main.rs")),
                ("old", text("one")),
                ("new", text("ONE")),
            ]))
            .unwrap();

        let journal = t.journal();
        assert_eq!(journal.len(), 2);
        // Creating records no `before`, so undoing it removes the file rather than writing an
        // empty one.
        let entries = format!("{journal:?}");
        assert!(entries.contains("created new.md"));
        assert!(entries.contains("edited src/main.rs"));

        // And the most recent is the one undo would reach first.
        assert_eq!(journal.last().unwrap().summary, "edited src/main.rs");
        assert_eq!(
            journal.last().unwrap().before.as_deref(),
            Some("one\ntwo\nthree\nfour\nfive\n")
        );
    }

    #[test]
    fn creating_and_replacing_are_different_sentences() {
        // The whole point of explaining first is that the sentence is true. "Create" and
        // "replace 5 lines" are not the same decision, and the difference is checked against
        // the real path rather than guessed from the string.
        let t = Tree::new("writeexplain");
        let writer = WriteFile::new(t.root(), t.undo());

        let creating = writer
            .explain(&args(&[
                ("path", text("src/new.rs")),
                ("content", text("a\nb")),
            ]))
            .unwrap();
        assert_eq!(creating.what, "create `src/new.rs` — 2 lines");

        let replacing = writer
            .explain(&args(&[
                ("path", text("src/main.rs")),
                ("content", text("only this")),
            ]))
            .unwrap();
        assert_eq!(replacing.what, "replace `src/main.rs` — 5 lines become 1");
    }

    #[test]
    fn a_write_lands_and_leaves_evidence() {
        let t = Tree::new("write");
        let out = WriteFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text("src/new.rs")),
                ("content", text("fn go() {}\n")),
            ]))
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(t.0.join("src/new.rs")).unwrap(),
            "fn go() {}\n"
        );
        // Unlike reading, this produced something. That is what makes History non-fictional.
        let made = out.evidence.expect("writing leaves something behind");
        assert_eq!(made.summary, "created src/new.rs");
        assert_eq!(made.reference, "src/new.rs");
    }

    #[test]
    fn a_write_makes_the_folders_it_needs() {
        // A model writing `src/thing/mod.rs` into a project with no `thing` folder is doing
        // ordinary work. Refusing would send it round the loop asking for a tool we do not
        // offer.
        let t = Tree::new("mkdir");
        WriteFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text("src/deep/inner/mod.rs")),
                ("content", text("//\n")),
            ]))
            .unwrap();
        assert!(t.0.join("src/deep/inner/mod.rs").is_file());
    }

    #[test]
    fn a_write_outside_the_project_is_refused_by_the_same_gate_as_a_read() {
        let t = Tree::new("writeescape");
        let err = WriteFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text("../escape.rs")),
                ("content", text("x")),
            ]))
            .unwrap_err();
        assert!(matches!(err, CapabilityError::Refused(ref why) if why.contains("outside")));
        // And a sensitive name is refused even inside it.
        let err = WriteFile::new(t.root(), t.undo())
            .run(&args(&[
                ("path", text(".env")),
                ("content", text("SECRET=1")),
            ]))
            .unwrap_err();
        assert!(
            matches!(err, CapabilityError::Refused(ref why) if why.contains("Epoch will touch"))
        );
    }

    #[test]
    fn every_explanation_names_the_actual_call() {
        // What the user approves must be the thing that runs, in words they can read.
        let t = Tree::new("explain");
        assert_eq!(
            ReadFile::new(t.root())
                .explain(&args(&[("path", text("src/main.rs"))]))
                .unwrap()
                .what,
            "read `src/main.rs`"
        );
        assert_eq!(
            ListFiles::new(t.root())
                .explain(&Arguments::new())
                .unwrap()
                .what,
            "list the project root"
        );
        assert_eq!(
            FindFiles::new(t.root())
                .explain(&args(&[("pattern", text("**/*.rs"))]))
                .unwrap()
                .what,
            "find files matching `**/*.rs`"
        );
    }

    #[test]
    fn bad_arguments_are_caught_before_the_filesystem_is_touched() {
        let t = Tree::new("badargs");
        let reader = ReadFile::new(t.root());
        assert!(matches!(
            reader.run(&Arguments::new()),
            Err(CapabilityError::BadArguments(_))
        ));
        // An argument nobody declared is refused rather than ignored: the model believes it
        // asked for something.
        assert!(matches!(
            reader.run(&args(&[
                ("path", text("src/main.rs")),
                ("recursive", Value::Boolean(true))
            ])),
            Err(CapabilityError::BadArguments(_))
        ));
    }
}
