//! Does killing a shell actually end what the shell started?
//!
//! ## Why this is a test and not a comment
//!
//! `capabilities::machine` hands its line to a shell, so the child Epoch holds is the shell and
//! the program somebody ran is its child. On Windows `taskkill /T` ends the tree and that was
//! measured a year ago. On Unix nothing did, and the file said so in a comment — which is the
//! shape of claim this project keeps finding to be false. The second audit (2026-09-07,
//! finding 5) named it, and this is the measurement that closes it.
//!
//! ## Why it is `#[ignore]`d
//!
//! It spawns a shell, spawns a grandchild that outlives it, and then asks the operating system
//! whether that grandchild is gone. That is a real process tree on a real machine, and it is
//! Unix-only. Run it deliberately:
//!
//! ```text
//! cargo test -p epoch-models --test a_group_is_ended_on_unix -- --ignored --nocapture
//! ```

#![cfg(unix)]

use epoch_models::quiet::{stop_tree, Quiet};
use std::process::{Command, Stdio};

/// Is a pid still there? `kill -0` asks without sending anything.
fn alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|it| it.success())
        .unwrap_or(false)
}

/// A shell that starts a long sleep in the background and prints its pid, exactly as a build
/// script or a dev server would leave something behind.
fn a_shell_with_a_grandchild(group: bool) -> (std::process::Child, String) {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "sleep 300 & echo $!; wait"])
        .quiet()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if group {
        command.own_group();
    }
    let mut child = command.spawn().expect("a shell");
    let mut pid = String::new();
    {
        use std::io::{BufRead, BufReader};
        let out = child.stdout.take().expect("piped");
        BufReader::new(out).read_line(&mut pid).expect("the pid");
    }
    let pid = pid.trim().to_owned();
    assert!(alive(&pid), "the grandchild started");
    (child, pid)
}

/// **The defect, as the thing that was being done.** Without a group of its own, the shell dies
/// and the program it started keeps running — holding the pipes that `GRACE` exists to stop
/// waiting on, and doing whatever it was doing after the user said stop.
#[test]
#[ignore = "spawns a process tree; Unix only"]
fn without_a_group_the_grandchild_survives() {
    let (mut child, pid) = a_shell_with_a_grandchild(false);
    stop_tree(&mut child);
    let _ = child.wait();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let still = alive(&pid);
    // Cleaned up either way: a test that leaks a five-minute sleep is a test somebody has to
    // clean up after.
    let _ = Command::new("kill").args(["-KILL", &pid]).status();
    assert!(
        still,
        "this is the behaviour the group is for — it survived"
    );
}

/// And with one, the whole group goes.
#[test]
#[ignore = "spawns a process tree; Unix only"]
fn with_a_group_the_grandchild_goes_too() {
    let (mut child, pid) = a_shell_with_a_grandchild(true);
    stop_tree(&mut child);
    let _ = child.wait();
    std::thread::sleep(std::time::Duration::from_millis(300));
    let still = alive(&pid);
    let _ = Command::new("kill").args(["-KILL", &pid]).status();
    assert!(
        !still,
        "the group was ended, so the grandchild went with it"
    );
}

/// **And Epoch is not in the group it ends.** `kill -KILL -<pid>` on a child that shares this
/// process's group would take the test runner with it — and, in production, Epoch. `stop_tree`
/// asks the operating system what the child's group actually is rather than trusting the spawn
/// site to have remembered; this is that guard, seen from outside.
#[test]
#[ignore = "spawns a process tree; Unix only"]
fn a_child_that_shares_our_group_does_not_take_us_with_it() {
    let (mut child, pid) = a_shell_with_a_grandchild(false);
    stop_tree(&mut child);
    let _ = child.wait();
    let _ = Command::new("kill").args(["-KILL", &pid]).status();
    // Reaching this line at all is the assertion: the process running it is still alive.
    assert!(alive(&std::process::id().to_string()), "we are still here");
}
