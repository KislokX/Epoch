//! Where a *running* Epoch would look, on this machine, right now.
//!
//! An integration test rather than a unit one: it runs from the same `target/` directory the app
//! does, so it measures the real answer instead of a constructed one. It exists because the
//! failure it guards against is silent — an app pointed at the wrong vault shows an empty
//! Launcher, which reads as "you have no crew" rather than as "I am looking in the wrong place".

use epoch_engine::paths::Paths;

#[test]
fn a_test_binary_finds_the_tree_it_was_built_in() {
    // `EPOCH_DATA` is a legitimate override and moving the vault is exactly what it is for, so a
    // machine that has one set is not the case this measures. Skipped and said, rather than
    // failing: a red test whose cause is an environment variable teaches people to ignore it.
    //
    // Found by setting it — while checking that a clone with no vault still passes, which it
    // does, because nothing here requires the vault to *exist*.
    if let Some(said) = std::env::var_os("EPOCH_DATA") {
        eprintln!(
            "skipped: EPOCH_DATA is set to {}, so this is not a plain source tree",
            said.to_string_lossy()
        );
        return;
    }

    let paths = Paths::discover();

    // Both must land in the source tree, because a test binary lives in `target/` exactly as a
    // development build does.
    assert!(
        paths.packs().is_dir(),
        "packs must exist where Epoch will look: {}",
        paths.packs().display()
    );
    // **A World Pack ships, rather than a named one.** This asked for `default`, which stopped
    // shipping on 2026-09-08 — and the property it was guarding was never about that pack: it is
    // that a running Epoch finds *something* to open. Read off the directory, so the day a second
    // World ships this needs no edit and the day none does it still fails.
    let shipped: Vec<String> = std::fs::read_dir(paths.packs())
        .expect("the packs directory is readable")
        .flatten()
        .filter(|entry| entry.path().join("pack.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        !shipped.is_empty(),
        "no World Pack ships with the product; {} holds {:?}",
        paths.packs().display(),
        shipped
    );

    // And the vault stays with it, which is what keeps a developer's crew from vanishing.
    assert_eq!(
        paths.vault().parent(),
        paths.packs().parent(),
        "in a source tree, shipped and data share one root"
    );
}
