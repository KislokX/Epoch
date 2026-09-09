//! Asset resolution (ADR-0020).
//!
//! An Asset is a file living inside a World. A Renderable references it by **relative path**,
//! and this module turns that reference into something the renderer can draw.
//!
//! Two rules here are security decisions, not conveniences:
//!
//! 1. **Paths are confined to the World.** A World is third-party content. Absolute paths,
//!    drive letters and any traversal escaping the World directory are rejected.
//! 2. **Assets are delivered as data-URI images, never as markup.** Inlining a World's SVG
//!    into the DOM would let any installed World execute script inside Epoch. An `<image>`
//!    fed a data URI does not run scripts — and SVG, PNG and every future raster format
//!    travel the identical path, so pixel art costs nothing later.

use std::path::{Component, Path, PathBuf};

use base64::Engine as _;

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("asset reference '{reference}' escapes the World; assets must live inside it")]
    Escapes { reference: String },
    #[error("asset reference '{reference}' must be relative, not absolute")]
    NotRelative { reference: String },
    #[error("asset '{reference}' not found at {path}")]
    Missing { reference: String, path: PathBuf },
    #[error("cannot read asset '{reference}': {source}")]
    Unreadable {
        reference: String,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "asset '{reference}' has an unsupported extension; expected one of svg, png, jpg, webp"
    )]
    UnsupportedFormat { reference: String },
}

/// An asset resolved into something the renderer can draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAsset {
    /// The reference as the World wrote it, kept for reporting.
    pub reference: String,
    /// A `data:` URI. The renderer draws this with `<image>` and never inlines it.
    pub data_uri: String,
}

/// Resolve a World-relative asset reference against the World's directory.
///
/// `world_dir` is the directory containing the World's manifest.
pub fn resolve(world_dir: &Path, reference: &str) -> Result<ResolvedAsset, AssetError> {
    let rel = Path::new(reference);

    if rel.is_absolute() || rel.components().any(|c| matches!(c, Component::Prefix(_))) {
        return Err(AssetError::NotRelative {
            reference: reference.to_string(),
        });
    }

    // Reject traversal before touching the filesystem, so a missing file cannot mask an
    // escape attempt.
    if rel.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(AssetError::Escapes {
            reference: reference.to_string(),
        });
    }

    let mime = mime_for(reference).ok_or_else(|| AssetError::UnsupportedFormat {
        reference: reference.to_string(),
    })?;

    let path = world_dir.join(rel);
    if !path.is_file() {
        return Err(AssetError::Missing {
            reference: reference.to_string(),
            path,
        });
    }

    // Belt and braces: confirm containment after the filesystem has had its say, so symlinks
    // cannot lead outside the World either.
    let canonical_world = world_dir
        .canonicalize()
        .map_err(|source| AssetError::Unreadable {
            reference: reference.to_string(),
            source,
        })?;
    let canonical_asset = path
        .canonicalize()
        .map_err(|source| AssetError::Unreadable {
            reference: reference.to_string(),
            source,
        })?;
    if !canonical_asset.starts_with(&canonical_world) {
        return Err(AssetError::Escapes {
            reference: reference.to_string(),
        });
    }

    let bytes = std::fs::read(&canonical_asset).map_err(|source| AssetError::Unreadable {
        reference: reference.to_string(),
        source,
    })?;

    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);

    Ok(ResolvedAsset {
        reference: reference.to_string(),
        data_uri: format!("data:{mime};base64,{encoded}"),
    })
}

/// Formats the renderer can draw today. Vector and raster deliberately share one path.
fn mime_for(reference: &str) -> Option<&'static str> {
    let ext = Path::new(reference)
        .extension()
        .and_then(|e| e.to_str())?
        .to_ascii_lowercase();
    match ext.as_str() {
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        // A World may supply its own sounds as well as its own windows. Exactly the formats
        // `SoundFormat` sniffs, and a test holds the two lists together — what a person may put
        // into a World has to stay what this function can hand back out (ADR-0024).
        "flac" => Some("audio/flac"),
        "mp3" => Some("audio/mpeg"),
        "ogg" => Some("audio/ogg"),
        "wav" => Some("audio/wav"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World(PathBuf);

    impl World {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("epoch-asset-{name}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("assets")).unwrap();
            Self(dir)
        }
        fn write(&self, rel: &str, body: &str) {
            let p = self.0.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, body).unwrap();
        }
    }

    impl Drop for World {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn an_asset_inside_the_world_resolves_to_a_data_uri() {
        let w = World::new("ok");
        w.write(
            "assets/lab.svg",
            "<svg xmlns='http://www.w3.org/2000/svg'/>",
        );
        let a = resolve(&w.0, "assets/lab.svg").unwrap();
        assert!(a.data_uri.starts_with("data:image/svg+xml;base64,"));
        assert_eq!(a.reference, "assets/lab.svg");
    }

    #[test]
    fn raster_and_vector_travel_the_same_path() {
        // Why pixel art costs nothing later.
        let w = World::new("raster");
        w.write("assets/lab.png", "not really a png, but bytes are bytes");
        let a = resolve(&w.0, "assets/lab.png").unwrap();
        assert!(a.data_uri.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn traversal_out_of_the_world_is_refused() {
        let w = World::new("escape");
        let err = resolve(&w.0, "../../../secrets.svg").unwrap_err();
        assert!(matches!(err, AssetError::Escapes { .. }));
    }

    #[test]
    fn absolute_paths_are_refused() {
        // **Absolute for the machine this is running on.** `C:/Windows/...` is an absolute path
        // on Windows and an ordinary relative name on Unix, where `C:` is just a folder somebody
        // could have — so the Windows spelling alone tested nothing on a Mac, and said it did.
        let w = World::new("absolute");
        let outside = if cfg!(windows) {
            "C:/Windows/System32/config.svg"
        } else {
            "/etc/passwd.svg"
        };
        let err = resolve(&w.0, outside).unwrap_err();
        assert!(matches!(
            err,
            AssetError::NotRelative { .. } | AssetError::Escapes { .. }
        ));
    }

    #[test]
    fn a_missing_asset_is_reported_rather_than_panicking() {
        let w = World::new("missing");
        let err = resolve(&w.0, "assets/nope.svg").unwrap_err();
        assert!(matches!(err, AssetError::Missing { .. }));
    }

    #[test]
    fn an_unsupported_format_is_refused_before_reading_anything() {
        let w = World::new("format");
        w.write("assets/lab.exe", "MZ");
        let err = resolve(&w.0, "assets/lab.exe").unwrap_err();
        assert!(matches!(err, AssetError::UnsupportedFormat { .. }));
    }
}
