//! Suite v2's own validators, checked against answers written by hand.
//!
//! ## Why this exists and why it runs without a GPU
//!
//! A benchmark's validators are the part nobody tests, and they are the part that decides every
//! number it will ever produce. v1 shipped three defects in exactly this layer: a constraint that
//! passed vacuously on an empty answer, a check that read `{"Ok": null}` as a failure, and a
//! truncated answer scored as wrong.
//!
//! So every v2 trial is given a **known-good answer and a known-bad one**, and the validator has
//! to agree. No model is involved, nothing is loaded, and the GPU is untouched — which matters
//! today, because this machine is in `RECOVERY_REQUIRED` and no performance measurement may be
//! taken.
//!
//! What this cannot check is whether the questions are *hard enough*. That needs models, and it
//! waits for a clean machine.

use epoch_engine::suite2;
use epoch_engine::trials::{Check, Trial};

/// Every reasoning trial, answered correctly and then wrongly.
///
/// The right answers are worked out in the comments beside each question in `suite2`; repeating
/// them here is deliberate — if the two disagree, one of them is wrong and the test says so.
#[test]
fn every_reasoning_answer_is_recognised_and_a_wrong_one_is_not() {
    let right = [
        ("r2-stock", "58"),
        ("r2-machines", "6 minutes"),
        ("r2-order", "alpha"),
        ("r2-plan", "75"),
        ("r2-rule", "No."),
        ("r2-rate", "10"),
        ("r2-units", "96 litres"),
        ("r2-sequence", "8"),
    ];
    let wrong = [
        ("r2-stock", "62"),
        ("r2-machines", "30 minutes"),
        ("r2-order", "epsilon"),
        ("r2-plan", "60"),
        ("r2-rule", "Yes."),
        ("r2-rate", "500"),
        ("r2-units", "150 litres"),
        ("r2-sequence", "57"),
    ];

    for trial in suite2::reasoning() {
        let id = trial.id().to_owned();
        let good = right.iter().find(|(name, _)| *name == id).expect(&id).1;
        let bad = wrong.iter().find(|(name, _)| *name == id).expect(&id).1;

        let judged = epoch_engine::trials::judge(&trial, good);
        assert_eq!(
            judged.score(),
            Some(1.0),
            "{id}: the right answer {good:?} was not recognised",
        );
        let judged = epoch_engine::trials::judge(&trial, bad);
        assert_eq!(judged.score(), Some(0.0), "{id}: {bad:?} was accepted");
    }
}

/// The intuitive-but-wrong answer is specifically rejected.
///
/// **This is what the trial is for.** A question whose distractor is accepted measures nothing —
/// and `r2-machines` in particular exists because a model that is pattern matching rather than
/// reasoning divides.
#[test]
fn the_distractors_are_rejected() {
    let traps = [
        ("r2-machines", "1 minute"),
        ("r2-machines", "30"),
        // Affirming the consequent.
        ("r2-rule", "Yes, it must have changed the schema."),
        // The base-rate trap: quoting the false-positive count as the answer.
        ("r2-rate", "499"),
        // Ignoring that the returns came back before the last sale.
        ("r2-stock", "54"),
    ];
    for (id, said) in traps {
        let trial = suite2::reasoning()
            .into_iter()
            .find(|it| it.id() == id)
            .expect(id);
        assert_eq!(
            epoch_engine::trials::judge(&trial, said).score(),
            Some(0.0),
            "{id} accepted the trap answer {said:?}",
        );
    }
}

/// Every instruction-following trial, satisfied and then broken one constraint at a time.
#[test]
fn every_instruction_trial_passes_a_good_answer_and_fails_a_bad_one() {
    let good: &[(&str, &str)] = &[
        (
            "f2-schema",
            r#"{"name": "Aurora", "year": 1994, "tags": ["one", "two", "four"]}"#,
        ),
        ("f2-transform", r#"["Oslo", "Cairo", "Lisbon"]"#),
        ("f2-lines", "> blue\n> red\n> teal\n> violet"),
        (
            "f2-length",
            "Because the deck expands and contracts across the seasons, the joints leave room for \
             that movement so the structure does not buckle or crack under its own restrained \
             forces over many long years.",
        ),
        (
            "f2-negative",
            r#"{"total": 3, "items": ["alpha", "beta", "gamma"]}"#,
        ),
    ];
    let bad: &[(&str, &str)] = &[
        // `year` as a string, which is the constraint that separates a schema from a shape.
        (
            "f2-schema",
            r#"{"name": "Aurora", "year": "1994", "tags": ["one", "two", "four"]}"#,
        ),
        // Right items, wrong order.
        ("f2-transform", r#"["Cairo", "Oslo", "Lisbon"]"#),
        // Right shape, and `red` comes before `blue`.
        ("f2-lines", "> red\n> blue\n> teal\n> violet"),
        // Right length, and it uses a banned word.
        (
            "f2-length",
            "Because the deck expands when the temperature rises and shrinks when it falls, the \
             joints leave room for that movement so the structure does not buckle or crack under \
             its own restrained forces.",
        ),
        // Right keys, and it carries the forbidden one.
        (
            "f2-negative",
            r#"{"total": 3, "count": 3, "items": ["alpha", "beta", "gamma"]}"#,
        ),
    ];

    for trial in suite2::following() {
        let id = trial.id().to_owned();
        let said = good.iter().find(|(name, _)| *name == id).expect(&id).1;
        let judged = epoch_engine::trials::judge(&trial, said);
        assert_eq!(
            judged.score(),
            Some(1.0),
            "{id}: a good answer failed — {judged:?}",
        );

        let said = bad.iter().find(|(name, _)| *name == id).expect(&id).1;
        let judged = epoch_engine::trials::judge(&trial, said);
        assert!(
            judged.score().is_some_and(|it| it < 1.0),
            "{id}: a bad answer passed — {said:?}",
        );
    }
}

/// An empty answer never satisfies anything.
///
/// v1's `following` once read 100% for a model that said nothing, because `AtMostWords(20)` is
/// true of an empty string. The rule lives in `Check::against`; this is the suite-level guard that
/// it still holds for every v2 trial.
#[test]
fn nothing_satisfies_nothing_in_any_v2_trial() {
    for trial in suite2::following().into_iter().chain(suite2::general()) {
        let id = trial.id().to_owned();
        let judged = epoch_engine::trials::judge(&trial, "");
        assert_eq!(judged.score(), Some(0.0), "{id} accepted an empty answer");
        let judged = epoch_engine::trials::judge(&trial, "   \n  ");
        assert_eq!(judged.score(), Some(0.0), "{id} accepted whitespace");
    }
}

/// The coding trials' own tests, run against a correct solution and a plausible wrong one.
///
/// **Judged by running, on this machine, with no network.** Skipped where Python is absent — a
/// machine without an interpreter is an ordinary machine and the model has nothing to answer for.
#[test]
fn every_coding_trial_passes_a_real_solution_and_fails_a_plausible_wrong_one() {
    if epoch_engine::ran::can_run(suite2::CODING_LANGUAGE).is_none() {
        eprintln!("no Python here; the coding validators are not checked");
        return;
    }

    let good: &[(&str, &str)] = &[
        (
            "c2-runs",
            "def longest_run(text):\n    best = run = 0\n    last = None\n    for c in text:\n\
             \x20       run = run + 1 if c == last else 1\n        last = c\n\
             \x20       best = max(best, run)\n    return best\n",
        ),
        (
            "c2-fix",
            "def second_largest(values):\n    seen = sorted(set(values))\n\
             \x20   return seen[-2] if len(seen) >= 2 else None\n",
        ),
        (
            "c2-transform",
            "def flatten(items):\n    out = []\n    stack = list(items)[::-1]\n    while stack:\n\
             \x20       it = stack.pop()\n        if isinstance(it, list):\n\
             \x20           stack.extend(it[::-1])\n        else:\n            out.append(it)\n\
             \x20   return out\n",
        ),
        (
            "c2-quirk",
            "def predict(n):\n    total = 0\n    while n > 1:\n        total += n % 3\n\
             \x20       n //= 2\n    return total\n",
        ),
        (
            "c2-parse",
            "def parse_range(text):\n    out = set()\n    for part in text.split(','):\n\
             \x20       part = part.strip()\n        if not part:\n            continue\n\
             \x20       if '-' in part:\n            a, b = (int(x) for x in part.split('-'))\n\
             \x20           lo, hi = (a, b) if a <= b else (b, a)\n\
             \x20           out.update(range(lo, hi + 1))\n        else:\n\
             \x20           out.add(int(part))\n    return sorted(out)\n",
        ),
    ];
    // Each of these is what a model plausibly writes, and each fails on exactly the edge case the
    // trial exists for. A wrong solution that passed would mean the trial measures nothing.
    let bad: &[(&str, &str)] = &[
        // Counts distinct characters rather than the longest run.
        (
            "c2-runs",
            "def longest_run(text):\n    return len(set(text))\n",
        ),
        // The original bug: sorted without deduplicating.
        (
            "c2-fix",
            "def second_largest(values):\n    if len(values) < 2:\n        return None\n\
             \x20   return sorted(values)[-2]\n",
        ),
        // Still recursive, which the trial explicitly forbids.
        (
            "c2-transform",
            "def flatten(items):\n    out = []\n    for it in items:\n\
             \x20       if isinstance(it, list):\n            out.extend(flatten(it))\n\
             \x20       else:\n            out.append(it)\n    return out\n",
        ),
        // Unrolls three steps instead of looping, so larger numbers are wrong.
        (
            "c2-quirk",
            "def predict(n):\n    return sum(x % 3 for x in [n, n // 2, n // 4] if x > 1)\n",
        ),
        // Does not handle a descending range.
        (
            "c2-parse",
            "def parse_range(text):\n    out = set()\n\
             \x20   for part in text.replace(' ', '').split(','):\n        if not part:\n\
             \x20           continue\n        if '-' in part:\n\
             \x20           a, b = (int(x) for x in part.split('-'))\n\
             \x20           out.update(range(a, b + 1))\n        else:\n\
             \x20           out.add(int(part))\n    return sorted(out)\n",
        ),
    ];

    for trial in suite2::coding() {
        let Trial::Coding { id, tests, .. } = &trial else {
            continue;
        };
        let solution = good.iter().find(|(name, _)| name == id).expect(id).1;
        let ran = epoch_engine::ran::run(solution, tests, suite2::CODING_LANGUAGE)
            .unwrap_or_else(|| panic!("{id} did not run"));
        assert!(
            ran.passed,
            "{id}: the reference solution fails — {}",
            ran.said
        );

        let broken = bad.iter().find(|(name, _)| name == id).expect(id).1;
        let ran = epoch_engine::ran::run(broken, tests, suite2::CODING_LANGUAGE)
            .unwrap_or_else(|| panic!("{id} did not run"));
        assert!(
            !ran.passed,
            "{id}: a plausible wrong solution passed, so the tests do not cover the edge case the \
             trial exists for",
        );
    }
}

/// Every check the suite uses rejects an empty answer, and none of them panics.
///
/// A trial carrying a validator that throws would fail at measurement time, hours in.
#[test]
fn every_validator_in_the_suite_rejects_nothing() {
    for trial in suite2::all() {
        let Trial::Following { id, must, .. } = &trial else {
            continue;
        };
        for check in must {
            assert!(
                check.against("").is_err(),
                "{id}: {check:?} accepted an empty answer",
            );
        }
    }
}

/// The agentic trials describe a script the harness can actually play.
#[test]
fn every_agentic_step_names_a_tool_that_was_offered() {
    for trial in suite2::tools() {
        let Trial::Agentic {
            id, offered, steps, ..
        } = &trial
        else {
            continue;
        };
        for step in steps {
            assert!(
                offered.iter().any(|it| it.name == step.tool),
                "{id} expects {} which was never offered",
                step.tool,
            );
            // A tool answer that is not JSON would be testing the model's tolerance for our
            // formatting rather than its tool use.
            serde_json::from_str::<serde_json::Value>(&step.responds)
                .unwrap_or_else(|why| panic!("{id}: a tool answer is not JSON: {why}"));
        }
    }
}

/// Nothing in v2 asks a question v1 already asked.
#[test]
fn the_two_suites_share_no_question() {
    let one = epoch_engine::suite::all();
    let two = suite2::all();
    for a in &one {
        for b in &two {
            assert_ne!(a.id(), b.id());
            assert_ne!(
                a.prompt(),
                b.prompt(),
                "the same question under two ids is worse than a duplicate id",
            );
        }
    }
}

/// The checks added for v2, each rejecting what it is meant to.
#[test]
fn the_new_checks_reject_what_they_are_meant_to() {
    assert!(Check::AtLeastWords(5).against("one two three").is_err());
    assert!(Check::AtLeastWords(3).against("one two three").is_ok());

    assert!(Check::KeyIs("n".into(), serde_json::json!(1))
        .against(r#"{"n": "1"}"#)
        .is_err());
    assert!(Check::KeyIs("n".into(), serde_json::json!(1))
        .against(r#"{"n": 1}"#)
        .is_ok());

    assert!(Check::Lacks("x".into()).against(r#"{"x": 1}"#).is_err());
    assert!(Check::Lacks("x".into()).against(r#"{"y": 1}"#).is_ok());

    assert!(Check::Before("a".into(), "b".into())
        .against("b then a")
        .is_err());
    assert!(Check::Before("a".into(), "b".into())
        .against("a then b")
        .is_ok());

    assert!(Check::NoneOf(vec!["red".into()])
        .against("a RED thing")
        .is_err());
    assert!(Check::NoneOf(vec!["red".into()])
        .against("a blue thing")
        .is_ok());

    assert!(Check::EveryLineStartsWith("> ".into())
        .against("> one\ntwo")
        .is_err());
    assert!(Check::EveryLineStartsWith("> ".into())
        .against("> one\n> two")
        .is_ok());

    assert!(Check::ItemsAre(vec![serde_json::json!("a")])
        .against(r#"["b"]"#)
        .is_err());
    assert!(Check::ItemsAre(vec![serde_json::json!("a")])
        .against(r#"["a"]"#)
        .is_ok());
}
