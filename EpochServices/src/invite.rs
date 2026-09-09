//! The code this machine shows, when the Host has to be the one to reach out.
//!
//! ## Why this exists
//!
//! The ordinary way round is [`crate::door::redeem`]: this machine dials the Host, so nobody
//! types an IP. It needs this machine to be able to *open a connection to* the Host, and on a
//! real network that turned out not to be true — with this machine on Wi-Fi and the Host on
//! Ethernet, every connection it started was dropped (445, 135, 3389 and 11501 alike, with the
//! Host's firewall rule present and a listener confirmed up), while every connection the Host
//! started here succeeded. That is client isolation, and it belongs to a router somebody may
//! not own.
//!
//! So the same exchange runs backwards: **this machine shows the code and the Host reaches out**.
//!
//! ## The code is what makes that safe
//!
//! In this direction the listener is exposed to the network by design — anything on it can send
//! an `Enrol`. The code is the whole of the defence, so it is checked here, it lasts five minutes,
//! and it is spent the moment it works.

use std::time::{Duration, Instant};

/// How long a shown code lasts.
///
/// The same five minutes the Host uses in the other direction. It is long enough to walk to the
/// other machine and short enough that a code left on a screen stops meaning anything.
pub const LIFE: Duration = Duration::from_secs(300);

/// How many wrong guesses a code survives.
///
/// **A code being guessed at has stopped being a code.** Thirty-one characters and six of them
/// is a wide enough space that nobody guesses one by luck, and that is an argument about *one*
/// attempt: this listener is on the network by design, so the thing to bound is how many
/// attempts the space is worth. Ten is far more than a person mistypes and nothing at all as a
/// search. Burning it is safe because it costs the owner one button — the code was always
/// meant to be shown for the length of one walk between two machines.
pub const TRIES: u32 = 10;

/// No `0`, `O`, `1`, `I` or `L`. Somebody reads this off one screen and types it into another.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// A short code, alive for [`LIFE`].
#[derive(Debug, Clone)]
pub struct Code {
    code: String,
    made: Instant,
    wrong: u32,
}

impl Code {
    /// A fresh code from the operating system's randomness.
    pub fn fresh() -> Self {
        let mut bytes = [0u8; 6];
        getrandom::getrandom(&mut bytes).expect("the operating system must provide randomness");
        Self {
            code: bytes
                .iter()
                .map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char)
                .collect(),
            made: Instant::now(),
            wrong: 0,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.code
    }

    /// Whether this code is no longer an answer — out of time, or out of attempts.
    ///
    /// One question rather than two, because everything that reads it wants the same thing: the
    /// page stops offering it, and `attempt` stops matching it. A screen that kept showing a
    /// burnt code would be an instrument reporting an offer that no longer exists.
    pub fn expired(&self) -> bool {
        self.made.elapsed() >= LIFE || self.wrong >= TRIES
    }

    /// Seconds left, for a screen to count down with. Zero once it has run out.
    pub fn left(&self) -> u64 {
        LIFE.saturating_sub(self.made.elapsed()).as_secs()
    }

    /// Offer a code, and pay for being wrong.
    ///
    /// The counting is the point: [`matches`](Self::matches) answers the question and this is
    /// what makes asking it repeatedly cost something. After [`TRIES`] wrong answers the code is
    /// burnt and the owner shows a fresh one.
    pub fn attempt(&mut self, offered: &str) -> bool {
        if self.matches(offered) {
            return true;
        }
        self.wrong = self.wrong.saturating_add(1);
        false
    }

    /// Whether an offered code is this one.
    ///
    /// **Constant time**, and false once expired. A comparison that stops at the first wrong
    /// byte tells whoever is guessing how much of the code was right, one request at a time —
    /// and this listener is reachable from the whole network, which is exactly the situation
    /// that makes that worth caring about.
    pub fn matches(&self, offered: &str) -> bool {
        if self.expired() {
            return false;
        }
        let mine = self.code.as_bytes();
        let theirs = offered.trim().to_uppercase();
        let theirs = theirs.as_bytes();
        if mine.len() != theirs.len() {
            return false;
        }
        let mut wrong = 0u8;
        for (a, b) in mine.iter().zip(theirs) {
            wrong |= a ^ b;
        }
        wrong == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_six_characters_a_person_can_read_aloud() {
        let code = Code::fresh();
        assert_eq!(code.as_str().len(), 6);
        // Nothing that can be misread as something else. Somebody is copying this between two
        // machines, and `0` against `O` is the difference between working and not.
        for c in code.as_str().chars() {
            assert!(ALPHABET.contains(&(c as u8)), "{c} is easy to mistype");
            assert!(!"01ILO".contains(c), "{c} is easy to misread");
        }
    }

    #[test]
    fn it_is_not_case_sensitive_because_a_person_is_typing_it() {
        let code = Code::fresh();
        assert!(code.matches(&code.as_str().to_lowercase()));
        assert!(code.matches(&format!("  {}  ", code.as_str())));
    }

    #[test]
    fn a_wrong_code_is_refused_and_so_is_a_short_one() {
        let code = Code::fresh();
        assert!(!code.matches(""));
        assert!(!code.matches("ABC"));
        assert!(!code.matches(&format!("{}X", code.as_str())));
    }

    #[test]
    fn an_expired_code_matches_nothing_including_itself() {
        // The listener is reachable from the whole network in this direction, so a code that
        // outlived its five minutes has to stop being an answer.
        let mut code = Code::fresh();
        code.made = code
            .made
            .checked_sub(LIFE + Duration::from_secs(1))
            .expect("a clock that has been running a second");
        assert!(code.expired());
        assert!(!code.matches(code.as_str().to_owned().as_str()));
        assert_eq!(code.left(), 0);
    }

    #[test]
    fn a_code_that_is_being_guessed_at_stops_being_one() {
        let mut code = Code::fresh();
        for _ in 0..TRIES {
            assert!(!code.attempt("ZZZZZZ"));
        }
        assert!(code.expired(), "ten wrong answers burn it");
        // Including for whoever finally types it correctly. That is the trade, said plainly:
        // the owner presses the button again, and a search does not get to keep going.
        assert!(!code.attempt(code.as_str().to_owned().as_str()));
    }

    #[test]
    fn a_right_answer_costs_nothing() {
        let mut code = Code::fresh();
        for _ in 0..(TRIES * 3) {
            assert!(code.attempt(code.as_str().to_owned().as_str()));
        }
        assert!(!code.expired());
    }

    #[test]
    fn a_fresh_code_has_its_whole_life_left() {
        assert!(Code::fresh().left() > LIFE.as_secs() - 5);
    }
}
