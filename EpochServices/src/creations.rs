//! The Creations Workshop, on a machine that is lending its card.
//!
//! ## Why this is here at all
//!
//! **A lent machine is the machine an asset would be installed onto** (ADR-0029 §9, and the
//! standing two-surfaces rule). A picture drawn on this computer loads *this* computer's
//! checkpoints and LoRAs; installing them on the Host would put a 6 GB file where nothing that
//! draws can reach it. So the shelf that fetches them belongs on both surfaces, or on the wrong
//! one.
//!
//! ## The same Engine, and none of the Engine
//!
//! Every question is asked through `epoch-models`: one `Catalogue`, one `Asset`, one library,
//! one install path — exactly as on the Host. What differs is the frame: no Kernel, no Quests,
//! no Trust, because this program owns none of those (ADR-0029 §9). The only thing written twice
//! is HTML.
//!
//! ## The same credential store
//!
//! A key lives in this operating system's own store — DPAPI, the Keychain, the Secret Service —
//! through the same `epoch-secrets` crate the Host uses. A machine lending its card is as likely
//! to be a Mac as a PC, which is the whole reason that crate exists rather than a second copy of
//! one. Settings is where a key is pasted; nothing here ever shows one back.

use epoch_models::catalogue::{self, Asset, Catalogue, MoreRow, Query, Refusal};

use crate::page;

/// What this tab is showing right now.
#[derive(Debug, Default)]
pub struct Shelf {
    /// What was typed, kept so an answer does not empty the field.
    pub query: String,
    /// Which kind was asked for, as the form spells it.
    pub kind: String,
    pub adult: bool,
    /// `most_downloaded` or `newest` — the two orders both sites genuinely support.
    pub order: String,
    /// A base model to narrow to, **in the site's own words**, or empty for all of them.
    pub base: String,
    /// Which medium to narrow the **results** to — `picture`, `video`, `sound`, `model`, or
    /// empty for all of them.
    ///
    /// Over what came back and not into the query: Civitai has no medium parameter and asking
    /// only Hugging Face would quietly turn a search of two sites into a search of one, which is
    /// what `base` does and says on the page.
    pub makes: String,
    /// Which medium the **shelves** list is narrowed to, or empty for all of them.
    ///
    /// Its own field and not the one above: the search asks two websites and the shelves are
    /// this machine's disk. One control governing both would move a list somebody was not
    /// looking at.
    pub shelved: String,
    /// The last page of results. `None` before anybody has searched.
    pub found: Option<Vec<Asset>>,
    /// **What is downloading, and how far it has got.** `None` when nothing is.
    ///
    /// The name, the bytes that have arrived, and the total **when the source stated one** —
    /// `None` there is *no total*, never zero, and the page then shows what has arrived rather
    /// than a bar guessing at a denominator.
    pub fetching: Option<(String, u64, Option<u64>)>,
    /// Sources that did not answer, in their own words. Never folded into an empty result.
    pub refused: Vec<String>,
    /// Where each source that has more got to.
    pub more: Vec<MoreRow>,
    /// Which page of results is on screen, counting from one.
    pub page: usize,
    /// How the last download ended.
    pub said: Option<String>,
}

/// Ask both catalogues, and keep what they said.
pub fn look(shelf: &mut Shelf, more: Vec<MoreRow>) {
    let civitai = epoch_models::civitai::Civitai::new();
    let hugging_face = epoch_models::hugging_face::HuggingFace::new();
    let sources: [&dyn Catalogue; 2] = [&civitai, &hugging_face];

    let mut found = Vec::new();
    let mut refused = Vec::new();
    let mut carry_on = Vec::new();
    for source in sources {
        let id = source_id(source.source());
        // Only the sources that can answer the question that was put. Civitai takes a base
        // model; Hugging Face publishes none, and filtering its answers on a word it never
        // prints would empty it.
        if !shelf.base.trim().is_empty() && !source.narrows_by_base() {
            continue;
        }
        let cursor = more.iter().find(|row| row.source == id);
        // Once paging has started, only the sources that still have somewhere to go are asked.
        // One site running out is not the end of the other, and re-asking a finished one would
        // put its first page back on the screen.
        if !more.is_empty() && cursor.is_none() {
            continue;
        }
        match source.search(&Query {
            words: shelf.query.clone(),
            kind: kind_named(&shelf.kind),
            adult: shelf.adult,
            order: match shelf.order.as_str() {
                "newest" => epoch_models::catalogue::Order::Newest,
                _ => epoch_models::catalogue::Order::MostDownloaded,
            },
            base: shelf.base.trim().to_owned(),
            more: cursor.map(|row| row.cursor.clone()),
        }) {
            Ok(listing) => {
                found.extend(listing.assets);
                if let Some(next) = listing.more {
                    carry_on.push(MoreRow {
                        source: id.to_owned(),
                        cursor: next,
                    });
                }
            }
            Err(why) => refused.push(format!("{}: {why}", source.source().name())),
        }
    }
    // Merged in the order that was asked for. For *newest* there is nothing local to sort on, so
    // the sites' own order is interleaved as it arrived rather than re-sorted by a number that
    // means something else.
    if shelf.order != "newest" {
        found.sort_by_key(|one| std::cmp::Reverse(one.downloads));
    }

    shelf.found = Some(found);
    shelf.refused = refused;
    shelf.more = carry_on;
}

/// Bring one asset onto *this* machine.
///
/// The Engine asks the catalogue again rather than trusting an id-shaped URL: what a form holds
/// is an id, and the address, the size and the hash come from the source at the moment of the
/// download.
pub fn install(source: &str, id: &str, watching: &dyn Fn(&str, u64, Option<u64>)) -> String {
    use epoch_models::generative::{Library, Shelf as Where};

    let civitai = epoch_models::civitai::Civitai::new();
    let hugging_face = epoch_models::hugging_face::HuggingFace::new();
    let catalogue: &dyn Catalogue = match source {
        "civitai" | "Civitai" => &civitai,
        "huggingface" | "Hugging Face" => &hugging_face,
        other => return format!("There is no catalogue called {other:?}."),
    };

    let asset = match catalogue.asset(id) {
        Ok(Some(asset)) => asset,
        Ok(None) => return format!("{source} no longer has {id}."),
        Err(why) => return why.to_string(),
    };
    let Some(file) = asset.principal().cloned() else {
        return "That has no file to download — it may have been withdrawn.".to_owned();
    };
    if file.needs_key {
        return format!(
            "{} hands files over only to an account, and this program has nowhere to keep a \
             credential. Install it from the Host, or fetch it there and copy it across.",
            asset.source.name()
        );
    }

    let library = Library::here();
    let where_to = library.shelf(Where::for_kind(asset.kind));
    match catalogue::fetch(&file, &where_to, None, &|done, total| {
        watching(&asset.name, done, total)
    }) {
        Ok(landed) => {
            let name = landed
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            match catalogue::identify(&landed, &[catalogue]) {
                Ok(known) => format!(
                    "{name} — {}, {}",
                    known.kind.plainly(),
                    known.family.plainly()
                ),
                Err(why) => format!("{name} arrived, and its own bytes could not be read: {why}"),
            }
        }
        Err(Refusal::NeedsKey(why)) => why,
        Err(why) => why.to_string(),
    }
}

const fn source_id(source: epoch_models::catalogue::Source) -> &'static str {
    use epoch_models::catalogue::Source;
    match source {
        Source::Civitai => "civitai",
        Source::HuggingFace => "huggingface",
    }
}

fn kind_named(said: &str) -> Option<epoch_assets::asset::Kind> {
    use epoch_assets::asset::Kind;
    match said {
        "checkpoint" => Some(Kind::Checkpoint),
        "lora" => Some(Kind::Lora),
        "vae" => Some(Kind::Vae),
        "controlnet" => Some(Kind::ControlNet),
        "embedding" => Some(Kind::Embedding),
        _ => None,
    }
}

/// Take a file this machine already has into the library.
///
/// ## A path, not an upload
///
/// This is a page rather than a desktop window, so there is no native picker to open and a
/// browser will not hand over a path. Uploading six gigabytes through a socket to land them
/// three folders away would be work for nothing — so the person on that machine types where the
/// file is, which is their own instruction about their own disk.
///
/// ## Copying one file is not adopting an installation
///
/// ADR-0032 refuses to copy tens of gigabytes and refuses to absorb somebody else's install
/// tree; both are things Epoch would do *on its own*. This is one file somebody pointed at. The
/// original stays exactly where it was.
pub fn import(path: &str) -> String {
    use epoch_models::generative::{Library, Shelf as Where};

    let from = std::path::Path::new(path.trim());
    if path.trim().is_empty() {
        return "Nothing was typed.".to_owned();
    }
    let read = match epoch_assets::asset::understand(from) {
        Ok(read) => read,
        Err(why) => return why.to_string(),
    };
    let Some(name) = from.file_name().map(|n| n.to_string_lossy().into_owned()) else {
        return "That is not a file.".to_owned();
    };

    let library = Library::here();
    // Which shelf is read from the bytes — never from the folder it came from and never from its
    // name: a file called `pixel_art_final_v3` in a folder called `loras` is two claims and no
    // facts.
    let shelf = library.shelf(Where::for_kind(read.kind));
    if let Err(why) = std::fs::create_dir_all(&shelf) {
        return why.to_string();
    }
    let landed = shelf.join(&name);
    if landed.exists() {
        return format!("{name} is already in the library.");
    }
    // Copied rather than moved: the file is theirs and another program may still be pointing at
    // it. Removing it would be this program deciding that for them.
    if let Err(why) = std::fs::copy(from, &landed) {
        return format!("it could not be copied in: {why}");
    }

    let civitai = epoch_models::civitai::Civitai::new();
    match catalogue::identify(&landed, &[&civitai]) {
        Ok(known) => format!(
            "{name} — {}, {}",
            known.kind.plainly(),
            known.family.plainly()
        ),
        Err(why) => format!("{name} arrived, and its own bytes could not be read: {why}"),
    }
}

/// What is already here, grouped by what it makes.
///
/// ## Why this page needed it too
///
/// The Host's Creations deck lists the same library off the same disk, and it grew a medium
/// filter on 2026-08-30 because a VAE that decodes sound was being offered to somebody drawing a
/// picture. The other surface shows the *catalogue* and never showed the shelves at all — so
/// somebody standing at this machine could install a model and have no way to see what was on
/// it, let alone what any of it makes.
///
/// **Grouped rather than filtered, because this page has `script-src: 'none'`.** A filter would
/// be a query parameter and a round trip; headings need neither, hide nothing, and answer the
/// same question — *what on this machine makes a video*. The reading itself is
/// `generative::held_on`, which both surfaces now share.
///
/// **Unplaced files get their own group and are never dropped.** A file Epoch could not read is
/// still the user's; leaving it out would take somebody's own model off the screen with nothing
/// on screen to explain it.
fn shelves(library: &epoch_models::generative::Library, wanted: &str) -> String {
    use epoch_models::generative;

    // In the order somebody thinks about them, and the last one is the honest leftover.
    const MEDIA: [(&str, &str); 6] = [
        ("picture", "MAKES A PICTURE"),
        ("video", "MAKES A VIDEO"),
        ("sound", "MAKES A SOUND"),
        ("model", "MAKES A 3D MODEL"),
        // Not a shrug: a text encoder feeds a picture graph, a video graph and a sound graph,
        // and the same file sits beside Flux and beside SDXL. It has no medium of its own.
        ("any", "USED WITH ANY MEDIUM"),
        ("", "EPOCH COULD NOT TELL WHAT THESE MAKE"),
    ];

    let everything = generative::everything_held(library);
    let total: usize = everything.iter().map(|(_, held)| held.len()).sum();
    if total == 0 {
        return format!(
            r#"<section class="card">
  <h2>ON THE SHELVES</h2>
  <p class="quiet">Nothing here yet. What you install lands in <code>{root}</code>.</p>
</section>"#,
            root = page::escape(&library.root().display().to_string())
        );
    }

    let mut out = format!(
        r#"<section class="card">
  <h2>ON THE SHELVES</h2>
  <p class="quiet">
    What is on <b>this</b> machine, read from each file's own bytes and never from its name.
  </p>
  <form method="post" action="/creations/shelved" class="row">
    <select name="shelved">{choices}</select>
    <button type="submit">SHOW</button>
  </form>"#,
        // The same five words the Host's dropdown uses, and the same shape this page's search
        // filter already had: a form, because a page with `script-src: 'none'` has one way to
        // offer a choice and it is a good one.
        choices = options(
            &[
                ("", "Everything"),
                ("picture", "Makes a picture"),
                ("video", "Makes a video"),
                ("sound", "Makes a sound"),
                ("model", "Makes a 3D model"),
                ("any", "Used with any medium"),
            ],
            wanted
        ),
    );

    for (medium, heading) in MEDIA {
        // **Narrowed to one when one is chosen, and all of them otherwise.** Grouping was the
        // first shape and the owner asked for the Host's: a dropdown, one list. The headings
        // stay for the *unfiltered* view, where they are the only thing separating four media.
        if !wanted.is_empty() && medium != wanted {
            continue;
        }
        let mut rows = String::new();
        for (shelf, held) in &everything {
            for one in held {
                let mine = match (&one.medium, medium) {
                    (Some(it), want) => it == want,
                    (None, "") => true,
                    (None, _) => false,
                };
                if !mine {
                    continue;
                }
                rows.push_str(&format!(
                    r#"<li><b>{file}</b> <span class="quiet">{kind} · {base} · {size} · {shelf}</span></li>"#,
                    file = page::escape(&one.file),
                    kind = page::escape(&one.kind),
                    base = page::escape(&one.base),
                    size = crate::workshop::gb(one.bytes),
                    shelf = page::escape(shelf.folder()),
                ));
            }
        }
        if rows.is_empty() {
            continue;
        }
        out.push_str(&format!(
            r#"<h3>{heading}</h3><ul class="plain">{rows}</ul>"#,
            heading = heading
        ));
    }
    out.push_str("</section>");
    out
}

/// The page.
pub fn render(shelf: &Shelf, library: &epoch_models::generative::Library) -> String {
    let mut out = String::new();
    // Measured once for the page rather than once per row: `nvidia-smi` is a process, and a page
    // of twenty-four would start twenty-four of them.
    let card = epoch_models::Machine::measure();

    out.push_str(&format!(
        r#"<section class="card">
  <h2>FIND SOMETHING THAT DRAWS</h2>
  <p class="quiet">
    Civitai and Hugging Face, asked together and ranked as one list. What lands here is on
    <b>this</b> machine — the one whose card draws the picture.
  </p>
  {narrowed}
  <form method="post" action="/creations/search" class="row">
    <input name="query" value="{query}" placeholder="watercolour, pixel art, a name…" autofocus>
    <select name="kind">{kinds}</select>
    <select name="order">{orders}</select>
    <select name="base">{bases}</select>
    <select name="makes">{media}</select>
    <label class="chip"><input type="checkbox" name="adult" {adult}> adult</label>
    <button type="submit">SEARCH</button>
  </form>
  <form method="post" action="/creations/import" class="row">
    <input name="path" placeholder="… or the full path to a file you already have">
    <button type="submit">IMPORT</button>
  </form>
  <p class="quiet">Library: <code>{root}</code></p>
  {card}
</section>"#,
        query = page::escape(&shelf.query),
        // Said only while it is true. Only Civitai publishes a base model, so choosing one
        // narrows the search to one site — showing the other unfiltered beside it would be a
        // filter that half-works.
        narrowed = if shelf.base.trim().is_empty() {
            String::new()
        } else {
            r#"<p class="warn">Only Civitai publishes a base model, so while one is chosen it is
    the only site asked.</p>"#
                .to_owned()
        },
        kinds = kinds(&shelf.kind),
        // What each site said, never what the bytes say: nothing here has been downloaded.
        media = options(
            &[
                ("", "Anything it makes"),
                ("picture", "Makes a picture"),
                ("video", "Makes a video"),
                ("sound", "Makes a sound"),
                ("model", "Makes a 3D model"),
            ],
            &shelf.makes
        ),
        orders = options(
            &[("most_downloaded", "Most downloaded"), ("newest", "Newest")],
            &shelf.order
        ),
        // The sites' own words, read from `epoch-models` rather than typed here: the list was
        // measured, and a second copy on a screen would be the second answer that drifts.
        bases = {
            let mut all = vec![("".to_owned(), "Any base model".to_owned())];
            all.extend(
                epoch_models::civitai::BASES
                    .iter()
                    .map(|it| ((*it).to_owned(), (*it).to_owned())),
            );
            options(
                &all.iter()
                    .map(|(v, l)| (v.as_str(), l.as_str()))
                    .collect::<Vec<_>>(),
                &shelf.base,
            )
        },
        adult = if shelf.adult { "checked" } else { "" },
        root = page::escape(&library.root().display().to_string()),
        // Only what was measured. A machine with no readable card says nothing rather than a
        // reassuring sentence about one nobody found.
        card = if card.card().is_empty() {
            String::new()
        } else {
            format!(
                r#"<p class="quiet">This machine: {}</p>"#,
                page::escape(&card.card())
            )
        },
    ));

    // **What is here, before what could be.** A page that only searches leaves somebody
    // installing a second copy of something already on the disk beside them.
    out.push_str(&shelves(library, shelf.shelved.trim()));

    // Beside the results, never instead of them: a busy server telling somebody their style does
    // not exist is the same failure as a cold instrument reading zero.
    for why in &shelf.refused {
        out.push_str(&format!(r#"<p class="warn">{}</p>"#, page::escape(why)));
    }
    if let Some(said) = &shelf.said {
        out.push_str(&format!(r#"<p class="note">{}</p>"#, page::escape(said)));
    }

    /*
        **What has arrived, and a bar only where there is a total.**

        `<progress>` with no `max` is the platform's indeterminate barber's pole, which says
        *something is happening* and nothing about how much. That is a fair reading when the
        source stated no `Content-Length` — and where it did state one, the fraction is real and
        is drawn.

        No script anywhere: this page re-renders every two seconds while a download is in flight,
        which is `<meta http-equiv="refresh">` and is older than the problem.
    */
    if let Some((name, done, total)) = &shelf.fetching {
        let named = if name.trim().is_empty() {
            "Starting".to_owned()
        } else {
            page::escape(name)
        };
        out.push_str(&format!(
            r#"<section class="card">
  <p><b>{named}</b> — {arrived}{of}</p>
  {bar}
</section>"#,
            arrived = crate::workshop::gb(*done),
            of = match total {
                Some(total) if *total > 0 => format!(
                    " of {} · {}%",
                    crate::workshop::gb(*total),
                    done.saturating_mul(100) / total
                ),
                // No total is not zero. It is a source that said nothing about the size, and the
                // honest reading is what has arrived.
                _ => " so far — this source did not say how big it is".to_owned(),
            },
            bar = match total {
                Some(total) if *total > 0 =>
                    format!(r#"<progress max="{total}" value="{done}"></progress>"#),
                _ => r#"<progress></progress>"#.to_owned(),
            },
        ));
    }

    let Some(found) = &shelf.found else {
        return out;
    };
    if found.is_empty() && shelf.refused.is_empty() {
        out.push_str(r#"<p class="quiet">Nothing matched. Both sites answered.</p>"#);
        return out;
    }

    /*
        **Unplaced results stay.** A site that did not say what a thing makes has not said it is
        not this one, and dropping it would hide somebody's answer for a failure of the
        catalogue's — the same rule the shelves follow, one measurement weaker.
    */
    let wanted = shelf.makes.trim();
    let showing: Vec<&Asset> = found
        .iter()
        .filter(|asset| {
            wanted.is_empty()
                || match asset.makes {
                    Some(made) => epoch_models::generative::medium_id(made) == wanted,
                    None => true,
                }
        })
        .collect();
    if showing.is_empty() {
        out.push_str(
            r#"<p class="quiet">Nothing that came back makes that. What a site did not place is
            still shown, so this is what both of them said.</p>"#,
        );
        return out;
    }

    out.push_str(r#"<div class="grid">"#);
    for asset in showing {
        let bytes = asset.principal().map(|file| file.bytes).unwrap_or(0);
        out.push_str(&format!(
            r#"<div class="card">
  {shot}
  <div class="row between"><b>{name}</b><span class="quiet">{from}</span></div>
  <p class="quiet">{kind} · {base}{family} · {size}{fits} · {downloads} downloads{by}</p>
  {unplaced}
  {triggers}
  <div class="row">
    <a class="btn" href="{page}" target="_blank" rel="noreferrer">SOURCE</a>
    <form method="post" action="/creations/install" class="row">
      <input type="hidden" name="source" value="{source}">
      {versions}
      <button type="submit">{action}</button>
    </form>
  </div>
</div>"#,
            // **The picture the source leads with, through this program's own door.**
            //
            // A token rather than the address: `/preview/<token>` is fetched by this program and
            // never by the browser, which is the same bargain the Host makes with `epoch://`.
            // `loading="lazy"` is HTML and needs no script — a page of twenty cards asks only
            // for the ones somebody scrolled to.
            //
            // Not drawn for an asset the source marked adult; that is what the filter above is
            // for, and a picture is a stronger thing to put on screen than a name.
            shot = match (&asset.preview, asset.adult) {
                (Some(url), false) => format!(
                    r#"<img class="shot" loading="lazy" alt="" src="/preview/{}">"#,
                    epoch_models::catalogue::remember_preview(url)
                ),
                _ => String::new(),
            },
            // **Why an unplaced result is in a filtered list**, said on the row that raises
            // the question — and only while a medium is chosen, because with all of them
            // showing there is nothing to explain.
            unplaced = if !wanted.is_empty() && asset.makes.is_none() {
                format!(
                    r#"<p class="warn">{} did not say what this makes — shown under every medium
                    rather than hidden from the right one.</p>"#,
                    asset.source.name()
                )
            } else {
                String::new()
            },
            name = page::escape(&asset.name),
            from = asset.source.name(),
            kind = asset.kind.plainly(),
            base = page::escape(if asset.said_base.is_empty() {
                "base not stated"
            } else {
                &asset.said_base
            }),
            family = match asset.family {
                epoch_assets::asset::Base::Unknown => String::new(),
                other => format!(" ({})", other.plainly()),
            },
            size = weigh(bytes),
            // Nothing said when nobody measured, and one that does not fit is still offered:
            // it is their disk and their decision.
            fits = match (bytes > 0).then(|| card.fits(bytes)).flatten() {
                Some(true) => " · fits",
                Some(false) => " · larger than the free memory",
                None => "",
            },
            downloads = asset.downloads,
            by = match &asset.by {
                Some(who) => format!(" · by {}", page::escape(who)),
                None => String::new(),
            },
            triggers = if asset.triggers.is_empty() {
                String::new()
            } else {
                format!(
                    r#"<p class="quiet">Needs in a prompt: {}</p>"#,
                    page::escape(&asset.triggers.join(", "))
                )
            },
            page = page::escape(&asset.page),
            source = source_id(asset.source),
            // Only where there is something to choose. One version is not a choice, and a
            // dropdown with a single entry is a control that changes nothing — so the id
            // travels hidden instead.
            versions = if asset.versions.len() > 1 {
                format!(
                    r#"<select name="id">{}</select>"#,
                    asset
                        .versions
                        .iter()
                        .map(|version| format!(
                            r#"<option value="{id}">{name} · {base} · {size}</option>"#,
                            id = page::escape(&version.id),
                            name = page::escape(&version.name),
                            base = page::escape(if version.said_base.is_empty() {
                                "base not stated"
                            } else {
                                &version.said_base
                            }),
                            size = weigh(version.bytes),
                        ))
                        .collect::<String>()
                )
            } else {
                format!(
                    r#"<input type="hidden" name="id" value="{}">"#,
                    page::escape(&asset.id)
                )
            },
            // Said before it is pressed rather than after: this program has nowhere to keep a
            // credential, and a button that fails is worse than one that explains.
            // Said before it is pressed rather than after. What is checked is whether this
            // machine holds a key, not whether the file wants one — a machine that has signed
            // in should not be told it cannot.
            action = if asset.files.iter().any(|file| file.needs_key)
                && !epoch_secrets::Store::at(crate::keep::dir())
                    .holds(&format!("catalogue:{}", source_id(asset.source)))
            {
                "NEEDS AN ACCOUNT"
            } else {
                "INSTALL"
            },
        ));
    }
    out.push_str("</div>");

    // Absent, not disabled-looking, when there is nowhere to go: a dead control is one more
    // thing to read.
    if shelf.page > 1 || !shelf.more.is_empty() {
        out.push_str(&format!(
            r#"<div class="pages">
  <span class="quiet">PAGE {page}</span>
  {next}
</div>"#,
            page = shelf.page,
            next = if shelf.more.is_empty() {
                String::new()
            } else {
                r#"<form method="post" action="/creations/more"><button type="submit">NEXT →</button></form>"#
                    .to_owned()
            },
        ));
    }
    out
}

fn kinds(chosen: &str) -> String {
    options(
        &[
            ("lora", "LoRAs"),
            ("checkpoint", "Models"),
            ("vae", "VAEs"),
            ("controlnet", "ControlNets"),
            ("embedding", "Embeddings"),
            ("", "Everything"),
        ],
        chosen,
    )
}

/// One `<select>`'s worth of options, with the chosen one marked.
fn options(all: &[(&str, &str)], chosen: &str) -> String {
    all.iter()
        .map(|(value, label)| {
            format!(
                r#"<option value="{value}"{on}>{label}</option>"#,
                on = if *value == chosen { " selected" } else { "" }
            )
        })
        .collect()
}

/// A file's size, the way somebody reads one.
///
/// Decimal, because that is what every download page says a checkpoint weighs.
fn weigh(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1e9)
    } else if bytes >= 1_000_000 {
        format!("{} MB", bytes / 1_000_000)
    } else if bytes == 0 {
        "size not stated".to_owned()
    } else {
        format!("{} KB", bytes / 1_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shelf_nobody_has_searched_shows_a_form_and_no_verdict() {
        // The difference between *nothing matched* and *nobody asked*. A page that said the
        // first before anybody typed would be answering a question nobody put.
        let out = render(
            &Shelf::default(),
            &epoch_models::generative::Library::at("/somewhere"),
        );
        assert!(out.contains("FIND SOMETHING THAT DRAWS"), "{out}");
        assert!(!out.contains("Nothing matched"), "{out}");
    }

    #[test]
    fn a_source_that_refused_is_named_beside_the_results() {
        let shelf = Shelf {
            found: Some(Vec::new()),
            refused: vec!["Civitai: overloaded right now".to_owned()],
            ..Shelf::default()
        };
        let out = render(&shelf, &epoch_models::generative::Library::at("/somewhere"));
        assert!(out.contains("overloaded right now"), "{out}");
        // And never folded into *nothing matched*, which would be a sentence about a busy
        // server delivered as a sentence about the world.
        assert!(!out.contains("Nothing matched"), "{out}");
    }

    #[test]
    fn a_file_that_needs_an_account_says_so_before_it_is_pressed() {
        // This program has nowhere to keep a credential and does not invent one.
        let asset = Asset {
            sample: None,
            preview: None,
            makes: None,
            id: "civitai:1".to_owned(),
            name: "Something".to_owned(),
            source: epoch_models::catalogue::Source::Civitai,
            kind: epoch_assets::asset::Kind::Lora,
            family: epoch_assets::asset::Base::Sdxl,
            said_base: "SDXL 1.0".to_owned(),
            by: None,
            downloads: 3,
            adult: false,
            triggers: Vec::new(),
            licence: Default::default(),
            files: vec![epoch_models::catalogue::File {
                name: "a.safetensors".to_owned(),
                bytes: 1,
                sha256: None,
                url: String::new(),
                needs_key: true,
            }],
            // One version, so nothing to choose.
            versions: Vec::new(),
            page: "https://civitai.com/models/1".to_owned(),
        };
        let out = render(
            &Shelf {
                found: Some(vec![asset]),
                ..Shelf::default()
            },
            &epoch_models::generative::Library::at("/somewhere"),
        );
        assert!(out.contains("NEEDS AN ACCOUNT"), "{out}");
        assert!(!out.contains(">INSTALL<"), "{out}");
    }

    #[test]
    fn a_model_published_twice_offers_both_and_a_model_published_once_offers_nothing() {
        // The defect this fixes: taking the largest file fetched a 170 MB Z-Image LoRA for a
        // Flux graph. And a dropdown with one entry is a control that changes nothing.
        let mut asset = Asset {
            sample: None,
            preview: None,
            makes: None,
            id: "civitai:1".to_owned(),
            name: "Something".to_owned(),
            source: epoch_models::catalogue::Source::Civitai,
            kind: epoch_assets::asset::Kind::Lora,
            family: epoch_assets::asset::Base::Unknown,
            said_base: "ZImageTurbo".to_owned(),
            by: None,
            downloads: 3,
            adult: false,
            triggers: Vec::new(),
            licence: Default::default(),
            files: vec![epoch_models::catalogue::File {
                name: "a.safetensors".to_owned(),
                bytes: 1,
                sha256: None,
                url: String::new(),
                needs_key: false,
            }],
            versions: vec![
                epoch_models::catalogue::Version {
                    id: "civitai:1#10".to_owned(),
                    name: "ZImageTurbo".to_owned(),
                    said_base: "ZImageTurbo".to_owned(),
                    family: epoch_assets::asset::Base::Unknown,
                    bytes: 170_000_000,
                },
                epoch_models::catalogue::Version {
                    id: "civitai:1#11".to_owned(),
                    name: "Flux".to_owned(),
                    said_base: "Flux.1 D".to_owned(),
                    family: epoch_assets::asset::Base::Flux,
                    bytes: 18_000_000,
                },
            ],
            page: "https://civitai.com/models/1".to_owned(),
        };

        let library = epoch_models::generative::Library::at("/somewhere");
        let out = render(
            &Shelf {
                found: Some(vec![asset.clone()]),
                ..Shelf::default()
            },
            &library,
        );
        assert!(out.contains(r#"<select name="id">"#), "{out}");
        assert!(out.contains("civitai:1#11"), "{out}");

        asset.versions.truncate(1);
        let out = render(
            &Shelf {
                found: Some(vec![asset]),
                ..Shelf::default()
            },
            &library,
        );
        assert!(!out.contains("<select name=\"id\">"), "{out}");
        assert!(out.contains(r#"name="id" value="civitai:1""#), "{out}");
    }

    /// Two autoencoders, written as real safetensors headers, grouped by what they decode.
    ///
    /// **The shapes are the owner's own files**, read on 2026-08-30: a picture VAE ends in a
    /// rank-4 convolution emitting three channels, and an audio one is rank 3 with no rank 4
    /// anywhere. Neither carries a family, which is why the medium is read from the shape.
    ///
    /// And the unreadable one has to survive: it is on his disk, Epoch failed to place it, and
    /// dropping it would take his own model off the screen with nothing saying why.
    #[test]
    fn the_shelves_are_grouped_by_what_each_file_makes() {
        let root = std::env::temp_dir().join("epoch-services-shelves-test");
        let _ = std::fs::remove_dir_all(&root);
        let library = epoch_models::generative::Library::at(&root);
        library.ensure().expect("a library to write into");

        let write = |shelf: epoch_models::generative::Shelf, name: &str, head: &str| {
            let path = library.shelf(shelf).join(name);
            let bytes = head.as_bytes();
            let mut file = (bytes.len() as u64).to_le_bytes().to_vec();
            file.extend_from_slice(bytes);
            std::fs::write(path, file).expect("a file on the shelf");
        };

        write(
            epoch_models::generative::Shelf::Vaes,
            "an_image_ae.safetensors",
            // A standalone VAE is an encoder and a decoder and nothing that draws, so both
            // halves have to be here or `kind_of` never calls it one.
            r#"{"encoder.conv_out.weight":{"dtype":"F16","shape":[32,512,3,3],"data_offsets":[0,0]},
                "decoder.conv_out.weight":{"dtype":"F16","shape":[3,128,3,3],"data_offsets":[0,0]}}"#,
        );
        write(
            epoch_models::generative::Shelf::Vaes,
            "a_sound_ae.safetensors",
            r#"{"encoder.block.0.weight":{"dtype":"F16","shape":[128,32,7],"data_offsets":[0,0]},
                "decoder.block.0.weight":{"dtype":"F16","shape":[2048,32,1],"data_offsets":[0,0]}}"#,
        );
        write(
            epoch_models::generative::Shelf::Vaes,
            "who_knows.safetensors",
            r#"{"mystery.weight":{"dtype":"F16","shape":[8,8],"data_offsets":[0,0]}}"#,
        );

        // Unfiltered, which is what the section shows until somebody chooses.
        let out = shelves(&library, "");
        assert!(out.contains("MAKES A PICTURE"), "{out}");
        assert!(out.contains("MAKES A SOUND"), "{out}");
        assert!(
            out.contains("EPOCH COULD NOT TELL WHAT THESE MAKE"),
            "the unplaced group has to exist when something is unplaced: {out}"
        );
        for file in [
            "an_image_ae.safetensors",
            "a_sound_ae.safetensors",
            "who_knows.safetensors",
        ] {
            assert!(out.contains(file), "{file} was dropped: {out}");
        }
        // And no group nobody can fill.
        assert!(!out.contains("MAKES A VIDEO"), "{out}");

        // **And narrowed to one medium**, which is the shape the owner asked for: one list
        // rather than four headings.
        let only_sound = shelves(&library, "sound");
        assert!(
            only_sound.contains("a_sound_ae.safetensors"),
            "{only_sound}"
        );
        assert!(
            !only_sound.contains("an_image_ae.safetensors"),
            "a picture is not a sound: {only_sound}"
        );
        assert!(
            !only_sound.contains("MAKES A PICTURE"),
            "and the heading it belonged to goes with it: {only_sound}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    /// A bar where the size is known, and bytes where it is not.
    ///
    /// **The denominator is the whole point.** `Content-Length` is a claim most sources make and
    /// some do not, and a percentage invented from nothing is the gauge this codebase keeps
    /// deleting. `<progress>` with no `max` is the platform's indeterminate one, which says
    /// *something is happening* and claims no fraction — which is exactly the reading available.
    #[test]
    fn a_download_shows_a_fraction_only_where_there_is_one() {
        let library = epoch_models::generative::Library::at("/somewhere");

        let known = render(
            &Shelf {
                fetching: Some((
                    "Illustrious-XL".to_owned(),
                    3_450_000_000,
                    Some(6_900_000_000),
                )),
                ..Shelf::default()
            },
            &library,
        );
        assert!(known.contains("Illustrious-XL"), "{known}");
        assert!(
            known.contains("50%"),
            "half of 6.9 GB is a real fraction: {known}"
        );
        assert!(
            known.contains(r#"<progress max="6900000000" value="3450000000">"#),
            "{known}"
        );

        let unknown = render(
            &Shelf {
                fetching: Some(("Something".to_owned(), 3_450_000_000, None)),
                ..Shelf::default()
            },
            &library,
        );
        assert!(
            unknown.contains("did not say how big it is"),
            "no total is a fact to state: {unknown}"
        );
        assert!(!unknown.contains('%'), "and never a percentage: {unknown}");
        assert!(
            unknown.contains("<progress></progress>"),
            "the indeterminate one claims no fraction: {unknown}"
        );

        // And nothing at all when nothing is arriving.
        let quiet = render(&Shelf::default(), &library);
        assert!(!quiet.contains("<progress"), "{quiet}");
    }
}
