//! **Epoch Setup** — the program somebody runs before Epoch exists on their machine.
//!
//! ## What it is, and what it is not
//!
//! It installs Epoch, finds what is already on the machine, offers the rest, and installs what
//! was chosen. It is one file: the app's own MSI rides inside this binary, so a person downloads
//! one program rather than a program and a list of instructions.
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
//! **A gauge with nothing behind it must read empty.** A setup built without the MSI in it says
//! exactly that and refuses, rather than reporting a successful install of nothing.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Emitter;

/// Epoch's own installer, carried inside this one.
///
/// Empty when this was built before the app was — see `build.rs`. That is a state the program
/// reports rather than a state it pretends out of.
const EPOCH: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/epoch.msi"));

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

/// Write the carried MSI somewhere and let Windows install it.
///
/// **Quietly, because this window is already reporting it.** `/qn` gives no second progress bar in
/// front of the one the person is watching; `/norestart` because deciding to reboot somebody's
/// machine is not a thing an installer does without asking.
///
/// The log goes to a file `msiexec` writes and the lines are read back afterwards: `msiexec`
/// returns immediately and says nothing to a pipe, so a runner that read its output would report
/// a successful install having watched nothing at all.
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
            "This setup installs Epoch from an MSI, which is Windows' own format. On macOS the \
             app is the bundle from `target/release/bundle`."
                .to_owned(),
        );
    }

    let began = std::time::Instant::now();
    let here = std::env::temp_dir().join("epoch-setup");
    std::fs::create_dir_all(&here).map_err(|why| format!("nowhere to unpack Epoch: {why}"))?;
    let msi = here.join("Epoch.msi");
    let log = here.join("install.log");
    std::fs::write(&msi, EPOCH).map_err(|why| format!("Epoch could not be unpacked: {why}"))?;
    say(&format!("Unpacked Epoch to {}", msi.display()));

    let said = std::process::Command::new("msiexec")
        .arg("/i")
        .arg(&msi)
        .args(["/qn", "/norestart", "/l*v"])
        .arg(&log)
        .status()
        .map_err(|why| format!("Windows Installer could not be started: {why}"))?;

    // Its own words, in its own log. The last lines are where a failure explains itself, and a
    // sentence written here would be Epoch guessing about Windows' refusal.
    if let Ok(written) = std::fs::read_to_string(&log) {
        for line in written
            .lines()
            .rev()
            .take(12)
            .collect::<Vec<_>>()
            .iter()
            .rev()
        {
            let line = line.trim();
            if !line.is_empty() {
                say(line);
            }
        }
    }

    if said.success() {
        return Ok(began.elapsed().as_secs_f64());
    }
    Err(format!(
        "Windows Installer stopped with {said}. The whole log is at {}.",
        log.display()
    ))
}

/// Open Epoch, and close this.
///
/// **Only where it was actually installed.** A launcher button that reports success by opening
/// nothing is the worst kind of instrument, so this reads the disk rather than the exit code it
/// saw a minute ago.
#[tauri::command]
fn open_epoch() -> Result<(), String> {
    let at = std::env::var_os("ProgramFiles")
        .map(std::path::PathBuf::from)
        .map(|base| base.join("Epoch").join("epoch-tauri.exe"))
        .filter(|it| it.is_file())
        .ok_or("Epoch is not where the installer puts it. Open it from the Start menu.")?;
    std::process::Command::new(at)
        .spawn()
        .map(|_| ())
        .map_err(|why| format!("Epoch would not open: {why}"))
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
    /// An MSI is an OLE compound file and begins with a fixed eight bytes. Cheap, exact, and it
    /// is the shape rather than the name — the file this is copied from is chosen by extension,
    /// which is a claim (ADR-0024).
    ///
    /// **Empty is a real state and is not a failure.** A fresh clone has no MSI until the app has
    /// been built once, and `build.rs` writes a placeholder on purpose so the workspace still
    /// compiles. It says so out loud rather than passing quietly, because a test that reports
    /// nothing and a test that measured nothing look identical in a green run.
    #[test]
    fn what_is_carried_is_an_installer_or_nothing_at_all() {
        const COMPOUND_FILE: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

        if EPOCH.is_empty() {
            eprintln!(
                "this setup carries no Epoch, which is what a build before `npm run world:build` \
                 produces — nothing to check here"
            );
            return;
        }
        assert!(
            EPOCH.starts_with(&COMPOUND_FILE),
            "carried {} bytes that do not begin like an installer",
            EPOCH.len()
        );
    }
}
