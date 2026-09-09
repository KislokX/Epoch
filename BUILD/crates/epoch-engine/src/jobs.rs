//! Work that outlasts the turn (ADR-0034).
//!
//! ## What this is for
//!
//! `turn::perform` calls a capability and blocks until it returns. Everything above it blocks
//! with it: the round, the turn, the model's next word, and the window. That has been survivable
//! because almost nothing takes long — and measured on this machine, a 4K picture through a 4x
//! upscaler takes **152 s**, with the window frozen for all of it.
//!
//! A capability may answer [`crate::capability::Begun`] instead of a result. What it began is
//! recorded here, and the turn goes on.
//!
//! ## Why a Job carries its Quest and its character
//!
//! Taken **when it starts**, never read back from whatever the surface has selected when it
//! finishes. This is not a precaution: it is the defect ADR-0025 already recorded, where evidence
//! was filed against the *active* Quest and a tool returning while the user had opened another
//! conversation credited the wrong one — a `spotify_mcp_play` chip inside a Quest called
//! `npm view react version`. A job outlives its turn by minutes, which makes that the normal case
//! rather than a race.
//!
//! ## What is deliberately not here
//!
//! **No queue, no priorities, and no cancelling somebody else's work.** ADR-0034 refuses all
//! three for now: none has a motivating case, and the first real one will say what shape it
//! needs. What *is* here is the single-slot rule the approval bridge already has — one job per
//! character — because two jobs for one character is the same ambiguity `running` already solved.
//!
//! **Nothing survives a restart.** A job alive when Epoch closes is [`State::Interrupted`], and
//! that is the true state: a half-finished ComfyUI render cannot be resumed, because the server
//! was killed too. Writing a durable queue to claim otherwise would be a gauge with nothing
//! behind it.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use epoch_kernel::{CharacterId, QuestId};

/// What became of a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    /// Still going, and this is how long it has been.
    Working,
    /// It finished and left something behind.
    ///
    /// **Both halves, because they are read by different people.** `made` is the evidence a
    /// Quest keeps (ADR-0025) and `said` is the sentence the capability wrote — *Drew a picture
    /// in 13.1s*. The first version kept only the evidence, and a test noticed within the hour:
    /// the verb that separates *drew* from *changed* lived in the sentence, so the Chronicle
    /// would have had nothing to print but a filename.
    Made {
        made: crate::capability::Made,
        said: String,
    },
    /// It finished and left nothing — a measurement, a check that passed.
    Told(String),
    /// It failed, in its own words.
    Failed(String),
    /// Epoch closed while it was running.
    ///
    /// **Not resumed and not forgotten.** ADR-0025's failure states exist so History can say
    /// *this stopped* rather than keep only the successes.
    Interrupted,
}

impl State {
    /// Whether this job is still going. Everything else is an ending.
    pub const fn working(&self) -> bool {
        matches!(self, State::Working)
    }
}

/// One piece of work that outlived the turn that began it.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    /// Whose Quest this belongs to, taken when it started.
    pub quest: QuestId,
    /// Who began it, taken when it started.
    pub character: CharacterId,
    /// What it is, in the words a person reads: `drawing a picture`.
    pub what: String,
    /// What it is waiting on, so a job that never ends can be explained rather than guessed at:
    /// `ComfyUI at http://127.0.0.1:8188`.
    pub waiting_on: String,
    pub since: SystemTime,
    pub state: State,
}

impl Job {
    /// How long it has been going, or how long it took.
    pub fn how_long(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.since)
            .unwrap_or_default()
    }
}

/// Every job this session has begun.
///
/// In memory, deliberately: see the module note on why nothing survives a restart.
#[derive(Debug, Default)]
pub struct Jobs {
    inner: Mutex<BTreeMap<String, Job>>,
}

impl Jobs {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin one, unless this character already has work in flight.
    ///
    /// **One per character, and it is said out loud rather than discovered.** A character with a
    /// job running takes no new turn until it lands — the same single-slot constraint the
    /// approval bridge and `running` already have. The refusal names who is busy and what with,
    /// because *Mage is rendering* is something a person can act on and a silent no is not.
    pub fn begin(
        &self,
        id: &str,
        quest: &QuestId,
        character: &CharacterId,
        what: &str,
        waiting_on: &str,
    ) -> Result<Job, String> {
        let mut held = self.inner.lock().map_err(|_| "the job list is poisoned")?;
        if let Some(busy) = held
            .values()
            .find(|job| &job.character == character && job.state.working())
        {
            return Err(format!(
                "{} is already {} and has been for {}s",
                character.as_str(),
                busy.what,
                busy.how_long().as_secs()
            ));
        }
        let job = Job {
            id: id.to_owned(),
            quest: quest.clone(),
            character: character.clone(),
            what: what.to_owned(),
            waiting_on: waiting_on.to_owned(),
            since: SystemTime::now(),
            state: State::Working,
        };
        held.insert(id.to_owned(), job.clone());
        Ok(job)
    }

    /// It landed. Answers the job as it was, so the caller knows whose Quest to file against.
    ///
    /// **The Quest comes from here and never from the surface.** That is the whole reason this
    /// type exists rather than a bare thread.
    pub fn finish(&self, id: &str, state: State) -> Option<Job> {
        let mut held = self.inner.lock().ok()?;
        let job = held.get_mut(id)?;
        job.state = state;
        Some(job.clone())
    }

    /// What is going on right now, oldest first.
    pub fn working(&self) -> Vec<Job> {
        let Ok(held) = self.inner.lock() else {
            return Vec::new();
        };
        let mut going: Vec<Job> = held
            .values()
            .filter(|job| job.state.working())
            .cloned()
            .collect();
        going.sort_by_key(|job| job.since);
        going
    }

    /// What this character is doing, if anything.
    pub fn what(&self, character: &CharacterId) -> Option<Job> {
        let held = self.inner.lock().ok()?;
        held.values()
            .find(|job| &job.character == character && job.state.working())
            .cloned()
    }

    /// Epoch is closing. Everything still going stopped, and says so.
    ///
    /// Answers what was interrupted so History can record it — a Quest whose work was killed
    /// mid-flight has to say that rather than simply have nothing.
    pub fn interrupt_everything(&self) -> Vec<Job> {
        let Ok(mut held) = self.inner.lock() else {
            return Vec::new();
        };
        let mut stopped = Vec::new();
        for job in held.values_mut().filter(|job| job.state.working()) {
            job.state = State::Interrupted;
            stopped.push(job.clone());
        }
        stopped
    }
}

/// Whether the graphics card is somebody else's to take.
///
/// ## Why this, and not "wait for the turn to end"
///
/// The first version counted turns in flight, on the reasoning that the turn has one model call
/// left in it — the sentence saying the work has begun — and starting a render underneath it
/// takes the card away. That reasoning was right and the *condition* was wrong, and the window
/// said so three times:
///
/// - measured, a cold `gemma4:12b` made one turn take **901 s**. The job's patience expired long
///   before, the render went ahead anyway, and the overlap was back.
/// - and while a turn is that long, a character waiting on a job never shows as waiting either,
///   because the state that says so is set when the turn ends.
///
/// The turn ending was a *proxy* for the thing that matters, and a bad one on a slow machine.
/// What the render actually needs is that **nothing else is holding the card** — which is a
/// question that can be asked rather than assumed (`/api/ps` answers it), and which is true at
/// the moment it becomes true rather than at the moment a turn happens to finish.
///
/// The shell owns the asking, because a capability knows nothing about providers (ADR-0008). It
/// sets this; the work waits on it.
static CARD_IS_FREE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Say whether anything is holding the card. Asked by the shell, which is the half that can ask.
pub fn the_card_is_free(free: bool) {
    CARD_IS_FREE.store(free, std::sync::atomic::Ordering::SeqCst);
}

/// Whether any work is waiting for the card right now.
///
/// The shell asks this before paying for a `/api/ps` round trip: with nothing waiting there is
/// nothing to answer for, and a probe every heartbeat would be a network call twice a second
/// forever.
static WAITING_FOR_THE_CARD: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

pub fn anything_waiting_for_the_card() -> bool {
    WAITING_FOR_THE_CARD.load(std::sync::atomic::Ordering::SeqCst) > 0
}

/// Wait until nothing else is holding the card, or until `patience` runs out.
///
/// **Bounded on purpose.** A model that never lets go would otherwise park the work for good, and
/// a picture drawn late is better than one never drawn — the same trade every other timeout in
/// this build makes.
pub fn wait_for_the_card(patience: Duration) {
    WAITING_FOR_THE_CARD.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let until = std::time::Instant::now() + patience;
    while !CARD_IS_FREE.load(std::sync::atomic::Ordering::SeqCst) {
        if std::time::Instant::now() >= until {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let _ = WAITING_FOR_THE_CARD.fetch_update(
        std::sync::atomic::Ordering::SeqCst,
        std::sync::atomic::Ordering::SeqCst,
        |had| Some(had.saturating_sub(1)),
    );
}

/// Where finished work waits to be collected.
///
/// ## Why a mailbox rather than a callback
///
/// A capability does the work and knows nothing about Quests, characters or the Activity Stream
/// (ADR-0008) — the layer above owns those. So the thread that finishes a render cannot file
/// evidence: it can only say *this id ended like this*, and leave the filing to the half that
/// knows whose work it was.
///
/// Process-wide, like the other one-of-a-kind facts in this build (`STUDIO_IS_OURS`, the panel's
/// remembered choice): there is one Engine in one process, and threading a handle through
/// `Capability::run` would put a Quest-shaped argument on a trait that ADR-0008 keeps clean.
static LANDED: std::sync::Mutex<Vec<(String, State)>> = std::sync::Mutex::new(Vec::new());

/// Work finished. Says how, under the name it was started with.
///
/// Called from the capability's own thread, minutes after the turn that began it ended.
pub fn land(id: &str, state: State) {
    if let Ok(mut held) = LANDED.lock() {
        held.push((id.to_owned(), state));
    }
}

/// Take everything that has landed since this was last asked. Empty is the ordinary answer.
///
/// **Drained rather than read**, so two callers cannot both file the same evidence — the failure
/// ADR-0025 named, arriving through a different door.
pub fn collect() -> Vec<(String, State)> {
    LANDED
        .lock()
        .map(|mut held| std::mem::take(&mut *held))
        .unwrap_or_default()
}

/// Wait for one named piece of work, taking only that one out of the box.
///
/// **For a caller that knows what it started**, which in practice is a test: `collect` drains
/// everything and two tests in one process would eat each other's results. The shell still
/// drains, because it files whatever landed and cares about all of it.
///
/// `None` if it has not landed within `patience`.
pub fn wait_for(id: &str, patience: Duration) -> Option<State> {
    let until = std::time::Instant::now() + patience;
    loop {
        if let Ok(mut held) = LANDED.lock() {
            if let Some(at) = held.iter().position(|(name, _)| name == id) {
                return Some(held.remove(at).1);
            }
        }
        if std::time::Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// What the model is told when a job has begun.
///
/// **Bounded and negative, and that shape is not a style.** ADR-0030's third amendment learned it
/// twice over: `see_image` ended its result with an invitation to ask again and got seven calls;
/// `draw_image` handed back the filename it had just written and got a forged Markdown image. A
/// started job has both hazards at once — a name that does not exist yet, and a gap a model will
/// fill — so this says what happened, says nothing exists, and does not invite another attempt.
pub fn told_it_started(what: &str) -> String {
    format!(
        "STARTED. {what} is under way and nothing has been made yet — no file exists and there is \
         nothing to describe. You will be told when it is done. Do not describe the result, do not \
         guess at it, and do not call this again for the same thing. Tell the user it has begun."
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn who(id: &str) -> CharacterId {
        CharacterId::new(id).expect("a character id")
    }

    fn quest(id: &str) -> QuestId {
        QuestId::from_raw(id)
    }

    /// **The Quest is taken when the work starts, and that is the whole point of the type.**
    ///
    /// ADR-0025 already recorded the defect this prevents: evidence filed against whatever the
    /// surface had selected, so a tool returning while the user had opened another conversation
    /// credited the wrong Quest. A job outlives its turn by minutes, which turns that race into
    /// the ordinary case.
    #[test]
    fn a_job_remembers_whose_work_it_is_from_the_first_instant() {
        let jobs = Jobs::new();
        let begun = jobs
            .begin(
                "j1",
                &quest("q-render"),
                &who("mage"),
                "drawing a picture",
                "ComfyUI",
            )
            .expect("it begins");
        assert_eq!(begun.quest, quest("q-render"));

        // Minutes later, and the user is looking at something else entirely.
        let landed = jobs
            .finish(
                "j1",
                State::Made {
                    made: crate::capability::Made {
                        reference: "a1b2c3.png".to_owned(),
                        summary: "a lighthouse".to_owned(),
                    },
                    said: "Drew a picture in 13.1s".to_owned(),
                },
            )
            .expect("it lands");
        assert_eq!(
            landed.quest,
            quest("q-render"),
            "evidence belongs to the Quest that asked, not to whatever is on screen now"
        );
    }

    /// One job per character, refused by name rather than silently.
    #[test]
    fn a_character_with_work_in_flight_is_told_so_and_told_what() {
        let jobs = Jobs::new();
        jobs.begin(
            "j1",
            &quest("q"),
            &who("mage"),
            "drawing a picture",
            "ComfyUI",
        )
        .expect("the first begins");

        let refused = jobs
            .begin(
                "j2",
                &quest("q"),
                &who("mage"),
                "drawing another",
                "ComfyUI",
            )
            .expect_err("the second is refused");
        assert!(refused.contains("mage"), "{refused}");
        assert!(refused.contains("drawing a picture"), "{refused}");

        // Somebody else is not busy because Mage is.
        jobs.begin("j3", &quest("q"), &who("paladin"), "measuring", "the card")
            .expect("a different character is free");

        // And when the first lands, that character is free again.
        jobs.finish("j1", State::Told("done".to_owned()));
        jobs.begin("j4", &quest("q"), &who("mage"), "drawing again", "ComfyUI")
            .expect("free once it landed");
    }

    /// **Interrupted, never quietly forgotten.**
    ///
    /// A half-finished render cannot be resumed — the server was killed too — so the honest state
    /// is that it stopped. ADR-0025's failure states exist precisely so History can say that
    /// instead of keeping only the successes.
    #[test]
    fn work_alive_when_epoch_closes_is_recorded_as_interrupted() {
        let jobs = Jobs::new();
        jobs.begin("j1", &quest("q"), &who("mage"), "drawing", "ComfyUI")
            .unwrap();
        jobs.begin("j2", &quest("q"), &who("paladin"), "measuring", "the card")
            .unwrap();
        jobs.finish("j2", State::Told("17.4 GB free".to_owned()));

        let stopped = jobs.interrupt_everything();
        assert_eq!(stopped.len(), 1, "only what was still going");
        assert_eq!(stopped[0].id, "j1");
        assert_eq!(stopped[0].state, State::Interrupted);
        assert!(jobs.working().is_empty());
    }

    /// **The thread that does the work cannot file the evidence, and must not try.**
    ///
    /// It knows an id and an outcome. Which Quest that belongs to is the registry's answer, and
    /// taking it from anywhere else is the defect ADR-0025 recorded.
    #[test]
    fn work_lands_by_name_and_is_collected_once() {
        // Drain whatever another test in this process left, so this measures its own.
        collect();

        land("j-far-away", State::Told("done".to_owned()));
        let first = collect();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].0, "j-far-away");
        assert!(
            collect().is_empty(),
            "collected twice is filed twice, which is how the wrong Quest gets credited"
        );
    }

    /// **Work waits for the card, and the condition is a measurement.**
    ///
    /// Waiting for the *turn* was the first answer: it has one model call left in it, and a
    /// render underneath takes the card away. Right about the danger, wrong about the condition —
    /// a cold model made one turn take 901 s, the patience expired, and the overlap came back
    /// while the character still showed as working rather than waiting.
    ///
    /// **One test, not two.** The flag and the counter are process-wide, so a second test
    /// touching them in parallel makes both flaky — and a test that fails depending on who else
    /// is running teaches nothing. The bounded give-up is asserted here rather than beside it.
    #[test]
    fn work_waits_until_the_card_is_free() {
        the_card_is_free(false);
        let started = std::time::Instant::now();
        let waiting = std::thread::spawn(|| {
            wait_for_the_card(Duration::from_secs(5));
            std::time::Instant::now()
        });

        std::thread::sleep(Duration::from_millis(150));
        assert!(
            anything_waiting_for_the_card(),
            "the shell has to know something is waiting, or it never asks"
        );
        the_card_is_free(true);
        let went = waiting.join().expect("the waiter finishes");

        assert!(went.duration_since(started) >= Duration::from_millis(140));
        assert!(went.duration_since(started) < Duration::from_secs(4));

        // And a card nobody ever gives back must not park the work for good: a picture drawn
        // late is better than one never drawn.
        the_card_is_free(false);
        let gave_up = std::time::Instant::now();
        wait_for_the_card(Duration::from_millis(200));
        assert!(gave_up.elapsed() >= Duration::from_millis(190));
        assert!(gave_up.elapsed() < Duration::from_secs(2));
        the_card_is_free(true);
    }

    /// What a tool says back is prompt, and this one has two hazards at once.
    #[test]
    fn the_sentence_says_nothing_exists_and_invites_nothing() {
        let said = told_it_started("drawing a picture");
        assert!(said.starts_with("STARTED."), "{said}");
        assert!(said.contains("nothing has been made yet"), "{said}");
        // The two failures this shape exists for, by name.
        assert!(
            said.contains("do not call this again"),
            "`see_image` invited a retry and got seven: {said}"
        );
        assert!(
            said.contains("Do not describe the result"),
            "`draw_image` handed back a filename and got a forged picture: {said}"
        );
    }
}
