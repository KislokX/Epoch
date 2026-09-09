//! No console window for work nobody asked to watch.
//!
//! Windows gives a console to every child process a GUI application starts, and Epoch starts a
//! great many: MCP servers, agent probes, `nvidia-smi`, a runtime being asked its version. Each
//! one flashes a black window over the World, and the long-lived ones — an MCP server is a
//! process that stays — leave one sitting there.
//!
//! That is an **immersion leak** in the sense `CLAUDE.md` names: it works correctly and it
//! reminds the user they are inside software. Reported 2026-08-23 as *"sometimes two cmd windows
//! open"*, and measured afterwards: `CREATE_NO_WINDOW` appeared nowhere in the tree, so it was
//! every spawn rather than any particular one.
//!
//! ## The two windows that are the point
//!
//! Hidden is the default, not the rule. A terminal Epoch opens **for the user to read** keeps its
//! window and must never call this:
//!
//! - `runtimes::open_in_terminal` — starting a server or an installer in the user's own terminal,
//!   where the output belongs to them (*Epoch opens the door; it never holds the key*).
//! - `agents::claude::open_sign_in` — an agent's own login, in the agent's own window.

use std::process::Command;

/// Stop a child **and everything it started**, where the operating system can be asked.
///
/// **A killed shim is not a stopped program, and its pipes stay open.** On Windows a Node CLI is
/// reached through a `.cmd` shim, so `Child` is the shim and the program is its grandchild —
/// `gemini.cmd` starts `node gemini.js`, which re-executes itself with a larger heap. Ending the
/// shim leaves that grandchild running with the inherited stdout handle, so a reader thread never
/// sees EOF and a `join()` after it waits forever. Measured: a Gemini turn that had already
/// answered, printed its last word and set `done` still never returned, and the orphan was
/// visible in Task Manager holding the model.
///
/// The same fact was found once before, in `capabilities::machine`, about a shell rather than a
/// shim — which is exactly why this lives here now rather than being spelled a second time. *A
/// rule kept by repetition is a rule that has already been broken.*
///
/// **The status is ignored on purpose.** `taskkill /T` walks from the leaves, so by the time it
/// reaches a process whose parent has already gone it reports `There is no running instance of
/// the task` and exits non-zero — a status code is a claim about the transport, not about the
/// work. `kill` below is what settles the direct child.
///
/// **On Unix the group is ended**, when the child was started with [`Quiet::own_group`]. A
/// negative pid is POSIX for *the process group with this id*, and because `process_group(0)`
/// makes the child its own group leader, that id is the child's pid. `/bin/kill` is used rather
/// than `libc::killpg` for one reason worth stating: it needs neither an `unsafe` block nor a
/// dependency, and this program has exactly one `unsafe` in it.
///
/// A child started **without** `own_group` shares its parent's group, and killing that group
/// would end Epoch itself. So the group is only ended when the child leads one — checked by
/// asking the operating system what the child's group actually is, rather than by trusting that
/// whoever wrote the spawn remembered.
pub fn stop_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        use std::process::Stdio;
        let _ = Command::new("taskkill")
            .quiet()
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(unix)]
    {
        use std::process::Stdio;
        let pid = child.id() as i32;
        // Only when the child really is its own group leader. `ps -o pgid=` is asked because the
        // alternative is assuming, and a wrong assumption here kills Epoch's own group.
        let leads = std::process::Command::new("ps")
            .args(["-o", "pgid=", "-p", &pid.to_string()])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()
            .and_then(|said| String::from_utf8(said.stdout).ok())
            .and_then(|said| said.trim().parse::<i32>().ok())
            == Some(pid);
        if leads {
            let _ = std::process::Command::new("kill")
                .args(["-KILL", &format!("-{pid}")])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    let _ = child.kill();
}

/// Run a child process without giving it a console window.
pub trait Quiet {
    /// Spawn with no console. A no-op everywhere but Windows.
    fn quiet(&mut self) -> &mut Self;

    /// Put the child at the head of its **own process group**, so the group can be ended later.
    ///
    /// **This is what makes [`stop_tree`] true on Unix**, and without it that function is a
    /// Windows feature with a comment apologising elsewhere. A shell started here forks the
    /// program somebody actually ran; killing the shell leaves that program with the pipes, and
    /// the second audit named it (2026-09-07, finding 5).
    ///
    /// `process_group` has been in the standard library since 1.64, so no `unsafe` and no
    /// dependency is needed for the half that used to look like it wanted both. A no-op on
    /// Windows, where the tree is asked for by PID instead.
    fn own_group(&mut self) -> &mut Self;
}

impl Quiet for Command {
    #[cfg(windows)]
    fn quiet(&mut self) -> &mut Self {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW`. Named here rather than pulled from `windows-sys` for one constant.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        self.creation_flags(CREATE_NO_WINDOW)
    }

    #[cfg(not(windows))]
    fn quiet(&mut self) -> &mut Self {
        self
    }

    #[cfg(unix)]
    fn own_group(&mut self) -> &mut Self {
        use std::os::unix::process::CommandExt;
        // 0 means "a new group, led by the child" — so the child's pid is also its group id,
        // which is what lets `stop_tree` name the group without asking anybody for it.
        self.process_group(0)
    }

    #[cfg(not(unix))]
    fn own_group(&mut self) -> &mut Self {
        self
    }
}
