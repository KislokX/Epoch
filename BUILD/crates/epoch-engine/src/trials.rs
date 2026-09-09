//! What a model can be asked that has a right answer.
//!
//! ## Why this is separate from `bench`
//!
//! [`crate::bench`] measures how fast a model is. This measures whether it is any good, and the
//! two are different questions with different failure modes: a benchmark that mixed them would
//! report a model that answers instantly and wrongly as a good result.
//!
//! ## Nothing here is a score somebody invented
//!
//! Four kinds of trial, and every one is checked by a machine against a fact:
//!
//! - **Reasoning** — a question with a verifiable answer. Right or wrong, and a percentage.
//! - **Coding** — a problem with tests, and **the code is actually run**. Tests passed of tests
//!   total. Reading the answer and deciding it looks correct is not a measurement.
//! - **Following** — constraints that can be checked: valid JSON, exactly seven items, never the
//!   word *however*, at most forty words. Each one passes or fails on its own.
//! - **Tools** — five separate questions, because "did it call a tool" is one bit and the useful
//!   information is in the other four: the right tool, the right arguments, a valid shape, a call
//!   that ran, and the right answer where the answer can be checked.
//!
//! **General quality is deliberately not here.** It is a judgement, so it stays a judgement: the
//! answers are kept and shown as Model A · B · C · D · E, and a person reads them without knowing
//! which is which ([`Blind`]).
//!
//! ## Running code a model wrote
//!
//! [`Coding`] executes what the model produced, which is the only way to know whether it works.
//! That is a real thing to do to somebody's computer, so it is bounded and it is said out loud:
//! a temporary directory that is deleted afterwards, a timeout, and nothing else — no project
//! root, no network work, no arguments from the model. The user asked for a benchmark; they did
//! not ask for a shell.

use serde::{Deserialize, Serialize};

/// One thing to ask, and how the answer is judged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Trial {
    /// A question whose answer can be compared with a fact.
    Reasoning {
        id: String,
        prompt: String,
        /// Every spelling that counts as right. Compared after normalising case, spaces and
        /// punctuation — a model that answers `42.` rather than `42` got it right.
        answers: Vec<String>,
    },
    /// A program, and the tests it has to pass.
    Coding {
        id: String,
        prompt: String,
        /// Appended to whatever the model wrote and run with it. Each assertion that survives is
        /// a test passed.
        tests: String,
        /// How many separate assertions `tests` makes. Stated rather than counted from the
        /// source, because counting `assert` in a string is a guess about somebody's formatting.
        total: u32,
    },
    /// Constraints a machine can check.
    Following {
        id: String,
        prompt: String,
        must: Vec<Check>,
    },
    /// A tool offered, and what calling it correctly looks like.
    Tools {
        id: String,
        prompt: String,
        /// The tools the model is given. The right one is `expect.tool`; the others are there so
        /// choosing is a choice.
        offered: Vec<Tool>,
        expect: Expected,
    },
    /// Several tools, several turns, and the answer at the end.
    ///
    /*
        **What Suite v1's `Tools` could not ask.** One call of one tool measures whether a model
        can reach for something. It cannot measure choosing between two tools that look alike,
        carrying a result from one call into the next, noticing that a tool answered with an
        error, or — the one everybody forgets — **not calling anything when nothing is needed**.

        The steps are a script the harness plays: each one says what should be called, and what
        the harness answers when it is. An answer may be an error, which is how recovery is
        measured rather than assumed.

        An empty `steps` is the *should not call* case, and it is a real trial: a model that
        reaches for a tool to answer `what is 2 + 2` has failed something worth failing.
    */
    Agentic {
        id: String,
        prompt: String,
        offered: Vec<Tool>,
        /// In order. Empty means nothing should be called at all.
        steps: Vec<Turn>,
        /// What the final answer has to satisfy, once the calling is done.
        finally: Vec<Check>,
    },
}

/// One expected call, and what the harness answers to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Turn {
    pub tool: String,
    /// Arguments that must be present with these values. Extra ones are not a failure.
    pub arguments: serde_json::Map<String, serde_json::Value>,
    /// What the tool returns. **Sometimes an error on purpose** — a model that carries on as if a
    /// failed call succeeded is a model that will do that with somebody's files.
    pub responds: String,
}

impl Trial {
    pub fn id(&self) -> &str {
        match self {
            Trial::Reasoning { id, .. }
            | Trial::Coding { id, .. }
            | Trial::Following { id, .. }
            | Trial::Tools { id, .. }
            | Trial::Agentic { id, .. } => id,
        }
    }

    pub fn prompt(&self) -> &str {
        match self {
            Trial::Reasoning { prompt, .. }
            | Trial::Coding { prompt, .. }
            | Trial::Following { prompt, .. }
            | Trial::Tools { prompt, .. }
            | Trial::Agentic { prompt, .. } => prompt,
        }
    }
}

/// One constraint that a machine can decide.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "check", content = "of")]
pub enum Check {
    /// The whole answer parses as JSON.
    Json,
    /// The answer is a JSON array of exactly this many items.
    Items(usize),
    /// A JSON object with every one of these keys.
    Keys(Vec<String>),
    /// Exactly this many non-empty lines.
    Lines(usize),
    /// This must appear, case-insensitively.
    Contains(String),
    /// This must not appear, case-insensitively.
    Forbids(String),
    /// No more than this many words.
    AtMostWords(usize),
    /// The answer begins with this, ignoring surrounding space.
    StartsWith(String),

    // ---- added for Suite v2, where one prompt carries several constraints at once ----
    /// At least this many words. Pairs with `AtMostWords` to pin a length rather than cap it.
    AtLeastWords(usize),
    /// A JSON object where this key holds exactly this value. Compared as JSON, so `2` and `"2"`
    /// are different — which is the point when a schema is what is being tested.
    KeyIs(String, serde_json::Value),
    /// A JSON object that must **not** carry this key. The negative half of `Keys`, and the one
    /// that catches a model adding what it was told to leave out.
    Lacks(String),
    /// Every non-empty line begins with this.
    EveryLineStartsWith(String),
    /// The first of these appears before the second. Order is a constraint a model can satisfy
    /// by accident once and never twice, which is why it is worth checking.
    Before(String, String),
    /// None of these appear. One check rather than several `Forbids`, so a prompt that bans a
    /// list reads as one constraint.
    NoneOf(Vec<String>),
    /// A JSON array whose items, in order, are exactly these. Stronger than `Items`, which only
    /// counts.
    ItemsAre(Vec<serde_json::Value>),
}

impl Check {
    /// Whether an answer satisfies it, and why not when it does not.
    pub fn against(&self, said: &str) -> Result<(), String> {
        let trimmed = said.trim();
        /*
            **Nothing satisfies nothing.** `AtMostWords(20)` is true of an empty string, and
            `Forbids("light")` is true of it too — so a model that said nothing at all scored
            *following 100%* on the first real run, which is the one number on that card nobody
            would have questioned.

            A check that passes vacuously is the gate-with-nothing-behind-it inverted: not a
            refusal read out of silence, but a *pass* read out of it. Both are the harness
            answering a question the model never did.

            `attempt` already turns a truncated answer into `Refused` before it reaches here, so
            what this catches is the other empty: a model that stopped cleanly with nothing to
            say. That is an answer, and it is not a good one.
        */
        if trimmed.is_empty() {
            return Err("it said nothing".to_owned());
        }
        match self {
            Check::Json => serde_json::from_str::<serde_json::Value>(trimmed)
                .map(|_| ())
                .map_err(|why| format!("not JSON: {why}")),
            Check::Items(want) => {
                let value: serde_json::Value =
                    serde_json::from_str(trimmed).map_err(|why| format!("not JSON: {why}"))?;
                let array = value.as_array().ok_or("not a JSON array")?;
                (array.len() == *want)
                    .then_some(())
                    .ok_or_else(|| format!("{} items, not {want}", array.len()))
            }
            Check::Keys(want) => {
                let value: serde_json::Value =
                    serde_json::from_str(trimmed).map_err(|why| format!("not JSON: {why}"))?;
                let object = value.as_object().ok_or("not a JSON object")?;
                let missing: Vec<&String> = want
                    .iter()
                    .filter(|key| !object.contains_key(*key))
                    .collect();
                missing
                    .is_empty()
                    .then_some(())
                    .ok_or_else(|| format!("missing {missing:?}"))
            }
            Check::Lines(want) => {
                let got = trimmed
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count();
                (got == *want)
                    .then_some(())
                    .ok_or_else(|| format!("{got} lines, not {want}"))
            }
            Check::Contains(word) => trimmed
                .to_lowercase()
                .contains(&word.to_lowercase())
                .then_some(())
                .ok_or_else(|| format!("never says {word:?}")),
            Check::Forbids(word) => (!trimmed.to_lowercase().contains(&word.to_lowercase()))
                .then_some(())
                .ok_or_else(|| format!("says {word:?}")),
            Check::AtMostWords(most) => {
                let got = trimmed.split_whitespace().count();
                (got <= *most)
                    .then_some(())
                    .ok_or_else(|| format!("{got} words, more than {most}"))
            }
            Check::StartsWith(head) => trimmed
                .to_lowercase()
                .starts_with(&head.to_lowercase())
                .then_some(())
                .ok_or_else(|| format!("does not begin with {head:?}")),

            Check::AtLeastWords(least) => {
                let got = trimmed.split_whitespace().count();
                (got >= *least)
                    .then_some(())
                    .ok_or_else(|| format!("{got} words, fewer than {least}"))
            }
            Check::KeyIs(key, want) => {
                let value: serde_json::Value =
                    serde_json::from_str(trimmed).map_err(|why| format!("not JSON: {why}"))?;
                let got = value.get(key).ok_or_else(|| format!("no key {key:?}"))?;
                (got == want)
                    .then_some(())
                    .ok_or_else(|| format!("{key:?} is {got}, not {want}"))
            }
            Check::Lacks(key) => {
                let value: serde_json::Value =
                    serde_json::from_str(trimmed).map_err(|why| format!("not JSON: {why}"))?;
                value
                    .get(key)
                    .is_none()
                    .then_some(())
                    .ok_or_else(|| format!("carries {key:?}, which was forbidden"))
            }
            Check::EveryLineStartsWith(want) => {
                let bad = trimmed
                    .lines()
                    .filter(|it| !it.trim().is_empty())
                    .find(|it| !it.trim_start().starts_with(want.as_str()));
                match bad {
                    Some(line) => Err(format!("a line does not begin with {want:?}: {line:?}")),
                    None => Ok(()),
                }
            }
            Check::Before(first, second) => {
                let said = trimmed.to_lowercase();
                let a = said.find(&first.to_lowercase());
                let b = said.find(&second.to_lowercase());
                match (a, b) {
                    (Some(a), Some(b)) if a < b => Ok(()),
                    (Some(_), Some(_)) => Err(format!("{second:?} comes before {first:?}")),
                    _ => Err(format!("{first:?} and {second:?} are not both there")),
                }
            }
            Check::NoneOf(banned) => {
                let said = trimmed.to_lowercase();
                match banned.iter().find(|it| said.contains(&it.to_lowercase())) {
                    Some(found) => Err(format!("contains {found:?}, which was forbidden")),
                    None => Ok(()),
                }
            }
            Check::ItemsAre(want) => {
                let value: serde_json::Value =
                    serde_json::from_str(trimmed).map_err(|why| format!("not JSON: {why}"))?;
                let got = value
                    .as_array()
                    .ok_or_else(|| "not a JSON array".to_owned())?;
                (got == want)
                    .then_some(())
                    .ok_or_else(|| format!("{} items, and not these ones", got.len()))
            }
        }
    }
}

/// A tool as it is offered to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// The JSON Schema of its arguments, as a provider would be given it.
    pub arguments: serde_json::Value,
}

/// What calling the right tool correctly looks like.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Expected {
    pub tool: String,
    /// Arguments that must be present with these values. Extra arguments are not a failure of
    /// *arguments* — a model that adds an optional one has still called it correctly.
    pub arguments: serde_json::Map<String, serde_json::Value>,
    /// What the tool answers when called correctly, for the trials where that is checkable.
    /// `None` where there is nothing to compare — and that is reported as *not checked* rather
    /// than as a pass.
    pub answer: Option<String>,
}

/// A tool call, however the model expressed it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Called {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// The five things worth knowing about a tool call, kept apart.
///
/// **"Did it emit tool_calls" is one bit and the least useful of the five.** A model that called
/// the wrong tool, or the right tool with nonsense, or a shape the schema refuses, has emitted a
/// tool call and done nothing useful — and a single pass/fail would score all of those the same.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    /// It made a call at all.
    pub called: bool,
    /// It called the tool the question was about.
    pub right_tool: bool,
    /// Every argument the answer needs, with the value it needs.
    pub right_arguments: bool,
    /// The arguments fit the schema the tool declared.
    pub valid_shape: bool,
    /// The call ran without the tool refusing it.
    pub ran: bool,
    /// The result matched. `None` where the trial has nothing to compare — never `Some(true)`
    /// for a thing nobody checked.
    pub right_answer: Option<bool>,
    /// What went wrong, in whichever layer's own words.
    pub note: Option<String>,
}

/// How a single trial went.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Outcome {
    Reasoning {
        correct: bool,
    },
    Coding {
        passed: u32,
        total: u32,
        /// What the runner said when it did not pass. The interpreter's own words.
        note: Option<String>,
    },
    Following {
        /// One entry per constraint, in the order they were asked for. `Ok(())` is a pass.
        checks: Vec<Result<(), String>>,
    },
    Tools(ToolResult),
    /// Several turns of tool use, and the answer at the end.
    ///
    /*
        **Five dimensions, never collapsed into one PASS.** The owner's list: tool selection,
        schema validity, argument correctness, execution success, final task success. A model that
        picked the right tool with the wrong arguments and a model that picked the wrong tool
        entirely are both failures and they are not the same failure, and a benchmark that cannot
        tell them apart cannot tell anybody what to fix.
    */
    Agentic {
        /// Every step that was expected, and how the call for it went. Shorter than the script
        /// where the model stopped early.
        steps: Vec<ToolResult>,
        /// How many were expected.
        expected: usize,
        /// Whether it stopped calling when it should have.
        stopped_correctly: bool,
        /// The constraints on the final answer.
        finally: Vec<Result<(), String>>,
        /// What the model said at the end, for the record.
        note: Option<String>,
    },
    /// The model did not answer at all.
    Refused {
        why: String,
    },
}

impl Outcome {
    /// The fraction of this trial that was satisfied, for a table that adds them up.
    ///
    /// **A trial nobody could run has no fraction.** `None` rather than zero: a model that was
    /// never asked is not a model that answered badly, and averaging a zero in would say it was.
    pub fn score(&self) -> Option<f64> {
        match self {
            Outcome::Reasoning { correct } => Some(if *correct { 1.0 } else { 0.0 }),
            Outcome::Coding { passed, total, .. } => {
                (*total > 0).then(|| f64::from(*passed) / f64::from(*total))
            }
            Outcome::Following { checks } => {
                if checks.is_empty() {
                    return None;
                }
                let good = checks.iter().filter(|it| it.is_ok()).count();
                Some(good as f64 / checks.len() as f64)
            }
            Outcome::Tools(it) => {
                // The five, with the fifth counted only where there was something to check —
                // otherwise a trial with no verifiable answer scores worse than one with.
                let mut had = vec![
                    it.called,
                    it.right_tool,
                    it.right_arguments,
                    it.valid_shape,
                    it.ran,
                ];
                if let Some(right) = it.right_answer {
                    had.push(right);
                }
                let good = had.iter().filter(|it| **it).count();
                Some(good as f64 / had.len() as f64)
            }
            Outcome::Agentic {
                steps,
                expected,
                stopped_correctly,
                finally,
                ..
            } => {
                /*
                    **Averaged across the dimensions that were asked for, and nothing invented.**
                    Each expected step contributes what its call scored; stopping correctly is one
                    part; each final constraint is one part. A trial with no steps and no final
                    constraints has nothing to score and says so.
                */
                let mut parts: Vec<f64> = Vec::new();
                for step in steps {
                    parts.push(Outcome::Tools(step.clone()).score().unwrap_or(0.0));
                }
                // Steps the model never reached count as zero rather than being left out —
                // stopping halfway is a failure of the task, not an absence of evidence.
                parts.resize(parts.len() + expected.saturating_sub(steps.len()), 0.0);
                parts.push(if *stopped_correctly { 1.0 } else { 0.0 });
                for check in finally {
                    parts.push(if check.is_ok() { 1.0 } else { 0.0 });
                }
                (!parts.is_empty()).then(|| parts.iter().sum::<f64>() / parts.len() as f64)
            }
            Outcome::Refused { .. } => None,
        }
    }
}

/// One model's answer to one trial, kept whole.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Answered {
    pub trial: String,
    /// The model, as it is really called. Never shown beside the text in a blind reading.
    pub model: String,
    /// Everything it said, unedited. This is the half a person judges.
    pub said: String,
    pub outcome: Outcome,
}

/// Answers with the models hidden, for a judgement that cannot be swayed by the name.
///
/// **The labels are positional and the mapping is kept.** A blind reading is worth nothing if the
/// order gives it away, and worth nothing either if nobody can find out afterwards which was
/// which — so the letters follow the order the models were run in and [`Blind::who`] answers when
/// the reading is done.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Blind {
    /// Letter to model. `A` is the first model measured, not the best or the worst.
    pub who: std::collections::BTreeMap<String, String>,
}

impl Blind {
    /// Label each model, in the order given.
    pub fn of(models: &[String]) -> Self {
        let mut who = std::collections::BTreeMap::new();
        for (at, model) in models.iter().enumerate() {
            // A–Z, then AA, AB… for a bench with more than twenty-six, which is not a case
            // anybody has but is cheaper than an unwrap that could one day be wrong.
            who.insert(letter(at), model.clone());
        }
        Self { who }
    }

    /// The letter a model was given.
    pub fn letter_of(&self, model: &str) -> Option<&str> {
        self.who
            .iter()
            .find(|(_, name)| name.as_str() == model)
            .map(|(letter, _)| letter.as_str())
    }
}

fn letter(mut at: usize) -> String {
    let mut said = String::new();
    loop {
        said.insert(0, char::from(b'A' + u8::try_from(at % 26).unwrap_or(0)));
        if at < 26 {
            return said;
        }
        at = at / 26 - 1;
    }
}

/// Judge one answer against its trial.
///
/// Everything except [`Trial::Coding`] and [`Trial::Tools`] is decided here; those two need to
/// run something, and each has its own function so the thing that executes is one place.
pub fn judge(trial: &Trial, said: &str) -> Outcome {
    match trial {
        Trial::Reasoning { answers, .. } => Outcome::Reasoning {
            correct: answers.iter().any(|want| says(said, want)),
        },
        Trial::Following { must, .. } => Outcome::Following {
            checks: must.iter().map(|check| check.against(said)).collect(),
        },
        // Judged by running, not by reading.
        Trial::Coding { .. } | Trial::Tools { .. } | Trial::Agentic { .. } => Outcome::Refused {
            why: "this trial is judged by running it".to_owned(),
        },
    }
}

/// Whether an answer contains the expected one, once both are normalised.
///
/// **Contains rather than equals**, because a model asked for a number answers *The answer is 42.*
/// and that is right. Normalised so `42.` and `42` and `  42 ` are one thing — a benchmark that
/// marked a correct answer wrong for its punctuation would be measuring formatting.
fn says(said: &str, want: &str) -> bool {
    let tidy = |raw: &str| {
        raw.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    };
    let said = tidy(said);
    let want = tidy(want);
    if want.is_empty() {
        return false;
    }
    // Word-bounded: `4` must not match inside `42`.
    said.split(' ').any(|word| word == want) || said.contains(&format!(" {want} "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_right_answer_wrapped_in_a_sentence_is_still_right() {
        let trial = Trial::Reasoning {
            id: "r1".into(),
            prompt: "What is 6 times 7?".into(),
            answers: vec!["42".into()],
        };
        for said in ["42", "The answer is 42.", "  42  ", "**42**"] {
            assert_eq!(
                judge(&trial, said),
                Outcome::Reasoning { correct: true },
                "{said:?} is right"
            );
        }
    }

    #[test]
    fn a_number_inside_another_number_is_not_the_answer() {
        // `4` must not match inside `42`, or every trial with a small answer is free.
        let trial = Trial::Reasoning {
            id: "r2".into(),
            prompt: "?".into(),
            answers: vec!["4".into()],
        };
        assert_eq!(judge(&trial, "42"), Outcome::Reasoning { correct: false });
        assert_eq!(
            judge(&trial, "it is 4"),
            Outcome::Reasoning { correct: true }
        );
    }

    #[test]
    fn every_constraint_is_decided_on_its_own() {
        // One failure among four is three quarters, not a zero — and the reason for the failure
        // is kept, because "it did not follow instructions" is not something to act on.
        let trial = Trial::Following {
            id: "f1".into(),
            prompt: "?".into(),
            must: vec![
                Check::Json,
                Check::Items(3),
                Check::Forbids("however".into()),
                Check::AtMostWords(20),
            ],
        };
        let Outcome::Following { checks } = judge(&trial, r#"["a","b"]"#) else {
            panic!("that is a Following trial");
        };
        assert!(checks[0].is_ok(), "it is JSON");
        assert!(checks[1].is_err(), "and it has two items");
        assert!(
            checks[1].as_ref().unwrap_err().contains("2 items"),
            "{checks:?}"
        );
        assert!(checks[2].is_ok());
        assert!(checks[3].is_ok());

        let score = judge(&trial, r#"["a","b"]"#).score().expect("four checks");
        assert!((score - 0.75).abs() < 1e-9, "{score}");
    }

    #[test]
    fn a_shape_check_says_what_is_missing() {
        let keys = Check::Keys(vec!["name".into(), "age".into()]);
        assert!(keys.against(r#"{"name":"a","age":1}"#).is_ok());
        let why = keys.against(r#"{"name":"a"}"#).expect_err("no age");
        assert!(why.contains("age"), "{why}");
        // And a thing that is not an object at all says that rather than listing keys.
        assert!(keys.against("[1,2]").expect_err("array").contains("object"));
    }

    #[test]
    fn a_trial_nobody_could_run_has_no_score() {
        // Never a zero: a model that was not asked is not a model that answered badly, and a
        // zero averaged into a table says it was.
        assert_eq!(
            Outcome::Refused {
                why: "server was down".into()
            }
            .score(),
            None
        );
        assert_eq!(Outcome::Following { checks: vec![] }.score(), None);
        assert_eq!(
            Outcome::Coding {
                passed: 0,
                total: 0,
                note: None
            }
            .score(),
            None
        );
    }

    #[test]
    fn a_tool_call_is_five_questions_and_the_fifth_may_not_apply() {
        /*
            **"Did it emit tool_calls" is the least useful of the five.** A model that called the
            wrong tool has emitted one and done nothing; scoring that the same as a correct call
            is what this shape exists to prevent.
        */
        let everything = ToolResult {
            called: true,
            right_tool: true,
            right_arguments: true,
            valid_shape: true,
            ran: true,
            right_answer: Some(true),
            note: None,
        };
        assert_eq!(Outcome::Tools(everything).score(), Some(1.0));

        // Called something, and the wrong thing: one of five.
        let wrong = ToolResult {
            called: true,
            ..ToolResult::default()
        };
        let score = Outcome::Tools(wrong).score().expect("five");
        assert!((score - 0.2).abs() < 1e-9, "{score}");

        // A trial with nothing to compare is out of five, not out of six with a zero — otherwise
        // a question without a checkable answer scores worse for having no answer to check.
        let unchecked = ToolResult {
            called: true,
            right_tool: true,
            right_arguments: true,
            valid_shape: true,
            ran: true,
            right_answer: None,
            note: None,
        };
        assert_eq!(Outcome::Tools(unchecked).score(), Some(1.0));
    }

    #[test]
    fn the_letters_follow_the_order_and_the_mapping_survives() {
        // A blind reading is worth nothing if the order gives it away, and worth nothing either
        // if nobody can find out afterwards which was which.
        let blind = Blind::of(&[
            "qwen3.6-35b".to_owned(),
            "gemma4-12b".to_owned(),
            "ministral-3-14b".to_owned(),
        ]);
        assert_eq!(blind.who.get("A").map(String::as_str), Some("qwen3.6-35b"));
        assert_eq!(
            blind.who.get("C").map(String::as_str),
            Some("ministral-3-14b")
        );
        assert_eq!(blind.letter_of("gemma4-12b"), Some("B"));
        assert_eq!(blind.letter_of("something else"), None);
    }

    #[test]
    fn more_models_than_letters_still_get_names() {
        let many: Vec<String> = (0..30).map(|n| format!("m{n}")).collect();
        let blind = Blind::of(&many);
        assert_eq!(blind.who.len(), 30);
        assert_eq!(blind.letter_of("m25"), Some("Z"));
        assert_eq!(blind.letter_of("m26"), Some("AA"));
    }
}
