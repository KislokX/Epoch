//! What is actually on this machine, and the one destructive button either surface has.
//!
//! ## The two-surfaces rule, applied on purpose rather than after the fact
//!
//! Epoch's MODELS deck answers *what is on this computer* — and on a lent machine, **this** is
//! the computer holding the weights. A Host's inventory is an inventory of the Host; the person
//! standing at the machine that actually has thirty gigabytes of models on it needs to see them
//! and be able to take one off.
//!
//! What counts as a model, how it is timed, and what removing one means all live in
//! `epoch_models` and are shared verbatim. Two copies of *what counts as a model this machine
//! has* is exactly how the two surfaces come to disagree.
//!
//! ## What this surface deliberately does not do
//!
//! It does not **search** for a loadout. That costs four minutes of a graphics card, and this is
//! somebody else's computer — the machine agreed to answer turns, not to be benchmarked by a
//! web page. `TIME IT` is here because it is one short answer and it is asked for; the search is
//! the Host's to offer, on the Host's own hardware.
//!
//! It does **choose** from a curve that has already been measured, because that costs nothing
//! and it is the machine's own recipe. Refusing to show it here meant the person standing at the
//! computer holding the weights could delete a model but not say how it loads — and the rule
//! that made the search the Host's job says nothing about a decision that is free.

use epoch_models::deck::ModelHere;
use epoch_models::loadout::{Loadout, Tried};

/// The runtimes a curve can actually be taken through, and what to call them.
///
/// LM Studio is deliberately absent rather than refused on press: it takes a context only when
/// a model is loaded, through its own CLI, so a curve there would end in a setting nothing
/// applies. A button that produces an inapplicable row is the control this codebase keeps
/// deleting.
const MEASURABLE: [(&str, &str); 2] = [("llama_cpp", "LLAMA.CPP"), ("ollama", "OLLAMA")];

use crate::page;

pub struct View<'a> {
    pub held: &'a [ModelHere],
    /// The runtimes answering on this machine, and which of these models each one holds.
    ///
    /// **Because TIME IT has to name one.** It used to send every request to Ollama's port under
    /// a comment saying it used whichever runtime was serving — so on a machine running
    /// llama.cpp the answer was a refusal, and on one running both it was a number attributed to
    /// a program nobody chose. With `script-src: 'none'` the choice is a `<select>` in the form
    /// that already exists, which needs no script at all.
    pub timeable: &'a [(String, String, Vec<String>)],
    /// What the last button said, if anything has been pressed.
    pub said: Option<&'a str>,
    /// Which model is one click from being removed, if REMOVE has been pressed once.
    pub asking: Option<&'a str>,
    /// The curve being mapped right now, as a line. `None` when none is.
    ///
    /// Drawn at the top rather than on the row, because a search holds the graphics card and
    /// every other row on the page is waiting for it too.
    pub measuring: Option<&'a str>,
}

pub fn render(view: &View<'_>) -> String {
    // What is happening now, above everything it is happening to.
    let running = match view.measuring {
        Some(line) => format!(
            r#"<div class="mdls__running"><p><b>MAPPING A CURVE</b> {}</p>
    <p class="quiet">It holds this graphics card until it finishes. Nothing else here can be
       measured meanwhile.</p></div>"#,
            page::escape(line)
        ),
        None => String::new(),
    };

    let body = if view.held.is_empty() {
        r#"<p class="quiet">Nothing here yet. The Models Workshop is where models are found and
           fetched onto this machine.</p>"#
            .to_owned()
    } else {
        view.held.iter().map(|one| row(one, view)).collect()
    };

    let total: u64 = view.held.iter().map(|one| one.bytes).sum();

    format!(
        r#"<section class="panel">
  <h2>Models on this machine</h2>
  <p class="quiet">{count} {word}, {total} in all.</p>
  {running}
  {said}
  <div class="mdls">{body}</div>
  <p class="quiet">
    Removing takes the shelf's hard link with it. Without that the bytes would stay on the disk
    and llama.cpp would keep offering a model Ollama no longer has. An Ollama model is unfiled by
    <code>ollama rm</code> rather than by deleting a blob several tags may share.
  </p>
</section>"#,
        count = view.held.len(),
        word = if view.held.len() == 1 {
            "model"
        } else {
            "models"
        },
        total = gb(total),
        said = match view.said {
            Some(said) => format!(r#"<p class="verdict">{}</p>"#, page::escape(said)),
            None => String::new(),
        },
    )
}

fn row(one: &ModelHere, view: &View<'_>) -> String {
    let asking = view.asking == Some(one.path.as_str());
    format!(
        r#"<div class="mdls__row">
  <div>
    <p class="mdls__name">{name}</p>
    <p class="quiet">{from} &middot; {size}{eyes}{speed}</p>
    {twin}
    <p class="quiet mdls__path">{path}</p>
  </div>
  <div class="mdls__doing">
    <form method="post" action="/models/time">
      <input type="hidden" name="name" value="{name}">
      {on}
    </form>
    {removal}
  </div>
  {recipe}
</div>"#,
        // Which runtimes hold *this* model. None is an honest dead end and says so; one needs
        // no choice and names itself on the button; several is where a control belongs.
        on = {
            let holders: Vec<&(String, String, Vec<String>)> = view
                .timeable
                .iter()
                .filter(|(_, _, models)| models.contains(&one.name))
                .collect();
            match holders.as_slice() {
                [] => r#"<button type="submit" disabled>NOTHING IS SERVING IT</button>"#.to_owned(),
                [(id, name, _)] => format!(
                    r#"<input type="hidden" name="on" value="{id}"><button type="submit">TIME IT ON {name}</button>"#,
                    id = page::escape(id),
                    name = page::escape(&name.to_uppercase()),
                ),
                many => format!(
                    r#"<select name="on">{options}</select><button type="submit">TIME IT</button>"#,
                    options = many
                        .iter()
                        .map(|(id, name, _)| format!(
                            r#"<option value="{id}">{name}</option>"#,
                            id = page::escape(id),
                            name = page::escape(name)
                        ))
                        .collect::<String>()
                ),
            }
        },
        name = page::escape(&one.name),
        from = page::escape(&one.from),
        size = gb(one.bytes),
        recipe = recipe(one),
        // The same weights stored twice. Said, never folded: both files are on the disk and
        // removing one row must not remove the other.
        twin = if one.same_weights_as.is_empty() {
            String::new()
        } else {
            format!(
                r#"<p class="notice">the same weights as {} &mdash; one copy can go</p>"#,
                page::escape(&one.same_weights_as.join(", "))
            )
        },
        eyes = if one.sees { " &middot; can see" } else { "" },
        // A speed is only ever a measurement. A row nobody timed says nothing rather than an
        // estimate, because an estimate belongs beside a model somebody does not have.
        speed = match (one.tokens_per_second, one.measured_on.as_deref()) {
            (Some(rate), Some(runtime)) =>
                format!(" &middot; {rate:.1} tok/s on {}", page::escape(runtime)),
            (Some(rate), None) => format!(" &middot; {rate:.1} tok/s"),
            _ => String::new(),
        },
        path = page::escape(&one.path),
        removal = if asking {
            // **Never on the first click.** A model is minutes of downloading and tens of
            // gigabytes, and there is no undo — so the button says what it is about to do, by
            // name, and waits for a second press.
            format!(
                r#"<form method="post" action="/models/remove">
      <input type="hidden" name="path" value="{path}">
      <button type="submit" class="danger">DELETE {name}</button>
    </form>
    <form method="post" action="/models/keep"><button type="submit">KEEP IT</button></form>"#,
                path = page::escape(&one.path),
                name = page::escape(&one.name),
            )
        } else {
            format!(
                r#"<form method="post" action="/models/ask">
      <input type="hidden" name="path" value="{path}">
      <button type="submit">REMOVE</button>
    </form>"#,
                path = page::escape(&one.path),
            )
        },
    )
}

/// How this model loads, and the measured curve it was chosen from.
///
/// **Cold where nothing was measured, and it says which.** An unmeasured model still loads —
/// with the safe half of every trade — so the honest sentence is *this is the default*, not a
/// blank. That distinction is the whole reason `measured` is a field rather than an inference
/// from an empty curve.
fn recipe(one: &ModelHere) -> String {
    let now = plainly(&one.loadout);
    if one.readings.is_empty() {
        /*
            **This used to send the reader to the Host, and the Host cannot answer.**

            The sentence was *Epoch’s own Workshop can map the curve — which is why this page
            will not start one*, and it was false in the way that costs most: a curve is kept
            against *model + card + build*, so the Host’s Workshop maps the Host’s card. A
            lent MacBook was being pointed at a measurement of somebody else’s hardware.

            The owner’s words when he found it: *it is a different PC with everything
            different; we are not going to use a configuration made on a 4070 SUPER for a
            MacBook.* So the button lives here now, on the machine whose card it is about.
        */
        let can = MEASURABLE
            .iter()
            .filter_map(|(id, name)| {
                let seen = epoch_models::runtimes::look_for(match *id {
                    "ollama" => epoch_models::runtimes::Runtime::Ollama,
                    _ => epoch_models::runtimes::Runtime::LlamaCpp,
                });
                (seen.installed && (*id != "ollama" || seen.serving)).then(|| {
                    format!(r#"<button name="on" value="{id}">MEASURE ON {name}</button>"#)
                })
            })
            .collect::<Vec<_>>()
            .join("");

        // A control with nothing behind it is worse than none: if neither runtime can be
        // measured through, this says which and offers no button.
        let act = if can.is_empty() {
            r#"<p class="quiet">Neither llama.cpp nor a running Ollama is here, so there is
               nothing to measure through yet.</p>"#
                .to_owned()
        } else {
            format!(
                r#"<form method="post" action="/models/measure" class="mdls__measure">
      <input type="hidden" name="name" value="{name}">
      {can}
    </form>"#,
                name = page::escape(&one.name)
            )
        };

        return format!(
            r#"<div class="mdls__recipe">
    <p class="quiet">Loads with {now}. Nothing has been measured for this model on this card,
       so that is the conservative default rather than a reading. Mapping the curve takes
       several minutes of <b>this</b> graphics card — and it has to happen here, because a
       curve measured anywhere else is an answer about a different machine.</p>
    {act}
  </div>"#
        );
    }

    // Ordered by what a person compares: the roomiest first, since that is the axis they are
    // trading speed away for.
    let mut curve: Vec<&Tried> = one.readings.iter().collect();
    curve.sort_by_key(|one| std::cmp::Reverse(one.loadout.context));

    let rows: String = curve
        .iter()
        .map(|tried| {
            let chosen = tried.loadout == one.loadout;
            let marked = Some(tried.loadout) == one.recommended;
            format!(
                r#"<tr class="{class}">
      <td>{context}</td>
      <td>{cache}</td>
      <td>{rate}</td>
      <td>{note}</td>
      <td>{action}</td>
    </tr>"#,
                class = if chosen { "on" } else { "" },
                context = tried.loadout.context,
                cache = page::escape(tried.loadout.cache.plainly()),
                rate = match tried.tokens_per_second {
                    Some(rate) => format!("{rate:.1} tok/s"),
                    // Measured and unusable is not the same as unmeasured, and the table has
                    // room to say which.
                    None => "—".to_owned(),
                },
                note = match (chosen, marked) {
                    (true, true) => "in use &middot; recommended",
                    (true, false) => "in use",
                    (false, true) => "recommended",
                    (false, false) => "",
                },
                action = if chosen {
                    String::new()
                } else if tried.tokens_per_second.is_none() {
                    // Nothing to choose: a row with no rate is one the probe could not complete,
                    // and offering it would be a button that fails on press.
                    String::new()
                } else {
                    format!(
                        r#"<form method="post" action="/models/recipe">
        <input type="hidden" name="path" value="{path}">
        <input type="hidden" name="context" value="{context}">
        <button type="submit">USE THIS</button>
      </form>"#,
                        path = page::escape(&one.path),
                        context = tried.loadout.context,
                    )
                },
            )
        })
        .collect();

    format!(
        r#"<details class="mdls__recipe">
    <summary>Loads with {now} &mdash; {count} measured settings</summary>
    <table class="curve">
      <tr><th>context</th><th>cache</th><th>speed</th><th></th><th></th></tr>
      {rows}
    </table>
    <p class="quiet">A change takes effect the next time llama.cpp starts.</p>
  </details>"#,
        count = curve.len(),
    )
}

/// One loadout, in the words a person reads.
/// One setting, in words.
///
/// `pub(crate)` since the search reports its progress with it: the line a running curve draws
/// names the same setting the finished table names, and two spellings of one fact are two that
/// drift apart.
pub(crate) fn plainly(loadout: &Loadout) -> String {
    format!(
        "{} tokens on the {}",
        loadout.context,
        loadout.cache.plainly()
    )
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / 1e9)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TIME IT names the runtime, or says nothing is serving it.
    ///
    /// **This page hardcoded Ollama's port** under a comment saying it used whichever runtime was
    /// serving — so a machine running llama.cpp got a refusal and a machine running both got a
    /// number attributed to a program nobody chose. A reading of the right quantity on a runtime
    /// the reader did not pick is the most convincing way an instrument lies.
    ///
    /// The choice is a `<select>` in the form that already exists, because this page has
    /// `script-src: 'none'` and a control that needs no script is the only kind it can have.
    #[test]
    fn time_it_names_the_runtime_it_would_use() {
        let held = [one("gemma4:12b", "Ollama", 7_400_000_000)];

        // Nothing answering: an honest dead end rather than a button that fails when pressed.
        let cold = render(&View {
            held: &held,
            said: None,
            asking: None,
            timeable: &[],
            measuring: None,
        });
        assert!(cold.contains("NOTHING IS SERVING IT"), "{cold}");

        // One runtime: no choice to make, and it says whose number this will be.
        let alone = render(&View {
            measuring: None,
            held: &held,
            said: None,
            asking: None,
            timeable: &[(
                "ollama".to_owned(),
                "Ollama".to_owned(),
                vec!["gemma4:12b".to_owned()],
            )],
        });
        assert!(alone.contains("TIME IT ON OLLAMA"), "{alone}");
        assert!(alone.contains(r#"name="on" value="ollama""#), "{alone}");
        assert!(!alone.contains("<select name=\"on\">"), "nothing to choose");

        // Two: the control appears exactly where there is a choice.
        let both = render(&View {
            measuring: None,
            held: &held,
            said: None,
            asking: None,
            timeable: &[
                (
                    "ollama".to_owned(),
                    "Ollama".to_owned(),
                    vec!["gemma4:12b".to_owned()],
                ),
                (
                    "llama_cpp".to_owned(),
                    "llama.cpp".to_owned(),
                    vec!["gemma4:12b".to_owned()],
                ),
            ],
        });
        assert!(both.contains(r#"<select name="on">"#), "{both}");
        assert!(
            both.contains(r#"<option value="llama_cpp">llama.cpp</option>"#),
            "{both}"
        );
    }

    /// An unmeasured model offers to be measured **here**, and says why here.
    ///
    /// This page used to send the reader to the Host's Workshop, which measures the Host's card.
    /// A curve is kept against *model + card + build*, so that advice was an answer about
    /// somebody else's hardware — the failure this codebase calls a real reading of the wrong
    /// quantity, wearing a sentence.
    #[test]
    fn an_unmeasured_model_offers_to_be_measured_on_this_machine() {
        let page = render(&View {
            measuring: None,
            held: &[one("qwen3:14b", "Saved here", 9_300_000_000)],
            said: None,
            asking: None,
            timeable: &[],
        });
        assert!(
            page.contains(r#"action="/models/measure""#) || page.contains("nothing to measure"),
            "an unmeasured model must either offer the search or say why it cannot: {page}"
        );
        assert!(
            !page.contains("Epoch's own Workshop can map the curve"),
            "the sentence that pointed at the Host's card is gone: {page}"
        );
        assert!(
            page.contains("a different machine"),
            "and it says why the measurement has to happen here: {page}"
        );
    }

    /// A running search says so above every row, and the rows are not offered meanwhile.
    #[test]
    fn a_running_search_is_announced_over_everything_it_holds_up() {
        let page = render(&View {
            measuring: Some("qwen3:14b: 3 of 12 — 24,576 tokens on the f16 cache"),
            held: &[one("qwen3:14b", "Saved here", 9_300_000_000)],
            said: None,
            asking: None,
            timeable: &[],
        });
        assert!(page.contains("MAPPING A CURVE"), "{page}");
        assert!(
            page.contains("3 of 12"),
            "the line is the runtime's own progress: {page}"
        );
        assert!(
            page.contains("holds this graphics card"),
            "and it says what that costs everything else: {page}"
        );
    }

    fn one(name: &str, from: &str, bytes: u64) -> ModelHere {
        ModelHere {
            name: name.into(),
            from: from.into(),
            path: format!(r"C:\models\{name}.gguf"),
            bytes,
            sees: false,
            trained_context: None,
            loadout: epoch_models::deck::conservative_default(),
            measured: false,
            readings: Vec::new(),
            recommended: None,
            tokens_per_second: None,
            measured_on: None,
            curve_on: None,
            tuning: epoch_models::tuning::Tuning::default(),
            drafts: Vec::new(),
            same_weights_as: Vec::new(),
        }
    }

    fn measured(name: &str, curve: Vec<Tried>, chose: Loadout) -> ModelHere {
        ModelHere {
            loadout: chose,
            measured: true,
            recommended: epoch_models::deck::recommended_of(&curve),
            readings: curve,
            tokens_per_second: Some(33.7),
            measured_on: Some("llama.cpp".into()),
            curve_on: None,
            tuning: epoch_models::tuning::Tuning::default(),
            drafts: Vec::new(),
            ..one(name, "Saved here", 10_624_771_968)
        }
    }

    #[test]
    fn an_empty_machine_says_where_models_come_from_rather_than_nothing() {
        let page = render(&View {
            held: &[],
            said: None,
            asking: None,
            timeable: &[],
            measuring: None,
        });
        assert!(page.contains("Nothing here yet"), "{page}");
        assert!(page.contains("Models Workshop"), "{page}");
    }

    /// **The second press is the whole safety.** A REMOVE that deleted on the first click would
    /// be one misplaced click away from a nine-gigabyte download.
    #[test]
    fn removing_needs_a_second_press_and_the_first_one_names_the_model() {
        let held = vec![one("qwen3:14b", "Ollama", 9_300_000_000)];
        let cold = render(&View {
            held: &held,
            said: None,
            asking: None,
            timeable: &[],
            measuring: None,
        });
        assert!(cold.contains("/models/ask"), "{cold}");
        assert!(
            !cold.contains("/models/remove"),
            "nothing deletes on the first press: {cold}"
        );

        let asked = render(&View {
            held: &held,
            said: None,
            asking: Some(&held[0].path),
            timeable: &[],
            measuring: None,
        });
        assert!(asked.contains("/models/remove"), "{asked}");
        assert!(
            asked.contains("DELETE qwen3:14b"),
            "it says what it will delete: {asked}"
        );
        assert!(asked.contains("KEEP IT"), "and how to back out: {asked}");
    }

    /// Only the row that was asked about. A page where every REMOVE armed at once is a page
    /// where the second click lands on the wrong model.
    #[test]
    fn arming_one_row_does_not_arm_the_others() {
        let held = vec![
            one("qwen3:14b", "Ollama", 9_300_000_000),
            one("gemma4:12b", "Ollama", 7_400_000_000),
        ];
        let page = render(&View {
            held: &held,
            said: None,
            asking: Some(&held[0].path),
            timeable: &[],
            measuring: None,
        });
        assert_eq!(page.matches("/models/remove").count(), 1, "{page}");
        assert!(page.contains("DELETE qwen3:14b"), "{page}");
        assert!(!page.contains("DELETE gemma4:12b"), "{page}");
    }

    /// The whole point of bringing the recipe here: the person at the machine holding the
    /// weights can see how it loads and change it.
    #[test]
    fn a_measured_model_shows_its_curve_and_offers_the_other_rows() {
        use epoch_models::loadout::Cache;
        let row = |context, cache, rate| Tried {
            loadout: Loadout { context, cache },
            tokens_per_second: Some(rate),
        };
        let curve = vec![
            row(16_384, Cache::Q8_0, 35.15),
            row(16_384, Cache::F16, 20.28),
            row(24_576, Cache::Q8_0, 18.53),
        ];
        let page = render(&View {
            held: &[measured(
                "Qwen3.8-27B",
                curve,
                Loadout {
                    context: 16_384,
                    cache: Cache::Q8_0,
                },
            )],
            said: None,
            asking: None,
            timeable: &[],
            measuring: None,
        });

        assert!(
            page.contains("35.2 tok/s") || page.contains("35.1 tok/s"),
            "{page}"
        );
        assert!(page.contains("in use"), "{page}");
        // The row already in use offers no button — pressing it would do nothing, and a
        // control that does nothing is worse than no control.
        assert_eq!(page.matches("USE THIS").count(), 2, "{page}");
        assert!(page.contains("/models/recipe"), "{page}");
        // The measured speed belongs on the row, beside the size.
        assert!(page.contains("33.7 tok/s on llama.cpp"), "{page}");
    }

    /// **Unmeasured is not blank.** A model still loads, with the conservative half of every
    /// trade, and the page has to say that rather than leave a hole a reader fills in.
    #[test]
    fn an_unmeasured_model_says_it_is_a_default_rather_than_a_reading() {
        let page = render(&View {
            held: &[one("gemma4:12b", "Ollama", 7_400_000_000)],
            said: None,
            asking: None,
            timeable: &[],
            measuring: None,
        });
        assert!(page.contains("conservative default"), "{page}");
        assert!(!page.contains("USE THIS"), "nothing to choose from: {page}");
        // And no speed is claimed for something nobody timed.
        assert!(!page.contains("tok/s"), "{page}");
    }

    /// **Said on both rows, folded on neither.** The disk really holds 21.2 GB; hiding one row
    /// would make a deck that disagrees with the free space.
    #[test]
    fn the_same_weights_twice_are_named_and_both_rows_stay() {
        let mut a = one("qwen-ollama", "Ollama", 10_624_771_968);
        let mut b = one("qwen-saved", "Saved here", 10_624_771_968);
        a.same_weights_as = vec![b.name.clone()];
        b.same_weights_as = vec![a.name.clone()];
        let page = render(&View {
            held: &[a, b],
            said: None,
            asking: None,
            timeable: &[],
            measuring: None,
        });
        assert_eq!(page.matches("one copy can go").count(), 2, "{page}");
        assert!(page.contains("the same weights as qwen-saved"), "{page}");
        assert!(page.contains("the same weights as qwen-ollama"), "{page}");
        // And the total still counts both, because the disk does.
        assert!(page.contains("21.2 GB in all"), "{page}");
    }

    /// The shelf a model is on decides what removing it means, so it is on screen.
    #[test]
    fn each_row_says_which_shelf_it_is_on_and_what_it_weighs() {
        let page = render(&View {
            held: &[
                one("qwen3:14b", "Ollama", 9_300_000_000),
                one("Qwen3.8-27B-Uncensored-IQ2_M", "Saved here", 10_624_771_968),
            ],
            said: None,
            asking: None,
            timeable: &[],
            measuring: None,
        });
        assert!(page.contains("Ollama"), "{page}");
        assert!(page.contains("Saved here"), "{page}");
        assert!(page.contains("9.3 GB"), "{page}");
        assert!(page.contains("10.6 GB"), "{page}");
        assert!(page.contains("19.9 GB in all"), "{page}");
    }
}
