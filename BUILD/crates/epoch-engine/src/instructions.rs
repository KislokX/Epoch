//! What the project itself says about working in it.
//!
//! A repository often already carries instructions for an assistant: `AGENTS.md` is the
//! cross-tool convention, `CLAUDE.md` the common equivalent. Reading them means a project
//! already set up for another tool **works in Epoch without being touched** — which is a large
//! amount of usefulness for a very small amount of code.
//!
//! ## Root only, and the first match wins
//!
//! Not climbed to a parent repository, and not gathered from subdirectories. The project root
//! is a boundary the user chose (ADR-0025), and an instruction file outside it is not something
//! they pointed at.
//!
//! One file, not both, so there is never a question about which of two conflicting documents
//! won.
//!
//! ## Why this is trusted, and why it is still announced
//!
//! It goes into the system role, which everything else read from disk is forbidden from doing
//! (`epoch_kernel::conversation` — tool output is wrapped and never authoritative). That is
//! defensible: the user chose this folder deliberately, so its instructions have the same
//! standing as a character's own prompt.
//!
//! But "the user chose the folder" is doing real work in that sentence, and a cloned repository
//! is somebody else's writing. So loading is **never silent**: the file that was read is
//! reported, every time, so a set of instructions can never influence a character without the
//! user being able to see that it did.

use std::path::Path;

/// Checked at the project root, in order. First match wins.
pub const FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// The most that will be read.
///
/// Generous — a real `CLAUDE.md` is a few kilobytes — and finite, because this is prepended to
/// every single turn. A repository with a 2 MB instruction file would spend the whole context
/// window before the user's question arrived.
pub const MAX_BYTES: usize = 32 * 1024;

/// Instructions a project left for whoever works in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Instructions {
    /// Which file it came from. Shown, always — see the module docs.
    pub file: &'static str,
    pub body: String,
    /// True when the file was longer than [`MAX_BYTES`].
    ///
    /// Said rather than hidden: a model reasoning from half a document does it confidently.
    pub truncated: bool,
}

impl Instructions {
    /// What the model is told, framed so it knows where this came from.
    pub fn compose(&self) -> String {
        let head = format!(
            "PROJECT INSTRUCTIONS, from {} at the root of this project. The person who owns \
             this project left them for whoever works in it. Follow them.",
            self.file
        );
        if self.truncated {
            format!(
                "{head}\n(Only the first {MAX_BYTES} bytes are shown.)\n\n{}",
                self.body
            )
        } else {
            format!("{head}\n\n{}", self.body)
        }
    }
}

/// Read the project's own instructions, if it has any.
///
/// Best effort in every direction: an unreadable file, a folder named `AGENTS.md`, a file of
/// nothing but whitespace — all produce `None`. A project that cannot state its conventions is
/// an ordinary project, not a broken one.
pub fn read(root: &Path) -> Option<Instructions> {
    for file in FILES {
        let path = root.join(file);
        if !path.is_file() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        // Lossy rather than refusing: an instruction file with one bad byte in it is still
        // instructions, and dropping the whole thing would be a worse answer than a mangled
        // character.
        let text = String::from_utf8_lossy(&bytes);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            continue;
        }

        let truncated = trimmed.len() > MAX_BYTES;
        let body = if truncated {
            // Cut on a character boundary — `String::truncate` panics otherwise, and a panic
            // reading a text file is not a way to fail.
            let mut end = MAX_BYTES;
            while end > 0 && !trimmed.is_char_boundary(end) {
                end -= 1;
            }
            trimmed[..end].to_owned()
        } else {
            trimmed.to_owned()
        };

        return Some(Instructions {
            file,
            body,
            truncated,
        });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tree(std::path::PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("epoch-instructions-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(d.join("src")).unwrap();
            Self(d)
        }
        fn write(&self, name: &str, body: &str) {
            std::fs::write(self.0.join(name), body).unwrap();
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_project_with_nothing_to_say_is_ordinary_not_broken() {
        let t = Tree::new("none");
        assert_eq!(read(&t.0), None);
    }

    #[test]
    fn agents_md_is_read_and_says_where_it_came_from() {
        // Never silent: a set of instructions must not be able to influence a character
        // without the user being able to see that it did.
        let t = Tree::new("agents");
        t.write("AGENTS.md", "# House rules\nMeasure twice.\n");
        let found = read(&t.0).unwrap();
        assert_eq!(found.file, "AGENTS.md");
        assert!(found.body.contains("Measure twice."));
        assert!(!found.truncated);
        assert!(found.compose().contains("from AGENTS.md"));
    }

    #[test]
    fn claude_md_works_too_and_agents_wins_when_both_are_there() {
        // One file, not both, so there is never a question about which of two conflicting
        // documents won.
        let t = Tree::new("both");
        t.write("CLAUDE.md", "from claude\n");
        assert_eq!(read(&t.0).unwrap().file, "CLAUDE.md");

        t.write("AGENTS.md", "from agents\n");
        let found = read(&t.0).unwrap();
        assert_eq!(found.file, "AGENTS.md");
        assert!(found.body.contains("from agents"));
    }

    #[test]
    fn an_empty_file_is_the_same_as_no_file() {
        // "The author wrote nothing" must not become an instruction block containing nothing.
        let t = Tree::new("empty");
        t.write("AGENTS.md", "   \n\n  ");
        assert_eq!(read(&t.0), None);
    }

    #[test]
    fn a_huge_file_is_cut_and_says_so() {
        // Prepended to every turn: a 2 MB instruction file would spend the context window
        // before the user's question arrived. And a model reasoning from half a document does
        // it confidently, so the cut is announced.
        let t = Tree::new("huge");
        t.write("AGENTS.md", &"x".repeat(MAX_BYTES * 2));
        let found = read(&t.0).unwrap();
        assert!(found.truncated);
        assert!(found.body.len() <= MAX_BYTES);
        assert!(found.compose().contains("first"));
    }

    #[test]
    fn a_folder_with_the_right_name_is_not_instructions() {
        let t = Tree::new("folder");
        std::fs::create_dir_all(t.0.join("AGENTS.md")).unwrap();
        assert_eq!(read(&t.0), None);
    }

    #[test]
    fn only_the_root_is_read() {
        // The project root is a boundary the user chose. An instruction file in a
        // subdirectory — or in a parent repository — is not something they pointed at.
        let t = Tree::new("root only");
        std::fs::write(t.0.join("src/AGENTS.md"), "deep instructions\n").unwrap();
        assert_eq!(read(&t.0), None);
    }
}
