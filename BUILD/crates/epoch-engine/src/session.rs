//! One search, and whether the machine it ran on stayed the same machine throughout.
//!
//! ## Why a search needs a state of its own
//!
//! Measured 2026-08-31: a configuration can change the driver's residency decisions and leave the
//! environment degraded **for everything measured after it**. A search that established its
//! reference once, at the start, produced ten rows of which seven were about a machine that no
//! longer existed — and nothing in the result said so.
//!
//! Bracketing the whole search at both ends would have caught it at the end. That is nine wasted
//! configurations to learn that row two was the last honest one.
//!
//! ## The sentinel
//!
//! So the baseline is not a row, it is a **control**, and it runs on both sides of every
//! candidate:
//!
//! ```text
//! control 46.7  ->  candidate 48.1  ->  control 46.4       the candidate is valid
//! control 46.7  ->  candidate 27.9  ->  control 27.1       the environment is gone
//! ```
//!
//! The second case says nothing about the candidate's speed and everything about what it did: the
//! candidate, or something that happened during it, left the machine in a different state. It is
//! recorded as a **collapse trigger**, the search stops immediately, and nothing else is measured
//! until the control reproduces again.
//!
//! Stopping is the point. Continuing would produce more rows and every one of them would be a
//! measurement of the damage.
//!
//! ## Recovery is a state, not a solution
//!
//! What restores a degraded environment is **not known**. What is known is that unloading and
//! reloading the model does not — measured, every retry after the first collapse collapsed again.
//!
//! So recovery here attempts only what is safe and cheap — end the instance, check nothing else
//! holds the card, wait, take the control again — and when that does not work it says so and
//! stops. **No driver reset, no restart, nothing aggressive without somebody deciding.** Epoch's
//! job at that point is to be clear:
//!
//! > Benchmark paused because the GPU residency state changed. A clean GPU state is required to
//! > continue.

use serde::{Deserialize, Serialize};

use crate::models::health::Tolerance;

/// Where a search is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum State {
    /// The control has been taken and reproduces. Candidates may run.
    Clean,
    /// A candidate is being measured.
    Running,
    /// The control no longer reproduces. Nothing measured from here is comparable.
    Degraded,
    /// Recovery was attempted and the control still does not reproduce.
    ///
    /// **This means "I know what healthy looked like and I am not there."** It is a claim about
    /// the machine, and it may only be made against a reference that is genuinely comparable.
    RecoveryRequired,
    /// No reference this control can be compared against.
    ///
    /// ## Not the same as `RecoveryRequired`, and the difference is the point
    ///
    /// | | |
    /// |---|---|
    /// | `RecoveryRequired` | *I know what healthy was, and I am not there.* |
    /// | `ReferenceRequired` | *I have nothing comparable enough to tell.* |
    ///
    /// The owner's distinction, 2026-08-31, after Epoch declared a machine healthy by comparing
    /// against nothing: absence of evidence is not evidence of damage either. Calling it
    /// `RecoveryRequired` would send somebody hunting a fault nobody has shown exists — the same
    /// invention as calling it clean, pointing the other way.
    ///
    /// Epoch **may** run a calibration from here: one control, one configuration, recorded with a
    /// full fingerprint, to establish a reference. What it may not do is produce comparative
    /// profiles, because nothing yet says the numbers would be about the same machine.
    ReferenceRequired,
    /// Every candidate was measured, and the control reproduced at the end.
    Complete,
    /// Somebody stopped it.
    ///
    /// **Not a verdict about anything measured.** Every candidate that ran was closed by its own
    /// control, and each one's `Bracket` already decides whether it may be recommended — so the
    /// bracketed rows stay eligible and cancelling costs no evidence. What this records is that
    /// the search did not finish, which `Complete` would have quietly denied.
    Cancelled,
    /// It never established a control at all.
    Invalid,
}

impl State {
    /// Whether a candidate measured while in this state may become a profile.
    pub fn comparable(self) -> bool {
        // Cancelled is here because stopping does not un-measure anything: a candidate that was
        // closed by a control that held is as good as one from a search that ran to the end, and
        // one that was not is refused by its own `Bracket` rather than by the session.
        matches!(
            self,
            State::Clean | State::Running | State::Complete | State::Cancelled
        )
    }
}

/// One reading of the control, and when.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Control {
    pub rate: Option<f64>,
    /// What it came before or after. `None` for the one that opens the search.
    pub around: Option<String>,
    pub at: u64,
}

/// A search, and the environment it ran in.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// The artefact being searched — its identity, not its filename.
    pub artifact: String,
    pub gpu: String,
    pub build: String,
    pub runtime: String,
    /// Every reading of the control, in order.
    pub controls: Vec<Control>,
    /// What counts as the same machine, derived from the opening control.
    pub tolerance: Option<Tolerance>,
    pub state_of: Option<State>,
    /// Which candidate the control stopped reproducing after.
    pub trigger: Option<String>,
    /// What Epoch would say to the person watching.
    pub note: Option<String>,
}

impl Session {
    /// The machine is not where a reference left it, and the run happens anyway.
    ///
    /// **A label, not a verdict.** The state stays whatever it was — a search opening here is
    /// `Clean` in the only sense that matters to a comparison: its controls reproduce each other.
    /// What is recorded is that the absolute numbers belong to a different day than the
    /// reference does, so a profile built from them can say so.
    pub fn measured_in_an_unusual_state(&mut self, standing: &str) {
        self.note = Some(format!(
            "Measured while this machine was not where its reference left it. {standing} The \
             comparisons below are between candidates measured minutes apart in the same state; \
             the absolute figures belong to this run."
        ));
    }

    /// How far the same state is known to vary here, as a share of the middle.
    ///
    /// The width the session judges with, wherever it came from — the reference that measured it,
    /// or this run's own control when nothing governs.
    pub fn allowed_share(&self) -> Option<f64> {
        self.tolerance
            .filter(|it| it.middle > 0.0)
            .map(|it| it.allowed / it.middle)
    }

    pub fn state(&self) -> State {
        self.state_of.unwrap_or(State::Invalid)
    }

    /// Open a search with the readings of its first control, under whatever governs it.
    ///
    /// The centre is this run and the width is whatever measured one properly; see the body for
    /// why each half is where it is. `governing` is `None` where nothing may govern, and
    /// `Tolerance::of` answering nothing with nothing governing means the control produced no
    /// reading at all — a search with no band is `Invalid` rather than optimistic.
    pub fn open(&mut self, rates: &[f64], governing: Option<Tolerance>, at: u64) {
        /*
            **This run's centre, and the reference's width.**

            Both halves were wrong in turn. Taking the whole band from the opening control threw
            away three calibrations' worth of evidence about how much this experiment varies;
            taking the whole band from the reference made a search refuse to start on any day the
            machine was not as fast as it had been — measured, `gemma4:12b` at 46.5 and then 49.1
            against a centre of 51.35, twice, with no profile at the end of it.

            A desktop is never in one state, so waiting for the state a reference was minted in is
            waiting for something that does not come back. And a search does not need a fast
            machine: it needs **the same machine across the candidates it is comparing**, which is
            a question about this run and nothing else.

            So the centre is today's control and the width is whatever measured one properly.
        */
        let mine = Tolerance::of(rates);
        self.tolerance = match (governing, mine) {
            (Some(band), Some(here)) => band.at(here.middle).or(Some(here)),
            (None, here) => here,
            (Some(band), None) => Some(band),
        };
        self.controls.push(Control {
            rate: crate::models::health::median(rates),
            around: None,
            at,
        });
        self.state_of = Some(match self.tolerance {
            Some(_) => State::Clean,
            None => State::Invalid,
        });
    }

    /// Take the control again, after a candidate.
    ///
    /// Returns whether the search may continue. A `false` has already set the state and the note.
    pub fn check(&mut self, candidate: &str, rate: Option<f64>, at: u64) -> bool {
        self.controls.push(Control {
            rate,
            around: Some(candidate.to_owned()),
            at,
        });
        let Some(tolerance) = self.tolerance else {
            self.state_of = Some(State::Invalid);
            return false;
        };
        // A control that produced no reading at all is not a reproduction.
        let reproduces = rate.is_some_and(|it| tolerance.reproduces(it));
        if reproduces {
            self.state_of = Some(State::Clean);
            return true;
        }
        self.state_of = Some(State::Degraded);
        self.trigger = Some(candidate.to_owned());
        self.note = Some(format!(
            "The control no longer reproduces after {candidate}: {} against {:.1} tok/s. \
             Nothing measured from here would be comparable, so the search stopped.",
            rate.map(|it| format!("{it:.1}"))
                .unwrap_or_else(|| "no answer".to_owned()),
            tolerance.middle,
        ));
        false
    }

    /// Record that recovery was tried and the control still does not reproduce.
    ///
    /// **The wording is the product.** Epoch knows what happened and does not know how to undo it,
    /// and saying both is more useful than either half — a reset it invented would be an action
    /// nobody asked for on somebody else's driver.
    pub fn recovery_failed(&mut self) {
        self.state_of = Some(State::RecoveryRequired);
        self.note = Some(
            "Benchmark paused because the GPU residency state changed. Ending and reloading the \
             model did not restore it. A clean GPU state is required to continue."
                .to_owned(),
        );
    }

    /// The control ran and there was nothing comparable to judge it against.
    ///
    /// **Not `RecoveryRequired`.** That says *I know what healthy looked like and I am not there*,
    /// which is a claim about the machine; this says *I cannot tell*, which is a claim about the
    /// record. Reporting the second as the first would send somebody hunting a fault nobody has
    /// shown exists — the same invention as declaring the machine clean, pointing the other way.
    /// **The consequence is stated once, by `Verdict::say`.** This used to append its own copy,
    /// and the first pause that actually used both ended: *"A calibration can establish one; no
    /// comparative search will run until it does. A calibration can establish a reference; until
    /// it does, no comparative search will run."* Two spellings of one sentence, in two files,
    /// agreeing by luck until something used both.
    pub fn needs_a_reference(&mut self, why: &str) {
        self.state_of = Some(State::ReferenceRequired);
        self.note = Some(format!("Nothing comparable to judge this against. {why}"));
    }

    /// The controls are sliding: still falling, and now outside what the reference allows.
    ///
    /// **Not a collapse and not a recovery case.** A collapse is one instance going wrong and
    /// reloading sometimes fixes it. This is the state leaving over hours — measured
    /// 2026-09-01 at 25% across six controls, where ten minutes of idle did not interrupt it —
    /// and nothing Epoch may do fixes it. So the session stops and says what it saw.
    pub fn drifted(&mut self, why: &str) {
        self.state_of = Some(State::Degraded);
        self.note = Some(why.to_owned());
    }

    /// Recovery worked: the control reproduces again.
    pub fn recovered(&mut self) {
        self.state_of = Some(State::Clean);
        self.note = Some("The control reproduces again. Continuing.".to_owned());
    }

    /// Every candidate ran and the closing control reproduced.
    pub fn complete(&mut self) {
        if self.state() == State::Clean {
            self.state_of = Some(State::Complete);
        }
    }

    /// Somebody pressed stop.
    ///
    /// **Only from a healthy state.** A search that was already degraded and then cancelled is
    /// degraded — that is the more important fact about it, and overwriting it would lose the
    /// reason the person probably pressed stop.
    pub fn cancelled(&mut self) {
        if matches!(self.state(), State::Clean | State::Running) {
            self.state_of = Some(State::Cancelled);
            self.note = Some(
                "Stopped. Everything measured up to here is kept; each candidate's own controls                  decide whether it may be recommended."
                    .to_owned(),
            );
        }
    }

    /// Whether results from this session may be turned into profiles.
    pub fn results_are_usable(&self) -> bool {
        self.state().comparable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_runs_centre_and_the_references_width() {
        /*
            Both halves were wrong in turn, and the pair is the answer.

            Taking the whole band from the opening control threw away three calibrations' worth of
            evidence about how much this experiment varies. Taking the whole band from the
            reference made a search refuse to start on any day the machine was not as fast as it
            had been — measured 2026-09-01, `gemma4:12b` opened at 46.5 and then 49.1 against a
            reference centred on 51.35, refused twice, and left the owner with no profile.

            A desktop is never in one state. What a search needs is not a fast machine but the
            same machine across the candidates it compares, which is a question about this run.
        */
        let governing = Tolerance::of(&[47.49, 48.50, 47.52, 46.63, 48.06]).expect("a band");
        let mut it = Session::default();
        it.open(&[48.90, 48.12, 48.66, 48.61, 47.69], Some(governing), 1);
        let band = it.tolerance.expect("a band");

        assert!(
            (band.middle - 48.61).abs() < 0.01,
            "the centre is this run's control: {band:?}",
        );
        let share = |it: Tolerance| it.allowed / it.middle;
        assert!(
            (share(band) - share(governing)).abs() < 1e-9,
            "and the width is the reference's, as a share: {band:?} against {governing:?}",
        );
        assert_eq!(it.state(), State::Clean);

        /*
            A machine running well below where a reference left it still opens, and its band is
            around where it actually is. This is the case that used to refuse.
        */
        let mut slow = Session::default();
        slow.open(&[46.4, 46.6, 46.5], Some(governing), 1);
        let band = slow.tolerance.expect("a band");
        assert!((band.middle - 46.5).abs() < 0.01, "{band:?}");
        assert!((share(band) - share(governing)).abs() < 1e-9, "{band:?}");

        /*
            And with nothing governing the band is this machine's own opening control, which is
            why this ever took the rates: a slower machine is judged as itself.
        */
        let mut alone = Session::default();
        alone.open(&[9.0, 9.1, 8.9], None, 1);
        assert_eq!(alone.tolerance, Tolerance::of(&[9.0, 9.1, 8.9]));
        assert_eq!(alone.state(), State::Clean);

        // Nothing measured and nothing governing is still Invalid rather than optimistic.
        let mut blind = Session::default();
        blind.open(&[], None, 1);
        assert_eq!(blind.state(), State::Invalid);
    }

    #[test]
    fn a_run_in_an_unusual_state_says_so_and_still_runs() {
        /*
            The label that replaced the gate. What it must not do is change the state: a search
            whose controls reproduce each other is comparable in the only sense a comparison
            needs, and calling it degraded would take away the answer the owner pressed for.
        */
        let mut it = Session::default();
        it.open(&[46.4, 46.6, 46.5], None, 1);
        it.measured_in_an_unusual_state("46.5 tok/s against a known clean 51.4.");
        assert_eq!(it.state(), State::Clean, "still comparable within itself");
        let note = it.note.clone().expect("a note");
        assert!(
            note.contains("51.4"),
            "it names what it was compared against: {note}"
        );
        assert!(
            note.contains("belong to this run"),
            "and says which figures are about the day: {note}",
        );
    }

    #[test]
    fn stopping_a_search_costs_no_evidence_and_does_not_claim_it_finished() {
        /*
            `complete()` promotes `Clean` to `Complete`, so a search somebody stopped after two of
            twenty configurations was recorded as having finished — and `Complete` is one of the
            states whose rows may become profiles.

            Cancelling un-measures nothing: every candidate that ran was closed by its own
            control, and each one's `Bracket` decides whether it may be recommended. So the rows
            stay comparable and only the claim about reaching the end goes away.
        */
        let mut it = Session::default();
        it.open(&[46.0, 46.2, 45.9], None, 1);
        assert_eq!(it.state(), State::Clean);

        it.cancelled();
        assert_eq!(it.state(), State::Cancelled);
        assert!(
            it.results_are_usable(),
            "bracketed rows are as good as any other"
        );
        assert!(it.note.as_deref().is_some_and(|it| it.contains("kept")));

        // And it is not `Complete`, which is the whole point.
        it.complete();
        assert_eq!(
            it.state(),
            State::Cancelled,
            "complete() must not overwrite it"
        );
    }

    #[test]
    fn a_degraded_search_that_is_then_cancelled_stays_degraded() {
        // The more important fact about it, and probably the reason somebody pressed stop.
        let mut it = Session::default();
        it.open(&[46.0, 46.2, 45.9], None, 1);
        assert!(!it.check("flash attention off", Some(27.0), 2));
        assert_eq!(it.state(), State::Degraded);

        it.cancelled();
        assert_eq!(it.state(), State::Degraded);
        assert!(!it.results_are_usable());
    }

    fn session() -> Session {
        Session {
            artifact: "Qwen3.6-35B-A3B-UD-IQ4_XS".to_owned(),
            gpu: "a card".to_owned(),
            build: "b1".to_owned(),
            runtime: "llama_cpp".to_owned(),
            ..Session::default()
        }
    }

    #[test]
    fn a_control_that_reproduces_lets_the_search_continue() {
        // control 46.7 -> candidate 48.1 -> control 46.4
        let mut it = session();
        it.open(&[46.0, 46.7, 47.0], None, 1);
        assert_eq!(it.state(), State::Clean);
        assert!(it.check("flash attention on", Some(46.4), 2));
        assert_eq!(it.state(), State::Clean);
        assert_eq!(it.trigger, None);
    }

    #[test]
    fn a_control_that_does_not_reproduce_stops_the_search_and_names_what_broke_it() {
        /*
            control 46.7 -> candidate 27.9 -> control 27.1. That says nothing about the
            candidate's speed and everything about what it did. Continuing would produce more rows
            and every one would be a measurement of the damage.
        */
        let mut it = session();
        it.open(&[46.0, 46.7, 47.0], None, 1);
        assert!(!it.check("KV cache q8_0", Some(27.1), 2));
        assert_eq!(it.state(), State::Degraded);
        assert_eq!(it.trigger.as_deref(), Some("KV cache q8_0"));
        let note = it.note.expect("it says why");
        assert!(note.contains("27.1") && note.contains("46.7"), "{note}");
    }

    #[test]
    fn a_search_with_no_control_is_invalid_rather_than_optimistic() {
        // Nothing to compare against is not the same as nothing wrong.
        let mut it = session();
        it.open(&[], None, 1);
        assert_eq!(it.state(), State::Invalid);
        assert!(!it.results_are_usable());
    }

    #[test]
    fn a_control_that_answered_nothing_is_not_a_reproduction() {
        let mut it = session();
        it.open(&[46.0, 46.7, 47.0], None, 1);
        assert!(!it.check("something", None, 2));
        assert_eq!(it.state(), State::Degraded);
    }

    #[test]
    fn recovery_that_fails_says_what_epoch_knows_and_what_it_does_not() {
        /*
            Measured: unloading and reloading the model does not restore a degraded environment.
            Epoch knows what happened and does not know how to undo it, and a driver reset it
            invented would be an action nobody asked for on somebody else's machine.
        */
        let mut it = session();
        it.open(&[46.0, 46.7, 47.0], None, 1);
        it.check("KV cache q8_0", Some(27.1), 2);
        it.recovery_failed();
        assert_eq!(it.state(), State::RecoveryRequired);
        assert!(!it.results_are_usable());
        let note = it.note.expect("it says what to do");
        assert!(note.contains("clean GPU state"), "{note}");
    }

    #[test]
    fn recovery_that_works_puts_the_search_back_to_clean() {
        let mut it = session();
        it.open(&[46.0, 46.7, 47.0], None, 1);
        it.check("KV cache q8_0", Some(27.1), 2);
        it.recovered();
        assert_eq!(it.state(), State::Clean);
        assert!(it.results_are_usable());
    }

    #[test]
    fn a_search_only_completes_from_clean() {
        // Calling a degraded session complete would be the whole defect this file exists for,
        // arriving at the last line.
        let mut it = session();
        it.open(&[46.0, 46.7, 47.0], None, 1);
        it.check("something", Some(27.0), 2);
        it.complete();
        assert_eq!(it.state(), State::Degraded, "not Complete");

        let mut good = session();
        good.open(&[46.0, 46.7, 47.0], None, 1);
        good.check("something", Some(46.5), 2);
        good.complete();
        assert_eq!(good.state(), State::Complete);
    }

    #[test]
    fn the_tolerance_is_the_machines_own_and_a_slower_one_is_judged_as_itself() {
        // A laptop answering at 9 tok/s is not degraded for answering at 9 tok/s.
        let mut slow = session();
        slow.open(&[9.0, 9.1, 8.9], None, 1);
        assert!(slow.check("something", Some(8.8), 2));
        assert!(!slow.check("something else", Some(4.0), 3));
    }

    #[test]
    fn every_control_reading_is_kept_in_order() {
        // The sequence is the evidence: which candidate sat between which two readings.
        let mut it = session();
        it.open(&[46.0, 46.7, 47.0], None, 1);
        it.check("a", Some(46.5), 2);
        it.check("b", Some(46.6), 3);
        assert_eq!(it.controls.len(), 3);
        assert_eq!(
            it.controls[0].around, None,
            "the opening one sits after nothing"
        );
        assert_eq!(it.controls[1].around.as_deref(), Some("a"));
    }
}
