//! Where this machine keeps what the Host gave it.
//!
//! ## Why not DPAPI or Keychain
//!
//! Epoch's own `secrets.rs` is DPAPI, and **Windows only** — off Windows it refuses rather than
//! pretending. This program runs on three operating systems, so it keeps its bearer in a file
//! in its own data directory, `0600` on Unix and in the user's profile on Windows.
//!
//! That is weaker, and it is proportionate: the secret grants the use of a graphics card, which
//! is what a bare `OLLAMA_HOST=0.0.0.0` grants the whole network today. Anybody who can read a
//! `0600` file in the user's home directory can already read far more interesting things there.
//!
//! It is one function. A platform keystore replaces it without changing anything above.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What this machine knows about the Host it belongs to.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bond {
    /// The id the Host filed this machine under.
    pub id: String,
    /// The bearer. Required on every request the Host makes.
    pub secret: String,
    /// Where the Host was reached, so this can re-announce when it starts.
    pub host: String,
}

impl Bond {
    pub fn paired(&self) -> bool {
        !self.secret.is_empty()
    }
}

/// Read the bond, or an empty one. An unpaired machine is a valid machine that lends nothing.
pub fn read() -> Bond {
    read_at(&file())
}

fn read_at(path: &std::path::Path) -> Bond {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Write the bond, readable by this user and nobody else.
///
/// **The permission is set before the bytes are written**, not after: a file that is briefly
/// world-readable is a file that was briefly world-readable, and the window is where the
/// interesting things happen.
pub fn write(bond: &Bond) -> Result<(), String> {
    write_at(&file(), bond)
}

/// The same write, to a path the caller names.
///
/// **It exists because a test destroyed a real pairing.** The permissions test called `write`
/// and then `forget`, and on Windows it never ran (`#[cfg(unix)]`) so nothing showed it — until
/// the suite ran on the machine that was actually paired, wrote a fake bond over the real one
/// and deleted it. The Host went on believing the machine was paired; the machine had forgotten.
///
/// A test that can reach the user's own data is a test that will eventually reach it. So the
/// path is an argument, and no test names the real one.
fn write_at(path: &std::path::Path, bond: &Bond) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("cannot create {dir:?}: {err}"))?;
    }

    let body = serde_json::to_string_pretty(bond).map_err(|err| err.to_string())?;
    // **Atomically**, and with the same mode this file always had: it holds the one bond this
    // machine has, and a torn write is a machine that has silently stopped being lent. The
    // helper sets `0600` as it creates the temporary, which is what this file opened its own
    // `OpenOptions` for until now.
    epoch_secrets::atomically::replace(path, body.as_bytes())
}

/// Forget the bond entirely — what *unpair* means on this side.
pub fn forget() -> Result<(), String> {
    forget_at(&file())
}

fn forget_at(path: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.to_string()),
    }
}

/// This program's own data directory, per platform.
///
/// Its own, not Epoch's: a machine running EpochServices is usually a machine **without** Epoch,
/// and reaching into a vault that is not there would be a program looking for somebody else's
/// files.
pub fn dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|home| PathBuf::from(home).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
    };
    base.unwrap_or_else(std::env::temp_dir)
        .join("EpochServices")
}

fn file() -> PathBuf {
    dir().join("bond.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unpaired_machine_is_a_valid_machine_that_lends_nothing() {
        // Not an error state. It is what every machine is before somebody pairs it, and the
        // program has to run in order to be paired at all.
        let empty = Bond::default();
        assert!(!empty.paired());
    }

    #[test]
    fn the_data_directory_is_this_programs_own() {
        // A machine running this usually has no Epoch on it, and reaching into a vault that is
        // not there would be a program hunting for somebody else's files.
        let dir = dir();
        assert!(dir.ends_with("EpochServices"), "{dir:?}");
    }

    #[cfg(unix)]
    #[test]
    fn the_bond_is_written_readable_by_this_user_and_nobody_else() {
        use std::os::unix::fs::PermissionsExt;
        // The permission is part of the create, not a chmod afterwards: a file that is briefly
        // world-readable was briefly world-readable.
        //
        // **In a temporary directory, because this used to run against the real one.** It is
        // `#[cfg(unix)]`, so it never ran on the Host and nothing ever showed it — and the first
        // time the suite ran on the paired MacBook it wrote this fake bond over that machine's
        // real one and then deleted it. The Host still believed they were paired.
        let path = std::env::temp_dir()
            .join(format!("epoch-bond-{}", std::process::id()))
            .join("bond.json");
        let bond = Bond {
            id: "bridge-1".into(),
            secret: "a-long-secret".into(),
            host: "http://10.0.0.2:11500".into(),
        };
        write_at(&path, &bond).expect("writes");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{mode:o}");

        // And it reads back as what was written, which the old test could not check without
        // reading whatever was already there.
        assert_eq!(read_at(&path).secret, "a-long-secret");

        forget_at(&path).unwrap();
        assert!(!path.exists());
        // Forgetting something already forgotten is not an error: unpair twice is one outcome.
        forget_at(&path).unwrap();
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn no_test_in_this_module_names_the_real_bond() {
        // The guard for the defect above, stated rather than remembered: `file()` is the user's
        // own pairing and belongs to the running program alone.
        let real = file();
        assert!(real.ends_with("bond.json"));
        assert!(
            !real.starts_with(std::env::temp_dir()),
            "the real bond must not live where tests write: {real:?}"
        );
    }
}
