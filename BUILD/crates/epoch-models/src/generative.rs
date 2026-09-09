//! The Generative Library — the one place on this machine Epoch's own assets live (ADR-0032).
//!
//! ## Why this is not the vault, and not a World
//!
//! A 6 GB checkpoint is a **fact about a machine**, in the same category as `num_gpu` (ADR-0026)
//! and for the same reason: it does not travel with anybody. Worlds reference it; none of them
//! contains it. So there is one library per installation, it lives beside the platform's other
//! application data, and it deliberately does **not** follow `Paths::data` — which, in a source
//! tree, is the source tree, and twenty gigabytes of weights do not belong in a repository.
//!
//! It lives here rather than in `epoch-engine` because a lent machine is the machine an asset
//! would be installed *onto* (ADR-0029 §9, Phase 11.12), and EpochServices does not link the
//! Engine.
//!
//! ## Epoch adds a search path; it never moves a file
//!
//! ComfyUI is told where the library is. Nothing is copied into ComfyUI's tree, nothing the user
//! already had is moved, renamed or adopted. Adding a line to a config file has an inverse;
//! copying 24 GB does not.
//!
//! ## Measured, 2026-08-23, against ComfyUI v0.33.3
//!
//! Three questions, asked of the program rather than remembered:
//!
//! | asked | answered |
//! |---|---|
//! | is `extra_model_paths.yaml` beside `main.py` read at all? | **yes** — `Adding extra search path checkpoints …` |
//! | is a **new file** in an existing search path seen without a restart? | **yes**, within seconds |
//! | is a **new search path** seen without a restart? | **no** |
//!
//! Read afterwards in `main.py` and it agrees: the file beside `main.py` is loaded unconditionally
//! at boot, before any `--extra-model-paths-config` argument. So the handshake happens **once**,
//! it needs one restart at that moment, and after it every asset that arrives is visible
//! immediately — which is the arrangement worth having, and the reason this was measured before
//! anything was built on it.
//!
//! Comfy Desktop generates its own `instance-model-paths/<id>.yaml` and stamps it *do not edit
//! manually*; it passes that as an argument. Epoch writes a different file, the one ComfyUI reads
//! on its own, so neither program overwrites the other.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// A shelf in the library.
///
/// The name Epoch uses, and the name ComfyUI uses for the same thing. They differ — `vaes` reads
/// like a folder and `vae` is what a search path is keyed by — and keeping both here means the
/// mapping exists in exactly one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Shelf {
    Models,
    /// A diffusion model on its own — Flux, Z-Image. Its own shelf because ComfyUI loads it
    /// through a different node, and a checkpoint list that contained one would offer a render
    /// that cannot start.
    DiffusionModels,
    /// `t5xxl`, `clip_l`. Loaded beside a diffusion model.
    TextEncoders,
    Loras,
    Vaes,
    Upscalers,
    ControlNet,
    Embeddings,
    Workflows,
    /// Piper voices. **The one shelf ComfyUI is never told about** — `comfy_key` answers `None`,
    /// exactly as `Workflows` does, and for the same reason: nothing here is weights ComfyUI
    /// loads.
    ///
    /// Deliberately not `vault/models`, where the text GGUFs live. A voice filed there would
    /// appear in the dropdown that picks a character's brain.
    Voices,
    /// RVC voices: the `.pth` somebody downloaded and the `.onnx` Epoch converted from it.
    ///
    /// **Its own shelf, and not `Voices`.** Both are `.onnx`, so one folder would make a
    /// timbre appear in the list of voices a character can *speak* with — and Piper, handed
    /// one, fails at the moment somebody talks. Two things that are only distinguishable by
    /// what happens to be beside them should not share a folder; the folder is the cheap half
    /// of the answer.
    Timbres,
}

impl Shelf {
    pub const ALL: [Shelf; 11] = [
        Shelf::Models,
        Shelf::DiffusionModels,
        Shelf::TextEncoders,
        Shelf::Loras,
        Shelf::Vaes,
        Shelf::Upscalers,
        Shelf::ControlNet,
        Shelf::Embeddings,
        Shelf::Workflows,
        Shelf::Voices,
        Shelf::Timbres,
    ];

    /// The folder, under the library root.
    pub const fn folder(self) -> &'static str {
        match self {
            Shelf::Models => "models",
            Shelf::DiffusionModels => "diffusion_models",
            Shelf::TextEncoders => "text_encoders",
            Shelf::Loras => "loras",
            Shelf::Vaes => "vaes",
            Shelf::Upscalers => "upscalers",
            Shelf::ControlNet => "controlnet",
            Shelf::Embeddings => "embeddings",
            Shelf::Workflows => "workflows",
            Shelf::Voices => "voices",
            Shelf::Timbres => "timbres",
        }
    }

    /// Which shelf a thing of that kind belongs on.
    ///
    /// One place, so a download and a listing can never disagree about where something lives.
    /// An unidentified file goes on `Models` — the shelf a checkpoint is on — because that is the
    /// only kind ComfyUI will offer for loading at all, and a file nobody can place is better
    /// somewhere visible than filed under a guess.
    pub const fn for_kind(kind: epoch_assets::asset::Kind) -> Self {
        use epoch_assets::asset::Kind;
        match kind {
            Kind::Checkpoint | Kind::Unknown => Shelf::Models,
            Kind::DiffusionModel => Shelf::DiffusionModels,
            Kind::TextEncoder => Shelf::TextEncoders,
            Kind::Lora => Shelf::Loras,
            Kind::Vae => Shelf::Vaes,
            Kind::Upscaler => Shelf::Upscalers,
            Kind::ControlNet => Shelf::ControlNet,
            Kind::Embedding => Shelf::Embeddings,
            Kind::Voice => Shelf::Voices,
        }
    }

    /// What ComfyUI calls this kind of folder in a search path, when it has a name for it.
    ///
    /// `Workflows` has none: a workflow is a document Epoch reads, not weights ComfyUI loads.
    pub const fn comfy_key(self) -> Option<&'static str> {
        match self {
            Shelf::Models => Some("checkpoints"),
            Shelf::DiffusionModels => Some("diffusion_models"),
            Shelf::TextEncoders => Some("text_encoders"),
            Shelf::Loras => Some("loras"),
            Shelf::Vaes => Some("vae"),
            Shelf::Upscalers => Some("upscale_models"),
            Shelf::ControlNet => Some("controlnet"),
            Shelf::Embeddings => Some("embeddings"),
            Shelf::Workflows | Shelf::Voices | Shelf::Timbres => None,
        }
    }
}

/// Where this installation keeps what Epoch downloaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    root: PathBuf,
}

impl Library {
    /// This machine's library.
    ///
    /// `EPOCH_LIBRARY` overrides it, which is how a test gets one of its own instead of writing
    /// into somebody's real twenty gigabytes.
    pub fn here() -> Self {
        if let Some(said) = std::env::var_os("EPOCH_LIBRARY") {
            return Self::at(said);
        }
        Self::at(
            application_data()
                .join("Epoch")
                .join("library")
                .join("generative"),
        )
    }

    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn shelf(&self, shelf: Shelf) -> PathBuf {
        self.root.join(shelf.folder())
    }

    /// Make every shelf exist.
    ///
    /// Eagerly, unlike the vault — and for a reason the vault does not have. A library is a place
    /// ComfyUI is *told about*, and a search path pointing at a folder that does not exist is a
    /// promise nothing can keep. There is also nothing misleading about an empty shelf: it says
    /// there is nowhere else these things could be hiding.
    pub fn ensure(&self) -> std::io::Result<()> {
        for shelf in Shelf::ALL {
            std::fs::create_dir_all(self.shelf(shelf))?;
        }
        Ok(())
    }

    /// What Epoch would write into ComfyUI's own config, for this library.
    ///
    /// Its own function so a test can read it without a ComfyUI on the machine.
    pub fn search_path_config(&self) -> String {
        let mut out = String::from(
            "# Written by Epoch. It points ComfyUI at Epoch's Generative Library.\n\
             #\n\
             # Epoch never moves, copies or renames a file you already had — this only says where\n\
             # its own library is. Deleting this file undoes it completely.\n\
             epoch:\n",
        );
        out.push_str(&format!("  base_path: '{}'\n", self.root.display()));
        for shelf in Shelf::ALL {
            if let Some(key) = shelf.comfy_key() {
                out.push_str(&format!("  {key}: '{}/'\n", shelf.folder()));
            }
        }
        out
    }
}

/// Every folder a ComfyUI on this machine loads a kind of asset from.
///
/// ## Why Epoch has to know this
///
/// A panel that greys an incompatible LoRA has to know what family the **checkpoint** is, and a
/// checkpoint the user already had lives in ComfyUI's tree rather than Epoch's library. Without
/// this the honest answer was *nobody measured it*, for a 6.9 GB file sitting right there — true,
/// unhelpful, and avoidable.
///
/// **Reading is not adopting.** ADR-0032 forbids moving, copying, renaming and installing into
/// somebody else's tree. It does not forbid opening a file to see what it is, which is the whole
/// basis of `understand`.
///
/// ## The paths are read from the config that declares them
///
/// Comfy Desktop generates `instance-model-paths/<id>.yaml` with a `base_path` and a folder per
/// kind. It is scanned line by line rather than parsed as YAML: two keys are needed, a YAML
/// dependency is not, and the file's own header says it is generated — so its shape is stable and
/// a line scan cannot go wrong in a way that matters. A path that does not exist is dropped, so a
/// stale entry costs nothing.
pub fn search_paths(shelf: Shelf) -> Vec<PathBuf> {
    let mut roots = vec![Library::here().shelf(shelf)];
    let Some(key) = shelf.comfy_key() else {
        return roots.into_iter().filter(|path| path.is_dir()).collect();
    };

    if let Some(dir) = comfy_desktop_dir().map(|dir| dir.join("instance-model-paths")) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let Ok(said) = std::fs::read_to_string(entry.path()) else {
                    continue;
                };
                if let Some(base) = declared(&said, "base_path") {
                    // The folder for this kind, when the file names one; otherwise the key
                    // itself, which is what ComfyUI's own default layout uses.
                    let folder = declared(&said, key).unwrap_or_else(|| key.to_owned());
                    roots.push(
                        folder
                            .trim_end_matches('/')
                            .split('/')
                            .fold(PathBuf::from(base), |at, part| at.join(part)),
                    );
                }
            }
        }
    }

    roots.retain(|path| path.is_dir());
    roots.dedup();
    roots
}

/// Every file on one shelf, named the way ComfyUI would name it — **without asking ComfyUI**.
///
/// ## Why this exists
///
/// The panel used to start the picture studio the moment it opened, because its lists came from
/// the server and there were none without it. That made opening a form cost a twenty-gigabyte
/// process and a terminal window, for somebody who might close it again — reported by the owner
/// as exactly that.
///
/// The server was never the only place these are known. `search_paths` already answers *which
/// folders ComfyUI looks in* — Epoch's library plus every path ComfyUI's own config declares —
/// so reading the names out of them gives the same list the server would report, from the same
/// files, seconds earlier and for nothing.
///
/// **This is listing, not adopting** (ADR-0032). Nothing is moved, copied, renamed or claimed;
/// the same rule `file_behind` already follows in the other direction.
///
/// What it cannot answer is anything that is a *node's vocabulary* rather than a file: the
/// encoder families a loader accepts, and which preparations exist. Those stay the server's, and
/// a panel without a server says so rather than showing an empty list that reads as *you have
/// none*.
pub fn names_on(shelf: Shelf) -> Vec<String> {
    let mut names = Vec::new();
    for root in search_paths(shelf) {
        walk_names(&root, &root, &mut names);
    }
    names.sort();
    names.dedup();
    names
}

/// Every model file under `at`, named relative to `root` with forward slashes.
///
/// Nested, because ComfyUI reports `sub/folder/name.safetensors` and `file_behind` already
/// expects that shape. Depth is bounded by the recursion following only real directories, and
/// the extensions are the ones a loader can actually take — a `.txt` beside a checkpoint is not
/// a checkpoint, and offering it would be a name the server refuses.
fn walk_names(root: &Path, at: &Path, into: &mut Vec<String>) {
    const LOADABLE: [&str; 6] = ["safetensors", "ckpt", "pt", "pth", "bin", "sft"];
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk_names(root, &path, into);
            continue;
        }
        let loadable = path
            .extension()
            .and_then(|it| it.to_str())
            .is_some_and(|it| LOADABLE.contains(&it.to_ascii_lowercase().as_str()));
        if !loadable {
            continue;
        }
        if let Ok(relative) = path.strip_prefix(root) {
            into.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// One `key: 'value'` from a generated config, unquoted.
///
/// Deliberately narrow: the first line whose first word is the key. A multi-line value (ComfyUI
/// allows `|-` blocks for `controlnet`) yields nothing rather than the block marker, which is the
/// right answer — several folders for one kind is not something a single path can hold.
fn declared(said: &str, key: &str) -> Option<String> {
    said.lines()
        .map(str::trim)
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().trim_matches('\'') == key)
        .map(|(_, value)| value.trim().trim_matches('\'').trim_matches('"').to_owned())
        .filter(|value| !value.is_empty() && !value.starts_with('|'))
}

/// The file behind a name a ComfyUI reported, if it can be found.
///
/// ComfyUI reports `sub/folder/name.safetensors` for a nested file, so the name is joined rather
/// than matched — and every candidate is checked for existence, never assumed.
pub fn file_behind(shelf: Shelf, reported: &str) -> Option<PathBuf> {
    let relative: PathBuf = reported
        .split(['/', '\\'])
        .fold(PathBuf::new(), |at, part| at.join(part));
    search_paths(shelf)
        .into_iter()
        .map(|root| root.join(&relative))
        .find(|path| path.is_file())
}

/// What happened when Epoch offered ComfyUI the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Link {
    /// The config was already exactly this. Nothing was written and nothing needs restarting.
    Current,
    /// Epoch wrote it just now. **ComfyUI must be restarted once** — measured, see the module
    /// docs. After that, assets appear without one.
    Written,
    /// No ComfyUI base directory was found, so there was nothing to tell.
    NoStudio,
    /// It was found and could not be written to. The reason, in the words the filesystem used.
    Refused(String),
}

/// The handshake, and where it happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Handshake {
    /// The ComfyUI base directory — the folder holding `main.py`.
    pub studio_at: Option<String>,
    /// The file Epoch owns, when there is one.
    pub config: Option<String>,
    pub link: Link,
}

/// Tell ComfyUI where the library is, if a ComfyUI can be found.
///
/// Idempotent by comparison, not by a flag: it reads what is there and writes only when it
/// differs. That is what lets it run at every launch without asking for a restart every launch.
pub fn offer_library_to_comfyui(library: &Library) -> Handshake {
    let Some(base) = comfyui_base() else {
        return Handshake {
            studio_at: None,
            config: None,
            link: Link::NoStudio,
        };
    };
    let config = base.join("extra_model_paths.yaml");
    let wanted = library.search_path_config();
    let link = match std::fs::read_to_string(&config) {
        Ok(found) if found == wanted => Link::Current,
        _ => match library
            .ensure()
            .and_then(|()| std::fs::write(&config, &wanted))
        {
            Ok(()) => Link::Written,
            Err(why) => Link::Refused(why.to_string()),
        },
    };
    Handshake {
        studio_at: Some(base.display().to_string()),
        config: Some(config.display().to_string()),
        link,
    }
}

/// The folder ComfyUI runs out of — the one holding `main.py`.
///
/// **Asked, then looked for.** Comfy Desktop keeps `installations.json` and each entry names its
/// `installPath` exactly, so the first answer is the one the program itself gives. Only when that
/// is absent does this look in the places an installer puts things, and even then it checks two
/// named files rather than a directory name — a folder called `ComfyUI` proves nothing.
pub fn comfyui_base() -> Option<PathBuf> {
    declared_installs()
        .into_iter()
        .chain(usual_installs())
        .find(|candidate| is_comfyui_base(candidate))
}

/// Whether this folder is a ComfyUI, by the two files that are always in one.
fn is_comfyui_base(dir: &Path) -> bool {
    dir.join("main.py").is_file() && dir.join("folder_paths.py").is_file()
}

/// What Comfy Desktop says it installed, in the order it lists them.
///
/// A cloud entry has no `installPath` and is skipped by construction: nothing to read means
/// nothing to add.
/// Where Comfy Desktop keeps what it knows about itself.
///
/// Public because two questions need it: which install exists, and which model-path config it
/// generated. One answer, one place.
pub fn comfy_desktop_dir() -> Option<PathBuf> {
    let dir = application_data().join("Comfy Desktop");
    dir.is_dir().then_some(dir)
}

fn declared_installs() -> Vec<PathBuf> {
    let Some(said) = std::fs::read_to_string(
        application_data()
            .join("Comfy Desktop")
            .join("installations.json"),
    )
    .ok() else {
        return Vec::new();
    };
    let Ok(entries) = serde_json::from_str::<serde_json::Value>(&said) else {
        return Vec::new();
    };
    entries
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("installPath")?.as_str())
                // Measured: the desktop's `installPath` is the *install*, and ComfyUI itself sits
                // one level in. Both are offered, and `is_comfyui_base` decides.
                .flat_map(|path| [PathBuf::from(path).join("ComfyUI"), PathBuf::from(path)])
                .collect()
        })
        .unwrap_or_default()
}

/// The places a ComfyUI ends up when nobody used the desktop installer.
fn usual_installs() -> Vec<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from);
    let mut roots = Vec::new();
    if let Some(home) = home {
        for name in ["ComfyUI", "Documents/ComfyUI", "Desktop/ComfyUI"] {
            roots.push(home.join(name));
        }
    }
    if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        roots.push(local.join("Programs/ComfyUI/ComfyUI"));
    }
    roots
}

/// Where this platform keeps a user's application data.
///
/// The same three answers `epoch-engine::paths` gives, spelled again rather than shared: this
/// crate depends on nothing of Epoch's, and one small function is a cheaper price than the
/// dependency that would remove it.
/// Where this platform keeps an application's own data.
///
/// Public because the voice engine keeps its binary beside the library rather than inside it:
/// Piper is a program, not an asset, and a second answer to *where does Epoch put its own
/// things* is how the two come to disagree.
pub fn application_data() -> PathBuf {
    if let Some(roaming) = std::env::var_os("APPDATA") {
        return PathBuf::from(roaming);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
    }
}

/// One file on a shelf, understood from its own bytes.
///
/// **Here rather than in a surface, because both surfaces show it.** The Host's Creations deck
/// and EpochServices' Creations page list the same library off the same disk; written twice they
/// would be two answers to *what is this file* that agree until one of them learns something.
/// ADR-0029 forbids EpochServices from linking the Engine, and `epoch-models` is what they share.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Held {
    pub file: String,
    /// What it is, in the words a person uses.
    pub kind: String,
    /// Which family it belongs to. `unknown` is unmeasured, never incompatible.
    pub base: String,
    pub bytes: u64,
    /// **Which medium it makes** — `picture`, `video`, `sound`, `model`, or `any`.
    ///
    /// Three answers, not two, and the third was found by looking at the list: of twenty-two
    /// files on the owner's machine, eight said *Epoch could not tell what this makes* — and for
    /// a text encoder that is true, permanent and useless. `clip_l.safetensors` is the same file
    /// beside Flux and beside SDXL and it feeds a picture graph, a video graph and a sound graph
    /// alike; **it has no medium of its own and never will.** That is a different fact from
    /// `minimax_h3_fl2va`, which has one and which Epoch failed to read.
    ///
    /// So `any` is *the question does not apply*, and `None` is *unplaced*. Neither is ever
    /// *not this one*: a surface filtering by medium keeps showing both, because there is
    /// nothing the user can do about a failure of Epoch's and nothing else would tell them why
    /// their own model vanished from the list.
    pub medium: Option<String>,
    /// Why nothing could be derived, when nothing could. Information, not a fault.
    pub unread: Option<String>,
}

/// A medium, in the one spelling every surface compares against.
pub const fn medium_id(makes: epoch_assets::asset::Makes) -> &'static str {
    use epoch_assets::asset::Makes;
    match makes {
        Makes::Picture => "picture",
        Makes::Video => "video",
        Makes::Sound => "sound",
        Makes::Model => "model",
    }
}

/// Everything on one shelf, read from its own bytes and never from its name.
///
/// Sorted by name so the list does not reorder itself between two openings of the same panel —
/// directory order is whatever the filesystem feels like, and a list that shuffles teaches
/// nobody anything.
pub fn held_on(shelf: &Path) -> Vec<Held> {
    let Ok(entries) = std::fs::read_dir(shelf) else {
        return Vec::new();
    };
    let mut held: Vec<Held> = entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        // What Epoch wrote *about* a file is not another file on the shelf. A manifest listed
        // beside its asset reads as a second thing somebody downloaded, and one that Epoch
        // could not identify at that.
        .filter(|entry| entry.path().extension().and_then(|it| it.to_str()) != Some("json"))
        .map(|entry| {
            let path = entry.path();
            let file = entry.file_name().to_string_lossy().into_owned();

            // A voice is described by the file beside it, not by its tensors.
            //
            // `understand` reads a picture model's weights to place it; an `.onnx` carries none
            // of what it looks for, so every voice on the shelf would read **unreadable, 0
            // bytes** — a wrong instrument about a file that is perfectly fine and that Epoch
            // can describe exactly. The sidecar is the measurement here (ADR-0024: from the
            // bytes, never from the name), and it is already on disk.
            if path.extension().and_then(|it| it.to_str()) == Some("onnx") {
                let spoken = crate::voices::Spoken::beside(&path);
                let said = spoken.as_ref().map(|it| it.plainly()).unwrap_or_default();
                return Held {
                    file,
                    kind: epoch_assets::asset::Kind::Voice.plainly().to_owned(),
                    // What a voice's `base` is: the language it speaks. `unknown` where the
                    // sidecar did not say, which is the same word every other shelf uses for
                    // *unmeasured* — never *incompatible*.
                    base: if said.is_empty() {
                        "unknown".to_owned()
                    } else {
                        said
                    },
                    bytes: std::fs::metadata(&path).map(|it| it.len()).unwrap_or(0),
                    // A voice makes sound, and it says so in the word the shelves already use.
                    medium: Some("sound".to_owned()),
                    // Present only when there is genuinely nothing to read — the sidecar is
                    // missing or will not parse, which means the voice will refuse when Piper
                    // opens it. Worth saying here rather than at the moment somebody speaks.
                    unread: spoken
                        .is_none()
                        .then(|| "its sidecar is missing or unreadable".to_owned()),
                };
            }

            match epoch_assets::asset::understand(&path) {
                Ok(read) => Held {
                    file,
                    kind: read.kind.plainly().to_owned(),
                    base: read.base.plainly().to_owned(),
                    bytes: read.bytes,
                    // A text encoder belongs to no medium and to all of them, which is a
                    // measurement rather than a shrug: the same file feeds a picture graph, a
                    // video graph and a sound graph.
                    medium: if read.kind == epoch_assets::asset::Kind::TextEncoder {
                        Some("any".to_owned())
                    } else {
                        read.makes().map(|it| medium_id(it).to_owned())
                    },
                    unread: read.unread.map(str::to_owned),
                },
                Err(why) => Held {
                    file,
                    kind: "unreadable".to_owned(),
                    base: "unknown".to_owned(),
                    bytes: 0,
                    // A file that could not be read is unplaced, which is what `None` says. It
                    // stays on the shelf and stays visible under every medium.
                    medium: None,
                    unread: Some(why.to_string()),
                },
            }
        })
        .collect();
    held.sort_by(|a, b| a.file.cmp(&b.file));
    held
}

/// Every shelf in this library, with what is on it.
pub fn everything_held(library: &Library) -> Vec<(Shelf, Vec<Held>)> {
    Shelf::ALL
        .into_iter()
        .map(|shelf| (shelf, held_on(&library.shelf(shelf))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The handshake, against whatever ComfyUI this machine really has.
    ///
    /// Ignored because it writes into another program's config directory, which is not something
    /// `cargo test` should do on somebody's behalf. Run deliberately, it is the only thing that
    /// proves `comfyui_base` finds a real install rather than a plausible path.
    #[test]
    #[ignore = "writes into this machine's ComfyUI"]
    fn what_is_here() {
        let library = Library::here();
        println!("library: {}", library.root().display());
        let done = offer_library_to_comfyui(&library);
        println!("{done:#?}");
        assert!(
            done.link != Link::NoStudio,
            "no ComfyUI found on this machine — nothing to measure"
        );
        // Twice, because the property worth having is that the second run is silent.
        assert_eq!(offer_library_to_comfyui(&library).link, Link::Current);
    }

    #[test]
    fn every_shelf_is_a_folder_and_most_are_a_search_path() {
        let library = Library::at("/tmp/whatever");
        for shelf in Shelf::ALL {
            assert!(library.shelf(shelf).ends_with(shelf.folder()));
        }
        // Workflows deliberately has no ComfyUI key: it is a document Epoch reads, not weights
        // ComfyUI loads. If that ever gains one, this test is the place that says so.
        assert_eq!(Shelf::Workflows.comfy_key(), None);
        assert_eq!(Shelf::Vaes.comfy_key(), Some("vae"));
    }

    #[test]
    fn the_config_names_the_library_and_every_shelf_comfyui_understands() {
        let written = Library::at("/somewhere/generative").search_path_config();
        assert!(
            written.contains("base_path: '/somewhere/generative'"),
            "{written}"
        );
        for shelf in Shelf::ALL {
            match shelf.comfy_key() {
                Some(key) => assert!(
                    written.contains(&format!("  {key}: '{}/'\n", shelf.folder())),
                    "{key} missing from:\n{written}"
                ),
                None => assert!(!written.contains(shelf.folder()), "{written}"),
            }
        }
    }

    #[test]
    fn a_second_offer_writes_nothing() {
        // The property that lets the handshake run at every launch: it compares rather than
        // remembering, so only a *changed* library ever asks for a restart.
        let library = Library::at("/somewhere/generative");
        assert_eq!(library.search_path_config(), library.search_path_config());
    }

    #[test]
    fn a_folder_named_comfyui_is_not_a_comfyui() {
        let dir = std::env::temp_dir().join("epoch-not-a-comfyui/ComfyUI");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(!is_comfyui_base(&dir));
        std::fs::write(dir.join("main.py"), "").unwrap();
        assert!(!is_comfyui_base(&dir), "one of the two files is not enough");
        std::fs::write(dir.join("folder_paths.py"), "").unwrap();
        assert!(is_comfyui_base(&dir));
        let _ = std::fs::remove_dir_all(dir.parent().unwrap());
    }

    #[test]
    fn the_library_makes_its_shelves() {
        let root = std::env::temp_dir().join("epoch-library-test");
        let _ = std::fs::remove_dir_all(&root);
        let library = Library::at(&root);
        library.ensure().unwrap();
        for shelf in Shelf::ALL {
            assert!(library.shelf(shelf).is_dir(), "{shelf:?}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_generated_config_yields_the_two_keys_that_matter() {
        // Trimmed from the file Comfy Desktop really wrote on this machine, quotes and all.
        let said = "\
# Generated by Comfy Desktop - do not edit manually.
comfy.desktop_0:
  base_path: 'C:\\Users\\somebody\\ComfyUI-Shared\\models'
  is_default: true
  'checkpoints': 'checkpoints/'
  'controlnet': |-
    controlnet/
    t2i_adapter/
  'loras': 'loras/'
";
        assert_eq!(
            declared(said, "base_path").as_deref(),
            Some("C:\\Users\\somebody\\ComfyUI-Shared\\models")
        );
        assert_eq!(
            declared(said, "checkpoints").as_deref(),
            Some("checkpoints/")
        );
        assert_eq!(declared(said, "loras").as_deref(), Some("loras/"));
        // A kind with several folders yields nothing rather than the block marker: one path
        // cannot hold two, and a `|-` stored as a path would be a path that never resolves.
        assert_eq!(declared(said, "controlnet"), None);
        assert_eq!(declared(said, "nothing_like_this"), None);
    }

    #[test]
    #[ignore = "reads this machine's ComfyUI"]
    fn where_this_machine_loads_from() {
        for shelf in Shelf::ALL {
            println!("{shelf:?}");
            for path in search_paths(shelf) {
                println!("   {}", path.display());
            }
        }
        println!(
            "sd_xl_base_1.0.safetensors -> {:?}",
            file_behind(Shelf::Models, "sd_xl_base_1.0.safetensors")
        );
    }
}
