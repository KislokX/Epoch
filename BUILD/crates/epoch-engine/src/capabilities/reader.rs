//! Draining a child's pipes without letting one line become the memory limit.
//!
//! Its own file because the fix is a *bounded* read, and a bounded read cannot be written
//! against a `Box<dyn BufRead>`: both `take` and `by_ref` want `Sized`, and a boxed trait object
//! is not. Making the reader generic is the whole change, and it does not fit inside the enum it
//! replaced.

use std::io::{BufRead, Read};
use std::sync::mpsc;

/// The most bytes one line may be before it is passed on as one anyway.
///
/// **The cap on what is kept was never a cap on what is held.** `split(b'\n')` reads until a
/// newline arrives, however long that takes and however much it costs — a program printing a
/// megabyte without one (a progress bar redrawing with `\r`, a minified bundle, a binary
/// accidentally sent to stdout) is a megabyte in memory before this code sees a byte of it.
/// Found by the second audit, 2026-09-07, finding 5.
///
/// A line past this is **split rather than dropped**. Losing the rest would make the output lie
/// about what a program printed, and this is a Terminal somebody reads.
pub(super) const MAX_LINE: usize = 8 * 1024;

/// Post one line at a time, holding at most [`MAX_LINE`] of any of them.
///
/// Lossy on purpose: a build that prints one invalid byte should not lose the rest of its output
/// to an encoding error.
pub(super) fn post_lines(mut reader: impl BufRead, post: &mpsc::SyncSender<String>) {
    let mut bytes = Vec::new();
    loop {
        bytes.clear();
        // Bounded per line rather than per read: `read_until` on a limited reader stops at the
        // newline or at the cap, whichever comes first.
        let read = match reader
            .by_ref()
            .take(MAX_LINE as u64)
            .read_until(b'\n', &mut bytes)
        {
            Ok(read) => read,
            Err(_) => break,
        };
        if read == 0 {
            break;
        }
        let text = String::from_utf8_lossy(&bytes)
            .trim_end_matches('\n')
            .trim_end_matches('\r')
            .to_owned();
        if post.send(text).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines_of(raw: &[u8]) -> Vec<String> {
        let (post, arriving) = mpsc::sync_channel(4096);
        post_lines(raw, &post);
        drop(post);
        arriving.into_iter().collect()
    }

    #[test]
    fn ordinary_output_arrives_a_line_at_a_time() {
        assert_eq!(lines_of(b"one\ntwo\n"), vec!["one", "two"]);
        // Windows line endings, and a last line with no newline at all.
        assert_eq!(lines_of(b"one\r\ntwo"), vec!["one", "two"]);
        assert!(lines_of(b"").is_empty());
    }

    /// **A program that never prints a newline must not be able to choose how much memory this
    /// holds.** The defect, as the thing that was being read: `split(b'\n')` on this input
    /// accumulates every byte before yielding anything.
    #[test]
    fn a_line_that_never_ends_is_split_rather_than_swallowed() {
        let endless = vec![b'x'; MAX_LINE * 3 + 17];
        let posted = lines_of(&endless);
        assert_eq!(posted.len(), 4, "three full lines and the remainder");
        for line in &posted {
            assert!(line.len() <= MAX_LINE, "nothing longer than the cap");
        }
        // And nothing is lost: what a program printed is what the Terminal shows.
        assert_eq!(posted.iter().map(String::len).sum::<usize>(), endless.len());
    }
}
