//! The Project Root — where a World actually works (ADR-0025).
//!
//! ## Two questions, and they are not the same one
//!
//! > **Project Root** answers *where does this World work.*
//! > **Trust** answers *what is this Quest allowed to do there.*
//!
//! Being inside the root grants nothing; having permission does not tell anyone where. Merging
//! them would produce a single "dangerous folder" switch, which is how a permission model stops
//! being explainable.
//!
//! Setting a root is therefore **ungated**. It is a workspace and a source of context, not an
//! authorisation, and it should arrive early — planning against the user's real codebase is
//! worth immeasurably more than planning against a hypothesis.
//!
//! ## Confinement is done against the real path, not the written one
//!
//! `../` is the least of it. A symlink inside the project pointing at `~/.ssh` is a path that
//! *looks* contained and is not, so containment is decided after both sides are resolved on
//! disk — the one place this module cannot be pure.
//!
//! Windows matters here: containment is compared case-insensitively, because `C:\Proj` and
//! `c:\proj` are one directory and a byte comparison would say otherwise.
//!
//! ## The root lives in the vault, never in the World Pack
//!
//! A pack is shipped content and cannot know where you keep your code — the same argument that
//! moved the crew roster out of packs (ADR-0023). Per World, because a single global root would
//! become a lie the moment a second World exists.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("that path does not exist")]
    Missing,
    #[error("that is a file; a project root is a folder")]
    NotADirectory,
    #[error("cannot read that path: {0}")]
    Unreadable(String),
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Malformed(String),
}

/// File and directory names nothing may reach, even inside the project root.
///
/// A repository can contain an `.ssh` folder — mine does not, yours might, and a checked-in
/// `id_rsa` in somebody's test fixtures is not a reason to hand it to a model. Confinement
/// answers "is it inside"; this answers "should anyone have asked".
///
/// Matched against every component of the resolved path, case-insensitively.
const NEVER: [&str; 8] = [
    ".ssh",
    ".gnupg",
    ".aws",
    ".config/gcloud",
    "id_rsa",
    "id_ed25519",
    ".env",
    ".netrc",
];

/// What was found in a folder somebody is about to hand a World.
///
/// Facts, not a verdict. Epoch does not decide whether a folder is a good project — it says
/// what is there and lets the person who chose it recognise it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectScan {
    /// Whether a Git repository is here — including worktrees and submodules, where `.git` is
    /// a file rather than a directory.
    pub git: bool,
    /// Things at the top level. Zero is a real answer: an empty folder is a legitimate place
    /// to start a project.
    pub entries: usize,
    pub folders: usize,
}

/// A folder a World is allowed to work in.
///
/// Constructed only from a path that exists and is a directory, and stored already resolved —
/// so every containment check downstream compares two real paths rather than two hopeful ones.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoot {
    resolved: PathBuf,
    /// What the user typed or chose, kept for display. Showing them a canonicalised path with
    /// a `\\?\` prefix would be technically accurate and unrecognisable.
    authored: String,
}

impl ProjectRoot {
    /// Accept a folder, or say why not.
    ///
    /// Checked at the moment it is set rather than at the moment a capability uses it: a root
    /// that does not exist should be a message in the Launcher, not a failure three turns into
    /// a Quest.
    pub fn open(authored: &str) -> Result<Self, ProjectError> {
        let trimmed = authored.trim();
        if trimmed.is_empty() {
            return Err(ProjectError::Missing);
        }
        let path = PathBuf::from(trimmed);
        let resolved = path.canonicalize().map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => ProjectError::Missing,
            _ => ProjectError::Unreadable(err.to_string()),
        })?;
        if !resolved.is_dir() {
            return Err(ProjectError::NotADirectory);
        }
        Ok(Self {
            resolved,
            authored: trimmed.to_owned(),
        })
    }

    /// What is actually in this folder.
    ///
    /// Every field is **measured**, and a field with nothing behind it says zero rather than
    /// something plausible — the Launcher's rule, applied to the one moment where a wrong
    /// answer would matter most. Somebody is about to point a World at their real source code;
    /// telling them a repository was found when none was is exactly the kind of confident
    /// wrongness that teaches people to stop reading the instruments.
    ///
    /// Deliberately shallow. This runs while the user is typing a path, and walking a large
    /// repository to count files would make the dialog stutter in proportion to how serious the
    /// project is. The top level answers the question being asked — *is this the right folder?*
    pub fn scan(&self) -> ProjectScan {
        // `.git` is a directory in a normal clone and a *file* in a worktree or submodule.
        // Checking only for a directory would report "no repository" inside a worktree, which
        // is both wrong and the kind of thing somebody would not think to doubt.
        let git = self.resolved.join(".git").exists();

        let mut entries = 0usize;
        let mut folders = 0usize;
        if let Ok(read) = std::fs::read_dir(&self.resolved) {
            for entry in read.flatten() {
                entries += 1;
                if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                    folders += 1;
                }
            }
        }

        ProjectScan {
            git,
            entries,
            folders,
        }
    }

    /// The path as the user knows it. What a surface shows.
    pub fn as_authored(&self) -> &str {
        &self.authored
    }

    /// The resolved path. What confinement is decided against.
    pub fn path(&self) -> &Path {
        &self.resolved
    }

    /// Whether this root still exists. A folder can be moved while Epoch is open.
    pub fn still_there(&self) -> bool {
        self.resolved.is_dir()
    }

    /// Turn a path a **model** produced into one that is safe to touch, or refuse it.
    ///
    /// This is the security-critical function in this file. It is deliberately strict and
    /// deliberately boring:
    ///
    /// - relative paths resolve under the root; absolute ones must already be inside it
    /// - the result is resolved on disk, so a symlink out is a symlink caught
    /// - a path that does not exist yet still resolves — its **parent** is checked, because
    ///   writing a new file is ordinary and must not require the file to exist first
    /// - sensitive names are refused wherever they appear, inside the root or not
    pub fn confine(&self, raw: &str) -> Result<PathBuf, String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err("a path is required".into());
        }
        let candidate = PathBuf::from(trimmed);
        // A home-relative path is a path outside the project by definition. Refusing `~`
        // outright is clearer than expanding it and then rejecting where it landed.
        if trimmed.starts_with('~') {
            return Err("'~' paths are outside the project".into());
        }

        let joined = if candidate.is_absolute() {
            candidate
        } else {
            self.resolved.join(candidate)
        };

        // Resolve as much as exists, then re-attach the rest.
        //
        // Writing `src/thing/mod.rs` into a project with no `thing` folder is ordinary work, so
        // neither the file nor several of its parents need exist. Walking up to the deepest
        // ancestor that *does* exist and canonicalising that still defeats a symlinked
        // ancestor, which is the case that matters.
        let resolved = match joined.canonicalize() {
            Ok(resolved) => resolved,
            Err(_) => {
                let mut existing = joined.as_path();
                let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
                while !existing.exists() {
                    let name = existing
                        .file_name()
                        // **A `..` is refused the moment it is walked past, not afterwards.**
                        // `Path::file_name` answers `None` for a path ending in `..`, so the
                        // check below could only be reached on a platform that had already
                        // stopped walking — and the two disagree about that: Windows normalises
                        // `a/../..` lexically before touching the filesystem, so the loop ends
                        // early and the `..` is in `rest`; Unix does not, so the walk ran past
                        // it and the refusal came out as *that path does not name anything*.
                        //
                        // Both refused, which is what matters, and they refused for two
                        // different stated reasons. One rule, one sentence, either machine.
                        .ok_or_else(|| format!("'{trimmed}' is outside the project"))?;
                    if name == std::ffi::OsStr::new("..") {
                        return Err(format!("'{trimmed}' is outside the project"));
                    }
                    rest.push(name);
                    existing = existing.parent().ok_or("that path has no parent")?;
                }
                let base = existing
                    .canonicalize()
                    .map_err(|_| format!("'{trimmed}' is not somewhere that exists"))?;

                // `..` inside the part that does not exist cannot be resolved against the
                // filesystem, so it is refused rather than assumed harmless — above, as each
                // component is walked past. Joining it blind would let `new/../../../elsewhere`
                // climb out below the containment check.

                let mut resolved = base;
                for part in rest.into_iter().rev() {
                    resolved.push(part);
                }
                resolved
            }
        };

        if !contains(&self.resolved, &resolved) {
            return Err(format!("'{trimmed}' is outside the project"));
        }
        if let Some(bad) = sensitive(&resolved) {
            return Err(format!("'{bad}' is not something Epoch will touch"));
        }
        Ok(resolved)
    }
}

/// Whether `path` is the base or lives under it. Case-insensitive, because Windows.
fn contains(base: &Path, path: &Path) -> bool {
    let base: Vec<String> = components(base);
    let path: Vec<String> = components(path);
    path.len() >= base.len() && path[..base.len()] == base[..]
}

fn components(path: &Path) -> Vec<String> {
    path.components()
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

/// The first sensitive name in a path, if any.
fn sensitive(path: &Path) -> Option<String> {
    let lowered: Vec<String> = components(path);
    NEVER.iter().find_map(|never| {
        // Multi-segment entries like `.config/gcloud` are matched as a run of components.
        let wanted: Vec<String> = never.split('/').map(str::to_owned).collect();
        lowered
            .windows(wanted.len())
            .any(|window| window == wanted.as_slice())
            .then(|| (*never).to_owned())
    })
}

/// Which folder each World works in.
///
/// In the vault, keyed by World id — never in the pack. See the module docs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectRoots {
    roots: BTreeMap<String, String>,
}

impl ProjectRoots {
    pub fn load(vault: &Path) -> Self {
        // A missing or unreadable file means no World has a root yet. Unlike trust, nothing is
        // lost by that: a World with no project root is complete, it just has no code to read.
        match std::fs::read_to_string(path(vault)) {
            Ok(raw) => toml::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self, vault: &Path) -> Result<(), ProjectError> {
        let body =
            toml::to_string_pretty(self).map_err(|e| ProjectError::Malformed(e.to_string()))?;
        std::fs::create_dir_all(vault).map_err(|source| ProjectError::Write {
            path: vault.to_path_buf(),
            source,
        })?;
        let path = path(vault);
        std::fs::write(&path, body).map_err(|source| ProjectError::Write { path, source })
    }

    /// What this World says its root is, exactly as authored. `None` means none set.
    pub fn authored(&self, world: &str) -> Option<&str> {
        self.roots.get(world).map(String::as_str)
    }

    /// This World's root, opened. `None` when unset **or when it has gone missing** — a folder
    /// can be moved while Epoch is open, and a root that is not there is not a root.
    pub fn open(&self, world: &str) -> Option<ProjectRoot> {
        self.roots
            .get(world)
            .and_then(|raw| ProjectRoot::open(raw).ok())
    }

    /// Set or clear a World's root. Validated here, so what is stored is known to have worked.
    pub fn set(&mut self, world: &str, authored: Option<&str>) -> Result<(), ProjectError> {
        match authored.map(str::trim).filter(|s| !s.is_empty()) {
            Some(raw) => {
                let root = ProjectRoot::open(raw)?;
                self.roots
                    .insert(world.to_owned(), root.as_authored().to_owned());
            }
            None => {
                self.roots.remove(world);
            }
        }
        Ok(())
    }
}

fn path(vault: &Path) -> PathBuf {
    vault.join("projects.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let d = std::env::temp_dir().join(format!("epoch-project-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(d.join("src")).unwrap();
            std::fs::write(d.join("src/main.rs"), "fn main() {}").unwrap();
            Self(d)
        }
        fn root(&self) -> ProjectRoot {
            ProjectRoot::open(self.0.to_str().unwrap()).unwrap()
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_root_must_be_a_folder_that_exists() {
        let t = Tree::new("open");
        assert!(t.root().still_there());
        assert!(matches!(ProjectRoot::open(""), Err(ProjectError::Missing)));
        assert!(matches!(
            ProjectRoot::open("Z:/nowhere/at/all"),
            Err(ProjectError::Missing) | Err(ProjectError::Unreadable(_))
        ));
        // A file is not a workspace.
        let file = t.0.join("src/main.rs");
        assert!(matches!(
            ProjectRoot::open(file.to_str().unwrap()),
            Err(ProjectError::NotADirectory)
        ));
    }

    #[test]
    fn the_path_shown_is_the_one_the_user_recognises() {
        // Canonicalising is for containment. Showing a `\\?\` prefixed path back to somebody
        // would be technically accurate and unrecognisable.
        let t = Tree::new("authored");
        let root = t.root();
        assert_eq!(root.as_authored(), t.0.to_str().unwrap());
    }

    #[test]
    fn a_relative_path_lands_inside_the_project() {
        let t = Tree::new("relative");
        let root = t.root();
        let confined = root.confine("src/main.rs").unwrap();
        assert!(confined.ends_with("main.rs"));
        assert!(contains(root.path(), &confined));
    }

    #[test]
    fn climbing_out_is_refused() {
        let t = Tree::new("climb");
        let root = t.root();
        for attempt in ["../secrets.txt", "src/../../secrets.txt", "src/../.."] {
            let refused = root.confine(attempt).unwrap_err();
            assert!(
                refused.contains("outside the project"),
                "{attempt}: {refused}"
            );
        }
    }

    #[test]
    fn an_absolute_path_elsewhere_is_refused() {
        let t = Tree::new("absolute");
        let root = t.root();
        let elsewhere = std::env::temp_dir();
        let refused = root.confine(elsewhere.to_str().unwrap()).unwrap_err();
        assert!(refused.contains("outside the project"), "{refused}");

        // An absolute path that is genuinely inside is fine — models produce these constantly.
        let inside = t.0.join("src/main.rs");
        assert!(root.confine(inside.to_str().unwrap()).is_ok());
    }

    #[test]
    fn a_home_relative_path_is_refused_by_name() {
        let t = Tree::new("home");
        let refused = t.root().confine("~/.ssh/id_rsa").unwrap_err();
        assert!(refused.contains("outside the project"), "{refused}");
    }

    #[test]
    fn a_file_that_does_not_exist_yet_still_resolves() {
        // Writing a new file is ordinary work. Requiring it to exist first would make the
        // confinement check refuse the most common legitimate write.
        let t = Tree::new("new");
        let root = t.root();
        let planned = root.confine("src/new_module.rs").unwrap();
        assert!(planned.ends_with("new_module.rs"));
        assert!(!planned.exists());

        // And a new file outside is still refused.
        assert!(root.confine("../escape.rs").is_err());
    }

    #[test]
    fn a_whole_new_branch_of_folders_resolves() {
        // Writing `src/thing/mod.rs` into a project with no `thing` folder is ordinary work:
        // neither the file nor several of its parents exist yet.
        let t = Tree::new("newbranch");
        let root = t.root();
        let planned = root.confine("src/deep/inner/mod.rs").unwrap();
        assert!(planned.ends_with("mod.rs"));
        assert!(contains(root.path(), &planned));
    }

    #[test]
    fn dot_dot_inside_a_path_that_does_not_exist_yet_is_refused() {
        // It cannot be resolved against the filesystem, so it is refused rather than assumed
        // harmless. Joining it blind would let this climb out below the containment check.
        let t = Tree::new("phantomclimb");
        let refused = t.root().confine("new/../../../elsewhere.rs").unwrap_err();
        assert!(refused.contains("outside the project"), "{refused}");
    }

    #[test]
    fn sensitive_names_are_refused_even_inside_the_project() {
        // Confinement answers "is it inside". This answers "should anyone have asked".
        let t = Tree::new("sensitive");
        std::fs::create_dir_all(t.0.join(".ssh")).unwrap();
        std::fs::write(t.0.join(".ssh/id_rsa"), "not really a key").unwrap();
        std::fs::write(t.0.join(".env"), "SECRET=1").unwrap();
        let root = t.root();

        for attempt in [".ssh/id_rsa", ".ssh", ".env"] {
            let refused = root.confine(attempt).unwrap_err();
            assert!(refused.contains("Epoch will touch"), "{attempt}: {refused}");
        }
        // The ordinary file next to them is untouched by the rule.
        assert!(root.confine("src/main.rs").is_ok());
    }

    #[test]
    fn containment_is_case_insensitive_because_windows() {
        // `C:\Proj` and `c:\proj` are one directory. A byte comparison says otherwise, and the
        // failure would be a legitimate path refused rather than an illegitimate one allowed —
        // annoying rather than dangerous, but wrong either way.
        let base = Path::new("C:/Projects/Epoch");
        assert!(contains(base, Path::new("c:/projects/epoch/src/main.rs")));
        assert!(!contains(base, Path::new("C:/Projects/EpochOther/x.rs")));
    }

    #[test]
    fn a_sibling_folder_with_the_same_prefix_is_not_inside() {
        // The classic containment bug: string-prefix matching says `/proj-secrets` is inside
        // `/proj`. Comparing components says it is not.
        assert!(!contains(
            Path::new("/proj"),
            Path::new("/proj-secrets/keys")
        ));
        assert!(contains(Path::new("/proj"), Path::new("/proj/src")));
        assert!(contains(Path::new("/proj"), Path::new("/proj")));
    }

    #[test]
    fn a_root_is_per_world_and_survives_a_round_trip() {
        let t = Tree::new("store");
        let vault = t.0.join("vault");
        let mut roots = ProjectRoots::default();

        roots
            .set("archipelago", Some(t.0.to_str().unwrap()))
            .unwrap();
        roots.save(&vault).unwrap();

        let read = ProjectRoots::load(&vault);
        assert_eq!(read.authored("archipelago"), Some(t.0.to_str().unwrap()));
        // A single global root would be a lie the moment a second World exists.
        assert_eq!(read.authored("default"), None);
        assert!(read.open("archipelago").is_some());
    }

    #[test]
    fn setting_a_root_that_does_not_work_is_refused_rather_than_stored() {
        // Reported in the Launcher while the user is looking at it, not three turns into a
        // Quest that suddenly cannot read anything.
        let mut roots = ProjectRoots::default();
        assert!(roots.set("archipelago", Some("Z:/nowhere")).is_err());
        assert_eq!(roots.authored("archipelago"), None);
    }

    #[test]
    fn clearing_a_root_is_allowed_and_leaves_a_valid_world() {
        let t = Tree::new("clear");
        let mut roots = ProjectRoots::default();
        roots
            .set("archipelago", Some(t.0.to_str().unwrap()))
            .unwrap();
        roots.set("archipelago", None).unwrap();
        assert_eq!(roots.authored("archipelago"), None);
        // A World with no project root is complete, not unfinished — it has no code to read.
        assert!(roots.open("archipelago").is_none());
    }

    #[test]
    fn a_root_that_has_gone_missing_reports_as_absent_rather_than_pretending() {
        let t = Tree::new("moved");
        let mut roots = ProjectRoots::default();
        roots
            .set("archipelago", Some(t.0.to_str().unwrap()))
            .unwrap();
        std::fs::remove_dir_all(&t.0).unwrap();

        // Still authored — the user's choice is not silently deleted because a drive was
        // unplugged — but not openable, so nothing tries to work in a folder that is gone.
        assert!(roots.authored("archipelago").is_some());
        assert!(roots.open("archipelago").is_none());
    }
}

#[cfg(test)]
mod scan_tests {
    use super::*;

    struct Dir(PathBuf);
    impl Dir {
        fn new(tag: &str) -> Self {
            let d = std::env::temp_dir().join(format!("epoch-scan-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&d);
            std::fs::create_dir_all(&d).unwrap();
            Self(d)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn an_empty_folder_reports_zero_rather_than_refusing() {
        // Zero is a real answer: an empty folder is a legitimate place to start a project, and
        // a scan that treated it as a failure would be Epoch deciding what a good project is.
        let d = Dir::new("empty");
        let scan = ProjectRoot::open(d.0.to_str().unwrap()).unwrap().scan();

        assert!(!scan.git);
        assert_eq!(scan.entries, 0);
    }

    #[test]
    fn a_worktree_is_a_repository_even_though_its_git_is_a_file() {
        // `.git` is a directory in a normal clone and a *file* in a worktree or submodule.
        // Checking only for a directory would report "no repository" inside a worktree — wrong,
        // and not the kind of thing anybody would think to doubt.
        let d = Dir::new("worktree");
        std::fs::write(d.0.join(".git"), "gitdir: ../.git/worktrees/x").unwrap();

        assert!(ProjectRoot::open(d.0.to_str().unwrap()).unwrap().scan().git);
    }

    #[test]
    fn a_clone_is_a_repository() {
        let d = Dir::new("clone");
        std::fs::create_dir(d.0.join(".git")).unwrap();
        std::fs::create_dir(d.0.join("src")).unwrap();
        std::fs::write(d.0.join("README.md"), "hello").unwrap();

        let scan = ProjectRoot::open(d.0.to_str().unwrap()).unwrap().scan();
        assert!(scan.git);
        assert_eq!(scan.entries, 3);
        assert_eq!(scan.folders, 2);
    }

    #[test]
    fn a_folder_that_is_not_there_is_refused_while_the_user_is_still_typing() {
        // The whole reason this is asked at creation rather than discovered mid-Quest.
        assert!(ProjectRoot::open("/definitely/not/here/at/all").is_err());
    }
}
