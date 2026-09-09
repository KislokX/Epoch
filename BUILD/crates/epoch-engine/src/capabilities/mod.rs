//! What the World can actually do.
//!
//! [`crate::capability`] is the *contract* — what a capability is, how it explains itself, how
//! it is registered. This is the set of them that exists, and it grows one at a time with a
//! reason each (Earn Complexity).
//!
//! Everything here needs a [`ProjectRoot`](crate::project::ProjectRoot): a capability with
//! nowhere to work has nothing to do, so a World with no project root offers none of them and
//! says why.
//!
//! Three of them only read, which is why they cost nothing to allow and never interrupt
//! anybody. [`files::WriteFile`] and [`files::EditFile`] change things, and they are the reason
//! the Trust Engine exists rather than things it now has to accommodate.
//!
//! Both record what they replaced ([`crate::journal`]), which is what lets them declare
//! `Undoable` — a reversal may only be declared when it is true.

mod glob;

pub mod connections;
/// Making a picture (ADR-0030).
pub mod draw;
pub mod files;
pub mod machine;
pub mod notes;
mod reader;
pub mod search;
pub mod see;
pub mod web;

use epoch_kernel::Descriptor;

use crate::capability::CapabilityRegistry;
use crate::project::ProjectRoot;

/// Everything this build knows how to do, whether or not a World can currently do it.
///
/// **The answer to "what may a character be asked for?"** - which is a different question from
/// "what can this World do right now". A character's requested capabilities are an intention
/// that travels with them into every World (ADR-0026: requested, never declared), so the list
/// offered to their author must not shrink because no folder has been chosen yet.
///
/// It replaces a hand-written constant in the Kernel. That constant had to be edited by hand
/// every time a capability was added, could never contain a tool from a connected MCP server -
/// the Kernel cannot know one exists - and quietly meant a character could not be given one at
/// all. This grows on its own, because it *is* the set.
pub fn catalogue() -> Vec<Descriptor> {
    let mut all = vec![
        connections::read_server_docs(),
        files::read_file(),
        files::list_files(),
        files::find_files(),
        files::write_file(),
        files::edit_file(),
        machine::run_command(),
        notes::read_note(),
        notes::search_notes(),
        // Sight belongs here for the same reason as the rest: this is what a character's author
        // may ask for, not what a World can do this minute. Whether anything on the machine can
        // actually see is answered when somebody looks — and answered as a sentence rather than
        // as a missing tick-box, because a capability that quietly disappears teaches nobody
        // anything (`crate::sight`).
        see::see_image(),
        web::fetch_url(),
        search::web_search(),
    ];
    all.sort_by(|a, b| a.id.cmp(&b.id));
    all
}

/// One source offering several capabilities under a single name.
///
/// The reason this type exists rather than a naming convention: **grouping by id prefix would be
/// a guess.** Only the thing that produced the capabilities knows which ones are its own, and a
/// server free to name its tools anything can trivially defeat a prefix rule — two servers whose
/// ids share a stem, or one whose tool is named after another. So the source states its members
/// and nothing infers them.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    /// What a character's file writes to ask for the whole source: `mcp:playwright`.
    pub id: String,
    /// What a person calls it. Never parsed, never matched on.
    pub label: String,
    /// The capability ids this source is offering **at this moment**. A count, a tooltip and an
    /// audit — deliberately not what gets written to the character's file, because the point of
    /// a group request is that it does not freeze.
    pub tools: Vec<String>,
    /// Whether the source actually answered when it was last asked.
    ///
    /// **A configured source always has a box, answering or not.** The first version omitted a
    /// silent one, reasoning that a box granting nothing is not a choice. Using it proved the
    /// opposite: a server that stopped starting — a broken argument is enough — took its
    /// tick-box off the screen, so a character who *had* been given it appeared not to have it,
    /// and the next save wrote that appearance to disk. The request was then really gone, and
    /// nothing had ever said so.
    ///
    /// A cold box is information. A missing one is a silent edit to somebody's character.
    pub answering: bool,
}

/// What a character is actually asking for, with every source request expanded.
///
/// Three cases, and they are three different facts:
///
/// - **Undecided** (`None`) — everything available. Nobody has chosen, so nothing is withheld;
///   the Trust Engine is what stops them, not an empty list somebody forgot to fill.
/// - **A capability id** — kept as-is, including one nothing offers. A request for something
///   this build cannot do is an intention, and dropping it here would silently rewrite what the
///   user authored (ADR-0026).
/// - **A source id** — replaced by whatever that source offers right now. A source that is no
///   longer connected contributes nothing, which is the honest answer: the request survives in
///   the file and starts working again when the server comes back.
pub fn resolve(
    wanted: Option<&epoch_kernel::RequestedCapabilities>,
    all: &[String],
    groups: &[Group],
) -> Vec<String> {
    let Some(chosen) = wanted else {
        return all.to_vec();
    };
    let mut out: Vec<String> = Vec::new();
    for request in chosen {
        match request.scope() {
            Some(_) => {
                if let Some(group) = groups.iter().find(|g| g.id == request.as_str()) {
                    out.extend(group.tools.iter().cloned());
                }
            }
            None => out.push(request.as_str().to_owned()),
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The inverse of [`resolve`]: what may be **asked for**, rather than what an ask means.
///
/// Every capability that belongs to no source, plus one entry per source. This is the list a
/// surface draws boxes from, and the list "nobody has decided" resolves to when somebody finally
/// decides one thing — starting from the expanded form there would write two dozen frozen ids
/// into a file the first time a box was ticked, which is the exact shape this replaced.
pub fn offerable(all: &[String], groups: &[Group]) -> Vec<String> {
    let inside: std::collections::BTreeSet<&str> = groups
        .iter()
        .flat_map(|g| g.tools.iter().map(String::as_str))
        .collect();
    let mut out: Vec<String> = all
        .iter()
        .filter(|id| !inside.contains(id.as_str()))
        .cloned()
        .chain(groups.iter().map(|g| g.id.clone()))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Every capability this build can offer in a World that has somewhere to work.
///
/// A World with no Project Root gets an **empty** registry rather than capabilities that fail
/// on first use. "Nobody here can read anything yet, choose a folder" is a sentence the
/// Launcher can say; "cannot read '.': no project root" three turns into a Quest is not.
pub fn for_project(
    root: ProjectRoot,
    undo: files::Undo,
    vault: impl Into<std::path::PathBuf>,
) -> CapabilityRegistry {
    let mut registry = without_project(vault);
    registry.register(Box::new(files::ReadFile::new(root.clone())));
    registry.register(Box::new(files::ListFiles::new(root.clone())));
    registry.register(Box::new(files::FindFiles::new(root.clone())));
    registry.register(Box::new(files::WriteFile::new(root.clone(), undo.clone())));
    registry.register(Box::new(files::EditFile::new(root.clone(), undo)));
    registry.register(Box::new(machine::RunCommand::new(root)));
    registry
}

/// Add the library tools, when this World has a library.
///
/// Separate from [`for_project`] because they answer a different question and read a different
/// folder (see [`crate::library`]). A World may have either, both or neither: notes with no
/// code and code with no notes are both ordinary, and neither is a degraded state.
/// Sight, given the pictures this conversation actually holds.
///
/// Its own function beside [`for_library`], and in the Engine rather than in the shell, because
/// the catalogue and a fully equipped World must agree — a capability registered only by the
/// surface is one the Engine's own tests cannot see, and the assertion that keeps those two
/// lists honest would pass while the editor offered nothing to tick.
///
/// `shared` is what makes the confinement structural: the map is built from the Quest, so there
/// is no argument a character could send that reaches a picture nobody offered it.
pub fn for_sight(
    registry: &mut CapabilityRegistry,
    images: impl Into<std::path::PathBuf>,
    shared: std::collections::BTreeMap<String, String>,
    eyes: Box<dyn crate::sight::Sight>,
) {
    registry.register(Box::new(see::SeeImage::new(images, shared, eyes)));
}

pub fn for_library(registry: &mut CapabilityRegistry, library: ProjectRoot) {
    registry.register(Box::new(notes::SearchNotes::new(library.clone())));
    registry.register(Box::new(notes::ReadNote::new(library)));
}

/// The capabilities that genuinely do not need a Project Root.
///
/// A World can be between projects and still have a browser or a configured MCP connection.
/// Hiding those because file tools need a root turns an unavailable *folder* into an unavailable
/// *World*. The caller may add project capabilities afterwards when a root is present.
///
/// `vault` is where Epoch keeps its own files — a connected server's documentation among them.
/// Not the project: reading what a server documents about itself is reading something Epoch
/// wrote, which is why it works in a World that has nowhere to work yet.
pub fn without_project(vault: impl Into<std::path::PathBuf>) -> CapabilityRegistry {
    let mut registry = CapabilityRegistry::new();
    registry.register(Box::new(web::FetchUrl));
    registry.register(Box::new(search::WebSearch::default()));
    registry.register(Box::new(connections::ReadServerDocs::new(vault)));
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playwright() -> Group {
        Group {
            id: "mcp:playwright".into(),
            label: "Playwright".into(),
            tools: vec!["playwright_click".into(), "playwright_close".into()],
            answering: true,
        }
    }

    fn everything() -> Vec<String> {
        vec![
            "playwright_click".into(),
            "playwright_close".into(),
            "read_file".into(),
            "write_file".into(),
        ]
    }

    #[test]
    fn asking_for_a_source_asks_for_whatever_it_offers_right_now() {
        // The whole point of the second shape. A file listing today's tool ids is frozen the
        // moment it is written; a file naming the source is never out of date.
        let mut wanted = epoch_kernel::RequestedCapabilities::new();
        wanted.insert(epoch_kernel::CapabilityRequest::new("read_file").unwrap());
        wanted.insert(epoch_kernel::CapabilityRequest::new("mcp:playwright").unwrap());

        assert_eq!(
            resolve(Some(&wanted), &everything(), &[playwright()]),
            vec!["playwright_click", "playwright_close", "read_file"]
        );

        // A tool the server adds later needs no edit to anybody's file.
        let mut grown = playwright();
        grown.tools.push("playwright_upload".into());
        assert!(resolve(Some(&wanted), &everything(), &[grown])
            .contains(&"playwright_upload".to_string()));
    }

    #[test]
    fn a_source_that_is_gone_contributes_nothing_and_the_request_survives() {
        // A laptop that is offline, or a server switched off. The honest answer is that the
        // character cannot reach those tools today — not that the user must re-author the file
        // to get them back when it returns.
        let mut wanted = epoch_kernel::RequestedCapabilities::new();
        wanted.insert(epoch_kernel::CapabilityRequest::new("mcp:playwright").unwrap());
        wanted.insert(epoch_kernel::CapabilityRequest::new("read_file").unwrap());

        assert_eq!(
            resolve(Some(&wanted), &everything(), &[]),
            vec!["read_file"]
        );
    }

    #[test]
    fn nobody_having_decided_is_not_the_same_as_having_chosen_none() {
        assert_eq!(resolve(None, &everything(), &[playwright()]), everything());
        assert!(resolve(
            Some(&epoch_kernel::RequestedCapabilities::new()),
            &everything(),
            &[playwright()]
        )
        .is_empty());
    }

    #[test]
    fn a_request_for_something_nothing_offers_is_kept_rather_than_quietly_dropped() {
        // ADR-0026: requested, never declared. `vision` is a real intention, and rewriting what
        // somebody authored because this build cannot honour it yet would erase the thing that
        // will select the capability on the day it exists.
        let mut wanted = epoch_kernel::RequestedCapabilities::new();
        wanted.insert(epoch_kernel::CapabilityRequest::new("vision").unwrap());
        assert_eq!(resolve(Some(&wanted), &everything(), &[]), vec!["vision"]);
    }

    #[test]
    fn what_may_be_asked_for_counts_a_source_once_and_hides_its_tools() {
        assert_eq!(
            offerable(&everything(), &[playwright()]),
            vec!["mcp:playwright", "read_file", "write_file"]
        );
    }

    #[test]
    fn the_catalogue_is_every_capability_a_fully_equipped_world_can_offer() {
        // The two must not drift: the catalogue is what a character's author may tick, and the
        // registry is what a turn can actually reach. A capability in one and not the other is
        // either a promise nothing keeps or a tool nobody can ask for.
        //
        // "Fully equipped" is now a root **and** a library. It was a root alone until a World
        // could know things as well as work somewhere, and this assertion is what noticed.
        let dir = std::env::temp_dir().join("epoch-caps-catalogue");
        std::fs::create_dir_all(&dir).unwrap();
        let root = ProjectRoot::open(dir.to_str().unwrap()).unwrap();
        let mut registry = for_project(
            root.clone(),
            files::Undo::new(dir.join("vault"), "test"),
            dir.join("vault"),
        );
        for_library(&mut registry, root);
        // Blind, because what a World *offers* does not depend on whether anything can see: a
        // character reaching for it is answered with a sentence either way (`crate::sight`).
        for_sight(
            &mut registry,
            dir.join("images"),
            Default::default(),
            Box::new(crate::sight::Blind),
        );

        let offered: Vec<String> = catalogue().iter().map(|d| d.id.to_string()).collect();
        let live: Vec<String> = registry
            .describe_all()
            .iter()
            .map(|d| d.id.to_string())
            .collect();
        assert_eq!(offered, live);
    }

    #[test]
    fn a_catalogue_entry_says_the_same_thing_the_capability_does() {
        // The descriptor moved out of `describe` rather than being copied beside it. If it had
        // been copied, this is the test that would have caught the day they disagreed.
        let dir = std::env::temp_dir().join("epoch-caps-catalogue-text");
        std::fs::create_dir_all(&dir).unwrap();
        let root = ProjectRoot::open(dir.to_str().unwrap()).unwrap();
        let mut registry = for_project(
            root.clone(),
            files::Undo::new(dir.join("vault"), "test"),
            dir.join("vault"),
        );
        for_library(&mut registry, root);
        for_sight(
            &mut registry,
            dir.join("images"),
            Default::default(),
            Box::new(crate::sight::Blind),
        );

        assert_eq!(catalogue(), registry.describe_all());
    }

    #[test]
    fn a_world_with_a_root_offers_what_this_build_can_do() {
        let dir = std::env::temp_dir().join("epoch-caps-registry");
        std::fs::create_dir_all(&dir).unwrap();
        let root = ProjectRoot::open(dir.to_str().unwrap()).unwrap();

        let registry = for_project(
            root,
            files::Undo::new(dir.join("vault"), "test"),
            dir.join("vault"),
        );
        // Registration validates every descriptor, so this also asserts none of them
        // contradicts itself.
        assert!(registry.problems().is_empty(), "{:?}", registry.problems());
        let names: Vec<String> = registry
            .describe_all()
            .iter()
            .map(|d| d.id.to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "edit_file",
                "fetch_url",
                "find_files",
                "list_files",
                "read_file",
                "read_server_docs",
                "run_command",
                "web_search",
                "write_file",
            ]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_world_without_a_project_still_offers_the_web() {
        let names: Vec<String> = without_project(std::env::temp_dir().join("epoch-nowhere"))
            .describe_all()
            .iter()
            .map(|d| d.id.to_string())
            .collect();
        // `read_server_docs` is here too: it reads a file Epoch wrote into its own vault, not
        // the project, so a World between projects can still read what its servers document.
        assert_eq!(names, vec!["fetch_url", "read_server_docs", "web_search"]);
    }
}
