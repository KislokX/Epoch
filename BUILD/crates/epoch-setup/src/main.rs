//! **Epoch Setup** — the program somebody runs before Epoch exists on their machine.
//!
//! ## What it is, and what it is not
//!
//! It installs Epoch, finds what is already on the machine, offers the rest, and installs what
//! was chosen. It is one file: the app's own installer rides inside this binary, so a person
//! downloads one program rather than a program and a list of instructions.
//!
//! It is **not** the Launcher, and the distinction is the whole reason this crate exists. The
//! Launcher plays with what it has and offers help for the extras — that is what `CONNECTIONS`
//! already does, and a second copy of it inside Epoch would be a deck answering a question the
//! deck next to it already answers. Setup runs **once, before there is a Launcher**.
//!
//! ## The rules it inherits
//!
//! **Offer, never impose.** Nothing is checked, sizes and what each program *gives the machine*
//! are on the row, and declining everything still installs Epoch — which then opens and says what
//! it is missing.
//!
//! **Epoch opens the door and never holds the key.** Nothing here signs in. There is no field on
//! any type in this crate that a credential could travel through, which is a guarantee the
//! compiler keeps rather than one a review does.
//!
//! **A gauge with nothing behind it must read empty.** A setup built without Epoch in it says
//! exactly that and refuses, rather than reporting a successful install of nothing.
//!
//! **Per user, because everything else here is.** Epoch lives in `%LOCALAPPDATA%`, its vault and
//! model library in `%APPDATA%`, and its uninstaller in the user's own registry hive. Setup used
//! to carry the perMachine MSI instead, which asks for a privilege nobody granted — see
//! `build.rs` for what that measured to on a machine that had not been elevated.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Emitter;

/// Epoch's own installer, carried inside this one.
///
/// Empty when this was built before the app was — see `build.rs`. That is a state the program
/// reports rather than a state it pretends out of.
const EPOCH: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/epoch-installer.exe"));

/// One step of the run, as the window draws it.
const STEP: &str = "setup:step";

/// What this machine already has.
#[tauri::command]
async fn survey() -> Result<Vec<epoch_engine::firstrun::Offer>, String> {
    tauri::async_runtime::spawn_blocking(epoch_engine::firstrun::survey)
        .await
        .map_err(|why| why.to_string())
}

/// Whether there is an Epoch in this setup at all.
///
/// Asked by the window before it offers to install anything, so the refusal arrives at the top of
/// the screen rather than as the failure of the first step.
#[tauri::command]
fn carries_epoch() -> bool {
    !EPOCH.is_empty()
}

/// Install Epoch, then the chosen programs, reporting every step as it happens.
///
/// **Epoch first, and unconditionally.** Somebody who declined every extra still asked for Epoch
/// by running this, and a setup that installed nothing because nothing was ticked would be a
/// program that misread its own purpose.
///
/// A step that fails does not stop the rest: four programs asked for and one refusing should
/// leave three installed and one explained.
#[tauri::command]
async fn install(app: tauri::AppHandle, wanted: Vec<String>) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let all = epoch_engine::firstrun::survey();
        let extras = epoch_engine::firstrun::plan(&all, &wanted);
        let total = extras.len() + 1;
        let mut refused: Vec<String> = Vec::new();

        let say = |index: usize,
                   id: &str,
                   name: &str,
                   because: Option<&str>,
                   state: &str,
                   seconds: Option<f64>,
                   line: Option<&str>| {
            let _ = app.emit(
                STEP,
                serde_json::json!({
                    "index": index, "total": total, "id": id, "name": name,
                    "because": because, "state": state, "seconds": seconds, "line": line,
                }),
            );
        };

        // --- Epoch itself ------------------------------------------------------------------
        say(0, "epoch", "Epoch", None, "running", None, None);
        match install_epoch(&|line| say(0, "epoch", "Epoch", None, "running", None, Some(line))) {
            Ok(seconds) => say(0, "epoch", "Epoch", None, "done", Some(seconds), None),
            Err(why) => {
                say(0, "epoch", "Epoch", None, "failed", None, Some(&why));
                refused.push(why);
            }
        }

        // --- and everything that was asked for ---------------------------------------------
        for (at, step) in extras.iter().enumerate() {
            let index = at + 1;
            let name = step.name.as_str();
            let id = step.id.as_str();
            let because = step.because.as_deref();
            say(
                index,
                id,
                name,
                because,
                "running",
                None,
                Some(&step.command),
            );
            match epoch_engine::firstrun::run(step, &|line| {
                say(index, id, name, because, "running", None, Some(line));
            }) {
                Ok(seconds) => say(index, id, name, because, "done", Some(seconds), None),
                Err(why) => {
                    say(index, id, name, because, "failed", None, Some(&why));
                    refused.push(why);
                }
            }
        }
        refused
    })
    .await
    .map_err(|why| why.to_string())
}

/// Where the installer puts Epoch. Per user, and that is the whole point of this file.
///
/// One answer, read by the step that installs and by the button that opens — because two places
/// spelling out the same path is how they come to disagree, and the half nobody is watching is
/// the one that gets it wrong.
fn epoch_lives_at() -> Option<std::path::PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .map(|base| base.join("Epoch").join("epoch-tauri.exe"))
}

/// Write the carried installer somewhere and run it.
///
/// **Quietly, because this window is already reporting it.** `/S` is NSIS' silent switch, so
/// there is no second progress bar in front of the one the person is watching.
///
/// ## A silent installer says nothing, so the side effect is what is read
///
/// The MSI this used to carry wrote a verbose log and the last lines of it were reported here,
/// which was the honest thing to do with a program that explains itself in a file. NSIS has no
/// such file — it succeeds or it does not — so the check is the thing itself: is Epoch on the
/// disk where the installer puts it?
///
/// That is this project's own rule about a success code being a claim about the transport rather
/// than about the work, and it is worth more here than the log ever was.
///
/// **The exit code is still read first, and the order matters.** On a machine that already has
/// Epoch, a *failed* run would leave the previous install sitting exactly where this looks — so
/// treating "the file is there" as success on its own would report a reinstall that did nothing
/// as a reinstall that worked. The code has to agree before the disk is believed.
fn install_epoch(say: &dyn Fn(&str)) -> Result<f64, String> {
    if EPOCH.is_empty() {
        return Err(
            "This setup was built without Epoch inside it, so there is nothing to install. \
             Build the app first — `npm run world:build` — and build setup again."
                .to_owned(),
        );
    }
    if !cfg!(target_os = "windows") {
        return Err(
            "This setup installs Epoch from a Windows installer. On macOS the app is the bundle \
             from `target/release/bundle`."
                .to_owned(),
        );
    }

    let began = std::time::Instant::now();
    let here = std::env::temp_dir().join("epoch-setup");
    std::fs::create_dir_all(&here).map_err(|why| format!("nowhere to unpack Epoch: {why}"))?;
    let installer = here.join("Epoch-Setup.exe");
    std::fs::write(&installer, EPOCH)
        .map_err(|why| format!("Epoch could not be unpacked: {why}"))?;
    say(&format!("Unpacked Epoch to {}", installer.display()));

    let said = std::process::Command::new(&installer)
        .arg("/S")
        .status()
        .map_err(|why| format!("Epoch's installer could not be started: {why}"))?;

    if !said.success() {
        return Err(format!(
            "Epoch's installer stopped with {said} and nothing was installed."
        ));
    }

    let Some(at) = epoch_lives_at() else {
        return Err(
            "This machine does not say where a user's programs live (%LOCALAPPDATA% is \
                    unset), so there is nowhere to check whether Epoch arrived."
                .to_owned(),
        );
    };
    if !at.is_file() {
        return Err(format!(
            "The installer finished without complaint and Epoch is not at {}. Nothing was \
             installed, whatever the exit code said.",
            at.display()
        ));
    }

    say(&format!("Epoch is at {}", at.display()));
    Ok(began.elapsed().as_secs_f64())
}

/// Open Epoch, and close this.
///
/// **Only where it was actually installed.** A launcher button that reports success by opening
/// nothing is the worst kind of instrument, so this reads the disk rather than the exit code it
/// saw a minute ago.
///
/// **And *close this* is the code now rather than only the sentence.** Measured by pressing the
/// button: Epoch opened and setup stayed behind it, because the frontend calls this and returns.
/// The doc had said so since it was written, which made it a promise with nothing keeping it —
/// a comment that summarises code is a claim with no test attached, and this one was already
/// false. Setup has one job and it is finished the moment Epoch is on screen.
///
/// **Only on success.** A refusal has to stay on screen to be read; exiting either way would
/// close the window over the one sentence explaining why nothing happened.
#[tauri::command]
fn open_epoch(app: tauri::AppHandle) -> Result<(), String> {
    let at = epoch_lives_at()
        .filter(|it| it.is_file())
        .ok_or("Epoch is not where the installer puts it. Open it from the Start menu.")?;
    std::process::Command::new(at)
        .spawn()
        .map_err(|why| format!("Epoch would not open: {why}"))?;
    app.exit(0);
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            survey,
            carries_epoch,
            install,
            open_epoch
        ])
        .run(tauri::generate_context!())
        .expect("the setup window could not be opened");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **What is carried is an installer, or it is nothing at all.**
    ///
    /// `carries_epoch` asks whether the blob is empty, which is the right question at runtime and
    /// a weak one at build time: a `build.rs` that found the wrong file would answer *yes* and
    /// this setup would go out carrying something that is not an Epoch.
    ///
    /// A Windows executable begins `MZ`. Cheap, exact, and it is the shape rather than the name —
    /// the file this is copied from is chosen by extension, which is a claim (ADR-0024).
    ///
    /// **It used to assert the OLE compound header, because what was carried used to be the MSI.**
    /// That artefact installs perMachine and could not be installed by an ordinary user at all;
    /// the header check was correct about the file and the file was the wrong one. A test that
    /// pins the format is only as good as the decision about which format belongs here, which is
    /// why the reason lives in `build.rs` beside the change rather than in this assertion.
    ///
    /// **Empty is a real state and is not a failure.** A fresh clone has no installer until the
    /// app has been built once, and `build.rs` writes a placeholder on purpose so the workspace
    /// still compiles. It says so out loud rather than passing quietly, because a test that
    /// reports nothing and a test that measured nothing look identical in a green run.
    #[test]
    fn what_is_carried_is_an_installer_or_nothing_at_all() {
        const EXECUTABLE: &[u8; 2] = b"MZ";

        if EPOCH.is_empty() {
            eprintln!(
                "this setup carries no Epoch, which is what a build before `npm run world:build` \
                 produces — nothing to check here"
            );
            return;
        }
        assert!(
            EPOCH.starts_with(EXECUTABLE),
            "carried {} bytes that do not begin like a Windows installer",
            EPOCH.len()
        );
    }

    /// **Per user, and the compiler is not what keeps that true — this is.**
    ///
    /// Both the install step and the OPEN button read `epoch_lives_at`, so the one thing that
    /// could quietly revert this change is somebody restoring a `ProgramFiles` path in either of
    /// them. That is exactly the failure that shipped: a setup looking in `Program Files` for a
    /// program every other part of Epoch installs into `%LOCALAPPDATA%`.
    #[test]
    fn epoch_is_looked_for_where_a_user_may_actually_install_it() {
        let Some(at) = epoch_lives_at() else {
            eprintln!("%LOCALAPPDATA% is unset on this machine; nothing to check");
            return;
        };
        let said = at.display().to_string();
        assert!(
            said.ends_with("Epoch\\epoch-tauri.exe") || said.ends_with("Epoch/epoch-tauri.exe"),
            "the installed program is not where the installer puts it: {said}"
        );
        assert!(
            !said.contains("Program Files"),
            "a per-user install must not be looked for in Program Files: {said}"
        );
    }
}
