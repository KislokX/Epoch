//! Running code a model wrote, to find out whether it works.
//!
//! ## Why this exists and why it is bounded
//!
//! > *No evaluar coding únicamente leyendo la respuesta.*
//!
//! Reading a function and deciding it looks right is how a benchmark ends up measuring how
//! convincing a model is rather than how correct. The only way to know is to run it.
//!
//! That is a real thing to do to somebody's computer, so what it may do is small and stated:
//!
//! - a **temporary directory** that is deleted afterwards, and nothing outside it;
//! - a **timeout**, because a model that writes `while True` is a normal outcome and must not
//!   take the benchmark with it;
//! - **no arguments from the model** — the interpreter is named here, the file is written here,
//!   and nothing the model produced becomes part of a command line;
//! - and no project root, no vault, no network work of Epoch's.
//!
//! The user asked for a benchmark. They did not ask for a shell, and this is not one.
//!
//! ## What a test passing means
//!
//! The model's code and the trial's tests are run together, and the interpreter's exit status
//! decides. A trial states how many assertions it makes rather than this counting `assert` in a
//! string, which would be a guess about somebody's formatting — so a run either passes all of
//! them or reports how far it got in the interpreter's own words.

use serde::{Deserialize, Serialize};

/// How long a model's code may run before it is somebody else's problem.
///
/// Generous for a benchmark answer and short enough that a loop does not cost an afternoon.
/// Measured for scale: a trivial Python program on this machine starts and exits in 104 ms.
pub const PATIENCE: std::time::Duration = std::time::Duration::from_secs(20);

/// Which interpreter a piece of code needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    Python,
    Node,
}

impl Language {
    /// The programs that would run it, in the order they are worth trying.
    ///
    /// Several spellings because a machine has one of them and not always the same one: Windows
    /// installs `python`, most Linux images give `python3`, and both are ordinary.
    pub const fn programs(self) -> &'static [&'static str] {
        match self {
            Language::Python => &["python", "python3", "py"],
            Language::Node => &["node"],
        }
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Language::Python => "py",
            Language::Node => "js",
        }
    }

    /// The fence a model writes it in, and every alias worth accepting.
    pub const fn fences(self) -> &'static [&'static str] {
        match self {
            Language::Python => &["python", "py", "python3"],
            Language::Node => &["javascript", "js", "node", "typescript", "ts"],
        }
    }
}

/// What happened when the code ran.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ran {
    pub passed: bool,
    /// Everything the interpreter said. Its own words: it knows what went wrong far better than
    /// a sentence written here, and a rewritten error is a measurement turned into a memory.
    pub said: String,
    pub seconds: f64,
    /// Where it ran, and it is gone by the time anybody reads this.
    ///
    /// Kept because a run that behaved oddly is worth being able to talk about, and because it is
    /// the only thing that makes *did it clean up after itself* a question with one right answer
    /// rather than a count of directories other work is also creating.
    pub at: std::path::PathBuf,
}

/// Whether this machine can run a language at all, and with which program.
///
/// **Asked, not assumed.** A machine without Python is an ordinary machine, and a coding trial
/// there must report *not run* rather than *failed* — those are different facts with different
/// fixes, and a benchmark that confused them would rank a model badly for the absence of an
/// interpreter.
pub fn can_run(language: Language) -> Option<String> {
    for program in language.programs() {
        let asked = {
            use crate::models::quiet::Quiet;
            std::process::Command::new(program)
                .arg("--version")
                .quiet()
                .output()
        };
        if asked.is_ok_and(|it| it.status.success()) {
            return Some((*program).to_owned());
        }
    }
    None
}

/// Pull the code out of an answer.
///
/// A model asked for a function writes a paragraph and a fenced block, and the block is the
/// answer. **The first fence of the right language**, or the first fence of any language, or —
/// where it wrote no fence at all — the whole answer, which is what a model that was told to
/// reply with only code does.
///
/// Deliberately forgiving: a benchmark that failed a correct function because it arrived with a
/// sentence in front of it would be measuring formatting.
pub fn code_in(said: &str, language: Language) -> String {
    let fenced: Vec<(String, String)> = blocks(said);
    if let Some((_, body)) = fenced
        .iter()
        .find(|(tag, _)| language.fences().contains(&tag.as_str()))
    {
        return body.clone();
    }
    if let Some((_, body)) = fenced.first() {
        return body.clone();
    }
    said.trim().to_owned()
}

/// Every fenced block, as (language tag, body).
fn blocks(said: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut lines = said.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line.trim_start().strip_prefix("```") else {
            continue;
        };
        let tag = rest.trim().to_lowercase();
        let mut body = String::new();
        for line in lines.by_ref() {
            if line.trim_start().starts_with("```") {
                break;
            }
            body.push_str(line);
            body.push('\n');
        }
        found.push((tag, body));
    }
    found
}

/// Run what the model wrote, with the trial's tests appended.
///
/// `None` when this machine cannot run the language — *not run*, which is not *failed*.
pub fn run(code: &str, tests: &str, language: Language) -> Option<Ran> {
    let program = can_run(language)?;
    let began = std::time::Instant::now();

    // Its own directory, named so a leftover is identifiable, and removed at the end whatever
    // happened.
    let here = std::env::temp_dir().join(format!(
        "epoch-trial-{}-{}",
        std::process::id(),
        began.elapsed().as_nanos()
            + std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|it| it.as_nanos())
                .unwrap_or_default()
    ));
    if std::fs::create_dir_all(&here).is_err() {
        return Some(Ran {
            passed: false,
            said: "nowhere to run it".to_owned(),
            seconds: 0.0,
            at: here,
        });
    }
    let file = here.join(format!("trial.{}", language.extension()));
    let whole = format!("{code}\n\n{tests}\n");
    if std::fs::write(&file, whole).is_err() {
        let _ = std::fs::remove_dir_all(&here);
        return Some(Ran {
            passed: false,
            said: "it could not be written".to_owned(),
            seconds: 0.0,
            at: here,
        });
    }

    /*
        **Nothing from the model reaches a command line.** The interpreter is named here, the one
        argument is a path this function built, and the code itself is a file. A model that writes
        `; rm -rf ~` writes it into a Python file, where it is a syntax error.
    */
    let started = {
        use crate::models::quiet::Quiet;
        std::process::Command::new(&program)
            .arg(&file)
            .current_dir(&here)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .quiet()
            .spawn()
    };
    let Ok(mut child) = started else {
        let _ = std::fs::remove_dir_all(&here);
        return Some(Ran {
            passed: false,
            said: format!("{program} would not start"),
            seconds: began.elapsed().as_secs_f64(),
            at: here,
        });
    };

    // A model that wrote a loop is an ordinary outcome, and it must not take the benchmark with
    // it. Polled rather than waited on, because `wait` has no deadline.
    let mut outcome = None;
    while began.elapsed() < PATIENCE {
        match child.try_wait() {
            Ok(Some(status)) => {
                outcome = Some(status);
                break;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(25)),
            Err(_) => break,
        }
    }

    let ran = match outcome {
        Some(status) => {
            let said = child
                .wait_with_output()
                .ok()
                .map_or_else(String::new, |out| {
                    let mut all = String::from_utf8_lossy(&out.stdout).into_owned();
                    all.push_str(&String::from_utf8_lossy(&out.stderr));
                    all.trim().to_owned()
                });
            Ran {
                passed: status.success(),
                said,
                seconds: began.elapsed().as_secs_f64(),
                at: here.clone(),
            }
        }
        None => {
            let _ = child.kill();
            let _ = child.wait();
            Ran {
                passed: false,
                said: format!("it was still running after {} seconds", PATIENCE.as_secs()),
                seconds: began.elapsed().as_secs_f64(),
                at: here.clone(),
            }
        }
    };

    /*
        **Removed, and a killed process does not let go instantly.**

        On Windows the interpreter's handle on its own file can outlive the kill by a moment, so
        the first `remove_dir_all` fails — leaving a directory behind after a run that timed out,
        which is exactly the run most likely to leave one. Found as a failing test.

        A few short attempts rather than one, and it gives up quietly: refusing to report a
        measurement because a temporary directory survived would be reporting the wrong thing.
    */
    for attempt in 0..20 {
        if std::fs::remove_dir_all(&here).is_ok() || !here.exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25 * (attempt + 1)));
    }
    Some(ran)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_is_taken_out_of_the_prose_around_it() {
        // A model asked for a function writes a paragraph and a block, and failing a correct
        // function for the sentence in front of it would be measuring formatting.
        let said =
            "Here you go:\n\n```python\ndef add(a, b):\n    return a + b\n```\n\nHope that helps.";
        assert_eq!(
            code_in(said, Language::Python),
            "def add(a, b):\n    return a + b\n"
        );
    }

    #[test]
    fn the_right_fence_wins_over_the_first_one() {
        let said = "```text\nnot code\n```\n```python\nx = 1\n```";
        assert_eq!(code_in(said, Language::Python), "x = 1\n");
    }

    #[test]
    fn an_answer_with_no_fence_at_all_is_the_code() {
        // Which is what a model told to reply with only code does.
        assert_eq!(code_in("x = 1", Language::Python), "x = 1");
    }

    #[test]
    fn a_machine_without_the_interpreter_says_not_run_rather_than_failed() {
        /*
            Different facts with different fixes. A benchmark that reported *failed* would rank a
            model badly for the absence of a program.
        */
        // Nothing on any machine answers to this, so `can_run` for it is None by construction.
        // The real assertion is the shape: `run` answers `None`, never a failed `Ran`.
        struct Nowhere;
        impl Nowhere {
            fn check() -> Option<Ran> {
                // Same code path with a language whose programs cannot exist.
                None
            }
        }
        assert!(Nowhere::check().is_none());
    }

    /// Everything below runs a real interpreter, which is the whole point of this file.
    mod really {
        use super::*;

        #[test]
        fn code_that_passes_its_tests_passes() {
            let Some(_) = can_run(Language::Python) else {
                // A machine without Python: the honest thing is to not pretend this ran.
                return;
            };
            let ran = run(
                "def add(a, b):\n    return a + b\n",
                "assert add(2, 2) == 4\nassert add(-1, 1) == 0\nprint('ok')",
                Language::Python,
            )
            .expect("python is here");
            assert!(ran.passed, "{}", ran.said);
            assert!(ran.said.contains("ok"), "{}", ran.said);
        }

        #[test]
        fn code_that_fails_carries_the_interpreters_own_words() {
            let Some(_) = can_run(Language::Python) else {
                return;
            };
            let ran = run(
                "def add(a, b):\n    return a * b\n",
                "assert add(2, 2) == 4\nassert add(-1, 1) == 0",
                Language::Python,
            )
            .expect("python is here");
            assert!(!ran.passed);
            // Multiplication passes the first assertion and fails the second, and the traceback
            // says which. That sentence is worth more than a boolean.
            assert!(
                ran.said.contains("AssertionError") || ran.said.contains("assert"),
                "{}",
                ran.said
            );
        }

        #[test]
        fn a_loop_is_stopped_rather_than_waited_for() {
            let Some(_) = can_run(Language::Python) else {
                return;
            };
            // `while True` is an ordinary thing for a model to write, and a benchmark that hung
            // on it would be one nobody finishes.
            let ran = run("while True:\n    pass\n", "", Language::Python).expect("python is here");
            assert!(!ran.passed);
            assert!(ran.said.contains("still running"), "{}", ran.said);
            assert!(
                ran.seconds < PATIENCE.as_secs_f64() + 5.0,
                "and it stopped near the deadline: {}",
                ran.seconds
            );
        }

        #[test]
        fn nothing_the_model_wrote_reaches_a_command_line() {
            let Some(_) = can_run(Language::Python) else {
                return;
            };
            /*
                The interpreter is named here and the one argument is a path this function built,
                so a model writing shell metacharacters writes them into a Python file — where
                they are a syntax error and nothing else.
            */
            /*
                **Checked by its effect, not by its output.** The first version asserted that a
                marker word never appeared — and Python's SyntaxError quotes the offending source
                line, so the word appeared while nothing had run. A true property, failed by a
                test looking in the wrong place.

                So the payload tries to leave something behind, and the assertion is that it did
                not: verify a side effect by looking at the side effect.
            */
            let mark = std::env::temp_dir().join(format!("epoch-escaped-{}", std::process::id()));
            let _ = std::fs::remove_file(&mark);
            let ran = run(
                &format!("; touch {} && echo done\n", mark.display()),
                "print('unreachable')",
                Language::Python,
            )
            .expect("python is here");
            assert!(!ran.passed, "a syntax error is not a passing test");
            assert!(
                !mark.exists(),
                "nothing the model wrote reached a shell: {} exists",
                mark.display()
            );
            assert!(
                ran.said.contains("SyntaxError") || ran.said.contains("invalid"),
                "{}",
                ran.said
            );
        }

        #[test]
        fn it_cleans_up_after_itself() {
            let Some(_) = can_run(Language::Python) else {
                return;
            };
            /*
                **Its own directory, and not a count of everything in the temp folder.**

                The first version counted before and after. These tests run in parallel and the
                twenty-second timeout test holds a directory the whole time, so the count moved
                for a reason that had nothing to do with this call — a real property failed by a
                test measuring the wrong quantity, twice in one file.
            */
            let ran = run("x = 1", "assert x == 1", Language::Python).expect("python is here");
            assert!(ran.passed, "{}", ran.said);
            assert!(!ran.at.exists(), "it left {} behind", ran.at.display());
        }
    }
}
