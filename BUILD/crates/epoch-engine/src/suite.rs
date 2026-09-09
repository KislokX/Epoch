//! The questions themselves, fixed and versioned.
//!
//! ## Why a version number
//!
//! A benchmark whose questions change is not a benchmark — it is an opinion with numbers on it.
//! Two models compared on different questions are not compared, and a model measured today
//! against one measured in March is only comparable if the questions were the same. So the set is
//! frozen and stamped, and a result carries the stamp of the set it was measured against.
//!
//! **Changing a question means a new version.** Never an edit in place: a table holding results
//! from two versions of one question is the thing this number exists to make impossible.
//!
//! ## What is here and what is deliberately not
//!
//! Every question has an answer a machine can check. Nothing scores style, helpfulness or tone —
//! those are judgements, and they are kept as judgements: the answers themselves are the record,
//! read blind (`trials::Blind`).
//!
//! The questions are short on purpose. A benchmark run is five models times four kinds times
//! several trials times three runs, and every extra hundred tokens is minutes of somebody's
//! evening. What is measured is whether a model can do the thing, not how long it can keep going.
//!
//! ## Two phases, and the questions belong to the first
//!
//! [`STANDARD_CONTEXT`] is the same for every model. The Limit phase — how far one model can be
//! pushed — asks different questions of each, and mixing the two produces a table where one model
//! ran at 128K and another at 32K, which compares nothing.

use crate::ran::Language;
use crate::trials::{Check, Expected, Tool, Trial};

/// Which set of questions a result was measured against.
///
/// Bumped whenever a question changes, is added or is removed. A result from `1` and a result
/// from `2` are not rows of the same table.
///
/// ## The ladder, decided 2026-08-31
///
/// Versions are **difficulty tiers**, not revisions, so raising the bar never invalidates what
/// was measured under a lower one:
///
/// - **v1 — sanity and basic competence.** Can this model reason at all, write a function that
///   passes tests, follow a stated constraint, reach for a tool. Frozen. It separates a 270M from
///   a 12B decisively and, measured, **it saturates above about twelve billion parameters** —
///   `gemma4:12b` and `Qwen3.6-35B-A3B` both answer everything they answer correctly.
/// - **v2 — competitive models.** Questions hard enough to rank 9B–35B against each other. Not
///   built yet, and it is a *new set*, never an edit to this one.
/// - **v3 — stress.** Long context, multi-step, agentic.
///
/// **A card names its tier**, so a v1 result and a v2 result are two facts about a model rather
/// than a contradiction. That is the whole reason this is a number and not a date.
pub const VERSION: u32 = 1;

/// The context every model is given in the Standard phase.
///
/// The owner's number. It is large enough that the KV cache is a real cost on a consumer card —
/// which is where the interesting differences between models live — and small enough that a 12 GB
/// card can hold a mid-size model at it.
pub const STANDARD_CONTEXT: u32 = 32_768;

/// How many tokens a trial answer may take.
///
/// **Raised from 700 by measuring, after 700 scored a capable model at zero.** `gemma4:12b`
/// thinks before it answers and its thinking is not in `content`: asked what day a 45-day task
/// ending on a Tuesday finishes, it spent all 700 tokens reasoning and emitted an empty answer,
/// which the first version of this suite scored as *wrong*. Given room it answers correctly —
/// measured on this machine at 950 tokens for that trial, 1,168 for the arithmetic one and 1,316
/// for the first coding one.
///
/// Then 2,048 was measured too and was still short. Three runs each at a cap of 8,192, on this
/// machine: the day-of-week trial takes 950 tokens every time, the bracket one 1,296 to 1,316,
/// the interval one 1,600 to 1,790, and the twenty-word one 2,496 twice and a full runaway once.
/// **The first run of each differs from the two after it** — a cold cache changes the numerics,
/// so a fixed seed and a temperature of zero do not make the length repeatable, which is itself
/// why the budget has to have room over the measurement rather than sit on it.
///
/// 4,096 is those measurements with room over them. It is a **cap and not a target**, so a model
/// that answers in forty tokens still costs forty; the only run it makes longer is one that would
/// otherwise have been scored on a truncated answer.
///
/// It is deliberately still finite. Measured in the same sitting: the same model asked for twenty
/// words about a lighthouse without saying *light* thought for 12,953 characters and never
/// answered at all. **No budget fixes that one**, which is why the size of the budget is only half
/// of this fix — the other half is that running out is recorded as *unanswered* rather than as a
/// wrong answer.
pub const TRIAL_TOKENS: u32 = 4_096;

/// Every trial, in the order they are run.
///
/// **The same order for every model**, because a model that runs second is a model whose server
/// is already warm — and a set shuffled per model would put that advantage in a different place
/// each time.
pub fn all() -> Vec<Trial> {
    let mut every = reasoning();
    every.extend(coding());
    every.extend(following());
    every.extend(tools());
    every
}

/// Questions with one right answer.
///
/// Deliberately not trivia: what is being measured is whether a model can follow a chain of small
/// steps without losing one, which is what a Quest asks of it. Every answer is a number or a
/// single word, because an answer that needs interpreting needs a judge.
pub fn reasoning() -> Vec<Trial> {
    let ask = |id: &str, prompt: &str, answers: &[&str]| Trial::Reasoning {
        id: id.to_owned(),
        prompt: format!("{prompt}\n\nAnswer with the final result only."),
        answers: answers.iter().map(|it| (*it).to_owned()).collect(),
    };
    vec![
        ask(
            "r-change",
            "A shop sells pens at 3 for 7 euros and notebooks at 4 euros each. Someone buys 9 \
             pens and 3 notebooks, and pays with a 50 euro note. How much change do they get?",
            // 9 pens = 3 groups = 21, plus 12 = 33, change 17.
            &["17", "17 euros", "€17"],
        ),
        ask(
            "r-days",
            "A task started on a Tuesday and took 45 days including the day it started. On what \
             day of the week did it finish?",
            // Day 1 Tuesday, day 45 = Tuesday + 44 = Tuesday + 2 = Thursday.
            &["thursday", "jueves"],
        ),
        ask(
            "r-letters",
            "How many times does the letter r appear in the word strawberry?",
            &["3", "three", "tres"],
        ),
        ask(
            "r-ages",
            "Ana is twice as old as her brother. In six years she will be one and a half times \
             his age. How old is Ana now?",
            // b*2 + 6 = 1.5(b + 6) -> 2b + 6 = 1.5b + 9 -> 0.5b = 3 -> b = 6, Ana = 12.
            &["12", "12 years", "12 anos", "12 años"],
        ),
        ask(
            "r-order",
            "Four runners finish a race. Bea finished before Carla. Dani finished after Carla but \
             before Ana. Carla was not first. Who finished first?",
            &["bea"],
        ),
    ]
}

/// Programs, judged by running them.
///
/// Python because it is the one an ordinary machine already has, and because a model that cannot
/// write ten lines of it cannot write ten lines of anything. `total` is stated rather than counted
/// from the string: counting `assert` would be a guess about somebody's formatting.
pub fn coding() -> Vec<Trial> {
    let ask = |id: &str, prompt: &str, tests: &str, total: u32| Trial::Coding {
        id: id.to_owned(),
        prompt: format!(
            "{prompt}\n\nReply with Python only, in one code block. Define exactly the function \
             asked for and nothing else. Do not include tests or examples."
        ),
        tests: tests.to_owned(),
        total,
    };
    vec![
        ask(
            "c-brackets",
            "Write a Python function `balanced(text)` that returns True when every round, square \
             and curly bracket in `text` is closed in the right order, and False otherwise. \
             Characters that are not brackets are ignored.",
            "assert balanced('') is True\n\
             assert balanced('(a[b]{c})') is True\n\
             assert balanced('(]') is False\n\
             assert balanced('(()') is False\n\
             assert balanced('a)b(') is False\n\
             print('ok')",
            5,
        ),
        ask(
            "c-merge",
            "Write a Python function `merge(spans)` that takes a list of (start, end) tuples and \
             returns them merged where they overlap or touch, sorted by start.",
            "assert merge([]) == []\n\
             assert merge([(1, 3)]) == [(1, 3)]\n\
             assert merge([(1, 3), (2, 6), (8, 10)]) == [(1, 6), (8, 10)]\n\
             assert merge([(5, 6), (1, 2)]) == [(1, 2), (5, 6)]\n\
             assert merge([(1, 4), (4, 5)]) == [(1, 5)]\n\
             print('ok')",
            5,
        ),
        ask(
            "c-roman",
            "Write a Python function `to_roman(n)` that converts an integer between 1 and 3999 \
             into a Roman numeral string.",
            "assert to_roman(1) == 'I'\n\
             assert to_roman(4) == 'IV'\n\
             assert to_roman(9) == 'IX'\n\
             assert to_roman(1994) == 'MCMXCIV'\n\
             assert to_roman(3999) == 'MMMCMXCIX'\n\
             print('ok')",
            5,
        ),
    ]
}

/// Which language the coding trials are written in.
pub const CODING_LANGUAGE: Language = Language::Python;

/// Constraints a machine can check.
///
/// Every one of these is something a Quest actually needs: a model that cannot return only JSON
/// cannot be given a tool, and one that cannot stop at forty words cannot fill a field.
pub fn following() -> Vec<Trial> {
    vec![
        Trial::Following {
            id: "f-json-list".to_owned(),
            prompt: "List exactly five colours. Reply with a JSON array of five strings and \
                     nothing else — no explanation, no code fence."
                .to_owned(),
            must: vec![Check::Json, Check::Items(5)],
        },
        Trial::Following {
            id: "f-json-object".to_owned(),
            prompt: "Reply with a single JSON object describing a book, with exactly the keys \
                     title, author and year. No explanation, no code fence."
                .to_owned(),
            must: vec![
                Check::Json,
                Check::Keys(vec![
                    "title".to_owned(),
                    "author".to_owned(),
                    "year".to_owned(),
                ]),
            ],
        },
        Trial::Following {
            id: "f-brief".to_owned(),
            prompt: "In at most twenty words, say what a lighthouse is for. Do not use the word \
                     light."
                .to_owned(),
            must: vec![Check::AtMostWords(20), Check::Forbids("light".to_owned())],
        },
        Trial::Following {
            id: "f-shape".to_owned(),
            prompt: "Reply with exactly three lines. Each line must begin with a dash and a \
                     space. Each line names one planet. Say nothing else."
                .to_owned(),
            must: vec![
                Check::Lines(3),
                Check::StartsWith("-".to_owned()),
                Check::Forbids("```".to_owned()),
            ],
        },
    ]
}

/// Tools offered, and what calling them correctly looks like.
///
/// Two tools per trial, so choosing is a choice: a model handed one tool and asked to use it has
/// demonstrated nothing about selection, which is the first of the five things worth knowing.
pub fn tools() -> Vec<Trial> {
    let convert = Tool {
        name: "convert_units".to_owned(),
        description: "Convert a quantity from one unit to another.".to_owned(),
        arguments: serde_json::json!({
            "type": "object",
            "properties": {
                "value": { "type": "number", "description": "the amount to convert" },
                "from": { "type": "string", "description": "the unit it is in" },
                "to": { "type": "string", "description": "the unit to convert to" }
            },
            "required": ["value", "from", "to"]
        }),
    };
    let weather = Tool {
        name: "get_weather".to_owned(),
        description: "Report the current weather in a named city.".to_owned(),
        arguments: serde_json::json!({
            "type": "object",
            "properties": { "city": { "type": "string" } },
            "required": ["city"]
        }),
    };
    let remember = Tool {
        name: "save_note".to_owned(),
        description: "Write a short note down so it can be read later.".to_owned(),
        arguments: serde_json::json!({
            "type": "object",
            "properties": {
                "text": { "type": "string" },
                "tag": { "type": "string", "description": "one word to file it under" }
            },
            "required": ["text"]
        }),
    };

    vec![
        Trial::Tools {
            id: "t-convert".to_owned(),
            prompt: "How many kilometres is 12 miles? Use a tool.".to_owned(),
            offered: vec![weather.clone(), convert.clone()],
            expect: Expected {
                tool: "convert_units".to_owned(),
                arguments: [
                    ("value".to_owned(), serde_json::json!(12)),
                    ("from".to_owned(), serde_json::json!("miles")),
                    ("to".to_owned(), serde_json::json!("kilometres")),
                ]
                .into_iter()
                .collect(),
                // 12 miles is 19.31 km. The tool answers, and the answer is checkable.
                answer: Some("19.31".to_owned()),
            },
        },
        Trial::Tools {
            id: "t-choose".to_owned(),
            prompt: "Write down that the meeting moved to Thursday, so I can find it later. \
                     Use a tool."
                .to_owned(),
            offered: vec![convert, weather, remember],
            expect: Expected {
                tool: "save_note".to_owned(),
                // Only that it wrote something down: what it writes is prose, and demanding an
                // exact sentence would measure phrasing rather than tool use.
                arguments: serde_json::Map::new(),
                answer: None,
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_trial_has_its_own_name() {
        // Two trials sharing an id would silently overwrite each other in a result, and the table
        // would be short by one row with nothing saying so.
        let ids: Vec<String> = all().iter().map(|it| it.id().to_owned()).collect();
        let mut unique = ids.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), ids.len(), "{ids:?}");
    }

    #[test]
    fn every_question_asks_for_something() {
        for trial in all() {
            assert!(
                trial.prompt().len() > 20,
                "{} has no real question",
                trial.id()
            );
        }
    }

    #[test]
    fn a_coding_trial_states_how_many_assertions_it_makes() {
        // Counting `assert` in the string would be a guess about somebody's formatting; stating
        // it is a fact the author of the trial knows. This checks the two agree today, which is
        // what makes the stated number trustworthy.
        for trial in coding() {
            let Trial::Coding {
                id, tests, total, ..
            } = &trial
            else {
                continue;
            };
            let counted = u32::try_from(tests.matches("assert ").count()).unwrap_or(0);
            assert_eq!(*total, counted, "{id} says {total} and makes {counted}");
        }
    }

    #[test]
    fn a_tool_trial_offers_more_than_one_tool() {
        // A model handed one tool and asked to use it has demonstrated nothing about choosing,
        // which is the first of the five things a tool trial is for.
        for trial in tools() {
            let Trial::Tools {
                id,
                offered,
                expect,
                ..
            } = &trial
            else {
                continue;
            };
            assert!(offered.len() > 1, "{id} offers no choice");
            assert!(
                offered.iter().any(|it| it.name == expect.tool),
                "{id} expects a tool it never offered"
            );
        }
    }

    #[test]
    fn the_reasoning_answers_are_the_right_ones() {
        /*
            **The arithmetic, checked here rather than trusted.** A benchmark whose expected
            answer is wrong marks every correct model wrong, and it is the one defect nobody
            notices — every model failing looks like a hard question.
        */
        // 9 pens at 3-for-7 is 3 groups, 21. Three notebooks at 4 is 12. 50 - 33 = 17.
        assert_eq!(50 - (3 * 7 + 3 * 4), 17);
        // Day 1 is Tuesday, so day 45 is 44 days later; 44 % 7 == 2; Tuesday + 2 = Thursday.
        assert_eq!(44 % 7, 2);
        // s-t-r-a-w-b-e-r-r-y
        assert_eq!("strawberry".matches('r').count(), 3);
        // 2b + 6 = 1.5(b + 6) gives b = 6, so Ana is 12.
        let brother = 6.0_f64;
        assert!((2.0 * brother + 6.0 - 1.5 * (brother + 6.0)).abs() < 1e-9);
        assert_eq!((brother * 2.0) as i32, 12);
        // 12 miles in kilometres, to two places.
        assert!(((12.0_f64 * 1.609_344) - 19.312).abs() < 0.01);
    }

    #[test]
    fn the_constraints_pass_on_an_answer_that_obeys_them() {
        /*
            **A constraint nobody can satisfy fails every model equally and reads as difficulty.**
            So each one is checked against an answer written to obey it — if this cannot pass,
            the trial is broken rather than hard.
        */
        let good: Vec<(&str, &str)> = vec![
            ("f-json-list", r#"["red","green","blue","black","white"]"#),
            (
                "f-json-object",
                r#"{"title":"Dune","author":"Herbert","year":1965}"#,
            ),
            ("f-brief", "It warns ships that the coast is near."),
            ("f-shape", "- Mars\n- Venus\n- Earth"),
        ];
        for trial in following() {
            let Trial::Following { id, must, .. } = &trial else {
                continue;
            };
            let said = good
                .iter()
                .find(|(name, _)| name == id)
                .map(|(_, said)| *said)
                .unwrap_or_else(|| panic!("{id} has no example answer"));
            for check in must {
                assert!(
                    check.against(said).is_ok(),
                    "{id}: {check:?} rejects an answer that obeys it — {:?}",
                    check.against(said)
                );
            }
        }
    }

    #[test]
    fn the_constraints_fail_on_an_answer_that_does_not() {
        // And the other half: a check that passes everything measures nothing.
        let bad = [
            ("f-json-list", "red, green, blue"),
            ("f-json-object", r#"{"title":"Dune"}"#),
            (
                "f-brief",
                "A lighthouse is a tall tower that shows a light so that ships out at sea can \
                 see where the rocks are and stay away from them safely.",
            ),
            ("f-shape", "```\n- Mars\n```"),
        ];
        for trial in following() {
            let Trial::Following { id, must, .. } = &trial else {
                continue;
            };
            let said = bad
                .iter()
                .find(|(name, _)| name == id)
                .expect("an example")
                .1;
            assert!(
                must.iter().any(|check| check.against(said).is_err()),
                "{id} accepts an answer that breaks it"
            );
        }
    }
}

#[cfg(test)]
mod frozen {
    use super::*;

    /// **v1 is frozen, and this is what freezing means in code.**
    ///
    /*
        A version number that lives in a constant is a promise somebody has to remember to keep.
        This one is kept by the build: the questions, their order, and the answer budget are
        pinned here, so changing any of them fails until the version moves with them.

        It exists because it already went wrong. `TRIAL_TOKENS` was raised from 700 to 4,096 while
        `VERSION` stayed at 1 — for a good reason, and it still meant two different measurements
        wearing one stamp. The 270M card taken under the old budget is not comparable with the two
        taken after it, and nothing in the file said so.

        The budget is part of the suite because it is part of the question. *Answer this, in at
        most N tokens* is a different question for each N — measured: at 700 a capable model
        scored zero on trials it answers correctly at 4,096.
    */
    #[test]
    fn v1_is_exactly_what_the_measured_cards_were_measured_against() {
        assert_eq!(VERSION, 1);
        assert_eq!(STANDARD_CONTEXT, 32_768);
        assert_eq!(TRIAL_TOKENS, 4_096);

        let every = all();
        let ids: Vec<&str> = every.iter().map(|it| it.id()).collect();
        assert_eq!(
            ids,
            vec![
                "r-change",
                "r-days",
                "r-letters",
                "r-ages",
                "r-order",
                "c-brackets",
                "c-merge",
                "c-roman",
                "f-json-list",
                "f-json-object",
                "f-brief",
                "f-shape",
                "t-convert",
                "t-choose",
            ],
            "v1 is frozen — to change a question, add a version rather than editing this one",
        );
    }
}
