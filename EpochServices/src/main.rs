//! EpochServices — one window, and the door behind it (ADR-0029 §9).
//!
//! ## What this program is for
//!
//! A machine with a graphics card and no Epoch on it. It lends the models it has to an Epoch
//! Host on the same network, over a connection that was **paired** rather than merely typed.
//!
//! The alternative it replaces is `OLLAMA_HOST=0.0.0.0`, which makes a machine useful to a Host
//! and to everything else on that network at the same time. **Here Ollama stays on
//! `127.0.0.1`** and this program is the only listener — the one that knows what a secret is.
//!
//! ## Why it may not link the Engine
//!
//! It depends on `epoch-kernel` and nothing else of Epoch's, and `Cargo.toml` is where that is
//! enforced. Linking `epoch-engine` would put Quests, Chronicles, Knowledge and Trust inside a
//! program that must own none of them; the boundary would become a rule somebody has to
//! remember rather than one the build holds.
//!
//! ## One window, and it is a view of the server
//!
//! The server is the product; the window is a frame around a page the server already had to be
//! able to draw. So there is no bundler, no `ui/` folder and no frontend build — the HTML is a
//! Rust string, and the same page answers a browser on a headless box.
//!
//! ## Two doors, because there are two audiences
//!
//! | door | who | where it is bound | what guards it |
//! |---|---|---|---|
//! | [`door::WINDOW_PORT`] | the person at this machine | `127.0.0.1` **only** | it is not on the network |
//! | [`door::PORT`] | the paired Host | every interface, **TLS** | the bearer, and the certificate |
//!
//! They were one listener until the audit, told apart by `if mine` on forty match arms. The
//! property everybody wanted — *the pages are not reachable from the network* — was therefore
//! true by repetition rather than by construction, and it needed the whole file to stay true.
//!
//! And the Host's half is encrypted now. What crosses it is the composed turn: the user's
//! Chronicle, their system prompt, their project's context — and, at pairing, the secret itself.
//! Anything on that network could read all of it. See `epoch_wire::tls` for what a certificate
//! means on a network with no authority to ask.

mod creations;
mod curve;
mod door;
mod here;
mod host;
mod invite;
mod keep;
mod modelspage;
mod page;
mod shelfpage;
mod workshop;

use std::sync::{Arc, Mutex};

fn main() {
    // The listener first. If the port is taken, this machine is already lending — and a second
    // copy silently doing nothing would be worse than saying so.
    let server = match tiny_http::Server::http(("127.0.0.1", door::WINDOW_PORT)) {
        Ok(server) => Arc::new(server),
        Err(err) => {
            /*
                **This used to exit quietly, and that cost an evening.**

                A second copy could not take the port, printed this to a stderr nobody sees —
                it is a windowed application — and exited. The *first* copy kept answering, so
                the new window opened onto the old program: an older build, with routes it did
                not have, answering `no` to a button that had just been added. There was nothing
                on screen to suggest two copies existed.

                So it says so, in a window, and it says which one is answering. Exiting silently
                and exiting loudly cost the same; only one of them is findable.
            */
            complain(&format!(
                "EpochServices could not listen on port {port}.\n\n Another copy is already running and answering on that port — most likely an older version. Quit it first: this one has stopped rather than open a window onto somebody else's program.\n\n({err})",
                port = door::WINDOW_PORT
            ));
            std::process::exit(1);
        }
    };

    let state = Arc::new(Mutex::new(keep::read()));
    // The code this machine is currently showing, if any. It lives here rather than in `Bench`
    // because it belongs to the Connect tab and to the network-facing route that redeems it.
    let showing: Arc<Mutex<Option<invite::Code>>> = Arc::new(Mutex::new(None));
    // What the Workshop is doing, across requests. A page with no script cannot hold anything
    // itself, so the last thing weighed and the download in progress live here — which is also
    // what lets the page refresh itself and show the same thing it showed a moment ago.
    let bench = Arc::new(Mutex::new(Bench::default()));
    let serving = Arc::clone(&server);
    let held = Arc::clone(&state);
    let benched = Arc::clone(&bench);
    let shown = Arc::clone(&showing);
    std::thread::spawn(move || answer(&serving, &held, &benched, &shown));

    /*
        **The Host's door, and it is the one that has to be right.**

        Opened after the window's, and its failure is reported rather than fatal: a machine whose
        network port is taken can still be *looked at* by the person sitting in front of it, and a
        window that says why is worth more than a program that exits. The page says so — a machine
        that cannot be lent must not look like one that can.

        The identity is made once and kept beside the bond, because the fingerprint is what the
        Host pinned: a fresh certificate every start would break every pairing silently.
    */
    let outward = epoch_wire::tls::identity(&keep::dir()).and_then(|identity| {
        // **Bound on this thread**, so a taken port is a sentence on the page rather than a
        // failure reported to a thread nobody is reading. It also removes a race that cost two
        // tests on the other operating system: a caller told *the door is open* while the
        // listener was still being made answers `connection refused` to whoever knocks first.
        let listener = epoch_wire::wire::open(door::PORT)?;
        let held = Arc::clone(&state);
        let shown = Arc::clone(&showing);
        let never = Arc::new(std::sync::atomic::AtomicBool::new(false));
        std::thread::spawn(move || {
            // The same split the pages already make, one door over: a turn carries base64
            // images and everything else on this door is a code, a name or nothing at all.
            // `/enrol` especially — it is reachable by anything on the network, by design.
            let ceiling = std::sync::Arc::new(|path: &str| match path {
                "/ask" | "/show" | "/easel/draw" => ASK_LIMIT,
                _ => epoch_wire::wire::MODEST,
            });
            let _ =
                epoch_wire::wire::answer_on(listener, &identity, never, ceiling, move |asked| {
                    host::answer(asked, &held, &shown)
                });
        });
        Ok(())
    });
    if let Err(why) = &outward {
        if let Ok(mut held) = bench.lock() {
            held.connect = Some(format!(
                "This machine cannot be lent: {why} Everything on this page still works."
            ));
        }
    }

    tauri::Builder::default()
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("EpochServices could not open its window");
}

/// Say something the user must see, on a machine where stderr goes nowhere.
///
/// A windowed program's standard error is read by nobody. This is the one message that has to
/// arrive before there is a window to put it in — so it uses whatever the operating system will
/// always show, and falls back to stderr when even that is unavailable.
fn complain(said: &str) {
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display dialog {:?} with title \"EpochServices\" buttons {{\"OK\"}} with icon caution",
            said
        );
        let shown = std::process::Command::new("osascript")
            .args(["-e", &script])
            .status();
        if shown.is_ok() {
            return;
        }
    }
    #[cfg(windows)]
    {
        // `mshta` is present on every Windows and needs no crate. The message is passed through
        // a JSON-encoded literal so a quote in it cannot end the script.
        let encoded = serde_json::to_string(said).unwrap_or_else(|_| "\"error\"".to_owned());
        let script = format!(
            "javascript:var m={encoded};new ActiveXObject('WScript.Shell')             .Popup(m,0,'EpochServices',48);close()"
        );
        if std::process::Command::new("mshta")
            .arg(script)
            .status()
            .is_ok()
        {
            return;
        }
    }
    eprintln!("{said}");
}

/// What the Workshop is holding between requests.
///
/// A page with `script-src: 'none'` keeps nothing of its own, so the last verdict and the
/// download in progress live on this side. That is not a workaround: it means a refresh shows
/// what was already true rather than re-asking the network every two seconds.
#[derive(Default)]
struct Bench {
    /// What was typed into the weighing field, kept so the field is not emptied by an answer.
    asked: String,
    /// The last verdict, or the reason there is none.
    weighed: Option<Result<epoch_models::Offer, String>>,
    /// What is downloading right now. `None` when nothing is.
    pulling: Option<String>,
    /// The curve being mapped right now, as a line to draw. `None` when none is.
    ///
    /// **Its own field and not `pulling`.** A download and a search are both long and both
    /// report a line, and sharing the field would let one clear the other's message - the
    /// Models tab and the Workshop tab are different pages and a person may leave one running
    /// while reading the other.
    measuring: Option<String>,
    /// How that download ended, once it has.
    said: Option<String>,
    /// What was typed into the search field, kept so an answer does not empty it.
    query: String,
    /// The last page of Hugging Face results, plus everything before it.
    found: Vec<epoch_models::Offer>,
    /// The cursor for the next page, when Hugging Face said there is one.
    more: Option<String>,
    /// Which facets the search asked for, kept so the chips stay pressed after an answer.
    wanted: Vec<epoch_models::Facet>,
    /// Which page of the results is being read.
    ///
    /// **Paged here rather than fetched per page.** Hugging Face pages by cursor and there is no
    /// way back to an earlier one, so everything fetched is kept and this is a window onto it —
    /// which is what makes a `‹ 1 2 3 ›` row possible at all with no script.
    page: usize,
    /// What this machine can run, once somebody has asked. `None` is *nobody pressed it*, which
    /// the page says differently from *nothing fits*.
    fits: Option<Result<Vec<epoch_models::Suited>, String>>,
    /// Which repository is open, if any. One at a time: two quantisation tables at once is a
    /// panel nobody can read.
    opened: Option<String>,
    /// What that repository publishes, or the reason it could not be read.
    variants: Option<Result<Vec<epoch_models::Group>, String>>,
    /// Which model REMOVE has been pressed once for, if any.
    ///
    /// **One at a time, and never armed by default.** A page where every REMOVE was live is a
    /// page where one misplaced click is a nine-gigabyte download.
    removing: Option<String>,
    /// What the last button on the Models tab said.
    models_said: Option<String>,
    /// A file being fetched with `hf`, and how the last one ended.
    filing: Option<String>,
    filed: Option<String>,
    /// Ollama's featured list, read once. It is a network call, and it does not change while
    /// somebody is looking at it.
    shelf: Vec<epoch_models::Offer>,
    shelf_read: bool,
    /// The Creations Workshop's own state. Its own struct because it is its own shelf, and
    /// mixing it into the model fields is how one page's search box empties another's.
    creations: creations::Shelf,
    /// What the Connect tab has to say about the last thing that happened there.
    ///
    /// It lives here because a redirect means the page that reports an action is a *different*
    /// request from the one that performed it. Taken rather than read, so a notice belongs to
    /// the action that produced it and not to every page after it.
    connect: Option<String>,
}

/// The most bytes a form from the page on this machine may carry.
///
/// Everything the person here posts is a handful of fields they typed. A megabyte is already
/// three orders of magnitude more than any of them and still nothing to hold.
const FORM_LIMIT: usize = 1024 * 1024;

/// The most bytes a paired Host may send in one turn.
///
/// A composed turn carries the whole Chronicle, a system prompt, tool declarations and — since
/// sight — base64 images. Sixty-four megabytes is generous for that and finite, which is the
/// property that was missing: `read_to_string` had no ceiling at all, so one caller could ask
/// this machine to hold as much as it cared to send.
const ASK_LIMIT: usize = 64 * 1024 * 1024;

/// How many requests may be in the building at once.
///
/// **Not the same number as how many may work at once.** One at a time is still the rule for
/// the work — this is a machine with one card, and a second simultaneous turn would be a
/// second model loading. What this bounds is the *threads*, and it exists because reading a
/// body is a blocking read on somebody else's socket: a caller that opens a connection,
/// promises a body and sends it one byte an hour used to hold the only loop this program had.
/// It now holds one worker, and past this it is told the door is busy.
const IN_FLIGHT: usize = 32;

/// Answer everything, forever.
///
/// **Accepting and working are different jobs, and they were the same one.** Every request was
/// read and served on this thread, so a slow body read stalled the door for everybody — the
/// person at this machine included. Each request now gets a worker, bounded by [`IN_FLIGHT`],
/// and the workers queue on one lock before doing anything: the serialisation this program
/// wants is *of the work*, not of the reading.
fn answer(
    server: &tiny_http::Server,
    state: &Arc<Mutex<keep::Bond>>,
    bench: &Arc<Mutex<Bench>>,
    showing: &Arc<Mutex<Option<invite::Code>>>,
) {
    // One at a time on purpose. This is a door and a proxy, not a server farm: a second
    // simultaneous turn would be a second model loading on a machine that has one card.
    let work = Arc::new(Mutex::new(()));
    let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    for request in server.incoming_requests() {
        if in_flight.load(std::sync::atomic::Ordering::Relaxed) >= IN_FLIGHT {
            // Said rather than dropped. A closed connection is indistinguishable from a machine
            // that went away, and the person here would be debugging their network.
            let _ = request.respond(tiny_http::Response::from_string("busy").with_status_code(503));
            continue;
        }
        in_flight.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let state = Arc::clone(state);
        let bench = Arc::clone(bench);
        let showing = Arc::clone(showing);
        let work = Arc::clone(&work);
        let counted = Arc::clone(&in_flight);
        let started = std::thread::Builder::new().spawn(move || {
            serve(request, &state, &bench, &showing, &work);
            counted.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        });
        if started.is_err() {
            in_flight.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

/// Read one request, wait for the door, answer it.
///
/// The body is read **before** the work lock is taken, which is the whole point of the split: a
/// caller trickling bytes waits on its own thread and nothing else waits on it.
fn serve(
    mut request: tiny_http::Request,
    state: &Mutex<keep::Bond>,
    bench: &Arc<Mutex<Bench>>,
    showing: &Mutex<Option<invite::Code>>,
    work: &Mutex<()>,
) {
    {
        let bond = state.lock().map(|held| held.clone()).unwrap_or_default();
        let mine = door::from_our_window(&request);
        let url = request.url().split('?').next().unwrap_or("/").to_owned();
        let method = request.method().as_str().to_owned();

        // **The body, read here and once, with a ceiling.** Every route used to read it for
        // itself with `read_to_string`, which has no limit — so the size of what this machine
        // holds was the caller's decision. The ceiling is the Host's turn limit only for the
        // route that carries a turn; everything else on this door is a form somebody typed.
        let ceiling = if url == "/ask" || url == "/easel/draw" || url == "/show" {
            ASK_LIMIT
        } else {
            FORM_LIMIT
        };
        let sent = match read_bounded(&mut request, ceiling) {
            Ok(sent) => sent,
            Err(()) => {
                let _ = request
                    .respond(tiny_http::Response::from_string("too large").with_status_code(413));
                return;
            }
        };

        // `going` is a redirect target, and it is the fix for a defect worth naming.
        //
        // **Every form used to answer with a page**, which left the browser standing on the URL
        // it had posted to — `/pull`. The meta refresh that reports a download then reloaded
        // *that* URL with GET, `GET /pull` is not a route, and the one refusal this program
        // gives came back: `no`. The download had already started and succeeded; what the user
        // saw was the word no.
        //
        // POST/Redirect/GET is the answer to that, and to two others it happened to fix:
        // reloading no longer re-submits the form, and the back button no longer repeats an
        // action.
        /*
            **A preview is bytes, and every other answer here is a string.**

            Answered before the dispatcher rather than inside it: `Response::from_string` is what
            the rest of this loop builds, and widening that tuple to carry an optional body of
            bytes would touch forty arms to serve one. It also wants the opposite cache header
            from every page — a page is live state and must never be kept; a preview is named
            after the hash of the address it came from and cannot come to mean a second picture.

            The same door the Host opens as `epoch://preview/<token>`, for the same reason: the
            page hands over a token, this decides what it stands for, and the browser never
            addresses Civitai.
        */
        if mine && method.as_str() == "GET" && url.starts_with("/preview/") {
            let token = url.trim_start_matches("/preview/").to_owned();
            let answer = match epoch_models::catalogue::preview_bytes(&token, &previews_dir()) {
                Some(bytes) => tiny_http::Response::from_data(bytes)
                    .with_status_code(200)
                    .with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/jpeg"[..])
                            .expect("a content type is a valid header"),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes(
                            &b"Cache-Control"[..],
                            &b"public, max-age=31536000, immutable"[..],
                        )
                        .expect("a cache directive is a valid header"),
                    ),
                None => tiny_http::Response::from_data(Vec::new()).with_status_code(404),
            };
            let _ = request.respond(answer);
            return;
        }

        // **One at a time starts here**, and not a line earlier. Everything above is reading
        // from a socket the caller controls; everything below touches this machine.
        let _turn = match work.lock() {
            Ok(held) => held,
            Err(poisoned) => poisoned.into_inner(),
        };

        let (status, kind, body, going) = match (method.as_str(), url.as_str()) {
            // ---- the person at this machine. Loopback only, no bearer, ever. ----
            ("GET", "/") if mine => {
                // The notice, if the last action left one, and then cleared: it belongs to the
                // action that produced it, not to every page after it.
                let said = bench.lock().ok().and_then(|mut held| held.connect.take());
                // What is on screen, and how long it has left. Read rather than kept: a code
                // that ran out while nobody was looking must not still be shown as an offer.
                let code = showing
                    .lock()
                    .ok()
                    .and_then(|held| held.as_ref().filter(|c| !c.expired()).cloned());
                (
                    200,
                    "text/html; charset=utf-8",
                    page::render(
                        &bond,
                        &here::measure(),
                        &here::address(door::PORT),
                        said.as_deref(),
                        code.as_ref().map(|c| (c.as_str(), c.left())),
                    ),
                    None,
                )
            }
            ("POST", "/pair") if mine => {
                let form = read_form(&sent);
                let said = door::redeem(
                    form.get("host").map(String::as_str).unwrap_or_default(),
                    form.get("code").map(String::as_str).unwrap_or_default(),
                );
                match said {
                    Ok(bond) => {
                        if let Ok(mut held) = state.lock() {
                            *held = bond;
                        }
                    }
                    Err(why) => {
                        if let Ok(mut held) = bench.lock() {
                            held.connect = Some(why);
                        }
                    }
                }
                see_other("/")
            }
            // Show a code, or stop showing one. The person at this machine, and nobody else.
            ("POST", "/invite") if mine => {
                if let Ok(mut held) = showing.lock() {
                    *held = Some(invite::Code::fresh());
                }
                see_other("/")
            }
            ("POST", "/uninvite") if mine => {
                if let Ok(mut held) = showing.lock() {
                    *held = None;
                }
                see_other("/")
            }

            ("POST", "/unpair") if mine => {
                let _ = keep::forget();
                if let Ok(mut held) = state.lock() {
                    *held = keep::Bond::default();
                }
                if let Ok(mut held) = bench.lock() {
                    held.connect = Some(
                        "Disconnected. This machine lends nothing until it is paired again."
                            .to_owned(),
                    );
                }
                see_other("/")
            }

            // ---- the Models Workshop. Loopback only, like the rest of the person's surface.
            //
            // It exists because Epoch's Workshop measures **Epoch's** machine. The machine that
            // has to hold the weights is this one, and a confident verdict about a different
            // computer is worse than no verdict: two models were pulled onto a 16 GB MacBook
            // that could not hold either, and nothing said a word.
            ("GET", "/workshop") if mine => workshop_page(bench),
            // ---- what is actually on this machine. Loopback only, like the rest of it.
            ("GET", "/models") if mine => models_page(bench),
            ("POST", "/models/ask") if mine => {
                let form = read_form(&sent);
                if let Ok(mut held) = bench.lock() {
                    held.removing = form.get("path").cloned();
                    held.models_said = None;
                }
                see_other("/models")
            }
            /*
                **The search runs here, on this machine's card.**

                A curve is kept against *model + card + build*, so one taken on the Host is an
                answer about the Host. This page used to say the Host's Workshop could map it,
                which was false in the direction that sends somebody to the wrong place.

                Started on a thread and reported by a line, exactly as a download is: it costs
                minutes of this graphics card, and a request held open for that long is a page
                that says nothing until it is over.
            */
            ("POST", "/models/measure") if mine => {
                let form = read_form(&sent);
                match (form.get("name"), form.get("on")) {
                    (Some(name), Some(on)) => {
                        start_measuring(Arc::clone(bench), name.clone(), on.clone())
                    }
                    _ => {
                        if let Ok(mut held) = bench.lock() {
                            held.models_said = Some("nothing was named".to_owned());
                        }
                    }
                }
                see_other("/models")
            }
            ("POST", "/models/keep") if mine => {
                if let Ok(mut held) = bench.lock() {
                    held.removing = None;
                }
                see_other("/models")
            }
            ("POST", "/models/remove") if mine => {
                let form = read_form(&sent);
                let said = match form.get("path") {
                    // **Only what this machine reported holding.** The path is matched against
                    // the same list the page was drawn from rather than trusted, so the one
                    // destructive route here can only be pointed at something it listed. A
                    // delete that accepts an arbitrary path is a delete somebody can point at
                    // anything — and this one answers over a socket.
                    Some(path) => epoch_models::runtimes::everything_here(&[models_dir()])
                        .into_iter()
                        .find(|one| one.path == *path)
                        .ok_or_else(|| {
                            "that is not a model this machine reported holding".to_owned()
                        })
                        .and_then(|one| epoch_models::runtimes::remove(&one, &shelf_dir())),
                    None => Err("nothing was named".to_owned()),
                };
                if let Ok(mut held) = bench.lock() {
                    held.removing = None;
                    held.models_said = Some(match said {
                        Ok(good) => good,
                        Err(why) => why,
                    });
                }
                see_other("/models")
            }
            // Choosing from a curve that already exists costs nothing — unlike the search
            // that produced it, which is the Host's to run on the Host's own card.
            ("POST", "/models/recipe") if mine => {
                let form = read_form(&sent);
                let said = match (form.get("path"), form.get("context")) {
                    (Some(path), Some(context)) => match context.parse::<u32>() {
                        Ok(context) => epoch_models::deck::choose(
                            &epoch_models::runtimes::everything_here(&[models_dir()]),
                            path,
                            context,
                        ),
                        Err(_) => Err("that is not a context size".to_owned()),
                    },
                    _ => Err("nothing was named".to_owned()),
                };
                if let Ok(mut held) = bench.lock() {
                    held.models_said = Some(match said {
                        Ok(said) => said,
                        Err(why) => why,
                    });
                }
                see_other("/models")
            }
            ("POST", "/models/time") if mine => {
                let form = read_form(&sent);
                let said = match (form.get("name"), form.get("on")) {
                    (Some(name), Some(on)) => time_one(name, on),
                    // The form always carries both; a request without them is not this page.
                    _ => Err("nothing was named".to_owned()),
                };
                if let Ok(mut held) = bench.lock() {
                    held.models_said = Some(match said {
                        Ok(good) => good,
                        Err(why) => why,
                    });
                }
                see_other("/models")
            }

            ("GET", "/creations") if mine => creations_page(bench),
            ("POST", "/creations/search") if mine => {
                let form = read_form(&sent);
                if let Ok(mut held) = bench.lock() {
                    held.creations.query = form.get("query").cloned().unwrap_or_default();
                    held.creations.kind = form.get("kind").cloned().unwrap_or_default();
                    held.creations.adult = form.contains_key("adult");
                    held.creations.order = form.get("order").cloned().unwrap_or_default();
                    held.creations.base = form.get("base").cloned().unwrap_or_default();
                    // Kept like every other field on this form, so an answer does not reset the
                    // control that produced it.
                    held.creations.makes = form.get("makes").cloned().unwrap_or_default();
                    held.creations.page = 1;
                    held.creations.said = None;
                    let mut shelf = std::mem::take(&mut held.creations);
                    drop(held);
                    creations::look(&mut shelf, Vec::new());
                    if let Ok(mut held) = bench.lock() {
                        held.creations = shelf;
                    }
                }
                see_other("/creations")
            }
            ("POST", "/creations/more") if mine => {
                if let Ok(mut held) = bench.lock() {
                    let mut shelf = std::mem::take(&mut held.creations);
                    drop(held);
                    let more = std::mem::take(&mut shelf.more);
                    shelf.page += 1;
                    creations::look(&mut shelf, more);
                    if let Ok(mut held) = bench.lock() {
                        held.creations = shelf;
                    }
                }
                see_other("/creations")
            }
            // Which medium the shelves list is narrowed to. Its own route because it is its
            // own control: the search asks two websites and this is what is on the disk.
            ("POST", "/creations/shelved") if mine => {
                let form = read_form(&sent);
                if let Ok(mut held) = bench.lock() {
                    held.creations.shelved = form.get("shelved").cloned().unwrap_or_default();
                }
                see_other("/creations")
            }
            ("POST", "/creations/import") if mine => {
                let form = read_form(&sent);
                let said = creations::import(&form.get("path").cloned().unwrap_or_default());
                if let Ok(mut held) = bench.lock() {
                    held.creations.said = Some(said);
                }
                see_other("/creations")
            }
            ("POST", "/creations/install") if mine => {
                let form = read_form(&sent);
                let source = form.get("source").cloned().unwrap_or_default();
                let id = form.get("id").cloned().unwrap_or_default();
                /*
                    **It ran on this thread, and the reason it did is worth keeping.**

                    *A spinner over a background thread would be a second thing to keep true* —
                    correct on the day it was written, when nothing here had one. The Workshop
                    tab has had one ever since `start_pulling`: a thread, a line of state, and a
                    `<meta http-equiv="refresh">` that redraws it. So the second thing to keep
                    true already exists and is already kept.

                    What blocking cost is what the owner asked about: six gigabytes with the
                    request held open and nothing on screen until it finished, on a page whose
                    only way to say anything is to be re-rendered.
                */
                start_fetching(Arc::clone(bench), source, id);
                see_other("/creations")
            }
            // A page of requests and one per repository, so it is a button rather than
            // something that happens when the tab opens.
            ("POST", "/fits") if mine => {
                let found =
                    epoch_models::deck::what_fits(&epoch_models::runtimes::everything_here(&[
                        models_dir(),
                    ]));
                if let Ok(mut held) = bench.lock() {
                    held.fits = Some(found);
                }
                see_other("/shelf")
            }
            ("POST", "/weigh") if mine => {
                let form = read_form(&sent);
                let asked = form.get("model").cloned().unwrap_or_default();
                let machine = epoch_models::Machine::measure();
                let weighed =
                    (!asked.trim().is_empty()).then(|| epoch_models::weigh(asked.trim(), &machine));
                if let Ok(mut held) = bench.lock() {
                    held.asked = asked;
                    held.weighed = weighed;
                    held.said = None;
                }
                see_other("/workshop")
            }
            ("POST", "/search") if mine => {
                let form = read_form(&sent);
                let cursor = form.get("cursor").cloned().unwrap_or_default();
                let installed = here::models();

                // A cursor means "more of what is already shown"; a query means a new list.
                // Appending rather than replacing, because Hugging Face pages by cursor and
                // there is no way back to an earlier page — replacing would make MORE a one-way
                // door that loses what somebody was reading.
                // **One checkbox per facet, named for itself.** A checkbox group shares a name
                // and repeats it, and `read_form` is a map — the second `facet=` would replace
                // the first. Distinct names is the fix that does not require a second parser.
                let wanted: Vec<epoch_models::Facet> = epoch_models::Facet::ALL
                    .into_iter()
                    .filter(|facet| form.contains_key(&format!("facet_{}", facet.id())))
                    .collect();

                let answer = if !cursor.is_empty() {
                    // The chips that produced this list still apply to the next page of it.
                    let wanted = bench
                        .lock()
                        .map(|held| held.wanted.clone())
                        .unwrap_or_default();
                    epoch_models::search_more(&cursor, &wanted, &installed)
                } else {
                    let query = form.get("query").cloned().unwrap_or_default();
                    if let Ok(mut held) = bench.lock() {
                        held.query = query.clone();
                        held.found.clear();
                        held.wanted = wanted.clone();
                        // A new list starts at its first page. Staying on page three of a list
                        // that no longer exists is a blank panel with a pager under it.
                        held.page = 0;
                        held.opened = None;
                        held.variants = None;
                    }
                    epoch_models::search(&query, &wanted, &installed)
                };

                if let Ok(mut held) = bench.lock() {
                    match answer {
                        Ok(found) => {
                            held.found.extend(found.offers);
                            held.more = found.more;
                            held.said = None;
                        }
                        // Hugging Face's own words. A search that failed and said nothing would
                        // look exactly like a search that found nothing.
                        Err(why) => {
                            held.more = None;
                            held.said = Some(why);
                        }
                    }
                }
                see_other("/workshop")
            }
            ("GET", "/services") if mine => {
                // The notice belongs to the action that produced it, and is taken rather than
                // read, so it belongs to that action alone rather than to every page after it.
                let said = bench.lock().ok().and_then(|mut held| held.connect.take());
                (
                    200,
                    "text/html; charset=utf-8",
                    // Measured on every open rather than at startup: somebody can install or
                    // start a runtime while this window is up, and a reading taken once would
                    // keep saying otherwise for as long as the program runs.
                    page::frame(page::Tab::Services, &page::services(said.as_deref()), None),
                    None,
                )
            }
            ("GET", "/settings") if mine => {
                // The notice belongs to the action that produced it, and taken rather than read
                // so it belongs to that action alone rather than to every page after it.
                let said = bench.lock().ok().and_then(|mut held| held.connect.take());
                (
                    200,
                    "text/html; charset=utf-8",
                    page::frame(
                        page::Tab::Settings,
                        // Measured on every open rather than at startup: somebody can install `hf`
                        // while this window is up, and a reading taken once would keep saying
                        // otherwise for as long as the program runs.
                        &page::settings(
                            &epoch_models::hf::cli(),
                            &models_dir().display().to_string(),
                            // Names and a yes/no. There is nowhere here a stored value could
                            // travel back through, which is the guarantee rather than the care.
                            &[(
                                "civitai",
                                "Civitai",
                                epoch_secrets::Store::at(keep::dir()).holds("catalogue:civitai"),
                            )],
                            said.as_deref(),
                        ),
                        None,
                    ),
                    None,
                )
            }
            ("POST", "/settings/key") if mine => {
                let form = read_form(&sent);
                let source = form.get("source").cloned().unwrap_or_default();
                let store = epoch_secrets::Store::at(keep::dir());
                let name = format!("catalogue:{}", source.trim());
                let said = if form.contains_key("forget") {
                    match store.forget(&name) {
                        Ok(()) => format!("{source}'s key was removed."),
                        Err(why) => why,
                    }
                } else {
                    // Trimmed here because every site's copy button takes a newline with it, and
                    // a key with one fails as a 401 that looks like a wrong key.
                    let value = form.get("value").cloned().unwrap_or_default();
                    match store.put(&name, value.trim()) {
                        Ok(()) if value.trim().is_empty() => "Nothing was typed.".to_owned(),
                        Ok(()) => format!(
                            "{source}'s key is kept by this machine's own credential store."
                        ),
                        Err(why) => why,
                    }
                };
                if let Ok(mut held) = bench.lock() {
                    held.connect = Some(said);
                }
                see_other("/settings")
            }
            ("POST", "/start") if mine => {
                // The same door as installing: this machine's own terminal, so the server
                // outlives the button that started it.
                let form = read_form(&sent);
                let wanted = form.get("runtime").cloned().unwrap_or_default();
                let said = epoch_models::runtimes::Runtime::ALL
                    .into_iter()
                    .find(|r| r.id() == wanted.trim())
                    .ok_or_else(|| format!("'{wanted}' is not a runtime this build knows"))
                    .and_then(|runtime| {
                        let seen = epoch_models::runtimes::look_for(runtime);
                        let command = seen.start.ok_or_else(|| {
                            format!(
                                "{} is not on this machine, so there is nothing to start.",
                                seen.name
                            )
                        })?;
                        epoch_models::runtimes::open_in_terminal(
                            &format!("EpochServices - {}", runtime.name()),
                            &command,
                        )
                        .map(|()| {
                            format!(
                                "A terminal opened on {}. Once it says it is listening, open this tab again.",
                                runtime.name()
                            )
                        })
                    });
                if let Ok(mut held) = bench.lock() {
                    held.connect = Some(match said {
                        Ok(good) => good,
                        Err(why) => why,
                    });
                }
                see_other("/services")
            }
            ("POST", "/shelf") if mine => {
                // **Every model this machine has, handed to the runtime that was asked about.**
                //
                // A lent machine is a machine somebody keeps models on, and until this existed
                // only Ollama could see them: the MacBook answered with 8 models through Ollama
                // and 0 through llama.cpp, on the same disk. The other two runtimes read files,
                // and Ollama's store holds blobs named by digest with no extension.
                //
                // The policy is `epoch-models`', identical to the Host's — two copies of *what
                // counts as a model this machine has* is how the two surfaces come to disagree
                // about what can be lent.
                let form = read_form(&sent);
                let wanted = form.get("runtime").cloned().unwrap_or_default();
                let held = epoch_models::runtimes::everything_here(&[models_dir()]);
                let said = match wanted.trim() {
                    "llama_cpp" => epoch_models::runtimes::serve_everything(
                        &shelf_dir(),
                        &held,
                        // **The conservative loadout, and this machine does not search.** A
                        // lent machine is somebody else's computer: spending four minutes of
                        // its graphics card measuring a model, unasked, is not a thing to do
                        // to a machine that agreed to answer turns. So it takes the safe half
                        // of every trade - three percent on a model that did not need the
                        // compressed cache, rather than two thirds on one that did.
                        //
                        // The policy is `epoch-models`' either way, so the two surfaces cannot
                        // come to disagree about what a preset should say.
                        &|_: &epoch_models::runtimes::Weights| {
                            epoch_models::loadout::conservative(
                                (epoch_models::loadout::A_REAL_TURN
                                    + epoch_models::loadout::ROOM_TO_ANSWER)
                                    .next_power_of_two(),
                            )
                        },
                        // **Nothing told, and that is the same reasoning one comment up.** A lent
                        // machine runs on llama.cpp's own defaults: flash attention and
                        // speculative decoding are choices about somebody else's computer, and
                        // the person sitting at it is the one entitled to make them.
                        &|_: &epoch_models::runtimes::Weights| {
                            epoch_models::tuning::Tuning::default()
                        },
                        "EpochServices - llama.cpp serving",
                    ),
                    "lm_studio" => epoch_models::runtimes::lend_everything(&held),
                    other => Err(format!("'{other}' has no shelf to stock")),
                };
                if let Ok(mut held) = bench.lock() {
                    held.connect = Some(match said {
                        Ok(good) => good,
                        Err(why) => why,
                    });
                }
                see_other("/services")
            }
            ("POST", "/install") if mine => {
                // Offered, never done: this opens the machine's own terminal on the command and
                // steps back. Nothing here reports success — surveying again is how you find
                // out, which is what reopening the tab does.
                let form = read_form(&sent);
                let wanted = form.get("runtime").cloned().unwrap_or_default();
                let said = epoch_models::runtimes::Runtime::ALL
                    .into_iter()
                    .find(|r| r.id() == wanted.trim())
                    .ok_or_else(|| format!("'{wanted}' is not a runtime this build knows"))
                    .and_then(|runtime| {
                        epoch_models::runtimes::open_in_terminal(
                            &format!("EpochServices - install {}", runtime.name()),
                            runtime.install_command(),
                        )
                        .map(|()| {
                            format!(
                                "A terminal opened on `{}`. When it finishes, open this tab again.",
                                runtime.install_command()
                            )
                        })
                    });
                if let Ok(mut held) = bench.lock() {
                    held.connect = Some(match said {
                        Ok(good) => good,
                        Err(why) => why,
                    });
                }
                see_other("/services")
            }
            // **The same two doors as a runtime, for the program that makes pictures.**
            //
            // Its own routes rather than a `kind` on the existing ones: what is being named is a
            // different closed set, and a single route taking either would need to decide what
            // `runtime=comfyui` means. Nothing there, and a 404 that reads as a bug.
            ("POST", "/studio-install") if mine => {
                let form = read_form(&sent);
                let said = studio_named(form.get("studio")).and_then(|studio| {
                    // Offered, never done — the licence, the elevation prompt and the output all
                    // belong to the person reading them, in their own terminal.
                    epoch_models::runtimes::open_in_terminal(
                        &format!("EpochServices - install {}", studio.name()),
                        studio.install_command(),
                    )
                    .map(|()| {
                        format!(
                            "A terminal opened on `{}`. When it finishes, open this tab again.",
                            studio.install_command()
                        )
                    })
                });
                say(bench, said);
                see_other("/services")
            }
            // **Epoch can stop what it starts.** ComfyUI answers 405 to every shutdown route
            // there is (`/shutdown`, `/api/shutdown`, `/exit`, `/quit` — asked, all four), so
            // the only honest option is the process, identified by the install folder in its
            // command line rather than by the most shared name on a machine.
            ("POST", "/studio-stop") if mine => {
                let form = read_form(&sent);
                say(
                    bench,
                    studio_named(form.get("studio")).and_then(epoch_models::studio::stop_server),
                );
                see_other("/services")
            }
            ("POST", "/studio-start") if mine => {
                let form = read_form(&sent);
                let said = studio_named(form.get("studio")).and_then(|studio| {
                    let seen = epoch_models::studio::look_for(studio);
                    let command = seen.start.ok_or_else(|| {
                        format!(
                            "{} is not on this machine, so there is nothing to open.",
                            seen.name
                        )
                    })?;
                    epoch_models::runtimes::open_in_terminal(
                        &format!("EpochServices - {}", studio.name()),
                        &command,
                    )
                    .map(|()| {
                        // **Which of the two happened is measured, not assumed.** `Easel` already
                        // carries it: `starts_server` is true when Epoch found `main.py` and a
                        // Python to run it with, and false when all it can press is the desktop
                        // application's own front door. This branch used to say the front-door
                        // sentence either way — so on a machine where Epoch really does start the
                        // server it sent somebody looking for an instance list that never opens.
                        // The Host's panel has always read the same two fields; this is the
                        // second surface catching up.
                        if seen.starts_server {
                            format!(
                                "A terminal opened on {}'s own server. {} Once it says it is listening, open this tab again.",
                                seen.name, seen.first_run
                            )
                        } else {
                            format!(
                                "{} is opening. {} Then open this tab again.",
                                seen.name, seen.first_run
                            )
                        }
                    })
                });
                say(bench, said);
                see_other("/services")
            }
            ("POST", "/page") if mine => {
                let form = read_form(&sent);
                let page = form
                    .get("page")
                    .and_then(|raw| raw.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if let Ok(mut held) = bench.lock() {
                    held.page = page;
                    // An open repository belongs to the page it was opened on. Carrying it
                    // across would show one page's results with another page's table under it.
                    held.opened = None;
                    held.variants = None;
                }
                see_other("/workshop")
            }
            ("POST", "/open") if mine => {
                let form = read_form(&sent);
                let repo = form.get("repo").cloned().unwrap_or_default();
                let repo = repo.trim().to_owned();

                // Pressing the open one closes it, which is the only way to close it without a
                // script — and the same gesture the desktop Workshop uses.
                let already = bench
                    .lock()
                    .map(|held| held.opened.as_deref() == Some(repo.as_str()))
                    .unwrap_or(false);
                if already || repo.is_empty() {
                    if let Ok(mut held) = bench.lock() {
                        held.opened = None;
                        held.variants = None;
                    }
                } else {
                    // Read now, outside the lock: it reaches Hugging Face, and holding the bench
                    // across a network call would stall every other request behind it.
                    let read = epoch_models::variants(&repo);
                    if let Ok(mut held) = bench.lock() {
                        held.opened = Some(repo);
                        held.variants = Some(read);
                    }
                }
                see_other("/workshop")
            }
            ("POST", "/file") if mine => {
                let form = read_form(&sent);
                let repo = form.get("repo").cloned().unwrap_or_default();
                let quant = form.get("quant").cloned().unwrap_or_default();
                if !repo.trim().is_empty() && !quant.trim().is_empty() {
                    start_filing(
                        Arc::clone(bench),
                        repo.trim().to_owned(),
                        quant.trim().to_owned(),
                    );
                }
                see_other("/workshop")
            }
            ("POST", "/pull") if mine => {
                let form = read_form(&sent);
                let model = form.get("model").cloned().unwrap_or_default();
                if !model.trim().is_empty() {
                    start_pulling(Arc::clone(bench), model.trim().to_owned());
                }
                see_other("/workshop")
            }

            // ---- the paired Host. The bearer, and nothing else, opens these. ----

            // **Drawing on a lent machine goes through this door, not around it.**
            //
            // The Host cannot dial this machine's ComfyUI. `Studio.endpoint` is reported as
            // `127.0.0.1:8188` because that is what it is *here*, and a Host that tried it would
            // reach its own ComfyUI while believing it had reached somebody else's
            // (`epoch_kernel::Studio`). Nor should it dial a LAN address: ComfyUI binds loopback
            // by default, opening it would be a second door into this machine, and that door
            // would have none of the bearer this one has.
            //
            // So the Host compiles the graph and hands it over, and this machine runs it against
            // its own loopback. *The Host owns reality; the Bridge owns computation* (ADR-0029),
            // and a picture is computation.
            // **One answer for every refusal.** A door that said "wrong secret" here and "not
            // from here" there would be a door that answers questions nobody asked it.
            _ => (403, "text/plain", "no".to_owned(), None),
        };

        let mut answer = tiny_http::Response::from_string(body)
            .with_status_code(status)
            .with_header(
                tiny_http::Header::from_bytes(&b"Content-Type"[..], kind.as_bytes())
                    .expect("a content type is a valid header"),
            )
            /*
                **Nothing here may be kept, and that is not a nicety.**
                Every answer this program gives is a view of live state: what Ollama has right
                now, what is downloading right now, what was weighed a moment ago. There is no
                asset, no bundle and no versioned URL — the CSS is inside the page, so the page
                *is* the build.
                Measured: with no header at all, a rebuilt program served a page the window
                never showed. The new binary answered correctly to `curl`; the WKWebView kept
                using what it already had in `~/Library/WebKit/epoch-services`, and a recompile
                looked like a recompile that changed nothing.
                `no-store` rather than `no-cache`: the first says do not keep it, the second
                says keep it but ask first — and asking first still leaves a cache that a
                rebuilt program has to talk its way past.
            */
            .with_header(
                tiny_http::Header::from_bytes(&b"Cache-Control"[..], &b"no-store"[..])
                    .expect("a cache directive is a valid header"),
            );
        if let Some(where_to) = going {
            answer = answer.with_header(
                tiny_http::Header::from_bytes(&b"Location"[..], where_to.as_bytes())
                    .expect("a location is a valid header"),
            );
        }
        let _ = request.respond(answer);
    }
}

/// Answer a form by sending the browser somewhere it can safely reload.
///
/// **303 rather than 302**: it says *go and GET this instead*, which is exactly the intent, and
/// it is the one redirect a browser will not turn back into a POST.
fn see_other(where_to: &'static str) -> (u16, &'static str, String, Option<&'static str>) {
    (303, "text/plain", String::new(), Some(where_to))
}

/// The Workshop tab, rendered from whatever the bench is holding.
/// What is on this machine, and the one destructive button this surface has.
fn models_page(bench: &Mutex<Bench>) -> (u16, &'static str, String, Option<&'static str>) {
    let held = match bench.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    };
    // Read now rather than kept: a model can be pulled by the Workshop tab in the next breath,
    // and an inventory drawn from a snapshot is an inventory that lies about the thing it exists
    // to list.
    let on_disk =
        epoch_models::deck::about(epoch_models::runtimes::everything_here(&[models_dir()]));
    // **Which runtimes are answering, asked once for the page.** Three local probes, each with
    // its own short connect deadline — a closed port on this machine is refused in milliseconds
    // and one that is filtered would otherwise take twenty-one seconds, which is why `look_for`
    // opens with a connect timeout rather than a request timeout.
    let timeable: Vec<(String, String, Vec<String>)> = epoch_models::runtimes::Runtime::ALL
        .into_iter()
        .map(epoch_models::runtimes::look_for)
        .filter(|seen| !seen.models.is_empty())
        .map(|seen| {
            let mine = on_disk
                .iter()
                .filter(|one| epoch_models::runtimes::offered_as(&one.name, &seen.models).is_some())
                .map(|one| one.name.clone())
                .collect();
            (seen.id.to_owned(), seen.name.to_owned(), mine)
        })
        .filter(|(_, _, mine): &(String, String, Vec<String>)| !mine.is_empty())
        .collect();

    let body = modelspage::render(&modelspage::View {
        held: &on_disk,
        said: held.models_said.as_deref(),
        asking: held.removing.as_deref(),
        timeable: &timeable,
        measuring: held.measuring.as_deref(),
    });
    // **Redrawn while a curve is being mapped, and only then.** The same three seconds the
    // Workshop uses for a download, one rung slower because a rung of the ladder takes tens of
    // seconds rather than a fraction of one - refreshing faster than the news changes is a page
    // that flickers to say nothing new. Always-on would be a page nobody could press a button on.
    let refreshing = held.measuring.is_some().then_some(3);
    drop(held);
    (
        200,
        "text/html; charset=utf-8",
        page::frame(page::Tab::Models, &body, refreshing),
        None,
    )
}

/// Time one model on a **named** local runtime.
///
/// **One short answer, and no search.** Epoch's Host offers a four-load search for the best
/// loadout; this machine does not, because it is somebody else's computer and it agreed to
/// answer turns rather than to be benchmarked by a web page.
///
/// ## The runtime used to be Ollama, and the sentence above it said otherwise
///
/// This was documented as timing *on whichever local runtime is serving it* and hardcoded
/// Ollama's port — so a machine running llama.cpp or LM Studio got either a refusal or, worse,
/// a number attributed to a program that was not asked. The Host had the same defect in a
/// gentler form (it took whichever provider answered first, which is always Ollama); both are
/// fixed the same way, by making the runtime something the caller names.
///
/// The name sent is the name **that** server uses: a shelf row is `gemma4:12b` and llama.cpp's
/// router calls the same weights `gemma4-12b`, because Epoch wrote that link itself.
fn time_one(model: &str, on: &str) -> Result<String, String> {
    let runtime = epoch_models::runtimes::Runtime::ALL
        .into_iter()
        .find(|it| it.id() == on)
        .ok_or_else(|| format!("there is no runtime called {on:?} here."))?;
    // Asked of the program rather than assumed: `look_for` is the same probe the deck uses, so
    // one answer serves both and neither can drift from the other.
    let seen = epoch_models::runtimes::look_for(runtime);
    let at = seen.endpoint.clone();
    let named = epoch_models::runtimes::offered_as(model, &seen.models)
        .ok_or_else(|| format!("{} is not serving {model:?}.", runtime.name()))?;
    let rate = epoch_models::speeds::time_it(
        &at,
        &named,
        runtime == epoch_models::runtimes::Runtime::Ollama,
    )?;
    let library = epoch_models::generative::Library::here();
    let mut speeds = epoch_models::speeds::Speeds::load(library.root());
    let bytes = epoch_models::runtimes::everything_here(&[models_dir()])
        .into_iter()
        .find(|one| one.name == model)
        .map(|one| one.bytes)
        .unwrap_or(0);
    let machine = epoch_models::Machine::measure();
    let fitted = machine
        .vram_free
        .is_some_and(|free| bytes > 0 && bytes <= free.saturating_sub(1_500_000_000));
    speeds.remember(epoch_models::speeds::Measured {
        model: model.to_owned(),
        // **The one that was asked, not the one this used to assume.** Timing moved to a named
        // runtime and this line did not, so a reading taken on llama.cpp was filed as Ollama —
        // measured on the MacBook: 10.6 tok/s through llama.cpp, on a row that said Ollama.
        // A correct number attributed to the wrong program is the lie A exists to stop.
        runtime: runtime.name().to_owned(),
        bytes,
        tokens_per_second: rate,
        fitted,
        at: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|since| since.as_millis() as u64)
            .unwrap_or(0),
    });
    let _ = speeds.save(library.root());
    Ok(format!("{rate:.1} tokens per second, measured here."))
}

fn workshop_page(bench: &Mutex<Bench>) -> (u16, &'static str, String, Option<&'static str>) {
    // Ollama's own list, read the first time somebody opens this tab rather than at startup: a
    // program that reaches the network to draw its first frame is a program that is slow to
    // start on a machine with no route out, for a list nobody may look at.
    let need_shelf = bench.lock().map(|held| !held.shelf_read).unwrap_or(false);
    if need_shelf {
        let installed = here::models();
        let shelf = epoch_models::featured(&installed).unwrap_or_default();
        if let Ok(mut held) = bench.lock() {
            held.shelf = shelf;
            // Marked read even when it failed. Retrying on every refresh would mean a machine
            // with no internet waits two seconds for a list it is never going to get.
            held.shelf_read = true;
        }
    }

    let held = match bench.lock() {
        Ok(held) => held,
        // A poisoned lock is a panic somewhere else, and an empty Workshop is a better answer
        // than no answer at all.
        Err(poisoned) => poisoned.into_inner(),
    };

    let body = shelfpage::render(&shelfpage::View {
        fits: held.fits.as_ref(),
        installed: &here::models(),
        machine: &epoch_models::Machine::measure(),
        weighed: held.weighed.as_ref(),
        asked: &held.asked,
        pulling: held.pulling.as_deref(),
        said: held.said.as_deref(),
        query: &held.query,
        wanted: &held.wanted,
        found: &held.found,
        page: held.page,
        opened: held.opened.as_deref(),
        variants: held.variants.as_ref(),
        more: held.more.as_deref(),
        shelf: &held.shelf,
        hf: &epoch_models::hf::cli(),
        filing: held.filing.as_deref(),
        filed: held.filed.as_deref(),
    });
    // Two seconds while something downloads: often enough that the runtime's own words look
    // live, rare enough that a page full of results is not thrashing.
    // A file download is the same kind of news as a pull: something is happening and the page
    // has to keep saying so.
    let refreshing = (held.pulling.is_some() || held.filing.is_some()).then_some(2);
    drop(held);

    (
        200,
        "text/html; charset=utf-8",
        // The page refreshes itself **only** while something is downloading. Always-on would be
        // a page nobody could type into.
        page::frame(page::Tab::Workshop, &body, refreshing),
        None,
    )
}

/// The Creations Workshop, rendered from whatever the shelf is holding.
fn creations_page(bench: &Mutex<Bench>) -> (u16, &'static str, String, Option<&'static str>) {
    let library = epoch_models::generative::Library::here();
    // Made before it is described, so the path on the page is a place that exists. A search path
    // pointing at a folder nobody created is a promise nothing can keep.
    let _ = library.ensure();
    // And told to whatever draws here, exactly as the Host does it.
    let _ = epoch_models::generative::offer_library_to_comfyui(&library);

    let held = match bench.lock() {
        Ok(held) => held,
        Err(poisoned) => poisoned.into_inner(),
    };
    let body = creations::render(&held.creations, &library);
    // **Redrawn while something is arriving**, and only then. The same two seconds the Workshop
    // uses for a pull: this page has `script-src: 'none'`, so being re-rendered is the only way
    // it can say a number that changes.
    let refreshing = held.creations.fetching.is_some().then_some(2);
    drop(held);
    (
        200,
        "text/html; charset=utf-8",
        page::frame(page::Tab::Creations, &body, refreshing),
        None,
    )
}

/// Put one quantisation on disk with `hf`, on its own thread.
///
/// **Beside Ollama's download rather than instead of it.** Ollama files a model where Ollama
/// will find it and nowhere else; llama.cpp and LM Studio take a GGUF from disk and have no
/// endpoint that installs one. A machine lending a graphics card may be running either.
///
/// The plan is read first. A quantisation filter is a glob, and a glob that matches nothing
/// downloads nothing while looking like success — so what it would fetch is named before
/// seventeen gigabytes are committed to it.
fn start_filing(bench: Arc<Mutex<Bench>>, repo: String, quant: String) {
    if let Ok(mut held) = bench.lock() {
        // One at a time, like a pull: the page has one line to report with.
        if held.filing.is_some() {
            return;
        }
        held.filing = Some(format!("{repo} · {quant}"));
        held.filed = None;
    }

    std::thread::spawn(move || {
        let outcome = match epoch_models::hf::plan(&repo, &quant) {
            Err(why) => Err(why),
            Ok(planned) if planned.is_empty() => {
                // Said plainly rather than starting a download that fetches nothing.
                Err(format!("Nothing in {repo} matches '{quant}'."))
            }
            Ok(planned) => {
                if let Ok(mut held) = bench.lock() {
                    held.filing = Some(format!(
                        "{repo} · {}",
                        planned
                            .iter()
                            .map(|one| format!("{} ({})", one.file, one.size))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                }
                let into = models_dir().join(repo.replace('/', "-"));
                match std::fs::create_dir_all(&into) {
                    Err(err) => Err(format!("cannot create {}: {err}", into.display())),
                    Ok(()) => epoch_models::hf::fetch(&repo, &quant, &into)
                        .map(|at| at.display().to_string()),
                }
            }
        };

        if let Ok(mut held) = bench.lock() {
            held.filing = None;
            held.filed = Some(match outcome {
                Ok(at) => format!("Saved to {at}. Point llama.cpp or LM Studio at it."),
                Err(why) => why,
            });
        }
    });
}

/// Where a saved GGUF goes on this machine.
///
/// **Named by this program, never by a request.** A path that arrived in a form would be a
/// remote choosing where this machine writes; and a download that landed wherever the process
/// happened to be started is a file nobody can find twice.
/// Where catalogue previews are kept once they have been fetched.
///
/// Beside the models rather than inside them: `gguf_in` reads `models/`, and a folder of
/// thumbnails in there would be counted as things somebody downloaded.
fn previews_dir() -> std::path::PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join("EpochServices").join("previews")
}

fn models_dir() -> std::path::PathBuf {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join("EpochServices").join("models")
}

/// Where the links that make one shelf out of every store live.
///
/// **Beside the models directory, never inside it.** `gguf_in` reads `models/`, so a shelf of
/// links in there would count every model a second time — the same reason the Host keeps its
/// shelf out of the vault's `models/`.
fn shelf_dir() -> std::path::PathBuf {
    models_dir()
        .parent()
        .map(|at| at.join("shelf"))
        .unwrap_or_else(|| std::path::PathBuf::from("shelf"))
}

/// Start a download and return immediately.
///
/// On its own thread because a pull is minutes and the listener answers one request at a time —
/// blocking here would make the window stop responding, including the page that reports the
/// progress. The bench is updated as it goes, and the page reads it on each refresh.
/// Bring one asset down in the background, keeping how far it has got where the page can read it.
///
/// The same shape as [`start_pulling`], for the same reason: this is gigabytes, and a page with
/// `script-src: 'none'` says things by being re-rendered. `refreshing` draws it every two seconds
/// while this is set.
fn start_fetching(bench: Arc<Mutex<Bench>>, source: String, id: String) {
    if let Ok(mut held) = bench.lock() {
        // One at a time, like a pull: two downloads would fight for the same disk and the page
        // has one bar to report with.
        if held.creations.fetching.is_some() {
            return;
        }
        // Named before a byte arrives, so the bar exists from the press rather than from the
        // first chunk — a button that looks unpressed is a button somebody presses twice.
        held.creations.fetching = Some((String::new(), 0, None));
        held.creations.said = None;
    }

    std::thread::spawn(move || {
        let progress = Arc::clone(&bench);
        let said = creations::install(&source, &id, &|name, done, total| {
            if let Ok(mut held) = progress.lock() {
                held.creations.fetching = Some((name.to_owned(), done, total));
            }
        });
        if let Ok(mut held) = bench.lock() {
            held.creations.fetching = None;
            held.creations.said = Some(said);
        }
    });
}

/// Map a model's curve on this machine, on a thread, reporting a line as it goes.
///
/// **One at a time**, for the reason every other long job here is: a second search would fight
/// the first for the same graphics card, and the page has one line to report with.
fn start_measuring(bench: Arc<Mutex<Bench>>, name: String, on: String) {
    if let Ok(mut held) = bench.lock() {
        if held.measuring.is_some() {
            return;
        }
        held.measuring = Some(format!("{name}: starting\u{2026}"));
        held.models_said = None;
    }

    std::thread::spawn(move || {
        let progress = Arc::clone(&bench);
        let named = name.clone();
        let outcome = crate::curve::map(&name, &on, &models_dir(), &|done, total, one| {
            if let Ok(mut held) = progress.lock() {
                held.measuring = Some(format!(
                    "{named}: {} of {total} \u{2014} {}",
                    done + 1,
                    crate::modelspage::plainly(&one)
                ));
            }
        });
        if let Ok(mut held) = bench.lock() {
            held.measuring = None;
            held.models_said = Some(match outcome {
                Ok(good) => good,
                Err(why) => why,
            });
        }
    });
}

fn start_pulling(bench: Arc<Mutex<Bench>>, model: String) {
    if let Ok(mut held) = bench.lock() {
        // One at a time. A second pull would fight the first for the same disk and the same
        // Ollama, and the page has one line to report with.
        if held.pulling.is_some() {
            return;
        }
        held.pulling = Some(model.clone());
        held.said = None;
    }

    std::thread::spawn(move || {
        let named = model.clone();
        let progress = Arc::clone(&bench);
        let outcome = epoch_models::pull(
            here::ollama(),
            &model,
            &|| false,
            &mut |step: epoch_models::Fetching| {
                if let Ok(mut held) = progress.lock() {
                    // The runtime's own words, with the percentage when it is moving bytes.
                    held.pulling = Some(match (step.total, step.completed) {
                        (Some(total), Some(done)) if total > 0 => format!(
                            "{named} · {} · {}%",
                            step.status,
                            done.saturating_mul(100) / total
                        ),
                        _ => format!("{named} · {}", step.status),
                    });
                }
            },
        );

        if let Ok(mut held) = bench.lock() {
            held.pulling = None;
            held.said = Some(match outcome {
                Ok(()) => format!("{model} is on this machine now, and lent to the Host."),
                // Ollama's own sentence. A missing model answers `pull model manifest: file does
                // not exist`, and nothing Epoch could write would be more useful.
                Err(why) => why,
            });
            // The verdict was about the model that just arrived; the shelf below is the truth now.
            held.weighed = None;
            // And what is *already here* changed, so the marks beside the lists are stale.
            held.shelf_read = false;
        }
    });
}

/// The image studio a form named, or a sentence saying it named nothing this build knows.
///
/// **A closed set, resolved here.** The same discipline `runner_at` follows one file over: what
/// arrives from a form is an id to look up, never an address to dial. There is one studio today
/// and this is still a lookup — the day there are two, nothing about the routes changes.
fn studio_named(asked: Option<&String>) -> Result<epoch_models::studio::Studio, String> {
    let wanted = asked.map(String::as_str).unwrap_or_default().trim();
    epoch_models::studio::Studio::ALL
        .into_iter()
        .find(|studio| studio.id() == wanted)
        .ok_or_else(|| format!("'{wanted}' is not an image studio this build knows"))
}

/// Leave one sentence for the next page to show, whichever way it went.
///
/// A failure is as much a thing to say as a success, and both are *taken* by the page that shows
/// them so neither outlives the action that produced it.
fn say(bench: &Arc<Mutex<Bench>>, said: Result<String, String>) {
    if let Ok(mut held) = bench.lock() {
        held.connect = Some(match said {
            Ok(good) => good,
            Err(why) => why,
        });
    }
}

/// The body, up to `ceiling` bytes, or `Err` if it is bigger than that.
///
/// **Two gates, because either alone has a hole.** The declared length is checked first, so an
/// honest caller announcing something enormous is refused before a byte of it is read; and the
/// read itself is bounded, because a length header is a claim and a chunked body carries none.
/// `take(ceiling + 1)` is what makes the second gate detect the overrun rather than silently
/// truncate — a body cut in half and then parsed is worse than a body refused.
fn read_bounded(request: &mut tiny_http::Request, ceiling: usize) -> Result<String, ()> {
    if request
        .body_length()
        .is_some_and(|declared| declared > ceiling)
    {
        return Err(());
    }
    use std::io::Read as _;
    let mut body = Vec::new();
    let read = request
        .as_reader()
        .take(ceiling as u64 + 1)
        .read_to_end(&mut body)
        .map_err(|_| ())?;
    if read > ceiling {
        return Err(());
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

/// A posted form, decoded. Small on purpose: two fields, typed by the person sitting here.
fn read_form(sent: &str) -> std::collections::BTreeMap<String, String> {
    sent.split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| (unescape(key), unescape(value)))
        .collect()
}

fn unescape(raw: &str) -> String {
    let bytes = raw.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&String::from_utf8_lossy(&bytes[i + 1..i + 3]), 16)
            {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typed_form_survives_the_wire() {
        let form = "code=K7M2QX&host=http%3A%2F%2F192.168.1.10%3A11500";
        let read: std::collections::BTreeMap<String, String> = form
            .split('&')
            .filter_map(|pair| pair.split_once('='))
            .map(|(k, v)| (unescape(k), unescape(v)))
            .collect();
        assert_eq!(read["code"], "K7M2QX");
        assert_eq!(read["host"], "http://192.168.1.10:11500");
    }

    #[test]
    fn a_plus_is_a_space_and_not_a_plus() {
        // Form encoding, which is its own small trap: a machine called "Studio Mac" arrives as
        // "Studio+Mac" and would otherwise be named with a plus in it.
        assert_eq!(unescape("Studio+Mac"), "Studio Mac");
    }
}
