//! Running things on the user's machine.
//!
//! ## There is a shell, and everything follows from that
//!
//! The line is handed to one — `powershell -NoProfile -NonInteractive -Command <line>` here, the
//! machine's own default elsewhere, or whichever shell the call named among the ones
//! `terminal::shells()` measured. So `&&`, `|`, `;`, backticks, `$(…)` and `>` **are** operators,
//! and a command may start any program on this machine.
//!
//! **This page said the opposite for months and it was load-bearing.** The old header read
//! *there is no shell*, and reasoned from it that shell injection was not blocked but
//! inexpressible — sound reasoning about a program that no longer existed. `parse` began
//! handing the raw line to a shell the day `git commit -m "two words"` had to work, and that
//! change correctly documented itself two hundred lines further down while the summary at the
//! top kept describing the version before it.
//!
//! A stale comment is worse than a missing one exactly here: somebody auditing this file reads
//! the header, finds a guarantee stated as an absence rather than a check, and stops looking for
//! the check. An audit did precisely that in reverse — it read further than the header and found
//! the shell.
//!
//! **What actually guards this is the gate the user can see.** `run_command` declares `Executes`
//! and `Permanent`, so it computes to the highest risk there is and [`decide`](epoch_kernel::trust::decide) spells the exact
//! command out before it runs (ADR-0009). That is not a second line of defence behind a stronger
//! one; it is the whole of it, and it is the half a user can overrule.
//!
//! ## An allowlist of programs, not of commands
//!
//! `git status` is safe and `git push` is not, and a list that tried to encode that would be
//! wrong for somebody within a week — every project has its own scripts, and sub-command
//! filtering is a game of catch-up nobody wins.
//!
//! So the list is of **programs**, and the second gate is the one that already works: this
//! declares `Executes` and `Permanent`, so it computes to the highest risk there is, and the
//! user sees the exact command spelled out before it runs (ADR-0009).
//!
//! In `Auto` mode it will run without asking — that is what Auto means, and a standing `Deny`
//! still wins. Said plainly rather than quietly special-cased.
//!
//! ## It runs in the project, and only there
//!
//! The working directory is the Project Root, or a folder inside it named by `directory`. A
//! program can of course write outside it — that is what running a program *is* — which is
//! precisely why this is the highest risk in the system rather than something confinement
//! pretends to contain.
//!
//! `directory` exists because without it a character could clone a repository and then not work
//! in it: every command ran at the root, so `cargo test` after a clone tested the wrong project.
//! The *choice of where* is confined like any other path even though what runs there is not.

use epoch_models::quiet::Quiet;
use std::io::BufReader;
use std::path::Path;
use std::process::{ChildStderr, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use epoch_kernel::{
    Arguments, CapabilityId, Descriptor, Effect, Explanation, Parameter, Reversal, ValueKind,
};

use crate::capability::{Capability, CapabilityError, Outcome};
use crate::project::ProjectRoot;

/// The ten programs this capability used to be limited to.
///
/// ## Why the list is gone, and what took its place
///
/// It was a second gate, and it was the invisible one. `decide()` (ADR-0009) already asks the
/// user before anything executes — `run_command` is `Risk::High` and `Reversal::Permanent`, so
/// Manual and Accept-edits stop and ask, and `Auto` is the user saying *free use of the tools
/// this character has*. On top of that sat a constant in a source file that no surface showed,
/// no setting reached and no approval could overrule: a character told `abre Brave` answered
/// *"mis herramientas están limitadas a git, node, python"* and offered the command for the
/// person to paste into a terminal themselves.
///
/// That answer was **accurate**, which is what made it worth changing. The model was reading its
/// own descriptor. The product had decided, in a place the owner could not see or reach, that
/// his machine was not his to use.
///
/// It is the same shape as the `Manual` mode that governed nothing (ADR-0027) and the closed
/// Style catalogue (ADR-0030): a control whose real answer lives somewhere the user cannot get
/// to. The rule that settles it is already written down — **Epoch adds tools; it does not take
/// them away.** A gate the user cannot open is not a gate, it is a wall with a doorframe
/// painted on it.
///
/// Kept as a name, empty, so anything still importing it fails to compile rather than silently
/// keeping a rule that no longer exists.
pub const ALLOWED: [&str; 0] = [];

/// How long a command may take before it is given up on.
///
/// Long enough for a real build, short enough that a program waiting on input somebody will
/// never type does not hold the turn open forever.
const TIMEOUT: Duration = Duration::from_secs(180);

/// The most output handed back. Past this it is cut, and the cut is announced.
const MAX_OUTPUT: usize = 32 * 1024;

/// How long the readers get after the child is killed before the turn stops waiting for them.
///
/// **Killing a shell does not kill what the shell started.** A grandchild that inherited the
/// pipes holds them open, so the loop below would never see the channel close and the deadline
/// would go back to being a measurement of how long somebody waited. `stop` asks the operating
/// system for the whole tree where it can be asked; this is what makes the deadline true on a
/// machine where it cannot.
const GRACE: Duration = Duration::from_secs(5);

/// How many lines may sit in the channel before the readers wait.
///
/// The body is capped at [`MAX_OUTPUT`] and the channel was not, so a program printing faster
/// than this loop consumes had an unbounded queue behind a bounded string — the cap looked like
/// a memory limit and only ever limited what was *kept*. A reader that waits costs nothing: the
/// child blocks on a full pipe, which is exactly what a terminal does to it.
const QUEUED: usize = 1024;

/// One of a child's two pipes, so both can be drained by the same code.
///
/// A plain `Box<dyn BufRead + Send>` would do the same job; this keeps the two named, which is
/// what a reader of the thread body wants to know.
enum Reader {
    Out(ChildStdout),
    Err(ChildStderr),
}

impl Reader {
    /// Read to the end, posting every line. Lossy on purpose: a build that prints one invalid
    /// byte should not lose the rest of its output to an encoding error.
    fn drain(self, post: &mpsc::SyncSender<String>) {
        // Generic on each arm rather than boxed: a bounded read needs `Sized`, and that
        // is the whole of the fix — see `super::reader`.
        match self {
            Reader::Out(pipe) => super::reader::post_lines(BufReader::new(pipe), post),
            Reader::Err(pipe) => super::reader::post_lines(BufReader::new(pipe), post),
        }
    }
}

/// Stop a child, and everything it started.
///
/// The line is handed to a shell, so `child` is the shell and the program somebody actually ran
/// is its child. This is [`epoch_models::quiet::stop_tree`] — the same fact turned up again in
/// the Gemini adapter, where a `.cmd` shim hides a Node grandchild, so it stopped being this
/// module's private knowledge.
fn stop(child: &mut std::process::Child) {
    epoch_models::quiet::stop_tree(child);
}

fn id(raw: &str) -> CapabilityId {
    CapabilityId::new(raw).expect("capability ids in this file are literals")
}

/// Run any command on this machine, in the project.
pub struct RunCommand {
    root: ProjectRoot,
}

impl RunCommand {
    pub fn new(root: ProjectRoot) -> Self {
        Self { root }
    }

    /// The folder named by `directory`, if there is one and it is not blank.
    fn asked_for(arguments: &Arguments) -> Option<&str> {
        match arguments.get("directory") {
            Some(epoch_kernel::Value::Text(raw)) if !raw.trim().is_empty() => Some(raw.trim()),
            _ => None,
        }
    }

    /// Where the command should run: the Project Root, or a folder inside it.
    ///
    /// Confined exactly like every other path, and required to exist. A missing folder is a
    /// mistake worth naming — quietly falling back to the root would run the command somewhere
    /// else and then report that it succeeded.
    fn directory(&self, arguments: &Arguments) -> Result<std::path::PathBuf, CapabilityError> {
        let Some(raw) = Self::asked_for(arguments) else {
            return Ok(self.root.path().to_path_buf());
        };
        let inside = self.root.confine(raw).map_err(CapabilityError::Refused)?;
        if !inside.is_dir() {
            return Err(CapabilityError::Failed(format!(
                "there is no folder `{raw}` in the project"
            )));
        }
        Ok(inside)
    }

    /// How the working directory reads to a person: relative to the root, or nothing at all.
    fn where_shown(arguments: &Arguments) -> String {
        Self::asked_for(arguments)
            .map(|raw| format!(" in `{raw}`"))
            .unwrap_or_default()
    }

    /// Split the requested command into a program and its arguments.
    ///
    /// The shell this command will be handed to.
    ///
    /// Named by the caller where they name one, and the machine's own first otherwise. **Only
    /// shells this machine actually reports** — `terminal::shells()` measures rather than lists,
    /// so a refusal here names what exists instead of what was imagined.
    fn shell(&self, arguments: &Arguments) -> Result<crate::terminal::Shell, CapabilityError> {
        let asked = match arguments.get("shell") {
            Some(epoch_kernel::Value::Text(raw)) if !raw.trim().is_empty() => {
                Some(raw.trim().to_owned())
            }
            _ => None,
        };
        let Some(want) = asked else {
            return crate::terminal::default_shell().ok_or_else(|| {
                CapabilityError::Failed(
                    "this machine reported no shell that can run a command".into(),
                )
            });
        };
        crate::terminal::shells()
            .into_iter()
            .find(|one| one.id == want && crate::terminal::to_run_one(one, "true").is_some())
            .ok_or_else(|| {
                CapabilityError::Refused(format!(
                    "this machine has no shell called '{want}'. It has: {}",
                    shell_ids().join(", ")
                ))
            })
    }

    /// The command, and the shell invocation that will run it.
    ///
    /// **A shell, now, and that is the change.** It used to split on whitespace and refuse
    /// anything outside a ten-program list, on the reasoning that no shell means nothing in the
    /// arguments can become an operator. That was true and it also meant `git commit -m "two
    /// words"` was three arguments — the file said so itself: *an argument containing a space is
    /// a limitation*. Handing the line to a shell fixes the quoting and grants the pipes,
    /// redirection and chaining that were the other half of what was missing.
    ///
    /// What guards this is [`decide`](epoch_kernel::trust::decide), which is the gate the user
    /// can actually see and change. See [`ALLOWED`] for why that is the whole of it.
    fn parse(&self, arguments: &Arguments) -> Result<(String, Vec<String>), CapabilityError> {
        let raw = arguments
            .text("command")
            .map_err(CapabilityError::BadArguments)?;
        if raw.trim().is_empty() {
            return Err(CapabilityError::BadArguments(
                "a command is required".into(),
            ));
        }
        let shell = self.shell(arguments)?;
        crate::terminal::to_run_one(&shell, raw.trim()).ok_or_else(|| {
            CapabilityError::Failed(format!("{} cannot be given a single command", shell.label))
        })
    }
}

impl Capability for RunCommand {
    fn describe(&self) -> Descriptor {
        run_command()
    }

    fn explain(&self, arguments: &Arguments) -> Result<Explanation, CapabilityError> {
        let descriptor = self.describe();
        let problems = arguments.check(&descriptor.parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        // Refused here as well as in `run`, so nobody is asked to approve a command that could
        // not have started.
        let (program, args) = self.parse(arguments)?;

        // **The command the person wrote, not the shell invocation around it.** `parse` now
        // answers `powershell -NoProfile -NonInteractive -Command <line>`, and showing that for
        // approval would bury the one part somebody is deciding about under four arguments
        // Epoch chose itself. The shell is named beside it, because *which* shell is part of
        // what is being approved.
        let _ = (program, args);
        let shown = arguments
            .text("command")
            .map_err(CapabilityError::BadArguments)?
            .trim()
            .to_owned();
        let through = self.shell(arguments)?;
        // Where is part of what is approved: `cargo test` at the root and `cargo test` inside a
        // repository somebody just cloned are different acts.
        Ok(Explanation::of(
            &descriptor,
            format!(
                "run `{shown}` in {}{}",
                through.label,
                Self::where_shown(arguments)
            ),
        )
        .expecting("whatever it prints, and whether it succeeded"))
    }

    fn run_watched(
        &self,
        arguments: &Arguments,
        saying: &mut dyn FnMut(&str),
    ) -> Result<Outcome, CapabilityError> {
        self.execute(arguments, Some(saying))
    }

    fn run(&self, arguments: &Arguments) -> Result<Outcome, CapabilityError> {
        self.execute(arguments, None)
    }
}

impl RunCommand {
    /// Start it, watch it, and stop it if it outstays the limit.
    ///
    /// ## Why this is not `Command::output()` any more
    ///
    /// `output()` waits for the program to end and hands back everything at once. Two things
    /// followed from that, and both were wrong:
    ///
    /// - **The timeout was measured, not enforced.** A program waiting on input nobody will
    ///   ever type held the turn open forever, and the code could only say afterwards that it
    ///   had taken too long. Now the deadline is real: past it the child is killed and what it
    ///   printed before dying is still returned.
    /// - **A three-minute build was a spinner.** There was no way to say what was happening,
    ///   because nothing left the call until it was over.
    ///
    /// Two reader threads rather than a poll over pipes: reading one pipe while the other fills
    /// its buffer is how a process deadlocks against its own output, and that is a bug that
    /// only appears once somebody runs something chatty.
    fn execute(
        &self,
        arguments: &Arguments,
        mut saying: Option<&mut dyn FnMut(&str)>,
    ) -> Result<Outcome, CapabilityError> {
        let problems = arguments.check(&self.describe().parameters);
        if !problems.is_empty() {
            return Err(CapabilityError::from_problems(problems));
        }
        let (program, args) = self.parse(arguments)?;

        if !self.root.still_there() {
            return Err(CapabilityError::Failed(
                "this World's project folder is not there any more".into(),
            ));
        }

        let working = self.directory(arguments)?;

        let started = Instant::now();
        // The shell, and the one command it was handed. Everything a person could type at a
        // prompt is available here, which is the point -- and which is why `decide()` is asked
        // first, every time, unless the user has put this World in Auto.
        let mut child = Command::new(&program)
            .quiet()
            // So `stop` can end the whole group and not just the shell — see `stop_tree`.
            .own_group()
            .args(&args)
            .current_dir(&working)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| {
                CapabilityError::Failed(format!("could not start '{program}': {err}"))
            })?;

        // One channel, two readers. Interleaved as it actually happened rather than stdout then
        // stderr — a compiler's warnings belong beside the file that caused them.
        let (post, lines) = mpsc::sync_channel::<String>(QUEUED);
        let mut readers = Vec::new();
        for pipe in [
            child.stdout.take().map(Reader::Out),
            child.stderr.take().map(Reader::Err),
        ]
        .into_iter()
        .flatten()
        {
            let post = post.clone();
            readers.push(std::thread::spawn(move || pipe.drain(&post)));
        }
        // The original sender must go, or the loop below waits on a channel nobody will close.
        drop(post);

        let mut body = String::new();
        let mut cut = false;
        let mut killed = false;
        let mut gave_up = None;
        loop {
            match lines.recv_timeout(Duration::from_millis(200)) {
                Ok(line) => {
                    if let Some(say) = saying.as_deref_mut() {
                        say(&line);
                    }
                    if body.len() + line.len() < MAX_OUTPUT {
                        body.push_str(&line);
                        body.push('\n');
                    } else {
                        // Kept running rather than killed: the command's *effect* is the point,
                        // and stopping a build because it printed too much would be absurd.
                        cut = true;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            if !killed && started.elapsed() > TIMEOUT {
                // Enforced, at last. What it printed before dying is still returned — a program
                // that hung after doing half the work should say which half.
                stop(&mut child);
                killed = true;
                gave_up = Some(Instant::now());
            }
            // **And the deadline holds even if something survived the kill.** Only the channel
            // closing ends this loop otherwise, and a grandchild holding the pipes never closes
            // it. Past the grace period the turn stops listening.
            if gave_up.is_some_and(|at| at.elapsed() > GRACE) {
                break;
            }
        }
        /*
            **Dropping the receiver frees a reader stuck sending. It does not free one stuck
            reading.**

            The first half was already true and is why this drop is here: a reader parked in
            `send` on a full channel waits for this thread, which would then be waiting for it.
            With the receiver gone the send fails and the reader leaves.

            The second half is what the second audit found (2026-09-07, finding 5). A grandchild
            that inherited the pipes holds them open after the shell is killed, so a reader
            blocked in `read` never returns and `join()` waits for it forever — which turns the
            deadline above back into a measurement of how long somebody waited, the exact thing
            `GRACE` exists to prevent.

            So the readers get the same grace and are then **abandoned**. That is a real cost and
            a bounded one: a thread holding a pipe and a stack until the grandchild exits. A turn
            that never returns is neither bounded nor recoverable, and it is the one the person
            in front of the World would be waiting on.
        */
        drop(lines);
        let waited = Instant::now();
        for reader in readers {
            while !reader.is_finished() && waited.elapsed() < GRACE {
                std::thread::sleep(Duration::from_millis(20));
            }
            if reader.is_finished() {
                let _ = reader.join();
            }
        }

        let outcome = child
            .wait()
            .map_err(|err| CapabilityError::Failed(format!("lost '{program}': {err}")))?;
        let took = started.elapsed();

        let mut notes = Vec::new();
        if cut {
            notes.push(format!("output cut at {MAX_OUTPUT} bytes"));
        }
        if killed {
            notes.push(format!("stopped after {}s", TIMEOUT.as_secs()));
        } else if took > TIMEOUT {
            notes.push(format!("took {}s", took.as_secs()));
        }

        let status = if killed {
            "was stopped for taking too long".to_owned()
        } else {
            match outcome.code() {
                Some(0) => "succeeded".to_owned(),
                Some(code) => format!("exited {code}"),
                None => "was stopped".to_owned(),
            }
        };
        /*
            **The command that was asked for, not the invocation Epoch wrapped around it.**

            This built the sentence from `program` and `args`, which was right while those were
            the command. They are the shell now, so the Chronicle read

                ran C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe
                    -NoProfile -NonInteractive -Command where brave

            for a character that ran `where brave`. Seen in the window the moment the shell
            landed. The approval sentence had already been given this treatment deliberately;
            the evidence line is read by the same person, later, and needed it more -- History is
            what somebody consults when they no longer remember what happened.
        */
        let shown = arguments
            .text("command")
            .map(|raw| raw.trim().to_owned())
            .unwrap_or_else(|_| format!("{program} {}", args.join(" ")));

        let place = Self::where_shown(arguments);
        let head = if notes.is_empty() {
            format!("{shown}{place} — {status}\n")
        } else {
            format!("{shown}{place} — {status} ({})\n", notes.join(", "))
        };

        // Evidence only when it worked. A command that failed produced a lesson, not an
        // artifact, and a History padded with failed attempts would be as misleading as one
        // that hid them (ADR-0025 — evidence, never narration).
        if outcome.success() {
            Ok(Outcome::made(
                format!("{head}{body}"),
                shown.clone(),
                format!("ran {shown}{place}"),
            ))
        } else {
            Ok(Outcome::told(format!("{head}{body}")))
        }
    }
}

/// Whether a program name is one Epoch will start. Exposed for the tests and for a surface that
/// wants to say what is possible before anybody tries.
/// Whether Epoch will start this program.
///
/// It will. Kept because callers ask, and answering `true` is the honest reading now that the
/// only gate is Trust — see [`ALLOWED`].
pub fn is_allowed(_program: &str) -> bool {
    true
}

/// The project root a command would run in. Used by callers that need to say where.
pub fn working_directory(root: &ProjectRoot) -> &Path {
    root.path()
}

/// What this capability is, without needing one to exist.
///
/// **Separate from `describe`, so the catalogue can be read without a Project Root.** A
/// descriptor is static text and parameters; it never depended on the root a capability was
/// constructed with. Keeping it reachable only through an instance meant the one list a surface
/// needs - what this build can be asked for - had to be a hand-written constant somewhere else,
/// and that constant could not grow when a capability did.
pub fn run_command() -> Descriptor {
    // **Measured, never listed.** A description naming PowerShell on a machine that has none is
    // the gauge nobody can explain, arriving in prompt form -- and a model reads a descriptor as
    // a statement of fact about the world it is in.
    let shells = shell_ids();
    let here = crate::terminal::default_shell()
        .map(|one| one.label)
        .unwrap_or_else(|| "no shell this machine reported".to_owned());

    Descriptor::acting(
        id("run_command"),
        format!(
            "Run any command on this machine, through {here}. Pipes, redirection, chaining and \
             quoting all work, and any program installed here can be started -- not only \
             development tools. When running something is what was asked for, run it; do not \
             write the command out for the user to run themselves.{}{}",
            detach_advice(),
            if shells.len() > 1 {
                format!(" Other shells here: {}.", shells.join(", "))
            } else {
                String::new()
            }
        ),
        [Effect::Executes, Effect::Writes, Effect::Reads],
        Reversal::Permanent,
    )
    .taking([
        Parameter::required(
            "command",
            ValueKind::Text,
            "The whole command line, exactly as it would be typed at a prompt — e.g. \
             `cargo test`, `git log --oneline | head -5`, or a full path in quotes.",
        ),
        Parameter::optional(
            "directory",
            ValueKind::Text,
            "Folder to run in, relative to the project root — e.g. a repository that was \
             just cloned. Omit to run at the root.",
        ),
        Parameter::optional(
            "shell",
            ValueKind::Text,
            &format!(
                "Which shell to use. Omit for {here}. This machine has: {}.",
                shells.join(", ")
            ),
        ),
    ])
}

/// How to start something that keeps running, in the words of the shell that is actually here.
///
/// ## Measured, and it is not a hypothetical
///
/// Asked to open a browser with its full path, `gemma-4-12B-it-qat` called
/// `& "…\brave.exe"` — correct PowerShell, and it **waits for the program to exit**. Brave
/// opened, and the turn then sat there: the character read WORKING for the whole three-minute
/// limit, after which the child is killed and a launch that plainly succeeded is reported as
/// *was stopped*.
///
/// Epoch does not rewrite the command — that is the user's and the character's (ADR-0033, one
/// subsystem over). What it can do is say which spelling does not block, in the vocabulary of
/// the shell this machine reported, rather than a general remark about backgrounding that is
/// wrong on two of the three.
///
/// Empty for a shell nothing was measured about: silence is the honest answer to a question
/// nobody asked, and a wrong incantation is worse than none.
fn detach_advice() -> String {
    let Some(shell) = crate::terminal::default_shell() else {
        return String::new();
    };
    let how = match shell.id.as_str() {
        "powershell" | "pwsh" => "Start-Process",
        "cmd" => "start \"\"",
        "git_bash" | "shell" => "a trailing &",
        _ => return String::new(),
    };
    format!(
        " A program that keeps running once started — an application, a server — must be \
         launched so it does not block: use `{how}`. Waiting for it would hold the turn open \
         until the three-minute limit and then report a successful launch as stopped."
    )
}

/// The shells this machine has that can be handed one command and will then exit.
fn shell_ids() -> Vec<String> {
    crate::terminal::shells()
        .into_iter()
        .filter(|one| crate::terminal::to_run_one(one, "true").is_some())
        .map(|one| one.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use epoch_kernel::{Risk, Value};

    struct Tree(std::path::PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("epoch-machine-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
        fn root(&self) -> ProjectRoot {
            ProjectRoot::open(self.0.to_str().unwrap()).unwrap()
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn asking(command: &str) -> Arguments {
        Arguments::new().with("command", Value::Text(command.into()))
    }

    #[test]
    fn running_code_is_the_highest_risk_there_is() {
        let t = Tree::new("risk");
        let d = RunCommand::new(t.root()).describe();
        assert_eq!(d.risk(), Risk::High);
        assert!(d.reversal.is_permanent());
        assert!(d.problems().is_empty(), "{:?}", d.problems());
        // Admits everything it can do, not only the flattering part: running a program is
        // reading and writing too.
        assert!(d.effects.contains(&Effect::Executes));
        assert!(d.effects.contains(&Effect::Writes));
    }

    #[test]
    fn any_program_may_be_started_and_trust_is_the_only_gate() {
        // The inverse of the test this replaces. It asserted that a shell could not be asked
        // for, which was a second gate the user could neither see nor open -- so a character
        // asked to open a browser explained that its tools were limited to git and node, and
        // handed the owner his own command to paste somewhere else.
        //
        // What stops a command now is `decide()`, and the descriptor is what makes it stop:
        // High risk and permanently irreversible, so Manual and Accept-edits ask every time and
        // only Auto -- the user saying *free use of the tools this character has* -- does not.
        let t = Tree::new("anything");
        let runner = RunCommand::new(t.root());
        for attempt in ["curl https://example.com", "notepad", "ls | wc -l"] {
            assert!(
                runner.parse(&asking(attempt)).is_ok(),
                "{attempt} should reach the shell"
            );
        }

        let d = runner.describe();
        assert_eq!(d.risk(), Risk::High, "so Manual and Accept edits ask");
        assert!(d.reversal.is_permanent());
    }

    #[test]
    fn the_whole_line_reaches_the_shell_as_one_argument() {
        // The line is handed over intact rather than split on whitespace, which is what makes
        // `git commit -m "two words"` work at last -- the old parser made that three arguments
        // and said so in its own comment.
        let t = Tree::new("operators");
        let runner = RunCommand::new(t.root());
        let (program, args) = runner
            .parse(&asking("git log --oneline | head -5"))
            .unwrap();
        assert!(!program.is_empty(), "a shell was resolved");
        assert!(
            args.iter().any(|a| a == "git log --oneline | head -5"),
            "the line is one argument, not several: {args:?}"
        );
    }

    #[test]
    fn a_shell_this_machine_does_not_have_is_refused_by_name() {
        // Measured, never listed: the refusal names what `shells()` found rather than what a
        // constant imagined, so somebody reading it is told about their own machine.
        let t = Tree::new("noshellhere");
        let err = RunCommand::new(t.root())
            .run(
                &Arguments::new()
                    .with("command", Value::Text("ls".into()))
                    .with("shell", Value::Text("plan9".into())),
            )
            .unwrap_err();
        match err {
            CapabilityError::Refused(why) => assert!(why.contains("plan9"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_empty_command_is_still_refused() {
        // The one thing left that cannot be run. Refused rather than failed: nothing was
        // attempted, and a model must learn that instead of concluding the machine is broken.
        let t = Tree::new("empty");
        let err = RunCommand::new(t.root()).run(&asking("   ")).unwrap_err();
        assert!(matches!(err, CapabilityError::BadArguments(_)), "{err:?}");
    }

    #[test]
    fn the_approval_shows_the_exact_command() {
        // The user is approving *this*, not the idea of running something.
        let t = Tree::new("explain");
        let explained = RunCommand::new(t.root())
            .explain(&asking("cargo test --lib"))
            .unwrap();
        assert!(
            explained.what.starts_with("run `cargo test --lib` in "),
            "the command as written, and the shell it goes to: {}",
            explained.what
        );
        // And never the invocation Epoch wrapped around it: `-NoProfile -NonInteractive
        // -Command` is four arguments nobody is deciding about, and burying the real one under
        // them is how an approval stops being read.
        assert!(!explained.what.contains("-NoProfile"), "{}", explained.what);
        assert!(explained.reversal.is_permanent());
    }

    #[test]
    fn a_command_that_could_not_start_is_never_offered_for_approval() {
        // Still true, and about the cases that remain: nothing to run, and a shell that is not
        // on this machine. Asking somebody to approve a command that could not have started is
        // a decision with no consequence, which teaches them the prompt is noise.
        let t = Tree::new("noapproval");
        let runner = RunCommand::new(t.root());
        assert!(runner.explain(&asking("")).is_err());
        assert!(runner
            .explain(
                &Arguments::new()
                    .with("command", Value::Text("ls".into()))
                    .with("shell", Value::Text("plan9".into()))
            )
            .is_err());
    }

    #[test]
    fn a_program_that_is_not_installed_fails_rather_than_being_refused() {
        // A different answer from "not allowed", because it is a different problem and the
        // model should not conclude it is forbidden from something it simply cannot find.
        let t = Tree::new("missing");
        // `tsc` is on the list and almost certainly absent from a bare test machine.
        match RunCommand::new(t.root()).run(&asking("tsc --version")) {
            Err(CapabilityError::Failed(why)) => assert!(why.contains("could not start")),
            // If it *is* installed, that is a fine outcome too — the point is that it was
            // attempted rather than refused.
            Ok(_) => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn only_a_command_that_worked_leaves_evidence() {
        // A failed command produced a lesson, not an artifact. A History padded with failed
        // attempts would mislead as much as one that hid them.
        let t = Tree::new("evidence");
        // `git` with no arguments exits non-zero on every platform that has it.
        if let Ok(outcome) = RunCommand::new(t.root()).run(&asking("git")) {
            assert_eq!(outcome.evidence, None);
            assert!(outcome.content.contains("exited"));
        }
    }

    fn asking_in(command: &str, directory: &str) -> Arguments {
        asking(command).with("directory", Value::Text(directory.into()))
    }

    #[test]
    fn a_directory_outside_the_project_is_refused() {
        // The command may write anywhere once it is running — that is what execution is. The
        // *choice of where to start it* is still confined, because that choice is Epoch's.
        let t = Tree::new("escape");
        let runner = RunCommand::new(t.root());
        // The last one is absolute **for the machine this is running on**. `C:\\Windows` is
        // a path on Windows and an ordinary relative folder name on Unix, where refusing it
        // would be wrong rather than safe — so on a Mac that entry tested nothing while reading
        // as though it tested the most important case.
        let elsewhere = if cfg!(windows) { "C:\\Windows" } else { "/etc" };
        for attempt in ["..", "../..", "~", elsewhere] {
            assert!(
                matches!(
                    runner.run(&asking_in("git status", attempt)),
                    Err(CapabilityError::Refused(_))
                ),
                "{attempt} should not be somewhere a command can be started"
            );
        }
    }

    #[test]
    fn a_directory_that_is_not_there_is_named_rather_than_ignored() {
        // Falling back to the root would run the command in the wrong place and then report
        // that it succeeded — the worst of the available answers.
        let t = Tree::new("missingdir");
        match RunCommand::new(t.root()).run(&asking_in("git status", "cloned-repo")) {
            Err(CapabilityError::Failed(why)) => assert!(why.contains("cloned-repo"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn where_it_runs_is_part_of_what_is_approved() {
        // `cargo test` at the root and `cargo test` inside a repository somebody just cloned are
        // different acts, so the approval may not read the same for both.
        let t = Tree::new("approvewhere");
        let runner = RunCommand::new(t.root());
        let root = runner.explain(&asking("cargo test")).unwrap();
        let inside = runner
            .explain(&asking_in("cargo test", "vendor/thing"))
            .unwrap();
        assert!(!root.what.contains("vendor/thing"), "{}", root.what);
        assert!(
            inside.what.ends_with("in `vendor/thing`"),
            "{}",
            inside.what
        );
    }

    #[test]
    fn a_command_runs_where_it_was_asked_to() {
        let t = Tree::new("runwhere");
        std::fs::create_dir_all(t.0.join("inner")).unwrap();
        // `git status` in a folder that is not a repository still proves *where* it ran: git
        // names the directory it was started in when it complains.
        if let Ok(outcome) = RunCommand::new(t.root()).run(&asking_in("git status", "inner")) {
            assert!(
                outcome.content.contains("in `inner`"),
                "{}",
                outcome.content
            );
        }
    }

    #[test]
    fn output_arrives_while_it_runs_and_still_arrives_at_the_end() {
        // Both, deliberately. The lines are for watching; the `Outcome` is what the model reads
        // and what History is built from — a version that streamed *instead* of returning would
        // have made the panel prettier and the turn emptier.
        let t = Tree::new("watched");
        let runner = RunCommand::new(t.root());
        let mut said = Vec::new();
        // `git --version` prints one line on every platform that has git; a machine without it
        // fails to start, which the test above already covers.
        if let Ok(outcome) = runner.run_watched(&asking("git --version"), &mut |line| {
            said.push(line.to_owned());
        }) {
            assert!(!said.is_empty(), "nothing was said while it ran");
            let printed = said.join("\n");
            assert!(
                outcome.content.contains(printed.trim()),
                "what was watched is missing from the result: {printed:?}"
            );
        }
    }

    #[test]
    fn a_capability_that_does_not_stream_is_unaffected() {
        // `run_watched` defaults to `run`, so adding it cost every other capability nothing.
        let t = Tree::new("silent");
        let mut said = 0;
        let _ = crate::capabilities::files::ListFiles::new(t.root())
            .run_watched(&Arguments::new(), &mut |_| said += 1);
        assert_eq!(said, 0, "listing a folder has nothing to narrate");
    }

    #[test]
    fn nothing_is_disallowed_any_more() {
        // `ALLOWED` is empty and `is_allowed` answers true: one fact, said the same way in both
        // places. A helper still answering `false` for `bash` while the capability happily ran
        // it would be the two-spellings defect, waiting for the first caller to believe it.
        assert!(ALLOWED.is_empty());
        assert!(is_allowed("git"));
        assert!(is_allowed("bash"));
    }
}
