//! Mapping a model's curve **on this machine's card**.
//!
//! ## Why this exists, and why it did not until now
//!
//! A curve is kept as [`Best`], keyed by *model + card + build* — so it is a fact about one
//! graphics card and one llama.cpp, by construction. `deck::about` reads the `loadouts.json`
//! that lives beside **this** machine's library, with **this** machine's card.
//!
//! On a lent machine that file was empty and **nothing could ever fill it.** This page refused
//! to start a search, the Host's Workshop measures the Host, and the page said so in a sentence
//! that was simply untrue:
//!
//! > *Epoch's own Workshop can map the curve.*
//!
//! It cannot. It maps the Host's card. A MacBook lent to a desktop was being shown a control
//! whose only advice pointed at a measurement of somebody else's hardware — which is worse than
//! a blank reading, because it sends the reader somewhere confidently wrong.
//!
//! The owner put it plainly: *it is a different PC with everything different; we are not going
//! to use a configuration made on a 4070 SUPER for a MacBook.* Exactly so.
//!
//! ## What had to move: nothing
//!
//! Worth writing down, because the cost was assumed to be a crate boundary and measured to be
//! zero. The search itself is [`loadout::explore`] and both real probes — `Bench` for
//! llama.cpp, `Served` for Ollama — are already in `epoch-models`, which this program links.
//! ADR-0029 keeps `epoch-engine` out of here and never came into it.
//!
//! So the search was runnable from this program the whole time and had simply never been
//! wired to a route. **The boundary was believed rather than checked**, and one grep at the
//! right altitude replaced several minutes of planning to move eight hundred lines.
//!
//! ## It stays the Host's decision to ask
//!
//! Mapping a curve costs minutes of the card somebody lent you, so this never happens on a read
//! path and never on a page load: it is a button on this machine, pressed by whoever owns it.
//! The Host cannot start one over the bridge, which keeps `/have` and `/draw` the only two
//! routes a turn reaches.

use epoch_models::loadout::{self, Best, Caches, Loadout, Probe, Run};
use epoch_models::{generative::Library, machine::Machine, runtimes};

/// How far a search has got, so a page can say what is happening rather than freeze.
pub struct Watched<'a> {
    inner: &'a mut dyn Probe,
    seen: &'a mut usize,
    say: &'a dyn Fn(usize, usize, Loadout),
    total: usize,
}

impl Probe for Watched<'_> {
    fn run(&mut self, one: Loadout) -> Option<Run> {
        // **Said before the probe rather than after it**, so the line reads *what it is doing
        // now* rather than what it has finished. A four-minute job that only speaks on
        // completion is one somebody kills at three.
        (self.say)(*self.seen, self.total, one);
        let got = self.inner.run(one);
        *self.seen += 1;
        got
    }
}

/// Map one model's curve here, and remember it against this card.
///
/// `on` is the runtime id — `llama_cpp` or `ollama`. LM Studio is refused by name and says why,
/// rather than offering a button whose answer nothing could apply.
pub fn map(
    name: &str,
    on: &str,
    models_dir: &std::path::Path,
    say: &dyn Fn(usize, usize, Loadout),
) -> Result<String, String> {
    let held = runtimes::everything_here(&[models_dir.to_path_buf()]);
    let one = held
        .iter()
        .find(|it| it.name == name)
        .ok_or_else(|| format!("{name} is not a model this machine reported holding."))?;
    let file = std::path::PathBuf::from(&one.path);

    // The ceiling is what this model was trained for, when the header says. A ladder that ran
    // past it would be measuring settings the model cannot use.
    let trained = epoch_models::gguf::read(&file)
        .ok()
        .and_then(|header| header.trained_context())
        .and_then(|it| u32::try_from(it).ok())
        .unwrap_or(loadout::LADDER[loadout::LADDER.len() - 1]);

    let (mut probe, caches, build): (Box<dyn Probe>, Caches, String) =
        match on {
            "llama_cpp" => (
                Box::new(runtimes::Bench::new(&file).ok_or(
                    "llama.cpp is not on this machine, so there is nothing to measure with.",
                )?),
                // llama.cpp is spawned per setting, so the cache type is a flag on the child and
                // both can be compared.
                Caches::Either,
                runtimes::llama_build(),
            ),
            "ollama" => {
                let seen = runtimes::look_for(runtimes::Runtime::Ollama);
                if !seen.serving {
                    return Err(
                        "Ollama is not answering, and this curve runs through the server that is \
                     already up. Start it under START SERVICES and measure again."
                            .to_owned(),
                    );
                }
                let named = runtimes::offered_as(&one.name, &seen.models)
                    .ok_or_else(|| format!("Ollama is not serving '{}'.", one.name))?;
                (
                    Box::new(runtimes::Served {
                        endpoint: seen.endpoint.clone(),
                        model: named,
                    }),
                    // Ollama reads its cache type once when the *server* starts and ignores a
                    // per-request one silently, so the ladder maps context only and says so.
                    Caches::Only(runtimes::cache_asked_for(&Default::default())),
                    "Ollama".to_owned(),
                )
            }
            _ => return Err(
                "LM Studio takes a context only when a model is loaded, through its own CLI, so \
                 a curve there would end in a setting nothing applies. Measure on llama.cpp or \
                 Ollama."
                    .to_owned(),
            ),
        };

    let total = loadout::how_many(trained, caches);
    let mut done = 0usize;
    let mut watched = Watched {
        inner: probe.as_mut(),
        seen: &mut done,
        say,
        total,
    };
    let map = loadout::explore(trained, caches, &mut watched);
    if !map.loaded() {
        return Err(format!(
            "{} did not load at any setting on this machine.",
            one.name
        ));
    }
    let chose = map
        .recommended()
        .ok_or("nothing that loaded can hold a real turn")?;
    let rate = map.recommended_rate().unwrap_or_default();

    // **This machine's card and this machine's build.** That is the whole point: a curve keyed
    // to somebody else's hardware is not an answer about this one.
    let machine = Machine::measure();
    let library = Library::here();
    let mut known = loadout::Loadouts::load(library.root());
    known.remember(Best {
        model: loadout::names(&file).ok_or("the file went away while it was being measured")?,
        name: one.name.clone(),
        card: machine.gpu.clone().unwrap_or_default(),
        build,
        runtime: on.to_owned(),
        most: map.most_that_loaded(),
        chose,
        tokens_per_second: rate,
        tried: map
            .readings
            .iter()
            .map(|r| loadout::Tried {
                loadout: r.loadout,
                tokens_per_second: r.tokens_per_second,
            })
            .collect(),
        at: now(),
    });
    known.save(library.root())?;

    Ok(format!(
        "Measured on {} — {} at {} tokens per second.",
        machine.gpu.unwrap_or_else(|| "this machine".to_owned()),
        thousands(chose.context),
        (rate * 10.0).round() / 10.0,
    ))
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|it| it.as_secs())
        .unwrap_or_default()
}

fn thousands(n: u32) -> String {
    let raw = n.to_string();
    let mut out = String::new();
    for (i, c) in raw.chars().enumerate() {
        if i > 0 && (raw.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}
