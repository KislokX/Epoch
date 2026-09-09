//! What the installed build actually accepts, read from the program itself.
//!
//! ## Why this exists
//!
//! A search's candidates were assembled from what llama.cpp is known to support — `Cache::F16`
//! and `Cache::Q8_0` written at the call site, `--flash-attn on|off` written in the planner. Both
//! were true of the build on this machine on the day they were written, and nothing in either
//! sentence is a measurement.
//!
//! `CLAUDE.md` has the rule twice already, from two different subsystems:
//!
//! > **A control assembled from what is conceivable will offer combinations that do not exist.
//! > One derived from what was measured cannot.**
//!
//! > **Somebody else's flag is a measurement, not a memory.** It was true when it was written
//! > down and nothing tells you the day it stops being true.
//!
//! And this machine already has two llama.cpp builds that differ — a CUDA one in `~/.llama/bin`
//! and the Vulkan winget package — which is the whole argument in one line. A planner that
//! assumes a flag produces a candidate that fails minutes into a search, and a run whose failure
//! looks like a finding.
//!
//! ## What it reads
//!
//! `--help`, which llama.cpp writes in a shape that says more than whether a flag exists:
//!
//! ```text
//! -ctk,  --cache-type-k TYPE              KV cache data type for K
//!                                         allowed values: f32, f16, bf16, q8_0, q4_0, q4_1, iq4_nl, q5_0, q5_1
//!                                         (default: f16)
//! ```
//!
//! So a flag carries its **allowed values** and its **default**, both published by the build that
//! will consume them. Nothing here is a list Epoch maintains.
//!
//! ## What it deliberately does not do
//!
//! It does not turn every published value into a candidate. `--cache-type-k` accepts `q4_0`, and
//! a speed search that recommended a 4-bit KV cache would be trading quality it cannot measure —
//! that is the Quality Suite's question, and until it can answer, offering the trade would be a
//! recommendation nobody can check. Epoch offers what it can both run *and* judge, and this is
//! how it finds out which of those it can run.

use std::collections::BTreeMap;

/// One option the build published, with whatever it said about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Flag {
    /// Every spelling on the line: `-ctk`, `--cache-type-k`.
    pub names: Vec<String>,
    /// What it said after `allowed values:`, in the order it said them. Empty means it published
    /// none — **not** that the flag takes nothing.
    pub allowed: Vec<String>,
    /// What it said after `(default:`, if it said one.
    pub default: Option<String>,
}

impl Flag {
    /// Whether this option publishes `want` as one of its values.
    ///
    /// **A flag that published no values answers `false`.** That is the conservative direction
    /// here and it is the opposite of the usual rule, deliberately: `silence is not no` protects
    /// a *capability* from being withheld, and this decides whether to spend two minutes of GPU
    /// time on an argument the program may reject. An unpublished value is not evidence the build
    /// takes it, and the cost of assuming wrong is a failed candidate in the middle of a search.
    pub fn takes(&self, want: &str) -> bool {
        self.allowed.iter().any(|it| it == want)
    }
}

/// Every option the installed build published.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Accepts {
    /// Keyed by long name including the dashes, so `has("--flash-attn")` reads like the flag.
    by_name: BTreeMap<String, Flag>,
}

impl Accepts {
    /// Ask the program. `None` when it could not be run or said nothing.
    ///
    /// **`None` is *unasked*, and callers must read it that way.** A build Epoch could not
    /// question is not a build with no options; it is one nobody managed to ask. What a caller
    /// does about that is a product decision — [`Accepts::unasked`] exists so the answer can be
    /// written down as "assume nothing" rather than assumed by accident.
    pub fn read(program: &std::path::Path) -> Option<Self> {
        let got = std::process::Command::new(program)
            .arg("--help")
            .output()
            .ok()?;
        // llama.cpp writes its help on stdout; other builds have used stderr. Both are read
        // rather than picking one and being wrong on a build nobody has here.
        let mut text = String::from_utf8_lossy(&got.stdout).into_owned();
        text.push('\n');
        text.push_str(&String::from_utf8_lossy(&got.stderr));
        let read = Self::from_help(&text);
        (!read.by_name.is_empty()).then_some(read)
    }

    /// Nothing was asked, and nothing may be assumed.
    pub fn unasked() -> Self {
        Self::default()
    }

    /// Whether anything was published at all — so a caller can tell *empty* from *unasked*.
    pub fn anything(&self) -> bool {
        !self.by_name.is_empty()
    }

    /// Parse a help text. Pure, so the shape of a real one can be a test rather than a run.
    pub fn from_help(text: &str) -> Self {
        let mut by_name: BTreeMap<String, Flag> = BTreeMap::new();
        let mut last: Option<String> = None;

        for line in text.lines() {
            let trimmed = line.trim_start();
            let indent = line.len() - trimmed.len();

            /*
                A continuation is an indented line under an option, and it is where the two
                interesting facts live. It is recognised by indentation rather than by not
                starting with `-`, because a description can begin with a dash.
            */
            if indent > 8 && !trimmed.starts_with('-') {
                if let Some(one) = last.as_ref().and_then(|it| by_name.get_mut(it)) {
                    if let Some(values) = trimmed.strip_prefix("allowed values:") {
                        one.allowed = values
                            .split(',')
                            .map(|it| it.trim().to_owned())
                            .filter(|it| !it.is_empty())
                            .collect();
                    } else if let Some(rest) = trimmed.strip_prefix("(default:") {
                        one.default = rest
                            .trim_end_matches(')')
                            .trim()
                            .trim_matches('\'')
                            .to_owned()
                            .into();
                    }
                }
                continue;
            }

            if !trimmed.starts_with('-') {
                // A heading, a blank line, or prose. It ends whatever option came before, so a
                // stray `allowed values:` further down cannot attach itself to it.
                last = None;
                continue;
            }

            let names = names_on(trimmed);
            let Some(long) = names.iter().find(|it| it.starts_with("--")).cloned() else {
                continue;
            };
            let one = by_name.entry(long.clone()).or_default();
            for name in names {
                if !one.names.contains(&name) {
                    one.names.push(name);
                }
            }
            last = Some(long);
        }

        Self { by_name }
    }

    /// What the build said about one option, by any of its spellings.
    pub fn flag(&self, name: &str) -> Option<&Flag> {
        self.by_name.get(name).or_else(|| {
            self.by_name
                .values()
                .find(|it| it.names.iter().any(|one| one == name))
        })
    }

    /// Whether this build has the option at all.
    pub fn has(&self, name: &str) -> bool {
        self.flag(name).is_some()
    }

    /// Whether this build has the option **and** publishes `value` for it.
    pub fn takes(&self, name: &str, value: &str) -> bool {
        self.flag(name).is_some_and(|it| it.takes(value))
    }
}

/// What the llama.cpp this machine would actually run publishes.
///
/// **Here rather than at the caller**, because resolving the binary means knowing about winget
/// packages and `~/.llama/bin`, and a planner has no business knowing either. `None` is *the
/// program could not be asked* — never *this build has no options*.
pub fn of_llama_cpp() -> Option<Accepts> {
    let at = crate::runtimes::found_in(
        "llama-server",
        crate::runtimes::Runtime::LlamaCpp.winget_package(),
        None,
    )?;
    Accepts::read(&at)
}

/// Every option spelling at the head of a help line.
///
/// `-ctk,  --cache-type-k TYPE` gives `["-ctk", "--cache-type-k"]`; the `TYPE` and everything
/// after it is the argument and the description, and neither is a name.
fn names_on(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    for piece in line.split(',') {
        let word = piece.split_whitespace().next().unwrap_or_default();
        if word.starts_with('-') && word.len() > 1 {
            found.push(word.to_owned());
        } else if !found.is_empty() {
            // The names run out at the first thing that is not one; what follows is prose, and
            // prose contains commas.
            break;
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real lines, copied from `llama-server --help` on this machine, 2026-09-01.
    const HELP: &str = "\
----- common params -----

-h,    --help, --usage                  print usage and exit
-b,    --batch-size N                   logical maximum batch size (default: 2048)
-ub,   --ubatch-size N                  physical maximum batch size (default: 512)
-fa,   --flash-attn [on|off|auto]       set Flash Attention use ('on', 'off', or 'auto', default: 'auto')
                                        (env: LLAMA_ARG_FLASH_ATTN)
-ctk,  --cache-type-k TYPE              KV cache data type for K
                                        allowed values: f32, f16, bf16, q8_0, q4_0, q4_1, iq4_nl, q5_0, q5_1
                                        (default: f16)
                                        (env: LLAMA_ARG_CACHE_TYPE_K)
-ot,   --override-tensor <tensor name pattern>=<buffer type>,...
                                        override tensor buffer type
-sm,   --split-mode {none,layer,row,tensor}
                                        how to split the model across multiple GPUs
";

    #[test]
    fn a_flag_carries_its_values_and_its_default() {
        let read = Accepts::from_help(HELP);
        let cache = read.flag("--cache-type-k").expect("published");
        assert_eq!(cache.names, ["-ctk", "--cache-type-k"]);
        assert_eq!(
            cache.allowed,
            ["f32", "f16", "bf16", "q8_0", "q4_0", "q4_1", "iq4_nl", "q5_0", "q5_1"],
        );
        assert_eq!(cache.default.as_deref(), Some("f16"));

        // Either spelling finds it.
        assert!(read.has("-ctk"));
        assert!(read.takes("--cache-type-k", "q8_0"));
        assert!(
            !read.takes("--cache-type-k", "q3_K"),
            "not published, so not offered"
        );
    }

    #[test]
    fn a_default_written_inside_a_description_is_not_a_published_list() {
        /*
            `-fa` says `[on|off|auto]` in its argument and `default: 'auto'` inside its prose, and
            neither is the `allowed values:` line. Reading the prose would be inventing a
            measurement out of a sentence — so the flag is known to **exist** with no values, and
            a caller that needs the values asks a build that publishes them.
        */
        let read = Accepts::from_help(HELP);
        let fa = read.flag("--flash-attn").expect("published");
        assert!(fa.allowed.is_empty());
        assert_eq!(fa.default, None);
        assert!(
            read.has("--flash-attn"),
            "and it does exist, which is the fact in hand"
        );
        assert!(!read.takes("--flash-attn", "on"));
    }

    #[test]
    fn an_option_with_no_argument_and_one_with_a_brace_list_are_both_options() {
        let read = Accepts::from_help(HELP);
        assert!(read.has("--help"));
        assert_eq!(
            read.flag("--help").map(|it| it.names.clone()),
            Some(vec![
                "-h".to_owned(),
                "--help".to_owned(),
                "--usage".to_owned()
            ]),
        );
        assert!(read.has("--split-mode"));
        assert!(read.has("--override-tensor"));
        assert!(read.has("-ot"));
    }

    #[test]
    fn a_build_that_says_nothing_is_unasked_rather_than_empty() {
        /*
            The distinction the whole file turns on. `Accepts::read` answers `None` where the
            program could not be run, and `unasked()` is what a caller writes down when it means
            *assume nothing* — so an empty surface is never mistaken for a build with no options.
        */
        let nothing = Accepts::unasked();
        assert!(!nothing.anything());
        assert!(!nothing.has("--flash-attn"));

        let read = Accepts::from_help(HELP);
        assert!(read.anything());

        // Prose that mentions a flag does not create one.
        let prose = Accepts::from_help("this build supports --flash-attn and --cache-type-k\n");
        assert!(!prose.anything());
    }

    #[test]
    fn a_stray_allowed_values_line_does_not_attach_to_the_option_above_it() {
        let read = Accepts::from_help(
            "-x,    --exes N                         a thing\n\
             \n\
             ----- another section -----\n\
             \x20                                       allowed values: nonsense\n",
        );
        assert_eq!(read.flag("--exes").map(|it| it.allowed.len()), Some(0));
    }

    /// The parser against the build that is actually installed, rather than against an excerpt
    /// somebody pasted. `--help` starts no model and touches no card.
    #[test]
    #[ignore = "reads the llama.cpp installed on this machine"]
    fn the_installed_build_publishes_what_the_planner_needs() {
        let program = crate::runtimes::found_in(
            "llama-server",
            crate::runtimes::Runtime::LlamaCpp.winget_package(),
            None,
        )
        .expect("llama-server on this machine");
        let read = Accepts::read(&program).expect("it answered --help");

        assert!(read.anything());
        for name in [
            "--cache-type-k",
            "--cache-type-v",
            "--flash-attn",
            "--override-tensor",
            "--batch-size",
            "--ubatch-size",
            "--split-mode",
            "--ctx-size",
        ] {
            assert!(
                read.has(name),
                "{name} was not published by {}",
                program.display()
            );
        }
        assert!(
            read.takes("--cache-type-k", "q8_0"),
            "allowed values were: {:?}",
            read.flag("--cache-type-k").map(|it| it.allowed.clone()),
        );
        assert_eq!(
            read.flag("--cache-type-k")
                .and_then(|it| it.default.clone())
                .as_deref(),
            Some("f16")
        );
    }
}
