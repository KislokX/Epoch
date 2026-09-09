//! What this machine answers the Host, on the one door that is on the network.
//!
//! ## Why these routes live apart from the page
//!
//! They always were a separate surface — `door.rs` has said so since it was written — and until
//! now they were a separate surface on **one listener**, told apart by whether the request
//! carried the secret. That worked and it made one property impossible to state simply: *the
//! person's pages are not reachable from the network.* It was true only because every one of
//! forty match arms remembered to say `if mine`.
//!
//! Now the two audiences have two doors. The page is served on loopback and cannot be reached
//! from anywhere else because it is **not bound** anywhere else; everything here is served over
//! TLS on the network port and never touches the page's routes, because they are not in this
//! file. A rule the shape of the program keeps is a rule nobody has to remember.
//!
//! ## Everything here is behind the bearer, except the one thing that mints it
//!
//! `/enrol` is how a bond begins, so it cannot require one. What it requires instead is the
//! short code the person is looking at on this machine's screen — checked here, spent the moment
//! it works, and burnt after ten wrong answers.

use std::sync::Mutex;

use epoch_wire::wire::{Answer, Asked};

use crate::keep::{self, Bond};
use crate::{door, here};

/// One request from the Host.
pub fn answer(
    asked: Asked,
    state: &Mutex<Bond>,
    showing: &Mutex<Option<crate::invite::Code>>,
) -> Answer {
    let bond = state.lock().map(|held| held.clone()).unwrap_or_default();
    let admitted = door::bearer_matches(&bond, asked.bearer.as_deref());

    match (asked.method.as_str(), asked.path.as_str()) {
        // **The introduction, in the direction where the Host reaches out.** Anything on the
        // network can send this, and without the code none of it can enrol.
        ("POST", "/enrol") => enrol(&asked.body, state, showing),

        ("GET", "/have") if admitted => Answer::new(
            200,
            "application/json",
            serde_json::to_string(&here::measure()).unwrap_or_default(),
        ),

        ("POST", "/ask") if admitted => {
            match serde_json::from_str::<epoch_kernel::Ask>(&asked.body) {
                Ok(turn) => match door::ask(&turn) {
                    Ok(told) => Answer::new(
                        200,
                        "application/json",
                        serde_json::to_string(&told).unwrap_or_default(),
                    ),
                    Err(why) => Answer::new(502, "text/plain", why),
                },
                Err(why) => Answer::new(400, "text/plain", format!("unreadable turn: {why}")),
            }
        }

        ("POST", "/release") if admitted => {
            // The Host finished a turn and does not want the crew kept warm. Answering 200
            // either way: whether a model was resident is not something the Host asked, and a
            // refusal here would look to it like a broken pairing.
            let said = serde_json::from_str::<serde_json::Value>(&asked.body).unwrap_or_default();
            let model = said
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or_default();
            // Which program is holding it. An older Host says nothing, which means its Ollama —
            // the same default an `Ask` without a runner carries.
            let runner = said.get("runner").and_then(|r| r.as_str());
            if !model.trim().is_empty() {
                let _ = door::release(model.trim(), runner);
            }
            Answer::new(200, "text/plain", String::new())
        }

        ("POST", "/show") if admitted => {
            // A question about a model, behind the same bearer as everything else the Host asks.
            // Answering `{}` for anything unreadable rather than a status: *unasked* is the
            // honest reading, and a 4xx here would look to the Host like a broken pairing.
            let said = serde_json::from_str::<serde_json::Value>(&asked.body).unwrap_or_default();
            let model = said
                .get("model")
                .and_then(|m| m.as_str())
                .unwrap_or_default();
            let runner = said.get("runner").and_then(|r| r.as_str());
            Answer::new(
                200,
                "application/json",
                serde_json::to_string(&door::shown(model, runner)).unwrap_or_default(),
            )
        }

        ("GET", "/easel/schema") if admitted => {
            match here::easel_at().and_then(|comfy| {
                ureq::get(&format!("{comfy}/object_info"))
                    .timeout(std::time::Duration::from_secs(30))
                    .call()
                    .map_err(|why| format!("ComfyUI did not answer: {why}"))?
                    .into_string()
                    .map_err(|why| format!("ComfyUI answered with something unreadable: {why}"))
            }) {
                Ok(info) => Answer::new(200, "application/json", info),
                Err(why) => Answer::new(503, "application/json", refusal(&why)),
            }
        }

        // The compiled graph in, the picture out. One call rather than four, because the response
        // here is a `String` and a PNG is not one — so the bytes come back base64, the way an
        // import already crosses IPC (ADR-0024).
        ("POST", "/easel/draw") if admitted => {
            let drawn = serde_json::from_str::<serde_json::Value>(&asked.body)
                .map_err(|why| format!("that was not a workflow: {why}"))
                .and_then(|said| {
                    let nodes = said
                        .get("prompt")
                        .cloned()
                        .ok_or_else(|| "no `prompt` in the request".to_owned())?;
                    let comfy = here::easel_at()?;
                    epoch_models::comfy::Comfy::at(&comfy).draw_graph(&nodes)
                });
            match drawn {
                Ok((bytes, name, seconds)) => {
                    use base64::Engine;
                    let png = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    Answer::new(
                        200,
                        "application/json",
                        serde_json::json!({ "file": name, "seconds": seconds, "png": png })
                            .to_string(),
                    )
                }
                // ComfyUI's own words, all of them: it names the node, the input and the value it
                // did not like, and that is the only thing that explains what to fix.
                Err(why) => Answer::new(503, "application/json", refusal(&why)),
            }
        }

        // **One answer for every refusal.** A door that said "wrong secret" here and "no such
        // route" there would be a door that answers questions nobody asked it.
        _ => Answer::new(403, "text/plain", "no".to_owned()),
    }
}

/// One shape for every refusal on the easel door, so the Host never has to guess whether a
/// failure was ComfyUI's words or this program's.
fn refusal(why: &str) -> String {
    serde_json::json!({ "problem": why }).to_string()
}

/// Check the code that is on screen and **take it**, without letting go in between.
///
/// ## Why this is one function and not two statements
///
/// It used to be: check under the lock, release the lock, build the bond, write it, and only
/// then clear the code. Two machines arriving in the same instant both passed the check before
/// either cleared it, so one invitation admitted two — check-then-act, on a listener that serves
/// concurrently by design. The other end of the same handshake had the identical shape and the
/// identical fix (`epoch_engine::pairing::wait_for_one`).
///
/// So the check and the taking happen under one lock, and the answer to *may I* is also the act
/// of taking it. There is no instant in between for a second request to occupy.
///
/// ## `attempt` rather than `matches`, and why a wrong code does not burn it
///
/// A wrong answer is counted, and after ten the code is spent. That is a different thing from
/// being redeemed, and it is why the code is taken only once it matched: this route is on the
/// network by design and the code is the whole of the defence, so how many times it may be
/// offered is part of it.
///
/// ## And it stays taken if the write then fails
///
/// The bond is written after this returns, and that can fail. The code is not handed back: a
/// one-shot credential that is returned on error is a credential whose exclusion depends on
/// nothing going wrong. Showing another code costs one press.
fn take(showing: &Mutex<Option<crate::invite::Code>>, offered: &str) -> bool {
    let Ok(mut held) = showing.lock() else {
        return false;
    };
    if held.as_mut().is_some_and(|code| code.attempt(offered)) {
        *held = None;
        return true;
    }
    false
}

fn enrol(body: &str, state: &Mutex<Bond>, showing: &Mutex<Option<crate::invite::Code>>) -> Answer {
    let said = serde_json::from_str::<epoch_kernel::Enrol>(body).ok();

    let admitted = said
        .as_ref()
        .is_some_and(|enrol| take(showing, &enrol.code));

    let (true, Some(enrol)) = (admitted, said) else {
        // The same single refusal every other unauthorised request gets. A wrong code and a
        // malformed body are indistinguishable from outside on purpose.
        return Answer::new(403, "text/plain", "no".to_owned());
    };

    let bond = Bond {
        // The Host files this machine under an id of its own; this side only needs to know it is
        // spoken for. Its own address is the useful name to keep, because that is what the Host
        // will present a bearer to.
        id: enrol.host.clone(),
        secret: enrol.secret,
        host: enrol.host,
    };
    let kept = keep::write(&bond);
    if let Ok(mut held) = state.lock() {
        *held = bond;
    }
    match kept {
        Ok(()) => Answer::new(
            200,
            "application/json",
            serde_json::to_string(&epoch_kernel::Enrolled {
                name: here::name(),
                have: here::measure(),
            })
            .unwrap_or_default(),
        ),
        Err(why) => Answer::new(500, "text/plain", why),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asking(method: &str, path: &str, bearer: Option<&str>, body: &str) -> Asked {
        // Built rather than declared: an `Asked` read off a door carries the body budget it was
        // counted against, and one assembled here reserved nothing. The constructor is what
        // keeps those two facts from being typed apart.
        Asked::made(method, path, bearer.map(str::to_owned), body)
    }

    fn paired() -> Mutex<Bond> {
        Mutex::new(Bond {
            id: "host".to_owned(),
            secret: "a-long-secret-nobody-guesses".to_owned(),
            host: "https://10.0.0.9:11501".to_owned(),
        })
    }

    #[test]
    fn the_page_is_not_on_this_door_at_all() {
        // Not "refused when it comes from the network" — *absent*. The person's surface is a
        // different listener, bound to loopback, and this is the property that makes the
        // `if mine` on forty match arms something the shape of the program guarantees.
        let state = paired();
        let showing = Mutex::new(None);
        for path in ["/", "/models", "/settings", "/install", "/start"] {
            let said = answer(
                asking("GET", path, Some("a-long-secret-nobody-guesses"), ""),
                &state,
                &showing,
            );
            assert_eq!(said.status, 403, "{path} must not be on the Host's door");
        }
    }

    #[test]
    fn every_route_but_one_needs_the_secret() {
        let state = paired();
        let showing = Mutex::new(None);
        for (method, path) in [
            ("GET", "/have"),
            ("POST", "/ask"),
            ("POST", "/release"),
            ("POST", "/show"),
            ("GET", "/easel/schema"),
            ("POST", "/easel/draw"),
        ] {
            assert_eq!(
                answer(asking(method, path, None, "{}"), &state, &showing).status,
                403,
                "{path} answered without a bearer"
            );
            assert_eq!(
                answer(asking(method, path, Some("wrong"), "{}"), &state, &showing).status,
                403,
                "{path} answered a wrong bearer"
            );
        }
    }

    /// **One code admits one machine, even when several arrive together.**
    ///
    /// The check and the taking were two statements with the lock released in between, so two
    /// requests could both be admitted by one invitation. Reproduced on the other end of this
    /// same handshake as `[true, true]`.
    ///
    /// ## The first version of this test passed against the defect
    ///
    /// It spawned eight threads and summed the answers, which is a race nobody runs: spawning
    /// takes longer than the window, so the threads arrived one after another and every round
    /// admitted exactly one whichever shape `take` had. A test that passes against the bug it
    /// was written for is worth less than no test, because it is also a claim that the bug is
    /// gone.
    ///
    /// So the threads are held at a barrier and released together, and it runs many rounds:
    /// a window of microseconds is hit by arriving simultaneously and repeatedly, not by
    /// arriving eight times. Checked against the two-statement version, which fails here.
    #[test]
    fn one_invitation_admits_one_machine() {
        const HANDS: usize = 8;
        for round in 0..300 {
            let showing = Mutex::new(Some(crate::invite::Code::fresh()));
            let right = {
                let held = showing.lock().expect("the code");
                held.as_ref()
                    .expect("a code is showing")
                    .as_str()
                    .to_owned()
            };
            let together = std::sync::Barrier::new(HANDS);

            let admitted: usize = std::thread::scope(|team| {
                let all: Vec<_> = (0..HANDS)
                    .map(|_| {
                        let right = right.clone();
                        let showing = &showing;
                        let together = &together;
                        team.spawn(move || {
                            together.wait();
                            usize::from(take(showing, &right))
                        })
                    })
                    .collect();
                all.into_iter().map(|one| one.join().unwrap_or(0)).sum()
            });

            assert_eq!(admitted, 1, "one code, one machine (round {round})");
            assert!(
                showing.lock().expect("the code").is_none(),
                "and it is gone afterwards (round {round})"
            );
        }
    }

    /// A wrong answer is counted, never taken: the attempt limit is what governs guessing, and
    /// burning the invitation on a typo would hand anybody on the network a way to end pairing.
    #[test]
    fn a_wrong_code_does_not_burn_the_invitation() {
        let showing = Mutex::new(Some(crate::invite::Code::fresh()));
        assert!(!take(&showing, "ZZZZZZ"));
        assert!(
            showing.lock().expect("the code").is_some(),
            "a wrong answer must leave the code on screen"
        );
    }

    #[test]
    fn enrolling_needs_the_code_that_is_on_screen() {
        let state = Mutex::new(Bond::default());
        let showing = Mutex::new(Some(crate::invite::Code::fresh()));
        let wrong = serde_json::json!({ "code": "ZZZZZZ", "secret": "s", "host": "h" }).to_string();
        assert_eq!(
            answer(asking("POST", "/enrol", None, &wrong), &state, &showing).status,
            403
        );
        // And it is still showing, because a wrong answer must not spend somebody else's code.
        assert!(showing.lock().expect("still there").is_some());
    }
}
