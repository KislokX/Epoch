//! Put Epoch's own installer where `main.rs` can embed it.
//!
//! **Setup is one file, and Epoch rides inside it.** That is the whole shape of the thing: a
//! person downloads one program, runs it, and has Epoch — the same as the reference the owner
//! gave. So the MSI the app build produces is copied into `OUT_DIR` and `include_bytes!` takes
//! it from there.
//!
//! **And an absent MSI is not a compile error.** `cargo check` on a fresh clone would fail on a
//! file that only exists after a full release build, which would make the workspace unbuildable
//! for anybody who had not built Epoch first. A placeholder is written instead, and the program
//! refuses at runtime with a sentence saying exactly that: a setup built without Epoch in it.
//! An empty gauge, rather than a compile error nobody can act on.

use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let into = out.join("epoch.msi");

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
        .map(|profile| profile.join("bundle/msi"))
        .and_then(|at| at.canonicalize().ok())
        .and_then(|at| {
            std::fs::read_dir(at).ok()?.flatten().find_map(|entry| {
                let path = entry.path();
                (path.extension().is_some_and(|it| it == "msi")).then_some(path)
            })
        });

    match from {
        Some(msi) => {
            /*
                **Rebuild when the MSI changes, and this line is why.**

                Without it, `cargo` sees no input change and reuses the last build — so setup goes
                on carrying whatever Epoch it embedded the first time. Measured the hard way: a
                setup handed over on the same afternoon still contained a screen that had been
                deleted from the application an hour earlier, because the app had been rebuilt and
                setup had not.

                The README says the order is not optional. This is what makes the tool agree.
            */
            println!("cargo:rerun-if-changed={}", msi.display());
            std::fs::copy(&msi, &into).expect("the installer could be copied");
        }
        None => {
            println!(
                "cargo:warning=No Epoch MSI found. This setup will build and refuse to install \
                 anything. Build the app first: npm run world:build"
            );
            std::fs::write(&into, []).expect("a placeholder could be written");
        }
    }

    tauri_build::build();
}
