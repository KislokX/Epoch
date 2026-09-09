//! Terminals the **user** opens, inside the World.
//!
//! ## This is not a capability, and it must never become one
//!
//! Everything else in this crate that runs a program is a [`Capability`](crate::capability):
//! described, judged by `decide()`, recorded as evidence. A terminal is the opposite of all
//! three — it is an interactive shell with no sandbox, no declared effects and no transcript a
//! Quest could carry.
//!
//! So it is deliberately **not registered**, has no [`Descriptor`](epoch_kernel::Descriptor), and
//! is reachable only from a command a person clicked. A model cannot open one, cannot type into
//! one, and cannot read one. If that ever changes it is a redesign with an ADR, not a new
//! function here: `run_command` exists precisely so that a model's shell access is bounded,
//! explained and approved, and a terminal would be the way around it.
//!
//! What this is for is the thing Epoch cannot do on the user's behalf: **somebody else's
//! sign-in.** An MCP server's OAuth wizard asks for a Client ID and opens a browser; Epoch opens
//! the door and never holds the key (the rule already written for agent sign-in). Before this
//! existed, the answer to "that server needs re-authenticating" was *go and find a terminal*.
//!
//! ## Why a pseudo-terminal rather than pipes
//!
//! Measured, not assumed: a program asks whether its output is a terminal. One that finds pipes
//! stops prompting, stops colouring, and buffers everything until it exits — so a wizard waiting
//! for input would appear to have hung with a blank window. ConPTY on Windows, a pty elsewhere.

use crate::guard::guard;
use epoch_models::quiet::Quiet;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::Serialize;

/// How much of a terminal's output is kept so a reopened window is not blank.
///
/// Scrollback, bounded. A terminal left running for an hour would otherwise be a transcript
/// growing in memory with nothing ever reading the beginning of it.
const SCROLLBACK: usize = 256 * 1024;

/// One shell this machine actually has.
///
/// **Measured, never listed.** A menu offering PowerShell Core on a machine that does not have it
/// is the same lie as a gauge nobody can explain: the user clicks, something fails, and the fix
/// is not where they are looking.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shell {
    /// Stable id, what `open` is addressed by.
    pub id: String,
    /// What to call it on screen.
    pub label: String,
    /// The program, resolved to a real path where one was found.
    pub program: String,
    pub args: Vec<String>,
}

/// Every shell present, in the order a Windows user expects to see them.
///
/// **Every separator is the platform's own.** `Path::join("System32/cmd.exe")` produces
/// `C:\WINDOWS\System32/cmd.exe`, and that mixed form is not merely untidy: PowerShell, Git Bash
/// and WSL all start from it, and **`cmd.exe` starts and then prints nothing at all** — no banner,
/// no prompt, a black window with a cursor. The Node.js prompt *is* cmd, so it failed the same way
/// with "The syntax of the command is incorrect". Measured on all five; joining one component at a
/// time fixed both, and a test now refuses a mixed path outright.
///
/// One deliberate omission and one deliberate care:
///
/// - **`bash` on PATH is not Git Bash.** On Windows it resolves to `System32\bash.exe`, which is
///   WSL — a different thing entirely, and present even with no distro installed, in which case
///   it prints an error and exits. So Git Bash is found by its own install paths, and WSL is
///   offered only when it can name a distribution.
/// - The **Node.js command prompt** is `cmd` plus the environment script Node ships, which is
///   what the shortcut in the Start menu actually is. Offering bare `node` instead would give a
///   REPL, which is not what somebody clicking "Node.js" while installing an MCP server wants.
pub fn shells() -> Vec<Shell> {
    let mut found = Vec::new();
    let system = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());

    let mut add = |id: &str, label: &str, program: PathBuf, args: Vec<String>| {
        if program.is_file() {
            found.push(Shell {
                id: id.to_owned(),
                label: label.to_owned(),
                program: program.to_string_lossy().into_owned(),
                args,
            });
        }
    };

    if cfg!(windows) {
        add(
            "powershell",
            "Windows PowerShell",
            Path::new(&system)
                .join("System32")
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe"),
            Vec::new(),
        );
        if let Some(pwsh) = on_path("pwsh.exe") {
            add("pwsh", "PowerShell", pwsh, Vec::new());
        }
        add(
            "cmd",
            "Command Prompt",
            Path::new(&system).join("System32").join("cmd.exe"),
            Vec::new(),
        );
        for base in program_files() {
            let git = base.join("Git").join("bin").join("bash.exe");
            if git.is_file() {
                add(
                    "git_bash",
                    "Git Bash",
                    git,
                    vec!["--login".into(), "-i".into()],
                );
                break;
            }
        }
        for base in program_files() {
            let vars = base.join("nodejs").join("nodevars.bat");
            if vars.is_file() {
                add(
                    "node",
                    "Node.js command prompt",
                    Path::new(&system).join("System32").join("cmd.exe"),
                    vec!["/k".into(), vars.to_string_lossy().into_owned()],
                );
                break;
            }
        }
        if wsl_has_a_distribution() {
            add(
                "wsl",
                "WSL",
                Path::new(&system).join("System32").join("wsl.exe"),
                Vec::new(),
            );
        }
    } else {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into());
        add("shell", "Shell", PathBuf::from(shell), Vec::new());
    }

    found
}

/// How to hand **one command** to a shell and have it exit afterwards.
///
/// ## Why this is not the `args` on [`Shell`]
///
/// Those args open a shell *for a person*: Git Bash gets `--login -i`, the Node.js prompt gets
/// `/k` so the window stays. Running a single command wants the opposite of all of them — no
/// login profile noise, no interactivity, and above all **exit when finished**, because
/// `run_command` waits for the child and `/k` never returns.
///
/// So the two live side by side rather than one being derived from the other: they answer
/// different questions about the same program, and the day somebody derives one from the other
/// is the day a capability hangs for three minutes on a shell that was waiting to be typed into.
///
/// `None` for a shell that has no such form — WSL and the Node.js prompt are offered to a person
/// and are not something a capability starts.
pub fn to_run_one(shell: &Shell, command: &str) -> Option<(String, Vec<String>)> {
    let args: Vec<String> = match shell.id.as_str() {
        // `-Command` rather than `-File`, and `-NoProfile` so a user's profile cannot change
        // what a command means. `-NonInteractive` makes a prompt an error instead of a hang.
        "powershell" | "pwsh" => vec![
            "-NoProfile".into(),
            "-NonInteractive".into(),
            "-Command".into(),
            command.to_owned(),
        ],
        // `/C` runs and exits; `/K` is the one that stays, which is why the interactive Node
        // prompt above uses it and this must not.
        "cmd" => vec!["/C".into(), command.to_owned()],
        // `-lc` rather than `-c`: a login shell is what puts a user's own tools on PATH, which
        // is the difference between `nvm`-installed node being found and not.
        "git_bash" | "shell" => vec!["-lc".into(), command.to_owned()],
        _ => return None,
    };
    Some((shell.program.clone(), args))
}

/// The shell a command runs in when nobody named one: the first this machine reports that can
/// run one command and exit.
///
/// Measured rather than named — `shells()` already asks the machine what it has, and picking the
/// first of *that* list means a machine without PowerShell is not offered PowerShell. A machine
/// with nothing at all answers `None`, which the caller words as what it is.
pub fn default_shell() -> Option<Shell> {
    shells()
        .into_iter()
        .find(|one| to_run_one(one, "true").is_some())
}

fn program_files() -> Vec<PathBuf> {
    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(PathBuf::from)
        .chain(std::iter::once(PathBuf::from("C:/Program Files")))
        .collect()
}

fn on_path(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|path| {
        std::env::split_paths(&path)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
    })
}

/// Whether WSL has anything to run.
///
/// `wsl.exe` exists on a plain Windows install with no distribution, where it prints an error and
/// exits — so its presence proves nothing. Asking costs one short process, once.
fn wsl_has_a_distribution() -> bool {
    std::process::Command::new("wsl.exe")
        .quiet()
        .args(["--list", "--quiet"])
        .stdin(std::process::Stdio::null())
        .output()
        .map(|out| {
            // The list is UTF-16 on Windows, so the bytes are read for *any* non-space content
            // rather than parsed. Whether a distribution is named `Ubuntu` is not the question.
            out.status.success() && out.stdout.iter().any(|b| !b.is_ascii_whitespace())
        })
        .unwrap_or(false)
}

/// A directory a shell will accept as its own.
///
/// `Path::canonicalize` on Windows returns the **verbatim** form — `\\?\C:\Users\…` — and a
/// Project Root is canonicalised deliberately, because that is what makes a traversal
/// unrepresentable rather than merely filtered. It is also a path that several programs refuse:
///
/// - `cmd.exe` starts, prints *"UNC paths are not supported. Defaulting to Windows directory"*
///   and lands somewhere else;
/// - the Node.js prompt inherits that and adds *"The syntax of the command is incorrect"*;
/// - `npx` run inside one answers *"Invalid file: URL, must comply with RFC 8089"*.
///
/// PowerShell tolerates it, which is why the first version looked like it worked. Measured on
/// all four. The prefix is dropped **here** rather than in [`crate::project`]: a shell's opinion
/// about how a path is spelled is not a reason to weaken the type that guards file access.
fn plainly(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    match raw.strip_prefix(r"\\?\") {
        // `\\?\UNC\server\share` is a real network path, and its plain form keeps both slashes.
        Some(rest) => PathBuf::from(
            rest.strip_prefix("UNC\\")
                .map_or_else(|| rest.to_owned(), |share| format!(r"\\{share}")),
        ),
        None => path.to_path_buf(),
    }
}

/// One running terminal.
struct Session {
    /// Kept so the window can be resized. Dropping it is what closes the terminal.
    master: Box<dyn portable_pty::MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// What has been printed, bounded. So a window that was minimised and reopened is not blank.
    scrollback: Arc<Mutex<Vec<u8>>>,
}

/// Every terminal the user has open.
#[derive(Default)]
pub struct Terminals {
    open: Mutex<BTreeMap<String, Session>>,
}

impl std::fmt::Debug for Terminals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminals")
            .field("open", &self.ids().len())
            .finish()
    }
}

impl Terminals {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ids(&self) -> Vec<String> {
        self.open
            .lock()
            .expect("terminals lock")
            .keys()
            .cloned()
            .collect()
    }

    /// Start a shell and stream what it prints.
    ///
    /// `say` is called from a reader thread with each chunk of output. The Engine does not know
    /// where that goes — the caller is holding whatever can deliver it — which is what keeps this
    /// module free of the surface it feeds.
    ///
    /// `cwd` is where the terminal opens. The World's project root when it has one, so a wizard
    /// that writes a file writes it where the user is working rather than wherever Epoch was
    /// launched from.
    pub fn open(
        &self,
        id: &str,
        shell: &Shell,
        cwd: Option<&Path>,
        size: (u16, u16),
        say: impl Fn(String) + Send + 'static,
    ) -> Result<(), String> {
        if guard(&self.open).contains_key(id) {
            return Err(format!("a terminal called '{id}' is already open"));
        }

        let (cols, rows) = size;
        let pty = portable_pty::native_pty_system()
            .openpty(portable_pty::PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| format!("this machine would not give Epoch a terminal: {err}"))?;

        let mut command = portable_pty::CommandBuilder::new(&shell.program);
        for argument in &shell.args {
            command.arg(argument);
        }
        if let Some(cwd) = cwd.filter(|path| path.is_dir()) {
            command.cwd(plainly(cwd));
        }

        let child = pty
            .slave
            .spawn_command(command)
            .map_err(|err| format!("could not start {}: {err}", shell.label))?;
        let mut reader = pty
            .master
            .try_clone_reader()
            .map_err(|err| format!("could not read from {}: {err}", shell.label))?;
        let writer = pty
            .master
            .take_writer()
            .map_err(|err| format!("could not type into {}: {err}", shell.label))?;

        let scrollback = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&scrollback);
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8 * 1024];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let chunk = &buffer[..n];
                        {
                            let mut held = guard(&kept);
                            held.extend_from_slice(chunk);
                            if held.len() > SCROLLBACK {
                                let cut = held.len() - SCROLLBACK;
                                held.drain(..cut);
                            }
                        }
                        // Lossy on purpose. A chunk can end mid-character, and a terminal that
                        // dropped a whole line because one byte was incomplete would lose the
                        // prompt somebody is waiting for.
                        say(String::from_utf8_lossy(chunk).into_owned());
                    }
                }
            }
        });

        guard(&self.open).insert(
            id.to_owned(),
            Session {
                master: pty.master,
                writer,
                child,
                scrollback,
            },
        );
        Ok(())
    }

    /// Send what the user typed. Bytes, verbatim — control characters included.
    pub fn write(&self, id: &str, keys: &str) -> Result<(), String> {
        let mut open = guard(&self.open);
        let session = open
            .get_mut(id)
            .ok_or_else(|| format!("there is no terminal called '{id}'"))?;
        session
            .writer
            .write_all(keys.as_bytes())
            .and_then(|()| session.writer.flush())
            .map_err(|err| format!("that terminal is not listening: {err}"))
    }

    /// Tell the program how big its window is now.
    ///
    /// Not cosmetic: a shell lays out its prompt and a wizard wraps its text from this, and one
    /// that was never told stays at whatever it started with while the window says otherwise.
    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let open = guard(&self.open);
        let session = open
            .get(id)
            .ok_or_else(|| format!("there is no terminal called '{id}'"))?;
        session
            .master
            .resize(portable_pty::PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|err| err.to_string())
    }

    /// What this terminal has printed so far.
    ///
    /// So a window that was minimised, or a World that was left and returned to, comes back with
    /// its history rather than blank. The process never stopped; neither should what it said.
    pub fn scrollback(&self, id: &str) -> Option<String> {
        let open = guard(&self.open);
        let held = guard(&open.get(id)?.scrollback);
        Some(String::from_utf8_lossy(&held).into_owned())
    }

    /// End it, and the program with it.
    ///
    /// **Distinct from minimising**, which is a presentation state this module never hears about.
    /// A terminal is a running process; putting its window away must not kill it, and the only
    /// thing that does is this.
    pub fn close(&self, id: &str) -> Result<(), String> {
        let mut session = self
            .open
            .lock()
            .expect("terminals lock")
            .remove(id)
            .ok_or_else(|| format!("there is no terminal called '{id}'"))?;
        // Asked to stop, then dropped. Dropping the master closes the pty, which is what ends a
        // shell that ignored the signal — and the reader thread ends with it rather than being
        // left blocked on a handle nobody owns.
        let _ = session.child.kill();
        let _ = session.child.wait();
        Ok(())
    }

    /// End every one. Called when the shell is closing, so nothing is left running headless.
    pub fn close_all(&self) {
        let ids = self.ids();
        for id in ids {
            let _ = self.close(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_shell_is_launched_through_a_path_with_mixed_separators() {
        // Not cosmetic. `C:\WINDOWS\System32/cmd.exe` starts cmd and then produces **nothing** —
        // no banner, no prompt, a black window with a cursor — and the Node.js prompt, which is
        // cmd, answered "The syntax of the command is incorrect". PowerShell, Git Bash and WSL
        // all tolerated the same form, which is why it survived the first round of measuring.
        for shell in shells() {
            if cfg!(windows) {
                assert!(
                    !shell.program.contains('/'),
                    "'{}' is launched through {}, which cmd will not run properly",
                    shell.id,
                    shell.program
                );
            }
            for argument in &shell.args {
                assert!(
                    !argument.contains(":/"),
                    "'{}' passes {argument}, a mixed path",
                    shell.id
                );
            }
        }
    }

    #[test]
    fn every_shell_offered_is_one_this_machine_actually_has() {
        // Measured, never listed. A menu offering PowerShell Core on a machine without it sends
        // somebody to debug a failure that is not where they are looking.
        for shell in shells() {
            assert!(
                Path::new(&shell.program).is_file(),
                "'{}' points at {}, which is not there",
                shell.id,
                shell.program
            );
            assert!(!shell.label.trim().is_empty());
        }
    }

    #[test]
    fn a_shell_is_given_a_directory_it_will_accept() {
        // A Project Root is canonicalised on purpose — that is what makes a traversal
        // unrepresentable. On Windows that is the verbatim form, and `cmd.exe` refuses it:
        // "UNC paths are not supported. Defaulting to Windows directory." The Node prompt then
        // adds "The syntax of the command is incorrect", and `npx` inside one answers "Invalid
        // file: URL, must comply with RFC 8089". All four measured; PowerShell was the only one
        // that tolerated it, which is why the first version looked like it worked.
        assert_eq!(
            plainly(Path::new(r"\\?\C:\Users\me\project")),
            PathBuf::from(r"C:\Users\me\project")
        );
        // A real network path stays a network path.
        assert_eq!(
            plainly(Path::new(r"\\?\UNC\server\share\work")),
            PathBuf::from(r"\\server\share\work")
        );
        // Anything already plain is untouched.
        assert_eq!(
            plainly(Path::new(r"C:\Users\me\project")),
            PathBuf::from(r"C:\Users\me\project")
        );
        assert_eq!(
            plainly(Path::new("/home/me/project")),
            PathBuf::from("/home/me/project")
        );
    }

    #[test]
    #[cfg(windows)]
    fn git_bash_is_never_confused_with_wsl() {
        // `bash` on PATH is `System32\bash.exe` — WSL, present even with no distribution
        // installed, in which case it prints an error and exits. Somebody clicking "Git Bash"
        // means the one that came with Git.
        for shell in shells() {
            if shell.id == "git_bash" {
                assert!(
                    shell.program.to_lowercase().contains("git"),
                    "{} is not Git Bash",
                    shell.program
                );
                assert!(!shell.program.to_lowercase().contains("system32"));
            }
        }
    }

    /// A terminal is a **conversation**, and this proves it in both directions.
    ///
    /// Ignored because it starts a real shell. Run it after touching anything here:
    /// `cargo test -p epoch-engine --lib terminal -- --ignored --nocapture`
    ///
    /// The first attempt produced exactly one thing in twenty seconds: `ESC[6n`. That is
    /// ConPTY asking the terminal where the cursor is, and **waiting** — a real emulator
    /// answers `ESC[row;colR` and the shell then starts. Nothing in the Engine answers it, and
    /// nothing should: the emulator on the other end is what knows, and a second reply would put
    /// a stray `R` into the shell's input. So this test plays the part of the emulator, which is
    /// also what the production path relies on.
    #[test]
    #[ignore = "spawns real shells; a probe, not a unit test"]
    fn every_shell_actually_starts_where_it_was_told() {
        // A shell that *starts* is not a shell that works. PowerShell tolerated a verbatim cwd
        // and cmd did not, so a probe that tried one proved nothing about the other. This tries
        // all of them, in a directory whose name has a space in it — which is the other thing
        // that quietly breaks a command line.
        //
        // `cargo test -p epoch-engine --lib terminal -- --ignored --nocapture`
        let here = std::env::temp_dir().join("epoch probe dir");
        std::fs::create_dir_all(&here).unwrap();

        for shell in shells() {
            let terminals = Arc::new(Terminals::new());
            let (tx, rx) = std::sync::mpsc::channel();
            let answering = Arc::clone(&terminals);
            let id = shell.id.clone();
            let said = id.clone();
            terminals
                .open(&id, &shell, Some(&here), (100, 28), move |chunk| {
                    if chunk.contains("\u{1b}[6n") {
                        let _ = answering.write(&said, "\u{1b}[1;1R");
                    }
                    let _ = tx.send(chunk);
                })
                .unwrap();

            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
            let mut seen = String::new();
            while std::time::Instant::now() < deadline {
                match rx.recv_timeout(std::time::Duration::from_millis(400)) {
                    Ok(chunk) => seen.push_str(&chunk),
                    Err(_) if seen.len() > 40 => break,
                    Err(_) => {}
                }
            }
            let visible: String = seen
                .chars()
                .filter(|c| !c.is_control() || *c == '\n')
                .collect();
            println!("=== {} :: {} ===", shell.id, shell.program);
            println!("{}\n", visible.trim());
            let _ = terminals.close(&id);
        }
        let _ = std::fs::remove_dir_all(&here);
    }

    #[test]
    #[ignore = "spawns a real shell; a probe, not a unit test"]
    fn a_terminal_runs_and_answers() {
        let terminals = Arc::new(Terminals::new());
        let shell = shells().into_iter().next().expect("no shell at all");
        let (tx, rx) = std::sync::mpsc::channel();

        let answering = Arc::clone(&terminals);
        terminals
            .open("probe", &shell, None, (80, 24), move |chunk| {
                if chunk.contains("\u{1b}[6n") {
                    let _ = answering.write("probe", "\u{1b}[1;1R");
                }
                let _ = tx.send(chunk);
            })
            .unwrap();

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut seen = String::new();
        let mut asked = false;
        while std::time::Instant::now() < deadline {
            if let Ok(chunk) = rx.recv_timeout(std::time::Duration::from_millis(500)) {
                seen.push_str(&chunk);
            }
            // Only once the shell has actually printed something is there a prompt to type at.
            if !asked && seen.len() > 16 {
                asked = true;
                terminals.write("probe", "echo epoch-probe\r").unwrap();
            }
            // Twice: once as the echoed keystrokes, once as the shell's own answer. The second
            // is what proves a program ran rather than a pipe carrying letters back.
            if seen.matches("epoch-probe").count() >= 2 {
                break;
            }
        }
        println!("{seen}");
        assert!(
            seen.matches("epoch-probe").count() >= 2,
            "the shell never answered"
        );
        assert_eq!(terminals.ids(), vec!["probe"]);
        assert!(terminals
            .scrollback("probe")
            .unwrap()
            .contains("epoch-probe"));

        terminals.close("probe").unwrap();
        assert!(terminals.ids().is_empty());
    }
}
