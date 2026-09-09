//! Who runs on a Brain, and whether they still can.
//!
//! ## The line this file enforces
//!
//! > **NPC chooses the Brain. MODELS decides how that Brain runs. Context Composer decides what
//! > that Brain sees.**
//!
//! ADR-0026's amendment, and the reason it needed code rather than only a document: a character no
//! longer carries a window, so nothing in a character file can be checked against a window at load
//! time. The check moved to **the moment a profile is applied**, and it is about every character on
//! that Brain rather than whichever one happens to be open.
//!
//! ## Why the moment matters
//!
//! `Required Context ≤ Character Effective Use ≤ Model Context Window`. When the first of those
//! exceeds the last, the character cannot run — and under the old design that was visible while
//! editing the character, because the character named the number. Now it is only visible where the
//! window is chosen.
//!
//! Left unchecked it would surface as a **failed turn**, on whichever character spoke first after
//! the change, hours later, wearing the shape of a model problem. That is the cold-instrument rule
//! arriving three layers too late.

use serde::{Deserialize, Serialize};

use crate::models::health;

/// One character measured against a window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Consumer {
    pub character: String,
    /// What this character needs before anything optional is added: system prompt, identity,
    /// required skills. **Measured from the blocks**, never estimated.
    pub required: u32,
    /// The policy it carries, for the record — it does not change whether it fits.
    pub policy: String,
    /// What it would actually use in this window. `None` where it cannot run at all.
    pub effective: Option<u32>,
}

impl Consumer {
    pub fn fits(&self) -> bool {
        self.effective.is_some()
    }
}

/// What applying a profile would do to everyone on that Brain.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Impact {
    pub model: String,
    pub window: u32,
    pub consumers: Vec<Consumer>,
}

impl Impact {
    /// Whether every character on this Brain can still run.
    pub fn everyone_fits(&self) -> bool {
        self.consumers.iter().all(Consumer::fits)
    }

    /// The ones that cannot.
    pub fn cannot_run(&self) -> Vec<&Consumer> {
        self.consumers.iter().filter(|it| !it.fits()).collect()
    }

    /// What Epoch says before applying, whether or not it refuses.
    ///
    /// **Every consumer is listed, not only the failures.** Somebody about to give up half their
    /// window wants to see the three characters that are fine as much as the one that is not —
    /// a report of only the problems is a report you cannot check.
    pub fn report(&self) -> Vec<String> {
        let mut said = vec![format!("Applying {} · {}K", self.model, self.window / 1024)];
        for one in &self.consumers {
            said.push(match one.effective {
                Some(_) => format!(
                    "  \u{2713} {:<14} requires {}K",
                    one.character,
                    one.required.div_ceil(1024)
                ),
                None => format!(
                    "  \u{26a0} {:<14} requires {}K \u{2014} cannot run with this configuration",
                    one.character,
                    one.required.div_ceil(1024)
                ),
            });
        }
        said
    }
}

/// Work out what a window would mean for a set of characters.
///
/// `required_of` measures what each character's required blocks cost. It is a closure because the
/// measuring belongs to the Composer and the deciding belongs here — and because a test can then
/// state a cost rather than build a character to have one.
pub fn impact_of(
    model: &str,
    window: u32,
    characters: &[(String, epoch_kernel::ContextPolicy, Option<u32>)],
    required_of: &dyn Fn(&str) -> u32,
) -> Impact {
    Impact {
        model: model.to_owned(),
        window,
        consumers: characters
            .iter()
            .map(|(name, policy, asked)| {
                let required = required_of(name);
                Consumer {
                    character: name.clone(),
                    required,
                    policy: policy.name().to_owned(),
                    effective: epoch_kernel::mind::effective_use(window, required, *policy, *asked),
                }
            })
            .collect(),
    }
}

/// Whether a measured configuration should be offered as an ordinary choice.
///
/// ## Capability gates the list rather than sitting beside it
///
/// The owner's rule, and it is what stops MODELS offering a window that loads and forgets. If the
/// context benchmark measured `effective = 128K`, then a 256K profile is not one of four equal
/// options — it is the maximum that **loads**, which is a different claim.
///
/// It is never hidden. `Advanced` still offers it, with what was measured about it, because
/// *can load* and *should normally use* are two facts and somebody may want the first.
pub fn ordinarily_usable(context: u32, effective: Option<u32>) -> bool {
    effective.is_none_or(|it| context <= it)
}

/// How a row is labelled in the list of measured configurations.
///
/// **The standard row is marked**, because it is the only one comparable against another model.
/// Without that, somebody reads `Gemma 40 t/s at 256K` beside `Qwen 46 t/s at 32K` and concludes
/// something false — the two are not the same benchmark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Badge {
    /// The window every model is compared at. Not necessarily the one to use.
    StandardBenchmark,
    /// Epoch's pick, from the capability measurement.
    Recommended,
    /// Longer than recommended and still holding up.
    LongContext,
    /// It loads, and the capability measurement says it stops using what is in it.
    MaxLoadable,
    /// An ordinary row.
    Plain,
}

impl Badge {
    pub fn label(self) -> &'static str {
        match self {
            Badge::StandardBenchmark => "STANDARD BENCHMARK",
            Badge::Recommended => "RECOMMENDED",
            Badge::LongContext => "LONG CONTEXT",
            Badge::MaxLoadable => "MAX LOADABLE",
            Badge::Plain => "",
        }
    }

    /// Whether this row belongs in the ordinary list or behind `Advanced`.
    pub fn ordinary(self) -> bool {
        self != Badge::MaxLoadable
    }
}

/// Which badge a measured window earns.
pub fn badge(
    context: u32,
    standard: u32,
    recommended: Option<u32>,
    effective: Option<u32>,
) -> Badge {
    if !ordinarily_usable(context, effective) {
        return Badge::MaxLoadable;
    }
    if Some(context) == recommended {
        return Badge::Recommended;
    }
    if context == standard {
        return Badge::StandardBenchmark;
    }
    if recommended.is_some_and(|it| context > it) {
        return Badge::LongContext;
    }
    Badge::Plain
}

/// Where a window reading came from.
///
/// ## A reading has to say what it is a reading of
///
/// The sixth face of the cold-instrument rule, and the quietest one: *a correct reading of the
/// right quantity, with nothing saying what it is a reading of*. `64K` means three different
/// things depending on who decided it, and a character's panel that printed the number alone
/// would be the gauge that identifies nobody.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Source {
    /// A profile the user applied in MODELS, from a search that ran here.
    Profile,
    /// What llama.cpp will be started with next time. Chosen in MODELS, never searched.
    Loadout,
    /// What the backend itself reports for this model — Ollama, LM Studio, a hosted API.
    /// Nothing in MODELS decided it, and Epoch does not decide it either.
    Reported,
    /// Nobody has said.
    #[default]
    Nothing,
}

impl Source {
    /// The qualifier printed beside the number.
    pub fn about(self) -> &'static str {
        match self {
            Source::Profile => "measured",
            Source::Loadout => "configured",
            Source::Reported => "reported by the model",
            Source::Nothing => "",
        }
    }
}

/// A one-line summary of a Brain's window, for a character's read-only view.
///
/// **Read-only is the whole point.** A character no longer has a window to edit; what it has is a
/// view of the one MODELS applied, and a link to where that is decided.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Inherited {
    pub model: String,
    /// `None` where nothing has been applied — which reads as *not configured*, never as a guess.
    pub window: Option<u32>,
    /// What the number above is a reading of.
    pub source: Source,
    /// Which profile, where one was applied. `None` for every other source.
    pub profile: Option<String>,
    /// Tokens per second, **and only ever from an applied profile**.
    ///
    /// A model's last timing was taken at whatever loadout was current that afternoon, so
    /// printing it beside a window somebody chose afterwards is a real reading of a different
    /// configuration — the most convincing way a gauge can lie. A window with no profile behind
    /// it therefore carries no speed at all, which is honest and reads as `—`.
    pub generation: Option<f64>,
    /// Whether the measurement behind it was stable.
    pub stable: bool,
}

/*
    **No `effective` and no `policy` here, deliberately.** Both were on this struct for a day and
    neither had a consumer, and carrying them would have made `None` mean two things one file
    apart: on `Consumer` it means *this character cannot run in that window*, and here it could
    only ever have meant *nobody measured what its required blocks cost*. Two spellings of one
    fact agree by luck.

    Measuring a character's Required cost is the Composer's job and needs a turn to do it against
    — which is why `impact_of` takes a closure rather than a number. This view is a reading of
    MODELS, and the character's own policy is already in the character.
*/

impl Inherited {
    /// Whether MODELS has anything to say about this Brain at all.
    pub fn configured(&self) -> bool {
        self.window.is_some()
    }
}

/// Median of a set of readings, so a summary quotes what was measured rather than the last one.
pub fn middle(rates: &[f64]) -> Option<f64> {
    health::median(rates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::ContextPolicy;

    fn crew() -> Vec<(String, ContextPolicy, Option<u32>)> {
        vec![
            ("Mage".to_owned(), ContextPolicy::Adaptive, None),
            ("Guardian".to_owned(), ContextPolicy::Compact, None),
            ("Historian".to_owned(), ContextPolicy::Long, None),
        ]
    }

    fn costs(name: &str) -> u32 {
        match name {
            "Mage" => 9_000,
            "Guardian" => 6_000,
            "Historian" => 38_000,
            _ => 1_000,
        }
    }

    #[test]
    fn applying_a_window_says_who_can_no_longer_run_before_it_is_applied() {
        /*
            The owner's example. Under the old design this was visible while editing a character,
            because the character named the number. Now it is only visible where the window is
            chosen — and unchecked it would arrive as a failed turn, hours later, on whichever
            character spoke first.
        */
        let it = impact_of("Gemma 4 12B", 32_768, &crew(), &costs);
        assert!(!it.everyone_fits());
        let broken = it.cannot_run();
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0].character, "Historian");

        let said = it.report().join("\n");
        assert!(said.contains("✓ Mage"), "{said}");
        assert!(said.contains("✓ Guardian"), "{said}");
        assert!(said.contains("⚠ Historian"), "{said}");
        assert!(said.contains("38K"), "{said}");
    }

    #[test]
    fn every_consumer_is_listed_and_not_only_the_failures() {
        // A report of only the problems is a report you cannot check.
        let it = impact_of("Gemma 4 12B", 32_768, &crew(), &costs);
        assert_eq!(it.report().len(), 4, "a heading and three characters");
    }

    #[test]
    fn a_window_everyone_fits_in_says_so_plainly() {
        let it = impact_of("Gemma 4 12B", 262_144, &crew(), &costs);
        assert!(it.everyone_fits());
        assert!(it.cannot_run().is_empty());
        assert!(it.report().iter().all(|it| !it.contains('⚠')));
    }

    #[test]
    fn the_policy_never_decides_whether_somebody_fits() {
        /*
            The floor is what fits, and the floor is Required. A `Compact` character and a `Long`
            one with the same required cost both fit or both do not — the policy divides what is
            *optional*, which only exists once the floor is paid.
        */
        let compact = impact_of(
            "m",
            32_768,
            &[("Historian".to_owned(), ContextPolicy::Compact, None)],
            &costs,
        );
        let long = impact_of(
            "m",
            32_768,
            &[("Historian".to_owned(), ContextPolicy::Long, None)],
            &costs,
        );
        assert_eq!(compact.everyone_fits(), long.everyone_fits());
        assert!(!compact.everyone_fits());
    }

    #[test]
    fn capability_gates_the_ordinary_list_without_hiding_what_loads() {
        /*
            If the context benchmark measured `effective = 128K`, a 256K profile is not one of four
            equal options — it is the maximum that *loads*. Never hidden: `can load` and `should
            normally use` are two facts and somebody may want the first.
        */
        assert!(ordinarily_usable(65_536, Some(131_072)));
        assert!(ordinarily_usable(131_072, Some(131_072)));
        assert!(!ordinarily_usable(262_144, Some(131_072)));
        // Nothing measured gates nothing — silence is not a refusal.
        assert!(ordinarily_usable(262_144, None));
    }

    #[test]
    fn the_standard_row_is_marked_so_it_is_not_compared_with_another_models_long_one() {
        /*
            Without the badge, somebody reads `Gemma 40 t/s at 256K` beside `Qwen 46 t/s at 32K`
            and concludes something false. They are not the same benchmark.
        */
        let standard = 32_768;
        let recommended = Some(65_536);
        let effective = Some(131_072);

        assert_eq!(
            badge(32_768, standard, recommended, effective),
            Badge::StandardBenchmark,
        );
        assert_eq!(
            badge(65_536, standard, recommended, effective),
            Badge::Recommended
        );
        assert_eq!(
            badge(131_072, standard, recommended, effective),
            Badge::LongContext
        );
        assert_eq!(
            badge(262_144, standard, recommended, effective),
            Badge::MaxLoadable
        );
    }

    #[test]
    fn the_row_that_loads_and_forgets_is_the_only_one_kept_out_of_the_ordinary_list() {
        assert!(!Badge::MaxLoadable.ordinary());
        for one in [
            Badge::StandardBenchmark,
            Badge::Recommended,
            Badge::LongContext,
            Badge::Plain,
        ] {
            assert!(one.ordinary(), "{one:?}");
        }
    }

    #[test]
    fn a_brain_with_nothing_applied_reads_as_unconfigured_rather_than_as_a_guess() {
        let nothing = Inherited::default();
        assert_eq!(nothing.window, None);
        assert_eq!(nothing.generation, None);
        assert_eq!(nothing.source, Source::Nothing);
        assert!(!nothing.configured());
    }

    #[test]
    fn a_window_says_which_of_three_things_it_is_a_reading_of() {
        /*
            `64K` means three different things depending on who decided it: a profile Epoch
            searched for, a loadout somebody picked, or a number the backend reports about a
            model nothing here configures. Printed alone it is the gauge that identifies nobody.
        */
        assert_eq!(Source::Profile.about(), "measured");
        assert_eq!(Source::Loadout.about(), "configured");
        assert_eq!(Source::Reported.about(), "reported by the model");
        assert_eq!(Source::Nothing.about(), "");
        assert_eq!(Source::default(), Source::Nothing);
    }
}
