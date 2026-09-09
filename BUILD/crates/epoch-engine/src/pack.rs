//! World Pack loading and concept resolution (ADR-0016, ADR-0017, ADR-0022).
//!
//! The engine asks for a **concept** or a **Place identity**. The pack chain answers. The
//! engine never names a file, a character or a location.
//!
//! A pack is one **contributor**. It does not own a Place; it contributes a patch to one.
//! Loading is the *Resolve* phase: manifests are parsed, assets are read from disk, invalid
//! fields are dropped and reported. What comes out is safe for a pure Compose to fold
//! (see [`crate::place`]).
//!
//! Three rules from the architecture are enforced here rather than merely documented:
//!
//! 1. **License metadata is mandatory.** A manifest without it fails to load
//!    (`CONTENT_PHILOSOPHY.md`, hard rule).
//! 2. **The fallback chain always resolves.** active pack -> ... -> default pack ->
//!    visible placeholder. Never a crash, never a blank.
//! 3. **Reject the field, not the World.** A broken Place, mark or anchor costs itself and
//!    is reported; the World still loads and still renders.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use epoch_kernel::PlaceConcept;
use serde::{Deserialize, Serialize};

use crate::map::WorldMap;
use crate::place::{PlaceContribution, PlaceDeclarations, PlaceId};

/// Anything that can go wrong loading a pack. A broken pack must never crash the engine,
/// so callers can report this and continue with whatever remains in the chain.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    #[error("cannot read pack manifest at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// Boxed, for the same reason as `DefinitionError::Parse`: a `toml::de::Error` is large
    /// enough to make every `Result` in this module that size, including the ones that succeed.
    #[error("invalid pack manifest at {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: Box<toml::de::Error>,
    },
    /// The hard rule of `CONTENT_PHILOSOPHY.md`, made executable.
    #[error("pack '{id}' declares no license; every pack must carry license metadata")]
    MissingLicense { id: String },
    #[error("cannot rename World '{id}': {reason}")]
    Rename { id: String, reason: String },
}

// `Resolved` lived here: a single scalar answered by walking the chain, carrying whether it
// fell through to a placeholder. Its last caller was the character name, and that left with
// the cast (ADR-0023).
//
// Deleted rather than kept for symmetry. Places already answer the same question better —
// `Place::is_placeholder` reports it for a whole composed Place instead of one field — so
// keeping this would have meant two ways to ask "did anyone actually supply this?".

// ---------------------------------------------------------------------------
// Manifest
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Manifest {
    pack: PackIdentity,
    license: Option<License>,
    /// The Places this World contributes, keyed by identity (ADR-0022).
    #[serde(default)]
    places: PlaceDeclarations,
    /// The pack's geography. Optional: a pack may declare Places without any land around
    /// them, and the World still renders (ADR-0016 degrades, never breaks).
    #[serde(default)]
    map: Option<WorldMap>,
    /// What this World's own windows look like.
    #[serde(default)]
    ui: Vec<SkinDeclaration>,
    /// What this World sounds like.
    #[serde(default)]
    sound: Vec<SoundDeclaration>,
}

/// One sound this World supplies, as its author declares it.
///
/// A flat list for the same reason `[[ui]]` is one: the concept is a dotted name and the id *is*
/// the contract. `sfx.click`, never a path.
#[derive(Debug, Deserialize)]
struct SoundDeclaration {
    /// The Asset Concept this supplies, e.g. `sfx.click`.
    concept: String,
    /// The audio, relative to the World folder. Resolved like every other Asset.
    file: String,
}

/// One piece of the interface, as its author declares it.
///
/// ## Why a list rather than nested tables
///
/// The concept is a dotted name - `ui.frame.window` - and TOML would read that as three levels
/// of table. A flat list keeps the id in one piece, which matters because the id *is* the
/// contract: the Engine asks for a concept and never for a path (ADR-0016).
#[derive(Debug, Deserialize)]
struct SkinDeclaration {
    /// The Asset Concept this supplies, e.g. `ui.frame.window`.
    concept: String,
    /// The image, relative to the World folder. Resolved like every other Asset.
    image: String,
    /// How much of each edge is corner, in the image's own pixels.
    ///
    /// **One number, or four.** One image and "the frame is twelve pixels" is how somebody draws
    /// a window; nine files is how a renderer thinks about one.
    ///
    /// Four exists because the first real frame needed it. Measured from it: top 11, right 5,
    /// bottom 12, left 6 - a bevel lit from above is not symmetric, and forcing one number on it
    /// would have stretched the highlight or eaten into the face. `[top, right, bottom, left]`,
    /// clockwise from the top, which is the order CSS uses and the order every art tool shows.
    corner: CornerInset,
    /// How the edges and the middle are drawn when the window is bigger than the image.
    ///
    /// `stretch` by default, and that is measured rather than assumed: the first frame's face is
    /// a soft vertical gradient, and repeating it bands. A tiled pattern would want `repeat`, so
    /// the author says.
    #[serde(default)]
    repeat: SkinRepeat,
    /// How many screen pixels one image pixel becomes. **Whole numbers only.**
    ///
    /// Epoch is pixel art; a fractional scale destroys it. Anything below 1 is refused rather
    /// than clamped, so a typo is visible instead of quietly ignored.
    #[serde(default = "one")]
    scale: u32,
}

fn one() -> u32 {
    1
}

/// One number for every side, or one per side.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum CornerInset {
    All(u32),
    /// `[top, right, bottom, left]`.
    Sides([u32; 4]),
}

impl CornerInset {
    fn sides(&self) -> [u32; 4] {
        match self {
            Self::All(n) => [*n; 4],
            Self::Sides(sides) => *sides,
        }
    }
}

/// How a skin fills a window larger than the image it was drawn at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SkinRepeat {
    /// Drawn once and stretched. Right for a gradient or a soft texture.
    #[default]
    Stretch,
    /// Tiled, cutting the last tile. Right for a pattern with no gradient across it.
    Repeat,
    /// Tiled, with the tile resized so a whole number fit.
    Round,
    /// Tiled, with the gaps spread between whole tiles.
    Space,
}

impl SkinRepeat {
    /// The CSS keyword. Named here rather than in the frontend so the vocabulary has one home.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stretch => "stretch",
            Self::Repeat => "repeat",
            Self::Round => "round",
            Self::Space => "space",
        }
    }
}

#[derive(Debug, Deserialize)]
struct PackIdentity {
    id: String,
    name: String,
    version: String,
    /// What kind of World this is, in the author's own words ("Finance OS", "Playground").
    ///
    /// Optional, and deliberately free text rather than an enum: the Engine never interprets
    /// it. It exists because a Launcher that can only say a World's *name* has nothing to
    /// tell you about a place you have not been to yet.
    #[serde(default)]
    kind: Option<String>,
    /// A sentence or two on what this World is. Same rule: authored, never interpreted.
    #[serde(default)]
    description: Option<String>,
    /// Key art: how this World wants to be *seen* before you enter it.
    ///
    /// Optional, and the reason it is optional is the whole design. Epoch can always draw a
    /// World from its own geography — terrain, roads, Place positions — which cannot promise
    /// anything the World does not contain. Key art can promise anything, and that is
    /// precisely why it belongs to the author rather than to us: they decide how much
    /// immersion this World earns, and how detailed it is allowed to look.
    ///
    /// No artwork means the derived chart, which is a complete answer rather than a gap.
    #[serde(default)]
    preview: Option<String>,
    /// What lies behind the world: sky, horizon, distance.
    ///
    /// A layer *under* the terrain, never a replacement for it. Roads, Places and inhabitants
    /// still draw on top, so the backdrop adds depth without the World becoming a picture that
    /// nothing can be derived from.
    #[serde(default)]
    backdrop: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct License {
    #[serde(rename = "type")]
    pub kind: String,
    pub holder: String,
    #[serde(default)]
    pub notes: Option<String>,
}

/// One resolved piece of interface.
///
/// **The same pipeline as everything else** (ADR-0016, ADR-0020): a concept, resolved by the
/// active pack, confined to the World folder, delivered as a `data:` URI so an installed World
/// cannot execute script. Nothing here is a new rendering system - it is the interface finally
/// being *content*, the way Characters, Places and terrain already are.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skin {
    /// The image, as a `data:` URI.
    pub image: String,
    /// Corner inset per side, `[top, right, bottom, left]`, in the image's own pixels.
    pub corner: [u32; 4],
    /// How the edges and middle fill a larger window.
    pub repeat: SkinRepeat,
    /// Whole-number scale. One image pixel becomes this many screen pixels.
    pub scale: u32,
    /// The smallest this window may be drawn, in screen pixels, `[width, height]`.
    ///
    /// **Derived here rather than left to the renderer**, because it is a fact about the
    /// artwork: below it the corners meet and the frame stops being a frame. The rule is the
    /// two opposite corners plus a middle at least as long as the larger of them - so there is
    /// always some window between the corners, and never a `[]` where a window should be.
    ///
    /// A surface may of course be larger, and every one of Epoch's is: this is a floor that
    /// protects the drawing, not a layout.
    pub min_size: [u32; 2],
}

/// A loaded World Pack: identity, license, and the contributions it makes.
#[derive(Debug, Clone)]
pub struct WorldPack {
    pub id: String,
    pub name: String,
    pub version: String,
    /// What kind of World this is, if the author said. Never interpreted by the Engine.
    pub kind: Option<String>,
    /// What this World is, if the author said.
    pub description: Option<String>,
    /// Key art, already resolved to a `data:` URI, when the author supplied any.
    ///
    /// Read through [`crate::asset`] like every other Asset: same confinement, same formats,
    /// same delivery as a data URI so an installed World cannot execute script (ADR-0020).
    /// Unreadable key art costs the key art and leaves the derived chart, never the World.
    pub preview: Option<String>,
    /// The backdrop **reference**, not its bytes.
    ///
    /// Deliberately unlike `preview`. A preview is small and every installed World needs one
    /// at once, so it is read eagerly. A backdrop is a full illustration — megabytes — and
    /// only the World you actually entered needs it. Reading it at load would hold one per
    /// discovered pack in memory, and putting it in `WorldView` would re-send it across IPC
    /// every time somebody turns a page (`world:changed` fires on any presence change).
    ///
    /// So it is fetched once, on demand, by [`Self::backdrop_data`].
    pub backdrop: Option<String>,
    pub license: License,
    /// This World's own interface: concept to image, already resolved.
    ///
    /// Empty for every World nobody has drawn one for, which is all of them today - and that is
    /// a complete World rather than an unfinished one, because Epoch draws its own windows.
    pub ui: BTreeMap<String, Skin>,
    /// This World's own voice: concept to audio, already resolved to a `data:` URI.
    ///
    /// **A supplied sound overrides a synthesised one; it never fills an empty slot.** `sfx.ts`
    /// ships no files on purpose, so that Epoch makes a noise on a machine that has downloaded
    /// nothing. A World that supplies one is replacing something that already worked.
    ///
    /// Delivered as a URI rather than a path for the same reason artwork is (ADR-0020): an
    /// installed World cannot be allowed to name a file the renderer then goes and opens.
    pub sounds: BTreeMap<String, String>,
    /// Where this World lives on disk. A World is a folder, and saying so lets it be edited.
    pub manifest: PathBuf,
    /// This pack's Place patches, already resolved: assets read, bad fields dropped.
    ///
    /// Resolving at load rather than per projection matters: the alternative reads files on
    /// every simulation tick. It also gives the right invalidation for free — reloading the
    /// World re-reads its assets.
    places: BTreeMap<String, PlaceContribution>,
    /// Geography, carried but never interpreted by the engine (see [`crate::map`]).
    pub map: Option<WorldMap>,
    /// What this pack got wrong, reported rather than silently rendering nothing.
    problems: Vec<String>,
    /// A fingerprint of every file under this pack when it was read.
    ///
    /// **A sweep of the folder, not a list of the files that were resolved.** The pack keeps the
    /// `data:` URI an asset became and never the path it came from — and a place's artwork is
    /// resolved inside `place.rs`, which reports problems and not paths. Threading every
    /// reference back out would widen three signatures to learn something a `read_dir` already
    /// knows.
    ///
    /// The cost is what decided it, measured rather than feared
    /// (`tests/the_cost_of_watching_a_pack.rs`): 0.15 ms for a pack the size of the ones that
    /// exist, 1.63 ms for two thousand files — 0.33% of wall time at the heartbeat's twice a
    /// second, which is the rate that matters because `reload_if_changed` holds the World's lock
    /// across it.
    ///
    /// **A fingerprint and not the newest write, which the first version was.** Found by using
    /// it: renaming a Place reached the World in half a second, and *putting it back* never did.
    /// A maximum only ever climbs, so restoring an older file — an undo, a backup, `git
    /// checkout` — leaves it exactly where it was, and deleting one does too. Every one of those
    /// is a thing somebody editing artwork does.
    print: Option<u64>,
}

/// A fingerprint of every file under a directory: its path, its size and when it was written.
///
/// `None` for a directory that cannot be read — a question that could not be asked, which must
/// not be confused with an answer.
///
/// **Combined by addition rather than by sorting.** Each file becomes one FNV-1a hash and they
/// are summed, so the order `read_dir` happens to return them in cannot change the answer — two
/// sweeps that disagreed about the order would disagree about the fingerprint, which is a reload
/// every tick forever. Collecting them to sort instead cost 5x, measured
/// (`tests/the_cost_of_watching_a_pack.rs`): 4.95 ms against 1.63 ms at two thousand files, and
/// this runs twice a second.
///
/// Nothing here defends against a *constructed* collision: the input is the user's own folder,
/// and the cost of an accidental one is a redraw that waits for the next save.
fn print_of(dir: &Path) -> Option<u64> {
    let mut total: u64 = 0;
    let mut stack = vec![dir.to_path_buf()];
    let mut read_anything = false;
    while let Some(at) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&at) else {
            continue;
        };
        read_anything = true;
        for entry in entries.flatten() {
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                stack.push(entry.path());
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let when = meta
                .modified()
                .ok()
                .and_then(|when| when.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |since| since.as_nanos() as u64);

            let mut one: u64 = 0xcbf2_9ce4_8422_2325;
            let mut eat = |bytes: &[u8]| {
                for byte in bytes {
                    one ^= u64::from(*byte);
                    one = one.wrapping_mul(0x1000_0000_01b3);
                }
            };
            // The name rather than the whole path: the walk already visited the folders above
            // it, and a rename inside one still changes this file's term.
            eat(entry.file_name().to_string_lossy().as_bytes());
            eat(&meta.len().to_le_bytes());
            eat(&when.to_le_bytes());
            total = total.wrapping_add(one);
        }
    }
    if !read_anything {
        return None;
    }
    Some(total)
}

impl WorldPack {
    /// Load a pack from its `pack.toml`. This is the **Resolve** phase.
    pub fn load(manifest_path: &Path) -> Result<Self, PackError> {
        let raw = std::fs::read_to_string(manifest_path).map_err(|source| PackError::Io {
            path: manifest_path.to_path_buf(),
            source,
        })?;

        let manifest: Manifest = toml::from_str(&raw).map_err(|source| PackError::Parse {
            path: manifest_path.to_path_buf(),
            source: Box::new(source),
        })?;

        let license = manifest.license.ok_or_else(|| PackError::MissingLicense {
            id: manifest.pack.id.clone(),
        })?;

        // Read every Asset now, against this World's own directory. Paths are confined to
        // the World (ADR-0020); failures are reported, not fatal. After this point nothing
        // in the Place pipeline touches a filesystem.
        let world_dir = manifest_path.parent().unwrap_or(Path::new("."));
        let mut places = BTreeMap::new();
        let mut problems = Vec::new();
        for (id, declaration) in &manifest.places {
            let (contribution, mut found) = declaration.resolve(&PlaceId::new(id), world_dir);
            problems.append(&mut found);
            places.insert(id.clone(), contribution);
        }

        // Key art travels the ordinary Asset path. A World that names artwork it does not have
        // says so and still opens, showing its derived chart instead.
        let preview = manifest.pack.preview.as_deref().and_then(|reference| {
            match crate::asset::resolve(world_dir, reference) {
                Ok(resolved) => Some(resolved.data_uri),
                Err(err) => {
                    problems.push(format!("preview: {err}"));
                    None
                }
            }
        });

        // The interface this World brings with it, resolved the same way its buildings are.
        //
        // A skin that cannot be read costs the skin and nothing else: Epoch draws its own
        // windows, so the World still opens and still looks like something. That is the
        // fallback chain doing its job rather than an error path (ADR-0016).
        let mut ui = BTreeMap::new();
        for declared in manifest.ui {
            match crate::asset::resolve(world_dir, &declared.image) {
                Ok(resolved) => {
                    let corner = declared.corner.sides();
                    // Refused, never clamped - the same rule authored parameters follow
                    // everywhere else (ADR-0026): a silently corrected value leaves the file
                    // saying one thing and the screen doing another.
                    if declared.scale == 0 {
                        problems.push(format!(
                            "{}: scale must be a whole number of at least 1",
                            declared.concept
                        ));
                        continue;
                    }
                    let [top, right, bottom, left] = corner;
                    ui.insert(
                        declared.concept,
                        Skin {
                            image: resolved.data_uri,
                            corner,
                            repeat: declared.repeat,
                            scale: declared.scale,
                            min_size: [
                                (left + right + left.max(right)) * declared.scale,
                                (top + bottom + top.max(bottom)) * declared.scale,
                            ],
                        },
                    );
                }
                Err(err) => problems.push(format!("{}: {err}", declared.concept)),
            }
        }

        // The same fallback chain: an unreadable sound costs that sound and nothing else. The
        // World keeps its synthesised voice, which is what every World has today.
        let mut sounds = BTreeMap::new();
        for declared in manifest.sound {
            match crate::asset::resolve(world_dir, &declared.file) {
                Ok(resolved) => {
                    sounds.insert(declared.concept, resolved.data_uri);
                }
                Err(err) => problems.push(format!("{}: {err}", declared.concept)),
            }
        }

        Ok(Self {
            id: manifest.pack.id,
            name: manifest.pack.name,
            version: manifest.pack.version,
            kind: manifest.pack.kind,
            description: manifest.pack.description,
            preview,
            backdrop: manifest.pack.backdrop,
            license,
            ui,
            sounds,
            manifest: manifest_path.to_path_buf(),
            places,
            map: manifest.map,
            problems,
            print: print_of(world_dir),
        })
    }

    /// True when anything under this pack has been written since it was read.
    ///
    /// **What this closes:** a character edited in another window reaches the next turn
    /// (`DefinitionRegistry::changed_on_disk`), and a sprite did not — the World had to be left
    /// and entered again. Artwork is the thing somebody iterates on, so it was the wrong half to
    /// leave out.
    ///
    /// A pack whose folder cannot be read answers **false**, not true: an unreadable directory is
    /// a question that could not be asked, and reading it as *changed* would reload the World
    /// twice a second forever.
    pub fn changed_on_disk(&self) -> bool {
        let now = print_of(self.dir());
        match (self.print, now) {
            (Some(had), Some(now)) => now != had,
            // It had nothing readable and still has nothing: unchanged.
            (None, None) => false,
            // A folder appeared where there was none, or one stopped answering. The first is a
            // change; the second is not, and telling them apart is what `now.is_some()` does.
            _ => now.is_some(),
        }
    }

    /// What this pack got wrong at load.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// How many Places this World actually declares.
    ///
    /// A World declares no characters at all any more (ADR-0023): the cast is the user's and
    /// lives in the vault. Counting names a World supplied for archetypes was what once let
    /// the Launcher report a population that did not exist.
    pub fn declared(&self) -> usize {
        self.places.len()
    }

    /// This World's roads, resolved to world-unit polylines.
    ///
    /// Roads are what makes a map recognisable at a glance — far more than building
    /// silhouettes, which at preview scale are five pixels of mush. A road needs both ends
    /// placed; one missing end means the World is partially authored, so it is simply not
    /// drawn.
    pub fn route_lines(&self) -> Vec<(Vec<[f32; 2]>, &'static str)> {
        let Some(map) = &self.map else {
            return Vec::new();
        };
        map.routes
            .iter()
            .filter_map(|route| {
                let from = self.places.get(&route.from)?.at?;
                let to = self.places.get(&route.to)?.at?;
                Some((
                    vec![[from.x, from.y], [to.x, to.y]],
                    match route.prominence {
                        crate::map::Prominence::Major => "major",
                        crate::map::Prominence::Minor => "minor",
                    },
                ))
            })
            .collect()
    }

    /// Where this World's Places sit, as `[x, y, footprint]`.
    ///
    /// Enough to draw the World small without composing it — a preview is a glance, not a
    /// projection.
    pub fn place_dots(&self) -> Vec<[f32; 3]> {
        let mut dots: Vec<[f32; 3]> = self
            .places
            .values()
            .filter_map(|c| c.at.map(|at| [at.x, at.y, c.footprint.unwrap_or(90.0)]))
            .collect();
        // `places` is a BTreeMap, so this is already stable; sorting keeps it stable even if
        // the collection type ever changes.
        dots.sort_by(|a, b| a[1].total_cmp(&b[1]).then(a[0].total_cmp(&b[0])));
        dots
    }

    /// Give this World a different name.
    ///
    /// **The identity never changes.** Only `name` is touched; `id` is what Places, routes,
    /// saved state and `enter` are addressed by, and a rename that moved it would break every
    /// one of them. This is the same rule Places follow, applied to the World itself.
    ///
    /// Edits exactly one line rather than re-serialising the manifest. A World's `pack.toml`
    /// is authored by a person, and its comments explain why the geography means what it
    /// means — rewriting the file would silently delete all of it.
    pub fn rename(&self, new_name: &str) -> Result<(), PackError> {
        let name = new_name.trim();
        let refuse = |reason: &str| PackError::Rename {
            id: self.id.clone(),
            reason: reason.to_string(),
        };

        if name.is_empty() {
            return Err(refuse("a World must have a name"));
        }
        if name
            .chars()
            .any(|c| c.is_control() || c == '"' || c == '\\')
        {
            return Err(refuse(
                "a name cannot contain quotes, backslashes or line breaks",
            ));
        }

        self.write_pack_key("name", Some(name))
            .map_err(|_| refuse("its manifest has no [pack] section to change"))
    }

    /// Where this World's own files live. A World is a folder, and saying so lets it be edited.
    pub fn dir(&self) -> &Path {
        self.manifest.parent().unwrap_or(Path::new("."))
    }

    /// Read this World's backdrop, now.
    ///
    /// Separate from the per-tick projection on purpose — see [`Self::backdrop`]. An
    /// unreadable backdrop yields `None` and its reason; the World renders without it, exactly
    /// as a World that never declared one does.
    pub fn backdrop_data(&self) -> (Option<String>, Option<String>) {
        let Some(reference) = self.backdrop.as_deref() else {
            return (None, None);
        };
        match crate::asset::resolve(self.dir(), reference) {
            Ok(resolved) => (Some(resolved.data_uri), None),
            Err(err) => (None, Some(format!("backdrop: {err}"))),
        }
    }

    /// Accept a backdrop for this World, or clear it.
    pub fn set_backdrop(&self, base64_data: Option<&str>) -> Result<(), PackError> {
        let refuse = |reason: String| PackError::Rename {
            id: self.id.clone(),
            reason,
        };

        match base64_data {
            Some(data) => {
                let name =
                    crate::import::accept_image(&self.dir().join("assets"), "backdrop", data)
                        .map_err(|err| refuse(err.to_string()))?;
                self.write_pack_key("backdrop", Some(&format!("assets/{name}")))
            }
            None => {
                crate::import::forget_image(&self.dir().join("assets"), "backdrop");
                self.write_pack_key("backdrop", None)
            }
        }
    }

    /// Accept key art for this World: write the image inside it, then point the manifest at it.
    ///
    /// The user decides how much immersion a World earns. Epoch can always fall back to the
    /// chart it derives from the geography, so artwork is an *addition* to something that
    /// already works rather than a hole that had to be filled.
    pub fn set_preview(&self, base64_data: &str) -> Result<(), PackError> {
        let name = crate::import::accept_image(&self.dir().join("assets"), "preview", base64_data)
            .map_err(|err| PackError::Rename {
                id: self.id.clone(),
                reason: err.to_string(),
            })?;

        self.write_pack_key("preview", Some(&format!("assets/{name}")))
            .map_err(|_| PackError::Rename {
                id: self.id.clone(),
                reason: "its manifest has no [pack] section to write to".into(),
            })
    }

    /// Drop this World's key art and go back to the derived chart.
    pub fn clear_preview(&self) -> Result<(), PackError> {
        crate::import::forget_image(&self.dir().join("assets"), "preview");
        self.write_pack_key("preview", None)
            .map_err(|_| PackError::Rename {
                id: self.id.clone(),
                reason: "its manifest has no [pack] section to write to".into(),
            })
    }

    /// Give this World a window of its own, or replace the one it has.
    ///
    /// ## The last thing in a World Pack that could only be typed
    ///
    /// The `[[ui]]` pipeline has worked since ADR-0016 and the default pack uses it. What did
    /// not exist was any way to reach it: a person with a drawing of a window had to find
    /// `pack.toml`, work out the nine-slice syntax and edit it by hand — which is the program
    /// this product exists to avoid opening, one file over.
    ///
    /// ## What the author has to say, and what they cannot be asked
    ///
    /// The image is not enough. A nine-slice needs the **corner inset**, and it cannot be read
    /// from the pixels: `[11, 5, 12, 6]` in the default pack was measured off the artwork by the
    /// person who drew it, because a bevel lit from above is not symmetric. `scale` is the same
    /// kind of fact.
    ///
    /// So they are asked for — and *asking somebody for numbers they have no way to choose
    /// between is how a panel stops being a panel*, which is why the surface draws the frame at
    /// two sizes while they move the sliders. The numbers are unguessable in the abstract and
    /// obvious against a picture.
    ///
    /// ## Edited by line, like every other write into a manifest
    ///
    /// A round trip through a TOML serialiser would reformat the file and drop every comment in
    /// it, and the default pack's `[[ui]]` block carries four lines explaining where its insets
    /// came from. Those belong to whoever wrote them. So the block is found by its `concept` and
    /// rewritten in place, or appended whole.
    pub fn set_skin(
        &self,
        concept: &str,
        base64_data: &str,
        corner: [u32; 4],
        repeat: &str,
        scale: u32,
    ) -> Result<(), PackError> {
        // A concept is a dotted name and a file name is not: `ui.frame.window` becomes
        // `window`, which is what the default pack already calls it.
        let stem = concept.rsplit('.').next().unwrap_or("skin");
        let name =
            crate::import::accept_image(&self.dir().join("assets").join("ui"), stem, base64_data)
                .map_err(|err| PackError::Rename {
                id: self.id.clone(),
                reason: err.to_string(),
            })?;

        // The lines, not the text. Joining them is `write_ui_block`'s job, because that is where
        // the file's own line ending is known — the first version built this with `\n`, and the
        // block it wrote was the one part of a CRLF manifest that came out LF.
        let block = [
            "[[ui]]".to_owned(),
            format!("concept = \"{concept}\""),
            format!("image = \"assets/ui/{name}\""),
            format!(
                "corner = [{}, {}, {}, {}]",
                corner[0], corner[1], corner[2], corner[3]
            ),
            format!("repeat = \"{repeat}\""),
            format!("scale = {scale}"),
        ];
        self.write_block("ui", concept, Some(&block))
    }

    /// Give this World a sound of its own, or replace the one it has.
    ///
    /// The same shape as [`Self::set_skin`] and deliberately so: a concept, artwork sniffed from
    /// its bytes, and a block found by its concept rather than its position. What differs is that
    /// a sound has nothing to measure off it — no inset, no scale — so there is nothing to ask
    /// the author beyond the file itself.
    pub fn set_sound(&self, concept: &str, base64_data: &str) -> Result<(), PackError> {
        let stem = concept.rsplit('.').next().unwrap_or("sound");
        let name =
            crate::import::accept_sound(&self.dir().join("assets").join("sfx"), stem, base64_data)
                .map_err(|err| PackError::Rename {
                    id: self.id.clone(),
                    reason: err.to_string(),
                })?;

        let block = [
            "[[sound]]".to_owned(),
            format!("concept = \"{concept}\""),
            format!("file = \"assets/sfx/{name}\""),
        ];
        self.write_block("sound", concept, Some(&block))
    }

    /// Take a World's own sound away, and let Epoch synthesise that one again.
    pub fn clear_sound(&self, concept: &str) -> Result<(), PackError> {
        let stem = concept.rsplit('.').next().unwrap_or("sound");
        crate::import::forget_sound(&self.dir().join("assets").join("sfx"), stem);
        self.write_block("sound", concept, None)
    }

    /// Take a World's own window away, and let Epoch draw its default one again.
    ///
    /// **The artwork is forgotten too**, because a `[[ui]]` block that no longer exists pointing
    /// at a file that still does is a folder that grows every time somebody changes their mind.
    pub fn clear_skin(&self, concept: &str) -> Result<(), PackError> {
        let stem = concept.rsplit('.').next().unwrap_or("skin");
        crate::import::forget_image(&self.dir().join("assets").join("ui"), stem);
        self.write_block("ui", concept, None)
    }

    /// Replace or remove the `[[ui]]` block for one concept, leaving the rest of the file alone.
    ///
    /// **Keyed on the concept, never on position.** A pack may declare several, and the one
    /// being edited is the one whose `concept` matches — the same rule the Engine itself
    /// follows, where the id *is* the contract and a path never is.
    ///
    /// A block runs from its `[[ui]]` line to the next line that starts a table, and the comments
    /// immediately above it belong to it: they are carried along on a replace and removed with it
    /// on a delete, because a comment explaining insets that no longer exist is worse than none.
    fn write_block(
        &self,
        table: &str,
        concept: &str,
        block: Option<&[String]>,
    ) -> Result<(), PackError> {
        let text = std::fs::read_to_string(&self.manifest).map_err(|source| PackError::Io {
            path: self.manifest.clone(),
            source,
        })?;

        let nl = Self::line_ending(&text);
        let lines: Vec<&str> = text.lines().collect();
        let wanted = format!("concept = \"{concept}\"");
        let opener = format!("[[{table}]]");

        // Find the block of this table whose concept matches, if the file has one.
        let mut found: Option<(usize, usize)> = None;
        let mut i = 0;
        while i < lines.len() {
            if lines[i].trim_start().starts_with(opener.as_str()) {
                let mut end = i + 1;
                let mut matches = false;
                while end < lines.len() && !lines[end].trim_start().starts_with('[') {
                    if lines[end].trim() == wanted {
                        matches = true;
                    }
                    end += 1;
                }
                if matches {
                    // Walk back over the comment block that introduces it, and the blank line
                    // above that: they describe this declaration and nothing else.
                    let mut start = i;
                    while start > 0 {
                        let above = lines[start - 1].trim_start();
                        if above.starts_with('#') {
                            start -= 1;
                        } else {
                            break;
                        }
                    }
                    found = Some((start, end));
                    break;
                }
                i = end;
            } else {
                i += 1;
            }
        }

        let mut out = String::with_capacity(text.len() + 256);
        match (found, block) {
            // Replace what is there, in place.
            (Some((start, end)), Some(new)) => {
                for line in &lines[..start] {
                    out.push_str(line);
                    out.push_str(nl);
                }
                for line in new {
                    out.push_str(line);
                    out.push_str(nl);
                }
                for line in &lines[end..] {
                    out.push_str(line);
                    out.push_str(nl);
                }
            }
            // Remove it, and the blank line it leaves behind.
            (Some((start, end)), None) => {
                for line in &lines[..start] {
                    out.push_str(line);
                    out.push_str(nl);
                }
                let mut after = end;
                if lines.get(after).is_some_and(|l| l.trim().is_empty()) {
                    after += 1;
                }
                for line in &lines[after..] {
                    out.push_str(line);
                    out.push_str(nl);
                }
            }
            // Nothing to remove is not a failure: the World already had no window of its own.
            (None, None) => return Ok(()),
            // Append, at the end, with a blank line before it.
            (None, Some(new)) => {
                out.push_str(&text);
                if !out.ends_with(nl) {
                    out.push_str(nl);
                }
                out.push_str(nl);
                for line in new {
                    out.push_str(line);
                    out.push_str(nl);
                }
            }
        }

        std::fs::write(&self.manifest, out).map_err(|source| PackError::Io {
            path: self.manifest.clone(),
            source,
        })
    }

    /// The line ending this file already uses.
    ///
    /// ## Why this is not a detail
    ///
    /// Both writers below rebuild the manifest line by line and used to end every one with
    /// `\n`. On Windows the packs are CRLF, so editing four numbers rewrote **every line in the
    /// file**: measured through the real editor, a `corner = [11, 5, 12, 6]` → `[40, 40, 40, 40]`
    /// change produced a diff of *448 insertions and 448 deletions*.
    ///
    /// That is the same violation `write_pack_key` was written to avoid, one level down. It goes
    /// out of its way not to reformat the file or drop the author's comments — and then silently
    /// rewrote every byte of whitespace between them. **A World Pack lives in somebody's folder
    /// and quite possibly in their git history**, and a four-number edit that touches four
    /// hundred lines is one nobody can review.
    ///
    /// Read from the file rather than from the platform: a pack authored on Linux and opened on
    /// Windows keeps its own endings, which is what *touching nothing else* has to mean.
    fn line_ending(text: &str) -> &'static str {
        if text.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        }
    }

    /// Set, replace or remove one key inside `[pack]`, touching nothing else.
    ///
    /// A whole-file round trip through a TOML serialiser would reformat the manifest and drop
    /// every comment in it — and a World Pack's comments belong to its author. So this edits
    /// lines: replace the key where it already is, or append it to the end of `[pack]`.
    ///
    /// `None` removes the key. Returns `Err(())` only when there is no `[pack]` section at
    /// all, which means the file is not a manifest.
    fn write_pack_key(&self, key: &str, value: Option<&str>) -> Result<(), PackError> {
        let text = std::fs::read_to_string(&self.manifest).map_err(|source| PackError::Io {
            path: self.manifest.clone(),
            source,
        })?;

        let nl = Self::line_ending(&text);
        let mut out = String::with_capacity(text.len() + 96);
        let mut in_pack = false;
        let mut seen_pack = false;
        let mut done = false;
        let line = |v: &str| format!("{key} = \"{v}\"{nl}");

        for raw in text.lines() {
            let trimmed = raw.trim_start();
            if trimmed.starts_with('[') {
                // Leaving [pack] without having found the key: append it here, so a new key
                // lands inside the section rather than under whatever follows.
                if in_pack && !done {
                    if let Some(v) = value {
                        out.push_str(&line(v));
                    }
                    done = true;
                }
                // Only keys directly inside [pack] are ours. Places and characters have names
                // and previews of their own, and they are none of this function's business.
                in_pack = trimmed.starts_with("[pack]");
                seen_pack |= in_pack;
            } else if in_pack && !done && is_key(trimmed, key) {
                if let Some(v) = value {
                    out.push_str(&line(v));
                }
                done = true;
                continue;
            }
            out.push_str(raw);
            out.push_str(nl);
        }

        // The manifest ended while still inside [pack].
        if in_pack && !done {
            if let Some(v) = value {
                out.push_str(&line(v));
            }
            done = true;
        }

        if !seen_pack || !done {
            return Err(PackError::Rename {
                id: self.id.clone(),
                reason: format!("its manifest has no [pack] section to write '{key}' into"),
            });
        }

        std::fs::write(&self.manifest, out).map_err(|source| PackError::Io {
            path: self.manifest.clone(),
            source,
        })
    }

    /// Make a World that does not exist yet, and return its identity.
    ///
    /// ## It is empty on purpose
    ///
    /// No Places, no geography, no crew. A new World is a blank one, and the World Editor is
    /// where it becomes somewhere — which is why creating one goes straight there. Shipping it
    /// with a starter town would mean everybody's first World was somebody else's, and deleting
    /// buildings you did not ask for is a worse first five minutes than placing your own.
    ///
    /// ## The manifest is authored, so it is written to be read
    ///
    /// Hand-written TOML with the fields a person would want to change, rather than a
    /// serialisation of a struct. A World is a folder somebody is meant to be able to open
    /// (ADR-0016), and `rename` already edits this file one line at a time for the same reason.
    ///
    /// ## Licence is mandatory, even for a World nobody will share
    ///
    /// `CONTENT_PHILOSOPHY.md`'s hard rule is enforced at load: a manifest without it fails.
    /// So one is written now, saying what is true — the contents are the user's own.
    pub fn create(dir: &Path, name: &str) -> Result<String, PackError> {
        let name = name.trim();
        let refuse = |id: &str, reason: &str| PackError::Rename {
            id: id.to_string(),
            reason: reason.to_string(),
        };

        if name.is_empty() {
            return Err(refuse("", "a World must have a name"));
        }
        if name
            .chars()
            .any(|c| c.is_control() || c == '"' || c == '\\')
        {
            return Err(refuse(
                name,
                "a name cannot contain quotes, backslashes or line breaks",
            ));
        }

        // The identity comes from the name once, here, and never moves again — same rule a
        // Place follows (ADR-0028). Anything that is not a plain word becomes a separator, so a
        // World called "Bram's Forge ⚒" is a folder every filesystem can hold.
        let stem: String = name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        let stem = stem
            .trim_matches('-')
            .split('-')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let stem = if stem.is_empty() {
            "world".to_string()
        } else {
            stem
        };

        // Never overwrite. A name collision is a second World, not a replacement of the first —
        // the same rule Character Pack import follows (ADR-0026).
        let mut id = stem.clone();
        let mut n = 2;
        while dir.join(&id).exists() {
            id = format!("{stem}-{n}");
            n += 1;
        }

        let home = dir.join(&id);
        std::fs::create_dir_all(&home).map_err(|source| PackError::Io {
            path: home.clone(),
            source,
        })?;

        let manifest = home.join("pack.toml");
        let body = format!(
            "# A World you made. This file is yours to edit.\n\
             #\n\
             # Places, roads and the land live in places.toml, written by the World Editor.\n\
             \n\
             [pack]\n\
             id = \"{id}\"\n\
             name = \"{name}\"\n\
             version = \"0.1.0\"\n\
             \n\
             # Mandatory, and checked when this World loads.\n\
             [license]\n\
             type = \"proprietary\"\n\
             holder = \"the author of this World\"\n\
             notes = \"Made in Epoch. Its contents belong to whoever made them.\"\n"
        );
        std::fs::write(&manifest, body).map_err(|source| PackError::Io {
            path: manifest,
            source,
        })?;

        Ok(id)
    }

    /// Find every World installed under a directory.
    ///
    /// Discovery reads the filesystem, whose enumeration order belongs to the operating
    /// system — so the result is **sorted by identity** before it is returned. A list of
    /// Worlds that came back in a different order on a different machine would be the same
    /// class of bug as a `HashMap` deciding draw order.
    ///
    /// A World that fails to load is reported and skipped. One broken World must never stop
    /// the others from being offered.
    pub fn discover(dir: &Path) -> (Vec<WorldPack>, Vec<String>) {
        let mut found = Vec::new();
        let mut problems = Vec::new();

        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => {
                problems.push(format!("cannot read Worlds at {}: {err}", dir.display()));
                return (found, problems);
            }
        };

        for entry in entries.flatten() {
            let manifest = entry.path().join("pack.toml");
            if !manifest.is_file() {
                continue;
            }
            match WorldPack::load(&manifest) {
                Ok(pack) => found.push(pack),
                Err(err) => problems.push(err.to_string()),
            }
        }

        found.sort_by(|a, b| a.id.cmp(&b.id));
        problems.sort();
        (found, problems)
    }

    /// Fraction of the concepts the engine knows that this pack supplies.
    ///
    /// The pack's coverage Profile in its simplest honest form — enough to tell a user "this
    /// pack covers 60%", which is what ADR-0016 asks for.
    /// Measured over **place concepts only**. Character archetypes left this sum with the
    /// cast (ADR-0023): a World cannot cover something it no longer supplies, and counting it
    /// would have made every World look permanently half-finished.
    pub fn coverage(&self) -> f32 {
        let covered: BTreeSet<&'static str> = self
            .places
            .values()
            .filter_map(|c| c.concept)
            .map(|c| c.id())
            .collect();
        covered.len() as f32 / PlaceConcept::ALL.len() as f32
    }
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// Whether a trimmed line assigns `key`, tolerating any spacing around the `=`.
fn is_key(line: &str, key: &str) -> bool {
    line.strip_prefix(key)
        .map(|rest| rest.trim_start().starts_with('='))
        .unwrap_or(false)
}

/// An ordered chain of packs: active first, base next, default last.
///
/// Resolution walks the chain and always terminates in a visible placeholder, so a
/// missing concept degrades gracefully instead of crashing or rendering nothing.
///
/// The chain is an **explicit ordered list**, never the result of a directory scan.
/// Filesystem enumeration order belongs to the operating system, and a World that composed
/// differently on a different machine would not be deterministic.
#[derive(Debug, Clone, Default)]
pub struct WorldPackChain {
    packs: Vec<WorldPack>,
}

impl WorldPackChain {
    /// True when any pack in this chain has been written since it was read.
    pub fn changed_on_disk(&self) -> bool {
        self.packs.iter().any(WorldPack::changed_on_disk)
    }

    /// Where each pack in this chain lives, highest precedence first.
    pub fn manifests(&self) -> Vec<PathBuf> {
        self.packs
            .iter()
            .map(|pack| pack.manifest.clone())
            .collect()
    }

    /// What this World's interface looks like, concept by concept.
    ///
    /// **The fallback chain, unchanged** (ADR-0016): the highest-precedence pack that declares a
    /// concept supplies it, and a concept nobody declares is simply absent — which is not a gap.
    /// Epoch draws its own windows, so an undeclared concept means the drawn one, and a World
    /// with no artwork at all is complete rather than unfinished.
    pub fn ui(&self) -> BTreeMap<String, Skin> {
        let mut skin = BTreeMap::new();
        // Lowest precedence first, so the highest overwrites — the same order everything else in
        // this chain composes in.
        for pack in self.packs.iter().rev() {
            for (concept, supplied) in &pack.ui {
                skin.insert(concept.clone(), supplied.clone());
            }
        }
        skin
    }

    /// What this World sounds like, composed like everything else in the chain.
    pub fn sounds(&self) -> BTreeMap<String, String> {
        let mut found = BTreeMap::new();
        for pack in self.packs.iter().rev() {
            for (concept, supplied) in &pack.sounds {
                found.insert(concept.clone(), supplied.clone());
            }
        }
        found
    }

    /// Build a chain from packs ordered by precedence (highest first).
    pub fn new(packs: Vec<WorldPack>) -> Self {
        Self { packs }
    }

    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }

    /// The pack that answers first, if any. Useful for reporting which world is active.
    pub fn active(&self) -> Option<&WorldPack> {
        self.packs.first()
    }

    // A chain no longer answers anything about characters. It used to name and dress each
    // archetype, which only worked while a World owned the cast. The cast is the user's
    // (ADR-0023), so a character's name and face come from their Definition and the chain
    // answers about *places* alone.

    /// The geography of the first pack in the chain that declares one.
    ///
    /// A chain with no map at all is valid: the World still renders and still names its
    /// Places — it simply has no land yet.
    pub fn map(&self) -> Option<&WorldMap> {
        self.packs.iter().find_map(|p| p.map.as_ref())
    }

    /// Every Place identity any contributor mentions, in a stable order.
    pub fn place_ids(&self) -> Vec<PlaceId> {
        let ids: BTreeSet<&str> = self
            .packs
            .iter()
            .flat_map(|p| p.places.keys().map(String::as_str))
            .collect();
        ids.into_iter().map(PlaceId::new).collect()
    }

    /// Every contribution to one Place, **in precedence order, most specific first**.
    ///
    /// This is the input to `place::compose`. The chain decides the order; the composition
    /// itself is a pure function of it.
    pub fn contributions(&self, id: &PlaceId) -> Vec<&PlaceContribution> {
        self.packs
            .iter()
            .filter_map(|p| p.places.get(id.as_str()))
            .collect()
    }

    /// Every problem across the chain, in a stable order, for reporting.
    pub fn problems(&self) -> Vec<String> {
        self.packs
            .iter()
            .flat_map(|p| p.problems().iter().cloned())
            .collect()
    }
}

#[cfg(test)]
impl WorldPack {
    /// A World whose only content is one road, for tests elsewhere in this crate.
    ///
    /// Here rather than in `world.rs` because `places` and `problems` are private, and they
    /// stay private: a pack is built by reading a manifest, and a second way to build one is a
    /// second thing that can disagree with the file.
    pub(crate) fn joining(from: &str, to: &str) -> Self {
        Self {
            id: "test".into(),
            name: "test".into(),
            version: "0.0.0".into(),
            license: License {
                kind: "CC0-1.0".into(),
                holder: "test".into(),
                notes: None,
            },
            kind: None,
            description: None,
            preview: None,
            backdrop: None,
            manifest: PathBuf::from("pack.toml"),
            places: BTreeMap::new(),
            map: Some(crate::map::WorldMap {
                size: crate::map::WorldSize {
                    width: 1000.0,
                    height: 1000.0,
                },
                terrain: Vec::new(),
                routes: vec![crate::map::Route {
                    from: from.into(),
                    to: to.into(),
                    prominence: crate::map::Prominence::Minor,
                }],
            }),
            ui: BTreeMap::new(),
            sounds: BTreeMap::new(),
            problems: Vec::new(),
            print: None,
        }
    }
}

#[cfg(test)]
mod noticing_that_the_artwork_changed {
    use super::*;

    fn a_pack(dir: &Path, preview: &str) -> PathBuf {
        std::fs::create_dir_all(dir.join("art")).unwrap();
        std::fs::write(dir.join("art/key.png"), preview).unwrap();
        let manifest = dir.join("pack.toml");
        std::fs::write(
            &manifest,
            concat!(
                "[pack]
",
                "id = \"w\"
",
                "name = \"W\"
",
                "version = \"1\"
",
                "preview = \"art/key.png\"

",
                "[license]
",
                "type = \"CC0\"
",
                "holder = \"nobody\"
",
            ),
        )
        .unwrap();
        manifest
    }

    /// **Editing a sprite is the thing somebody does over and over**, and it was the half hot
    /// reload left out: a character reached the next turn, artwork needed the World left and
    /// entered again.
    #[test]
    fn a_redrawn_sprite_is_noticed_and_an_untouched_pack_is_not() {
        let dir = std::env::temp_dir().join(format!("epoch-pack-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let manifest = a_pack(&dir, "first");

        let pack = WorldPack::load(&manifest).expect("it loads");
        assert!(!pack.changed_on_disk(), "nothing has been written since");

        // Redrawn in another window. The pause is for the clock, not for the disk: NTFS keeps
        // modification times to 100 ns and ext4 to a nanosecond, so twenty milliseconds is four
        // orders of magnitude of room — but writing twice inside one tick of a *coarse* clock
        // would be a test that passes here and fails on somebody's FAT32 stick.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.join("art/key.png"), "second and different").unwrap();

        assert!(
            pack.changed_on_disk(),
            "a sprite written after the pack was read is a change"
        );

        // **Putting it back is a change too, and the first version of this missed it.** Found by
        // renaming a Place in a running World: the rename arrived in half a second and the
        // restore never did, because the sweep was a *maximum* mtime and a maximum only climbs.
        // Undo, a backup, `git checkout` — all of them write something older, and all of them are
        // what somebody editing artwork does.
        let reloaded = WorldPack::load(&manifest).expect("it loads again");
        assert!(!reloaded.changed_on_disk());
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.join("art/key.png"), "first").unwrap();
        let older = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        let _ = std::fs::File::open(dir.join("art/key.png")).map(|f| f.set_modified(older));
        assert!(
            reloaded.changed_on_disk(),
            "an older file is a change, or an undo never reaches the World"
        );

        // And a file taken away, which a maximum cannot see either.
        let after = WorldPack::load(&manifest).expect("it loads again");
        assert!(!after.changed_on_disk());
        std::fs::remove_file(dir.join("art/key.png")).unwrap();
        assert!(after.changed_on_disk(), "a deleted file is a change");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A folder that cannot be read is a question that could not be asked.
    ///
    /// Reading it as *changed* would reload the World twice a second forever — the same
    /// inversion as reading silence as a refusal, with a heartbeat attached.
    #[test]
    fn a_pack_whose_folder_went_away_does_not_claim_it_changed() {
        let dir = std::env::temp_dir().join(format!("epoch-pack-gone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let manifest = a_pack(&dir, "first");
        let pack = WorldPack::load(&manifest).expect("it loads");

        std::fs::remove_dir_all(&dir).unwrap();
        assert!(!pack.changed_on_disk(), "unreadable is not changed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::compose;

    fn pack(id: &str, places: &[(&str, PlaceContribution)]) -> WorldPack {
        WorldPack {
            id: id.into(),
            name: id.into(),
            version: "0.0.0".into(),
            license: License {
                kind: "CC0-1.0".into(),
                holder: "test".into(),
                notes: None,
            },
            kind: None,
            description: None,
            preview: None,
            backdrop: None,
            manifest: PathBuf::from("pack.toml"),
            places: places
                .iter()
                .map(|(k, v)| ((*k).to_string(), v.clone()))
                .collect(),
            map: None,
            ui: BTreeMap::new(),
            sounds: BTreeMap::new(),
            problems: Vec::new(),
            print: None,
        }
    }

    /// A pack that also draws its own windows.
    fn skinned(id: &str, concept: &str, image: &str, corner: u32) -> WorldPack {
        let mut supplied = pack(id, &[]);
        supplied.ui.insert(
            concept.to_string(),
            Skin {
                image: image.into(),
                corner: [corner; 4],
                repeat: SkinRepeat::Stretch,
                scale: 1,
                min_size: [corner * 3, corner * 3],
            },
        );
        supplied
    }

    /// A pack that dresses Epoch’s own window does so through the whole pipeline.
    ///
    /// **Asserted against a fixture, because the shipped Worlds carry no artwork.** This read
    /// `packs/default`, and the moment that pack stopped declaring a skin — the artwork left
    /// the repository — the claim had nothing left to be made about. The artwork had been
    /// load-bearing as *evidence*, which is a thing worth noticing before deleting anything:
    /// `tests/fixtures/pack` exists to carry that evidence and is nobody’s content.
    ///
    /// Every number below is measured off `assets/window.png`, exactly as the authoring
    /// guidance requires — a bevel lit from above is not symmetric, and the insets say so.
    #[test]
    fn a_pack_that_supplies_a_window_draws_it_through_the_whole_pipeline() {
        let pack = WorldPack::load(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pack/pack.toml"),
        )
        .expect("the fixture World must load");
        assert!(pack.problems().is_empty(), "{:?}", pack.problems());

        let window = pack
            .ui
            .get("ui.frame.window")
            .expect("the fixture World supplies a window");

        // Read, confined to the World, and delivered as a data URI like every other Asset.
        assert!(window.image.starts_with("data:image/png;base64,"));
        // A bevel lit from above is not symmetric, and this is what it measured.
        assert_eq!(window.corner, [4, 2, 6, 3]);
        assert_eq!(window.scale, 1);
        assert_eq!(window.repeat, SkinRepeat::Stretch);
        // The floor is the drawing's, not a layout's: two opposite corners plus a middle at least as
        // long as the larger of them.
        assert_eq!(window.min_size, [3 + 2 + 3, 4 + 6 + 6]);
    }

    /// The interface is content, and it resolves like all the other content.
    #[test]
    fn the_highest_pack_that_draws_a_window_supplies_it() {
        // The fallback chain, unchanged (ADR-0016). A World laid over another takes the windows
        // it drew and leaves the ones it did not.
        let chain = WorldPackChain::new(vec![
            skinned("over", "ui.frame.window", "data:image/png;base64,OVER", 12),
            skinned("under", "ui.frame.window", "data:image/png;base64,UNDER", 8),
            skinned("under", "ui.frame.tooltip", "data:image/png;base64,TIP", 4),
        ]);

        let skin = chain.ui();
        assert_eq!(skin["ui.frame.window"].image, "data:image/png;base64,OVER");
        assert_eq!(skin["ui.frame.window"].corner, [12; 4]);
        // Not overwritten by the pack above it, because that pack never mentioned it.
        assert_eq!(skin["ui.frame.tooltip"].image, "data:image/png;base64,TIP");
    }

    #[test]
    fn a_world_that_drew_no_windows_is_complete_rather_than_unfinished() {
        // Every World today. Epoch draws its own windows, so an empty answer is the ordinary
        // one and never a gap to be filled or an error to be reported.
        let chain = WorldPackChain::new(vec![pack("plain", &[])]);
        assert!(chain.ui().is_empty());
    }

    fn named(concept: PlaceConcept, title: &str) -> PlaceContribution {
        PlaceContribution {
            concept: Some(concept),
            title: Some(title.into()),
            ..Default::default()
        }
    }

    #[test]
    fn an_empty_chain_mentions_no_places_at_all() {
        // And that is a valid World: the projection still shows every concept the engine
        // knows, as a visible placeholder (ADR-0016).
        let chain = WorldPackChain::default();
        assert!(chain.place_ids().is_empty());
    }

    #[test]
    fn the_first_pack_in_the_chain_wins_for_scalars() {
        let chain = WorldPackChain::new(vec![
            pack(
                "active",
                &[(
                    "research_lab",
                    named(PlaceConcept::ResearchLab, "Active Lab"),
                )],
            ),
            pack(
                "default",
                &[(
                    "research_lab",
                    named(PlaceConcept::ResearchLab, "Default Lab"),
                )],
            ),
        ]);
        let id = PlaceId::new("research_lab");
        let place = compose(&id, &chain.contributions(&id)).unwrap();
        assert_eq!(place.title, "Active Lab");
    }

    #[test]
    fn a_gap_in_the_active_pack_falls_through_to_the_next() {
        let sparse = PlaceContribution {
            subtitle: Some("reskinned".into()),
            ..Default::default()
        };
        let chain = WorldPackChain::new(vec![
            pack("active", &[("research_lab", sparse)]),
            pack(
                "default",
                &[(
                    "research_lab",
                    named(PlaceConcept::ResearchLab, "Default Lab"),
                )],
            ),
        ]);
        let id = PlaceId::new("research_lab");
        let place = compose(&id, &chain.contributions(&id)).unwrap();

        // The reskin said nothing about the name, so the base still shows through — which is
        // what keeps a partial contributor from having to redeclare a whole Place.
        assert_eq!(place.title, "Default Lab");
        assert_eq!(place.subtitle.as_deref(), Some("reskinned"));
        assert!(!place.is_placeholder);
    }

    #[test]
    fn place_identities_enumerate_in_a_stable_order() {
        // The determinism guarantee starts here: if this order varied, so would draw order,
        // reports and every downstream cache.
        let chain = WorldPackChain::new(vec![
            pack(
                "active",
                &[
                    ("guild", named(PlaceConcept::Guild, "G")),
                    ("command_center", named(PlaceConcept::CommandCenter, "C")),
                ],
            ),
            pack(
                "default",
                &[("research_lab", named(PlaceConcept::ResearchLab, "L"))],
            ),
        ]);
        let ids: Vec<String> = chain.place_ids().iter().map(|i| i.to_string()).collect();
        assert_eq!(ids, vec!["command_center", "guild", "research_lab"]);
    }

    #[test]
    fn renaming_changes_the_name_keeps_the_identity_and_preserves_the_file() {
        let dir = std::env::temp_dir().join("epoch-rename-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pack.toml");
        std::fs::write(
            &path,
            "# why this World is shaped the way it is\n\
             [pack]\n\
             id = \"keepme\"\n\
             name   =    \"Old Name\"\n\
             version = \"0.1.0\"\n\n\
             [license]\n\
             type = \"CC0-1.0\"\n\
             holder = \"test\"\n\n\
             # the Guild is named here, and must not be touched\n\
             [places.guild]\n\
             concept = \"guild\"\n\
             name = \"not the World's name\"\n",
        )
        .unwrap();

        let pack = WorldPack::load(&path).unwrap();
        pack.rename("  A New Name  ").unwrap();

        let after = std::fs::read_to_string(&path).unwrap();
        assert!(after.contains("name = \"A New Name\""), "{after}");
        assert!(
            after.contains("id = \"keepme\""),
            "identity must survive a rename"
        );
        // Comments are the reasoning behind a World. Re-serialising would delete them.
        assert!(after.contains("# why this World is shaped the way it is"));
        assert!(after.contains("# the Guild is named here"));
        // Only the [pack] name changes; a Place's own name is none of its business.
        assert!(after.contains("name = \"not the World's name\""));

        let reloaded = WorldPack::load(&path).unwrap();
        assert_eq!(reloaded.name, "A New Name");
        assert_eq!(reloaded.id, "keepme");

        assert!(matches!(
            reloaded.rename("   ").unwrap_err(),
            PackError::Rename { .. }
        ));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_manifest_without_a_license_is_rejected() {
        // CONTENT_PHILOSOPHY.md hard rule, enforced rather than merely documented.
        let dir = std::env::temp_dir().join("epoch-pack-license-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("pack.toml");
        std::fs::write(
            &path,
            "[pack]\nid = \"nolicense\"\nname = \"No License\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();

        let err = WorldPack::load(&path).unwrap_err();
        assert!(matches!(err, PackError::MissingLicense { .. }));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_new_world_is_empty_and_loads() {
        // Empty on purpose: the World Editor is where it becomes somewhere. A starter town
        // would mean everybody's first World was somebody else's.
        let dir = std::env::temp_dir().join(format!("epoch-new-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let id = WorldPack::create(&dir, "Bram's Forge").unwrap();
        assert_eq!(id, "bram-s-forge", "an identity every filesystem can hold");

        let pack = WorldPack::load(&dir.join(&id).join("pack.toml")).unwrap();
        assert_eq!(
            pack.name, "Bram's Forge",
            "and the name they actually chose"
        );
        assert_eq!(pack.declared(), 0, "no Places until somebody builds one");
        assert!(pack.map.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn creating_a_world_twice_makes_two_worlds() {
        // A name collision is a second World, never a replacement of the first — the same rule
        // Character Pack import follows (ADR-0026). Overwriting would delete somebody's map.
        let dir = std::env::temp_dir().join(format!("epoch-twice-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let first = WorldPack::create(&dir, "Home").unwrap();
        let second = WorldPack::create(&dir, "Home").unwrap();

        assert_ne!(first, second);
        assert!(dir.join(&first).join("pack.toml").is_file());
        assert!(dir.join(&second).join("pack.toml").is_file());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_world_cannot_be_created_without_a_name() {
        let dir = std::env::temp_dir().join(format!("epoch-noname-{}", std::process::id()));
        assert!(WorldPack::create(&dir, "   ").is_err());
    }
}

#[cfg(test)]
mod dressing_a_world {
    use super::*;

    /// A one-pixel PNG, so the real import door runs rather than a stand-in for it.
    const PNG: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    /// A pack whose `[[ui]]` block carries the comments its author wrote above it.
    fn pack_with_a_window(dir: &Path) -> PathBuf {
        let manifest = dir.join("pack.toml");
        std::fs::write(
            &manifest,
            "[pack]\n\
             id = \"test\"\n\
             name = \"Test\"\n\
             version = \"0.1.0\"\n\
             \n\
             [license]\n\
             type = \"CC0\"\n\
             holder = \"Test\"\n\
             \n\
             # The insets are measured from the artwork, not chosen.\n\
             #   top 11 \u{b7} right 5 \u{b7} bottom 12 \u{b7} left 6\n\
             [[ui]]\n\
             concept = \"ui.frame.window\"\n\
             image = \"assets/ui/window.png\"\n\
             corner = [11, 5, 12, 6]\n\
             repeat = \"stretch\"\n\
             scale = 1\n\
             \n\
             [places.knowledge_center]\n\
             name = \"The Library\"\n",
        )
        .unwrap();
        manifest
    }

    fn read(pack: &WorldPack) -> String {
        std::fs::read_to_string(&pack.manifest).unwrap()
    }

    fn open(manifest: PathBuf) -> WorldPack {
        // Loaded directly rather than discovered. The first version discovered, and when the
        // fixtures had the licence shape wrong every test failed with *the pack was written* --
        // a directory listing reporting emptiness rather than the manifest saying what was
        // wrong with it. A test's failure is read far more often than its assertion.
        WorldPack::load(&manifest).expect("the manifest loads")
    }

    fn tree(name: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("epoch-skin-{name}-{n}"));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("world")).unwrap();
        root
    }

    /// **The property this writer exists to protect.**
    ///
    /// A round trip through a TOML serialiser would reformat the file and drop every comment in
    /// it. The default pack's `[[ui]]` block carries four lines explaining where its insets came
    /// from, and those belong to whoever wrote them — the same reason `write_pack_key` edits
    /// lines instead of re-serialising.
    #[test]
    fn replacing_a_window_keeps_the_rest_of_the_manifest() {
        let root = tree("replace");
        let pack = open(pack_with_a_window(&root.join("world")));

        pack.set_skin("ui.frame.window", PNG, [4, 4, 4, 4], "repeat", 2)
            .expect("a window was accepted");

        let after = read(&pack);
        assert!(after.contains("corner = [4, 4, 4, 4]"), "{after}");
        assert!(after.contains("repeat = \"repeat\""), "{after}");
        assert!(after.contains("scale = 2"), "{after}");
        // Everything that was not being edited is still there, in order.
        assert!(after.contains("id = \"test\""), "{after}");
        assert!(after.contains("[places.knowledge_center]"), "{after}");
        assert!(after.contains("The Library"), "{after}");
        // Exactly one declaration: replaced, never appended beside the old one.
        assert_eq!(after.matches("[[ui]]").count(), 1, "{after}");
    }

    /// A pack that never had one gets a block appended, and keeps everything it did have.
    #[test]
    fn a_world_with_no_window_gains_one() {
        let root = tree("append");
        let dir = root.join("world");
        std::fs::write(
            dir.join("pack.toml"),
            "[pack]\nid = \"bare\"\nname = \"Bare\"\nversion = \"0.1.0\"\n\n[license]\ntype = \"CC0\"\nholder = \"Test\"\n",
        )
        .unwrap();
        let pack = open(dir.join("pack.toml"));

        pack.set_skin("ui.frame.window", PNG, [3, 3, 3, 3], "stretch", 1)
            .expect("a window was accepted");

        let after = read(&pack);
        assert_eq!(after.matches("[[ui]]").count(), 1, "{after}");
        assert!(after.contains("name = \"Bare\""), "{after}");
    }

    /// **Taking the window away takes the artwork with it.**
    ///
    /// A declaration that no longer exists pointing at a file that still does is a folder that
    /// grows every time somebody changes their mind — and the next import would land beside it
    /// under a different name rather than replacing it.
    #[test]
    fn clearing_a_window_removes_the_block_and_the_file() {
        let root = tree("clear");
        let pack = open(pack_with_a_window(&root.join("world")));
        pack.set_skin("ui.frame.window", PNG, [2, 2, 2, 2], "stretch", 1)
            .unwrap();

        let ui_dir = pack.dir().join("assets").join("ui");
        assert!(
            std::fs::read_dir(&ui_dir).unwrap().count() > 0,
            "the artwork landed"
        );

        pack.clear_skin("ui.frame.window").expect("cleared");

        let after = read(&pack);
        assert!(!after.contains("[[ui]]"), "{after}");
        assert!(
            after.contains("[places.knowledge_center]"),
            "the rest survives: {after}"
        );
        assert_eq!(
            std::fs::read_dir(&ui_dir).map(|d| d.count()).unwrap_or(0),
            0,
            "the artwork went with it"
        );
    }

    /// Clearing something that was never there is not a failure.
    ///
    /// It is the ordinary state of every World: Epoch draws its own windows, and a pack that
    /// supplies none is complete rather than unfinished.
    #[test]
    fn clearing_a_window_that_was_never_there_is_fine() {
        let root = tree("nothing");
        let dir = root.join("world");
        std::fs::write(
            dir.join("pack.toml"),
            "[pack]\nid = \"bare\"\nname = \"Bare\"\nversion = \"0.1.0\"\n\n[license]\ntype = \"CC0\"\nholder = \"Test\"\n",
        )
        .unwrap();
        let pack = open(dir.join("pack.toml"));
        assert!(pack.clear_skin("ui.frame.window").is_ok());
    }

    /// A World can be given a voice, and the same writer serves both tables.
    ///
    /// `[[ui]]` and `[[sound]]` are one function with a table name, which is what stops the two
    /// from drifting into two rules about how a manifest is edited. This asserts the second one
    /// actually works rather than trusting that a parameter was threaded through.
    #[test]
    fn a_world_can_be_given_a_voice() {
        const WAV: &str =
            "data:audio/wav;base64,UklGRiQAAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YQAAAAA=";

        let root = tree("voice");
        let pack = open(pack_with_a_window(&root.join("world")));

        pack.set_sound("sfx.click", WAV)
            .expect("a sound was accepted");

        let after = read(&pack);
        assert!(after.contains("[[sound]]"), "{after}");
        assert!(after.contains("concept = \"sfx.click\""), "{after}");
        assert!(after.contains("assets/sfx/click.wav"), "{after}");
        // The window is untouched: two tables, and editing one is not editing the other.
        assert!(after.contains("corner = [11, 5, 12, 6]"), "{after}");
        assert_eq!(after.matches("[[ui]]").count(), 1, "{after}");

        // And a reload resolves it, so the surface is handed a playable URI rather than a path.
        let reloaded = WorldPack::load(&pack.manifest).expect("still loads");
        assert!(
            reloaded
                .sounds
                .get("sfx.click")
                .is_some_and(|uri| uri.starts_with("data:audio/wav;base64,")),
            "{:?}",
            reloaded.sounds
        );

        let sfx_dir = pack.dir().join("assets").join("sfx");
        pack.clear_sound("sfx.click").expect("cleared");
        let after = read(&pack);
        assert!(!after.contains("[[sound]]"), "{after}");
        assert_eq!(
            std::fs::read_dir(&sfx_dir).map(|d| d.count()).unwrap_or(0),
            0,
            "the audio went with it"
        );
    }

    /// A picture is not a click, and the door says so.
    ///
    /// Not because a PNG is dangerous \u2014 because a picture filed as a sound is a failure nobody
    /// can diagnose from where it surfaces, which is a browser silently playing nothing.
    #[test]
    fn a_sound_that_is_not_a_sound_is_refused() {
        let root = tree("notasound");
        let pack = open(pack_with_a_window(&root.join("world")));
        assert!(pack.set_sound("sfx.click", PNG).is_err());
    }

    /// **A manifest keeps the line endings it had.**
    ///
    /// Found by reading a `git diff` after the first end-to-end run, not by a test: editing four
    /// numbers in the CRLF pack that ships here produced *448 insertions and 448 deletions*,
    /// because the writer ended every line it rebuilt with `\n`. Five tests passed the whole
    /// time — they asserted on content, and nothing asserted on whitespace.
    ///
    /// That is the same blind spot this repository already has a note about, one subsystem over:
    /// eleven prompts were quietly mangled by scripted edits and 1113 tests never saw one.
    ///
    /// The cost is not tidiness. A World Pack lives in somebody's folder and quite possibly in
    /// their git history, and a four-number change that touches four hundred lines is one nobody
    /// can review.
    #[test]
    fn a_manifest_keeps_the_line_endings_it_had() {
        let root = tree("endings");
        let dir = root.join("world");
        let crlf = "[pack]\r\nid = \"crlf\"\r\nname = \"CRLF\"\r\nversion = \"0.1.0\"\r\n\r\n\
                    [license]\r\ntype = \"CC0\"\r\nholder = \"Test\"\r\n";
        std::fs::write(dir.join("pack.toml"), crlf).unwrap();
        let pack = open(dir.join("pack.toml"));

        pack.set_skin("ui.frame.window", PNG, [2, 2, 2, 2], "stretch", 1)
            .unwrap();

        let after = std::fs::read_to_string(pack.manifest.clone()).unwrap();
        assert!(
            after.contains("corner = [2, 2, 2, 2]"),
            "the edit landed: {after:?}"
        );
        assert!(
            !after.replace("\r\n", "").contains('\n'),
            "every line still ends CRLF: {after:?}"
        );

        // And the other direction: a pack authored with LF must not gain carriage returns
        // because it was opened on Windows.
        let lf = tree("endings-lf");
        let lf_dir = lf.join("world");
        std::fs::write(
            lf_dir.join("pack.toml"),
            "[pack]\nid = \"lf\"\nname = \"LF\"\nversion = \"0.1.0\"\n\n[license]\ntype = \"CC0\"\nholder = \"Test\"\n",
        )
        .unwrap();
        let pack = open(lf_dir.join("pack.toml"));
        pack.set_skin("ui.frame.window", PNG, [2, 2, 2, 2], "stretch", 1)
            .unwrap();
        let after = std::fs::read_to_string(pack.manifest.clone()).unwrap();
        assert!(
            !after.contains('\r'),
            "no carriage returns appeared: {after:?}"
        );
    }

    /// Two concepts are two declarations, and editing one leaves the other alone.
    ///
    /// **Keyed on the concept, never on position.** The id *is* the contract (ADR-0016), and a
    /// writer that edited "the first `[[ui]]`" would corrupt the second one the day a pack
    /// declares a tooltip as well as a window.
    #[test]
    fn one_concept_is_edited_without_touching_another() {
        let root = tree("two");
        let pack = open(pack_with_a_window(&root.join("world")));
        pack.set_skin("ui.frame.tooltip", PNG, [1, 1, 1, 1], "stretch", 1)
            .unwrap();
        pack.set_skin("ui.frame.window", PNG, [9, 9, 9, 9], "round", 3)
            .unwrap();

        let after = read(&pack);
        assert_eq!(after.matches("[[ui]]").count(), 2, "{after}");
        assert!(
            after.contains("corner = [1, 1, 1, 1]"),
            "tooltip kept: {after}"
        );
        assert!(
            after.contains("corner = [9, 9, 9, 9]"),
            "window changed: {after}"
        );
    }
}
