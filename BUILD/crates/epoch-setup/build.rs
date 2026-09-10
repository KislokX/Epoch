//! Put Epoch's own installer where `main.rs` can embed it.
//!
//! **Setup is one file, and Epoch rides inside it.** That is the whole shape of the thing: a
//! person downloads one program, runs it, and has Epoch — the same as the reference the owner
//! gave. So the installer the app build produces is copied into `OUT_DIR` and `include_bytes!`
//! takes it from there.
//!
//! ## It carried the MSI, and the MSI is the one artefact that cannot be installed
//!
//! Measured 2026-09-09 by downloading the v0.1.0 draft and double-clicking it the way the README
//! tells somebody to. Windows' own words:
//!
//! ```text
//! Error 1925. You do not have sufficient privileges to complete this installation for all
//! users of the machine.
//! MSI_LUA: Installation UI level is silent, no credential elevation is possible
//! ```
//!
//! The MSI is **perMachine** — it installs into `Program Files` — while every other thing Epoch
//! does is per user: `%LOCALAPPDATA%` for the program, `%APPDATA%` for the vault and the model
//! library, an uninstaller in the user's own registry hive. It is why the release deliberately
//! does not publish the MSI at all. Setup embedding it was the one place that disagreed, and
//! running it `/qn` meant not even a UAC prompt could appear: silent plus unelevated is a
//! refusal with nowhere to ask.
//!
//! So setup now carries the **NSIS installer**, which is the one the README describes and the one
//! a person downloading the plain file already gets. Nothing about the shape changes — one file,
//! Epoch inside it — and the install stops requiring a privilege nobody was asked for.
//!
//! **And an absent installer is not a compile error.** `cargo check` on a fresh clone would fail
//! on a file that only exists after a full release build, which would make the workspace
//! unbuildable for anybody who had not built Epoch first. A placeholder is written instead, and
//! the program refuses at runtime with a sentence saying exactly that: a setup built without
//! Epoch in it. An empty gauge, rather than a compile error nobody can act on.

use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let into = out.join("epoch-installer.exe");

    // **Found from `OUT_DIR` rather than from the source tree**, because the bundle is wherever
    // *this* build is writing and that is not always `BUILD/target`. A path spelled relative to
    // the crate is right until somebody sets `CARGO_TARGET_DIR` — which is the ordinary way to
    // build the installer on a machine where a shell is sitting in `target/release` — and then
    // it silently finds nothing and produces a setup that refuses to install anything.
    //
    // `OUT_DIR` is `<target>/<profile>/build/<crate>-<hash>/out`, so three levels up is the
    // profile directory the bundle sits beside. One source for the answer instead of two.
    let from = out
        .ancestors()
        .nth(3)
        .map(|profile| profile.join("bundle/nsis"))
        .and_then(|at| at.canonicalize().ok())
        .and_then(|at| {
            std::fs::read_dir(at).ok()?.flatten().find_map(|entry| {
                let path = entry.path();
                (path.extension().is_some_and(|it| it == "exe")).then_some(path)
            })
        });

    match from {
        Some(installer) => {
            /*
                **Rebuild when the installer changes, and this line is why.**

                Without it, `cargo` sees no input change and reuses the last build — so setup goes
                on carrying whatever Epoch it embedded the first time. Measured the hard way: a
                setup handed over on the same afternoon still contained a screen that had been
                deleted from the application an hour earlier, because the app had been rebuilt and
                setup had not.

                The README says the order is not optional. This is what makes the tool agree.
            */
            println!("cargo:rerun-if-changed={}", installer.display());
            std::fs::copy(&installer, &into).expect("the installer could be copied");
        }
        None => {
            println!(
                "cargo:warning=No Epoch installer found in bundle/nsis. This setup will build \
                 and refuse to install anything. Build the app first: npm run world:build"
            );
            std::fs::write(&into, []).expect("a placeholder could be written");
        }
    }

    tauri_build::build();
}
