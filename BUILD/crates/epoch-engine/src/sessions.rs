//! What a paused benchmark remembers, and what it needs before it may start again.
//!
//! ## The situation this was built for
//!
//! 2026-08-31. A search opened its control at 28.0 tok/s on a machine that produces ~46 when it is
//! healthy — so the environment was already degraded before anything started, and every recovery
//! step Epoch is permitted to take left it lower rather than better.
//!
//! The owner was working remotely and could not reboot without risking access, and the standing
//! instruction is explicit: **no reboot, no `--gpu-reset`, no driver restart, nothing that could
//! take down the display or a remote session.** So the honest thing for Epoch to do is stop, keep
//! everything, and say what it needs.
//!
//! ## Two things are kept, and they are different
//!
//! - A [`Reference`] — the last control this machine produced **while it was known healthy**. It
//!   is what a later control is compared against, and it is per artefact, card, build and runtime,
//!   because a number from another machine is not evidence about this one.
//! - A [`Paused`] session — the whole diagnosis of a search that stopped: which candidate broke
//!   it, what the controls read, what the card and the shared pool were doing, when.
//!
//! ## Coming back
//!
//! On return, Epoch does not resume the old search. It says *an interrupted benchmark is waiting
//! for a clean-state check*, runs **only the control**, and compares it to the reference. Clean
//! means a **new session from scratch**; still degraded means the pause stands.
//!
//! Never continuing from the middle: the rows before the break and the rows after it were measured
//! on two different machines, and joining them would produce exactly the table this whole
//! mechanism exists to prevent.
//!
//! ## And Epoch does not claim to know the cure
//!
//! It may say *a clean GPU state may require restarting the machine*. It may not decide that, and
//! it may not do it.

use serde::{Deserialize, Serialize};

use crate::models::health::Tolerance;
use crate::session::Session;

/// What this machine produced while it was known healthy.
///
/// **Per artefact, card, build and runtime.** The same four that make a benchmark comparable —
/// a reference from another card is not evidence that this one has changed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reference {
    pub artifact: String,
    pub gpu: String,
    pub build: String,
    pub runtime: String,
    /// The control's median while healthy.
    pub rate: f64,
    /// What counted as the same machine, derived from that control's own readings.
    pub tolerance: Tolerance,
    pub at: u64,
}

impl Reference {
    pub fn about(&self, artifact: &str, gpu: &str, build: &str, runtime: &str) -> bool {
        self.artifact == artifact
            && self.gpu == gpu
            && self.build == build
            && self.runtime == runtime
    }
}

/// A search that stopped, kept whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paused {
    pub session: Session,
    /// What the control read when it stopped.
    pub last_control: Option<f64>,
    /// **Which** healthy figure this pause was measured against — by id, never by value.
    ///
    /// The bug this replaces: the number was copied here as a bare `f64` while the gate read a
    /// separate list, so one fact lived in two places and the gate read the empty one. A pause
    /// now points at the one entity; the number, and what it is a number about, live there.
    #[serde(default)]
    pub clean_reference_id: Option<String>,
    /// The value at the moment of pausing, **for the record and never for a decision.**
    ///
    /// An audit snapshot: it says what the banner showed that day even if the reference is later
    /// superseded. Nothing in the logic reads it — that is the whole point, and it is why it is
    /// named for what it is rather than left looking like a second source of truth.
    #[serde(default, alias = "cleanReference")]
    pub clean_reference_seen: Option<f64>,
    /// The card and the shared pool at the moment it paused, in bytes.
    pub vram_used: Option<u64>,
    pub shared_used: Option<u64>,
    pub at: u64,
}

impl Paused {
    /// What to show somebody coming back to this.
    ///
    /// **The clean reference appears only when there is one.** A normal user has no use for
    /// *known clean reference: —*, and inventing a figure to fill the line would be the invented
    /// gauge in the one place this whole subsystem exists to keep honest.
    pub fn why(&self) -> String {
        let now = self
            .last_control
            .map(|it| format!("{it:.1} tok/s"))
            .unwrap_or_else(|| "no answer".to_owned());
        // The snapshot, and only because this is a sentence rather than a decision.
        match self.clean_reference_seen {
            Some(clean) => format!(
                "GPU state is not suitable for a reliable benchmark. Current control: {now}. \
                 Known clean reference: {clean:.1} tok/s. A clean GPU state is required, and it \
                 may require restarting the machine.",
            ),
            None => format!(
                "GPU state is not suitable for a reliable benchmark. Current control: {now}. \
                 A clean GPU state is required, and it may require restarting the machine.",
            ),
        }
    }

    /// The one line that goes on a deck when Epoch reopens.
    pub fn waiting(&self) -> String {
        format!(
            "An interrupted benchmark for {} is waiting for a clean-state check.",
            self.session.artifact
        )
    }
}

/// Everything remembered about benchmark sessions on this machine.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Kept {
    /// Every healthy figure this machine has produced, each carrying what it is a figure about.
    ///
    /// **The single entity.** A paused session points at one of these by `id` and keeps no copy
    /// of the number — two independent spellings of one fact is exactly the defect of
    /// 2026-08-31, where the gate read the empty one and declared a degraded machine healthy.
    #[serde(default)]
    kept: Vec<crate::reference::PerformanceReference>,
    /// The older shape, read once and converted. Written back empty.
    ///
    /// Those records carry an artefact, a card, a build and a runtime, and nothing about the
    /// context, the cache or the workload — so they convert to **legacy** references: kept,
    /// shown, and not allowed to decide whether a machine is itself.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    references: Vec<Reference>,
    /// Every calibration run that happened, whether or not anything was built on it.
    ///
    /// **Observations are not references.** A run that produced three usable rates instead of
    /// five, or whose runs spanned two states, is evidence and is not an authority — and it is
    /// kept either way, because a series of them is the only thing that can say whether a fresh
    /// process reproduces a state at all.
    #[serde(default)]
    seen: Vec<crate::reference::Observation>,
    /// Searches that stopped and are waiting for a clean state.
    #[serde(default)]
    paused: Vec<Paused>,
}

pub fn path(library: &std::path::Path) -> std::path::PathBuf {
    library.join("sessions.json")
}

impl Kept {
    /// Never fails: an unreadable file is an empty memory, so a new pause is never lost to an old
    /// one that will not parse.
    pub fn load(library: &std::path::Path) -> Self {
        std::fs::read_to_string(path(library))
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(&raw).ok())
            .unwrap_or_default()
            .migrate()
    }

    pub fn save(&self, library: &std::path::Path) -> Result<(), String> {
        let raw = serde_json::to_string_pretty(self).map_err(|why| why.to_string())?;
        if let Some(home) = path(library).parent() {
            std::fs::create_dir_all(home).map_err(|why| why.to_string())?;
        }
        std::fs::write(path(library), raw).map_err(|why| why.to_string())
    }

    /// Fold any record written in the older shape into the one entity.
    ///
    /// **Kept, labelled, and demoted — never deleted and never promoted.** An old record knows
    /// the artefact, the card, the build and the runtime; it knows nothing about the context, the
    /// cache, the flash-attention state or the workload, which are four of the things that decide
    /// what a throughput number means. So it becomes a `legacy` reference: worth showing, because
    /// *we once saw 46 here* is real information about this machine, and disqualified from
    /// deciding whether a later number is a regression.
    fn migrate(mut self) -> Self {
        /*
            **A pause that names a number nothing points at.** The record on this machine carried
            `cleanReference: 46.0` and the reference list was empty, which is how the gate came to
            compare against nothing. The number is real — somebody measured 46 here — and its
            provenance is gone, so it becomes a legacy figure with an id the pause can point at:
            kept, shown, and unable to decide anything.

            Not deleted, because *we once saw 46 here* is information about this machine. Not
            promoted, because a number and a date cannot say whether a later number is a
            regression.
        */
        let orphans: Vec<(usize, f64, u64, String)> = self
            .paused
            .iter()
            .enumerate()
            .filter(|(_, it)| it.clean_reference_id.is_none())
            .filter_map(|(n, it)| {
                Some((
                    n,
                    it.clean_reference_seen?,
                    it.at,
                    it.session.artifact.clone(),
                ))
            })
            .collect();
        for (n, rate, at, artifact) in orphans {
            let id = format!("observed-{artifact}-{at}");
            let session = &self.paused[n].session;
            self.remember(
                crate::reference::PerformanceReference {
                    id: id.clone(),
                    fingerprint_id: String::new(),
                    fingerprint: crate::reference::Fingerprint {
                        artifact: Some(session.artifact.clone()),
                        gpu: Some(session.gpu.clone()),
                        build: Some(session.build.clone()),
                        runtime: Some(session.runtime.clone()),
                        ..Default::default()
                    },
                    median: rate,
                    dispersion: None,
                    runs: None,
                    rates: Vec::new(),
                    observed: Default::default(),
                    sources: Vec::new(),
                    policy: None,
                    between: None,
                    at,
                    legacy: true,
                    standing: Default::default(),
                    superseded_by: None,
                    short_term: Default::default(),
                    temporal: Default::default(),
                }
                .keyed(),
            );
            self.paused[n].clean_reference_id = Some(id);
        }

        for old in std::mem::take(&mut self.references) {
            let one = crate::reference::PerformanceReference {
                id: format!("legacy-{}-{}", old.artifact, old.at),
                fingerprint_id: String::new(),
                fingerprint: crate::reference::Fingerprint {
                    artifact: Some(old.artifact.clone()),
                    gpu: Some(old.gpu.clone()),
                    build: Some(old.build.clone()),
                    runtime: Some(old.runtime.clone()),
                    ..Default::default()
                },
                median: old.rate,
                dispersion: Some(old.tolerance),
                runs: None,
                rates: Vec::new(),
                observed: Default::default(),
                sources: Vec::new(),
                policy: None,
                between: None,
                at: old.at,
                legacy: true,
                standing: Default::default(),
                superseded_by: None,
                short_term: Default::default(),
                temporal: Default::default(),
            }
            .keyed();
            self.remember(one);
        }
        self
    }

    /// Every reference for one artefact on this machine, newest first.
    ///
    /// **Everything, including the legacy ones.** Which of them may act as a gate is
    /// `is_safety_reference`'s question, asked by the caller — a lookup that silently dropped the
    /// unusable ones would leave a person unable to see the number their banner has been showing.
    pub fn references_for(
        &self,
        artifact: &str,
        gpu: &str,
        build: &str,
        runtime: &str,
    ) -> Vec<&crate::reference::PerformanceReference> {
        let mut found: Vec<_> = self
            .kept
            .iter()
            .filter(|it| {
                let f = &it.fingerprint;
                f.artifact.as_deref() == Some(artifact)
                    && f.gpu.as_deref() == Some(gpu)
                    && f.build.as_deref() == Some(build)
                    && f.runtime.as_deref() == Some(runtime)
            })
            .collect();
        found.sort_by_key(|it| std::cmp::Reverse(it.at));
        found
    }

    pub fn by_id(&self, id: &str) -> Option<&crate::reference::PerformanceReference> {
        self.kept.iter().find(|it| it.id == id)
    }

    /// Keep a figure. **Nothing is ever replaced by a newer measurement of the same thing.**
    ///
    /// ## Fingerprint is not a measurement id
    ///
    /// This keyed on the fingerprint for a day, so the second calibration of an experiment
    /// overwrote the first. Both were real: 39.68 tok/s and 41.90 tok/s, three runs each, run
    /// sets that do not overlap, every other reading identical. The survivor was whichever ran
    /// last, and the difference between them — the only interesting thing in the pair — was
    /// gone.
    ///
    /// A fingerprint answers *what kind of experiment is this*. A measurement answers *what
    /// happened this time*. Several of the second share one of the first, and drift is what that
    /// looks like.
    ///
    /// Writing an id that already exists still replaces, because an id names one measurement and
    /// re-writing it is a correction rather than a second observation.
    pub fn remember(&mut self, one: crate::reference::PerformanceReference) {
        /*
            **One `Current` per experiment, or `current` means nothing.**

            Measured 2026-09-01: two references for the same fingerprint both stood `Current`, and
            `governing_reference_for` picks the newest — so which one governed was decided by a
            timestamp rather than by anything about the two. The older is superseded here, at the
            one point where a replacement actually arrives.

            **Its numbers are untouched.** `Superseded` says it no longer decides, never that it
            was wrong; the record of what this machine did stays exactly as it was measured.
        */
        self.kept.retain(|it| it.id != one.id);
        for it in self.kept.iter_mut() {
            if it.fingerprint_id == one.fingerprint_id
                && it.standing == crate::reference::Standing::Current
            {
                it.standing = crate::reference::Standing::Superseded;
                // **By which one.** Retiring `one` later must not reinstate this; that is a
                // decision somebody makes, not a consequence of a standing changing elsewhere.
                it.superseded_by = Some(one.id.clone());
            }
        }
        self.kept.push(one);
    }

    /// Keep an observation. **Nothing is ever replaced**: two calibrations of one experiment are
    /// two rows, and the difference between them is the whole subject.
    pub fn observed(&mut self, one: crate::reference::Observation) {
        self.seen.retain(|it| it.id != one.id);
        self.seen.push(one);
    }

    /// Mark a reference as no longer describing this machine, **without touching its numbers**.
    ///
    /// The distinction the 2026-09-01 measurements forced: `R-001` was a true summary of the three
    /// processes it was built from and a poor description of what the machine does over hours. Its
    /// record of how it was built stays exactly as it was; what changes is whether it may decide
    /// anything. Widening its band until later readings fit would have destroyed the only
    /// instrument that detected the thing worth knowing.
    pub fn restand(&mut self, id: &str, standing: crate::reference::Standing) -> bool {
        match self.kept.iter_mut().find(|it| it.id == id) {
            Some(one) => {
                one.standing = standing;
                true
            }
            None => false,
        }
    }

    /// `O-001`, `O-002`, … Monotonic, and never derived from the content.
    pub fn next_observation_id(&self) -> String {
        let highest = self
            .seen
            .iter()
            .filter_map(|it| it.id.strip_prefix("O-"))
            .filter_map(|it| it.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("O-{:03}", highest + 1)
    }

    /// Every calibration of one kind of experiment, oldest first.
    pub fn observations_of(&self, fingerprint_id: &str) -> Vec<crate::reference::Observation> {
        let mut found: Vec<_> = self
            .seen
            .iter()
            .filter(|it| it.fingerprint_id == fingerprint_id)
            .cloned()
            .collect();
        found.sort_by_key(|it| it.at);
        found
    }

    /// Exactly these observations, by id, in the order given.
    ///
    /// ## A series is what one calibration run took
    ///
    /// `observations_of` answers *everything ever recorded for this kind of experiment*, which is
    /// the right answer for a history and the wrong one for a series. Measured 2026-09-01: a
    /// search took three fresh calibrations that agreed to 0.9% — 48.17, 47.99, 47.73 — and
    /// minting refused, because the set it was handed also contained an observation from two
    /// hours earlier whose runs spanned 5.24%.
    ///
    /// **The fingerprint says what kind of experiment; it does not say when.** A series has to be
    /// named by its members.
    pub fn these(&self, ids: &[String]) -> Vec<crate::reference::Observation> {
        ids.iter()
            .filter_map(|id| self.seen.iter().find(|it| &it.id == id))
            .cloned()
            .collect()
    }

    /// The two levels of variability for one experiment, kept apart.
    pub fn series_of(&self, fingerprint_id: &str) -> crate::reference::Series {
        crate::reference::Series::of(&self.observations_of(fingerprint_id))
    }

    /// The next measurement id: `R-001`, `R-002`, … Monotonic across everything kept.
    ///
    /// **Never derived from the content.** An id built from the fingerprint and a timestamp is an
    /// id two measurements can collide on, which is how this went wrong the first time.
    pub fn next_reference_id(&self) -> String {
        let highest = self
            .kept
            .iter()
            .filter_map(|it| it.id.strip_prefix("R-"))
            .filter_map(|it| it.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("R-{:03}", highest + 1)
    }

    /// Every measurement of one kind of experiment, newest first.
    pub fn history_of(&self, fingerprint_id: &str) -> Vec<&crate::reference::PerformanceReference> {
        let mut found: Vec<_> = self
            .kept
            .iter()
            .filter(|it| it.fingerprint_id == fingerprint_id)
            .collect();
        found.sort_by_key(|it| std::cmp::Reverse(it.at));
        found
    }

    /// The newest measurement of one kind of experiment that **may act as a gate**.
    ///
    /// **An index over the history, not a second store.** History keeps everything; this is the
    /// convenience that answers *which one governs right now*.
    ///
    /// **Named for authority rather than for recency, because the two came apart.** A caller took
    /// the newest and then asked whether it governed — measured 2026-09-01, that answered *no
    /// governing reference exists* on a store holding `R-002` governing and a retired `R-003`
    /// above it, and a search calibrated from scratch against a perfectly good authority sitting
    /// one row down. Whether a reference governs is a property of its standing, and which one is
    /// the authority must never fall out of the order of a vector.
    ///
    /// The lifecycle decides, and it decides explicitly: a reference replaced by another carries
    /// `superseded_by`, so retiring the replacement does not quietly reinstate what it replaced.
    pub fn governing_reference_for(
        &self,
        fingerprint_id: &str,
    ) -> Option<&crate::reference::PerformanceReference> {
        self.history_of(fingerprint_id)
            .into_iter()
            .find(|it| it.is_safety_reference())
    }

    /// A search waiting for a clean state, if there is one for this artefact.
    pub fn paused_for(&self, artifact: &str) -> Option<&Paused> {
        self.paused
            .iter()
            .find(|it| it.session.artifact == artifact)
    }

    /// Anything waiting at all, so a deck can say so without knowing what to ask about.
    pub fn anything_paused(&self) -> Option<&Paused> {
        self.paused.first()
    }

    pub fn pause(&mut self, one: Paused) {
        self.paused
            .retain(|it| it.session.artifact != one.session.artifact);
        self.paused.push(one);
    }

    /// The clean-state check passed: the pause is over.
    ///
    /// **It does not resume anything.** The rows before the break and the rows after it were
    /// measured on two different machines, and a new session starts from the beginning.
    pub fn cleared(&mut self, artifact: &str) {
        self.paused.retain(|it| it.session.artifact != artifact);
    }
}
/// Whether a search may start, given what the control just read.
///
/// ## Three answers, and the middle one is new
///
/// | | |
/// |---|---|
/// | `Ok(Verdict::Clean)` | a comparable reference exists and this control reproduces it |
/// | `Ok(Verdict::NoReference)` | nothing comparable exists — Epoch cannot tell, either way |
/// | `Err(..)` | a comparable reference exists and this control does not reproduce it |
///
/// **`NoReference` is not a pass and it is not a failure.** Measured 2026-08-31: this function
/// returned `Ok(())` for *no reference at all*, on the reasoning that a machine with nothing
/// measured must be allowed its first search — which is true, and which made "I have never
/// measured this" indistinguishable from "I checked and it is fine". Epoch then said *"this
/// machine is itself again"* about a comparison that never happened.
///
/// *A gate with nothing behind it must stay open* is a rule about not withholding a capability.
/// A verification is the opposite case: **a permission may default to yes; a verification may
/// not.** Calibration is still allowed from here — that is what produces the reference — and
/// comparative profiles are not.
pub fn may_begin(
    control: Option<f64>,
    reference: Option<&crate::reference::PerformanceReference>,
    against: &crate::reference::Fingerprint,
) -> Result<Verdict, String> {
    let Some(control) = control else {
        return Err("the control produced no reading at all.".to_owned());
    };
    let Some(reference) = reference.filter(|it| it.is_safety_reference()) else {
        return Ok(Verdict::NoReference);
    };
    let comparison = reference.fingerprint.comparable_to(against);
    if !comparison.yes() {
        return Ok(Verdict::NotComparable(comparison));
    }
    let Some((band, whence)) = reference.band() else {
        return Ok(Verdict::NoReference);
    };
    if band.reproduces(control) {
        return Ok(Verdict::Clean {
            reference: reference.median,
            band: whence,
        });
    }
    /*
        **The reading, and not what anybody does about it.**

        This ended *"The benchmark stays paused and no search will run"*, which was true while
        the reference was a gate. It stopped being true the moment a search began measuring in
        whatever state it found and labelling the result — and the sentence went on being
        printed, on a panel headed `OPTIMIZATION COMPLETE`, under eight configurations that had
        just run. Measured 2026-09-02, `gemma4:12b`.

        A sentence about *why* a value is what it is has to move when the reason does. RETRY does
        still keep the pause, so it says so where it decides that; here there is only the
        comparison.
    */
    Err(format!(
        "The control does not reproduce. {control:.1} tok/s against a known clean {:.1} — \
         {:.0}% short, and more than the {:.0}% this comparison allows ({}).",
        reference.median,
        band.short_by(control) * 100.0,
        band.allowed / band.middle * 100.0,
        whence.about(),
    ))
}

/// What a clean-state check concluded.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    /// A comparable reference exists and this control reproduces it.
    Clean {
        reference: f64,
        band: crate::reference::Band,
    },
    /// Nothing exists that this control could be compared against.
    NoReference,
    /// A reference exists and is about something else.
    NotComparable(crate::reference::Comparison),
}

impl Verdict {
    /// Whether comparative work may proceed. **Only `Clean`.**
    pub fn may_compare(&self) -> bool {
        matches!(self, Verdict::Clean { .. })
    }

    /// What to tell somebody, with the control they just took.
    pub fn say(&self, control: f64) -> String {
        match self {
            Verdict::Clean { reference, band } => format!(
                "The control reproduces: {control:.1} tok/s against a known clean \
                 {reference:.1} ({}). This machine is itself again, and a new search can \
                 start from the beginning.",
                band.about(),
            ),
            Verdict::NoReference => format!(
                "Nothing was compared. The control read {control:.1} tok/s, and no reference \
                 with enough provenance to compare against exists for this model on this card \
                 and build — so Epoch cannot say whether the machine is itself. A calibration \
                 can establish one; no comparative search will run until it does."
            ),
            Verdict::NotComparable(why) => format!(
                "Nothing was compared. The control read {control:.1} tok/s, and the reference \
                 on file is about something else — {}. A calibration can establish one that \
                 matches; no comparative search will run until it does.",
                why.why(),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `R-001` retired, `R-002` governing, `R-003` aside — and the store answers with the one
    /// that has the authority, never with the one that happens to be last in the vector.
    ///
    /// Four cases, and the pair that matters is A against B: the same three ids, the same
    /// standings, and two different right answers depending on **why** `R-003` stood aside.
    mod authority {
        use super::*;

        fn at(id: &str, when: u64, median: f64) -> crate::reference::PerformanceReference {
            let mut it = known(median);
            it.id = id.to_owned();
            it.at = when;
            it.fingerprint = shape();
            it.fingerprint_id = shape().key();
            it
        }

        fn governs(kept: &Kept) -> Option<String> {
            kept.governing_reference_for(&shape().key())
                .map(|it| it.id.clone())
        }

        #[test]
        fn a_newer_reference_standing_aside_does_not_hide_the_one_that_governs() {
            /*
                Measured 2026-09-01 on the real store: `R-002` current, `R-003` superseded above
                it, and a search reported *no governing reference* and calibrated from scratch.
                The caller took the newest and then asked whether it governed.
            */
            let mut kept = Kept::default();
            kept.remember(at("R-002", 200, 47.52));

            // Aside for its own reasons: nothing says it replaced R-002.
            let mut aside = at("R-003", 300, 48.61);
            aside.standing = crate::reference::Standing::Superseded;
            kept.kept.push(aside);

            assert_eq!(governs(&kept).as_deref(), Some("R-002"));
        }

        #[test]
        fn a_reference_that_was_replaced_does_not_come_back() {
            /*
                The other reading of the same three rows. `R-003` arrived *as* the replacement,
                so `R-002` is not an older authority waiting underneath — it is a retired one,
                and retiring its successor leaves nothing rather than reinstating it.
            */
            let mut kept = Kept::default();
            kept.remember(at("R-002", 200, 47.52));
            kept.remember(at("R-003", 300, 48.61));

            let replaced = kept
                .history_of(&shape().key())
                .into_iter()
                .find(|it| it.id == "R-002")
                .expect("kept")
                .clone();
            assert_eq!(replaced.superseded_by.as_deref(), Some("R-003"));

            // Now the replacement itself stops describing the machine.
            assert!(kept.restand(
                "R-003",
                crate::reference::Standing::TemporalCoverageInsufficient
            ));
            assert_eq!(
                governs(&kept),
                None,
                "nothing governs, and that is the honest answer"
            );

            // And standing alone would have let it back: what decides is *what replaced it*.
            assert!(kept.restand("R-002", crate::reference::Standing::Current));
            assert_eq!(governs(&kept), None);
        }

        #[test]
        fn the_newest_governing_one_is_the_authority() {
            let mut kept = Kept::default();
            kept.remember(at("R-002", 200, 47.52));
            kept.remember(at("R-003", 300, 48.61));
            assert_eq!(governs(&kept).as_deref(), Some("R-003"));
        }

        #[test]
        fn a_legacy_or_incomplete_newest_does_not_hide_a_governing_older_one() {
            /*
                D. The historical `46.0` has no provenance and may never gate anything — and it is
                newer than everything, because it was written back on load. A selector that took
                the newest and filtered would answer `None` with `R-002` sitting right there.
            */
            let mut kept = Kept::default();
            kept.remember(at("R-002", 200, 47.52));

            let mut remembered = at("legacy-46", 400, 46.0);
            remembered.legacy = true;
            remembered.sources.clear();
            kept.kept.push(remembered);

            // And one that is not legacy but could not record what a comparison needs.
            let mut blind = at("R-blind", 500, 49.0);
            blind.fingerprint.backend = None;
            kept.kept.push(blind);

            assert_eq!(
                kept.history_of(&shape().key())
                    .first()
                    .map(|it| it.id.clone()),
                Some("R-blind".to_owned()),
                "the newest is neither of the two that may decide",
            );
            assert_eq!(governs(&kept).as_deref(), Some("R-002"));
        }
    }

    #[test]
    fn the_consequence_is_stated_once() {
        /*
            Measured 2026-09-01, from the pause a golden test actually wrote: "… A calibration can
            establish one; no comparative search will run until it does. A calibration can
            establish a reference; until it does, no comparative search will run."

            Two spellings of one sentence, in two files, agreeing by luck until something used
            both.
        */
        let mut session = crate::session::Session::default();
        session.needs_a_reference(&Verdict::NoReference.say(49.8));
        let note = session.note.clone().expect("a note");
        assert_eq!(
            note.matches("no comparative search will run").count(),
            1,
            "said twice: {note}",
        );
        assert!(
            note.contains("49.8 tok/s"),
            "and the reading survives: {note}"
        );
    }

    #[test]
    fn one_current_per_experiment() {
        /*
            Measured 2026-09-01: `R-002` was minted from three fresh calibrations and `R-003` from
            a single control sixty-four seconds later. Both stood `Current` for one fingerprint,
            and `governing_reference_for` picks the newest — so which one governed was decided by a
            timestamp.
        */
        let mut kept = Kept::default();
        let one = |id: &str, at: u64, median: f64| {
            let mut it = known(median);
            it.id = id.to_owned();
            it.at = at;
            it.fingerprint = shape();
            it.fingerprint_id = shape().key();
            it
        };
        kept.remember(one("R-002", 100, 47.52));
        assert_eq!(
            kept.governing_reference_for(&shape().key())
                .map(|it| it.id.clone()),
            Some("R-002".to_owned())
        );

        kept.remember(one("R-003", 200, 48.61));
        assert_eq!(
            kept.governing_reference_for(&shape().key())
                .map(|it| it.id.clone()),
            Some("R-003".to_owned())
        );

        // The older one stands aside; its numbers are untouched.
        let older = kept
            .history_of(&shape().key())
            .into_iter()
            .find(|it| it.id == "R-002")
            .expect("kept");
        assert_eq!(older.standing, crate::reference::Standing::Superseded);
        assert_eq!(
            older.median, 47.52,
            "superseded says it stopped deciding, never that it lied"
        );

        // And a reference for a different experiment is not touched at all.
        let mut elsewhere = one("R-004", 300, 51.0);
        elsewhere.fingerprint_id = "another experiment".to_owned();
        kept.remember(elsewhere);
        let still = kept
            .history_of(&shape().key())
            .into_iter()
            .find(|it| it.id == "R-003")
            .expect("kept");
        assert_eq!(still.standing, crate::reference::Standing::Current);
    }

    #[test]
    fn ok_is_not_a_synonym_for_may_proceed() {
        /*
            **The defect the first golden test found, and the only one it was looking for.**

            Minting refused, `may_begin` answered `Ok(Verdict::NoReference)`, and the caller
            checked `if let Err` — so a candidate was measured with no governing reference. Epoch
            used evidence it had no right to use.

            `Ok` means *this function did not fail*. `may_compare` means *comparative work may
            proceed*, and only `Clean` is that.
        */
        let nothing = may_begin(Some(48.0), None, &shape()).expect("not an error");
        assert_eq!(nothing, Verdict::NoReference);
        assert!(!nothing.may_compare(), "and this is what a caller must ask");

        let elsewhere = crate::reference::Fingerprint {
            build: Some("another build".into()),
            ..shape()
        };
        let other = may_begin(Some(48.0), Some(&known(48.0)), &elsewhere).expect("not an error");
        assert!(matches!(other, Verdict::NotComparable(_)));
        assert!(!other.may_compare());

        // The one that may.
        assert!(may_begin(Some(48.0), Some(&known(48.0)), &shape())
            .expect("clean")
            .may_compare());
    }

    #[test]
    fn a_series_is_the_calibrations_one_run_took() {
        /*
            Measured 2026-09-01: a search took three fresh calibrations that agreed to 0.9% —
            48.17, 47.99, 47.73 — and minting refused, because the set it was handed also held an
            observation from two hours earlier whose runs spanned 5.24%.

            **The fingerprint says what kind of experiment; it does not say when.**
        */
        let mut kept = Kept::default();
        let make = |id: &str, at: u64, rates: Vec<f64>| {
            crate::reference::Observation {
                fingerprint: shape(),
                protocol: crate::protocol::STANDARD_CONTROL_V2.id(),
                rates,
                at,
                ..Default::default()
            }
            .named(id)
        };

        // Two hours ago, during the drift.
        kept.observed(make("O-005", 100, vec![37.30, 39.12, 38.38, 39.32, 38.58]));
        // This run.
        let took: Vec<String> = ["O-007", "O-008", "O-009"]
            .iter()
            .map(|it| (*it).to_owned())
            .collect();
        kept.observed(make("O-007", 200, vec![47.16, 46.60, 48.32, 48.54, 48.17]));
        kept.observed(make("O-008", 210, vec![48.44, 47.90, 47.99, 48.20, 47.50]));
        kept.observed(make("O-009", 220, vec![47.30, 47.50, 47.73, 47.80, 47.90]));

        // Everything with this fingerprint: four, and one of them is not this series.
        assert_eq!(kept.observations_of(&shape().key()).len(), 4);

        // Exactly what the run took.
        let series = kept.these(&took);
        assert_eq!(series.len(), 3);
        assert_eq!(
            series.iter().map(|it| it.id.clone()).collect::<Vec<_>>(),
            took,
        );
        assert!(
            crate::reference::Series::of(&series).reading()
                == crate::reference::SeriesReading::Measured,
            "the three that ran together are one configuration each",
        );

        // And a name nothing answers to is simply absent, never a panic.
        assert!(kept.these(&["O-999".to_owned()]).is_empty());
    }

    fn shape() -> crate::reference::Fingerprint {
        crate::reference::Fingerprint {
            artifact: Some("Qwen3.6-35B-A3B-UD-IQ4_XS".into()),
            artifact_bytes: Some(17_730_509_792),
            backend: Some("CUDA".into()),
            control: Some(crate::optimize::STANDARD_CONTROL.into()),
            measurement_protocol: Some(crate::protocol::STANDARD_CONTROL_V2.id()),
            gpu: Some("a card".into()),
            build: Some("b1".into()),
            runtime: Some("llama_cpp".into()),
            context: Some(32_768),
            cache: Some("f16".into()),
            flash_attention: Some("auto".into()),
            workload: Some("standard control".into()),
            ..Default::default()
        }
    }

    fn known(rate: f64) -> crate::reference::PerformanceReference {
        crate::reference::PerformanceReference {
            id: "r1".into(),
            fingerprint_id: String::new(),
            fingerprint: shape(),
            median: rate,
            dispersion: Tolerance::of(&[45.9, 46.1, 46.7]),
            runs: Some(3),
            rates: Vec::new(),
            observed: Default::default(),
            sources: Vec::new(),
            policy: None,
            between: None,
            at: 1,
            legacy: false,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
        }
    }

    fn reference(rate: f64) -> Reference {
        Reference {
            artifact: "Qwen3.6-35B-A3B-UD-IQ4_XS".to_owned(),
            gpu: "a card".to_owned(),
            build: "b1".to_owned(),
            runtime: "llama_cpp".to_owned(),
            rate,
            tolerance: Tolerance::of(&[45.9, 46.1, 46.7]).expect("three readings"),
            at: 1,
        }
    }

    #[test]
    fn a_machine_that_no_longer_reproduces_its_reference_may_not_start_a_search() {
        /*
            Measured 2026-08-31: a control opened at 28.0 where the healthy figure is ~46. Starting
            an optimizer there produces twenty rows about a machine that does not exist.
        */
        let refused = may_begin(Some(28.0), Some(&known(46.2)), &shape()).expect_err("refused");
        assert!(
            refused.contains("28.0") && refused.contains("46.2"),
            "{refused}"
        );
    }

    #[test]
    fn nothing_to_compare_against_is_its_own_answer_and_is_not_a_pass() {
        /*
            **The defect of 2026-08-31.** This returned `Ok(())` for *no reference at all*, on the
            reasoning that a machine with nothing measured must be allowed its first search —
            which is true, and which made "I have never measured this" indistinguishable from "I
            checked and it is fine". Epoch then reported *"this machine is itself again"* about a
            comparison that never happened.
        */
        let got = may_begin(Some(9.0), None, &shape()).expect("not an error");
        assert_eq!(got, Verdict::NoReference);
        assert!(
            !got.may_compare(),
            "calibration yes, comparative profiles no"
        );
        assert!(
            got.say(9.0).contains("Nothing was compared"),
            "{}",
            got.say(9.0)
        );

        // A control that produced no reading at all is still a failure, not an absence.
        assert!(may_begin(None, None, &shape()).is_err());
    }

    #[test]
    fn a_reference_about_something_else_does_not_become_a_verdict_about_this() {
        // Comparing across builds and calling the difference a degraded machine would send
        // somebody hunting a fault that is a package: one build carries one GPU backend.
        let other = crate::reference::Fingerprint {
            build: Some("a different build".into()),
            ..shape()
        };
        let got = may_begin(Some(40.1), Some(&known(46.0)), &other).expect("not an error");
        assert!(matches!(got, Verdict::NotComparable(_)), "{got:?}");
        assert!(!got.may_compare());
        assert!(
            got.say(40.1).contains("about something else"),
            "{}",
            got.say(40.1)
        );
    }

    #[test]
    fn a_legacy_figure_cannot_decide_that_a_machine_is_broken() {
        /*
            The historical 46.0 on this machine is a number and a timestamp. Using it to declare a
            40.1 control degraded would be a memory with a decimal point deciding whether hours of
            measurement may be trusted — and calling it RECOVERY_REQUIRED would send somebody
            hunting a fault nobody has shown exists.
        */
        let anecdote = crate::reference::PerformanceReference {
            fingerprint: crate::reference::Fingerprint::default(),
            legacy: true,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
            sources: Vec::new(),
            policy: None,
            between: None,
            dispersion: None,
            runs: None,
            rates: Vec::new(),
            observed: Default::default(),
            ..known(46.0)
        };
        let got = may_begin(Some(40.1), Some(&anecdote), &shape()).expect("never an error");
        assert_eq!(got, Verdict::NoReference);
    }

    #[test]
    fn a_healthy_control_starts_normally() {
        let got = may_begin(Some(46.0), Some(&known(46.2)), &shape()).expect("clean");
        assert!(got.may_compare());
        assert!(got.say(46.0).contains("itself again"));
        // And one that came back faster is the machine having settled, not a fault.
        assert!(may_begin(Some(48.0), Some(&known(46.2)), &shape())
            .expect("clean")
            .may_compare());
    }

    #[test]
    fn a_reference_is_about_one_machine_and_one_artefact() {
        let it = reference(46.2);
        assert!(it.about("Qwen3.6-35B-A3B-UD-IQ4_XS", "a card", "b1", "llama_cpp"));
        assert!(!it.about(
            "Qwen3.6-35B-A3B-UD-IQ4_XS",
            "another card",
            "b1",
            "llama_cpp"
        ));
        assert!(!it.about(
            "Qwen3.6-35B-A3B-UD-IQ4_XS · MTP",
            "a card",
            "b1",
            "llama_cpp"
        ));
    }

    #[test]
    fn a_pause_without_a_reference_does_not_invent_one() {
        /*
            A normal user has no use for *known clean reference: —*, and filling the line with a
            number nobody measured would be the invented gauge in the one place this subsystem
            exists to keep honest.
        */
        let bare = Paused {
            session: crate::session::Session::default(),
            last_control: Some(28.0),
            clean_reference_id: None,
            clean_reference_seen: None,
            vram_used: None,
            shared_used: None,
            at: 1,
        };
        let why = bare.why();
        assert!(why.contains("28.0"));
        assert!(!why.contains("clean reference"), "{why}");

        let known = Paused {
            clean_reference_id: Some("r1".into()),
            clean_reference_seen: Some(46.2),
            ..bare
        };
        assert!(known.why().contains("Known clean reference: 46.2"));
    }

    #[test]
    fn a_pause_carries_the_number_the_gate_needs_when_the_list_has_none() {
        /*
            **Measured 2026-08-31, after a real reboot, and it is the whole reason this exists.**

            The paused session carried a clean reference of 46.0 tok/s. `references` was empty —
            only `optimize` writes to that list, and this search stopped long before it got there.
            So `may_begin` had nothing to compare against, returned `Ok`, and RETRY answered *"The
            control reproduces: 40.1 tok/s. This machine is itself again"* — a sentence about a
            comparison that never happened, at the exact moment somebody is deciding whether to
            trust the machine. 40.1 against 46.0 is 13% short.

            One fact was written down in two places and the gate was reading the empty one.
        */
        let paused = Paused {
            session: crate::session::Session::default(),
            last_control: Some(25.5),
            clean_reference_id: Some("r1".into()),
            clean_reference_seen: Some(46.0),
            vram_used: None,
            shared_used: None,
            at: 1,
        };
        // **The pause points at the figure; it does not carry a second copy of it.** The
        // snapshot beside the id is for the record, and nothing reads it to decide anything.
        assert_eq!(paused.clean_reference_id.as_deref(), Some("r1"));

        let mut kept = Kept::default();
        kept.remember(crate::reference::PerformanceReference {
            id: "r1".into(),
            fingerprint_id: String::new(),
            fingerprint: crate::reference::Fingerprint::default(),
            median: 46.0,
            dispersion: None,
            runs: None,
            rates: Vec::new(),
            observed: Default::default(),
            sources: Vec::new(),
            policy: None,
            between: None,
            at: 1,
            legacy: true,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
        });
        let one = kept
            .by_id("r1")
            .expect("the pause points at something real");

        // Even with the lookup fixed, this figure may not decide anything: `46.0` and a date
        // says nothing about the artefact, the build, the context, the cache or the workload.
        assert!(!one.is_safety_reference(), "provenance did not survive");

        // And the band it would have used, had it been usable, still refuses the reading that
        // was called a pass.
        let (band, _) = one.band().expect("a band");
        assert!(!band.reproduces(40.1), "13% short is not the same machine");
        assert!(band.reproduces(45.0));
        assert!(band.reproduces(47.0), "faster is never a refusal");

        // **Not the session's own tolerance**, which was derived from the controls that were
        // already collapsing. Judging a recovery by the spread of the illness passes anything.
        let ill = crate::models::health::Tolerance::of(&[28.0, 27.9, 27.7, 27.3, 26.5, 25.5])
            .expect("a tolerance");
        assert!(
            ill.reproduces(40.1),
            "which is exactly why it must not be used here"
        );
    }

    #[test]
    fn clearing_a_pause_does_not_resume_anything() {
        /*
            The rows before the break and the rows after it were measured on two different
            machines. A new session starts from the beginning.
        */
        let mut kept = Kept::default();
        kept.pause(Paused {
            session: crate::session::Session {
                artifact: "m".to_owned(),
                ..crate::session::Session::default()
            },
            last_control: Some(28.0),
            clean_reference_id: Some("r1".into()),
            clean_reference_seen: Some(46.0),
            vram_used: None,
            shared_used: None,
            at: 1,
        });
        assert!(kept.paused_for("m").is_some());
        assert!(kept.anything_paused().is_some());
        kept.cleared("m");
        assert!(kept.paused_for("m").is_none());
    }

    #[test]
    fn what_a_deck_says_when_epoch_reopens() {
        let paused = Paused {
            session: crate::session::Session {
                artifact: "Qwen3.6-35B-A3B-UD-IQ4_XS".to_owned(),
                ..crate::session::Session::default()
            },
            last_control: Some(28.0),
            clean_reference_id: Some("r1".into()),
            clean_reference_seen: Some(46.2),
            vram_used: None,
            shared_used: None,
            at: 1,
        };
        assert!(paused.waiting().contains("waiting for a clean-state check"));
        assert!(paused.waiting().contains("Qwen3.6-35B-A3B-UD-IQ4_XS"));
    }

    #[test]
    fn what_was_written_reads_back_and_a_broken_file_loses_nothing_new() {
        let here = std::env::temp_dir().join(format!("epoch-sessions-{}", std::process::id()));
        let mut kept = Kept::default();
        kept.remember(crate::reference::PerformanceReference {
            id: "r1".into(),
            fingerprint_id: String::new(),
            fingerprint: crate::reference::Fingerprint {
                artifact: Some("Qwen3.6-35B-A3B-UD-IQ4_XS".into()),
                artifact_bytes: Some(17_730_509_792),
                backend: Some("CUDA".into()),
                control: Some(crate::optimize::STANDARD_CONTROL.into()),
                measurement_protocol: Some(crate::protocol::STANDARD_CONTROL_V2.id()),
                gpu: Some("a card".into()),
                build: Some("b1".into()),
                runtime: Some("llama_cpp".into()),
                context: Some(32_768),
                cache: Some("f16".into()),
                flash_attention: Some("auto".into()),
                workload: Some("standard control".into()),
                ..Default::default()
            },
            median: 46.2,
            dispersion: crate::models::health::Tolerance::of(&[46.2, 46.0, 46.3]),
            runs: Some(3),
            rates: Vec::new(),
            observed: Default::default(),
            sources: Vec::new(),
            policy: None,
            between: None,
            at: 7,
            legacy: false,
            standing: Default::default(),
            superseded_by: None,
            short_term: Default::default(),
            temporal: Default::default(),
        });
        kept.save(&here).expect("saved");
        let read = Kept::load(&here);
        let found = read.references_for("Qwen3.6-35B-A3B-UD-IQ4_XS", "a card", "b1", "llama_cpp");
        assert_eq!(found.len(), 1);
        assert!(
            found[0].is_safety_reference(),
            "a complete one may be a gate"
        );

        std::fs::write(path(&here), "{ not json").expect("written");
        assert!(Kept::load(&here).anything_paused().is_none());
        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    fn two_measurements_of_one_experiment_are_two_rows_and_neither_is_lost() {
        /*
            **Calibration A and Calibration B, 2026-09-01.** Two real observations of one
            experiment: 39.68 and 41.90 tok/s, three runs each, run sets that do not overlap,
            every other reading identical. The store keyed on the fingerprint, so B replaced A —
            and the only interesting thing about the pair, that they disagree, went with it.

            A fingerprint answers *what kind of experiment is this*. A measurement answers *what
            happened this time*. Several of the second share one of the first, and drift is what
            that looks like.
        */
        let here = std::env::temp_dir().join(format!("epoch-history-{}", std::process::id()));
        let mut kept = Kept::default();

        let a = crate::reference::Observation {
            id: String::new(),
            fingerprint_id: String::new(),
            fingerprint: shape(),
            protocol: crate::protocol::STANDARD_CONTROL_V2.id(),
            source: "calibration".into(),
            sequence: Default::default(),
            rates: vec![39.68, 39.32, 40.50, 39.90, 40.10],
            observed: Default::default(),
            at: 100,
        };
        let b = crate::reference::Observation {
            rates: vec![41.44, 41.90, 42.30, 41.70, 42.00],
            at: 200,
            ..a.clone()
        };

        let first = kept.next_reference_id();
        assert_eq!(first, "R-001");
        kept.remember(a.validate(&first, 5).expect("A is a reference"));

        let second = kept.next_reference_id();
        assert_eq!(
            second, "R-002",
            "monotonic, and never derived from the content"
        );
        kept.remember(b.validate(&second, 5).expect("B is a reference"));

        kept.save(&here).expect("saved");
        let read = Kept::load(&here);

        let fid = shape().key();
        let history = read.history_of(&fid);
        assert_eq!(history.len(), 2, "both survive");
        assert_eq!(history[0].id, "R-002", "newest first");
        assert_eq!(history[1].id, "R-001");
        assert!((history[1].median - 39.90).abs() < 0.3);
        assert!((history[0].median - 41.90).abs() < 0.3);

        // The index over the history, not a second store.
        let governs = read.governing_reference_for(&fid).expect("one governs");
        assert_eq!(governs.id, "R-002");

        // A different experiment has its own history and is not mixed in.
        let elsewhere = crate::reference::Fingerprint {
            context: Some(65_536),
            ..shape()
        };
        assert!(read.history_of(&elsewhere.key()).is_empty());
        assert_eq!(read.governing_reference_for(&elsewhere.key()), None);

        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    fn an_older_record_survives_as_a_legacy_figure_rather_than_a_gate() {
        /*
            **Kept, labelled, demoted.** The older shape knows the artefact, the card, the build
            and the runtime and nothing about the context, the cache or the workload — four of
            the things that decide what a throughput number means. Deleting it would throw away
            *we once saw 46 here*, which is real information; promoting it would let a memory with
            a decimal point decide whether hours of measurement may be trusted.
        */
        let here = std::env::temp_dir().join(format!("epoch-legacy-{}", std::process::id()));
        std::fs::create_dir_all(&here).expect("a scratch library");
        std::fs::write(
            path(&here),
            serde_json::to_string(&serde_json::json!({
                "references": [{
                    "artifact": "Qwen3.6-35B-A3B-UD-IQ4_XS",
                    "gpu": "a card",
                    "build": "b1",
                    "runtime": "llama_cpp",
                    "rate": 46.0,
                    "tolerance": { "middle": 46.0, "mad": 0.1, "allowed": 1.38 },
                    "at": 5
                }],
                "paused": []
            }))
            .expect("json"),
        )
        .expect("written");

        let read = Kept::load(&here);
        let found = read.references_for("Qwen3.6-35B-A3B-UD-IQ4_XS", "a card", "b1", "llama_cpp");
        assert_eq!(found.len(), 1, "it is kept");
        assert_eq!(found[0].median, 46.0);
        assert!(found[0].legacy, "and labelled");
        assert!(!found[0].is_safety_reference(), "and it may not be a gate");
        assert!(found[0].describe().contains("LEGACY"));
        let _ = std::fs::remove_dir_all(&here);
    }
}
