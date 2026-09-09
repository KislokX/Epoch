//! The Workshop — where the crew's tools come from.
//!
//! Reading a catalogue of MCP servers and saying, for each one, **exactly what installing it
//! would run on this machine**. Nothing here installs anything; that is a separate, deliberate
//! act with the command already on screen.
//!
//! ## Why one source
//!
//! Four catalogues were measured before this module was written, and only one is machine
//! readable:
//!
//! | Source | What it answered |
//! |---|---|
//! | `registry.modelcontextprotocol.io` | A documented JSON API with everything needed to install. |
//! | `api.pulsemcp.com/v0beta` | **410 Gone.** |
//! | `mcpservers.org` | Logo and preview endpoints only; no index. |
//! | `appcypher/awesome-mcp-servers` | A CC0 markdown README, no consistent install commands. |
//!
//! So there is one [`Catalogue`] implementation and no scraping. A directory written for humans
//! is a place to *read* when deciding what to feature — parsing it would mean inventing fields
//! its authors never declared, and rebuilding the parser every time somebody restyles a page.
//!
//! ## Nothing is hidden and nothing is guessed
//!
//! Installing an MCP server means running a third party's program, and every tool it offers is
//! registered with every effect and no reversal because MCP declares neither (ADR-0008). So the
//! offer carries the literal command and arguments, and a server this build cannot run says so
//! in a sentence rather than failing after the button.
//!
//! Two things are read from the catalogue rather than assumed: which inputs a server needs
//! (`isRequired`, `isSecret`, `description`, `default`, `placeholder`, `choices` — so the form is
//! generated, never written per server) and which runtime it wants. Where `runtimeHint` is absent
//! — which is common — the runner comes from `registryType` by the mapping the specification
//! describes, and an unknown `registryType` is **refused rather than guessed**.

use epoch_models::quiet::Quiet;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// How long to wait for the catalogue. Short: a Workshop that hangs is worse than a cold one,
/// because the cached listing is right there and already useful.
const TIMEOUT: u64 = 12;
const CONNECT_TIMEOUT: u64 = 6;

/// Where the catalogue lives. The registry the MCP specification itself publishes.
const REGISTRY: &str = "https://registry.modelcontextprotocol.io/v0/servers";

/// One server, as the Workshop shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listing {
    /// The catalogue's canonical name — `io.github.microsoft/playwright`. Identity, never shown
    /// as the title.
    pub name: String,
    /// What to call it on screen.
    pub title: String,
    pub description: String,
    pub version: String,
    /// Who published it, read from the reverse-DNS half of the name. **Provenance**: installing
    /// runs their code, so who they are belongs next to the button and not behind it.
    pub publisher: String,
    /// Where the source is, when the catalogue says.
    pub repository: Option<String>,
    /// Which directory inside that repository this entry is, when it is a monorepo.
    ///
    /// Carried because it is where the *useful* README lives: a monorepo's root README is about
    /// the monorepo, and what somebody installed is one directory inside it.
    pub subfolder: Option<String>,
    /// Whether the registry still lists it as active. A withdrawn server says so rather than
    /// looking like any other card.
    pub active: bool,
    /// Every way this server is offered, in the order this build prefers them.
    pub offers: Vec<Offer>,
    /// The id Epoch would file it under. Derived, and checked against the id rules a configured
    /// server has to obey.
    pub suggested_id: String,
}

/// One way a catalogue entry could become a working connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Offer {
    /// A program Epoch would start. The only kind the bridge can currently use.
    Start {
        /// Which registry the package comes from: `npm`, `pypi`, `oci`, …
        registry: String,
        /// The package, exactly as the catalogue names it.
        package: String,
        /// The program Epoch would execute — already in this platform's spelling.
        command: String,
        /// Its arguments, in order. Shown verbatim: this is the whole of what would run.
        args: Vec<String>,
        /// What the server needs told before it can work.
        inputs: Vec<Input>,
    },
    /// A hosted server Epoch would talk to over HTTP.
    ///
    /// Listed and **not** offered. The bridge speaks to servers it starts; a hosted one needs a
    /// transport that does not exist here yet. Saying that is information; hiding the card would
    /// make a server the user can plainly find on the registry look absent.
    Reach { transport: String, url: String },
    /// Described in a way this build cannot carry out.
    Unusable { why: String },
}

/// Something a server must be told before it will run.
///
/// Read from the catalogue rather than written per server: a Workshop that knew Notion wants
/// `NOTION_TOKEN` would need editing to add the next server, which is the mistake ADR-0026
/// named for Provider controls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Input {
    pub name: String,
    pub description: String,
    pub required: bool,
    /// A token or password. **Decides where the value is stored**, not merely how it is drawn.
    pub secret: bool,
    pub default: Option<String>,
    pub placeholder: Option<String>,
    /// A closed set of values, when the catalogue declares one.
    pub choices: Vec<String>,
}

/// Which runners this machine actually has.
///
/// Measured by asking each program its version, the same way agents are measured, and for the
/// same reason: installed and not installed have different fixes, and a guess sends people to
/// fix the wrong thing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Runtimes {
    pub present: Vec<String>,
}

impl Runtimes {
    /// Ask this machine what it has. Spawns one short process per distinct runner.
    pub fn measure() -> Self {
        Self {
            present: ["npx", "uvx", "docker"]
                .into_iter()
                .filter(|runner| responds(runner))
                .map(str::to_owned)
                .collect(),
        }
    }

    pub fn has(&self, runner: &str) -> bool {
        self.present.iter().any(|p| p == runner)
    }
}

/// Whether a runner is on this machine, asked rather than assumed.
fn responds(runner: &str) -> bool {
    std::process::Command::new(as_this_platform_writes_it(runner))
        .quiet()
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// A Node launcher is `npx.cmd` on Windows, not `npx`.
///
/// The same correction a configured server already documents. Without it the failure reads
/// "program not found" for a program the user can plainly see is installed.
///
/// **Only `npx`, and that was measured rather than reasoned about.** A first version applied the
/// suffix to `uvx` too, on the symmetry that both are "the thing that runs a package". Asking
/// this machine disagreed: `npx` is `C:\Program Files\nodejs\npx.cmd` — a shim, and Windows
/// process creation appends only `.exe`, so the suffix is required — while `uvx` is a real
/// `uvx.exe` that resolves bare. The symmetric rule would have reported `uvx` missing on a
/// machine that has it, and sent the user to install something already installed.
fn as_this_platform_writes_it(runner: &str) -> String {
    if cfg!(windows) && runner == "npx" {
        format!("{runner}.cmd")
    } else {
        runner.to_owned()
    }
}

/// Which runner a package registry implies when the catalogue does not name one.
///
/// The specification says `runtimeHint` *should* be present when runtime arguments are — which
/// means it is routinely absent otherwise, and measurement agreed: most npm entries omit it. So
/// the standard runner for the registry is used, and an unrecognised registry is refused rather
/// than run with a guess.
fn runner_for(registry: &str) -> Option<&'static str> {
    match registry {
        "npm" => Some("npx"),
        "pypi" => Some("uvx"),
        "oci" => Some("docker"),
        _ => None,
    }
}

impl Listing {
    /// The offer this build would actually use, and whether this machine can.
    ///
    /// Preference order is the order the catalogue's packages were declared in, filtered to the
    /// ones that can start. A machine with `npx` but no `docker` gets the npm package even when
    /// the author listed the container first — and which one was chosen is visible, because the
    /// command is on screen.
    pub fn ready(&self, runtimes: &Runtimes) -> Option<&Offer> {
        self.offers.iter().find(|offer| match offer {
            Offer::Start { command, .. } => runtimes.has(base(command)),
            _ => false,
        })
    }

    /// Why nothing can be installed, in the user's terms. `None` when something can.
    ///
    /// Said **before** the button rather than after a failure: a missing runtime is a fact about
    /// this machine that is knowable in advance, and Epoch never installs one silently.
    pub fn blocked(&self, runtimes: &Runtimes) -> Option<String> {
        if self.ready(runtimes).is_some() {
            return None;
        }
        let wanted: Vec<&str> = self
            .offers
            .iter()
            .filter_map(|offer| match offer {
                Offer::Start { command, .. } => Some(base(command)),
                _ => None,
            })
            .collect();
        Some(match wanted.first() {
            Some(runner) => format!(
                "needs {runner}, which is not on this machine. Install it and open the Workshop \
                 again — Epoch never installs a runtime for you."
            ),
            None if self.offers.iter().any(|o| matches!(o, Offer::Reach { .. })) => {
                "is a hosted server. Epoch connects to servers it starts; talking to a hosted one \
                 is not built yet."
                    .into()
            }
            None => "is described in a way this build cannot run.".into(),
        })
    }
}

/// A command without its platform spelling, so `npx.cmd` is still `npx`.
fn base(command: &str) -> &str {
    command.strip_suffix(".cmd").unwrap_or(command)
}

/// What the catalogue answered, and how to ask for more of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub listings: Vec<Listing>,
    /// Where the next page starts. `None` is the end.
    pub next: Option<String>,
    /// When this was fetched, in milliseconds since the epoch.
    ///
    /// Shown when the answer came from the cache, so a stale catalogue names its own date instead
    /// of pretending to be current. A number rather than a formatted date: the surface knows the
    /// reader's locale and the Engine does not.
    pub fetched_ms: Option<u64>,
    /// Why the catalogue could not be reached, when it could not. The listings are then whatever
    /// was cached, which may be nothing.
    pub problem: Option<String>,
}

/// One catalogue entry with this machine's verdict already applied.
///
/// The verdict is computed here rather than sent as ingredients, so a surface never re-implements
/// "can this run" — a second copy of that rule would eventually disagree with the one the install
/// path uses, and the disagreement would look like a button that lies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shown {
    pub listing: Listing,
    /// The offer that would actually run, when one would.
    pub ready: Option<Offer>,
    /// Why nothing would, in the user's terms.
    pub blocked: Option<String>,
    /// The ids this entry is already installed under, if any.
    ///
    /// Matched on the catalogue name a server recorded when it was installed, **never on the
    /// id**. Installing renames on collision, so a second copy of Notion is `notion_2` — and an
    /// id match would call it a different server and cheerfully offer a third.
    ///
    /// A list rather than a flag because installing twice is still possible for anybody who
    /// means it; what must not happen is doing it by accident.
    pub installed_as: Vec<String>,
}

/// A page of the catalogue, as a surface shows it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    pub shown: Vec<Shown>,
    pub next: Option<String>,
    pub fetched_ms: Option<u64>,
    pub problem: Option<String>,
    /// What this machine can run at all. Shown once rather than repeated on every blocked card.
    pub runtimes: Runtimes,
}

impl Shelf {
    pub fn of(page: Page, runtimes: Runtimes, configured: &[crate::mcp::Configured]) -> Self {
        let mut shown: Vec<Shown> = page
            .listings
            .into_iter()
            .map(|listing| Shown {
                ready: listing.ready(&runtimes).cloned(),
                blocked: listing.blocked(&runtimes),
                installed_as: configured
                    .iter()
                    .filter(|s| came_from(s, &listing))
                    .map(|s| s.id.clone())
                    .collect(),
                listing,
            })
            .collect();
        // What this machine can actually use, first. Found by opening it: the registry's
        // unfiltered first page came back as nine cards of which one was installable, because
        // hosted servers are common and this build starts servers rather than reaching them.
        //
        // Sorted rather than filtered, and the distinction matters. Hiding them would make a
        // server the user can plainly find on the registry look absent, and would quietly hide
        // how much of the catalogue is waiting on a transport Epoch has not built. A **stable**
        // sort, so the registry's own order survives inside each half.
        shown.sort_by_key(|entry| entry.ready.is_none());
        Self {
            shown,
            next: page.next,
            fetched_ms: page.fetched_ms,
            problem: page.problem,
            runtimes,
        }
    }
}

/// Whether a configured server came from this catalogue entry.
///
/// Normally it says so itself, and that is the only answer that can be trusted: installing
/// renames on collision, so a second copy of Notion is `notion_2` and an id match would call it a
/// different server.
///
/// The fallback exists for servers installed **before** the origin was recorded, which would
/// otherwise be invisible to the Workshop forever — their card would keep offering INSTALL and
/// quietly make a third copy. It matches only the exact shape [`unclaimed`] produces: the
/// suggested id, or the suggested id with `_2`, `_3` after it. That is recognising Epoch's own
/// naming rather than guessing at a stranger's, and it applies only where nothing was recorded —
/// a server that names a *different* origin is never claimed by this entry.
fn came_from(server: &crate::mcp::Configured, listing: &Listing) -> bool {
    match &server.source {
        Some(recorded) => recorded == &listing.name,
        None => {
            let id = &server.id;
            id == &listing.suggested_id
                || id
                    .strip_prefix(&listing.suggested_id)
                    .and_then(|tail| tail.strip_prefix('_'))
                    .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
        }
    }
}

/// The MCP registry, read.
pub struct Catalogue {
    vault: std::path::PathBuf,
}

impl Catalogue {
    pub fn at(vault: &Path) -> Self {
        Self {
            vault: vault.to_path_buf(),
        }
    }

    fn cache(&self) -> std::path::PathBuf {
        self.vault.join("workshop-catalogue.json")
    }

    /// Search the catalogue, falling back to what was last fetched.
    ///
    /// **Offline-first.** A desktop application may not need the network to draw its own first
    /// frame, and that rule does not stop at the Launcher: the Workshop opens with what is
    /// already known, and a cold catalogue says which day it is from rather than showing an
    /// empty grid that reads as broken.
    ///
    /// Only an unfiltered first page is cached. A cache keyed by every search anybody ever typed
    /// would answer a *different* question offline than it did online, which is worse than
    /// answering none.
    pub fn search(&self, query: &str, cursor: Option<&str>) -> Page {
        let fresh = self.ask(query, cursor);
        let cacheable = query.trim().is_empty() && cursor.is_none();

        match fresh {
            Ok(page) => {
                if cacheable {
                    // A cache that cannot be written is not worth a message: the listings are on
                    // screen and correct, and the only cost is one more round of asking.
                    let _ = std::fs::write(
                        self.cache(),
                        serde_json::to_string(&page).unwrap_or_default(),
                    );
                }
                page
            }
            Err(why) => {
                let mut cached = self.remembered().unwrap_or_default();
                cached.next = None;
                cached.problem = Some(match (cached.listings.is_empty(), cacheable) {
                    (true, _) => format!("{why}, and nothing has been fetched yet."),
                    (false, true) => format!("{why}. Showing what was fetched before."),
                    (false, false) => {
                        format!("{why}. Searching needs the catalogue; this is the cached list.")
                    }
                });
                cached
            }
        }
    }

    fn remembered(&self) -> Option<Page> {
        serde_json::from_str(&std::fs::read_to_string(self.cache()).ok()?).ok()
    }

    fn ask(&self, query: &str, cursor: Option<&str>) -> Result<Page, String> {
        // `version=latest` is not decoration. Without it the registry returns every published
        // version of every server, so a search for "filesystem" came back as the same server
        // five times — measured against the live registry, not assumed.
        let mut request = ureq::builder()
            .timeout_connect(std::time::Duration::from_secs(CONNECT_TIMEOUT))
            .timeout(std::time::Duration::from_secs(TIMEOUT))
            .build()
            .get(REGISTRY)
            .query("version", "latest")
            .query("limit", "30");
        if !query.trim().is_empty() {
            request = request.query("search", query.trim());
        }
        if let Some(cursor) = cursor {
            request = request.query("cursor", cursor);
        }

        let body: serde_json::Value = request
            .call()
            .map_err(|err| match err {
                ureq::Error::Status(code, _) => format!("the catalogue answered {code}"),
                ureq::Error::Transport(_) => "the catalogue could not be reached".to_owned(),
            })?
            .into_json()
            .map_err(|_| "the catalogue sent something unreadable".to_owned())?;

        Ok(read_page(&body))
    }
}

/// Read one page of the registry's answer.
///
/// An entry that cannot be read is skipped rather than failing the page: one malformed server
/// must not be able to empty the Workshop.
pub fn read_page(body: &serde_json::Value) -> Page {
    let listings = body["servers"]
        .as_array()
        .map(|entries| entries.iter().filter_map(read_listing).collect())
        .unwrap_or_default();
    Page {
        listings,
        next: body["metadata"]["nextCursor"]
            .as_str()
            .map(str::to_owned)
            .filter(|c| !c.is_empty()),
        fetched_ms: Some(crate::quest::now_ms()),
        problem: None,
    }
}

fn read_listing(entry: &serde_json::Value) -> Option<Listing> {
    let server = &entry["server"];
    let name = server["name"].as_str()?.to_owned();
    let official = &entry["_meta"]["io.modelcontextprotocol.registry/official"];

    let mut offers: Vec<Offer> = server["packages"]
        .as_array()
        .map(|packages| packages.iter().map(read_package).collect())
        .unwrap_or_default();
    offers.extend(
        server["remotes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(read_remote),
    );

    Some(Listing {
        title: server["title"]
            .as_str()
            .filter(|t| !t.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| readable(&name)),
        description: server["description"]
            .as_str()
            .filter(|d| !d.trim().is_empty())
            .map(str::to_owned)
            // Never invented. A server nobody described says so, exactly as an undescribed MCP
            // tool does rather than being given a purpose.
            .unwrap_or_else(|| "The catalogue carries no description for this one.".to_owned()),
        version: server["version"].as_str().unwrap_or("—").to_owned(),
        publisher: publisher(&name),
        repository: server["repository"]["url"].as_str().map(str::to_owned),
        subfolder: server["repository"]["subfolder"]
            .as_str()
            .map(str::to_owned),
        // Absent means the registry did not say. Treated as active, because the alternative is
        // marking every entry of a registry that stops sending the field as withdrawn.
        active: official["status"].as_str().unwrap_or("active") == "active",
        suggested_id: suggested_id(&name),
        offers,
        name,
    })
}

fn read_package(package: &serde_json::Value) -> Offer {
    let registry = package["registryType"].as_str().unwrap_or_default();
    let identifier = package["identifier"].as_str().unwrap_or_default();
    let version = package["version"].as_str();

    let Some(runner) = package["runtimeHint"]
        .as_str()
        .filter(|hint| !hint.trim().is_empty())
        .or_else(|| runner_for(registry))
    else {
        return Offer::Unusable {
            why: format!("comes from '{registry}', which Epoch does not know how to run"),
        };
    };
    if identifier.is_empty() {
        return Offer::Unusable {
            why: "names no package to install".into(),
        };
    }

    let mut args: Vec<String> = package["runtimeArguments"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(literal_argument)
        .collect();
    // `npx` without it stops to ask whether to download, on a stdin nothing is reading — the
    // server would simply never start. The catalogue usually declares it; adding it when it did
    // not is the one argument Epoch contributes, and it is visible in the command like any other.
    if runner == "npx" && !args.iter().any(|a| a == "-y") {
        args.insert(0, "-y".into());
    }
    args.push(match version {
        // Pinned, deliberately. A version that floats is a third party's code changing under a
        // decision the user already made.
        Some(version) if runner == "npx" => format!("{identifier}@{version}"),
        _ => identifier.to_owned(),
    });
    args.extend(
        package["packageArguments"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(literal_argument),
    );

    Offer::Start {
        registry: registry.to_owned(),
        package: identifier.to_owned(),
        command: as_this_platform_writes_it(runner),
        args,
        inputs: package["environmentVariables"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(read_input)
            .collect(),
    }
}

/// An argument the catalogue already knows the value of.
///
/// One it does *not* — a `{placeholder}` the user is meant to fill — is dropped rather than
/// passed through, because sending a literal `{path}` to a program is worse than sending
/// nothing. Those arrive as inputs when this grows to need them.
fn literal_argument(argument: &serde_json::Value) -> Option<String> {
    let value = argument["value"]
        .as_str()
        .or_else(|| argument["default"].as_str())?;
    if value.contains('{') {
        return None;
    }
    match argument["name"].as_str().filter(|n| !n.is_empty()) {
        Some(flag) => Some(format!("{flag}={value}")),
        None => Some(value.to_owned()),
    }
}

fn read_input(raw: &serde_json::Value) -> Option<Input> {
    let name = raw["name"].as_str().filter(|n| !n.is_empty())?.to_owned();
    Some(Input {
        description: raw["description"]
            .as_str()
            .filter(|d| !d.trim().is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("'{name}', which the catalogue did not describe")),
        required: raw["isRequired"].as_bool().unwrap_or(false),
        secret: raw["isSecret"].as_bool().unwrap_or(false),
        default: raw["default"].as_str().map(str::to_owned),
        placeholder: raw["placeholder"].as_str().map(str::to_owned),
        choices: raw["choices"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c.as_str().map(str::to_owned))
            .collect(),
        name,
    })
}

fn read_remote(raw: &serde_json::Value) -> Option<Offer> {
    Some(Offer::Reach {
        transport: raw["type"].as_str().unwrap_or("http").to_owned(),
        url: raw["url"].as_str()?.to_owned(),
    })
}

/// Who published it, from the reverse-DNS half of the catalogue's name.
///
/// `io.github.microsoft/playwright` was published by `microsoft` on GitHub. Presented as the
/// catalogue's own claim — the registry verifies namespace ownership, Epoch does not, and it
/// must not appear to.
fn publisher(name: &str) -> String {
    let domain = name.split('/').next().unwrap_or(name);
    let parts: Vec<&str> = domain.split('.').collect();
    match parts.as_slice() {
        ["io", "github", who, ..] => format!("{who} on GitHub"),
        [.., last] => (*last).to_owned(),
        [] => domain.to_owned(),
    }
}

/// The id Epoch would file this under, obeying the rules a configured server has to obey.
fn suggested_id(name: &str) -> String {
    let tail = name.rsplit('/').next().unwrap_or(name);
    let cleaned: String = tail
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('_');
    if trimmed.is_empty() {
        "server".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// A catalogue name as a title, when the entry did not carry one.
fn readable(name: &str) -> String {
    let tail = name.rsplit('/').next().unwrap_or(name);
    tail.split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The values a user supplied for one server's inputs, on their way to being stored.
///
/// A separate type so the two destinations cannot be confused: a secret goes to the encrypted
/// store and never to a file in the vault, and that is decided by the catalogue's own `isSecret`
/// rather than by whoever writes the install path.
pub type Answers = BTreeMap<String, String>;

/// Add a catalogue entry to this vault's servers.
///
/// Returns the id it was filed under, which may not be the one suggested — **installing renames
/// on collision and never overwrites**, the same rule importing a Character Pack follows
/// (ADR-0026). Silently replacing a server somebody configured by hand, because a catalogue
/// entry happened to derive the same name, would be a data loss with no undo.
///
/// The credential separation is enforced here and nowhere else: a value the catalogue marked
/// `isSecret` goes to the encrypted store and only its *name* is written to `mcp.toml`. Nothing
/// downstream has to remember the rule, because the value never reaches the code that writes
/// the file.
pub fn install(
    vault: &Path,
    listing: &Listing,
    offer: &Offer,
    answers: &Answers,
) -> Result<String, String> {
    let Offer::Start {
        command,
        args,
        inputs,
        ..
    } = offer
    else {
        return Err("that is not something Epoch can start".into());
    };

    let mut servers = crate::mcp::Servers::load(vault);
    if let Some(problem) = &servers.problem {
        // Refusing rather than saving over it. `Servers::save` already declines to overwrite a
        // file it could not read; saying so before the install is the version somebody can act on.
        return Err(format!("your MCP configuration cannot be read: {problem}"));
    }
    let id = unclaimed(&listing.suggested_id, &servers);

    let mut entry = crate::mcp::Configured {
        id: id.clone(),
        command: command.clone(),
        args: args.clone(),
        // Where it came from, so the Workshop can recognise it later. Recorded here rather than
        // derived from the id, which the rename above may well have changed.
        source: Some(listing.name.clone()),
        ..Default::default()
    };

    let store = crate::secrets::Secrets::at(vault);
    for input in inputs {
        let value = answers
            .get(&input.name)
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .or_else(|| input.default.clone());
        let Some(value) = value else {
            if input.required {
                return Err(format!("{} needs a value", input.name));
            }
            continue;
        };
        if input.secret {
            store
                .put(
                    &epoch_kernel::SecretName::for_mcp_input(&id, &input.name),
                    &epoch_kernel::Secret::new(value),
                )
                // The entry is not written when the credential could not be stored. A server in
                // the file whose secret is not in the store is one that never starts, and the
                // reason would be a line in a problems list rather than this sentence.
                .map_err(|why| format!("{} could not be stored safely: {why}", input.name))?;
            entry.secrets.push(input.name.clone());
        } else {
            entry.env.insert(input.name.clone(), value);
        }
    }

    servers.servers.push(entry);
    servers.save(vault)?;

    // Fetched here, once, rather than when somebody asks. A character that had to reach the
    // network mid-answer would fail differently every time — and the moment the documentation is
    // wanted is the moment the connection is already not working.
    //
    // Never fatal: an install with no README is an install.
    if let Some(readme) = fetch_readme(listing, offer) {
        let at = docs_path(vault, &id);
        if let Some(parent) = at.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(at, readme);
    }

    Ok(id)
}

/// Most of a server's own documentation this build will keep.
///
/// A README is written for people and some are enormous. Kept whole up to here so the file on
/// disk stays something a person could open; what a character is *given* is cut again, smaller,
/// at the moment it asks.
const MAX_README: usize = 128 * 1024;

/// Where a server's own documentation is kept, once it has been fetched.
pub fn docs_path(vault: &Path, server: &str) -> PathBuf {
    vault.join("mcp-docs").join(format!("{server}.md"))
}

/// Fetch a server's README, so the crew can read it later.
///
/// **Why this is worth a network call at install time.** Asked how to reconnect a Spotify server,
/// a character answered with instructions for connecting apps inside ChatGPT — confidently, and
/// about the wrong product entirely. It had nothing about the actual server, so it reached for the
/// nearest thing it knew. The server's own README says exactly how to authenticate it, and the
/// catalogue already names where that README lives.
///
/// Fetched **once, on install**, rather than when somebody asks: a character that had to reach
/// the network mid-answer would fail differently every time, and the moment somebody needs the
/// documentation is the moment their connection is already not working.
///
/// Two sources, in this order and for this reason:
///
/// 1. **The package registry.** npm serves the README as a field of the package Epoch is actually
///    installing — one call, exactly the version being installed, no branch to guess.
/// 2. **The repository.** For everything else, and as the fallback. `HEAD` rather than `main`,
///    because a repository's default branch is its own business.
///
/// Failure is never fatal. An install with no README is an install; the Workshop says the
/// documentation could not be fetched and the server works exactly as before.
pub fn fetch_readme(listing: &Listing, offer: &Offer) -> Option<String> {
    if let Offer::Start {
        registry, package, ..
    } = offer
    {
        if registry == "npm" {
            if let Some(readme) = from_npm(package) {
                return Some(readme);
            }
        }
    }
    from_repository(listing.repository.as_deref()?, listing.subfolder.as_deref())
}

fn get(url: &str) -> Option<String> {
    let body = ureq::builder()
        .timeout_connect(std::time::Duration::from_secs(CONNECT_TIMEOUT))
        .timeout(std::time::Duration::from_secs(TIMEOUT))
        .build()
        .get(url)
        .call()
        .ok()?
        .into_string()
        .ok()?;
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.chars().take(MAX_README).collect())
}

fn from_npm(package: &str) -> Option<String> {
    let body: serde_json::Value = ureq::builder()
        .timeout_connect(std::time::Duration::from_secs(CONNECT_TIMEOUT))
        .timeout(std::time::Duration::from_secs(TIMEOUT))
        .build()
        .get(&format!("https://registry.npmjs.org/{package}"))
        .call()
        .ok()?
        .into_json()
        .ok()?;
    let readme = body["readme"].as_str()?.trim();
    // npm keeps this placeholder for packages that published none. It is not documentation.
    if readme.is_empty() || readme.eq_ignore_ascii_case("ERROR: No README data found!") {
        return None;
    }
    Some(readme.chars().take(MAX_README).collect())
}

/// The raw README for a repository, and for the subfolder within it when the catalogue names one.
///
/// The subfolder is tried **first**: a monorepo's root README is about the monorepo, and the
/// entry the user installed is one directory inside it.
fn from_repository(repository: &str, subfolder: Option<&str>) -> Option<String> {
    let raw = github_raw(repository)?;
    let mut tried = Vec::new();
    if let Some(sub) = subfolder.map(str::trim).filter(|s| !s.is_empty()) {
        let sub = sub.trim_matches('/');
        tried.push(format!("{raw}/{sub}/README.md"));
        tried.push(format!("{raw}/{sub}/readme.md"));
    }
    tried.push(format!("{raw}/README.md"));
    tried.push(format!("{raw}/readme.md"));
    tried.iter().find_map(|url| get(url))
}

/// `https://github.com/owner/repo(.git)` becomes the raw content root at the default branch.
///
/// `HEAD` rather than `main`: which branch a repository calls its default is its own business,
/// and guessing produces a 404 that looks like a missing README.
fn github_raw(repository: &str) -> Option<String> {
    let rest = repository
        .trim_end_matches('/')
        .strip_prefix("https://github.com/")
        .or_else(|| {
            repository
                .trim_end_matches('/')
                .strip_prefix("http://github.com/")
        })?;
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let owner = parts.next().filter(|p| !p.is_empty())?;
    let repo = parts.next().filter(|p| !p.is_empty())?;
    Some(format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/HEAD"
    ))
}

/// An id no configured server is already using.
fn unclaimed(wanted: &str, servers: &crate::mcp::Servers) -> String {
    let taken = |id: &str| servers.servers.iter().any(|s| s.id == id);
    if !taken(wanted) {
        return wanted.to_owned();
    }
    (2..)
        .map(|n| format!("{wanted}_{n}"))
        .find(|candidate| !taken(candidate))
        .unwrap_or_else(|| wanted.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from the live registry rather than written by hand. Every shape asserted below
    /// is one the real catalogue actually sent.
    fn search_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("../tests/fixtures/registry-search.json")).unwrap()
    }

    fn remote_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("../tests/fixtures/registry-remote.json")).unwrap()
    }

    fn find<'a>(page: &'a Page, name: &str) -> &'a Listing {
        page.listings
            .iter()
            .find(|l| l.name == name)
            .unwrap_or_else(|| panic!("no '{name}' in the fixture"))
    }

    #[test]
    fn a_package_becomes_the_literal_command_that_would_run() {
        let page = read_page(&search_fixture());
        let listing = find(&page, "com.pulsemcp/remote-filesystem");

        let Offer::Start {
            command,
            args,
            registry,
            ..
        } = &listing.offers[0]
        else {
            panic!("an npm package is something Epoch can start");
        };
        assert_eq!(registry, "npm");
        assert_eq!(base(command), "npx");
        // Pinned to the version the catalogue named. A floating version is a third party's code
        // changing under a decision the user already made.
        assert_eq!(args, &["-y", "remote-filesystem-mcp-server@0.1.5"]);
    }

    #[test]
    fn the_form_is_read_from_the_catalogue_rather_than_written_per_server() {
        let page = read_page(&search_fixture());
        let listing = find(&page, "com.pulsemcp/remote-filesystem");
        let Offer::Start { inputs, .. } = &listing.offers[0] else {
            panic!("expected a package");
        };

        let bucket = inputs.iter().find(|i| i.name == "GCS_BUCKET").unwrap();
        assert!(bucket.required, "the catalogue said this one is required");
        assert!(!bucket.secret);

        // `isSecret` decides where the value is stored, not merely how it is drawn.
        let key = inputs.iter().find(|i| i.name == "GCS_PRIVATE_KEY").unwrap();
        assert!(key.secret);
        assert!(!key.required, "secret and required are different questions");

        let public = inputs.iter().find(|i| i.name == "GCS_MAKE_PUBLIC").unwrap();
        assert_eq!(public.default.as_deref(), Some("false"));
    }

    #[test]
    fn what_this_machine_can_use_comes_first_and_the_rest_are_still_there() {
        // Found by opening the Workshop rather than by reading it: the registry's unfiltered
        // first page was nine cards of which one could be installed. Sorted, never filtered —
        // hiding them would make a server the user can plainly find look absent, and would hide
        // how much of the catalogue is waiting on a transport this build has not written.
        let page = read_page(&remote_fixture());
        let before = page.listings.len();
        let shelf = Shelf::of(
            page,
            Runtimes {
                present: vec!["npx".into()],
            },
            &[],
        );

        assert_eq!(shelf.shown.len(), before, "nothing was dropped");
        let first_blocked = shelf
            .shown
            .iter()
            .position(|s| s.ready.is_none())
            .expect("this fixture has hosted-only servers");
        assert!(
            shelf.shown[first_blocked..]
                .iter()
                .all(|s| s.ready.is_none()),
            "installable and blocked are interleaved"
        );
        assert!(
            first_blocked > 0,
            "this fixture has installable servers too"
        );
    }

    #[test]
    fn a_hosted_server_is_listed_and_honestly_refused() {
        // The bridge speaks to servers it starts. Hiding the card would make a server the user
        // can plainly find on the registry look absent; offering it would be a lie.
        let page = read_page(&remote_fixture());
        let listing = find(&page, "com.notion/mcp");

        assert!(matches!(listing.offers[0], Offer::Reach { .. }));
        assert!(listing.ready(&Runtimes::default()).is_none());
        let why = listing.blocked(&Runtimes::default()).unwrap();
        assert!(why.contains("hosted"), "{why}");
    }

    #[test]
    fn a_missing_runtime_is_named_before_the_button_not_after_a_failure() {
        let page = read_page(&search_fixture());
        let listing = find(&page, "com.pulsemcp/remote-filesystem");

        let why = listing.blocked(&Runtimes::default()).unwrap();
        assert!(why.contains("npx"), "{why}");
        assert!(
            why.contains("never installs a runtime"),
            "Epoch detects and explains; it does not install runtimes silently: {why}"
        );

        let have = Runtimes {
            present: vec!["npx".into()],
        };
        assert!(listing.blocked(&have).is_none());
        assert!(listing.ready(&have).is_some());
    }

    #[test]
    fn a_machine_with_one_runner_gets_the_package_it_can_actually_start() {
        // `com.mcparmory/notion` ships a pypi package and a container. A machine with only
        // Docker must be offered the container, and which one was chosen is visible because the
        // command is on screen.
        let page = read_page(&remote_fixture());
        let listing = find(&page, "com.mcparmory/notion");

        let docker_only = Runtimes {
            present: vec!["docker".into()],
        };
        let Offer::Start { command, .. } = listing.ready(&docker_only).unwrap() else {
            panic!("expected a startable offer");
        };
        assert_eq!(base(command), "docker");

        let uvx_only = Runtimes {
            present: vec!["uvx".into()],
        };
        let Offer::Start { command, .. } = listing.ready(&uvx_only).unwrap() else {
            panic!("expected a startable offer");
        };
        assert_eq!(base(command), "uvx");
    }

    #[test]
    fn a_package_with_no_runtime_hint_uses_the_registrys_own_runner() {
        // Measured: most npm entries omit `runtimeHint` entirely, because the specification only
        // asks for it when runtime arguments are present.
        let page = read_page(&remote_fixture());
        let listing = find(&page, "io.github.ai-aviate/better-notion");
        let Offer::Start { command, .. } = &listing.offers[0] else {
            panic!("an npm package with no hint is still an npm package");
        };
        assert_eq!(base(command), "npx");
    }

    #[test]
    fn a_registry_epoch_does_not_know_is_refused_rather_than_run_with_a_guess() {
        let offer = read_package(&serde_json::json!({
            "registryType": "something_new",
            "identifier": "whatever",
            "version": "1.0.0",
        }));
        let Offer::Unusable { why } = offer else {
            panic!("a runner nobody knows must not be invented");
        };
        assert!(why.contains("something_new"), "{why}");
    }

    #[test]
    #[cfg(windows)]
    fn only_the_shim_gets_a_suffix_because_that_is_what_this_machine_said() {
        // `npx` is a `.cmd` shim and Windows process creation appends only `.exe`, so it needs
        // its suffix. `uvx` is a real `uvx.exe` and resolves bare. The symmetric rule — both are
        // "the thing that runs a package", so both get `.cmd` — was written first and was wrong:
        // it reports `uvx` missing on a machine that has it.
        assert_eq!(as_this_platform_writes_it("npx"), "npx.cmd");
        assert_eq!(as_this_platform_writes_it("uvx"), "uvx");
        assert_eq!(as_this_platform_writes_it("docker"), "docker");
        // And the spelling never changes which runner an offer needs.
        assert_eq!(base("npx.cmd"), "npx");
    }

    /// What this machine actually has. Spawns real processes, so it is not part of the suite —
    /// but it is how the spelling above gets checked on a machine that is not this one.
    ///
    /// `cargo test -p epoch-engine --lib workshop -- --ignored --nocapture`
    #[test]
    #[ignore = "spawns real processes; a probe, not a unit test"]
    fn what_this_machine_can_actually_run() {
        let found = Runtimes::measure();
        println!("runners present: {:?}", found.present);
        assert!(
            !found.present.is_empty(),
            "no runner at all is possible, but on a development machine it usually means the \
             spelling is wrong rather than that nothing is installed"
        );
    }

    #[test]
    fn provenance_comes_from_the_name_and_is_never_dressed_up() {
        assert_eq!(
            publisher("io.github.microsoft/playwright"),
            "microsoft on GitHub"
        );
        assert_eq!(publisher("com.notion/mcp"), "notion");
        assert_eq!(publisher("ai.smithery/smithery-notion"), "smithery");
    }

    #[test]
    fn the_id_epoch_would_file_it_under_obeys_the_rules_a_server_id_has_to() {
        for name in [
            "io.github.microsoft/playwright",
            "com.pulsemcp/remote-filesystem",
            "io.github.n24q02m/better-notion-mcp",
        ] {
            let id = suggested_id(name);
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "'{id}' could not be a server id"
            );
            assert!(!id.is_empty());
        }
        assert_eq!(
            suggested_id("com.pulsemcp/remote-filesystem"),
            "remote_filesystem"
        );
    }

    #[test]
    fn asking_for_the_latest_version_is_what_stops_one_server_filling_the_page() {
        // Without `version=latest` the registry returns every published version, and a search
        // for "filesystem" came back as the same server five times. Reproduced against the live
        // registry; the fixture is what it answered once the parameter was added.
        let page = read_page(&search_fixture());
        let mut names: Vec<&str> = page.listings.iter().map(|l| l.name.as_str()).collect();
        let total = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), total, "one server, one card");
    }

    #[test]
    fn one_unreadable_entry_cannot_empty_the_workshop() {
        let body = serde_json::json!({
            "servers": [
                { "server": { "description": "nameless" } },
                { "server": { "name": "com.x/good", "version": "1.0.0", "packages": [] } },
            ],
            "metadata": { "count": 2 },
        });
        let page = read_page(&body);
        assert_eq!(page.listings.len(), 1);
        assert_eq!(page.listings[0].name, "com.x/good");
    }

    #[test]
    fn a_server_nobody_described_says_so_rather_than_being_given_a_purpose() {
        let page = read_page(&serde_json::json!({
            "servers": [{ "server": { "name": "com.x/quiet", "version": "1.0.0" } }],
        }));
        assert!(page.listings[0].description.contains("no description"));
    }

    fn vault(name: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "epoch-workshop-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_credential_reaches_the_encrypted_store_and_never_the_configuration_file() {
        // The separation ADR-0026 makes structural for Character Packs, applied here: only the
        // *name* of a secret is written, so `mcp.toml` cannot contain a key even if somebody
        // later forgets that it must not.
        let dir = vault("secret");
        // Both tests here write a real credential, so a locked store is nobody at the machine
        // rather than a fault. Only *locked* skips; *broken* still fails.
        if let epoch_secrets::Reach::Locked(why) = crate::secrets::Secrets::at(&dir).reachable() {
            eprintln!("skipped: this machine's credential store is locked — {why}");
            return;
        }
        let page = read_page(&search_fixture());
        let listing = find(&page, "com.pulsemcp/remote-filesystem");

        let answers: Answers = [
            ("GCS_BUCKET".to_owned(), "my-bucket".to_owned()),
            (
                "GCS_PRIVATE_KEY".to_owned(),
                "s3cr3t-key-material".to_owned(),
            ),
        ]
        .into_iter()
        .collect();

        let id = install(&dir, listing, &listing.offers[0], &answers).unwrap();
        assert_eq!(id, "remote_filesystem");

        let written = std::fs::read_to_string(dir.join("mcp.toml")).unwrap();
        assert!(
            written.contains("my-bucket"),
            "plain configuration is plainly in the file"
        );
        assert!(
            !written.contains("s3cr3t-key-material"),
            "a credential is in the file:\n{written}"
        );
        assert!(
            written.contains("GCS_PRIVATE_KEY"),
            "its name is kept, so it can be resolved"
        );

        let entry = &crate::mcp::Servers::load(&dir).servers[0];
        assert_eq!(
            entry.env.get("GCS_BUCKET").map(String::as_str),
            Some("my-bucket")
        );
        assert_eq!(entry.secrets, vec!["GCS_PRIVATE_KEY"]);
        // Untouched inputs with a default are carried; untouched inputs without one are absent
        // rather than set to an empty string, which several programs read as "configured".
        assert_eq!(
            entry.env.get("GCS_MAKE_PUBLIC").map(String::as_str),
            Some("false")
        );
        assert!(!entry.env.contains_key("GCS_ROOT_PATH"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_required_input_left_empty_refuses_before_anything_is_written() {
        let dir = vault("required");
        let page = read_page(&search_fixture());
        let listing = find(&page, "com.pulsemcp/remote-filesystem");

        let why = install(&dir, listing, &listing.offers[0], &Answers::new()).unwrap_err();
        assert!(why.contains("GCS_BUCKET"), "{why}");
        assert!(
            crate::mcp::Servers::load(&dir).servers.is_empty(),
            "a refused install leaves nothing behind"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn installing_renames_on_collision_and_never_overwrites() {
        // The same rule importing a Character Pack follows. Replacing a server somebody
        // configured by hand, because a catalogue entry derived the same name, is a data loss
        // with no undo.
        let dir = vault("collision");
        let mut servers = crate::mcp::Servers::default();
        servers.servers.push(crate::mcp::Configured {
            id: "remote_filesystem".into(),
            command: "mine".into(),
            ..Default::default()
        });
        servers.save(&dir).unwrap();

        let page = read_page(&search_fixture());
        let listing = find(&page, "com.pulsemcp/remote-filesystem");
        let answers: Answers = [("GCS_BUCKET".to_owned(), "b".to_owned())]
            .into_iter()
            .collect();

        let id = install(&dir, listing, &listing.offers[0], &answers).unwrap();
        assert_eq!(id, "remote_filesystem_2");

        let after = crate::mcp::Servers::load(&dir);
        assert_eq!(after.servers.len(), 2);
        assert_eq!(after.servers[0].command, "mine", "theirs is untouched");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_copy_is_recognised_as_the_same_entry_rather_than_a_new_one() {
        // Found by installing the same server twice. The first copy is `image_diff`, the second
        // is renamed to `image_diff_2`, and a check that matched on the id called the second a
        // different server — so the card kept offering INSTALL and a third would have followed.
        // What is matched on is the catalogue name each server recorded when it was installed.
        let dir = vault("twice");
        let page = read_page(&search_fixture());
        let listing = find(&page, "com.pulsemcp/remote-filesystem").clone();
        let answers: Answers = [("GCS_BUCKET".to_owned(), "b".to_owned())]
            .into_iter()
            .collect();

        let first = install(&dir, &listing, &listing.offers[0], &answers).unwrap();
        let second = install(&dir, &listing, &listing.offers[0], &answers).unwrap();
        assert_eq!(
            (first.as_str(), second.as_str()),
            ("remote_filesystem", "remote_filesystem_2")
        );

        let shelf = Shelf::of(
            read_page(&search_fixture()),
            Runtimes {
                present: vec!["npx".into()],
            },
            &crate::mcp::Servers::load(&dir).servers,
        );
        let card = shelf
            .shown
            .iter()
            .find(|s| s.listing.name == "com.pulsemcp/remote-filesystem")
            .unwrap();
        assert_eq!(
            card.installed_as,
            vec!["remote_filesystem", "remote_filesystem_2"]
        );

        // And a server the user added by hand belongs to no catalogue entry, so it is never
        // claimed by one.
        let mut servers = crate::mcp::Servers::load(&dir);
        servers.remember(crate::mcp::Configured {
            id: "mine".into(),
            command: "whatever".into(),
            ..Default::default()
        });
        let shelf = Shelf::of(
            read_page(&search_fixture()),
            Runtimes::default(),
            &servers.servers,
        );
        assert!(shelf
            .shown
            .iter()
            .all(|s| !s.installed_as.contains(&"mine".to_string())));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_server_installed_before_origins_were_recorded_is_still_recognised() {
        // Otherwise those cards keep offering INSTALL forever and quietly make a third copy.
        // The fallback matches only the shape `unclaimed` produces — Epoch's own naming, never a
        // stranger's — and only where nothing was recorded.
        let listing = find(
            &read_page(&search_fixture()),
            "com.pulsemcp/remote-filesystem",
        )
        .clone();
        let old = |id: &str| crate::mcp::Configured {
            id: id.into(),
            command: "npx.cmd".into(),
            ..Default::default()
        };

        assert!(came_from(&old("remote_filesystem"), &listing));
        assert!(came_from(&old("remote_filesystem_2"), &listing));
        assert!(came_from(&old("remote_filesystem_10"), &listing));
        // Not everything that merely starts the same way.
        assert!(!came_from(&old("remote_filesystem_backup"), &listing));
        assert!(!came_from(&old("remote_files"), &listing));
        assert!(!came_from(&old("remote_filesystem2"), &listing));

        // And a server that names a different origin is never claimed, however its id reads.
        let mut theirs = old("remote_filesystem");
        theirs.source = Some("com.somebody/else".into());
        assert!(!came_from(&theirs, &listing));
    }

    #[test]
    fn removing_a_server_says_which_credentials_it_was_holding() {
        // A credential left encrypted on disk after its server is gone is a secret nobody can
        // see, nobody can use, and nobody would ever think to remove.
        let dir = vault("forget");
        // Both tests here write a real credential, so a locked store is nobody at the machine
        // rather than a fault. Only *locked* skips; *broken* still fails.
        if let epoch_secrets::Reach::Locked(why) = crate::secrets::Secrets::at(&dir).reachable() {
            eprintln!("skipped: this machine's credential store is locked — {why}");
            return;
        }
        let page = read_page(&remote_fixture());
        let listing = find(&page, "io.github.awkoy/notion-mcp-server");
        let Offer::Start { inputs, .. } = &listing.offers[0] else {
            panic!("expected a package");
        };
        let secret = inputs
            .iter()
            .find(|i| i.secret)
            .expect("this entry declares a credential");
        let answers: Answers = inputs
            .iter()
            .filter(|i| i.required)
            .map(|i| (i.name.clone(), "value".to_owned()))
            .collect();

        let id = install(&dir, listing, &listing.offers[0], &answers).unwrap();
        let mut servers = crate::mcp::Servers::load(&dir);
        let held = servers.forget(&id).expect("it was there");
        assert!(held.contains(&secret.name), "{held:?}");
        assert!(
            servers.forget(&id).is_none(),
            "forgetting twice is not an error to invent"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_repository_becomes_the_address_its_readme_is_actually_at() {
        // Measured against GitHub, not reasoned about. `HEAD` rather than `main`: which branch a
        // repository calls its default is its own business, and guessing produces a 404 that
        // looks exactly like a missing README.
        assert_eq!(
            github_raw("https://github.com/awkoy/notion-mcp-server").unwrap(),
            "https://raw.githubusercontent.com/awkoy/notion-mcp-server/HEAD"
        );
        // The catalogue supplies both spellings.
        assert_eq!(
            github_raw("https://github.com/domdomegg/filesystem-mcp.git").unwrap(),
            "https://raw.githubusercontent.com/domdomegg/filesystem-mcp/HEAD"
        );
        assert_eq!(
            github_raw("https://github.com/pulsemcp/mcp-servers/").unwrap(),
            "https://raw.githubusercontent.com/pulsemcp/mcp-servers/HEAD"
        );
        // Anywhere else is not somewhere this knows how to read.
        assert!(github_raw("https://gitlab.com/who/what").is_none());
        assert!(github_raw("https://github.com/only-an-owner").is_none());
    }

    #[test]
    fn a_monorepo_entry_carries_the_directory_its_own_readme_is_in() {
        // A monorepo's root README is about the monorepo. What somebody installed is one
        // directory inside it, and that is where the useful document is.
        let page = read_page(&search_fixture());
        let listing = find(&page, "com.pulsemcp/remote-filesystem");
        assert_eq!(
            listing.subfolder.as_deref(),
            Some("experimental/remote-filesystem")
        );

        let alone = find(&page, "io.github.domdomegg/filesystem-mcp");
        assert_eq!(alone.subfolder, None);
    }

    #[test]
    fn a_hosted_server_cannot_be_installed_even_when_asked_directly() {
        let dir = vault("hosted");
        let page = read_page(&remote_fixture());
        let listing = find(&page, "com.notion/mcp");

        assert!(install(&dir, listing, &listing.offers[0], &Answers::new()).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_offline_workshop_shows_what_it_has_and_names_the_day_it_is_from() {
        let dir = std::env::temp_dir().join(format!("epoch-workshop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let catalogue = Catalogue::at(&dir);

        // Nothing fetched yet: the honest answer is that there is nothing, and why.
        let cold = catalogue.remembered();
        assert!(cold.is_none());

        let mut page = read_page(&search_fixture());
        page.fetched_ms = Some(1_760_000_000_000);
        std::fs::write(
            dir.join("workshop-catalogue.json"),
            serde_json::to_string(&page).unwrap(),
        )
        .unwrap();

        let warm = catalogue.remembered().unwrap();
        assert_eq!(warm.listings.len(), page.listings.len());
        assert_eq!(warm.fetched_ms, Some(1_760_000_000_000));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
