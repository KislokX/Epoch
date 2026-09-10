//! Where Epoch's three kinds of thing live.
//!
//! ## The bug this file exists to fix
//!
//! Every path in the product resolved from `CARGO_MANIFEST_DIR` — a constant baked in at compile
//! time, pointing at the source tree of whichever machine built the binary. That works perfectly
//! in development and cannot work at all once installed: the app would look for Worlds inside a
//! folder that does not exist on the user's computer.
//!
//! It also made the repository mean the wrong thing. Shipped content and one developer's own crew
//! sat in the same folder, so "what a user installs" and "what this machine happens to contain"
//! were indistinguishable.
//!
//! ## Three, not one
//!
//! | | Holds | Written by | Where |
//! |---|---|---|---|
//! | **shipped** | the World Packs that came with Epoch | the installer, never the app | beside the binary |
//! | **data** | the vault: crew, Worlds, Quests, settings, secrets | the app | `%APPDATA%\Epoch` |
//! | project roots | the user's own codebases | the user | anywhere — chosen, never guessed |
//!
//! Project roots are deliberately absent from this type. They are not a location Epoch knows;
//! they are an answer the user gave, stored per World (ADR-0025), and putting them here would
//! invite something to guess one.
//!
//! ## Worlds live in both, and which one says who made it
//!
//! A World the installer ships is in `shipped`; a World somebody makes is in `data`, at
//! `vault/worlds/<id>/`, beside its own map and Quests. Until 2026-09-10 the table above said
//! so and the code did not: `create` wrote into `shipped`, the installer's folder, and a World
//! made in an installed Epoch was left behind by its uninstall. See `relocate`.
//!
//! ## Resolved once
//!
//! A free function computing a path on every call is how a program ends up with two answers to
//! one question. [`Paths::discover`] runs at startup; everything downstream is handed the result.
//!
//! ## Development keeps working
//!
//! When the binary sits inside a `target/` directory, the source tree is right there and is the
//! right answer — `packs/` and `assets/` are checked in, and a developer editing a pack expects
//! the running app to see it. So `shipped` walks up from the executable looking for the marker
//! that says "this is the tree", and only falls back to *beside the binary* when there is none.
//!
//! **`data` follows it.** A binary running out of the source tree keeps its vault there too, so a
//! developer's crew, Worlds and Quests stay exactly where they already are — switching that to
//! `%APPDATA%` would have made every character they had disappear on the next launch, which is
//! not a migration, it is a loss.
//!
//! `EPOCH_DATA` overrides both, and is how a test gets a vault of its own instead of writing into
//! somebody's real one.

use std::path::{Path, PathBuf};

/// The marker that identifies the source tree.
///
/// A file that only exists there, rather than a directory name: `BUILD` could be anywhere, and
/// matching on a name would make a user's folder called `BUILD` into Epoch's install.
const MARKER: &str = "Cargo.toml";

/// Where this machine keeps the two kinds of Epoch folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Read-only, installed with the binary. Packs and the asset kit.
    shipped: PathBuf,
    /// Read-write, this user's own. The vault.
    data: PathBuf,
}

impl Paths {
    /// Work out where everything is, once.
    pub fn discover() -> Self {
        let tree = std::env::current_exe()
            .ok()
            .as_deref()
            .and_then(source_tree);
        Self {
            shipped: tree.clone().unwrap_or_else(installed_beside),
            // In the source tree, the vault stays in the source tree. An installed build has no
            // tree, so it uses the platform's data directory — and an explicit `EPOCH_DATA`
            // outranks both, which is how a test avoids the developer's real crew.
            data: match (std::env::var_os("EPOCH_DATA"), tree) {
                (Some(said), _) => PathBuf::from(said),
                (None, Some(tree)) => tree,
                (None, None) => data(),
            },
        }
    }

    /// State them explicitly. For tests, and for anything that must not touch a real machine.
    pub fn at(shipped: impl Into<PathBuf>, data: impl Into<PathBuf>) -> Self {
        Self {
            shipped: shipped.into(),
            data: data.into(),
        }
    }

    /// Installed Worlds — the packs that came with Epoch.
    pub fn packs(&self) -> PathBuf {
        self.shipped.join("packs")
    }

    /// Worlds this user made: `vault/worlds/`, one folder each, beside the map and the Quests.
    ///
    /// Never `shipped`. The installer owns that folder and replaces it on every update; a World
    /// somebody made there lasted only as long as nothing tidied it.
    pub fn worlds(&self) -> PathBuf {
        self.vault().join("worlds")
    }

    /// Where shipped artwork would go, if anything shipped there.
    ///
    /// **Nothing does, and the folder is gone** (owner, 2026-09-07). `assets/kit` was 4.8 MB of
    /// four PNGs and a README riding in every installer, and the only caller of this function
    /// was the test below asserting where it would be — measured before removing it, not
    /// assumed. A pack's own artwork lives inside that pack (`assets/preview.png` is relative to
    /// the pack), which is what `packs/archipelago` already does and what a World Pack is *for*.
    ///
    /// The function stays and the directory does not. The path is still the right answer to the
    /// question, and inventing it again the day something ships there would be the same work
    /// done twice.
    pub fn assets(&self) -> PathBuf {
        self.shipped.join("assets")
    }

    /// Everything the user owns: crew, Worlds, Quests, settings, secrets.
    pub fn vault(&self) -> PathBuf {
        self.data.join("vault")
    }

    /// The user's crew.
    pub fn definitions(&self) -> PathBuf {
        self.vault().join("definitions")
    }

    /// Where a crash can leave something the user could send (Phase 6.3).
    pub fn logs(&self) -> PathBuf {
        self.data.join("logs")
    }

    /// Make the data directory exist.
    ///
    /// **Lazily, and only what is asked for.** An empty tree created eagerly at first run is a
    /// promise that something is configured; the honest first launch has a vault that appears the
    /// moment somebody makes their first character.
    ///
    /// Reports rather than panics: a read-only or full disk is the user's situation to be told
    /// about, not a reason for the World to fail to open.
    pub fn ensure(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)
    }
}

/// Where on the `PATH` a file of this name is, if anywhere.
///
/// Shared, because two agents now need it and a second copy is a second answer. Somebody who put
/// a program on their `PATH` meant *that* one, so this is always consulted before any well-known
/// install location.
pub fn on_path(name: &str) -> Option<PathBuf> {
    on_path_matching(name, |_| true)
}

/// Where on the `PATH` an acceptable file of this name is, if anywhere.
///
/// A file being present only says the filesystem can see it. A caller that knows a particular
/// wrapper cannot be launched directly can keep looking rather than mistaking that wrapper for
/// the user's intended program.
pub fn on_path_matching(name: &str, accepts: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file() && accepts(candidate))
}

/// Where an installed build keeps what shipped with it: beside the binary.
fn installed_beside() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        // Nothing could be measured. The current directory is a guess, and a wrong one shows up
        // immediately as an empty World rather than as something subtly misplaced.
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Walk up from the executable looking for the workspace that contains it.
///
/// `target/debug/epoch-tauri.exe` is three levels below `BUILD/Cargo.toml`, and a workspace
/// nested deeper is possible, so this searches rather than counting.
fn source_tree(exe: &Path) -> Option<PathBuf> {
    let mut at = exe.parent()?;
    // Only inside a `target` directory: a released binary must never adopt a stray `Cargo.toml`
    // that happens to sit above wherever the user installed it.
    if !at.components().any(|c| c.as_os_str() == "target") {
        return None;
    }
    loop {
        if at.join(MARKER).is_file() && at.join("packs").is_dir() {
            return Some(at.to_path_buf());
        }
        at = at.parent()?;
    }
}

/// `%APPDATA%\Epoch`, or wherever this platform keeps a user's application data.
fn data() -> PathBuf {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);

    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join("Library/Application Support"));

    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));

    base.unwrap_or_else(|| PathBuf::from("."))
        .join(if cfg!(windows) { "Epoch" } else { "epoch" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_kinds_of_folder_stay_apart() {
        // The whole point: shipped content and the user's own things are not the same place, and
        // nothing can accidentally write into the first.
        let paths = Paths::at(
            "C:/Program Files/Epoch",
            "C:/Users/somebody/AppData/Roaming/Epoch",
        );

        assert!(paths.packs().starts_with("C:/Program Files/Epoch"));
        assert!(paths.assets().starts_with("C:/Program Files/Epoch"));
        assert!(paths.vault().starts_with("C:/Users/somebody"));
        assert!(
            paths.worlds().starts_with("C:/Users/somebody"),
            "a World somebody made is theirs, not the installer's"
        );
        assert!(paths.definitions().starts_with(paths.vault()));
        assert!(paths.logs().starts_with("C:/Users/somebody"));
    }

    #[test]
    fn an_installed_binary_never_adopts_a_stray_workspace() {
        // A `Cargo.toml` sitting above where somebody installed Epoch is not Epoch's source tree,
        // and treating it as one would make the app read Worlds out of a stranger's repository.
        let installed = Path::new("C:/Users/somebody/AppData/Local/Programs/Epoch/epoch.exe");
        assert_eq!(source_tree(installed), None);
    }

    #[test]
    fn a_development_binary_looks_for_the_tree_it_was_built_in() {
        // Only the `target` requirement is asserted here — whether the marker exists is a fact
        // about a real filesystem, and this test does not touch one.
        let built = Path::new("/somewhere/BUILD/target/debug/epoch-tauri.exe");
        // It searches (and finds nothing, because `/somewhere` does not exist), rather than
        // refusing outright the way it does for an installed path.
        assert_eq!(source_tree(built), None);
    }

    /// The development fallback, against a real filesystem.
    ///
    /// Asserted rather than reasoned about, because getting it wrong means a developer's running
    /// app stops seeing the packs they are editing — and the symptom is an empty World, which
    /// looks like a content problem rather than a path one.
    #[test]
    fn a_binary_inside_a_target_directory_finds_the_tree_around_it() {
        let root = std::env::temp_dir().join("epoch-paths-test");
        let deep = root.join("target/debug");
        std::fs::create_dir_all(&deep).expect("temp dirs");
        std::fs::create_dir_all(root.join("packs")).expect("temp dirs");
        std::fs::write(
            root.join(MARKER),
            "[workspace]
",
        )
        .expect("temp marker");

        let found = source_tree(&deep.join("epoch-tauri.exe"));
        assert_eq!(found.as_deref(), Some(root.as_path()));

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A tree with a manifest but no packs is somebody else's workspace.
    #[test]
    fn a_workspace_without_packs_is_not_epochs_tree() {
        let root = std::env::temp_dir().join("epoch-paths-stranger");
        let deep = root.join("target/debug");
        std::fs::create_dir_all(&deep).expect("temp dirs");
        std::fs::write(
            root.join(MARKER),
            "[workspace]
",
        )
        .expect("temp marker");

        assert_eq!(source_tree(&deep.join("epoch-tauri.exe")), None);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_explicit_data_directory_wins() {
        // How a test gets a vault of its own. Without it, a test run writes into the developer's
        // real crew, and losing somebody's characters to a test is not a recoverable mistake.
        temp_env("EPOCH_DATA", "C:/tmp/epoch-test", || {
            assert_eq!(
                Paths::discover().vault(),
                PathBuf::from("C:/tmp/epoch-test/vault")
            );
        });
    }

    /// Running from the source tree must not move anybody's crew.
    #[test]
    fn a_development_build_keeps_its_vault_where_it_already_is() {
        // The whole point of the fallback. Pointing `data` at `%APPDATA%` while `shipped` stayed
        // in the tree would have emptied the Launcher on the next launch — every character, every
        // World, every Quest, apparently gone. They would still be on disk, which makes it worse:
        // the product would be lying about what exists.
        let paths = Paths::discover();
        if paths
            .packs()
            .starts_with(paths.vault().parent().expect("a vault has a parent"))
        {
            assert_eq!(
                paths.vault().parent(),
                Some(paths.packs().parent().expect("packs has a parent")),
                "in a source tree, shipped and data are the same root"
            );
        }
    }

    /// Set one variable for the duration of a closure, then put it back.
    fn temp_env(key: &str, value: &str, run: impl FnOnce()) {
        let before = std::env::var_os(key);
        // SAFETY: single-threaded within this test; restored before returning.
        unsafe { std::env::set_var(key, value) };
        run();
        match before {
            Some(old) => unsafe { std::env::set_var(key, old) },
            None => unsafe { std::env::remove_var(key) },
        }
    }
}
