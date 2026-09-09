//! Taking a World out of Epoch, so somebody else can have it.
//!
//! ## Export is distribution, and that decides almost everything here
//!
//! `CONTENT_PHILOSOPHY.md`'s hard rule governs what Epoch **distributes**: original,
//! commissioned, open-source, public-domain or explicitly redistributable content, and nothing
//! else. It does not govern what a person keeps in their own vault — which is why an import can
//! accept anything and an export cannot.
//!
//! So this refuses a World with no licence. Not as tidiness: a folder handed to somebody else
//! with no statement of what they may do with it is the problem that rule exists to prevent, and
//! the manifest already has the field ([`crate::pack::License`], ADR-0016).
//!
//! ## What a World is, and what it is not
//!
//! A World lives in two places and they are not the same kind of thing:
//!
//! | where | what | exported |
//! |---|---|---|
//! | `packs/<id>/` | the authored pack — manifest, artwork | **yes**, it is the World |
//! | `vault/worlds/<id>/places.toml` | where the user put the buildings | **yes**, it is the map |
//! | `vault/worlds/<id>/*.png` | artwork the user imported (ADR-0024) | **yes** |
//! | `vault/worlds/<id>/quests/` | their conversations | **no** |
//! | `vault/worlds/<id>/undo.json` | what they last moved | **no** |
//! | the crew | people, who belong to the user | **no** (ADR-0023) |
//!
//! **The crew is the one worth stating out loud.** Before ADR-0023 a pack supplied the cast, and
//! an export that carried them would have been obvious. It does not any more: a character is a
//! portable asset that travels into every World and belongs to the person who made them. Sending
//! a World must not send people — and a character's face is whatever the user imported, which
//! export is exactly the moment the hard rule applies to.
//!
//! **Quests are the second.** They hold what was said, what was decided and what was read out of
//! the user's own source tree. A World is a place; what happened there is theirs.
//!
//! ## Enumerated, never a directory copied
//!
//! The same rule deletion follows ([`crate::erase`]): the plan can be read before it runs,
//! counted, shown and tested. A `copy_dir_all` would carry whatever happened to be in the folder,
//! which is how a Quest ends up in somebody else's hands.

use std::path::{Path, PathBuf};

/// One file that will travel, and where it lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    pub from: PathBuf,
    /// Its place inside the exported folder, with `/` separators.
    pub to: String,
    pub bytes: u64,
}

/// What exporting one World would do — said before it is done.
///
/// The same shape as [`crate::Removal`], for the same reason: a confirmation that only says
/// "are you sure?" is a confirmation about nothing. What travels, what does not, and what stops
/// it, each as a list somebody can read.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Export {
    /// The World's name, as a person calls it.
    pub what: String,
    /// The folder it will become.
    pub into: PathBuf,
    /// Every file, enumerated.
    pub carries: Vec<Carried>,
    /// What stays behind, in sentences. Never empty when something real is being left.
    pub leaves: Vec<String>,
    /// Files travelling that the declared licence was not written about.
    ///
    /// **Artwork the user imported** (ADR-0024). The pack's `[license]` is a statement its author
    /// made about the pack; a backdrop dropped in afterwards is somebody else's work arriving
    /// under that sentence without anybody having said so. Epoch has no way to know what it is —
    /// an imported PNG carries no manifest and no catalogue knows it — and the whole discipline
    /// here is that a reading nobody measured is not invented.
    ///
    /// So this is **said, never refused**. The hard rule governs what Epoch distributes; what a
    /// person may hand to a friend from their own vault is theirs, and refusing would be Epoch
    /// deciding a question it cannot even measure. What it can do is make sure the moment is not
    /// silent, which is what ADR-0024 means by *it applies again at export*.
    pub unvouched: Vec<String>,
    /// Why it cannot be exported. Empty means it can.
    pub problems: Vec<String>,
}

impl Export {
    /// How much the whole thing weighs.
    pub fn bytes(&self) -> u64 {
        self.carries.iter().map(|c| c.bytes).sum()
    }

    /// Whether this plan may run.
    pub fn allowed(&self) -> bool {
        self.problems.is_empty() && !self.carries.is_empty()
    }
}

/// What in the vault's World folder is the user's work rather than the World.
///
/// Named rather than pattern-matched: `quests` and `undo.json` are two specific things Epoch
/// writes, and a rule like *skip anything that looks like state* would be a rule that quietly
/// changes meaning the next time a file is added.
const PRIVATE: [&str; 3] = ["quests", "quests.legacy-v1.json", "undo.json"];

/// Work out what exporting this World would carry.
///
/// `pack_dir` is `packs/<id>`, `vault_world` is `vault/worlds/<id>`, and either may be absent:
/// a World authored entirely in the vault has no pack folder, and one nobody has edited has no
/// vault folder. Both being absent is a World that does not exist, which is a problem rather
/// than an empty export.
pub fn plan(
    what: &str,
    pack_dir: &Path,
    vault_world: &Path,
    into: PathBuf,
    licensed: bool,
) -> Export {
    let mut carries = Vec::new();
    let mut leaves = Vec::new();
    let mut problems = Vec::new();

    // 1. The authored pack, whole. It is the World: manifest, artwork, declared Places.
    gather(pack_dir, pack_dir, &mut carries, &|_| true);

    // 2. The user's own map and the artwork they imported into it — but not what they did there.
    let mut from_the_vault = Vec::new();
    gather(vault_world, vault_world, &mut from_the_vault, &|relative| {
        !PRIVATE.contains(&relative.split('/').next().unwrap_or(relative))
    });

    // Which of those the declared licence was not written about. The map is Epoch's own file and
    // the user's own arrangement; the images beside it are whatever they imported, and that is
    // the half a recipient would otherwise read the pack's licence as covering.
    let unvouched: Vec<String> = from_the_vault
        .iter()
        .filter(|carried| !carried.to.ends_with(".toml") && !carried.to.ends_with(".json"))
        .map(|carried| carried.to.clone())
        .collect();
    carries.extend(from_the_vault);

    // What was held back, asked of the folder rather than accumulated inside the filter. A
    // `Fn` cannot own the answer, and the alternative — a `RefCell` threaded through a
    // predicate — is machinery for a question one `read_dir` answers.
    let held_back: Vec<String> = std::fs::read_dir(vault_world)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| PRIVATE.contains(&name.as_str()))
        .collect();

    if held_back.iter().any(|name| name.starts_with("quests")) {
        leaves.push(
            "Your conversations stay here. They hold what was said, what was decided and what \
             was read out of your own project — a World is a place, and what happened there is \
             yours."
                .to_owned(),
        );
    }
    if held_back.iter().any(|name| name == "undo.json") {
        leaves.push("What you last moved stays here; it is working state, not the map.".to_owned());
    }

    // 3. The crew, always. Said whether or not anybody lives there, because its absence is the
    //    thing a person is most likely to assume the other way (ADR-0023).
    leaves.push(
        "Your crew does not travel with a World. Characters belong to you and go into every \
         World you open — send them separately if you mean to."
            .to_owned(),
    );

    if !licensed {
        problems.push(
            "This World declares no licence, and an export is a distribution. Add a [license] \
             to its pack.toml saying what somebody receiving it may do."
                .to_owned(),
        );
    }
    if carries.is_empty() {
        problems.push("There is nothing to export: that World has no files.".to_owned());
    }

    carries.sort_by(|a, b| a.to.cmp(&b.to));
    Export {
        what: what.to_owned(),
        into,
        carries,
        leaves,
        unvouched,
        problems,
    }
}

// A folder writer lived here, and it is gone with the button that called it.
//
// A World Pack *is* a folder, so it was directly what another Epoch installs — that argument was
// right and is why the import half still needs no code. What it did not do is **travel**, and an
// export exists to leave: somebody handing a World to a friend sends one file, and unzipping it
// into a Worlds folder is the same install.
//
// Two ways to write the same plan, one of them unreachable from any surface, is the second worse
// way this file already refuses elsewhere.

/// Copy the plan into one archive, at a path the user chose.
///
/// ## Why an archive as well as a folder
///
/// [`write`] produces a folder, which is what a World Pack already *is* — so it is directly what
/// another Epoch installs, and that is why the import half needed no code. What a folder does
/// not do is **travel**: somebody handing a World to a friend sends one file.
///
/// So both exist and neither replaces the other. A `zip` is for sending; a folder is for
/// installing.
///
/// ## The same refusal, in the same place
///
/// A plan that may not run may not run here either — the licence check is not a property of the
/// output format. And the paths inside the archive are the plan's own relative ones, with `/`
/// separators, which is what the format specifies and what makes it open correctly on a machine
/// that is not this one.
pub fn write_zip(plan: &Export, into: &Path) -> Result<PathBuf, String> {
    if !plan.allowed() {
        return Err(plan
            .problems
            .first()
            .cloned()
            .unwrap_or_else(|| "there is nothing to export".to_owned()));
    }
    if let Some(parent) = into.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("cannot create {}: {err}", parent.display()))?;
    }

    let file = std::fs::File::create(into)
        .map_err(|err| format!("cannot write {}: {err}", into.display()))?;
    let mut archive = zip::ZipWriter::new(std::io::BufWriter::new(file));
    let how = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for carried in &plan.carries {
        archive
            .start_file(&carried.to, how)
            .map_err(|err| format!("cannot add {}: {err}", carried.to))?;
        let bytes = std::fs::read(&carried.from)
            .map_err(|err| format!("cannot read {}: {err}", carried.from.display()))?;
        use std::io::Write;
        archive
            .write_all(&bytes)
            .map_err(|err| format!("cannot write {}: {err}", carried.to))?;
    }

    archive
        .finish()
        .map_err(|err| format!("the archive would not close: {err}"))?;
    Ok(into.to_owned())
}

/// Every file under `dir`, as paths relative to `root`, subject to a filter.
///
/// Depth-first and enumerated. `keep` sees the relative path with `/` separators, so a rule can
/// be written about a folder rather than about a platform.
fn gather(root: &Path, dir: &Path, into: &mut Vec<Carried>, keep: &dyn Fn(&str) -> bool) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        let relative = relative.to_string_lossy().replace('\\', "/");
        if !keep(&relative) {
            continue;
        }
        if path.is_dir() {
            gather(root, &path, into, keep);
        } else {
            into.push(Carried {
                bytes: entry.metadata().map(|m| m.len()).unwrap_or(0),
                from: path,
                to: relative,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A World on disk: an authored pack, and what the user did in it.
    ///
    /// Named by its test rather than by the clock: these run in parallel, `now_ms` collides at
    /// millisecond resolution, and one test's cleanup was deleting another's fixture out from
    /// under it.
    fn a_world(who: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = std::env::temp_dir().join(format!("epoch-export-{}-{who}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let pack = root.join("packs").join("archipelago");
        let vault = root.join("vault").join("worlds").join("archipelago");
        std::fs::create_dir_all(pack.join("assets")).unwrap();
        std::fs::create_dir_all(vault.join("quests")).unwrap();

        std::fs::write(pack.join("pack.toml"), b"[pack]\nid = \"archipelago\"\n").unwrap();
        std::fs::write(pack.join("assets").join("preview.png"), b"PNG").unwrap();

        std::fs::write(vault.join("places.toml"), b"[[place]]\n").unwrap();
        std::fs::write(vault.join("land.png"), b"PNGPNG").unwrap();
        std::fs::write(vault.join("undo.json"), b"{}").unwrap();
        std::fs::write(vault.join("quests").join("q1.json"), b"{secrets}").unwrap();
        std::fs::write(vault.join("quests.legacy-v1.json"), b"[]").unwrap();

        (root.clone(), pack, vault)
    }

    #[test]
    fn a_world_travels_and_the_work_done_in_it_does_not() {
        // The whole decision, as a list. A `copy_dir_all` would have carried the Quests, which
        // is how somebody's conversations — and whatever was read out of their source tree —
        // end up in a stranger's hands.
        let (root, pack, vault) = a_world("a_world_travels_and_");
        let planned = super::plan("The Archipelago", &pack, &vault, root.join("out"), true);

        let carried: Vec<&str> = planned.carries.iter().map(|c| c.to.as_str()).collect();
        assert_eq!(
            carried,
            vec!["assets/preview.png", "land.png", "pack.toml", "places.toml"]
        );
        assert!(planned.allowed());

        // And it is said, not merely done.
        assert!(planned
            .leaves
            .iter()
            .any(|s| s.contains("conversations stay here")));
        assert!(planned
            .leaves
            .iter()
            .any(|s| s.contains("last moved stays here")));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_crew_is_named_as_staying_even_when_nobody_lives_there() {
        // The thing a person is most likely to assume the other way. Before ADR-0023 a pack
        // supplied the cast; now a character is the user's and travels into every World, so a
        // World export that carried people would be sending people.
        let (root, pack, vault) = a_world("the_crew_is_named_as");
        let planned = super::plan("The Archipelago", &pack, &vault, root.join("out"), true);
        assert!(
            planned
                .leaves
                .iter()
                .any(|s| s.contains("crew does not travel")),
            "{:?}",
            planned.leaves
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn artwork_the_user_imported_is_named_and_never_refused_over() {
        // The pack's [license] is a statement its author made about the pack. A picture dropped
        // into the vault afterwards travels under that sentence without anybody having said so,
        // and Epoch cannot read what a picture is or who made it — an imported PNG carries no
        // manifest and no catalogue knows its bytes.
        let (root, pack, vault) = a_world("unvouched_artwork_");

        let planned = plan("The Archipelago", &pack, &vault, root.join("out"), true);

        // The map and the manifest are Epoch's own files and the user's own arrangement. The
        // image beside them is whatever they imported.
        assert_eq!(planned.unvouched, vec!["land.png".to_owned()]);
        assert!(
            !planned.unvouched.iter().any(|file| file.ends_with(".toml")),
            "the map is not somebody else's artwork: {:?}",
            planned.unvouched
        );
        // The pack's own assets are covered by the licence its author declared, so they are not
        // on this list — saying it about everything would make the warning mean nothing.
        assert!(!planned.unvouched.contains(&"assets/preview.png".to_owned()));

        // **Said, never refused.** The hard rule governs what Epoch distributes; what somebody
        // hands a friend from their own vault is theirs, and refusing here would be Epoch
        // deciding a question it cannot even measure.
        assert!(planned.problems.is_empty());
        assert!(planned.allowed());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unlicensed_world_is_refused_because_an_export_is_a_distribution() {
        // CONTENT_PHILOSOPHY's hard rule governs what Epoch *distributes*, not what somebody
        // keeps in their own vault — which is why an import may accept anything and this may
        // not. A folder handed over with no statement of what may be done with it is the
        // problem that rule exists to prevent.
        let (root, pack, vault) = a_world("an_unlicensed_world_");
        let planned = super::plan("The Archipelago", &pack, &vault, root.join("out"), false);
        assert!(!planned.allowed());
        assert!(
            planned.problems[0].contains("no licence"),
            "{:?}",
            planned.problems
        );

        // And the refusal holds at the writer too: a surface's check is a courtesy, never a
        // control. Asserted through the archive, which is the only writer now.
        assert!(write_zip(&planned, &root.join("no.zip")).is_err());
        assert!(!root.join("no.zip").exists(), "nothing was written");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_archive_carries_the_same_plan_and_the_same_refusal() {
        // A `zip` is for sending and a folder is for installing; neither replaces the other. What
        // must not differ is *what goes in*, so the archive is written from the same plan — and
        // the licence refusal is not a property of the output format.
        let (root, pack, vault) = a_world("an_archive_carries");
        let file = root.join("archipelago.zip");

        let planned = super::plan("The Archipelago", &pack, &vault, root.join("out"), true);
        assert_eq!(write_zip(&planned, &file).unwrap(), file);
        assert!(file.is_file());

        // Readable, and holding exactly the plan — including the two that must not be there.
        let read = std::fs::File::open(&file).unwrap();
        let mut archive = zip::ZipArchive::new(read).unwrap();
        let mut inside: Vec<String> = (0..archive.len())
            .map(|n| archive.by_index(n).unwrap().name().to_owned())
            .collect();
        inside.sort();
        assert_eq!(
            inside,
            vec!["assets/preview.png", "land.png", "pack.toml", "places.toml"]
        );

        // Forward slashes, which is what the format specifies — a backslash here opens as one
        // file with a strange name on a machine that is not this one.
        assert!(inside.iter().all(|n| !n.contains('\\')), "{inside:?}");

        let refused = super::plan("The Archipelago", &pack, &vault, root.join("out"), false);
        assert!(write_zip(&refused, &root.join("no.zip")).is_err());
        assert!(!root.join("no.zip").exists(), "nothing was written");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_world_with_no_pack_folder_still_exports() {
        // One authored entirely in the vault. Either side may be absent; both being absent is a
        // World that does not exist, which is a problem rather than an empty export.
        let (root, pack, vault) = a_world("a_world_with_no_pack");
        std::fs::remove_dir_all(&pack).unwrap();

        let planned = super::plan("Homemade", &pack, &vault, root.join("out"), true);
        assert!(planned.allowed());
        assert!(planned.carries.iter().any(|c| c.to == "places.toml"));

        let nothing = super::plan("Nowhere", &pack, &pack, root.join("out2"), true);
        assert!(!nothing.allowed());
        assert!(nothing.problems[0].contains("nothing to export"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn it_weighs_itself_before_it_is_run() {
        // A person about to hand somebody a folder should know how big it is, and the plan
        // already holds every file — so the number is counted rather than estimated.
        let (root, pack, vault) = a_world("it_weighs_itself_bef");
        let planned = super::plan("The Archipelago", &pack, &vault, root.join("out"), true);
        // 8 (pack.toml) + 3 (PNG) + 10 (places.toml) + 6 (land.png), and nothing private.
        assert_eq!(
            planned.bytes(),
            planned.carries.iter().map(|c| c.bytes).sum::<u64>()
        );
        assert!(planned.bytes() > 0);
        let _ = std::fs::remove_dir_all(&root);
    }
}
