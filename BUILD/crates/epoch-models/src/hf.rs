//! The Hugging Face CLI, when this machine has one.
//!
//! ## What it adds that Ollama cannot
//!
//! `ollama pull` fetches a model and files it where Ollama will find it — and that is the whole
//! of what it does, for Ollama. llama.cpp and LM Studio take a GGUF **from disk** and have no
//! endpoint that installs one, so until now Epoch could only hand them a URL and wish them luck.
//!
//! `hf` puts the file on disk, resumably, with a cache, and with an account attached. That last
//! part is not a nicety: gated repositories are unreachable without one, and today they fail
//! during weighing with an error that explains nothing.
//!
//! ## What it does not do
//!
//! **Run anything.** Its commands are `download`, `auth`, `cache`, `upload`, `repos`, `jobs`.
//! Running a model still needs Ollama or llama.cpp, and a surface that implied otherwise would
//! be promising something no part of this can deliver.
//!
//! ## Everything here was measured, on 2026-08-19, against 1.28.0
//!
//! ```text
//! hf version --format json      → {"version": "1.28.0"}
//! hf auth whoami --format json  → {"user": "KislokX", "orgs": null, "endpoint": null}
//! hf download … --dry-run --json → [{"file": "Qwen3-8B-Q4_K_M.gguf", "size": "5.0G"}]
//! hf download … --format json    → {"path": "C:\\…\\hfdl"}
//! ```
//!
//! Two of those shape the design. `--dry-run` says **exactly which file** a filter will fetch
//! before 17 GB is committed to it. And the download reports only a final path: there is no
//! progress stream, and none appears on stderr when it is not talking to a terminal — so this
//! reports the size it already knows and refuses to invent a percentage.

use crate::quiet::Quiet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Whether this machine has `hf`, and whether it is signed in.
///
/// Two facts with two fixes, kept apart for the same reason every agent's are: one is solved by
/// installing, the other by signing in, and a surface that collapsed them would send somebody to
/// do the wrong thing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cli {
    pub installed: bool,
    /// What it says it is. `None` when it is not there — never a guess.
    pub version: Option<String>,
    /// Where it was found, so "not installed" is a fact somebody can go and check.
    pub found_at: Option<String>,
    /// Who is signed in, as `hf` reports it. `None` is *unasked or signed out* — and unlike
    /// Gemini's, this one **can** be asked, so `None` here means it answered nothing useful.
    pub user: Option<String>,
}

/// Where `hf` is on this machine.
///
/// **PATH is not enough, and that is measured rather than assumed.** Its installer puts the
/// binary in `~/.local/bin`, which is not on the PATH of an application launched from a desktop
/// — on the machine this was written on, `hf` was installed and invisible to both shells Epoch
/// might inherit. Looking only at PATH would report "not installed" about a machine that has it.
pub fn found() -> Option<PathBuf> {
    let name = if cfg!(windows) { "hf.exe" } else { "hf" };

    // The installer's own location first: it is where it will be when nothing has been added to
    // a PATH, which is the common case.
    if let Some(home) = home() {
        let mine = home.join(".local").join("bin").join(name);
        if mine.is_file() {
            return Some(mine);
        }
    }

    // Then the PATH, for a machine where somebody put it somewhere else.
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

fn home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// Ask `hf` what it is and who it is.
///
/// Never fails: a machine without it is a machine that downloads through Ollama instead, which
/// is a smaller set of things it can do rather than a fault.
pub fn cli() -> Cli {
    let Some(binary) = found() else {
        return Cli::default();
    };

    #[derive(Deserialize)]
    struct Version {
        version: String,
    }
    #[derive(Deserialize)]
    struct Whoami {
        #[serde(default)]
        user: Option<String>,
    }

    let version = ask(&binary, &["version", "--format", "json"])
        .and_then(|said| serde_json::from_str::<Version>(&said).ok())
        .map(|read| read.version);

    Cli {
        installed: version.is_some(),
        version,
        found_at: Some(binary.display().to_string()),
        // Signed out exits non-zero, which `ask` already turns into `None`. An answer that
        // cannot be read is not an answer, here as everywhere else.
        user: ask(&binary, &["auth", "whoami", "--format", "json"])
            .and_then(|said| serde_json::from_str::<Whoami>(&said).ok())
            .and_then(|read| read.user),
    }
}

/// One file a download would fetch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Planned {
    pub file: String,
    /// As `hf` prints it: `5.0G`. Its own words rather than bytes, because that is what it gives
    /// and converting it would be Epoch restating a number it did not measure.
    pub size: String,
}

/// What a download would fetch, without fetching it.
///
/// **The reason this exists rather than downloading and finding out.** A quantisation filter is
/// a glob, and a glob that matches nothing downloads nothing while looking like success — or
/// matches four shards when somebody expected one file. `--dry-run` answers both before 17 GB
/// is committed.
pub fn plan(repo: &str, quant: &str) -> Result<Vec<Planned>, String> {
    let binary = found().ok_or_else(|| "the Hugging Face CLI is not on this machine".to_owned())?;
    let filter = glob_for(quant);
    let said = ask(
        &binary,
        &[
            "download",
            repo,
            "--include",
            &filter,
            "--dry-run",
            "--format",
            "json",
        ],
    )
    .ok_or_else(|| format!("`hf` could not read '{repo}'. It may be gated, or misspelled."))?;

    serde_json::from_str(&said).map_err(|err| format!("`hf` answered something unreadable: {err}"))
}

/// The glob that selects one quantisation.
///
/// Wrapped in stars because a repository names its files after the model as well as the
/// quantisation, and anchored on nothing else because naming schemes differ between publishers.
/// Shards come along by construction — `*Q4_K_XL*` matches `-00001-of-00002` too, which is
/// correct: a variant is all of its parts.
fn glob_for(quant: &str) -> String {
    format!("*{}*.gguf", quant.trim())
}

/// Fetch one quantisation onto this machine.
///
/// Blocking, and **without a percentage**. Measured: `--format json` reports only a final
/// `{"path": …}`, and no progress appears on stderr when it is not talking to a terminal. The
/// size is already known from [`plan`], so a surface can say what it is waiting for — inventing
/// a percentage from nothing is the one thing it must not do.
pub fn fetch(repo: &str, quant: &str, into: &std::path::Path) -> Result<PathBuf, String> {
    let binary = found().ok_or_else(|| "the Hugging Face CLI is not on this machine".to_owned())?;

    #[derive(Deserialize)]
    struct Landed {
        path: String,
    }

    let said = ask(
        &binary,
        &[
            "download",
            repo,
            "--include",
            &glob_for(quant),
            "--local-dir",
            &into.display().to_string(),
            "--format",
            "json",
        ],
    )
    .ok_or_else(|| format!("`hf` could not download '{quant}' from '{repo}'."))?;

    serde_json::from_str::<Landed>(&said)
        .map(|landed| PathBuf::from(landed.path))
        .map_err(|err| format!("`hf` answered something unreadable: {err}"))
}

/// Run `hf` and return its stdout, or `None` if it refused.
///
/// `None` rather than the error text on purpose: every caller here has a better sentence to say
/// than `hf`'s, because every caller knows what it was asking for.
fn ask(binary: &std::path::Path, arguments: &[&str]) -> Option<String> {
    let out = std::process::Command::new(binary)
        .quiet()
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let said = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    (!said.is_empty()).then_some(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_quantisation_becomes_a_filter_that_catches_its_shards() {
        // A variant is all of its parts. `BF16` arrives as `-00001-of-00002` and
        // `-00002-of-00002`, and a filter that caught only one would fetch half a model.
        assert_eq!(glob_for("Q4_K_XL"), "*Q4_K_XL*.gguf");
        assert_eq!(glob_for(" BF16 "), "*BF16*.gguf");
    }

    #[test]
    fn a_machine_without_it_is_not_a_broken_machine() {
        // It downloads through Ollama instead. Fewer things it can do, not a fault — so this
        // answers rather than failing.
        let read = Cli::default();
        assert!(!read.installed);
        assert_eq!(read.version, None);
        assert_eq!(read.user, None);
    }

    #[test]
    #[ignore = "reads this machine"]
    fn it_is_found_where_its_installer_puts_it() {
        // **PATH is not enough**, measured: the installer writes to `~/.local/bin`, which is not
        // on the PATH of an application launched from a desktop. On the machine this was written
        // on, `hf` was installed and invisible to every PATH Epoch might inherit — looking only
        // there would have reported "not installed" about a machine that had it.
        let read = cli();
        println!("{read:?}");
        assert!(read.installed, "hf is installed on this machine");
        assert!(read.version.is_some());
        assert!(
            read.found_at
                .as_deref()
                .is_some_and(|at| at.contains(".local")),
            "{read:?}"
        );
    }

    #[test]
    #[ignore = "reaches Hugging Face"]
    fn a_plan_names_the_file_before_anything_is_fetched() {
        // The whole reason to ask first: a glob that matches nothing downloads nothing while
        // looking like success.
        let planned = plan("unsloth/Qwen3-8B-GGUF", "Q4_K_M").expect("the repository exists");
        println!("{planned:?}");
        assert_eq!(planned.len(), 1, "one quantisation, one file here");
        assert!(planned[0].file.to_lowercase().ends_with(".gguf"));
        assert!(!planned[0].size.is_empty(), "and it says how big");

        // And a quantisation nobody published plans nothing, rather than pretending.
        let none = plan("unsloth/Qwen3-8B-GGUF", "Q9_IMAGINARY").expect("still answers");
        assert!(none.is_empty(), "{none:?}");
    }
}
