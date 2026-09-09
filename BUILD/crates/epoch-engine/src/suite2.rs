//! Suite v2 — Competitive.
//!
//! ## Why a second suite rather than harder questions in the first
//!
//! v1 saturates. Measured 2026-08-31: `gemma4:12b` and `Qwen3.6-35B-A3B` both answered everything
//! they answered correctly, so the only things separating a 12B from a 35B were speed and how many
//! trials they managed to finish. That is a working sanity check and it is not a ranking.
//!
//! **v1 is untouched.** The three cards taken against it stay reproducible, and a v1 result and a
//! v2 result are two facts about a model rather than a contradiction — which is the whole reason
//! [`crate::suite::VERSION`] is a difficulty tier and not a revision number.
//!
//! ## What it is for
//!
//! Separating models in the range the owner actually runs: 9B · 12B · 14B · 27B · 35B MoE, and
//! whatever local frontier model arrives next. A suite where every strong model scores 100% has
//! told you nothing about any of them.
//!
//! The shape aimed for, and it is a *shape* rather than a target — designing questions to produce
//! a number is how a benchmark starts measuring itself:
//!
//! | | |
//! |---|---|
//! | a small model | 20–40% |
//! | a competent mid-size one | 50–70% |
//! | a very strong local model | 70–85% |
//! | an extraordinary one | 85–95% |
//!
//! **If every reference model scores 100%, this suite is still too easy** and the honest response
//! is v3 rather than a comfortable table.
//!
//! ## Our own tasks, and why that matters twice
//!
//! Nothing here is copied from SWE-bench, LiveCodeBench, GPQA, BFCL, IFEval or TerminalBench. The
//! *kinds* of problem are borrowed openly — a bug to fix, an instruction stack to satisfy, a tool
//! chain to walk — and every task is written here.
//!
//! That buys two things. **Contamination**: a question in a public dataset is a question a model
//! may have been trained on, and a benchmark cannot tell recall from reasoning. And
//! **distribution**: Epoch can ship this suite without depending on anybody's licence or server.
//!
//! ## Everything v1 established, kept
//!
//! No invented scores. Verifiable answers where a machine can verify them, and the model's whole
//! answer kept where it cannot ([`crate::trials::Blind`]). The Engine scores once. Frozen at
//! publication: the questions, their order, the budgets, the validators, the tests and the context
//! are all pinned by a test, and changing any of them requires a new version.

use crate::ran::Language;
use crate::trials::{Check, Expected, Tool, Trial, Turn};

/// Which set of questions a v2 result was measured against.
pub const VERSION: u32 = 2;

/// The same window as v1, on purpose.
///
/// A model's context is not what this suite is asking about, and changing it would make a v1 and a
/// v2 card differ in two ways at once.
pub const STANDARD_CONTEXT: u32 = 32_768;

/// How many tokens a v2 answer may take.
///
/// **Larger than v1's 4,096, because the questions are larger.** v1 arrived at 4,096 by measuring
/// what a thinking model spends before it begins an answer; these questions carry distractors and
/// several steps, and the same model will spend more on them. Eight thousand is that with room,
/// and it is still a cap rather than a target.
pub const ANSWER_TOKENS: u32 = 8_192;

/// How much of that a model may spend before it starts answering.
///
/*
    **Reported, not enforced.** No runtime this suite talks to accepts a separate reasoning budget
    — measured, `llama-server` takes `max_tokens` and nothing else — so this cannot be sent. What
    it is for is reading the result: an answer that consumed more than this before emitting
    anything is recorded as having spent its budget thinking, which is `completion: fail` and
    `correctness: not applicable` rather than *wrong*.

    v1 learned that the expensive way: at 700 tokens a capable model scored zero on trials it
    answers correctly at 4,096, and the first version called that incorrect.
*/
pub const REASONING_BUDGET: u32 = 6_000;

/// The language coding trials are written in.
pub const CODING_LANGUAGE: Language = Language::Python;

/// Every question, in order.
pub fn all() -> Vec<Trial> {
    let mut every = reasoning();
    every.extend(coding());
    every.extend(following());
    every.extend(tools());
    every.extend(general());
    every
}

// ---------------------------------------------------------------------------------------------
// Reasoning
// ---------------------------------------------------------------------------------------------

fn ask(id: &str, prompt: &str, answers: &[&str]) -> Trial {
    Trial::Reasoning {
        id: id.to_owned(),
        prompt: prompt.to_owned(),
        answers: answers.iter().map(|it| (*it).to_owned()).collect(),
    }
}

/// Multi-step logic, arithmetic with distractors, and problems whose intuitive answer is wrong.
///
/// **Every one has an answer a machine can check**, which is what keeps this out of the territory
/// where a benchmark starts grading prose. Where a problem needed a number to be checkable it was
/// written to need one — not softened until it had one.
pub fn reasoning() -> Vec<Trial> {
    vec![
        // Distractors: three of the five numbers are irrelevant, and the intuitive subtraction is
        // wrong because the returns happened before the second delivery.
        ask(
            "r2-stock",
            "A shop opens Monday with 84 crates. On Tuesday it sells 27 and 6 are returned. On \
             Wednesday a delivery of 40 arrives, and the shop sells 31. On Thursday 4 of \
             Wednesday's sales are returned, and the shop sells 18. The shop has 12 staff and \
             3 vans. How many crates does it have at the end of Thursday?",
            // 84 - 27 + 6 = 63; +40 - 31 = 72; +4 - 18 = 58
            &["58"],
        ),
        // The intuitive answer is 5 minutes. It is not.
        ask(
            "r2-machines",
            "Six printers take 6 minutes to print 6 posters. Each printer prints at the same \
             steady rate. How many minutes would 30 printers take to print 30 posters?",
            &["6", "six", "seis"],
        ),
        // Ordering under negative constraints, with one constraint that is only usable last.
        ask(
            "r2-order",
            "Five files finished processing in some order: alpha, beta, gamma, delta, epsilon. \
             Gamma finished after beta. Delta finished before alpha but after gamma. Epsilon was \
             not last and not first. Beta was first. Which file finished last? Answer with one \
             word.",
            // beta, gamma, {delta, epsilon}, ... delta before alpha, epsilon not last -> alpha
            &["alpha"],
        ),
        // Planning with a real constraint interaction, and the greedy answer is wrong.
        ask(
            "r2-plan",
            "You must run four jobs on one machine. Build takes 30 minutes and must finish before \
             Test. Test takes 20. Docs takes 15 and depends on nothing. Deploy takes 10 and must \
             start only after both Test and Docs have finished. Only one job runs at a time. What \
             is the smallest total number of minutes to finish all four?",
            // Nothing runs in parallel, so it is the sum: 30 + 20 + 15 + 10.
            &["75"],
        ),
        // Deduction where the stated rule is a conditional and its converse is not implied.
        ask(
            "r2-rule",
            "Rule: every deployment that changes the schema requires an approval. A deployment \
             happened and it had an approval. Can you conclude that it changed the schema? Answer \
             yes or no.",
            &["no"],
        ),
        // Base rates: the intuitive answer ignores how rare the condition is.
        ask(
            "r2-rate",
            "One request in 1000 is fraudulent. A detector flags 100% of fraudulent requests and \
             also flags 5% of legitimate ones. A request has just been flagged. Out of every \
             10,000 requests, how many flagged ones are actually fraudulent? Answer with a whole \
             number.",
            // 10 fraudulent (all flagged) ; 9990 legitimate * 5% = 499.5 -> the count asked for
            // is the fraudulent ones among the flagged, which is 10.
            &["10"],
        ),
        // Arithmetic where units change halfway and the distractor is a plausible wrong unit.
        ask(
            "r2-units",
            "A pump moves 250 millilitres per second for 4 minutes, then 0.4 litres per second \
             for 90 seconds. How many litres has it moved in total?",
            // 0.25*240 = 60 ; 0.4*90 = 36 ; 96
            &["96"],
        ),
        // A sequence whose obvious pattern breaks, so the confident answer is wrong.
        ask(
            "r2-sequence",
            "A log records these values in order: 2, 4, 8, 16, 31, 57. Each value after the first \
             two is the sum of all previous values plus 1, except one value which does not follow \
             that rule. Which single value breaks the rule? Answer with the number.",
            // 2,4 -> 2+4+1=7 (not 8). Sums: after 2,4: 7. The one that breaks it is 8.
            &["8"],
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// Coding
// ---------------------------------------------------------------------------------------------

fn code(id: &str, prompt: &str, tests: &str, total: u32) -> Trial {
    Trial::Coding {
        id: id.to_owned(),
        prompt: prompt.to_owned(),
        tests: tests.to_owned(),
        total,
    }
}

/// Written, fixed, transformed and reasoned about — and every one of them **run**.
///
/// Four kinds rather than four sizes: implementing, fixing somebody else's bug, transforming code
/// that already works, and getting an edge case right that a plausible implementation misses.
/// Nothing here reaches the network.
pub fn coding() -> Vec<Trial> {
    vec![
        // Implementation with an edge case the obvious version gets wrong (overlapping matches).
        code(
            "c2-runs",
            "Write a Python function `longest_run(text)` that returns the length of the longest \
             run of one repeated character in `text`. An empty string has a longest run of 0.",
            "assert longest_run('') == 0\n\
             assert longest_run('a') == 1\n\
             assert longest_run('aabbbcc') == 3\n\
             assert longest_run('abcabc') == 1\n\
             assert longest_run('zzzzz') == 5\n\
             print('ok')",
            5,
        ),
        // Bug fixing: the code is given, it is subtly wrong, and the fix is one line.
        code(
            "c2-fix",
            "This Python function is meant to return the second largest **distinct** value in a \
             list, or None when there is no second distinct value. It is wrong. Return a \
             corrected version of the whole function, named `second_largest`.\n\n\
             ```python\n\
             def second_largest(values):\n\
             \x20   if len(values) < 2:\n\
             \x20       return None\n\
             \x20   values = sorted(values)\n\
             \x20   return values[-2]\n\
             ```",
            "assert second_largest([1, 2, 3]) == 2\n\
             assert second_largest([5, 5, 5]) is None\n\
             assert second_largest([7, 7, 4]) == 4\n\
             assert second_largest([2]) is None\n\
             assert second_largest([-1, -1, -3]) == -3\n\
             print('ok')",
            5,
        ),
        // Transformation: same behaviour, different shape, and the tests hold the behaviour.
        code(
            "c2-transform",
            "Rewrite this recursive Python function as an iterative one with the same name and \
             the same behaviour, without recursion.\n\n\
             ```python\n\
             def flatten(items):\n\
             \x20   out = []\n\
             \x20   for it in items:\n\
             \x20       if isinstance(it, list):\n\
             \x20           out.extend(flatten(it))\n\
             \x20       else:\n\
             \x20           out.append(it)\n\
             \x20   return out\n\
             ```",
            "assert flatten([]) == []\n\
             assert flatten([1, [2, [3, [4]]]]) == [1, 2, 3, 4]\n\
             assert flatten([[[]]]) == []\n\
             assert flatten([1, 2, 3]) == [1, 2, 3]\n\
             import inspect\n\
             src = inspect.getsource(flatten)\n\
             assert src.count('flatten(') <= 1, 'still recursive'\n\
             print('ok')",
            5,
        ),
        // Reasoning about existing code: the answer is a function that must agree with what the
        // given code actually does, including its quirk.
        code(
            "c2-quirk",
            "Here is a Python function. Write a function `predict(n)` that returns exactly what \
             `mystery(n)` returns for any integer n from -5 to 20, without calling `mystery`.\n\n\
             ```python\n\
             def mystery(n):\n\
             \x20   total = 0\n\
             \x20   while n > 1:\n\
             \x20       total += n % 3\n\
             \x20       n //= 2\n\
             \x20   return total\n\
             ```",
            "def mystery(n):\n\
             \x20   total = 0\n\
             \x20   while n > 1:\n\
             \x20       total += n % 3\n\
             \x20       n //= 2\n\
             \x20   return total\n\
             for n in range(-5, 21):\n\
             \x20   assert predict(n) == mystery(n), n\n\
             print('ok')",
            1,
        ),
        // A larger one: several rules interacting, and the edge cases are where models fall down.
        code(
            "c2-parse",
            "Write a Python function `parse_range(text)` that turns a compact range string into a \
             sorted list of unique integers. `'1-3,7,9-10'` becomes `[1, 2, 3, 7, 9, 10]`. \
             Whitespace is ignored. A descending range like `'5-3'` yields `[3, 4, 5]`. An empty \
             string yields `[]`. Duplicates appear once.",
            "assert parse_range('') == []\n\
             assert parse_range('1-3,7,9-10') == [1, 2, 3, 7, 9, 10]\n\
             assert parse_range(' 4 , 2 ') == [2, 4]\n\
             assert parse_range('5-3') == [3, 4, 5]\n\
             assert parse_range('1-3,2-4') == [1, 2, 3, 4]\n\
             print('ok')",
            5,
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// Instruction following
// ---------------------------------------------------------------------------------------------

fn follow(id: &str, prompt: &str, must: Vec<Check>) -> Trial {
    Trial::Following {
        id: id.to_owned(),
        prompt: prompt.to_owned(),
        must,
    }
}

/// Several constraints in one prompt, including negative ones.
///
/// **v1 asked for one thing at a time and every strong model did it.** What separates models here
/// is holding four or five constraints at once — a schema, a length, an order, a ban — and
/// noticing that satisfying one does not license breaking another.
pub fn following() -> Vec<Trial> {
    vec![
        follow(
            "f2-schema",
            "Reply with a single JSON object and nothing else. It must have exactly the keys \
             `name`, `year` and `tags`. `name` is the string \"Aurora\". `year` is the number 1994 \
             — a number, not a string. `tags` is an array of exactly three lowercase strings. Do \
             not include a `description` key. Do not wrap the JSON in a code fence.",
            vec![
                Check::Json,
                Check::Keys(vec!["name".into(), "year".into(), "tags".into()]),
                Check::KeyIs("name".into(), serde_json::json!("Aurora")),
                Check::KeyIs("year".into(), serde_json::json!(1994)),
                Check::Lacks("description".into()),
                Check::Forbids("```".into()),
            ],
        ),
        follow(
            "f2-transform",
            "Here are three cities: Lisbon, Oslo, Cairo. Reply with a JSON array of those city \
             names sorted by the number of letters, shortest first, ties broken alphabetically. \
             Output only the array.",
            /*
                **Every instruction the prompt states, checked.** This trial originally carried
                two checks while its prompt said *output only the array* — an instruction nothing
                verified, so a model that wrapped the answer in a code fence and a paragraph of
                explanation scored the same as one that did what it was told.

                A stated instruction with no validator is not a constraint, it is decoration, and
                the frozen test that counts constraints is what caught it.
            */
            vec![
                Check::Json,
                Check::StartsWith("[".into()),
                Check::Forbids("```".into()),
                Check::ItemsAre(vec![
                    serde_json::json!("Oslo"),
                    serde_json::json!("Cairo"),
                    serde_json::json!("Lisbon"),
                ]),
            ],
        ),
        follow(
            "f2-lines",
            "Write exactly four lines. Every line must begin with `> `. Each line names one colour \
             and nothing else. The word `blue` must appear before the word `red`. Do not use the \
             words `green` or `yellow`. Do not add any other text.",
            vec![
                Check::Lines(4),
                Check::EveryLineStartsWith("> ".into()),
                Check::Before("blue".into(), "red".into()),
                Check::NoneOf(vec!["green".into(), "yellow".into()]),
                // *Do not add any other text* — `Lines(4)` counts them and this stops a fenced
                // block being one of the four.
                Check::Forbids("```".into()),
            ],
        ),
        follow(
            "f2-length",
            "In between 25 and 40 words, explain why a bridge has expansion joints. Do not use \
             the words `heat`, `hot` or `temperature`. Begin with the word `Because`.",
            vec![
                Check::AtLeastWords(25),
                Check::AtMostWords(40),
                Check::StartsWith("Because".into()),
                Check::NoneOf(vec!["heat".into(), "hot".into(), "temperature".into()]),
            ],
        ),
        follow(
            "f2-negative",
            "Reply with a JSON object having exactly the keys `total` and `items`. `total` must be \
             the number 3. `items` must be an array of exactly three strings, none of which is the \
             word `three`. Do not include a `count` key and do not include any explanation.",
            vec![
                Check::Json,
                Check::Keys(vec!["total".into(), "items".into()]),
                Check::KeyIs("total".into(), serde_json::json!(3)),
                Check::Lacks("count".into()),
                Check::Forbids("three".into()),
                // *Do not include any explanation*: the answer is the object and nothing else.
                Check::StartsWith("{".into()),
            ],
        ),
    ]
}

// ---------------------------------------------------------------------------------------------
// Tool calling
// ---------------------------------------------------------------------------------------------

fn tool(name: &str, description: &str, arguments: serde_json::Value) -> Tool {
    Tool {
        name: name.to_owned(),
        description: description.to_owned(),
        arguments,
    }
}

fn args(pairs: &[(&str, serde_json::Value)]) -> serde_json::Map<String, serde_json::Value> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect()
}

/// Similar tools, chained calls, errors to recover from, and one case where nothing should be
/// called at all.
///
/// Every dimension the owner asked for stays separate in the result: which tool, whether the shape
/// was valid, whether the arguments were right, whether it ran, and whether the task finished.
pub fn tools() -> Vec<Trial> {
    let find_user = tool(
        "find_user",
        "Look a user up by email address and return their record, including their numeric id.",
        serde_json::json!({
            "type": "object",
            "properties": { "email": { "type": "string" } },
            "required": ["email"],
        }),
    );
    let get_orders = tool(
        "get_orders",
        "List the orders belonging to a user, by their numeric id.",
        serde_json::json!({
            "type": "object",
            "properties": { "user_id": { "type": "integer" } },
            "required": ["user_id"],
        }),
    );
    // Deliberately similar to `get_orders`: one takes an id, the other a name, and only the
    // description separates them.
    let search_orders = tool(
        "search_orders",
        "Search orders by free text across their description. Does not accept a user id.",
        serde_json::json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"],
        }),
    );
    let convert = tool(
        "convert_units",
        "Convert a value between two units of length.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "value": { "type": "number" },
                "from": { "type": "string" },
                "to": { "type": "string" },
            },
            "required": ["value", "from", "to"],
        }),
    );

    vec![
        // Two calls, and the second's argument only exists in the first's answer.
        Trial::Agentic {
            id: "t2-chain".to_owned(),
            prompt: "How many orders does the user with the email ana@example.com have? Use the \
                     tools, then answer with just the number."
                .to_owned(),
            offered: vec![find_user.clone(), get_orders.clone(), search_orders.clone()],
            steps: vec![
                Turn {
                    tool: "find_user".to_owned(),
                    arguments: args(&[("email", serde_json::json!("ana@example.com"))]),
                    responds: r#"{"id": 4172, "name": "Ana", "email": "ana@example.com"}"#
                        .to_owned(),
                },
                Turn {
                    tool: "get_orders".to_owned(),
                    // The id is nowhere but in the previous answer.
                    arguments: args(&[("user_id", serde_json::json!(4172))]),
                    responds: r#"{"orders": [{"id": 1}, {"id": 2}, {"id": 3}]}"#.to_owned(),
                },
            ],
            finally: vec![Check::Contains("3".into())],
        },
        // Nothing should be called. A model that reaches for a tool here has failed.
        Trial::Agentic {
            id: "t2-restraint".to_owned(),
            prompt: "What is 17 plus 25? Answer with just the number. Tools are available but you \
                     do not have to use them."
                .to_owned(),
            offered: vec![convert.clone(), find_user.clone(), get_orders.clone()],
            steps: Vec::new(),
            finally: vec![Check::Contains("42".into())],
        },
        // The first call fails. Carrying on as though it succeeded is the failure being measured.
        Trial::Agentic {
            id: "t2-recover".to_owned(),
            prompt: "Find the user with the email ghost@example.com and tell me how many orders \
                     they have. If the user cannot be found, say exactly: NOT FOUND"
                .to_owned(),
            offered: vec![find_user.clone(), get_orders.clone()],
            steps: vec![Turn {
                tool: "find_user".to_owned(),
                arguments: args(&[("email", serde_json::json!("ghost@example.com"))]),
                responds: r#"{"error": "no user with that email"}"#.to_owned(),
            }],
            finally: vec![Check::Contains("NOT FOUND".into())],
        },
        // Choosing between two tools that look alike: only one takes an id.
        Trial::Tools {
            id: "t2-choose".to_owned(),
            prompt: "List the orders for user id 88.".to_owned(),
            offered: vec![search_orders.clone(), get_orders.clone(), find_user.clone()],
            expect: Expected {
                tool: "get_orders".to_owned(),
                arguments: args(&[("user_id", serde_json::json!(88))]),
                answer: None,
            },
        },
        // Ambiguous but resolvable: the unit is spelled out in prose and has to be normalised.
        Trial::Tools {
            id: "t2-ambiguous".to_owned(),
            prompt: "I measured twelve and a half kilometres. How many miles is that?".to_owned(),
            offered: vec![convert, search_orders, get_orders],
            expect: Expected {
                tool: "convert_units".to_owned(),
                arguments: args(&[("value", serde_json::json!(12.5))]),
                answer: None,
            },
        },
    ]
}

// ---------------------------------------------------------------------------------------------
// General
// ---------------------------------------------------------------------------------------------

/// Comprehension and synthesis, judged by a person rather than by a validator.
///
/*
    **Deliberately not trivia, and deliberately not scored.** These have no ground truth a machine
    can check, so nothing computes a percentage from them — the answers are kept whole and read
    blind (`trials::Blind`), which is the arrangement v1 established and the only honest one.

    They are `Following` trials with a single structural constraint, so the harness has something
    to record, and the constraint is never what the trial is about.
*/
pub fn general() -> Vec<Trial> {
    vec![
        follow(
            "g2-relevant",
            "A team reports: the deploy failed at 14:02; the database was migrated at 13:40; a \
             new logging library was added on Monday; the on-call engineer was in a meeting from \
             13:30 to 14:30; CPU on the web tier was at 12%. In at most 60 words, say which one \
             fact is most likely to explain the failure and which one is a distraction, and why.",
            vec![Check::AtMostWords(60)],
        ),
        follow(
            "g2-synthesis",
            "Two colleagues disagree. One says caching should be added because the page is slow. \
             The other says the query should be fixed because the page is slow. In at most 70 \
             words, describe what you would measure to decide between them, and what result would \
             favour each.",
            vec![Check::AtMostWords(70)],
        ),
        follow(
            "g2-situation",
            "A user reports that a file they uploaded yesterday is missing. The upload log shows \
             it succeeded. The file is not in the storage bucket. Backups run nightly at 02:00. In \
             at most 70 words, give the order in which you would check things and say what you \
             would tell the user right now.",
            vec![Check::AtMostWords(70)],
        ),
    ]
}

#[cfg(test)]
mod frozen {
    use super::*;

    /// **v2 is frozen, and this is what freezing means in code.**
    ///
    /// The questions, their order, the budgets and the context are pinned here, so changing any of
    /// them fails until the version moves with them. v1 has the same test for the same reason, and
    /// it exists because v1's budget was once changed without its version — two different
    /// measurements wearing one stamp.
    #[test]
    fn v2_is_exactly_what_it_will_be_measured_against() {
        assert_eq!(VERSION, 2);
        assert_eq!(STANDARD_CONTEXT, 32_768);
        assert_eq!(ANSWER_TOKENS, 8_192);
        assert_eq!(REASONING_BUDGET, 6_000);

        let every = all();
        let ids: Vec<&str> = every.iter().map(|it| it.id()).collect();
        assert_eq!(
            ids,
            vec![
                "r2-stock",
                "r2-machines",
                "r2-order",
                "r2-plan",
                "r2-rule",
                "r2-rate",
                "r2-units",
                "r2-sequence",
                "c2-runs",
                "c2-fix",
                "c2-transform",
                "c2-quirk",
                "c2-parse",
                "f2-schema",
                "f2-transform",
                "f2-lines",
                "f2-length",
                "f2-negative",
                "t2-chain",
                "t2-restraint",
                "t2-recover",
                "t2-choose",
                "t2-ambiguous",
                "g2-relevant",
                "g2-synthesis",
                "g2-situation",
            ],
            "v2 is frozen — to change a question, add a version rather than editing this one",
        );
    }

    #[test]
    fn v1_is_untouched_by_any_of_this() {
        // The three cards taken against v1 stay reproducible. A v1 result and a v2 result are two
        // facts about a model rather than a contradiction.
        assert_eq!(crate::suite::VERSION, 1);
        assert_eq!(crate::suite::all().len(), 14);
        let one = crate::suite::all();
        let two = all();
        let v1: std::collections::BTreeSet<&str> = one.iter().map(|it| it.id()).collect();
        let v2: std::collections::BTreeSet<&str> = two.iter().map(|it| it.id()).collect();
        assert!(v1.is_disjoint(&v2), "no question appears in both");
    }

    #[test]
    fn every_area_is_represented_and_none_of_them_is_one_question() {
        use crate::card::Kind;
        let every = all();
        for kind in [Kind::Reasoning, Kind::Coding, Kind::Following, Kind::Tools] {
            let n = every.iter().filter(|it| Kind::of_trial(it) == kind).count();
            assert!(
                n >= 5,
                "{kind:?} has only {n} — one bad question would be a fifth of it"
            );
        }
    }

    #[test]
    fn the_tool_trials_ask_the_things_one_call_cannot() {
        /*
            v1's tool trials were one call of one tool. What separates models is choosing between
            tools that look alike, carrying a result forward, recovering from an error, and
            **not calling anything when nothing is needed**.
        */
        let every = tools();
        let chained = every
            .iter()
            .any(|it| matches!(it, Trial::Agentic { steps, .. } if steps.len() > 1));
        let restrained = every
            .iter()
            .any(|it| matches!(it, Trial::Agentic { steps, .. } if steps.is_empty()));
        let recovers = every.iter().any(|it| {
            matches!(it, Trial::Agentic { steps, .. }
                if steps.iter().any(|s| s.responds.contains("error")))
        });
        assert!(chained, "one call cannot measure carrying a result forward");
        assert!(restrained, "nor whether a model knows when to stop");
        assert!(recovers, "nor what it does when a tool fails");
    }

    #[test]
    fn the_instruction_trials_stack_constraints_rather_than_asking_one_thing() {
        /*
            v1 asked one thing at a time and every strong model did it.

            **And it caught a real gap the first time it ran**: `f2-transform` had two checks while
            its prompt said *output only the array*. A stated instruction with no validator is not
            a constraint, it is decoration — a model that wrapped the answer in a code fence and a
            paragraph scored the same as one that did what it was told.
        */
        for trial in following() {
            let Trial::Following { id, must, .. } = &trial else {
                continue;
            };
            assert!(
                must.len() >= 4,
                "{id} carries only {} constraints",
                must.len(),
            );
        }
    }

    #[test]
    fn every_coding_trial_is_judged_by_running_it() {
        // Not by reading it. Each one appends real assertions to whatever the model wrote.
        for trial in coding() {
            let Trial::Coding {
                id, tests, total, ..
            } = &trial
            else {
                continue;
            };
            assert!(tests.contains("assert"), "{id} has nothing to run");
            assert!(*total >= 1, "{id} counts no assertions");
        }
    }

    #[test]
    fn the_general_trials_are_kept_rather_than_scored() {
        /*
            No ground truth a machine can check, so nothing computes a percentage from them. The
            single constraint each one carries is so the harness has something to record; it is
            never what the trial is about.
        */
        for trial in general() {
            let Trial::Following { id, must, .. } = &trial else {
                panic!("a general trial should be recorded, not validated");
            };
            assert_eq!(must.len(), 1, "{id} is being graded on prose");
        }
    }
}
