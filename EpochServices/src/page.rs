//! The one page this program has.
//!
//! ## Why a served page and not a window
//!
//! This program already needs an HTTP listener — that is its whole job. Adding a webview
//! framework on top of a server that exists would be the heaviest way to draw four fields, and
//! it would also be the only reason this could not run on a headless Linux box that somebody
//! reaches over SSH.
//!
//! So the interface is one page, served on loopback, in Epoch's own visual language: the same
//! dark ground, the same gold, the same monospace. Everything is inline — no font from a CDN,
//! no script from anywhere. A program whose job is to be a careful door does not fetch its own
//! appearance from the internet.
//!
//! ## What it says when nothing has happened
//!
//! Unpaired is not an error and does not look like one. It is what every machine is before
//! somebody pairs it, and the page's whole content in that state is the one thing to do next.

use epoch_kernel::Have;

use crate::keep::Bond;

/// Which tab is showing.
///
/// Two URLs rather than a script: this program's CSP is `script-src: 'none'`, and that is the
/// reason a careful door can be trusted rather than an obstacle to route around.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Connect,
    /// The programs on this machine that can run a model, and the one button each needs.
    ///
    /// **Its own tab because it answers its own question** (owner, 2026-08-20). It lived inside
    /// Settings, where it was the largest thing on a page otherwise about `hf` and a directory
    /// — and *start the thing that runs models* is not a setting. It is the second thing
    /// somebody does on a machine they are lending, right after connecting it.
    Services,
    Workshop,
    /// What is *on* this machine, as opposed to what could be.
    ///
    /// **Its own tab under the Workshop, for the reason the Host's MODELS deck is its own
    /// deck.** The Workshop is a catalogue and a search — ranked, speculative, about models that
    /// mostly are not here. This is an inventory: a list of facts with a size beside each, and
    /// the only irreversible button either surface has. On a lent machine it matters more than
    /// on the Host, because *this* is the computer holding the thirty gigabytes.
    Models,
    /// What this machine can draw with (ADR-0029 §9, and the two-surfaces rule).
    ///
    /// **Its own tab for the same reason the Models Workshop is one.** A picture drawn on this
    /// computer loads *this* computer's checkpoints and LoRAs; installing them on the Host would
    /// put a six-gigabyte file where nothing that draws can reach it. A Brain is not a
    /// checkpoint (ADR-0031's amendment), so it is not another row on the models shelf either.
    Creations,
    /// What this machine has, as opposed to what it is lending.
    ///
    /// The Workshop answers *will it run here*; this answers *what can this machine do at all*.
    /// They were one card for a while and it was the wrong shape: a tool being installed is not
    /// news about a model.
    Settings,
}

/// The Connect tab, wrapped in the window.
pub fn render(
    bond: &Bond,
    have: &Have,
    address: &str,
    said: Option<&str>,
    // The code this machine is showing, and how many seconds it has left.
    showing: Option<(&str, u64)>,
) -> String {
    let body = format!(
        r#"{notice}
  {body}

  <section class="card">
    <span class="label">This machine</span>
    <p>{name}</p>
    <p class="quiet">{address}</p>
    <p class="quiet">{hardware}</p>
    <span class="label">Models here</span>
    <p class="quiet">{models}</p>
  </section>"#,
        notice = said
            .map(|text| format!(r#"<p class="notice">{}</p>"#, escape(text)))
            .unwrap_or_default(),
        body = if bond.paired() {
            paired(bond, have)
        } else {
            waiting(showing)
        },
        name = escape(&crate::here::name()),
        address = escape(address),
        hardware = escape(&hardware(have)),
        models = if have.models.is_empty() {
            "None. Weigh one in the Workshop and pull it from there.".to_owned()
        } else {
            escape(&have.models.join(" · "))
        },
    );
    // One second while a code is counting down, and nothing otherwise. There is no form on
    // screen in that state, so there is nothing for a reload to lose.
    frame(Tab::Connect, &body, showing.map(|_| 1))
}

/// One window, two tabs, **one frame** — so the second page cannot drift into looking like a
/// different program.
///
/// `refresh_every` adds a meta refresh, which is how anything changes over time in a page with no
/// script. **Seconds rather than a flag**, because the two things that need it need different
/// ones: a download reports progress every couple of seconds, and a countdown has to tick every
/// one or it is not a countdown — it sat at `4:59` until something else happened to reload the
/// page.
///
/// `None` the rest of the time. A page that reloaded itself forever would be a page nobody could
/// type into, which is also why the Connect tab shows **one direction at a time**: while a code
/// is ticking there is no form on screen for a reload to empty.
pub fn frame(tab: Tab, body: &str, refresh_every: Option<u32>) -> String {
    format!(
        r#"<!doctype html>
<html lang="en"><head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
{refresh}
<title>EpochServices</title>
<style>{STYLE}</style>
</head><body class="{tab}">
<main>
  <header>
    <span class="mark"></span>
    <div>
      <h1>EpochServices</h1>
      <p class="quiet">Lends this machine's models to an Epoch Host.</p>
    </div>
  </header>

  <nav class="tabs">
    <a href="/" class="{connect}">CONNECT</a>
    <a href="/services" class="{services}">START SERVICES</a>
    <a href="/workshop" class="{workshop}">MODELS WORKSHOP</a>
    <a href="/models" class="{models}">MODELS</a>
    <a href="/creations" class="{creations}">CREATIONS WORKSHOP</a>
    <a href="/settings" class="{settings}">SETTINGS</a>
  </nav>

{body}

  <footer class="quiet">
    Every runtime stays on 127.0.0.1. Nothing here is exposed to your network except this
    program, and this program answers nobody without the secret your Host gave it. A turn names
    which runtime should run it; it can never name where one is.
    <br>
    <!--
      **Which copy is answering.** A second copy cannot take the port, so a new window can open
      onto an older program still holding it — and an older program answers `no` to routes it
      does not have, which reads as a bug in the new one. The version is the cheapest way to
      tell those apart, and it belongs on the page rather than in an About box nobody opens.
    -->
    EpochServices v{version}
  </footer>
</main>
</body></html>"#,
        refresh = match refresh_every {
            Some(seconds) => format!(r#"<meta http-equiv="refresh" content="{seconds}">"#),
            None => String::new(),
        },
        version = env!("CARGO_PKG_VERSION"),
        tab = match tab {
            Tab::Connect => "connect",
            Tab::Workshop => "workshop",
            // A path to a binary is long, and a column narrow enough for Connect would wrap one
            // across three lines.
            Tab::Services => "workshop",
            // The same wide frame: a path to a model is long, and the row carries three
            // buttons beside it.
            Tab::Models => "workshop",
            // The same wide frame: a grid of asset cards needs the width the Connect tab does
            // not have.
            Tab::Creations => "workshop",
            // The same wide frame the Workshop uses: a path to a binary is long, and a column
            // narrow enough for the Connect tab would wrap one across three lines.
            Tab::Settings => "workshop",
        },
        connect = if tab == Tab::Connect { "on" } else { "" },
        services = if tab == Tab::Services { "on" } else { "" },
        workshop = if tab == Tab::Workshop { "on" } else { "" },
        models = if tab == Tab::Models { "on" } else { "" },
        creations = if tab == Tab::Creations { "on" } else { "" },
        settings = if tab == Tab::Settings { "on" } else { "" },
    )
}

/// START, when the program is here and its server is not answering.
///
/// **The machine's own terminal, not a child of this program.** A server EpochServices spawned
/// would die when this window closes and would print where nobody can read it — and the whole
/// point of starting one is that it outlives the button.
///
/// Nothing beside something already serving: that button's job is already done.
/// The one press that makes every model this machine has usable by the runtime beside it.
///
/// ## Why a machine that *lends* needs this most
///
/// Measured on the paired MacBook, 2026-08-21: Ollama answering with 8 models, llama.cpp
/// answering with **none**. Both were installed, both were running, and only one of them could
/// be asked for anything — because the other two runtimes only ever see files, and every model
/// on that machine was inside Ollama's store under a name with no extension.
///
/// The Host grew a shelf for exactly that and a lent machine did not, which made *what this
/// machine can lend* depend on which program you looked at it through. Same policy, same
/// `epoch-models` functions, two surfaces.
///
/// Ollama is absent from this deliberately: it does not take a file from a directory, it
/// imports — three and a half minutes of hashing to produce a second name for a model it
/// already has. The Host offers that as its own separate press, and it is not what this button
/// means.
fn shelf_button(one: &epoch_models::runtimes::Available) -> String {
    let (label, why) = match one.id {
        "llama_cpp" => (
            "START WITH EVERYTHING",
            "Serves every model this machine has, loading one when a turn asks for it.",
        ),
        "lm_studio" => (
            "LEND IT EVERYTHING",
            "Puts every model this machine has on LM Studio's shelf, linked rather than copied.",
        ),
        _ => return String::new(),
    };
    // Only when it is here. Offering to stock a shelf for a program that is not installed would
    // be a button whose only outcome is an explanation.
    if !one.installed {
        return String::new();
    }
    format!(
        r#"<form method="post" action="/shelf">
      <input type="hidden" name="runtime" value="{id}">
      <button type="submit">{label}</button>
    </form>
    <p class="quiet">{why}</p>"#,
        id = escape(one.id),
        label = label,
        why = why,
    )
}

fn start_button(one: &epoch_models::runtimes::Available) -> String {
    if one.serving {
        return String::new();
    }
    let Some(command) = one.start.as_deref() else {
        return String::new();
    };
    format!(
        r#"<form method="post" action="/start">
      <input type="hidden" name="runtime" value="{id}">
      <button type="submit">START {name}</button>
    </form>
    <p class="quiet">{command}</p>"#,
        id = escape(one.id),
        name = escape(one.name),
        command = escape(command),
    )
}

/// INSTALL, when there is something to install.
///
/// **Offered, never done.** The command runs in the machine's own terminal, where the licence,
/// the elevation prompt and the output belong to the person reading them — the same rule the
/// Host follows for agents. And nothing claims success: a terminal opened is all this can
/// honestly say, so the page says to come back and look.
///
/// A form rather than a link, because this program has no script and a POST is how a page with
/// none asks for something to happen.
fn install_button(one: &epoch_models::runtimes::Available) -> String {
    if one.installed {
        return String::new();
    }
    format!(
        r#"<form method="post" action="/install">
      <input type="hidden" name="runtime" value="{id}">
      <button type="submit">INSTALL {name}</button>
    </form>"#,
        id = escape(one.id),
        name = escape(one.name),
    )
}

/// One image studio's three facts, its fixes, and nothing invented.
///
/// ## Why it is beside the runtimes and not among them
///
/// A person opening this tab is asking one question — *what can this machine do for a World* —
/// and the answer has two halves. Drawing them in one list would put ComfyUI where a Brain
/// belongs; drawing them on two tabs would make somebody hunt. So: one page, two sections, and
/// a heading that says which is which.
///
/// **INSTALLED · NOT SERVING is the ordinary state**, and it is the one that matters most here.
/// The desktop application opens on a dashboard and its server does not start until an instance
/// is opened — measured, and the reason this says *open it* rather than *reinstall it*.
fn easel(one: &epoch_models::studio::Easel) -> String {
    let (word, tone) = match (one.installed, one.serving) {
        (_, true) => ("SERVING", "yes"),
        (true, false) => ("INSTALLED &#183; NOT SERVING", "unknown"),
        (false, false) => ("NOT INSTALLED", "no"),
    };
    let detail = match (one.installed, one.serving) {
        (_, true) => format!(
            "Answering at {}. {}",
            one.endpoint,
            if one.models.is_empty() {
                // The interesting case, and it reads as working until somebody asks for a
                // picture. A running ComfyUI with nothing to load is running perfectly.
                "It reports no checkpoint, so it is running and cannot yet make a picture."
                    .to_owned()
            } else {
                format!("Holding {}.", one.models.join(", "))
            }
        ),
        (true, false) => format!(
            "Found at {}. Its server starts when you open an instance, not when the application opens.",
            one.found_at.as_deref().unwrap_or("this machine")
        ),
        (false, false) => format!("Install it with: {}", one.install),
    };

    let start = match (one.serving, one.start.as_deref()) {
        (false, Some(command)) => format!(
            r#"<form method="post" action="/studio-start">
      <input type="hidden" name="studio" value="{id}">
      <button type="submit">OPEN {name}</button>
    </form>
    <p class="quiet">{command}</p>
    <p class="quiet">{first_run}</p>"#,
            id = escape(one.id),
            name = escape(one.name),
            command = escape(command),
            first_run = escape(one.first_run),
        ),
        // **Only what Epoch started, and only while it is answering.** A studio somebody
        // opened themselves is theirs; a STOP that appears next to something this program did
        // not launch is a button offering to kill a stranger's process.
        (true, _) => format!(
            r#"<form method="post" action="/studio-stop">
      <input type="hidden" name="studio" value="{id}">
      <button type="submit" class="danger">STOP {name}</button>
    </form>
    <p class="quiet">
      Measured on this kind of machine: an idle ComfyUI holds about 2.6 GB of system memory and
      <code>POST /free</code> returns none of it — what stays is the interpreter, torch's CUDA
      context and the node modules, and none of that goes while the process lives.
    </p>"#,
            id = escape(one.id),
            name = escape(one.name),
        ),
        _ => String::new(),
    };

    let install = if one.installed {
        String::new()
    } else {
        format!(
            r#"<form method="post" action="/studio-install">
      <input type="hidden" name="studio" value="{id}">
      <button type="submit">INSTALL {name}</button>
    </form>"#,
            id = escape(one.id),
            name = escape(one.name),
        )
    };

    format!(
        r#"<p class="verdict">
      <b class="runs runs--{tone}">{word}</b>
      {name}
    </p>
    <p class="quiet">{detail}</p>"#,
        name = escape(one.name),
        detail = escape(&detail),
    ) + &start
        + &install
}

/// The Settings tab: what this machine has, measured.
///
/// ## Why the Hugging Face CLI gets a tab rather than a line
///
/// It is the difference between a machine that can hand a GGUF to llama.cpp or LM Studio and
/// one that can only feed Ollama — and it is a property of *this* computer, which is exactly
/// the kind of fact a Host cannot answer on its behalf. Epoch's own Machines deck asks every
/// machine this question separately for the same reason.
///
/// ## Three facts, kept apart
///
/// **installed** · **signed in** · **where it is**. They have different fixes — install it, sign
/// in, or nothing at all — and a surface that collapsed any two would send somebody to do the
/// wrong one. The same discipline the agents' readiness follows in Epoch.
///
/// ## Cold instruments
///
/// Nothing here is invented. A machine without `hf` keeps the panel, loses its light, and reads
/// NOT INSTALLED with the one command that fixes it. A gauge nobody can explain is worse than
/// no gauge.
pub fn settings(
    hf: &epoch_models::hf::Cli,
    models_dir: &str,
    keys: &[(&str, &str, bool)],
    said: Option<&str>,
) -> String {
    let (word, tone) = match (hf.installed, hf.user.is_some()) {
        (false, _) => ("NOT INSTALLED", "no"),
        // Installed and signed out is a real, working state: it downloads anything ungated, and
        // only a gated repository needs the second half. Said plainly rather than as a fault.
        (true, false) => ("SIGNED OUT", "unknown"),
        (true, true) => ("READY", "yes"),
    };

    format!(
        r#"  {notice}
  <section class="card">
    <span class="label">Hugging Face CLI</span>
    <p class="verdict">
      <b class="runs runs--{tone}">{word}</b>
      {version}
    </p>
    <p class="quiet">{said}</p>
    {found}
  </section>

  <section class="card">
    <span class="label">Catalogue keys</span>
    <p class="quiet">
      Some sites hand a file over only to an account. Searching never needs one, so the shelf
      works before you sign in anywhere.
    </p>
    <p class="quiet">
      Kept by this operating system's own store &#8212; DPAPI on Windows, the Keychain on macOS,
      the Secret Service on Linux. Never in a file this program writes, and never shown back.
    </p>
    {keys}
  </section>

  <section class="card">
    <span class="label">Where a saved model goes</span>
    <p class="quiet">{models_dir}</p>
    <p class="quiet">
      SAVE THE FILE in the Workshop writes here. The path is chosen by this program and never
      by a request &#8212; a remote naming it would be a remote choosing where this machine
      writes.
    </p>
  </section>

  <section class="card">
    <span class="label">What this tab is not</span>
    <p class="quiet">
      Ollama needs none of this. It fetches its own models and files them where it will find
      them, and that is what is lent to the paired Host. The Hugging Face CLI is the other
      destination: a GGUF on disk, for llama.cpp or LM Studio, which have no endpoint that can
      be told to fetch one.
    </p>
  </section>"#,
        version = hf
            .version
            .as_deref()
            .map(|v| format!("hf {}", escape(v)))
            .unwrap_or_else(|| "not on this machine".to_owned()),
        said = escape(&match (hf.installed, hf.user.as_deref()) {
            (false, _) => "Install it with: curl -LsSf https://hf.co/cli/install.sh | bash -s"
                .to_owned(),
            (true, None) => "Signed out. It still downloads anything ungated; sign in with `hf auth login` for a gated repository."
                .to_owned(),
            (true, Some(who)) => format!("Signed in as {who}."),
        }),
        // Where it was found, so "not installed" is a fact somebody can go and check &#8212; and
        // the reason the PATH alone was not enough: its installer writes to `~/.local/bin`,
        // which is not on the PATH of an application started from a desktop.
        found = hf
            .found_at
            .as_deref()
            .map(|at| format!(r#"<p class="quiet">{}</p>"#, escape(at)))
            .unwrap_or_default(),
        models_dir = escape(models_dir),
        keys = keys
            .iter()
            .map(|(id, name, held)| format!(
                r#"<form method="post" action="/settings/key" class="row">
      <input type="hidden" name="source" value="{id}">
      <span>{name}</span>
      {field}
    </form>
    <p class="quiet">{caution}</p>"#,
                id = escape(id),
                name = escape(name),
                // Write-only: there is no field a stored value could come back through, which is
                // a guarantee the shape makes rather than one somebody has to remember.
                field = if *held {
                    r#"<span class="quiet">a key is stored</span>
      <button type="submit" name="forget" value="yes">FORGET</button>"#.to_owned()
                } else {
                    r#"<input type="password" name="value" placeholder="paste your key" autocomplete="off">
      <button type="submit">KEEP IT</button>"#.to_owned()
                },
                caution = "A key like this is an account token, not a download-scoped one.                            EpochServices uses it only to fetch a file. Treat it like a password.",
            ))
            .collect::<String>(),
        notice = said
            .map(|text| format!(r#"<p class="notice">{}</p>"#, escape(text)))
            .unwrap_or_default(),
    )
}

/// Every program on this machine that can run a model, and the one thing each needs next.
///
/// ## Why it is not in Settings
///
/// It was, and it was the largest thing on a page otherwise about the Hugging Face CLI and a
/// directory. *Start the thing that runs models* is not a setting — it is the second thing
/// somebody does on a machine they are lending, right after connecting it, and burying it under
/// a tab named for configuration made it something to find rather than something to press.
///
/// ## Three facts, three fixes
///
/// **Installed** · **serving** · **neither**. Install it, start it, or nothing at all. They stay
/// apart because they have different answers, and telling somebody to install what they already
/// have is exactly the failure keeping them apart prevents.
///
/// Starting is **offered, never done** on this machine's behalf: the command opens in the
/// machine's own terminal, where its output and any prompt belong to the person reading them.
/// Nothing here claims success either — a terminal opened is all it can honestly say.
/// What a runtime says it will run a model **on**, in its own words, and what that costs.
///
/// Only llama.cpp answers this (`--list-devices`), and the answer turned out to matter more than
/// anything else on the deck: the same GGUF took 36.7 s through a Vulkan build and 11.3 s
/// through a CUDA one, on the same card. Somebody who installed llama.cpp from the button above
/// is getting a third of their card with nothing on screen to say so.
///
/// `handicap` is `None` for a vendor Epoch cannot name a native path for, and silence is the
/// honest answer there — a menu of backends would eventually recommend hardware that cannot run
/// one. Empty `devices` means the question was never answerable, not that there is no card.
fn measured_devices(one: &epoch_models::runtimes::Available) -> String {
    let mut said = String::new();
    if !one.devices.is_empty() {
        // **Escape each part, then join with the separator.** Escaping the joined string turns
        // `&middot;` into `&amp;middot;`, which a browser draws as the literal text — measured on
        // the MacBook, where the device line read `Accelerate (…) &middot; MTL0: Apple M2`.
        said.push_str(&format!(
            r#"<p class="quiet">{}</p>"#,
            joined(one.devices.iter().map(|it| escape(it)))
        ));
    }
    if let Some(handicap) = one.handicap.as_deref() {
        said.push_str(&format!(r#"<p class="notice">{}</p>"#, escape(handicap)));
    }
    said
}

/// Which GPU backends this runtime has here, read from what is installed.
///
/// **A reading, not yet a control.** Epoch's own deck lets the user pick one; this machine has
/// nowhere to write that down — `keep.rs` holds a pairing and nothing else — so the honest thing
/// is to say what is there and to say plainly that nothing is being chosen. A dropdown that
/// forgot its answer on the next request would be worse than a sentence.
///
/// Worth having even so, and the MacBook is why: `Ollama.app` there carries `mlx_metal_v3` and
/// `mlx_metal_v4`, and *does this Mac use the GPU?* had no answer anywhere on this page.
///
/// `Unknown` prints nothing. It means nobody could read it — never "there is only CPU".
/// Parts that are already escaped, joined with the separator this page uses.
///
/// Its own function because the order matters and getting it wrong is invisible in the source:
/// escaping after joining is what put `&amp;middot;` on the page.
fn joined(parts: impl Iterator<Item = String>) -> String {
    parts.collect::<Vec<_>>().join(" &middot; ")
}

fn which_graphics(found: &epoch_models::engines::Engines) -> String {
    match found {
        epoch_models::engines::Engines::Unknown => String::new(),
        epoch_models::engines::Engines::Fixed(named) => {
            format!(r#"<p class="quiet">this build is {}</p>"#, escape(named))
        }
        epoch_models::engines::Engines::Choice(found) => {
            let names: Vec<String> = found
                .iter()
                .map(|it| {
                    if it.chosen {
                        format!("{} (in use)", it.name)
                    } else {
                        it.name.clone()
                    }
                })
                .collect();
            format!(
                r#"<p class="quiet">graphics installed here: {} &mdash; chosen on the machine itself</p>"#,
                joined(names.iter().map(|it| escape(it)))
            )
        }
    }
}

/// One runtime, as the Services page draws it.
///
/// Its own function so the two readings on it have a test: what a server **offers** and what
/// is actually **on the card** are different questions, and printing the first as the second
/// is a defect this row already carried once.
fn services_row(
    one: &epoch_models::runtimes::Available,
    graphics: &epoch_models::engines::Engines,
) -> String {
    let (word, tone) = match (one.installed, one.serving) {
        (_, true) => ("SERVING", "yes"),
        // Installed and not answering is an ordinary state: its server is started from
        // the application or a terminal. Saying OFFLINE would send somebody to reinstall
        // something they already have.
        (true, false) => ("INSTALLED &#183; NOT SERVING", "unknown"),
        (false, false) => ("NOT INSTALLED", "no"),
    };
    format!(
            r#"<p class="verdict">
  <b class="runs runs--{tone}">{word}</b>
  {name}
</p>
<p class="quiet">{detail}</p>"#,
            name = escape(one.name),
            detail = escape(&match (one.installed, one.serving) {
                // **Two readings, because they are two facts.**
                //
                // `models` is what the server *offers* — a shelf. It was printed here as
                // "Holding …", so a llama.cpp router listing five files on disk reported
                // five models in memory, to somebody watching a 7.5 GB process in Task
                // Manager. `resident` is the card, measured from each server's own word for
                // it.
                //
                // Both are said out loud even when empty. A gauge that disappears when it
                // reads zero is one nobody can trust when it reads something — and this is
                // the same fix Epoch's own deck already carries, arriving late because the
                // shared crate was only shared in one direction.
                (_, true) => format!(
                    "Answering at {}. The Host can use it as an OpenAI-compatible Service.                          Offers {}. In memory: {}.",
                    one.endpoint,
                    if one.models.is_empty() {
                        "nothing on its shelf".to_owned()
                    } else {
                        one.models.join(", ")
                    },
                    if one.resident.is_empty() {
                        "nothing".to_owned()
                    } else {
                        one.resident.join(", ")
                    }
                ),
                (true, false) => format!(
                    "Found at {}. Start its server and it becomes usable.",
                    one.found_at.as_deref().unwrap_or("this machine")
                ),
                (false, false) => format!("Install it with: {}", one.install),
            }),
        ) + &measured_devices(one) + &which_graphics(graphics)
            + &shelf_button(one)
            + &start_button(one)
            + &install_button(one)
}

pub fn services(said: Option<&str>) -> String {
    // Measured when the tab opens, like everything else here. A TCP probe with its own short
    // deadline, so a machine with nothing listening costs milliseconds rather than the twenty
    // seconds a filtered port can take to give up.
    let runtimes = epoch_models::runtimes::survey();
    // **Read here, not inside the row.** `services_row` and `measured_devices` are pure
    // functions of what they are given, which is what makes them testable at all — a row that
    // reached for the filesystem itself answered differently on the machine running the test.
    let runtimes: String = runtimes
        .iter()
        .map(|one| {
            let graphics = epoch_models::runtimes::Runtime::ALL
                .into_iter()
                .find(|it| it.id() == one.id)
                .map_or(epoch_models::engines::Engines::Unknown, |runtime| {
                    epoch_models::engines::engines(runtime)
                });
            services_row(one, &graphics)
        })
        .collect();

    // Measured on the same open, for the same reason: somebody can install ComfyUI or open an
    // instance while this window is up.
    let easels: String = epoch_models::studio::survey().iter().map(easel).collect();

    format!(
        r#"  {notice}
  <section class="card">
    <span class="label">Services on this machine</span>
    {runtimes}
  </section>

  <section class="card">
    <span class="label">What the Host can use</span>
    <p class="quiet">
      All three speak an API Epoch already speaks, so none of them needs anything added to it
      &#8212; only to be running. A turn arriving from the Host names which one should answer
      it, and this program resolves that to one of its own loopback ports. It never carries an
      address.
    </p>
    <p class="quiet">
      llama.cpp and LM Studio take a GGUF from disk; Ollama fetches its own and keeps it under a
      name with no extension, which is why neither of the other two could see it. The button
      beside each of them puts every model this machine has &#8212; Ollama's included, and
      whatever SAVE THE FILE saved &#8212; where that runtime looks, linked rather than copied,
      so nothing is downloaded twice and Ollama keeps its own.
    </p>
  </section>

  <section class="card">
    <span class="label">Making pictures on this machine</span>
    {easels}
    <p class="quiet">
      A World asks for a picture by naming a style &#8212; <em>pixel art</em>, <em>realistic</em>
      &#8212; and never a workflow or a file. This is the program that answers. It is not a
      Brain and never appears as one: a character does not think in pictures, it asks for one.
    </p>
  </section>"#,
        runtimes = runtimes,
        easels = easels,
        notice = said
            .map(|text| format!(r#"<p class="notice">{}</p>"#, escape(text)))
            .unwrap_or_default(),
    )
}

/// Before anybody has paired: two ways round, and the second one is not a fallback.
///
/// The ordinary way is this machine dialling the Host, so nobody types an IP. It needs this
/// machine to be able to open a connection *to* the Host — and on a real network that was not
/// true: with this machine on Wi-Fi and the Host on Ethernet, every connection started here was
/// dropped while every connection the Host started succeeded. That is client isolation, and it
/// belongs to a router somebody may not own.
///
/// So the same exchange is offered backwards, and it is offered **plainly** rather than hidden
/// behind a failure. Somebody who already knows their network isolates its clients should not
/// have to watch a timeout first to be told about the other door.
fn waiting(showing: Option<(&str, u64)>) -> String {
    // **While a code is up, it is the only thing on screen.**
    //
    // Two reasons, and they are the same reason. A countdown has to reload every second to be a
    // countdown, and a reload empties every field on the page — so a form beside a ticking code
    // is a form nobody can finish typing into. And having chosen a direction, the other one is
    // not a choice any more; showing it would be offering to start over as though nothing had
    // happened.
    if let Some((code, left)) = showing {
        return format!(
            r#"<section class="card lit">
    <span class="label">Waiting for the Host</span>
    <p class="showing">{code}<b class="left">{clock}</b></p>
    <p class="quiet">
      In Epoch on the Host: <b>Machines</b>, then type this code and this machine's address,
      shown below. It is spent the moment it works.
    </p>
    <form method="post" action="/uninvite">
      <button type="submit" class="quietbtn">STOP SHOWING</button>
    </form>
  </section>"#,
            code = escape(code),
            clock = format_args!("{}:{:02}", left / 60, left % 60),
        );
    }

    // **One door, by the owner's decision (2026-08-20).**
    //
    // Both were offered, and offering both was defensible: they are the same exchange with the
    // socket opened from opposite ends, and each works on networks the other does not.
    //
    // What settled it is that on this network only one of them has ever worked. The MacBook on
    // Wi-Fi and the Host on Ethernet could not open a connection in that direction at all —
    // every port refused, with the firewall rule present and a listener confirmed up — while
    // every connection the Host started succeeded. A second door somebody has to read about,
    // choose between, and then watch time out is worse than one door that works.
    //
    // The route and `door::redeem` are kept: they are the other half of a documented protocol
    // (ADR-0029), tested, and a network where this one is the only one that works is a real
    // network. What is removed is asking a person to decide between them.
    r#"<section class="card">
    <span class="label">Let the Host reach out</span>
    <p class="quiet">
      Show a code here and type it into Epoch, under <b>Machines</b>, with this machine's
      address below. It is spent the moment it works.
    </p>
    <form method="post" action="/invite">
      <button type="submit">SHOW A CODE</button>
    </form>
  </section>"#
        .to_owned()
}

/// After: what it is lending, and how to stop.
fn paired(bond: &Bond, have: &Have) -> String {
    format!(
        r#"<section class="card lit">
    <span class="label">Connected</span>
    <p>Lending {count} {word} to the Host at {host}.</p>
    <p class="quiet">
      Turns arrive from that Host and are answered here. Nothing about its Worlds, its
      conversations or its decisions is kept on this machine.
    </p>
    <form method="post" action="/unpair">
      <button type="submit" class="quietbtn">DISCONNECT</button>
    </form>
  </section>"#,
        count = have.models.len(),
        word = if have.models.len() == 1 {
            "model"
        } else {
            "models"
        },
        host = escape(&bond.host),
    )
}

/// What the hardware is, or honestly that it could not be asked.
fn hardware(have: &Have) -> String {
    let mut parts = Vec::new();
    match (&have.gpu, have.vram_total) {
        // The lent machine's own architecture, carried on the wire. A Mac reports one pool the
        // GPU reads directly, and printing "video memory" over it would describe a machine three
        // metres away as if it were this one.
        (Some(gpu), Some(total)) => parts.push(format!(
            "{gpu} · {} {}",
            gb(total),
            if have.unified {
                "unified memory"
            } else {
                "video memory"
            }
        )),
        (Some(gpu), None) => parts.push(gpu.clone()),
        // No card this program can ask about. Splitting system memory into a made-up video share
        // would invent a division the machine does not have.
        (None, _) => {}
    }
    // Not repeated on a unified machine: the total above is the same pool.
    if let Some(ram) = have.ram_total {
        if !(have.unified && have.vram_total.is_some()) {
            parts.push(format!("{} memory", gb(ram)));
        }
    }
    if parts.is_empty() {
        return "Hardware could not be read on this machine.".to_owned();
    }
    parts.join(" · ")
}

fn gb(bytes: u64) -> String {
    format!("{:.0} GB", bytes as f64 / 1_000_000_000.0)
}

/// Everything shown is escaped. A model name and a machine name both come from outside this
/// program, and a page that pasted them in raw would be a page somebody could write into.
pub fn escape(raw: &str) -> String {
    raw.chars()
        .map(|c| match c {
            '&' => "&amp;".to_owned(),
            '<' => "&lt;".to_owned(),
            '>' => "&gt;".to_owned(),
            '"' => "&quot;".to_owned(),
            '\'' => "&#39;".to_owned(),
            other => other.to_string(),
        })
        .collect()
}

/// Epoch's language, in the smallest form that still reads as Epoch: the dark ground, the gold,
/// the monospace, and light instead of colour to carry weight.
const STYLE: &str = r#"
:root{--void:#07060c;--panel:#171320;--sunk:rgba(6,5,11,.6);--edge:#322a44;
--gold:#e8b74a;--gold-lit:#ffd77a;--parchment:#f4e6c4;--ink:#efe8da;--quiet:rgba(239,232,218,.45);
--mono:"IBM Plex Mono",ui-monospace,SFMono-Regular,Menlo,Consolas,monospace}
*{box-sizing:border-box}
body{margin:0;background:var(--void);color:var(--ink);font:400 13px/1.6 var(--mono);
display:flex;justify-content:center;padding:36px 18px}
main{width:100%;max-width:560px;display:grid;gap:16px}
/*
  The Workshop needs the width and the Connect tab does not.

  Connect is four fields and a paragraph; 560 is a comfortable measure for reading and a silly
  one for a form. The Workshop is a list of repository names, and those are long — a narrow
  column meant the names had nowhere to go, so they overlapped each other and the panel.
*/
body.workshop main{max-width:1100px}
header{display:flex;gap:12px;align-items:center}
.mark{width:26px;height:26px;background:linear-gradient(180deg,var(--gold-lit),var(--gold));
box-shadow:0 0 0 2px var(--void),0 0 0 3px rgba(232,183,74,.3)}
h1{margin:0;font:400 15px var(--mono);letter-spacing:2px;color:var(--parchment)}
p{margin:0 0 6px}
.grid{display:grid;grid-template-columns:repeat(auto-fill,minmax(290px,1fr));gap:10px;margin-top:10px}
.row{display:flex;flex-wrap:wrap;align-items:center;gap:8px}
.row.between{justify-content:space-between}
.row > input[name=query]{flex:1;min-width:180px}
.pages{display:flex;align-items:center;justify-content:center;gap:12px;margin-top:14px}
a.btn{display:inline-flex;align-items:center;padding:10px 13px;text-decoration:none;
  color:var(--parchment);background:linear-gradient(180deg,#221b31,#15111f)}
a.btn:hover{box-shadow:0 0 0 2px #05040a,0 0 0 3px var(--gold)}
.warn{color:var(--gold-lit)}
.note{color:var(--parchment)}
.quiet{color:var(--quiet)}
/* The picture a catalogue leads with. Fixed height and cover, because twenty results are
   twenty aspect ratios and a grid that reflows around each one moves while it is read. */
.shot{display:block;width:100%;height:132px;object-fit:cover;margin:0 0 8px;
  border:1px solid rgba(255,255,255,.12);background:rgba(0,0,0,.35)}
.card{background:linear-gradient(180deg,var(--panel),#100d17);padding:16px 18px;display:grid;gap:7px;
box-shadow:0 0 0 2px #05040a,0 0 0 3px rgba(232,183,74,.22)}
.card.lit{box-shadow:0 0 0 2px #05040a,0 0 0 3px rgba(232,183,74,.6)}
.label{font:500 9px var(--mono);letter-spacing:1.6px;text-transform:uppercase;color:var(--quiet)}
form{display:grid;gap:8px;margin-top:4px}
input{background:var(--sunk);border:0;box-shadow:0 0 0 1px var(--edge);color:var(--ink);
padding:10px 11px;font:400 13px var(--mono)}
input:focus{outline:0;box-shadow:0 0 0 1px var(--gold)}
input[name=code]{letter-spacing:6px;text-transform:uppercase;font-size:18px;color:var(--gold-lit)}
button{padding:10px 13px;border:0;cursor:pointer;color:var(--parchment);background:linear-gradient(180deg,#221b31,#15111f);
font:400 11px var(--mono);letter-spacing:1.5px;
box-shadow:0 0 0 2px #05040a,0 0 0 3px rgba(232,183,74,.45)}
button:hover{box-shadow:0 0 0 2px #05040a,0 0 0 3px var(--gold)}
.quietbtn{box-shadow:0 0 0 2px #05040a,0 0 0 3px rgba(70,58,92,.7)}
.notice{background:var(--sunk);border-left:2px solid var(--gold);padding:9px 11px;color:var(--parchment)}
code{color:var(--parchment)}
.tabs{display:flex;gap:2px}
.tabs a{padding:8px 13px;text-decoration:none;color:var(--quiet);font:400 10px var(--mono);
letter-spacing:1.6px;background:linear-gradient(180deg,#15111f,#100d17);
box-shadow:0 0 0 1px #05040a}
.tabs a.on{color:var(--parchment);box-shadow:0 0 0 1px #05040a,inset 0 -2px 0 var(--gold)}
.tabs a:hover{color:var(--ink)}
.verdict{display:flex;align-items:center;gap:8px;flex-wrap:wrap;margin-top:4px}
.runs{padding:1px 5px;font-size:9px;letter-spacing:1.2px;box-shadow:0 0 0 1px currentColor}
.runs--yes{color:#7fd08a}
.runs--no{color:var(--gold)}
.runs--unknown{color:var(--parchment);opacity:.55}
.showing{font-size:26px;letter-spacing:9px;color:var(--gold-lit);margin:6px 0 2px}
.showing .left{font-size:12px;letter-spacing:1px;opacity:.55;margin-left:12px;vertical-align:middle}
.models{margin:2px 0 0;padding-left:16px}
.models li{margin:2px 0}
/*
  Model names across, not down.

  A single column turned forty results into forty screens of scrolling, in a window that is 640
  wide and has two other panels under it. `auto-fill` rather than a fixed count so the same page
  works in a narrow window and in a wide one — the desktop Workshop reads the same way.
*/
.offers{list-style:none;margin:6px 0 0;padding:0;
display:grid;grid-template-columns:repeat(auto-fill,minmax(260px,1fr));gap:3px 14px}
/*
  **`min-width:0` on everything that has to shrink**, and that is the whole fix.

  A grid item and a flex item both refuse to go narrower than their content unless told
  otherwise, so `text-overflow:ellipsis` never fired: the names ran out of their cells and drew
  on top of each other. It has to be said on the `li`, on the `form` between them, and on the
  button itself — a single one of the three left out and the chain still cannot shrink.
*/
.offers li{display:flex;align-items:baseline;gap:6px;min-width:0}
.offers form{display:flex;margin:0;min-width:0;flex:1}
/* A name is a link, because pressing it asks a question rather than performing an action. */
.offers button.link{background:none;box-shadow:none;padding:0;text-align:left;
color:var(--parchment);font:400 11px var(--mono);letter-spacing:0;
min-width:0;width:100%;
overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.offers button.link:hover{color:var(--gold-lit);box-shadow:none;text-decoration:underline}
.offers em{font-style:normal;font-size:9px;white-space:nowrap;flex:none}
footer{font-size:11px;line-height:1.7}
/*
  The facet chips, the pager, and a repository opened into its quantisations.

  All three exist on the desktop Workshop and none of them needed a line of script there either
  - a chip is a checkbox inside the search form, a page is a form with a number in it, and an
  open repository is a page the server drew that way. `script-src: 'none'` is not a limitation
  being worked around here; it is what makes a careful door trustworthy, and the two Workshops
  ended up the same shape regardless.
*/
.chips{display:flex;flex-wrap:wrap;gap:6px}
.chip{display:inline-flex;align-items:center;gap:5px;padding:5px 9px;cursor:pointer;
background:var(--sunk);box-shadow:0 0 0 1px var(--edge);
font:400 10px var(--mono);letter-spacing:1px;color:var(--quiet)}
.chip:hover{color:var(--parchment)}
.chip--on{color:var(--gold-lit);box-shadow:0 0 0 1px var(--gold)}
.chip input{width:11px;height:11px;padding:0;box-shadow:none;accent-color:var(--gold)}
/* An open result spans the whole grid: twenty-seven quantisations do not fit in a 260px cell. */
.offers li.open{grid-column:1/-1;display:block}
.offers li.open form{display:block}
.offers li.open button.link{color:var(--gold-lit);white-space:normal;overflow:visible}
/*
  The quantisation table, in the same measurements Epoch's Workshop uses.

  Copied rather than re-invented: two screens showing one repository had drifted into two
  layouts, and the one somebody compares against is the desktop. An `inset` shadow keeps a chip's
  box exactly its own size — an outer one spreads the row without adding anything to read.
*/
.quants{margin:6px 0 2px;padding:9px 10px;background:var(--sunk);
box-shadow:0 0 0 1px var(--edge);display:grid;gap:3px}
.qrow{display:grid;grid-template-columns:54px minmax(0,1fr);gap:8px;align-items:center}
.qbits{font:400 9px var(--mono);letter-spacing:1.2px;color:var(--gold);opacity:.75;text-align:right}
.qlist{display:flex;flex-wrap:wrap;gap:4px;min-width:0}
.qform{display:contents}
/*
  **Gold and weight, never a traffic light** - the same rule the verdict word follows. What runs
  here is ringed; what does not is dimmed rather than hidden, because it is still a real thing
  somebody may want.
*/
.quant{display:inline-flex;align-items:baseline;gap:5px;padding:2px 6px;border:0;cursor:pointer;
background:rgba(6,5,11,.5);color:var(--parchment);font:400 10px var(--mono);letter-spacing:0;
box-shadow:inset 0 0 0 1px rgba(120,105,150,.35)}
.quant:hover{box-shadow:inset 0 0 0 1px var(--gold)}
.quant b{font-weight:500}
.quant span{opacity:.55}
.quant--yes{box-shadow:inset 0 0 0 1px rgba(127,208,138,.55)}
.quant--no{opacity:.45}
.quant--unknown{box-shadow:inset 0 0 0 1px rgba(120,105,150,.35)}
.qtag{font-style:normal;color:var(--gold);opacity:.85}
.qparts{font-style:normal;opacity:.5}
.pager{display:flex;flex-wrap:wrap;gap:3px;align-items:center;margin-top:8px}
.pgf{display:inline;margin:0}
.pg{padding:4px 8px;font:400 10px var(--mono);background:none;box-shadow:0 0 0 1px var(--edge);
color:var(--quiet)}
.pg:hover{box-shadow:0 0 0 1px var(--gold);color:var(--parchment)}
.pg--on{color:var(--gold-lit);box-shadow:0 0 0 1px var(--gold)}
/* An end stop is not a button: it cannot do anything, so it does not look like it could. */
.pg--dead{display:inline-block;padding:4px 8px;font:400 10px var(--mono);
color:var(--quiet);opacity:.3;box-shadow:0 0 0 1px var(--edge)}
/* Two destinations side by side, because they are two answers to one question. */
.acts{display:flex;flex-wrap:wrap;gap:7px;margin-top:4px}
.acts form{margin:0}

/*
  What fits on this machine: twenty boxes, five across.

  A grid rather than a list because the question is a comparison — twenty rows read as a ranking
  and twenty boxes read as a shelf, and only one of those is what this is.
*/
.fits{display:grid;grid-template-columns:repeat(5,1fr);gap:8px;margin-top:12px}
.fits__box{position:relative;padding:10px 10px 12px;background:var(--sunk);
border:1px solid var(--edge);display:grid;gap:4px;align-content:start}
/* Already here is a fact about this machine, so it is marked rather than sorted away. */
.fits__box.on{border-color:var(--gold)}
.fits__rank{position:absolute;top:6px;right:8px;color:var(--quiet);font-size:11px}
.fits__name{margin:0;padding-right:18px;color:var(--parchment);word-break:break-word}
.fits p{margin:0;font-size:12px}
.fits button{margin-top:4px;width:100%}
@media (max-width:900px){.fits{grid-template-columns:repeat(2,1fr)}}

/* Models: one row per model, with its recipe folded underneath. */
.mdls{display:grid;gap:8px;margin-top:12px}
.mdls__row{padding:10px 12px;background:var(--sunk);border:1px solid var(--edge);
display:grid;grid-template-columns:1fr auto;gap:8px 12px;align-items:start}
.mdls__row p{margin:0}
.mdls__name{color:var(--parchment)}
.mdls__path{font-size:11px;word-break:break-all}
.mdls__doing{display:flex;gap:6px;align-items:start}
.mdls__recipe{grid-column:1 / -1;margin-top:4px;border-top:1px solid var(--edge);padding-top:8px}
.mdls__recipe summary{cursor:pointer;color:var(--quiet)}
.curve{width:100%;border-collapse:collapse;margin-top:8px;font-size:12px}
.curve th{text-align:left;color:var(--quiet);font-weight:400;padding:2px 8px 2px 0}
.curve td{padding:2px 8px 2px 0}
/* The row in use, marked by the same gold everything chosen is marked by. */
.curve tr.on td{color:var(--gold-lit)}

"#;

#[cfg(test)]
mod tests {
    /// The three states of the Hugging Face CLI, which have three different fixes.
    #[test]
    fn settings_keeps_installed_and_signed_in_as_separate_facts() {
        // Collapsing them would send somebody to install something that is already there.
        let ready = super::settings(
            &epoch_models::hf::Cli {
                installed: true,
                version: Some("1.28.0".into()),
                found_at: Some("/Users/someone/.local/bin/hf".into()),
                user: Some("KislokX".into()),
            },
            "/Users/someone/EpochServices/models",
            &[("civitai", "Civitai", false)],
            None,
        );
        assert!(ready.contains("READY"));
        assert!(ready.contains("Signed in as KislokX"));
        // Where it was found, so its absence elsewhere is checkable - and the reason the PATH
        // alone was not enough: the installer writes to `~/.local/bin`, which is not on the
        // PATH of an application started from a desktop.
        assert!(ready.contains(".local/bin/hf"));

        // Installed and signed out is a real, working state: it downloads anything ungated.
        let out = super::settings(
            &epoch_models::hf::Cli {
                installed: true,
                version: Some("1.28.0".into()),
                found_at: Some("/usr/local/bin/hf".into()),
                user: None,
            },
            "/models",
            &[("civitai", "Civitai", false)],
            None,
        );
        assert!(out.contains("SIGNED OUT"));
        assert!(out.contains("hf auth login"));
        assert!(!out.contains("install.sh"), "it is already installed");

        // A cold instrument: the panel keeps its frame, loses its light, and names the one
        // command that fixes it.
        let none = super::settings(
            &epoch_models::hf::Cli::default(),
            "/models",
            &[("civitai", "Civitai", false)],
            None,
        );
        assert!(none.contains("NOT INSTALLED"));
        assert!(none.contains("install.sh"));
        assert!(none.contains("not on this machine"));
    }

    #[test]
    fn an_install_is_offered_only_for_what_is_missing() {
        // Offered, never done — the command runs in this machine's own terminal, where the
        // licence and the elevation prompt belong to the person reading them. A button beside
        // something already installed would be a control with nothing to do.
        let absent = epoch_models::runtimes::Available {
            cached: Vec::new(),
            resident: Vec::new(),
            devices: Vec::new(),
            handicap: None,
            id: "llama_cpp",
            name: "llama.cpp",
            installed: false,
            found_at: None,
            serving: false,
            endpoint: "http://127.0.0.1:8080".into(),
            models: Vec::new(),
            install: "brew install llama.cpp",
            // Not here, so there is nothing to start — the two facts move together.
            start: None,
        };
        let offered = super::install_button(&absent);
        assert!(offered.contains(r#"action="/install""#), "{offered}");
        assert!(offered.contains(r#"value="llama_cpp""#), "{offered}");

        let here = epoch_models::runtimes::Available {
            cached: Vec::new(),
            installed: true,
            found_at: Some("/opt/homebrew/bin/llama".into()),
            start: Some("\"/opt/homebrew/bin/llama-server\"".into()),
            ..absent
        };
        assert!(super::install_button(&here).is_empty());

        // And START is offered instead: here, and not answering.
        let offered = super::start_button(&here);
        assert!(offered.contains(r#"action="/start""#), "{offered}");
        assert!(
            offered.contains("llama-server"),
            "the exact command: {offered}"
        );

        // Nothing beside something already serving — that button's job is done.
        let running = epoch_models::runtimes::Available {
            cached: Vec::new(),
            serving: true,
            ..here
        };
        assert!(super::start_button(&running).is_empty());
    }

    #[test]
    fn settings_carries_the_answer_to_what_was_just_pressed() {
        // A redirect means the page that reports an action is a different request from the one
        // that performed it, so the sentence has to travel.
        let page = super::settings(
            &epoch_models::hf::Cli::default(),
            "/models",
            &[("civitai", "Civitai", false)],
            Some("A terminal opened on `brew install llama.cpp`."),
        );
        assert!(page.contains("A terminal opened"));
    }

    /// An image studio, made up, so the three states can be read rather than reasoned about.
    fn easel_here(installed: bool, serving: bool, models: &[&str]) -> epoch_models::studio::Easel {
        epoch_models::studio::Easel {
            id: "comfyui",
            name: "ComfyUI",
            installed,
            found_at: installed.then(|| "/Applications/Comfy Desktop.app".to_owned()),
            serving,
            endpoint: "http://127.0.0.1:8188".to_owned(),
            models: models.iter().map(|m| (*m).to_owned()).collect(),
            install: "brew install --cask comfyui",
            start: installed.then(|| "\"/Applications/Comfy Desktop.app\"".to_owned()),
            // The made-up one opens the program's own front door rather than its server,
            // which is the state this fixture is about.
            starts_server: false,
            first_run: "It asks where to keep its models.",
        }
    }

    #[test]
    fn a_machine_without_it_is_told_how_to_get_it_and_offered_nothing_else() {
        let drawn = easel(&easel_here(false, false, &[]));
        assert!(drawn.contains("NOT INSTALLED"));
        assert!(drawn.contains("brew install --cask comfyui"));
        assert!(drawn.contains("/studio-install"));
        // Nothing to open, so no button that could not work.
        assert!(!drawn.contains("/studio-start"));
    }

    #[test]
    fn installed_and_quiet_says_open_an_instance_rather_than_reinstall() {
        // The ordinary state, and the one that reads as broken if the sentence is wrong: the
        // desktop application opens on a dashboard and its server starts with an instance.
        // Measured 2026-08-22, and the reason this is not called OFFLINE.
        let drawn = easel(&easel_here(true, false, &[]));
        assert!(drawn.contains("INSTALLED"));
        assert!(drawn.contains("NOT SERVING"));
        assert!(drawn.contains("/studio-start"));
        assert!(!drawn.contains("/studio-install"));
        assert!(drawn.contains("open an instance"));
    }

    #[test]
    fn serving_with_no_checkpoint_is_running_and_cannot_draw() {
        // The interesting case. A fresh ComfyUI answers perfectly and can make nothing, and a
        // row that only said SERVING would send somebody to debug Epoch.
        let drawn = easel(&easel_here(true, true, &[]));
        assert!(drawn.contains("SERVING"));
        assert!(drawn.contains("cannot yet make a picture"));
        // Serving: neither button, because neither would do anything.
        assert!(!drawn.contains("/studio-start"));
        assert!(!drawn.contains("/studio-install"));
    }

    #[test]
    fn serving_with_checkpoints_names_them() {
        let drawn = easel(&easel_here(true, true, &["sdxl.safetensors"]));
        assert!(drawn.contains("Holding sdxl.safetensors"));
        assert!(!drawn.contains("cannot yet make a picture"));
    }

    #[test]
    fn only_an_id_this_build_knows_opens_either_door() {
        // The same discipline `runner_at` follows: what arrives from a form is looked up in a
        // closed set, never dialled. A route that took an address would be a route that could
        // point this machine's terminal at something else.
        assert!(crate::studio_named(Some(&"comfyui".to_owned())).is_ok());
        assert!(crate::studio_named(Some(&"http://evil.example".to_owned())).is_err());
        assert!(crate::studio_named(Some(&String::new())).is_err());
        assert!(crate::studio_named(None).is_err());
    }

    /// One press, on the machine that most needs it.
    ///
    /// Measured on the paired MacBook, 2026-08-21: Ollama answering with 8 models and llama.cpp
    /// answering with none, on one disk. A machine that lends its card is a machine somebody
    /// keeps models on, and until this existed only one runtime there could see them.
    fn runtime_here(installed: bool, serving: bool) -> epoch_models::runtimes::Available {
        epoch_models::runtimes::Available {
            cached: Vec::new(),
            id: "llama_cpp",
            name: "llama.cpp",
            installed,
            found_at: installed.then(|| r"C:\Users\me\.llama\bin".to_owned()),
            serving,
            endpoint: "http://127.0.0.1:8080".to_owned(),
            models: Vec::new(),
            resident: Vec::new(),
            devices: Vec::new(),
            handicap: None,
            install: "winget install llama.cpp",
            start: None,
        }
    }

    #[test]
    fn a_lent_machine_offers_the_same_shelf_the_host_does() {
        let here = |id: &'static str, installed: bool| epoch_models::runtimes::Available {
            cached: Vec::new(),
            resident: Vec::new(),
            devices: Vec::new(),
            handicap: None,
            id,
            name: "whatever",
            installed,
            found_at: None,
            serving: false,
            endpoint: String::new(),
            models: Vec::new(),
            install: "",
            start: None,
        };

        assert!(super::shelf_button(&here("llama_cpp", true)).contains("START WITH EVERYTHING"));
        assert!(super::shelf_button(&here("lm_studio", true)).contains("LEND IT EVERYTHING"));

        // Ollama imports rather than reading a directory — three and a half minutes of hashing
        // to make a second name for a model it already has. Not what this button means.
        assert!(super::shelf_button(&here("ollama", true)).is_empty());

        // A shelf for a program that is not here would be a button whose only outcome is an
        // explanation.
        assert!(super::shelf_button(&here("llama_cpp", false)).is_empty());
    }

    /// **Two readings, and both said out loud.** A shelf of five files reported as five models
    /// in memory is the defect this row already lost once, on the other surface.
    #[test]
    fn a_serving_runtime_says_what_it_offers_and_what_is_on_the_card() {
        let mut one = runtime_here(true, true);
        one.models = vec!["gemma4-12b".into(), "qwen3-14b".into()];
        one.resident = vec!["gemma4-12b".into()];
        let said = super::services_row(&one, &epoch_models::engines::Engines::Unknown);
        assert!(said.contains("Offers gemma4-12b, qwen3-14b"), "{said}");
        assert!(said.contains("In memory: gemma4-12b"), "{said}");

        // Nothing loaded reads `nothing`, out loud. A gauge that vanishes at zero is one nobody
        // can trust when it reads something.
        one.resident.clear();
        let empty = super::services_row(&one, &epoch_models::engines::Engines::Unknown);
        assert!(empty.contains("In memory: nothing"), "{empty}");
    }

    /// A vendor Epoch cannot name a native path for gets **silence**, never a menu. A list of
    /// backends offers every card every option, so it eventually recommends hardware that
    /// cannot run one.
    #[test]
    fn the_separator_is_not_escaped_into_visible_junk() {
        // Measured on the MacBook: the device line read `Accelerate (…) &middot; MTL0: Apple M2`
        // because the whole joined string was escaped, entity and all. Each part is escaped and
        // then joined, and the order is invisible in the source — hence a test on it.
        let said = super::which_graphics(&epoch_models::engines::Engines::Choice(vec![
            epoch_models::engines::Engine {
                id: "mlx_metal_v3".into(),
                name: "Metal 3".into(),
                chosen: false,
            },
            epoch_models::engines::Engine {
                id: "mlx_metal_v4".into(),
                name: "Metal 4".into(),
                chosen: true,
            },
        ]));
        assert!(said.contains("Metal 3 &middot; Metal 4"), "{said}");
        assert!(!said.contains("&amp;middot;"), "{said}");
        // The one in use says so; a list where every row reads alike cannot be used to pick.
        assert!(said.contains("Metal 4 (in use)"), "{said}");
    }

    #[test]
    fn nothing_read_says_nothing_rather_than_cpu_only() {
        // `Unknown` is *nobody could read it*. On this Mac homebrew's llama.cpp ships no separate
        // backend library at all, so there is genuinely nothing to say — and saying "CPU" would
        // be an invented reading of a machine with an M2 in it.
        assert_eq!(
            super::which_graphics(&epoch_models::engines::Engines::Unknown),
            ""
        );
        assert!(
            super::which_graphics(&epoch_models::engines::Engines::Fixed("CUDA".into()))
                .contains("this build is CUDA")
        );
    }

    #[test]
    fn a_measured_device_is_named_and_an_unnameable_one_is_not() {
        let mut one = runtime_here(true, true);
        one.devices = vec!["Vulkan0: NVIDIA GeForce RTX 4070 SUPER".into()];
        one.handicap = Some("This build has no CUDA backend.".into());
        let said = super::measured_devices(&one);
        assert!(said.contains("RTX 4070 SUPER"), "{said}");
        assert!(said.contains("no CUDA backend"), "{said}");

        assert_eq!(super::measured_devices(&runtime_here(true, true)), "");
    }

    /// **Only what is answering.** A STOP beside something this program did not launch is a
    /// button offering to kill a stranger's process.
    #[test]
    fn a_studio_can_be_stopped_only_while_it_is_serving() {
        let serving = super::easel(&easel_here(true, true, &["sd_xl_base_1.0.safetensors"]));
        assert!(serving.contains("/studio-stop"), "{serving}");
        assert!(serving.contains("STOP ComfyUI"), "{serving}");

        let idle = super::easel(&easel_here(true, false, &[]));
        assert!(!idle.contains("/studio-stop"), "{idle}");
        assert!(idle.contains("OPEN ComfyUI"), "{idle}");
    }

    /// **It never ran either.** The `#[test]` that belonged to this was sitting eighty lines
    /// above, stacked on a different test where it did nothing. Same defect as three others
    /// found in this pass: an attribute or a doc comment that survived an edit its code did not.
    #[test]
    fn services_is_its_own_page_and_settings_is_not_about_runtimes() {
        // It lived inside Settings, where it was the largest thing on a page otherwise about
        // `hf` and a directory (owner, 2026-08-20). *Start the thing that runs models* is not a
        // setting — it is the second thing somebody does on a machine they are lending.
        let page = super::services(None);
        assert!(page.contains("Services on this machine"));
        for one in ["Ollama", "llama.cpp", "LM Studio"] {
            assert!(page.contains(one), "{one} is missing from START SERVICES");
        }

        let settings = super::settings(
            &epoch_models::hf::Cli::default(),
            "/models",
            &[("civitai", "Civitai", false)],
            None,
        );
        assert!(!settings.contains("Services on this machine"));
        assert!(
            settings.contains("Hugging Face CLI"),
            "settings keeps its own question"
        );
    }

    #[test]
    fn the_services_page_carries_the_answer_to_what_was_just_pressed() {
        // A terminal opened is all it can honestly claim, and the claim belongs to the page
        // that offered the button.
        let page = super::services(Some("A terminal opened on ollama serve."));
        assert!(page.contains("A terminal opened on ollama serve."));
    }

    #[test]
    fn settings_says_where_a_saved_model_lands() {
        // The path is chosen by this program and never by a request - a remote naming it would
        // be a remote choosing where this machine writes.
        let page = super::settings(
            &epoch_models::hf::Cli::default(),
            "/Users/someone/EpochServices/models",
            &[("civitai", "Civitai", false)],
            None,
        );
        assert!(page.contains("/Users/someone/EpochServices/models"));
    }

    #[test]
    fn every_tab_is_reachable_from_every_tab() {
        // A tab that only appears once you are on it is a page nobody finds twice.
        for tab in [Tab::Connect, Tab::Workshop, Tab::Creations, Tab::Settings] {
            let page = frame(tab, "", None);
            assert!(page.contains(r#"href="/""#), "{tab:?}");
            assert!(page.contains(r#"href="/workshop""#), "{tab:?}");
            assert!(page.contains(r#"href="/settings""#), "{tab:?}");
        }
        // And exactly one of them is lit, or the nav says nothing about where you are.
        assert_eq!(
            frame(Tab::Settings, "", None)
                .matches("class=\"on\"")
                .count(),
            1
        );
    }

    use super::*;

    #[test]
    fn a_machine_name_cannot_write_into_the_page() {
        // A hostname comes from outside this program, and a page that pasted it in raw would be
        // a page somebody could write into.
        assert_eq!(
            escape("<script>x</script>"),
            "&lt;script&gt;x&lt;/script&gt;"
        );
        assert_eq!(escape("a & \"b\""), "a &amp; &quot;b&quot;");
    }

    #[test]
    fn unpaired_shows_the_one_thing_to_do_next() {
        // Not an error, and not a wall of state. It is what every machine is before somebody
        // pairs it.
        let page = render(
            &Bond::default(),
            &Have::default(),
            "http://127.0.0.1:11500",
            None,
            None,
        );
        assert!(page.contains("Let the Host reach out"));
        // **One thing**, literally. There is no code field to type into any more — the code is
        // shown here and typed on the Host, which is the only direction that has ever worked on
        // the network this was built against.
        assert!(page.contains("SHOW A CODE"));
        assert!(!page.contains("name=\"code\""));
        assert!(!page.contains("DISCONNECT"));
    }

    #[test]
    fn paired_says_what_it_lends_and_what_it_does_not_keep() {
        let bond = Bond {
            id: "bridge-1".into(),
            secret: "s".into(),
            host: "http://10.0.0.2:11500".into(),
        };
        let have = Have {
            models: vec!["qwen3:14b".into(), "qwen3:8b".into()],
            ..Have::default()
        };
        let page = render(&bond, &have, "http://10.0.0.9:11500", None, None);
        assert!(page.contains("Lending 2 models"));
        assert!(page.contains("DISCONNECT"));
        assert!(
            page.contains("Nothing about its Worlds"),
            "the page says what this machine does not hold"
        );
    }

    #[test]
    fn one_door_is_offered_and_it_is_the_one_that_works_here() {
        // **Both were offered, and offering both was defensible** — they are the same exchange
        // with the socket opened from opposite ends, and each works where the other does not.
        //
        // What settled it (owner, 2026-08-20) is that on this network only one of them has ever
        // worked: the MacBook on Wi-Fi could not open a connection to the Host at all, on any
        // port, with the rule present and a listener confirmed up, while every connection the
        // Host started succeeded. A second door somebody has to read about, choose between, and
        // then watch time out is worse than one door that works.
        //
        // The route and `door::redeem` stay — the other direction is half of a documented
        // protocol (ADR-0029) and a real network somewhere needs it. What is gone is asking a
        // person to decide.
        let page = render(
            &Bond::default(),
            &Have::default(),
            "http://127.0.0.1:11500",
            None,
            None,
        );
        assert!(page.contains("Let the Host reach out"));
        assert!(page.contains("SHOW A CODE"));
        assert!(!page.contains("Or connect to a Host from here"));
        assert!(
            !page.contains("action=\"/pair\""),
            "no second form to choose between"
        );
        // And nothing is on screen until somebody asks for it: a code showing by itself would be
        // an open invitation nobody chose to make.
        assert!(!page.contains("STOP SHOWING"));
    }

    #[test]
    fn a_shown_code_says_how_long_it_has_left() {
        // Its absence cost an evening in the other direction: a code ran out mid-typing and the
        // only symptom was a failure on the far machine that named the network.
        let page = render(
            &Bond::default(),
            &Have::default(),
            "http://127.0.0.1:11500",
            None,
            Some(("K7M2QX", 185)),
        );
        assert!(page.contains("K7M2QX"));
        assert!(page.contains("3:05"), "minutes and seconds, not 185");
        assert!(page.contains("STOP SHOWING"));
        assert!(!page.contains("SHOW A CODE"), "one state at a time");

        // **It ticks.** Without this the number sat at 4:59 until something else happened to
        // reload the page, which is a clock that lies about the one thing it is for.
        assert!(
            page.contains(r#"http-equiv="refresh" content="1""#),
            "a countdown has to reload every second to be a countdown"
        );
        // And there is nothing on screen for that reload to empty.
        assert!(
            !page.contains(r#"action="/pair""#),
            "no form beside a ticking code"
        );
    }

    #[test]
    fn nothing_is_fetched_from_anywhere() {
        // A program whose job is to be a careful door does not fetch its own appearance from
        // the internet — and on a machine with no route out, a page that did would render bare.
        let page = render(
            &Bond::default(),
            &Have::default(),
            "http://127.0.0.1:11500",
            None,
            None,
        );
        for reaching in ["http://fonts", "https://", "cdn", "<script"] {
            assert!(!page.contains(reaching), "{reaching} must not appear");
        }
    }

    #[test]
    fn unified_memory_is_not_split_into_a_video_share() {
        // An Apple machine has no VRAM to report, and inventing one would be a division the
        // hardware does not have.
        let mac = Have {
            ram_total: Some(17_179_869_184),
            ..Have::default()
        };
        let said = hardware(&mac);
        assert_eq!(said, "17 GB memory");
    }

    #[test]
    fn a_key_that_is_stored_offers_no_field_to_read_it_back_from() {
        // Write-only by shape rather than by care: there is nowhere on this page a stored value
        // could travel back through.
        let held = super::settings(
            &epoch_models::hf::Cli::default(),
            "/models",
            &[("civitai", "Civitai", true)],
            None,
        );
        assert!(held.contains("a key is stored"), "{held}");
        assert!(held.contains("FORGET"), "{held}");
        assert!(!held.contains("paste your key"), "{held}");

        let empty = super::settings(
            &epoch_models::hf::Cli::default(),
            "/models",
            &[("civitai", "Civitai", false)],
            None,
        );
        assert!(empty.contains("paste your key"), "{empty}");
        assert!(empty.contains(r#"type="password""#), "{empty}");
        // And the caution is on screen before anybody pastes anything.
        assert!(empty.contains("account token"), "{empty}");
    }
}
