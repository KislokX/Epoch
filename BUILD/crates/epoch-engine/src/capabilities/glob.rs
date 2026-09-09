//! A small glob matcher.
//!
//! Not a dependency, because what is needed here is small, and the alternative is a crate whose
//! semantics we would have to learn, document and then explain when they surprise somebody.
//! What we need is exactly what people already type: `*.rs`, `src/**/*.toml`, `**/test_*`.
//!
//! Deliberately absent: character classes, braces, negation, `extglob`. They are not in the
//! patterns a model produces, and every one of them is a way for a pattern to mean something
//! other than it looks like it means.
//!
//! Always matched against a **relative, forward-slashed** path, so a pattern behaves the same
//! on every machine. The caller normalises before asking.

/// Whether a relative path matches a pattern.
///
/// `*` matches within one segment, `?` matches one character within one segment, and `**`
/// matches any number of whole segments including none.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let path: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    segments(&pattern, &path)
}

fn segments(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.first() {
        // The pattern is used up: so must the path be.
        None => path.is_empty(),
        Some(&"**") => {
            // Zero or more whole segments. Try every split, shortest first — the pattern is a
            // handful of segments long, so the branching is not worth being clever about.
            (0..=path.len()).any(|skip| segments(&pattern[1..], &path[skip..]))
        }
        Some(first) => match path.first() {
            Some(head) if segment(first, head) => segments(&pattern[1..], &path[1..]),
            _ => false,
        },
    }
}

/// One segment against one segment. `*` and `?` never cross a `/`.
fn segment(pattern: &str, name: &str) -> bool {
    // Both sides as chars: a pattern is authored text and a filename can be anything, so
    // indexing by byte would split a multi-byte character and match nonsense.
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    walk(&p, &n)
}

fn walk(pattern: &[char], name: &[char]) -> bool {
    match pattern.first() {
        None => name.is_empty(),
        Some('*') => {
            // `*` is greedy-agnostic: try consuming nothing, then one more character each time.
            (0..=name.len()).any(|skip| walk(&pattern[1..], &name[skip..]))
        }
        Some('?') => !name.is_empty() && walk(&pattern[1..], &name[1..]),
        Some(c) => match name.first() {
            // Case-insensitive, because the filesystems this runs on are. A pattern that finds
            // a file on the author's machine and not on the user's is a bug report nobody can
            // reproduce.
            Some(head) if head.eq_ignore_ascii_case(c) => walk(&pattern[1..], &name[1..]),
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bare_name_matches_itself_and_nothing_else() {
        assert!(matches("main.rs", "main.rs"));
        assert!(!matches("main.rs", "src/main.rs"));
        assert!(!matches("main.rs", "main.rst"));
    }

    #[test]
    fn a_star_stays_inside_one_segment() {
        assert!(matches("*.rs", "main.rs"));
        assert!(matches("src/*.rs", "src/main.rs"));
        // The one that matters: `*` must not swallow a separator, or `*.rs` would reach into
        // every folder and a pattern would quietly mean far more than it says.
        assert!(!matches("*.rs", "src/main.rs"));
        assert!(!matches("src/*.rs", "src/deep/main.rs"));
    }

    #[test]
    fn a_double_star_crosses_segments_including_none() {
        assert!(matches("**/*.rs", "main.rs"), "zero segments still matches");
        assert!(matches("**/*.rs", "src/main.rs"));
        assert!(matches("**/*.rs", "crates/engine/src/main.rs"));
        assert!(matches("src/**/*.toml", "src/a/b/c.toml"));
        assert!(matches("src/**", "src/a/b/c.toml"));
    }

    #[test]
    fn a_question_mark_is_exactly_one_character() {
        assert!(matches("v?.rs", "v1.rs"));
        assert!(!matches("v?.rs", "v10.rs"));
        assert!(!matches("v?.rs", "v.rs"));
    }

    #[test]
    fn matching_ignores_case_because_the_filesystem_does() {
        assert!(matches("*.RS", "main.rs"));
        assert!(matches("SRC/*.rs", "src/main.rs"));
    }

    #[test]
    fn dot_dot_is_an_ordinary_segment_here_and_this_is_not_the_boundary() {
        // `**` matches any segments, `..` included — and that is correct, because this matcher
        // is not a security boundary and must not be mistaken for one.
        //
        // Paths reaching it have already been through `ProjectRoot::confine`, which resolves
        // against the real filesystem. Nothing here decides what may be touched; it only
        // decides what a pattern means. Asserted so a future reader does not add a `..` check
        // and conclude the walk is safe because of it.
        assert!(matches("**/*.rs", "../main.rs"));
        assert!(matches("../*.rs", "../main.rs"));
        // It is a plain segment: a pattern that does not mention it does not match it.
        assert!(!matches("*.rs", "../main.rs"));
    }

    #[test]
    fn stars_next_to_each_other_do_not_hang() {
        // The classic catastrophic-backtracking shape. Small and bounded here, but asserted so
        // a future rewrite cannot quietly reintroduce it.
        assert!(matches("****.rs", "main.rs"));
        assert!(!matches("a*a*a*a*a*b", "aaaaaaaaaaaaaaaaaaaaaaaaaaaa"));
    }
}
