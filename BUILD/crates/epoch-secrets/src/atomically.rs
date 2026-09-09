//! Writing a file that must never be found half-written.
//!
//! ## Why this is here rather than anywhere else
//!
//! `fs::write` truncates the file and then writes into it. Between those two moments the old
//! contents are gone and the new ones have not arrived, and a machine that loses power, runs out
//! of disk or is killed in that window leaves an empty or truncated file behind.
//!
//! For most of the ~200 files Epoch writes that is a nuisance: a cache is rebuilt, a page is
//! redrawn, a snapshot is discarded (ADR-0018 says a snapshot is always disposable). For a
//! handful it is a loss of something that **cannot be re-derived from anything**:
//!
//! | file | what a torn write costs |
//! |---|---|
//! | `secrets.dat` | every provider key, every agent door token, every machine's bearer |
//! | `bridges.toml` | every pairing, including the fingerprints that make them usable |
//! | `settings.toml` | what the user chose about their own machine |
//! | `bond.json` | the one bond a lent machine has |
//!
//! So this lives beside the credential store, because it answers the same question the rest of
//! this crate answers: *where does something go that nobody can get back?*
//!
//! ## What it does
//!
//! Write a temporary file in the same folder, flush it to the disk, then **rename it over** the
//! real one. A rename within one directory is the closest thing a filesystem offers to a single
//! step: after it, readers see either the old file or the new one.
//!
//! The temporary is in the same folder deliberately — a rename across drives is a copy, and a
//! copy is the thing being avoided.
//!
//! ## Windows renames differently, and that difference is the whole reason this file exists
//!
//! `std::fs::rename` refuses when the destination already exists on Windows, so the obvious
//! spelling would work on macOS and Linux and fail on the platform Epoch ships on — or, worse,
//! be "fixed" by deleting the destination first, which re-opens exactly the window this closes.
//! `MoveFileExW` with `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH` is Windows' answer and
//! it is what is called here.

use std::path::Path;

/// Write `bytes` to `path`, so that a reader sees either the old file or this one.
///
/// The temporary is left behind if the rename fails, and that is deliberate: it is the only copy
/// of the new content at that moment, and deleting it would turn a failed write into a lost one.
/// Its name says what it is.
pub fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .filter(|it| !it.as_os_str().is_empty())
        .ok_or_else(|| format!("{} has no folder to write into", path.display()))?;
    std::fs::create_dir_all(parent).map_err(|why| format!("{}: {why}", parent.display()))?;

    let name = path
        .file_name()
        .map(|it| it.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".to_owned());
    // Process id and a clock reading, because two Epochs on one machine writing one vault is a
    // situation this must not turn into a corrupt file. They would still race on the rename, and
    // a rename is the one step where racing is safe: one of them wins whole.
    let temp = parent.join(format!(
        ".{name}.{}.{}.new",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|it| it.as_nanos())
            .unwrap_or_default()
    ));

    {
        let mut file = create(&temp)
            .map_err(|why| format!("{} could not be written: {why}", temp.display()))?;
        file.write_all(bytes)
            .map_err(|why| format!("{} could not be written: {why}", temp.display()))?;
        // **The flush is not optional.** Without it the rename can land before the contents do,
        // which is a file that exists, is the right length, and is full of nothing.
        file.sync_all()
            .map_err(|why| format!("{} could not be flushed: {why}", temp.display()))?;
    }

    rename_over(&temp, path)
}

/// Create the temporary, readable by this account and nobody else where that can be said.
///
/// **The mode is set as the file is created**, not afterwards: a file that exists
/// world-readable for a moment has been world-readable. Every caller of this module holds
/// something private — keys, bearers, pairings, what the user chose — so the strict mode is the
/// default here rather than an option somebody has to remember to pass.
///
/// Windows has no mode, and all of these live inside the user's own profile, which is already
/// theirs. Pretending to set one would be the kind of gesture that reads as a protection and is
/// not.
fn create(at: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        return std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(at);
    }
    #[cfg(not(unix))]
    std::fs::File::create(at)
}

#[cfg(windows)]
fn rename_over(from: &Path, to: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt as _;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    fn wide(path: &Path) -> Vec<u16> {
        path.as_os_str().encode_wide().chain(Some(0)).collect()
    }

    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call, which is the whole
    // of this function's contract with the operating system.
    let moved = unsafe {
        MoveFileExW(
            wide(from).as_ptr(),
            wide(to).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(format!(
            "{} could not replace {}: {}",
            from.display(),
            to.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn rename_over(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::rename(from, to).map_err(|why| {
        format!(
            "{} could not replace {}: {why}",
            from.display(),
            to.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn somewhere(what: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "epoch-atomic-{}-{what}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|it| it.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("a temporary folder");
        dir
    }

    #[test]
    fn a_file_that_already_exists_is_replaced_whole() {
        // The case `std::fs::rename` refuses on Windows, which is every save after the first.
        let dir = somewhere("replace");
        let at = dir.join("settings.toml");
        replace(&at, b"first = true").expect("the first write");
        replace(&at, b"second = true").expect("and the second");
        assert_eq!(
            std::fs::read_to_string(&at).expect("readable"),
            "second = true"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn nothing_is_left_beside_it_when_it_works() {
        // A folder filling up with `.settings.toml.1234.new` would be this fix leaking into the
        // user's vault, which they read.
        let dir = somewhere("tidy");
        let at = dir.join("bridges.toml");
        replace(&at, b"one").expect("a write");
        replace(&at, b"two").expect("another");
        let left: Vec<String> = std::fs::read_dir(&dir)
            .expect("a folder")
            .filter_map(|it| it.ok())
            .map(|it| it.file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left, vec!["bridges.toml".to_owned()], "{left:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_folder_is_made_if_it_is_not_there() {
        let dir = somewhere("fresh");
        let at = dir.join("deeper").join("secrets.dat");
        replace(&at, b"sealed").expect("a write into a folder that did not exist");
        assert_eq!(std::fs::read(&at).expect("readable"), b"sealed");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
