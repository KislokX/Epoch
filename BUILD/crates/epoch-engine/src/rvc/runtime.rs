//! ONNX Runtime, fetched the day somebody wants a voice and never before.
//!
//! ## Why this is a download rather than a dependency
//!
//! `ort`'s default features link a runtime at **build** time: everybody who clones Epoch pays
//! for a subsystem most of them will never open, and a machine with no network cannot build the
//! product at all. `load-dynamic` moves the whole cost to the one person who asked for it — the
//! crate compiles against nothing and opens a library by path when a voice is actually spoken.
//!
//! That is the same arrangement Piper and whisper.cpp already have, and the same sentence:
//! **offered, never imposed.** A machine without it has RVC visibly dormant and every Piper
//! voice still working.
//!
//! ## The version is pinned to the crate, not chosen
//!
//! `ort 2.0.0-rc.13` wraps ONNX Runtime **1.28**, so that is what is fetched. The two are one
//! fact and a mismatch is not a compile error — it is a `dlopen` that either fails at the moment
//! somebody speaks or, worse, succeeds against a different ABI. `=2.0.0-rc.13` in the manifest
//! is what holds the other half.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The ONNX Runtime release that matches the `ort` version this crate is built against.
const VERSION: &str = "1.28.2";

/// What this platform's CPU build is called, and what it weighs.
///
/// **CPU only.** A GPU build is 365–454 MB against 9–79, and nothing here is slow: the whole
/// point of converting a voice once is that speaking it afterwards is small work.
fn release() -> Option<(&'static str, &'static str, u64)> {
    if cfg!(all(windows, target_arch = "x86_64")) {
        Some(("onnxruntime-win-x64", "zip", 79))
    } else if cfg!(all(windows, target_arch = "aarch64")) {
        Some(("onnxruntime-win-arm64", "zip", 80))
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some(("onnxruntime-osx-arm64", "tgz", 32))
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some(("onnxruntime-linux-x64", "tgz", 9))
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        Some(("onnxruntime-linux-aarch64", "tgz", 8))
    } else {
        None
    }
}

/// Roughly what fetching it costs, said before it is pressed.
///
/// `None` where Epoch has no build to point at. **Silence rather than a number**, because a
/// platform with no release is not a platform with a small download.
pub fn cost() -> Option<String> {
    release().map(|(_, _, mb)| {
        format!("About {mb} MB, once. It is what actually runs a converted voice.")
    })
}

/// Where the library lives once it is here.
fn runtime_dir() -> PathBuf {
    super::home().join("runtime")
}

/// What the library is called on this platform.
///
/// The Unix names carry the version, which is why they are built from `VERSION` rather than
/// written out: a hard-coded `libonnxruntime.so.1.28.2` is a string that stops matching the
/// crate the day the crate moves, silently, in the direction of *not found*.
fn library_names() -> Vec<String> {
    if cfg!(windows) {
        vec!["onnxruntime.dll".to_owned()]
    } else if cfg!(target_os = "macos") {
        vec![
            format!("libonnxruntime.{VERSION}.dylib"),
            "libonnxruntime.dylib".to_owned(),
        ]
    } else {
        vec![
            format!("libonnxruntime.so.{VERSION}"),
            "libonnxruntime.so".to_owned(),
        ]
    }
}

/// The library Epoch fetched, if it is here.
pub fn library() -> Option<PathBuf> {
    let dir = runtime_dir();
    library_names()
        .into_iter()
        .map(|it| dir.join(it))
        .find(|it| it.is_file())
}

/// Whether a voice could be spoken right now, as far as the runtime goes.
pub fn ready() -> bool {
    library().is_some()
}

/// Fetch it.
///
/// The archive carries headers, static libraries and a handful of provider stubs; only the one
/// shared library is kept. **Everything under `tools/rvc/runtime`**, so deleting that folder
/// undoes it and touches nothing else — including no `PATH`, no registry key, and no library
/// installed where another program would find it.
pub fn install(watching: &dyn Fn(u64, Option<u64>)) -> Result<PathBuf, String> {
    let Some((name, extension, _)) = release() else {
        return Err(
            "Epoch has no ONNX Runtime build for this platform, so RVC voices cannot be spoken \
             here. Piper voices are unaffected."
                .to_owned(),
        );
    };

    let dir = runtime_dir();
    std::fs::create_dir_all(&dir).map_err(|why| format!("could not make {dir:?}: {why}"))?;

    let file = format!("{name}-{VERSION}.{extension}");
    let url =
        format!("https://github.com/microsoft/onnxruntime/releases/download/v{VERSION}/{file}");

    let scratch = dir.join("unpacking");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch)
        .map_err(|why| format!("could not make {scratch:?}: {why}"))?;

    let archive = crate::speech::fetch(&url, &file, &scratch, watching)?;
    crate::speech::unpack(&archive, &scratch)?;

    // The archive nests everything under its own name, and the layout has moved between
    // releases. Looking for the file rather than assuming where it sits costs one walk and
    // survives a release that rearranges itself.
    let wanted = library_names();
    let found = walk(&scratch)
        .into_iter()
        .find(|it| {
            it.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| wanted.iter().any(|w| w == n))
        })
        .ok_or_else(|| {
            format!(
                "that archive holds no {}",
                wanted.first().cloned().unwrap_or_default()
            )
        })?;

    let kept = dir.join(found.file_name().expect("a file that was found has a name"));
    std::fs::copy(&found, &kept).map_err(|why| format!("could not keep {kept:?}: {why}"))?;
    let _ = std::fs::remove_dir_all(&scratch);
    Ok(kept)
}

/// Every file under a directory, depth first.
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push(path);
        }
    }
    found
}

/// Point `ort` at the library Epoch fetched, once per process.
///
/// **`ort` reads this at first use and never again**, so it is set before any session is built
/// and guarded so a second voice cannot race the first. Returning the path rather than `()`
/// means the caller can say *which* library answered, which is the difference between a working
/// runtime and one somebody else installed.
pub fn arm() -> Result<PathBuf, String> {
    static ARMED: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    ARMED
        .get_or_init(|| {
            let path = library().ok_or_else(|| {
                "The ONNX Runtime is not on this machine yet, so a converted voice cannot be \
                 spoken. Epoch can fetch it."
                    .to_owned()
            })?;
            // Safe here: this runs once, before any thread has asked `ort` for anything.
            std::env::set_var("ORT_DYLIB_PATH", &path);
            Ok(path)
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_platform_epoch_ships_on_has_a_build() {
        // The three the product is actually built for. A platform with no release answers
        // `None` and says so out loud rather than failing at the moment somebody speaks.
        assert!(
            release().is_some(),
            "this platform has no ONNX Runtime build named"
        );
        assert!(cost().is_some());
    }

    #[test]
    fn the_library_name_carries_the_pinned_version() {
        // The Unix names are built from `VERSION`, so this is what stops the constant and the
        // filename drifting apart — the one pair a mismatch would fail silently.
        let names = library_names();
        assert!(!names.is_empty());
        if !cfg!(windows) {
            assert!(
                names.iter().any(|it| it.contains(VERSION)),
                "no name carries {VERSION}: {names:?}"
            );
        }
    }
}
