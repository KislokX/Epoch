//! The Skill Registry — authored ways of working, read from the vault.
//!
//! The second Definition type (ADR-0011), and its arrival answers the question that ADR left
//! open rather than confirming it.
//!
//! ADR-0011 said a generic `Definition` trait would be designed *when a second type appeared*.
//! It has, and the trait is the wrong seam: [`epoch_kernel::CharacterDefinition`] carries a
//! dozen fields about a person and [`SkillDefinition`] carries five about a method, so a trait
//! over both would abstract two fields — an id and a name — while the real duplication sat
//! somewhere else entirely.
//!
//! What the two genuinely share is the **loading**: the sorted list of authored `.toml` files in
//! a directory, the rule that a file's name is the id inside it, and problems collected rather
//! than raised. That is [`crate::definition::authored_files`], one small function called by both
//! registries, and it is the whole of the reuse.
//!
//! Recorded here rather than only in a commit message because the next Definition type will ask
//! the same question, and the evidence is that shape is not what repeats — machinery is.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use epoch_kernel::{SkillDefinition, SkillId};

/// A loaded Skill and the file it came from.
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub definition: SkillDefinition,
    pub path: PathBuf,
    modified: Option<SystemTime>,
}

/// Every Skill this vault holds, and what was wrong with the ones that did not load.
///
/// A broken file never stops the others: it is reported and skipped, the same discipline the
/// character registry follows. One unparseable Skill must not take a crew's other ways of
/// working away from them.
#[derive(Debug, Default)]
pub struct SkillRegistry {
    root: PathBuf,
    /// Ordered by id, so every listing this feeds is stable across machines and runs.
    skills: BTreeMap<SkillId, LoadedSkill>,
    problems: Vec<String>,
    /// How many `.toml` files the last load saw, so an added or deleted one is noticed.
    files_seen: usize,
}

impl SkillRegistry {
    pub fn load(root: impl Into<PathBuf>) -> Self {
        let mut registry = Self {
            root: root.into(),
            ..Self::default()
        };
        registry.reload();
        registry
    }

    /// Where Skills live: beside the characters, under the same `definitions` folder.
    ///
    /// A sibling rather than a subfolder of `characters`, because a Skill belongs to nobody —
    /// several characters may be given the same one, and filing it under the first person who
    /// used it would make that person its owner.
    pub fn skills_dir(&self) -> PathBuf {
        self.root.join("definitions/skills")
    }

    pub fn reload(&mut self) {
        self.skills.clear();
        self.problems.clear();
        self.files_seen = 0;

        let dir = self.skills_dir();
        // Absent is the ordinary first-run state: a vault with no Skills yet is not a problem,
        // it is a vault with no Skills yet. The character registry reports a missing directory
        // because a World with no characters in it is genuinely wrong.
        if !dir.is_dir() {
            return;
        }

        let paths = match crate::definition::authored_files(&dir) {
            Ok(paths) => paths,
            Err(problem) => {
                self.problems.push(problem);
                return;
            }
        };
        self.files_seen = paths.len();

        for path in paths {
            match load_one(&path) {
                Ok(loaded) => {
                    let id = loaded.definition.id.clone();
                    if let Some(first) = self.skills.get(&id) {
                        self.problems.push(format!(
                            "two skills claim '{id}': {} and {}",
                            first.path.display(),
                            loaded.path.display()
                        ));
                        continue;
                    }
                    self.problems.extend(loaded.definition.problems());
                    self.skills.insert(id, loaded);
                }
                Err(problem) => self.problems.push(problem),
            }
        }
    }

    /// True when any Skill file changed on disk since it was read.
    ///
    /// Same contract as the character registry's, because the reason is the same: these are
    /// authored files a person edits in their own editor, and a World that only noticed on
    /// restart would make hot reload a claim rather than a behaviour (ADR-0003).
    pub fn changed_on_disk(&self) -> bool {
        let dir = self.skills_dir();
        let on_disk = std::fs::read_dir(&dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("toml"))
                    .count()
            })
            .unwrap_or(0);
        if on_disk != self.files_seen {
            return true;
        }

        self.skills.values().any(|loaded| {
            let current = std::fs::metadata(&loaded.path)
                .and_then(|m| m.modified())
                .ok();
            current != loaded.modified
        })
    }

    pub fn get(&self, id: &SkillId) -> Option<&SkillDefinition> {
        self.skills.get(id).map(|l| &l.definition)
    }

    /// Every Skill, in id order.
    pub fn all(&self) -> impl Iterator<Item = &SkillDefinition> {
        self.skills.values().map(|l| &l.definition)
    }

    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

fn load_one(path: &Path) -> Result<LoadedSkill, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|source| format!("cannot read {}: {source}", path.display()))?;
    let definition: SkillDefinition = toml::from_str(&raw)
        .map_err(|source| format!("invalid skill at {}: {source}", path.display()))?;

    // The filename is the id. Letting them disagree would allow two files to claim one Skill,
    // and which one survived would depend on directory order.
    if path.file_stem().and_then(|s| s.to_str()) != Some(definition.id.as_str()) {
        return Err(format!(
            "skill at {} declares id '{}' but is not named '{}.toml'",
            path.display(),
            definition.id,
            definition.id
        ));
    }

    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    Ok(LoadedSkill {
        definition,
        path: path.to_path_buf(),
        modified,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Vault(PathBuf);

    impl Vault {
        fn new(name: &str) -> Self {
            static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!("epoch-skills-{name}-{n}"));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(dir.join("definitions/skills")).unwrap();
            Self(dir)
        }
        fn write(&self, file: &str, body: &str) {
            std::fs::write(self.0.join("definitions/skills").join(file), body).unwrap();
        }
    }

    impl Drop for Vault {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    const REVIEW: &str = r#"
id = "code-review"
name = "Code review"
summary = "How this project reviews a change."
method = "Read the diff twice. Say what would break before saying what is nice."
requires = ["mcp:playwright", "read_file"]
"#;

    #[test]
    fn a_vault_with_no_skills_is_a_vault_with_no_skills() {
        // Not a problem, and the asymmetry with the character registry is deliberate: a World
        // with no characters is wrong, and a World with no Skills is Tuesday.
        let dir = std::env::temp_dir().join("epoch-skills-absent");
        let _ = std::fs::remove_dir_all(&dir);
        let registry = SkillRegistry::load(&dir);

        assert!(registry.is_empty());
        assert!(registry.problems().is_empty(), "{:?}", registry.problems());
    }

    #[test]
    fn a_skill_is_read_with_what_it_needs_kept_as_a_request() {
        let v = Vault::new("read");
        v.write("code-review.toml", REVIEW);
        let registry = SkillRegistry::load(&v.0);

        let skill = registry
            .get(&SkillId::new("code-review").unwrap())
            .expect("it loaded");
        assert_eq!(skill.name, "Code review");
        assert!(skill.method.contains("Read the diff twice"));
        // `mcp:playwright`, not two dozen tool ids that are wrong next month.
        assert!(skill
            .requires
            .iter()
            .any(|r| r.as_str() == "mcp:playwright"));
        assert!(registry.problems().is_empty(), "{:?}", registry.problems());
    }

    #[test]
    fn one_broken_skill_does_not_take_the_others_with_it() {
        // The rule the character registry already keeps: a file somebody is halfway through
        // editing must not remove every other way of working from the crew.
        let v = Vault::new("broken");
        v.write("code-review.toml", REVIEW);
        v.write("half-typed.toml", "id = \"half-typed\"\nname =");
        let registry = SkillRegistry::load(&v.0);

        assert_eq!(registry.all().count(), 1, "the good one survived");
        assert_eq!(registry.problems().len(), 1);
        assert!(
            registry.problems()[0].contains("half-typed"),
            "and the problem names the file: {:?}",
            registry.problems()
        );
    }

    #[test]
    fn the_filename_is_the_id() {
        // Otherwise two files could claim one Skill and the survivor would depend on directory
        // order — which is how a character silently loses the way of working they were given.
        let v = Vault::new("misnamed");
        v.write("reviewing.toml", REVIEW);
        let registry = SkillRegistry::load(&v.0);

        assert!(registry.is_empty());
        assert!(
            registry.problems()[0].contains("is not named 'code-review.toml'"),
            "{:?}",
            registry.problems()
        );
    }

    #[test]
    fn a_skill_with_no_method_loads_and_says_what_is_wrong_with_it() {
        // Reported, not refused. Somebody editing a Skill in another window should see the
        // sentence rather than watch it disappear from the list.
        let v = Vault::new("methodless");
        v.write(
            "empty.toml",
            "id = \"empty\"\nname = \"Empty\"\nmethod = \"  \"\n",
        );
        let registry = SkillRegistry::load(&v.0);

        assert_eq!(registry.all().count(), 1);
        assert!(
            registry.problems()[0].contains("no steps"),
            "{:?}",
            registry.problems()
        );
    }

    #[test]
    fn a_file_edited_in_another_window_is_noticed() {
        let v = Vault::new("hot");
        v.write("code-review.toml", REVIEW);
        let registry = SkillRegistry::load(&v.0);
        assert!(!registry.changed_on_disk());

        // A new file counts, which is the case a modified-time check alone would miss.
        v.write(
            "second.toml",
            "id = \"second\"\nname = \"S\"\nmethod = \"Go.\"\n",
        );
        assert!(registry.changed_on_disk());
    }
}
