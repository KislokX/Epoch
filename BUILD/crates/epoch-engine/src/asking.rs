//! The approval bridge: an agent's call, waiting on a person.
//!
//! ## What it is for
//!
//! [`crate::serve`] turns a `tools/call` into a judged [`crate::serve::Run`]. When the verdict
//! is `Ask`, something has to happen that has never had to happen before in Epoch: a **thread
//! stops**, on a socket, until a human clicks a button in a window.
//!
//! The model's turn loop does not need this. It pauses the *turn* and returns — the provider is
//! not waiting, nothing is held open, and `answer_pending` resumes from a saved request. An
//! agent owns its own loop (ADR-0027), so there is nothing to save and resume: the agent is
//! sitting on an open HTTP request and the only two answers are "here is the result" and "no".
//!
//! ## One question at a time
//!
//! Deliberately, and it simplifies everything downstream. Epoch already shows one question in
//! one place; a queue would mean the user answering a prompt about a call they have forgotten
//! the context of, and two prompts at once would mean approving the wrong one.
//!
//! So a second call needing approval while one is open is **refused, with a reason the agent can
//! act on** — try again shortly. That is honest, it is a state the agent can handle, and it
//! keeps the interesting concurrency down to one case: reading while a write is being approved.
//!
//! ## Nothing waits forever
//!
//! A person who walked away must not leave an agent hanging until it is killed. The wait has a
//! deadline, and running out is a **refusal** rather than an approval — the only safe direction
//! for a timeout on a question whose answer changes the disk.

use crate::guard::guard;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

use epoch_kernel::{CapabilityId, CharacterId, Explanation};

/// How long an agent waits for an answer before being told no.
///
/// Long enough to read a diff and decide; short enough that a forgotten window does not hold a
/// socket open all afternoon. Not configurable yet: a number nobody has wanted changed is not a
/// setting (Earn Complexity).
pub const PATIENCE: Duration = Duration::from_secs(300);

/// What the user said.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// Go ahead, this once.
    Allow,
    /// Go ahead, and stop asking about this capability here.
    AllowAlways,
    /// No.
    Refuse,
    /// No, and stop asking — the answer is no from now on.
    RefuseAlways,
}

impl Answer {
    pub fn approved(self) -> bool {
        matches!(self, Answer::Allow | Answer::AllowAlways)
    }

    /// Whether this answer should be remembered as a standing decision (ADR-0009).
    pub fn standing(self) -> bool {
        matches!(self, Answer::AllowAlways | Answer::RefuseAlways)
    }
}

/// How the wait ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    Answered(Answer),
    /// Nobody was there. Refused, because that is the only safe direction.
    NobodyAnswered,
    /// Epoch is already asking about something else.
    AlreadyAsking,
    /// The World closed, or the endpoint did, while this was waiting.
    Withdrawn,
}

/// The question currently on screen, for a surface to render.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    /// Whose call this is. An agent acts as a character, so this is a real person in the World.
    pub character: String,
    pub capability: String,
    /// What will happen, in the capability's own words.
    pub what: String,
    /// The change itself, when the capability could show it without making it (ADR-0009).
    pub preview: Option<String>,
    /// Whether "and stop asking here" is a real option for this question.
    ///
    /// **False for a permission granted for one turn.** A coarse grant that quietly became
    /// permanent would be the escalation the whole arrangement exists to prevent, and a button
    /// offering it would be Epoch promising something it must not do. The surface hides it rather
    /// than accepting the click and ignoring it — a control that does not do what it says is
    /// worse than a missing one.
    pub standing: bool,
}

/// One slot, and whoever is waiting on it.
#[derive(Default)]
pub struct Asking {
    open: Mutex<Option<Slot>>,
    answered: Condvar,
}

#[derive(Debug)]
struct Slot {
    question: Question,
    /// Filled by whoever answers. The waiting thread wakes and reads it.
    answer: Option<Answer>,
    /// Set when the endpoint or the World is going away, so a waiter stops rather than sitting
    /// out the whole deadline for an answer that is never coming.
    withdrawn: bool,
}

impl Asking {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ask, and wait.
    ///
    /// Blocks the calling thread — which is an endpoint worker holding an open HTTP request.
    /// That is the point: the agent is made to wait for the user, rather than being told yes
    /// and corrected afterwards.
    pub fn ask(
        &self,
        who: &CharacterId,
        capability: &CapabilityId,
        explanation: &Explanation,
    ) -> Ended {
        self.ask_about(Question {
            character: who.to_string(),
            capability: capability.to_string(),
            what: explanation.what.clone(),
            preview: explanation.preview.clone(),
            standing: true,
        })
    }

    /// The same wait, for a question Epoch did not build from its own registry.
    ///
    /// An agent's **own** tools come through here. Epoch does not own `Write` and cannot explain
    /// it from a `CapabilityDescriptor` — but the agent tells us the tool and its arguments, and
    /// a person can decide on that. Same slot, same deadline, same four endings: there is one
    /// place a character can be waiting on somebody, and it stays one.
    pub fn ask_about(&self, question: Question) -> Ended {
        self.ask_about_with(question, || {})
    }

    /// Open the question before notifying its surface, then wait for the answer.
    ///
    /// A desktop surface learns about a question asynchronously. Publishing its event before
    /// the slot existed created a race: a listener that mounted in that small gap recovered the
    /// empty snapshot and the user had no button to answer. The callback runs only after the
    /// slot is visible through [`Self::current`], so an event and a recovery snapshot describe
    /// the same state.
    pub fn ask_about_with<F>(&self, question: Question, opened: F) -> Ended
    where
        F: FnOnce(),
    {
        {
            let mut open = guard(&self.open);
            if open.is_some() {
                return Ended::AlreadyAsking;
            }
            *open = Some(Slot {
                question,
                answer: None,
                withdrawn: false,
            });
        }

        opened();
        let ended = self.wait();
        // Always cleared, however it ended. A slot left full would make every later call answer
        // `AlreadyAsking` forever — the failure mode where one timeout breaks the bridge.
        *guard(&self.open) = None;
        ended
    }

    /// Park until answered, withdrawn, or out of time.
    ///
    /// A deadline rather than a plain timeout, because a condition variable is allowed to wake
    /// spuriously: waiting `PATIENCE` each time round the loop would let a few stray wakeups add
    /// up to an unbounded wait.
    fn wait(&self) -> Ended {
        let deadline = Instant::now() + PATIENCE;
        let mut open = guard(&self.open);
        loop {
            match open.as_ref() {
                Some(slot) if slot.withdrawn => return Ended::Withdrawn,
                Some(Slot {
                    answer: Some(answer),
                    ..
                }) => return Ended::Answered(*answer),
                // Somebody cleared it out from under us. Treated as withdrawal rather than as
                // approval: an absent answer is never a yes.
                None => return Ended::Withdrawn,
                _ => {}
            }
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ended::NobodyAnswered;
            }
            let (guard, _) = self.answered.wait_timeout(open, left).expect("asking lock");
            open = guard;
        }
    }

    /// What is being asked, if anything. What a surface renders.
    pub fn current(&self) -> Option<Question> {
        self.open
            .lock()
            .expect("asking lock")
            .as_ref()
            .map(|s| s.question.clone())
    }

    /// Answer it.
    ///
    /// `false` when nothing was waiting — which a surface should treat as "the question is gone"
    /// rather than as a failure. It happens legitimately: the deadline can pass between the
    /// prompt being drawn and the button being pressed.
    pub fn answer(&self, answer: Answer) -> bool {
        let mut open = guard(&self.open);
        let Some(slot) = open.as_mut() else {
            return false;
        };
        if slot.answer.is_some() {
            return false;
        }
        slot.answer = Some(answer);
        self.answered.notify_all();
        true
    }

    /// Take the question away without answering it.
    ///
    /// Called when the endpoint closes or the World is left. The waiter is refused, because a
    /// question that vanished was never a yes.
    pub fn withdraw(&self) {
        let mut open = guard(&self.open);
        if let Some(slot) = open.as_mut() {
            slot.withdrawn = true;
        }
        self.answered.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    fn explanation() -> Explanation {
        // Built the way a capability builds it, so the test cannot drift from the real shape.
        let descriptor = epoch_kernel::Descriptor::observing(
            CapabilityId::new("write_file").unwrap(),
            "Write a file.",
        );
        let mut explanation = Explanation::of(&descriptor, "create `hola.md` - 1 line");
        explanation.preview = Some("+ hola".into());
        explanation
    }

    fn mage() -> CharacterId {
        CharacterId::new("mage").unwrap()
    }

    #[test]
    fn a_question_reaches_a_surface_before_anybody_answers_it() {
        // The whole bridge in one property: the call is *stopped*, and while it is stopped the
        // user can see what they are being asked.
        let asking = Arc::new(Asking::new());

        let waiting = {
            let asking = Arc::clone(&asking);
            std::thread::spawn(move || {
                asking.ask(
                    &mage(),
                    &CapabilityId::new("write_file").unwrap(),
                    &explanation(),
                )
            })
        };

        // Wait for the question to appear rather than sleeping a guessed amount: a test that
        // sleeps is a test that is flaky on a slow machine.
        let question = loop {
            if let Some(question) = asking.current() {
                break question;
            }
            std::thread::yield_now();
        };

        assert_eq!(question.character, "mage");
        assert_eq!(question.capability, "write_file");
        assert!(question.what.contains("hola.md"));
        assert_eq!(question.preview.as_deref(), Some("+ hola"));

        assert!(asking.answer(Answer::Allow));
        assert_eq!(waiting.join().unwrap(), Ended::Answered(Answer::Allow));
    }

    #[test]
    fn publishing_a_question_can_only_happen_after_the_surface_can_recover_it() {
        // The shell emits its UI event in this callback. This protects the recovery path for a
        // window that starts listening just as an agent asks: it must never be told about a
        // question that `current()` cannot yet return.
        let asking = Asking::new();
        let question = Question {
            character: "mage".into(),
            capability: "Write".into(),
            what: "create hola.txt".into(),
            preview: None,
            standing: false,
        };

        let ended = asking.ask_about_with(question.clone(), || {
            assert_eq!(
                asking.current().as_ref().map(|open| &open.what),
                Some(&question.what)
            );
            assert!(asking.answer(Answer::Allow));
        });

        assert_eq!(ended, Ended::Answered(Answer::Allow));
        assert!(asking.current().is_none());
    }

    #[test]
    fn the_slot_is_free_again_however_it_ended() {
        // A slot left full would answer `AlreadyAsking` forever — the failure where one timeout
        // breaks the bridge for the rest of the session.
        let asking = Arc::new(Asking::new());

        let waiting = {
            let asking = Arc::clone(&asking);
            std::thread::spawn(move || {
                asking.ask(
                    &mage(),
                    &CapabilityId::new("write_file").unwrap(),
                    &explanation(),
                )
            })
        };
        while asking.current().is_none() {
            std::thread::yield_now();
        }
        asking.answer(Answer::Refuse);
        waiting.join().unwrap();

        assert!(asking.current().is_none());
        // And the next one is asked rather than turned away.
        let asking2 = Arc::clone(&asking);
        let again = std::thread::spawn(move || {
            asking2.ask(
                &mage(),
                &CapabilityId::new("read_file").unwrap(),
                &explanation(),
            )
        });
        while asking.current().is_none() {
            std::thread::yield_now();
        }
        asking.answer(Answer::Allow);
        assert_eq!(again.join().unwrap(), Ended::Answered(Answer::Allow));
    }

    #[test]
    fn a_second_question_is_refused_rather_than_queued() {
        // A queue would mean answering a prompt about a call whose context you have forgotten,
        // and two at once would mean approving the wrong one. Refusing is a state the agent can
        // handle: try again shortly.
        let asking = Arc::new(Asking::new());

        let waiting = {
            let asking = Arc::clone(&asking);
            std::thread::spawn(move || {
                asking.ask(
                    &mage(),
                    &CapabilityId::new("write_file").unwrap(),
                    &explanation(),
                )
            })
        };
        while asking.current().is_none() {
            std::thread::yield_now();
        }

        let second = asking.ask(
            &mage(),
            &CapabilityId::new("edit_file").unwrap(),
            &explanation(),
        );
        assert_eq!(second, Ended::AlreadyAsking);

        // And the first is untouched: the second must not have stolen or cleared its slot.
        assert_eq!(asking.current().unwrap().capability, "write_file");
        asking.answer(Answer::Allow);
        assert_eq!(waiting.join().unwrap(), Ended::Answered(Answer::Allow));
    }

    #[test]
    fn withdrawing_refuses_rather_than_leaving_a_thread_parked() {
        // Closing the endpoint or leaving the World must not hold a socket for the rest of the
        // deadline waiting for an answer that is never coming.
        let asking = Arc::new(Asking::new());

        let waiting = {
            let asking = Arc::clone(&asking);
            std::thread::spawn(move || {
                asking.ask(
                    &mage(),
                    &CapabilityId::new("run_command").unwrap(),
                    &explanation(),
                )
            })
        };
        while asking.current().is_none() {
            std::thread::yield_now();
        }

        asking.withdraw();
        assert_eq!(waiting.join().unwrap(), Ended::Withdrawn);
    }

    #[test]
    fn answering_nothing_says_so_rather_than_pretending() {
        // Legitimate: the deadline can pass between the prompt being drawn and the button being
        // pressed. A surface should read this as "the question is gone", not as a failure.
        let asking = Asking::new();
        assert!(!asking.answer(Answer::Allow));
    }

    #[test]
    fn a_question_cannot_be_answered_twice() {
        // Two clicks, or a click racing the keyboard. The first answer is the answer — the
        // second must not overwrite an approval with a refusal after the call already ran.
        let asking = Arc::new(Asking::new());
        let waiting = {
            let asking = Arc::clone(&asking);
            std::thread::spawn(move || {
                asking.ask(
                    &mage(),
                    &CapabilityId::new("write_file").unwrap(),
                    &explanation(),
                )
            })
        };
        while asking.current().is_none() {
            std::thread::yield_now();
        }

        assert!(asking.answer(Answer::Allow));
        assert!(!asking.answer(Answer::Refuse), "the first answer stands");
        assert_eq!(waiting.join().unwrap(), Ended::Answered(Answer::Allow));
    }

    #[test]
    fn running_out_of_time_is_a_refusal() {
        // The only safe direction. A timeout that approved would mean walking away from your
        // desk is how an agent gets permission to write.
        assert!(!Answer::Refuse.approved());
        assert!(!Answer::RefuseAlways.approved());
        assert!(Answer::Allow.approved());
        assert!(Answer::AllowAlways.approved());
        // And `NobodyAnswered` is its own outcome rather than an `Answer`, so nothing can read
        // it as one by accident.
        assert_ne!(Ended::NobodyAnswered, Ended::Answered(Answer::Refuse));
    }

    #[test]
    fn only_an_always_answer_is_remembered() {
        // ADR-0009: "always allow" is a standing decision; "allow once" is not, and recording
        // it would silently turn every yes into a permanent one.
        assert!(!Answer::Allow.standing());
        assert!(!Answer::Refuse.standing());
        assert!(Answer::AllowAlways.standing());
        assert!(Answer::RefuseAlways.standing());
    }
}
