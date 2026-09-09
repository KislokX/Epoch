//! The Models Workshop, as a page.
//!
//! ## No script, and the tabs are links
//!
//! This program's CSP is `script-src: 'none'`, and that is not an obstacle to work around — it is
//! the reason a careful door can be trusted. So the tabs are two URLs, every action is a form,
//! and the one thing that genuinely needs to change over time — a download in progress — uses
//! `<meta http-equiv="refresh">`, which is HTML older than the problem.
//!
//! ## What it refuses to hide
//!
//! A model larger than this machine's memory can still be pulled. The button says **PULL ANYWAY**
//! and the sentence beside it says what will happen, because Epoch does not decide for somebody
//! who already decided. What it will not do is let that happen *silently*, which is exactly what
//! happened before this existed: 17.7 GB arrived on a 17.2 GB machine and nothing said a word.

use epoch_models::{Facet, Group, Machine, Offer};

use crate::workshop::{gb, verdict};

/// Everything the Workshop tab shows.
///
/// A struct rather than ten arguments, and the compiler said so first — a parameter list that
/// long is a set of things that wanted to be one thing. It is also what the bench holds between
/// requests, which is not a coincidence: a page with `script-src: 'none'` keeps nothing itself,
/// so this *is* the tab's state.
pub struct View<'a> {
    /// What Ollama on this machine reports having.
    pub installed: &'a [String],
    pub machine: &'a Machine,
    /// The last verdict, or the reason there is none.
    pub weighed: Option<&'a Result<Offer, String>>,
    /// What was typed into the weighing field, kept so an answer does not empty it.
    pub asked: &'a str,
    /// What is downloading right now, in the runtime's own words.
    pub pulling: Option<&'a str>,
    /// How the last download ended.
    pub said: Option<&'a str>,
    /// What was typed into the search field.
    pub query: &'a str,
    /// Which facet chips are pressed.
    pub wanted: &'a [Facet],
    pub found: &'a [Offer],
    /// Which page of the results is being read.
    pub page: usize,
    /// Which repository is open into its quantisations, if any.
    pub opened: Option<&'a str>,
    /// What that repository publishes, or why it could not be read.
    pub variants: Option<&'a Result<Vec<Group>, String>>,
    /// The cursor for the next page, when Hugging Face said there is one.
    pub more: Option<&'a str>,
    pub shelf: &'a [Offer],
    /// Whether this machine can put a GGUF on disk, and who it is signed in as.
    pub hf: &'a epoch_models::hf::Cli,
    /// A file being fetched right now, in `hf`'s terms.
    pub filing: Option<&'a str>,
    /// What this machine can run, when somebody has asked. `None` means nobody pressed it —
    /// which is different from an empty list, and the page says which.
    pub fits: Option<&'a Result<Vec<epoch_models::Suited>, String>>,
    /// How the last file download ended.
    pub filed: Option<&'a str>,
}

/// How many results one page shows.
///
/// The same number the desktop Workshop uses. It is a window onto everything fetched rather than
/// a request for a page: Hugging Face pages by cursor and there is no way back to an earlier
/// one, so everything that arrives is kept and this decides how much of it is on screen.
const PER_PAGE: usize = 24;

/// The Workshop tab, whole.
pub fn render(view: &View<'_>) -> String {
    format!(
        r#"  {notice}

  <section class="card">
    <span class="label">This machine</span>
    <p class="quiet">{hardware}</p>
    <p class="quiet">
      Every verdict below is measured <b>here</b>. The Host's Workshop measures the Host —
      which is a different computer, and a confident answer about somewhere else is worse
      than none.
    </p>
  </section>

  <section class="card">
    <span class="label">Will it run here?</span>
    <form method="post" action="/weigh">
      <input name="model" value="{asked}" placeholder="qwen3.8:9b" autocomplete="off"
             spellcheck="false">
      <button type="submit">WEIGH</button>
    </form>
    <p class="quiet">
      An Ollama name, or <code>hf.co/user/repo</code>. The size comes from the manifest —
      what a pull actually downloads.
    </p>
    {verdict}
  </section>

  <section class="card">
    <span class="label">What fits on this machine</span>
    <p class="quiet">
      Ordered by downloads, which is Hugging Face's own count — Epoch has no way to measure
      whether a model is <i>good</i>, and a list ordered by something it invented would be a
      gauge nobody can explain with a download button under it. What Epoch measures is whether
      it <b>fits</b>: this card's memory against the real byte count of each quantisation.
    </p>
    <form method="post" action="/fits"><button type="submit">WHAT FITS</button></form>
    {fits}
  </section>

  <section class="card">
    <span class="label">Search Hugging Face</span>
    <form method="post" action="/search">
      <input name="query" value="{query}" placeholder="qwen, llama, gemma…" autocomplete="off"
             spellcheck="false">
      {chips}
      <button type="submit">SEARCH</button>
    </form>
    <p class="quiet">
      GGUF only — what Ollama can actually pull. Open a repository to see every quantisation
      it publishes and what each costs <b>here</b>: a repository holds a different size for
      every one of them, and only some of them run on this machine.
    </p>
    {found}
  </section>

  <section class="card">
    <span class="label">Featured by Ollama</span>
    <p class="quiet">
      A short list Ollama is promoting, not the whole library — and it names models Ollama
      runs in its own cloud, which have nothing to download.
    </p>
    {shelf}
  </section>

  <section class="card">
    <span class="label">Models here</span>
    {installed}
  </section>
"#,
        notice = [view.said, view.filed]
            .into_iter()
            .flatten()
            .map(|text| format!(r#"<p class="notice">{}</p>"#, crate::page::escape(text)))
            .collect::<String>(),
        hardware = crate::page::escape(&describe(view.machine)),
        asked = crate::page::escape(view.asked),
        verdict = verdict_block(view),
        query = crate::page::escape(view.query),
        chips = chips_block(view.wanted),
        found = results_block(view),
        fits = fits_block(view),
        shelf = offers_block(view.shelf, "Ollama's list could not be read."),
        installed = installed_block(view.installed),
    )
}

/// What this machine can run, twenty at a time.
///
/// **Nothing until it is asked.** It costs a page of requests and one per repository, so an
/// empty area here means nobody pressed the button — which is a different sentence from *nothing
/// fits*, and both are said in their own words.
fn fits_block(view: &View<'_>) -> String {
    let found = match view.fits {
        None => return String::new(),
        Some(Err(why)) => {
            return format!(r#"<p class="notice">{}</p>"#, crate::page::escape(why));
        }
        Some(Ok(found)) if found.is_empty() => {
            return r#"<p class="quiet">Nothing on that list fits this machine's card once room
               for the conversation is kept back.</p>"#
                .to_owned();
        }
        Some(Ok(found)) => found,
    };

    let boxes: String = found
        .iter()
        .map(|one| {
            format!(
                r#"<div class="fits__box{here}">
      <span class="fits__rank">{rank}</span>
      <p class="fits__name">{name}</p>
      <p class="quiet">{quant}{size}</p>
      <p class="quiet">{speed}</p>
      {action}
    </div>"#,
                here = if one.here.is_some() { " on" } else { "" },
                rank = one.rank,
                name = crate::page::escape(shortly(&one.repo)),
                quant = if one.quant.is_empty() {
                    String::new()
                } else {
                    format!("{} &middot; ", crate::page::escape(&one.quant))
                },
                size = crate::page::escape(&weigh(one.bytes)),
                // Measured says where; an estimate says so. A machine that has timed nothing
                // gets neither, because there is no honest way to turn a size into a speed
                // without one real answer to divide by.
                speed = match (one.tokens_per_second, one.measured_on.as_deref()) {
                    (Some(rate), Some(runtime)) => format!(
                        "{rate:.1} tok/s, measured on {}",
                        crate::page::escape(runtime)
                    ),
                    (Some(rate), None) => format!("about {rate:.1} tok/s, estimated"),
                    _ => "no speed measured here yet".to_owned(),
                },
                action = match &one.here {
                    // Already on this machine. Offering a download for ten gigabytes somebody
                    // already has is the mistake this list was taught to stop making.
                    Some(where_) => format!(
                        r#"<p class="quiet">already here &middot; {}</p>"#,
                        crate::page::escape(where_)
                    ),
                    None if one.pull.is_empty() => String::new(),
                    None => format!(
                        r#"<form method="post" action="/pull">
        <input type="hidden" name="model" value="{pull}">
        <button type="submit">DOWNLOAD</button>
      </form>"#,
                        pull = crate::page::escape(&one.pull),
                    ),
                },
            )
        })
        .collect();

    format!(r#"<div class="fits">{boxes}</div>"#)
}

/// A repository name, as short as it can be without becoming ambiguous.
///
/// The owner is dropped because twenty rows of `unsloth/` is twenty repetitions of the one thing
/// that does not distinguish them.
fn shortly(repo: &str) -> &str {
    repo.rsplit('/').next().unwrap_or(repo)
}

/// Bytes, in the units a person compares cards in.
fn weigh(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

/// The facet chips, as checkboxes that survive an answer.
///
/// **Checkboxes rather than links, and one name each.** They live inside the search form, so a
/// chip and a query are submitted together — pressing SEARCH is the one gesture, exactly as it
/// is on the desktop. A checkbox group would share a name and repeat it, and this program reads
/// a form into a map: the second `facet=` would replace the first.
///
/// Every facet is here, including the ones a Hugging Face search cannot be asked about — they
/// are filtered from what arrives. Hiding them would make the two Workshops disagree about what
/// a search can do.
fn chips_block(wanted: &[Facet]) -> String {
    let chips: String = Facet::ALL
        .into_iter()
        .map(|facet| {
            format!(
                r#"<label class="chip{on}">
        <input type="checkbox" name="facet_{id}"{checked}> {id}
      </label>"#,
                id = facet.id(),
                on = if wanted.contains(&facet) {
                    " chip--on"
                } else {
                    ""
                },
                checked = if wanted.contains(&facet) {
                    " checked"
                } else {
                    ""
                },
            )
        })
        .collect();
    format!(r#"<div class="chips">{chips}</div>"#)
}

/// The search results: one page of them, each openable, with a pager under it.
///
/// **A window onto everything fetched, never a request for a page.** Hugging Face pages by
/// cursor and there is no way back to an earlier one — so MORE RESULTS appends, and this decides
/// how much of the pile is on screen. That is what makes a numbered pager possible with no
/// script at all: every page is already here.
fn results_block(view: &View<'_>) -> String {
    if view.found.is_empty() {
        return r#"<p class="quiet">Nothing searched yet.</p>"#.to_owned();
    }

    let pages = view.found.len().div_ceil(PER_PAGE).max(1);
    // A page number that outlived its list is clamped rather than refused: it is a window, and a
    // window past the end of what exists shows the end of what exists.
    let page = view.page.min(pages - 1);
    let from = page * PER_PAGE;
    let rows: String = view.found[from..(from + PER_PAGE).min(view.found.len())]
        .iter()
        .map(|offer| one_result(offer, view))
        .collect();

    let more = view
        .more
        .map(|cursor| {
            format!(
                r#"<form method="post" action="/search">
      <input type="hidden" name="cursor" value="{}">
      <button type="submit">MORE RESULTS</button>
    </form>"#,
                crate::page::escape(cursor)
            )
        })
        .unwrap_or_default();

    format!(
        r#"<ul class="offers">{rows}</ul>
    {pager}
    <p class="quiet">{count} fetched so far{ending}</p>{more}"#,
        pager = pager_block(page, pages),
        count = view.found.len(),
        // **What arrived, never how much exists.** Hugging Face never says how many results a
        // query has, so neither does this.
        ending = if view.more.is_none() {
            " · that is the end of the results"
        } else {
            ""
        },
    )
}

/// One result, and its quantisations when it is the open one.
fn one_result(offer: &Offer, view: &View<'_>) -> String {
    let open = view.opened == Some(offer.name.as_str());
    format!(
        r#"<li class="{class}">
      <form method="post" action="/open">
        <input type="hidden" name="repo" value="{name}">
        <button type="submit" class="link" title="{name}">{arrow}{name}</button>
      </form>
      {facets}{mark}
      {table}
    </li>"#,
        class = if open { "open" } else { "" },
        name = crate::page::escape(&offer.name),
        // The same gesture as the desktop: pressing the open one closes it.
        arrow = if open { "&#9662; " } else { "&#9656; " },
        // What the source says it can do, in the source's own words. Absent when it says
        // nothing, because an empty list and *not described* are different answers.
        facets = if offer.facets.is_empty() {
            String::new()
        } else {
            format!(
                r#"<em class="quiet">{}</em>"#,
                offer
                    .facets
                    .iter()
                    .map(|f| f.id())
                    .collect::<Vec<_>>()
                    .join(" &#183; ")
            )
        },
        mark = if offer.installed {
            r#"<em class="quiet">already here</em>"#
        } else {
            ""
        },
        table = if open {
            variants_block(view)
        } else {
            String::new()
        },
    )
}

/// Every quantisation a repository publishes, laid out the way its own page lays them out.
///
/// ## Why a repository opens rather than resolving to a size
///
/// `unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF` publishes twenty-seven of these, from 8.91 GB to
/// 61.1 GB. "Will it run here?" has no answer for the repository and a different answer for each
/// of them, and choosing between `UD-Q4_K_M` and `UD-Q4_K_XL` is a real decision somebody is
/// making — not one to be made for them.
///
/// ## And the one thing Hugging Face's own page cannot say
///
/// Which of them run on **this** machine, measured here. A list of names and sizes is available
/// anywhere; the verdict beside each is the reason to have one inside this program.
fn variants_block(view: &View<'_>) -> String {
    let read = match view.variants {
        None => return r#"<p class="quiet">Reading what it publishes&#8230;</p>"#.to_owned(),
        Some(Err(why)) => return format!(r#"<p class="notice">{}</p>"#, crate::page::escape(why)),
        Some(Ok(groups)) => groups,
    };
    if read.is_empty() {
        return r#"<p class="quiet">It publishes no GGUF files.</p>"#.to_owned();
    }

    let rows: String = read
        .iter()
        .map(|group| {
            let quants: String = group
                .variants
                .iter()
                .map(|variant| {
                    // The same rule the verdict panel uses, from the same function. Asking
                    // only about video memory made every chip on an Apple machine read *cannot
                    // say*, on a page that was saying `RUNS HERE` about the same file.
                    let fits = crate::workshop::fits(variant.bytes, view.machine);
                    format!(
                        r#"<form method="post" action="/weigh" class="qform">
            <input type="hidden" name="model" value="{pull}">
            <button type="submit" class="quant quant--{tone}" title="{why}">
              {tag}<b>{quant}</b> <span>{size}</span>{parts}
            </button>
          </form>"#,
                        pull = crate::page::escape(&variant.pull),
                        tone = match fits {
                            Some(true) => "yes",
                            Some(false) => "no",
                            None => "unknown",
                        },
                        why = crate::page::escape(&why_of(variant, fits, view.machine)),
                        // `MTP` is a module published beside the model, not a version of it —
                        // 1.37 GB next to a 27B. Unlabelled, the cheapest-looking entry in the
                        // row is the one that is not the model at all.
                        tag = variant
                            .tag
                            .as_deref()
                            .map(|t| format!(r#"<i class="qtag">{}</i>"#, crate::page::escape(t)))
                            .unwrap_or_default(),
                        quant = crate::page::escape(&variant.quant),
                        size = gb(variant.bytes),
                        // Several files is a shard set, and Ollama refuses those by name:
                        // *"Ollama does not yet support pulling sharded GGUF via the
                        // registry"*. Said on the chip, because that is where somebody is
                        // choosing — and the file is still fetchable with `hf`, so one
                        // destination closes rather than the variant disappearing.
                        parts = if variant.files > 1 {
                            format!(
                                r#"<em class="qparts">{} files{}</em>"#,
                                variant.files,
                                if variant.pullable {
                                    ""
                                } else {
                                    " &#183; not via Ollama"
                                },
                            )
                        } else {
                            String::new()
                        },
                    )
                })
                .collect();
            format!(
                r#"<div class="qrow">
          <span class="qbits">{bits}</span>
          <div class="qlist">{quants}</div>
        </div>"#,
                // Zero means the name said nothing about width — honest, and rare.
                bits = if group.bits > 0 {
                    format!("{}-bit", group.bits)
                } else {
                    "other".to_owned()
                },
            )
        })
        .collect();

    format!(
        r#"<div class="quants">{rows}<p class="quiet">
      Press one to weigh it. Nothing downloads until then &#8212; the whole point of this tab
      is that a size is measured against <b>this</b> machine first.{filing}
    </p></div>"#,
        filing = if view.hf.installed {
            " A weighed one can also be saved as a file, for llama.cpp or LM Studio."
        } else {
            ""
        },
    )
}

/// Why one quantisation runs here or does not, in a sentence a tooltip can carry.
fn why_of(variant: &epoch_models::Variant, fits: Option<bool>, machine: &Machine) -> String {
    let head = match variant.tag.as_deref() {
        Some("MTP") => format!(
            "{} - the MTP module, published beside the model rather than a version of it. ",
            variant.quant
        ),
        _ => format!("{} - ", variant.quant),
    };
    match fits {
        None => format!(
            "{head}this machine's memory could not be read, so nobody can say whether it runs here."
        ),
        Some(true) => match machine.vram_free {
            Some(_) if machine.unified => {
                format!("{head}fits in this machine's free unified memory.")
            }
            Some(_) => format!("{head}fits in this machine's free video memory."),
            None => format!("{head}fits in this machine's memory."),
        },
        Some(false) => match (machine.vram_free, machine.ram_total) {
            // One pool, so nothing spills: what does not fit does not run slowly, it does not
            // load. The number is the same; the advice it implies is the opposite.
            (Some(free), _) if machine.unified => format!(
                "{head}larger than the {} of unified memory free - this machine shares one pool \
                 with the system, so there is nowhere for the rest to go.",
                gb(free)
            ),
            (Some(free), _) => format!(
                "{head}larger than the {} of video memory free - it will spill into system \
                 memory and run slowly.",
                gb(free)
            ),
            // No card to spill *out of*: on this machine that memory is the memory a model
            // runs in, and saying "video memory" about it would name something it does not have.
            (None, Some(ram)) => format!(
                "{head}needs more than this machine's {} of memory.",
                gb(ram)
            ),
            (None, None) => format!("{head}larger than this machine can hold."),
        },
    }
}

/// The numbered pager, as forms.
///
/// Nothing to press when there is one page: a pager over a single page is a control that cannot
/// do anything, which is the same defect as a gauge nobody can explain.
fn pager_block(page: usize, pages: usize) -> String {
    if pages < 2 {
        return String::new();
    }
    fn step(to: usize, label: &str, on: bool, live: bool) -> String {
        if !live {
            return format!(r#"<span class="pg pg--dead">{label}</span>"#);
        }
        format!(
            r#"<form method="post" action="/page" class="pgf">
        <input type="hidden" name="page" value="{to}">
        <button type="submit" class="pg{on}">{label}</button>
      </form>"#,
            on = if on { " pg--on" } else { "" },
        )
    }

    let numbers: String = (0..pages)
        .map(|n| step(n, &(n + 1).to_string(), n == page, n != page))
        .collect();
    format!(
        r#"<div class="pager">{back}{numbers}{next}</div>"#,
        back = step(page.saturating_sub(1), "&#8249;", false, page > 0),
        next = step(page + 1, "&#8250;", false, page + 1 < pages),
    )
}

/// A list of offers with no page of its own - Ollama's featured shelf.
///
/// It is short, fixed, and not searchable, so it keeps the plain list the results outgrew.
fn offers_block(offers: &[Offer], empty: &str) -> String {
    if offers.is_empty() {
        return format!(r#"<p class="quiet">{}</p>"#, crate::page::escape(empty));
    }

    let rows: String = offers
        .iter()
        .map(|offer| {
            format!(
                r#"<li>
      <form method="post" action="/weigh">
        <input type="hidden" name="model" value="{pull}">
        <button type="submit" class="link" title="{name}">{name}</button>
      </form>
      {mark}
    </li>"#,
                pull = crate::page::escape(&offer.pull),
                name = crate::page::escape(&offer.name),
                // Already here is a measurement, not an offer - it comes from what Ollama on
                // this machine reports having.
                mark = if offer.installed {
                    r#"<em class="quiet">already here</em>"#
                } else {
                    ""
                },
            )
        })
        .collect();

    format!(r#"<ul class="offers">{rows}</ul>"#)
}

/// What was weighed, what it means here, and what can be done about it.
fn verdict_block(view: &View<'_>) -> String {
    // A download in progress outranks a verdict: it is the thing happening now, and the page
    // refreshes itself until it stops.
    if let Some(model) = view.pulling {
        return format!(
            r#"<p class="notice">Downloading <b>{}</b>… this page refreshes itself.</p>"#,
            crate::page::escape(model)
        );
    }
    // The same, for the other destination. **No percentage**: measured, `hf download` reports a
    // final path and nothing in between, and none appears on stderr when it is not talking to a
    // terminal. The file and its size are already known from the plan, so this says what it is
    // waiting for rather than inventing how far along it is.
    if let Some(what) = view.filing {
        return format!(
            r#"<p class="notice">Saving <b>{}</b>… this page refreshes itself.</p>"#,
            crate::page::escape(what)
        );
    }

    let Some(weighed) = view.weighed else {
        return String::new();
    };
    let offer = match weighed {
        Ok(offer) => offer,
        Err(why) => return format!(r#"<p class="notice">{}</p>"#, crate::page::escape(why)),
    };

    let said = verdict(offer, view.machine);
    let (word, tone) = match said.runs {
        Some(true) => ("RUNS HERE", "yes"),
        Some(false) => ("TOO BIG", "no"),
        None => ("CANNOT SAY", "unknown"),
    };

    format!(
        r#"<p class="verdict">
      <b class="runs runs--{tone}">{word}</b>
      {name} · {size}
    </p>
    <p class="quiet">{explained}</p>
    <div class="acts">
      {ollama}
      {file}
    </div>
    <p class="quiet">{destinations}</p>"#,
        name = crate::page::escape(&offer.name),
        size = offer.bytes.map(gb).unwrap_or_else(|| "size unknown".into()),
        explained = crate::page::escape(&said.said),
        // **Not disabled for being too big.** Somebody may want it on a machine they are about
        // to add memory to, or to run slowly on purpose. What is not allowed is for it to
        // happen quietly — so PULL ANYWAY exists and says what it is.
        //
        // A sharded one is different: it is not a decision anybody can override. A control that
        // cannot do what it says is worse than a missing one, so a sharded
        // variant loses the button and keeps the reason.
        ollama = if offer.pullable {
            format!(
                r#"<form method="post" action="/pull">
        <input type="hidden" name="model" value="{pull}">
        <button type="submit">{action}</button>
      </form>"#,
                pull = crate::page::escape(&offer.pull),
                action = if said.runs == Some(false) {
                    "PULL ANYWAY"
                } else {
                    "PULL"
                },
            )
        } else {
            r#"<p class="quiet">Ollama will not fetch a sharded GGUF through its registry &#8212; that is its own refusal, not a guess. Save the file instead.</p>"#.to_owned()
        },
        file = file_button(offer, view.hf),
        destinations = destinations(offer, view.hf),
    )
}

/// SAVE THE FILE, when there is something it could save.
///
/// **Only for a Hugging Face name, and only when `hf` is here.** Ollama's own registry has no
/// file to hand over - it files a model where Ollama will find it and nowhere else - and a
/// button offering one would be a control that cannot do what it says.
fn file_button(offer: &Offer, hf: &epoch_models::hf::Cli) -> String {
    if !hf.installed {
        return String::new();
    }
    let Some((repo, Some(quant))) = split(&offer.pull) else {
        return String::new();
    };
    format!(
        r#"<form method="post" action="/file">
        <input type="hidden" name="repo" value="{repo}">
        <input type="hidden" name="quant" value="{quant}">
        <button type="submit" class="quietbtn">SAVE THE FILE</button>
      </form>"#,
        repo = crate::page::escape(&repo),
        quant = crate::page::escape(&quant),
    )
}

/// What each destination is for, said once beside both.
fn destinations(offer: &Offer, hf: &epoch_models::hf::Cli) -> String {
    let hugging = offer.pull.starts_with("hf.co/");
    match (hugging, hf.installed) {
        (true, true) => format!(
            "Ollama runs it; SAVE THE FILE puts the GGUF on disk for llama.cpp or LM Studio{}.",
            hf.user
                .as_deref()
                .map(|who| format!(" (hf, signed in as {who})"))
                .unwrap_or_default()
        ),
        // A machine without it downloads through Ollama instead - fewer things it can do rather
        // than a fault. Said, rather than shown as a missing button nobody can explain.
        (true, false) => "Into Ollama on this machine. For llama.cpp or LM Studio the file has \
             to be copied instead; the Hugging Face CLI would do it here, and it is not on this \
             machine."
            .to_owned(),
        _ => "Into Ollama on this machine, which is what is lent to the paired Host.".to_owned(),
    }
}

/// A pull string as a repository and a quantisation.
///
/// `hf.co/unsloth/x:Q4_K_M` is Ollama's spelling; `hf` takes the two halves separately. The
/// colon is the split, and a repository path has none - so the last one is the one that matters.
fn split(pull: &str) -> Option<(String, Option<String>)> {
    let rest = pull
        .strip_prefix("hf.co/")
        .or_else(|| pull.strip_prefix("huggingface.co/"))?;
    Some(match rest.rsplit_once(':') {
        Some((repo, quant)) => (repo.to_owned(), Some(quant.to_owned())),
        None => (rest.to_owned(), None),
    })
}

fn installed_block(installed: &[String]) -> String {
    if installed.is_empty() {
        return r#"<p class="quiet">
      None. Ollama is not running, or nothing has been pulled — weigh something above and
      pull it from here.
    </p>"#
            .to_owned();
    }
    let rows: String = installed
        .iter()
        .map(|name| format!(r#"<li>{}</li>"#, crate::page::escape(name)))
        .collect();
    format!(
        r#"<ul class="models">{rows}</ul>
    <p class="quiet">Everything here is lent to the paired Host.</p>"#
    )
}

/// The hardware in one line, with every absent number simply absent.
fn describe(machine: &Machine) -> String {
    let mut parts = Vec::new();
    // **The words follow the architecture.** An Apple machine now answers this question — it
    // always could, nobody had asked it — and it answers with one pool the GPU reads directly.
    // Printing "video memory" over that number is a correct reading in the vocabulary of a
    // different machine, which is its own kind of wrong instrument.
    let pool = if machine.unified {
        "unified memory"
    } else {
        "video memory"
    };
    match (&machine.gpu, machine.vram_free, machine.vram_total) {
        (Some(gpu), Some(free), Some(total)) => {
            parts.push(format!("{gpu} · {} of {} {pool} free", gb(free), gb(total)))
        }
        (Some(gpu), _, _) => parts.push(gpu.clone()),
        // No card this program can ask about. Splitting system memory into a made-up video share
        // would invent a division the machine does not have.
        (None, _, _) => {}
    }
    // On a unified machine the total above *is* the system memory. Saying it twice reads as two
    // pools, which is exactly the impression the wording above exists to avoid.
    let said_already = machine.unified && machine.vram_total.is_some();
    if let Some(ram) = machine.ram_total {
        if !said_already {
            parts.push(format!("{} memory", gb(ram)));
        }
    }
    if parts.is_empty() {
        return "Hardware could not be read on this machine.".to_owned();
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Mac as Epoch used to see one: nothing but its RAM.
    ///
    /// Kept, because it is still a real machine — an Intel Mac whose card cannot be asked how
    /// much is free, or a Host reading an older EpochServices that does not send `unified`. It is
    /// no longer what an Apple Silicon machine looks like; see [`apple_silicon`].
    fn apple() -> Machine {
        Machine {
            unified: false,
            gpu: None,
            vram_total: None,
            vram_free: None,
            ram_total: Some(17_179_869_184),
        }
    }

    /// The owner's MacBook, measured 2026-08-25 once `Machine` learned to ask it.
    ///
    /// `Apple M2`, 17.18 GB of unified memory, 7.10 GB of it free — the free figure
    /// cross-checked against the `ram_free` ComfyUI's own torch build reported in the same
    /// second. Before this it answered nothing at all, because the only question being asked was
    /// `nvidia-smi`.
    fn apple_silicon() -> Machine {
        Machine {
            unified: true,
            gpu: Some("Apple M2".to_owned()),
            vram_total: Some(17_179_869_184),
            vram_free: Some(7_100_000_000),
            ram_total: Some(17_179_869_184),
        }
    }

    fn suited(rank: usize, repo: &str, here: Option<&str>) -> epoch_models::Suited {
        epoch_models::Suited {
            rank,
            repo: repo.into(),
            quant: "Q4_K_M".into(),
            bytes: 7_400_000_000,
            pull: format!("hf.co/{repo}:Q4_K_M"),
            facets: Vec::new(),
            here: here.map(str::to_owned),
            tokens_per_second: None,
            measured_on: None,
        }
    }

    /// **Three states, and they are three different sentences.** Unasked, asked-and-empty, and
    /// asked-and-answered. Collapsing the first two would tell somebody nothing fits on a machine
    /// nobody has measured.
    #[test]
    fn what_fits_says_nothing_until_it_is_asked() {
        let machine = apple_silicon();
        let hf = epoch_models::hf::Cli::default();
        let page = render(&plain(&machine, &hf));
        assert!(
            page.contains("WHAT FITS"),
            "the button is always there: {page}"
        );
        assert!(
            !page.contains("Nothing on that list fits"),
            "unasked is not empty: {page}"
        );
        assert!(!page.contains("fits__box"), "{page}");
    }

    #[test]
    fn an_empty_answer_says_it_was_asked_and_nothing_fits() {
        let machine = apple_silicon();
        let hf = epoch_models::hf::Cli::default();
        let empty = Ok(Vec::new());
        let page = render(&View {
            fits: Some(&empty),
            ..plain(&machine, &hf)
        });
        assert!(page.contains("Nothing on that list fits"), "{page}");
    }

    #[test]
    fn a_model_already_here_is_marked_rather_than_offered_for_download() {
        // The mistake this list was taught to stop making: a DOWNLOAD button under ten
        // gigabytes somebody already has.
        let machine = apple_silicon();
        let hf = epoch_models::hf::Cli::default();
        let found = Ok(vec![
            suited(1, "unsloth/Qwen3.8-27B-GGUF", Some("Ollama")),
            suited(2, "bartowski/gemma-4-12b-GGUF", None),
        ]);
        let page = render(&View {
            fits: Some(&found),
            ..plain(&machine, &hf)
        });
        assert!(page.contains("already here &middot; Ollama"), "{page}");
        assert_eq!(page.matches("DOWNLOAD").count(), 1, "{page}");
        // The owner is dropped: twenty rows of `unsloth/` distinguishes nothing.
        assert!(page.contains("Qwen3.8-27B-GGUF"), "{page}");
        // And a machine that has timed nothing claims no speed for anything.
        assert!(page.contains("no speed measured here yet"), "{page}");
    }

    /// A Workshop with nothing on it.
    ///
    /// Struct-update syntax rather than eight literals: every field added to `View` was eight
    /// edits to this module, and the eighth is where somebody sets one of them differently by
    /// accident and a test stops testing what it says it tests.
    fn plain<'a>(machine: &'a Machine, hf: &'a epoch_models::hf::Cli) -> View<'a> {
        View {
            installed: &[],
            machine,
            fits: None,
            weighed: None,
            asked: "",
            pulling: None,
            said: None,
            query: "",
            wanted: &[],
            found: &[],
            page: 0,
            opened: None,
            variants: None,
            more: None,
            shelf: &[],
            hf,
            filing: None,
            filed: None,
        }
    }

    /// A machine with no `hf` on it. The ordinary case, and the one every existing test assumed.
    fn no_hf() -> epoch_models::hf::Cli {
        epoch_models::hf::Cli::default()
    }

    fn offer(name: &str, bytes: u64) -> Offer {
        Offer {
            pullable: true,
            name: name.into(),
            pull: name.into(),
            installed: false,
            bytes: Some(bytes),
            fits: None,
            source: "featured",
            facets: Vec::new(),
            described: false,
        }
    }

    #[test]
    fn a_model_that_does_not_fit_can_still_be_pulled_but_never_quietly() {
        // Epoch does not decide for somebody who already decided — somebody may be about to add
        // memory, or may want it slow on purpose. What is refused is silence: 17.7 GB arrived on
        // a 17.2 GB machine once and nothing said a word.
        let machine = apple();
        let hf = no_hf();
        let page = render(&View {
            weighed: Some(&Ok(offer("qwen3.8:27b", 17_700_000_000))),
            asked: "qwen3.8:27b",
            ..plain(&machine, &hf)
        });
        assert!(page.contains("TOO BIG"), "the verdict is a word");
        assert!(page.contains("PULL ANYWAY"), "and it is still possible");
        assert!(page.contains("will not fit"), "and it says why");
    }

    #[test]
    fn a_model_that_fits_gets_an_ordinary_button() {
        let machine = apple();
        let hf = no_hf();
        let page = render(&View {
            weighed: Some(&Ok(offer("qwen3.8:9b", 5_000_000_000))),
            asked: "qwen3.8:9b",
            ..plain(&machine, &hf)
        });
        assert!(page.contains("RUNS HERE"));
        assert!(!page.contains("PULL ANYWAY"));
    }

    #[test]
    fn a_download_in_progress_outranks_a_verdict() {
        // It is the thing happening now, and the page refreshes itself until it stops.
        let machine = apple();
        let hf = no_hf();
        let page = render(&View {
            weighed: Some(&Ok(offer("qwen3.8:9b", 5_000_000_000))),
            asked: "qwen3.8:9b",
            pulling: Some("qwen3.8:9b"),
            ..plain(&machine, &hf)
        });
        assert!(page.contains("Downloading"));
        assert!(
            !page.contains("RUNS HERE"),
            "the old verdict is not the news"
        );
    }

    #[test]
    fn the_page_says_where_it_measured() {
        // A verdict measured on the Host is worse than none: it is confident, and it is about a
        // different computer.
        let machine = apple();
        let hf = no_hf();
        let page = render(&View {
            ..plain(&machine, &hf)
        });
        assert!(page.contains("measured <b>here</b>"));
        assert!(page.contains("17.2 GB memory"));
    }

    #[test]
    fn a_name_on_a_list_can_only_be_weighed_never_pulled() {
        // The whole point of this tab: a size is measured against *this* machine before anything
        // is downloaded. A PULL beside an unweighed name would be exactly the silent download
        // that put 17.7 GB onto a 17.2 GB machine.
        let results = vec![offer("unsloth/Qwen3-8B-GGUF", 0), offer("bartowski/x", 0)];
        let machine = apple();
        let hf = no_hf();
        let page = render(&View {
            query: "qwen",
            found: &results,
            more: Some("https://huggingface.co/api/models?cursor=abc"),
            ..plain(&machine, &hf)
        });

        assert!(
            page.contains("unsloth/Qwen3-8B-GGUF"),
            "the result is listed"
        );
        // **Open, not weigh.** A repository is not one size: pressing its name asks what it
        // publishes, and picking a quantisation is what asks the question about a file.
        assert!(page.contains(r#"action="/open""#), "and it can be opened");
        assert!(
            !page.contains(r#"action="/pull""#),
            "nothing unweighed may offer a download"
        );
        assert!(page.contains("MORE RESULTS"), "and there is a next page");
        assert!(page.contains("qwen"), "the query survives the answer");
    }

    #[test]
    fn the_last_page_offers_nothing_to_press() {
        // No cursor is the end of the list, and a MORE button that fetched the same page forever
        // would be worse than no button.
        let machine = apple();
        let hf = no_hf();
        let page = render(&View {
            query: "qwen",
            found: &[offer("a/b", 0)],
            ..plain(&machine, &hf)
        });
        assert!(!page.contains("MORE RESULTS"));
    }

    #[test]
    fn both_lists_are_there_and_an_empty_one_says_which_it_is() {
        // Two sources with two different silences: nobody has searched yet, and Ollama's list
        // could not be read. Sharing one sentence would make a network failure look like an
        // empty search box.
        let machine = apple();
        let hf = no_hf();
        let page = render(&View {
            ..plain(&machine, &hf)
        });
        assert!(page.contains("Search Hugging Face"));
        assert!(page.contains("Featured by Ollama"));
        assert!(page.contains("Nothing searched yet."));
        assert!(page.contains("Ollama&#39;s list could not be read."));
    }

    fn variant(quant: &str, bytes: u64, files: usize) -> epoch_models::Variant {
        epoch_models::Variant {
            own_head: false,
            pullable: true,
            quant: quant.into(),
            bytes,
            files,
            pull: format!("hf.co/unsloth/Qwen3-8B-GGUF:{quant}"),
            tag: None,
        }
    }

    /// A machine with a card, for the tests that are about a verdict per quantisation.
    fn carded(free: u64) -> Machine {
        Machine {
            unified: false,
            gpu: Some("NVIDIA GeForce RTX 4070 SUPER".into()),
            vram_total: Some(12_900_000_000),
            vram_free: Some(free),
            ram_total: Some(34_000_000_000),
        }
    }

    #[test]
    fn a_repository_opens_into_its_quantisations_with_a_verdict_on_each() {
        // The one thing Hugging Face's own page cannot say: which of them run *here*. A list of
        // names and sizes is available anywhere.
        let machine = carded(9_100_000_000);
        let hf = no_hf();
        let groups = Ok(vec![epoch_models::Group {
            bits: 4,
            variants: vec![
                variant("Q4_K_M", 5_000_000_000, 1),
                variant("UD-Q4_K_XL", 17_600_000_000, 2),
            ],
        }]);
        let found = [offer("unsloth/Qwen3-8B-GGUF", 0)];
        let page = render(&View {
            query: "qwen",
            found: &found,
            opened: Some("unsloth/Qwen3-8B-GGUF"),
            variants: Some(&groups),
            ..plain(&machine, &hf)
        });

        assert!(page.contains("4-bit"), "laid out by width");
        assert!(page.contains("Q4_K_M"));
        assert!(page.contains("quant--yes"), "5 GB fits in 9.1 GB free");
        assert!(page.contains("quant--no"), "17.6 GB does not");
        // A download that arrives in parts is worth knowing about before it starts.
        assert!(page.contains("2 files"));
        // And picking one asks about that exact file, not about the repository.
        assert!(page.contains("hf.co/unsloth/Qwen3-8B-GGUF:UD-Q4_K_XL"));
    }

    #[test]
    fn a_sharded_variant_keeps_its_reason_and_loses_the_button() {
        // Measured against Ollama's own answer, not inferred: asking for a sharded tag returns
        // `400: This tag is a sharded GGUF. Ollama does not yet support pulling sharded GGUF
        // via the registry`. A PULL button beside it is a control that cannot do what it says.
        let machine = apple();
        let hf = no_hf();
        let sharded = Ok(epoch_models::Offer {
            pullable: false,
            ..offer("hf.co/unsloth/Qwen3.8-27B-GGUF:BF16", 55_600_000_000)
        });
        let page = render(&View {
            weighed: Some(&sharded),
            ..plain(&machine, &hf)
        });
        assert!(!page.contains(r#"action="/pull""#), "no button to press");
        assert!(page.contains("sharded"), "and it says whose refusal it is");

        // The ordinary one still has one.
        let single = Ok(offer(
            "hf.co/unsloth/Qwen3.8-27B-GGUF:UD-IQ1_S",
            6_190_000_000,
        ));
        let page = render(&View {
            weighed: Some(&single),
            ..plain(&machine, &hf)
        });
        assert!(page.contains(r#"action="/pull""#));
    }

    #[test]
    fn an_apple_machine_answers_from_the_memory_it_actually_has() {
        // It has no video memory to have — its memory is unified — and that is not a gap. The
        // chips used to ask only about a card, so every one of them read *cannot say* on a page
        // that was simultaneously saying RUNS HERE about the same file.
        let machine = apple();
        let hf = no_hf();
        let groups = Ok(vec![epoch_models::Group {
            bits: 4,
            variants: vec![variant("Q4_K_M", 5_000_000_000, 1)],
        }]);
        let found = [offer("unsloth/Qwen3-8B-GGUF", 0)];
        let page = render(&View {
            query: "qwen",
            found: &found,
            opened: Some("unsloth/Qwen3-8B-GGUF"),
            variants: Some(&groups),
            ..plain(&machine, &hf)
        });
        // 5 GB in 17.2 GB of unified memory.
        assert!(page.contains("quant--yes"), "{page}");
        assert!(
            page.contains("fits in this machine&#39;s memory"),
            "and says which memory"
        );
        assert!(
            !page.contains("video memory"),
            "which this machine does not have"
        );
    }

    /// One pool has to be printed as one pool.
    ///
    /// The page had its own copy of the wording, so when `Machine` started answering on a Mac the
    /// deck read **"Apple M2 · 7.4 GB of 17.2 GB video memory free · 17.2 GB memory"** — a true
    /// number in the vocabulary of a machine this is not, and the same total printed twice as if
    /// it were two pools. Found by opening the page on the machine, not by reading it.
    #[test]
    fn an_apple_silicon_machine_is_described_as_the_machine_it_is() {
        let machine = apple_silicon();
        let hf = no_hf();
        let groups = Ok(vec![epoch_models::Group {
            bits: 4,
            // Larger than the 7.1 GB free, so the refusal's wording is under test too.
            variants: vec![variant("Q8_0", 12_000_000_000, 1)],
        }]);
        let found = [offer("unsloth/Qwen3-8B-GGUF", 0)];
        let page = render(&View {
            query: "qwen",
            found: &found,
            opened: Some("unsloth/Qwen3-8B-GGUF"),
            variants: Some(&groups),
            ..plain(&machine, &hf)
        });

        assert!(page.contains("Apple M2"), "{page}");
        assert!(page.contains("unified memory"), "{page}");
        assert!(
            !page.contains("video memory"),
            "this machine has none: {page}"
        );
        // The same pool, said once. Two totals reads as two pools.
        assert!(
            !page.contains("17.2 GB memory"),
            "the total above is the system memory: {page}"
        );
        // And nothing to spill into, which is the advice rather than the number.
        assert!(!page.contains("spill into system"), "{page}");
    }

    #[test]
    fn a_machine_that_could_not_be_read_at_all_still_says_nothing_rather_than_no() {
        // Unknown is a different answer from *nothing fits*. This is the machine nothing could
        // be measured on, which is the only case left that has no answer.
        let machine = Machine {
            unified: false,
            gpu: None,
            vram_total: None,
            vram_free: None,
            ram_total: None,
        };
        let hf = no_hf();
        let groups = Ok(vec![epoch_models::Group {
            bits: 4,
            variants: vec![variant("Q4_K_M", 5_000_000_000, 1)],
        }]);
        let found = [offer("unsloth/Qwen3-8B-GGUF", 0)];
        let page = render(&View {
            query: "qwen",
            found: &found,
            opened: Some("unsloth/Qwen3-8B-GGUF"),
            variants: Some(&groups),
            ..plain(&machine, &hf)
        });
        assert!(page.contains("quant--unknown"));
        assert!(!page.contains("quant--no"));
    }

    #[test]
    fn the_chips_stay_pressed_after_an_answer() {
        // They live inside the search form, so a chip and a query are one gesture — and a chip
        // that forgot itself would silently widen the next search.
        let machine = apple();
        let hf = no_hf();
        let wanted = [epoch_models::Facet::Vision];
        let page = render(&View {
            query: "qwen",
            wanted: &wanted,
            ..plain(&machine, &hf)
        });
        assert!(page.contains(r#"name="facet_vision" checked"#), "{page}");
        // Every facet is offered, including the ones a search cannot be asked about — they are
        // filtered from what arrives, and hiding them would make the two Workshops disagree.
        assert!(page.contains(r#"name="facet_thinking""#));
    }

    #[test]
    fn a_second_page_appears_only_when_there_is_one() {
        // A pager over a single page is a control that cannot do anything.
        let machine = apple();
        let hf = no_hf();
        let one: Vec<Offer> = (0..10).map(|n| offer(&format!("a/{n}"), 0)).collect();
        let page = render(&View {
            query: "q",
            found: &one,
            ..plain(&machine, &hf)
        });
        assert!(!page.contains("class=\"pager\""));

        let many: Vec<Offer> = (0..60).map(|n| offer(&format!("a/{n}"), 0)).collect();
        let page = render(&View {
            query: "q",
            found: &many,
            page: 1,
            ..plain(&machine, &hf)
        });
        assert!(page.contains("class=\"pager\""));
        assert!(
            page.contains("a/24"),
            "the second page starts where the first ended"
        );
        assert!(!page.contains(">a/0<"), "and the first page is not on it");
    }

    #[test]
    fn a_page_number_past_the_end_shows_the_end() {
        // It is a window onto everything fetched, and a window past what exists shows what
        // exists — refusing would be a blank panel with a pager under it.
        let machine = apple();
        let hf = no_hf();
        let some: Vec<Offer> = (0..30).map(|n| offer(&format!("a/{n}"), 0)).collect();
        let page = render(&View {
            query: "q",
            found: &some,
            page: 99,
            ..plain(&machine, &hf)
        });
        assert!(page.contains("a/29"));
    }

    #[test]
    fn the_file_is_offered_only_where_there_is_one_to_save() {
        // Ollama's own registry has no file to hand over, and a machine without `hf` cannot
        // fetch one. A button in either case would be a control that cannot do what it says.
        let machine = apple();
        let weighed = Ok(offer("hf.co/unsloth/Qwen3-8B-GGUF:Q4_K_M", 5_000_000_000));

        let without = no_hf();
        let page = render(&View {
            weighed: Some(&weighed),
            ..plain(&machine, &without)
        });
        assert!(!page.contains("SAVE THE FILE"));
        assert!(page.contains("not on this"), "and it says why");

        let with = epoch_models::hf::Cli {
            installed: true,
            version: Some("1.28.0".into()),
            found_at: Some("/Users/someone/.local/bin/hf".into()),
            user: Some("KislokX".into()),
        };
        let page = render(&View {
            weighed: Some(&weighed),
            ..plain(&machine, &with)
        });
        assert!(page.contains("SAVE THE FILE"));
        assert!(page.contains("KislokX"), "who it would download as");
        assert!(page.contains(r#"name="quant" value="Q4_K_M""#));

        // An Ollama name has no repository behind it, so there is nothing to save even here.
        let ollama = Ok(offer("gemma4:12b", 8_000_000_000));
        let page = render(&View {
            weighed: Some(&ollama),
            ..plain(&machine, &with)
        });
        assert!(!page.contains("SAVE THE FILE"));
    }

    #[test]
    fn a_file_download_is_news_the_same_way_a_pull_is() {
        let machine = apple();
        let hf = no_hf();
        let weighed = Ok(offer("hf.co/unsloth/Qwen3-8B-GGUF:Q4_K_M", 5_000_000_000));
        let page = render(&View {
            weighed: Some(&weighed),
            filing: Some("unsloth/Qwen3-8B-GGUF · Qwen3-8B-Q4_K_M.gguf (5.0G)"),
            ..plain(&machine, &hf)
        });
        assert!(page.contains("Saving"));
        // The size it already knows, and no percentage: `hf` reports a final path and nothing
        // in between, so inventing progress is the one thing it must not do.
        assert!(page.contains("5.0G"));
        assert!(!page.contains('%'));
        assert!(
            !page.contains("RUNS HERE"),
            "the old verdict is not the news"
        );
    }

    #[test]
    fn a_model_name_cannot_write_into_the_page() {
        let machine = apple();
        let hf = no_hf();
        let page = render(&View {
            installed: &["<script>x</script>".to_owned()],
            ..plain(&machine, &hf)
        });
        assert!(!page.contains("<script>x"));
        assert!(page.contains("&lt;script&gt;"));
    }
}
