//! A Provider whose machine is somebody else's (ADR-0029).
//!
//! ## What is actually new here
//!
//! Nothing about the turn. The Composer builds the same `Conversation`, Trust judges the same
//! calls, the Chronicle keeps the same record. The only difference is that the thinking happened
//! on a machine across the room — so this is a **transport**, and it is deliberately the
//! thinnest Provider in the Engine.
//!
//! ## Why not simply point Ollama's Provider at the other machine's IP
//!
//! Because that requires `OLLAMA_HOST=0.0.0.0` over there, which lends the card to the Host
//! *and* to everything else on that network. EpochServices exists so the answer to "who may use
//! this card" is a secret rather than a subnet, and this is the side that carries the secret.
//!
//! ## What crosses, and what does not
//!
//! Crossing: the model name, the composed conversation, the parameters, the tool declarations.
//! Not crossing: Quests, Chronicles, Knowledge, Trust, credentials for anything else. The remote
//! keeps nothing — it is asked, it answers, and unplugging it loses no state (ADR-0029 §2).
//!
//! **Tool calls come back reported, never run.** The remote has no Trust and no capabilities;
//! what a model over there reached for is judged and executed *here*, exactly as if it had been
//! thought here.

use std::time::Duration;

use epoch_kernel::{Ask, Have, Told};

use crate::provider::{
    Answer, Chunk, Declared, Provider, ProviderError, ProviderStatus, Request, Surface,
};

/// How long to wait for a remote machine to **answer** one turn.
///
/// Generous, because the thing being waited for is a model loading on a card that may have been
/// idle — the same reason the local Provider is patient.
const PATIENCE: Duration = Duration::from_secs(600);

/// How long to wait to **reach** it at all.
///
/// ## Two questions, and they were sharing one number
///
/// *Is that machine there?* is answered in a moment. *Has the model finished loading?* deserves
/// ten minutes. Both used [`PATIENCE`], so a machine that was switched off cost exactly as much
/// as a model that was working — and a turn does several rounds, so the waits stacked.
///
/// Measured, and it is why this exists: a character was assigned to a lent machine's LM Studio
/// while that program was not running. The turn hung for **thirty-five minutes** behind a
/// blinking caret, STOP could not interrupt it (the turn loop's only seam is between rounds),
/// and nothing named the machine or the program. The machine itself was answering this one in
/// **80 ms** the whole time.
///
/// Five seconds rather than one: a paired machine may be across a slow link, and refusing a
/// working Bridge to save four seconds would be the opposite mistake.
const REACH: Duration = Duration::from_secs(5);

/// A machine on the network that agreed to think for this one — **and one program on it**.
///
/// ## Why a machine can be several Providers
///
/// The shape a person means is `Provider · Brain · Model`: the device, the program that runs
/// the model, the model. Two of those already existed — a Provider id and a model name — and
/// the middle one did not, so `macbook + LM Studio` was not something Epoch could say.
///
/// It is expressed by there being **one Provider per (machine, program)** rather than by a new
/// term on the turn. That is not a workaround; it is what the local side has always looked
/// like. `ADD AS A SERVICE` on LM Studio here makes a second Backend at a second address on the
/// same computer, and the character editor already groups Services by machine. A remote machine
/// behaving differently would have been the odd one out.
///
/// The default Provider keeps the id it always had (`bridge:<machine>`), which is why every
/// character saved before this still resolves.
pub struct Bridge {
    id: String,
    name: String,
    address: String,
    /// What the computer is called, as a person named it. Kept apart from `name`, which is the
    /// program: one machine now runs several.
    machine: String,
    secret: String,
    /// The certificate that machine answers with, learned when it was paired.
    ///
    /// `None` is a bond made before bridges were encrypted. It is **not** permission to accept
    /// whatever answers: see [`Bridge::dial`].
    pin: Option<String>,
    /// Which program over there should run the turn — an id from the closed set, never an
    /// address. `None` is *that machine's default*, which is what every Host meant before this
    /// existed and what EpochServices still resolves to its Ollama.
    runner: Option<String>,
}

impl Bridge {
    /// Build the Provider for one paired machine, using whatever it runs by default.
    ///
    /// The secret is read once, at registry build time, and held only here — the same shape
    /// every keyed backend uses, and the reason nothing above this line ever holds a bearer.
    pub fn to(paired: &crate::pairing::Paired, secret: String) -> Self {
        Self {
            id: format!("bridge:{}", paired.id),
            // **The program, not the computer.** This answered with the machine's name while a
            // machine was one program, and the crew editor's `Brain` list duly offered
            // `studio-mac.local` as something to think with. The machine has its own
            // field now; this one says what runs there — and the default Provider is that
            // machine's Ollama, which is what EpochServices resolves an unnamed runner to.
            name: runner_name("ollama"),
            machine: paired.name.clone(),
            address: paired.address.trim_end_matches('/').to_owned(),
            secret,
            pin: paired.fingerprint.clone(),
            runner: None,
        }
    }

    /// The same machine, addressed at one named program on it.
    ///
    /// The id is the machine's with the program appended, so it stays derivable from two facts
    /// the user chose rather than being a third thing to store and keep in step.
    pub fn to_runner(paired: &crate::pairing::Paired, secret: String, runner: &str) -> Self {
        Self {
            id: format!("bridge:{}:{runner}", paired.id),
            name: runner_name(runner),
            runner: Some(runner.to_owned()),
            ..Self::to(paired, secret)
        }
    }

    /// A client that will speak to that machine and to nothing pretending to be it.
    ///
    /// **A bond with no fingerprint cannot connect at all.** The alternative — accepting any
    /// certificate when none was recorded — would mean the encryption bought nothing precisely
    /// on the bonds nobody has looked at since. Anything on the network could answer at that
    /// address, be believed, and be handed the whole composed turn.
    ///
    /// It costs one code to fix, and the sentence says so.
    fn dial(&self) -> Result<ureq::AgentBuilder, String> {
        dial_to(&self.address, self.pin.as_deref(), &self.machine)
    }

    /// The far machine could not be used — **in Epoch's words, not the transport's**.
    ///
    /// ## Somebody else's error message is not a sentence for a person
    ///
    /// This used to append `ureq`'s own text to the address. Measured in the window on
    /// 2026-09-07 by stopping EpochServices on the MacBook part-way through an answer, what a
    /// person read was a correct sentence followed by a doubled *Network Error*, the address a
    /// second time, and **a link to rustls' documentation**. The turn ended in under a second
    /// and named the right machine; the noise buried both facts.
    ///
    /// ## And the kind alone is not enough, which is why this is not a two-line match
    ///
    /// Measured against the real MacBook (`kinds_a_person_reads`), `ureq` answers
    /// **`ConnectionFailed` for two different situations**:
    ///
    /// | what happened | kind | what `ureq` says |
    /// |---|---|---|
    /// | nothing listening on that port | `ConnectionFailed` | `Connect error: connection timed out` |
    /// | the certificate is not the pinned one | `ConnectionFailed` | *tls connection init failed: …* |
    /// | the name does not resolve | `Dns` | `No such host is known` |
    /// | it stopped answering part-way | `BadStatus` | `peer closed connection without sending TLS close_notify` |
    ///
    /// So a mapping written from the kind alone would report **a certificate that changed** as a
    /// machine that is switched off: it would send somebody to power on a computer that is
    /// already running, and say nothing about the one fact worth saying. The pinning case is
    /// told apart by the sentence `epoch-wire` itself wrote, shared as a constant so the two
    /// ends cannot drift.
    fn unreachable(&self, why: &ureq::Error) -> ProviderError {
        ProviderError::Unreachable {
            // The machine, because that is what somebody has to go and switch on.
            provider: format!("{} on {}", self.name, self.machine),
            // The address, and only the address.
            endpoint: self.address.clone(),
            because: because_of(why),
        }
    }

    /// When Epoch has something of its own to add, and nothing when it does not.
    fn refused_to_serialise(&self, detail: impl std::fmt::Display) -> ProviderError {
        ProviderError::Refused {
            provider: self.name.clone(),
            detail: format!("the turn could not be written for the wire: {detail}"),
        }
    }

    /// Ask the remote what it currently has.
    /// What that machine has, asked of it.
    ///
    /*
        **`timeout` does not bound a machine that is switched off, and this had only `timeout`.**

        `provider.rs` has carried the reason since it was written: the whole-call clock has not
        started yet, because there is no call until there is a socket. A firewall that *drops*
        rather than refuses leaves `connect` on the operating system's retry schedule, and only
        `timeout_connect` bounds that.

        Measured in the window with the crew's MacBook paired and switched off: 21.1 s per
        attempt to `192.168.1.20:11500`, and `who_can_time` — which asks this once for the machine
        and again through every Bridge built from it — took 88.2 s. Parallelising the survey
        halved it; the other half was here, five seconds after the five seconds this line says.

        The same rule, written down in one file and not applied in the one next to it. `REACH`
        was already the number and was already about exactly this question.
    */
    fn have(&self) -> Result<Have, String> {
        self.dial()?
            .timeout_connect(REACH)
            .timeout(REACH)
            .build()
            .get(&format!("{}/have", self.address))
            .set("Authorization", &format!("Bearer {}", self.secret))
            .call()
            .map_err(|err| match err {
                // The one refusal EpochServices gives for everything. Saying *what* it means
                // here is this side's job: from here there is exactly one reason a bearer we
                // hold stops working.
                ureq::Error::Status(403, _) => {
                    "this machine no longer accepts our key — pair it again".to_owned()
                }
                // **The same rule as the turn's, because this is read in the same way.**
                // This string becomes `ProviderStatus.note` — the sentence under a machine on
                // the MACHINES and CONNECTIONS decks. It used to be `ureq`'s own text, so a
                // deck could show a person a doubled kind and a link to rustls' documentation.
                ref other => format!(
                    "{} could not be reached at {}{}",
                    self.machine,
                    self.address,
                    because_of(other)
                ),
            })?
            .into_json()
            .map_err(|err| format!("answered something unreadable: {err}"))
    }
}

impl Provider for Bridge {
    fn id(&self) -> &str {
        &self.id
    }

    fn probe(&self) -> ProviderStatus {
        let (online, models, note) = match self.have() {
            Ok(have) => {
                // **This program's shelf, not the machine's.** A machine with Ollama and LM
                // Studio holds different files in each, and offering one runtime's models
                // against the other is how a turn gets routed to a model that is not there.
                //
                // `models` is the fallback for an EpochServices too old to itemise, where the
                // one list *was* the answer.
                let wanted = self.runner.as_deref().unwrap_or("ollama");
                let runner = have.runners.iter().find(|runner| runner.id == wanted);
                let models = match self.runner.as_deref() {
                    None if have.runners.is_empty() => have.models,
                    _ => runner
                        .map(|runner| runner.models.clone())
                        .unwrap_or_default(),
                };
                /*
                    **Empty is three different facts, and this said one of them.**

                    It read *reachable, but no models are installed on it* whenever the list came
                    back empty — and on the owner's MacBook the truth was that Ollama is
                    installed there and **not running**. Two facts with two fixes, shown as one:
                    `ollama pull` is useless advice to somebody whose runtime is switched off.

                    Nothing new crosses the Bridge for this. `Runner` has carried `installed` and
                    `serving` since it existed, measured on that machine; the Host simply was not
                    reading them. The local Ollama Provider has told these apart all along, which
                    is what made the lent one wrong rather than merely quiet.

                    An EpochServices too old to itemise its runners has nothing to be read, so it
                    keeps the sentence that was true of it — unasked is not the same as no.
                */
                let note = if !models.is_empty() {
                    None
                } else if let Some(runner) = runner {
                    Some(if !runner.installed {
                        format!("reachable, but {} is not on that machine", runner.name)
                    } else if !runner.serving {
                        format!(
                            "reachable, but {} is not running on that machine",
                            runner.name
                        )
                    } else {
                        "reachable, but no models are installed on it".to_owned()
                    })
                } else {
                    Some("reachable, but no models are installed on it".to_owned())
                };
                (true, models, note)
            }
            Err(why) => (false, Vec::new(), Some(why)),
        };
        ProviderStatus {
            id: self.id.clone(),
            name: self.name.clone(),
            machine: Some(self.machine.clone()),
            endpoint: self.address.clone(),
            online,
            // **Not local.** It costs no money, but it is another machine — and `local` is what
            // the disclosure question keys on. Calling somebody else's computer "this PC" would
            // be the surface lying about where a turn went.
            local: false,
            models,
            note,
        }
    }

    /// What one of that machine's models says about itself.
    ///
    /// **It was answering nothing, and nothing reads as *no*.** The crew editor drew vision,
    /// audio, tools and thinking for `gemma4:12b` on this computer and drew an empty space for
    /// the same model on the MacBook — not because it had been asked and refused, but because a
    /// Bridge never implemented this and took the trait's silent default.
    ///
    /// One call, and the far machine answers for the runtime this Provider addresses. No
    /// controls: `num_ctx` is a knob on a runtime somebody else owns, and the canonical
    /// parameters (ADR-0026) already travel in the turn.
    fn surface(&self, model: &str) -> Surface {
        // `timeout_connect` for the same reason `have` needed it: a machine that is switched off
        // is not slow to answer, it is slow to *connect*, and this one is on the turn path.
        let Ok(dialling) = self.dial() else {
            // A bond with nothing to pin answers nothing about a model, which is *unasked* — the
            // same reading as an unreachable machine. What the user is told about it belongs on
            // the turn, where there is somewhere to say it.
            return Surface::default();
        };
        let shown: epoch_kernel::Shown = match dialling
            .timeout_connect(REACH)
            .timeout(Duration::from_secs(15))
            .build()
            .post(&format!("{}/show", self.address))
            .set("Authorization", &format!("Bearer {}", self.secret))
            .send_json(serde_json::json!({ "model": model, "runner": self.runner }))
            .ok()
            .and_then(|answer| answer.into_json().ok())
        {
            Some(shown) => shown,
            // Unreachable, or an EpochServices too old to have the route. Both are *unasked*.
            None => return Surface::default(),
        };
        Surface {
            window: shown.window,
            controls: Vec::new(),
            can: shown.can,
        }
    }

    fn declares(&self, model: &str) -> Option<Declared> {
        self.surface(model).can
    }

    fn take_turn(
        &self,
        request: &Request,
        sink: &mut dyn FnMut(Chunk),
    ) -> Result<Answer, ProviderError> {
        if request.model.trim().is_empty() {
            return Err(ProviderError::NoModel {
                provider: self.name.clone(),
            });
        }

        let ask = ask_for(request, self.runner.as_deref());

        // The last bridge turn, written where a person can read it.
        //
        // The same instrument the local Provider has, for the same reason: from outside, "the
        // far model was never told" and "it was told and ignored it" look identical. That
        // ambiguity has now cost two evenings — once for tool declarations, once for a window
        // and a keep-alive that were never on the wire at all.
        //
        // A separate file from `last-turn.json` deliberately. They are different wires, and
        // the whole class of defect here is *one of the two* being out of date; sharing a file
        // would hide exactly the thing it exists to show.
        trace(&ask);

        let told: Told = self
            .dial()
            .map_err(|why| ProviderError::Refused {
                provider: self.name.clone(),
                detail: why,
            })?
            .timeout_connect(REACH)
            .timeout(PATIENCE)
            .build()
            .post(&format!("{}/ask", self.address))
            .set("Authorization", &format!("Bearer {}", self.secret))
            // **Not `unreachable`.** Failing to write our own request says nothing about the
            // far machine, and a notice sending somebody to look at a computer that is fine is
            // the same defect as one naming the wrong program.
            .send_json(serde_json::to_value(&ask).map_err(|err| self.refused_to_serialise(err))?)
            .map_err(|err| match err {
                ureq::Error::Status(403, _) => ProviderError::Refused {
                    provider: self.name.clone(),
                    detail: "it no longer accepts our key — pair it again".to_owned(),
                },
                ureq::Error::Status(code, _) => ProviderError::Refused {
                    provider: self.name.clone(),
                    detail: format!("it answered {code}"),
                },
                ref other => self.unreachable(other),
            })?
            .into_json()
            .map_err(|err| ProviderError::Unreadable {
                provider: self.name.clone(),
                detail: err.to_string(),
            })?;

        // **One chunk, and that is honest.** A turn over the bridge arrives whole, so the World
        // shows it whole rather than fake-typing it out. Streaming across the wire is a real
        // improvement and belongs on the far side first; pretending to stream here would be the
        // UI inventing information the Engine does not have.
        if !told.text.is_empty() {
            sink(Chunk::Token(told.text.clone()));
        }
        sink(Chunk::Done);

        Ok(Answer {
            text: told.text,
            calls: told.calls,
            // The far machine's own Engine measured this and does not yet send it across. `None`
            // is *not asked*: a rate invented on this side would describe the network.
            pace: None,
        })
    }

    /// Ask the far machine to let go of the model now.
    ///
    /// `keep_loaded_seconds` on the turn already tells it to unload, but only once *its* timer
    /// notices. This asks now — the same thing the local Provider does, and the difference
    /// between a borrowed graphics card that is free when the conversation ends and one that is
    /// free a few minutes later.
    ///
    /// Best-effort and silent: an older EpochServices has no such route, and a Host that
    /// complained about it would be reporting somebody else's version as a fault of this turn.
    /// Let go of a model on the far machine — **on the program that is actually holding it**.
    ///
    /// This sent a model name and nothing else, from the days when a machine was one program. So
    /// finishing a turn on the MacBook's LM Studio told that machine to release the model, and
    /// that machine asked its **Ollama** to unload something Ollama has never heard of: a 404
    /// over there, LM Studio still holding the weights, and Epoch believing it had let go.
    ///
    /// The same failure shape as everything else this week — a surface saying something it did
    /// not measure — and the quietest instance of it, because nothing reports a release.
    /// Whether that model is **on the far machine's card right now**, measured there.
    ///
    /// ## Why this existed as a silent `None` for as long as Bridges have
    ///
    /// The trait's default answers `None`, which is *unasked* -- correct for a hosted backend
    /// that holds nothing of the user's. A Bridge took that default without ever asking, so the
    /// KEEP control, which is hidden when there is no reading, was absent for every character
    /// whose brain lives on another computer.
    ///
    /// It is the identical omission `surface` had, in this same file, and its comment already
    /// says the shape: *it was answering nothing, and nothing reads as no*. The holding worked
    /// the whole time -- `keep_loaded` crosses on the turn and the far machine honours it,
    /// measured with 8.1 GB resident on a MacBook between turns. What was missing was the
    /// reading, and therefore the switch.
    ///
    /// ## The runner, not the machine
    ///
    /// A machine is several Providers, one per program it serves, and each holds its own
    /// models. Asking *is this on that computer* would light the lamp for a model LM Studio has
    /// while this Bridge routes to Ollama. So the answer comes from **this Bridge's own runner**,
    /// and a Bridge with no runner named reads the machine's default list, exactly as everything
    /// else here does.
    ///
    /// ## What each answer means
    ///
    /// - `None` -- the machine could not be reached, or its EpochServices is old enough that it
    ///   was never asked. Neither of those is *the model is not loaded*.
    /// - `Some(false)` -- asked, and it is not on the card.
    /// - `Some(true)` -- asked, and it is.
    fn resident(&self, model: &str) -> Option<bool> {
        resident_in(&self.have().ok()?, self.runner.as_deref(), model)
    }

    fn release(&self, model: &str) {
        let Ok(dialling) = self.dial() else { return };
        let _ = dialling
            .timeout_connect(REACH)
            .timeout(Duration::from_secs(10))
            .build()
            .post(&format!("{}/release", self.address))
            .set("Authorization", &format!("Bearer {}", self.secret))
            .send_json(serde_json::json!({ "model": model, "runner": self.runner }));
    }
}

/// Every paired machine that may think, as Providers.
///
/// A machine without [`crate::Grant::Compute`] is left out rather than built and refused: a
/// Provider that exists and is never allowed to answer is one that can be probed by accident,
/// which is the same reasoning `Backends::registry` uses for a disabled backend.
///
/// A machine whose secret cannot be read is also left out — and that is not silence, because
/// its absence from the Services list is exactly as visible as a machine that is offline.
pub fn bridges(
    pairings: &crate::Pairings,
    secrets: &crate::secrets::Secrets,
) -> Vec<Box<dyn Provider>> {
    pairings
        .all()
        .iter()
        .filter(|paired| paired.may(crate::Grant::Compute))
        .flat_map(|paired| {
            let Some(secret) = secrets.get(&crate::pairing::secret_name(&paired.id)) else {
                return Vec::new();
            };
            let secret = secret.expose().to_owned();

            // The machine itself, at whatever it runs by default. Always present: it is the id
            // every character saved before this names, and a machine that has never been probed
            // has nothing itemised to offer instead.
            let mut built: Vec<Box<dyn Provider>> =
                vec![Box::new(Bridge::to(paired, secret.clone()))];

            // And one more for each *other* program it was serving when last asked. `ollama` is
            // skipped because that is what the entry above already is — two Providers reaching
            // the same runtime would be one choice the user has to make twice.
            built.extend(
                paired
                    .runners
                    .iter()
                    .filter(|runner| runner.as_str() != "ollama")
                    .map(|runner| {
                        Box::new(Bridge::to_runner(paired, secret.clone(), runner))
                            as Box<dyn Provider>
                    }),
            );
            built
        })
        .collect()
}

/// What a person calls one of the programs, from its id.
///
/// Written here rather than reached for in `epoch-models` because a machine may report a runner
/// this build has never heard of — a newer EpochServices, an id added later — and the honest
/// answer then is the id itself rather than nothing.
pub fn runner_name(id: &str) -> String {
    match id {
        "ollama" => "Ollama".to_owned(),
        "llama_cpp" => "llama.cpp".to_owned(),
        "lm_studio" => "LM Studio".to_owned(),
        other => other.to_owned(),
    }
}

/// One turn, as it crosses to another machine.
///
/// Its own function so the decisions in it can be held still by a test. Both of them were
/// found by using the product, and both were *absences* — a field that was never written into
/// the JSON is exactly the kind of defect a live probe reports as "it works, but".
///
/// **What the turn needs, resolved here.** The far side runs Ollama, whose default window is a
/// few thousand tokens and does not consult the model — the same defect the local Provider
/// already fixes. Resolved on this side because this is the side that knows: the Host composed
/// the turn, so the Host can measure it, and the arithmetic stays in one place rather than
/// being redeployed to every machine that ever lends a card. An explicit `context_tokens` still
/// wins; a person asking for less has said something Epoch must not overrule.
///
/// **And how long to keep it afterwards.** Without that, the far machine kept every model warm
/// for its own five minutes while the Host believed it had let go.
fn ask_for(request: &Request, runner: Option<&str>) -> Ask {
    let mut parameters = request.parameters.clone();
    if parameters.context_tokens.is_none() {
        parameters.context_tokens = Some(crate::provider::window_for(request));
    }

    Ask {
        model: request.model.clone(),
        runner: runner.map(str::to_owned),
        conversation: request.conversation.clone(),
        parameters,
        tools: request.tools.clone(),
        keep_loaded_seconds: Some(match request.keep_loaded {
            crate::provider::KeepLoaded::Never => 0,
            crate::provider::KeepLoaded::For(d) => d.as_secs(),
        }),
    }
}

/// A client that will speak to that machine, over TLS, and to nothing pretending to be it.
///
/// One function because there were two, byte for byte, on `Bridge` and on `LentEasel` — and a
/// rule kept by being typed twice is a rule that has already been broken somewhere.
///
/// ## Two refusals, and both happen before anything is sent
///
/// **No fingerprint, no connection.** Accepting any certificate when none was recorded would
/// mean the encryption bought nothing precisely on the bonds nobody has looked at since:
/// anything on the network could answer at that address, be believed, and be handed the whole
/// composed turn. It costs one code to fix, and the sentence says so.
///
/// **And no `https`, no connection.** Configuring TLS is not the same as requiring it: `ureq`
/// given an `http` URL does not do TLS at all, so the pinned verifier is never consulted, no
/// certificate is ever compared, and the turn — the whole Chronicle, the system prompt, the
/// project's context — crosses in clear text with the bearer in a header. The pin was set on
/// every one of these calls and could be bypassed by the address alone.
///
/// A roster written before this could hold such an address, which is why it is checked here
/// rather than only where an address is typed.
fn dial_to(address: &str, pin: Option<&str>, machine: &str) -> Result<ureq::AgentBuilder, String> {
    let Some(pin) = pin.filter(|it| !it.trim().is_empty()) else {
        return Err(format!(
            "{machine} was paired before Epoch encrypted this connection, so there is nothing \
             to recognise it by. Pair it again — it takes one code."
        ));
    };
    if !address.trim().to_ascii_lowercase().starts_with("https://") {
        return Err(format!(
            "{machine} is filed at an address that is not encrypted, and a turn carries your \
             conversation. Pair it again — it takes one code."
        ));
    }
    Ok(ureq::builder().tls_config(epoch_wire::tls::pinned_to(pin)))
}

/// Which of a machine's answers the lamp over one conversation is about.
///
/// Pure, and separate from the request, because every interesting case here is a shape of
/// [`Have`] rather than a network condition — and a helper that needed a live machine to be
/// tested would be tested by nobody.
///
/// The three answers are the point:
///
/// - `None` — this program was never asked, or its answer did not include the field. Neither of
///   those is *the model is not loaded*, and a surface drawing a dark lamp for them would be
///   reporting silence as a reading.
/// - `Some(false)` — asked, and it is not on the card.
/// - `Some(true)` — asked, and it is.
fn resident_in(have: &epoch_kernel::Have, runner: Option<&str>, model: &str) -> Option<bool> {
    // The runner this Provider addresses, not the machine. Two programs on one computer hold
    // different things, and a lamp that answered *is it anywhere on that machine* would light
    // for a model LM Studio has while the turn goes to Ollama.
    let holding = match runner {
        Some(runner) => have.runners.iter().find(|one| one.id == runner)?,
        // No runner named is the machine's default program — the only shape an older
        // EpochServices could answer at all, and its first runner is that program.
        None => have.runners.first()?,
    };
    let holding = holding.resident.as_ref()?;
    Some(
        holding
            .iter()
            .any(|reported| crate::provider::same_model(reported, model)),
    )
}

/// What Epoch observed, in its own words — or nothing.
///
/// **Every sentence here is Epoch's.** None of `ureq`'s or `rustls`' text is repeated, and no
/// address, code or URL appears: the address is already in the sentence this joins, and the rest
/// is plumbing a person cannot act on. Where nothing was measured this is empty, which renders
/// exactly as the notice did before any of this existed.
///
/// The leading separator lives here rather than in the format string so a provider with nothing
/// to add cannot leave a dangling dash.
fn because_of(why: &ureq::Error) -> String {
    let ureq::Error::Transport(transport) = why else {
        return String::new();
    };
    // **The pinning refusal first, because it wears `ConnectionFailed`'s clothes.** Told apart
    // by the sentence `epoch-wire` wrote, shared as a constant so the two ends cannot drift.
    if transport
        .message()
        .is_some_and(|said| said.contains(epoch_wire::tls::NOT_THE_SAME_MACHINE))
        || format!("{transport}").contains(epoch_wire::tls::NOT_THE_SAME_MACHINE)
    {
        return format!(
            " — {}. Pair it again; it takes one code.",
            epoch_wire::tls::NOT_THE_SAME_MACHINE
        );
    }
    match transport.kind() {
        ureq::ErrorKind::Dns => " — that name could not be looked up on this network.".to_owned(),
        ureq::ErrorKind::ConnectionFailed => {
            " — nothing answered there. Check that machine is on and EpochServices is running."
                .to_owned()
        }
        // Reached, and then lost: a socket existed, which is a different thing to go and look at.
        // Measured by stopping EpochServices part-way through an answer — `BadStatus`, because
        // the status line never arrived.
        ureq::ErrorKind::BadStatus | ureq::ErrorKind::BadHeader | ureq::ErrorKind::Io => {
            " — it stopped answering part-way through the turn.".to_owned()
        }
        // Anything else is unmeasured, and an invented reading is worse than none.
        _ => String::new(),
    }
}

/// Write the turn that just crossed, when somebody asked to see it.
///
/// Off unless `EPOCH_TRACE_DIR` is set, and one file overwritten every turn: a diagnostic, not
/// a log. It can contain project text, so it lives beside the other vault files - on the
/// user's own machine, like everything else here.
fn trace(ask: &Ask) {
    let Ok(pretty) = serde_json::to_string_pretty(ask) else {
        return;
    };
    let Some(dir) = std::env::var_os("EPOCH_TRACE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("last-bridge-turn.json"), pretty);
}

/// Ask one paired machine what it currently has.
///
/// **Live, never remembered.** The roster keeps what a machine answered at pairing time, and a
/// machine changes: Ollama pulls a model, somebody installs the Hugging Face CLI, a card fills
/// up. Anything reporting on a remote asks it now, and says *unreachable* when it cannot —
/// which is a different answer from *has nothing*.
///
/// Deliberately outside the [`Provider`] trait: this is a question about a computer, and every
/// surface asking it would otherwise have to build a registry to reach a method on one entry.
pub fn have_of(
    paired: &crate::pairing::Paired,
    secrets: &crate::secrets::Secrets,
) -> Result<Have, String> {
    let secret = secrets
        .get(&crate::pairing::secret_name(&paired.id))
        .ok_or_else(|| "no key is held for that machine — pair it again".to_owned())?;
    Bridge::to(paired, secret.expose().to_owned()).have()
}

/// A ComfyUI on a machine that lends its card, reached through the Bridge and never directly.
///
/// **The Bridge is the only door.** A lent studio reports `127.0.0.1:8188` because that is what
/// it is *there* (`epoch_kernel::Studio`), and dialling a LAN address instead would mean asking
/// the user to open a second way into that machine — one with none of the bearer this one has.
/// So the Host compiles the graph and hands it over, and that machine runs it against its own
/// loopback. *The Host owns reality; the Bridge owns computation* (ADR-0029), and a picture is
/// computation.
pub struct LentEasel {
    address: String,
    secret: String,
    /// The certificate that machine answers with. Same rule as [`Bridge::pin`].
    pin: Option<String>,
    /// What to call the machine when something goes wrong there. A failure that says only
    /// *"could not draw"* leaves somebody guessing which computer to go and look at.
    pub machine: String,
}

impl LentEasel {
    /// Build one for a paired machine, if a key is still held for it.
    pub fn of(
        paired: &crate::pairing::Paired,
        secrets: &crate::secrets::Secrets,
    ) -> Result<Self, String> {
        let secret = secrets
            .get(&crate::pairing::secret_name(&paired.id))
            .ok_or_else(|| "no key is held for that machine — pair it again".to_owned())?;
        Ok(Self {
            address: paired.address.trim_end_matches('/').to_owned(),
            secret: secret.expose().to_owned(),
            pin: paired.fingerprint.clone(),
            machine: paired.name.clone(),
        })
    }

    /// A client that will speak to that machine and to nothing pretending to be it.
    fn dial(&self) -> Result<ureq::AgentBuilder, String> {
        dial_to(&self.address, self.pin.as_deref(), &self.machine)
    }

    /// That machine's ComfyUI schema, so the graph is compiled against the server that will run
    /// it rather than against this one.
    pub fn schema(&self) -> Result<epoch_assets::workflow::Schema, String> {
        let info: serde_json::Value = self
            .dial()?
            .timeout_connect(REACH)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .get(&format!("{}/easel/schema", self.address))
            .set("Authorization", &format!("Bearer {}", self.secret))
            .call()
            .map_err(|why| self.blame(why))?
            .into_json()
            .map_err(|why| format!("{} answered with something unreadable: {why}", self.machine))?;
        Ok(epoch_assets::workflow::Schema::read(&info))
    }

    /// Hand over the compiled graph, and get back what it drew.
    ///
    /// One call rather than four: the queue, the wait and the fetch all happen there, which also
    /// keeps a slow render off this machine's event loop entirely.
    pub fn draw_graph(
        &self,
        nodes: &impl serde::Serialize,
    ) -> Result<(Vec<u8>, String, f32), String> {
        let answered: serde_json::Value = self
            .dial()?
            .timeout_connect(REACH)
            // Long, because this is somebody else's card doing the work and the wait is the
            // point. Shorter than the Bridge's own patience so the failure is ours to explain.
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .post(&format!("{}/easel/draw", self.address))
            .set("Authorization", &format!("Bearer {}", self.secret))
            .send_json(serde_json::json!({ "prompt": nodes }))
            .map_err(|why| self.blame(why))?
            .into_json()
            .map_err(|why| format!("{} answered with something unreadable: {why}", self.machine))?;

        let png = answered["png"]
            .as_str()
            .ok_or_else(|| format!("{} drew and did not send the picture", self.machine))?;
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(png)
            .map_err(|why| {
                format!(
                    "{} sent a picture that could not be read: {why}",
                    self.machine
                )
            })?;
        let name = answered["file"]
            .as_str()
            .unwrap_or("picture.png")
            .to_owned();
        let seconds = answered["seconds"].as_f64().unwrap_or_default() as f32;
        Ok((bytes, name, seconds))
    }

    /// A refusal from there, in that machine's own words where it sent any.
    ///
    /// ComfyUI names the node, the input and the value it did not like, and the Bridge passes
    /// that through — throwing it away in favour of *"generation failed"* would discard the
    /// only thing that explains what to fix.
    fn blame(&self, why: ureq::Error) -> String {
        match why {
            ureq::Error::Status(_, response) => match response.into_string() {
                Ok(body) => serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|said| said["problem"].as_str().map(str::to_owned))
                    .unwrap_or(body),
                Err(_) => format!("{} refused and said nothing", self.machine),
            },
            // **And the transport's are not that machine's words.** The arm above passes
            // ComfyUI's own refusal through because it names the node and the value; this one
            // used to pass `ureq`'s through as if it were the same kind of thing.
            ref other => format!(
                "{} could not be reached at {}{}",
                self.machine,
                self.address,
                because_of(other)
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pairing::{Paired, Pairings};

    /// **Reaching a machine and waiting for a model are two questions.**
    ///
    /// They shared one number, and it cost thirty-five minutes behind a blinking caret when a
    /// lent machine's runtime was not running — while the machine itself answered in 80 ms.
    #[test]
    fn a_machine_that_is_not_there_is_not_worth_ten_minutes() {
        assert!(
            REACH < Duration::from_secs(15),
            "reaching a machine is decided in moments"
        );
        assert!(
            PATIENCE > REACH * 10,
            "and waiting for a model is not: {PATIENCE:?} against {REACH:?}"
        );
    }

    /// A real one, against nothing listening. It has to fail *fast* and name the machine.
    #[test]
    fn an_address_nobody_answers_fails_in_seconds_and_says_whose() {
        let gone = Paired {
            id: "gone".to_owned(),
            name: "studio-mac.local".to_owned(),
            address: "https://127.0.0.1:1".to_owned(),
            fingerprint: Some("aa".repeat(32)),
            grants: vec![crate::Grant::Compute],
            paired_at: 0,
            runners: Vec::new(),
            easels: Vec::new(),
        };
        let easel = LentEasel {
            address: gone.address.clone(),
            secret: "not-a-real-secret".to_owned(),
            pin: gone.fingerprint.clone(),
            machine: gone.name.clone(),
        };
        let started = std::time::Instant::now();
        let why = easel.schema().expect_err("nothing listens on port 1");
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "took {:?}",
            started.elapsed()
        );
        assert!(why.contains("studio-mac.local"), "{why}");
    }

    /// **Nothing anybody else wrote reaches the person.**
    ///
    /// Measured in the window on 2026-09-07: stopping EpochServices part-way through an answer
    /// put a doubled *Network Error*, the address a second time and a link to rustls'
    /// documentation into the Chronicle, after a first sentence that was already correct.
    ///
    /// This runs the real failure — nothing listens on port 1 — and asserts on what may not be
    /// in the result rather than on what may, because the next leak will be a phrase nobody has
    /// seen yet.
    #[test]
    fn a_failure_a_person_reads_carries_no_plumbing() {
        let easel = LentEasel {
            address: "https://127.0.0.1:1".to_owned(),
            secret: "not-a-real-secret".to_owned(),
            pin: Some("aa".repeat(32)),
            machine: "studio-mac.local".to_owned(),
        };
        let bridge = Bridge {
            id: "bridge:test".to_owned(),
            name: "Ollama".to_owned(),
            machine: easel.machine.clone(),
            address: easel.address.clone(),
            secret: easel.secret.clone(),
            pin: easel.pin.clone(),
            runner: Some("ollama".to_owned()),
        };
        let why = bridge
            .have()
            .expect_err("nothing listens on port 1")
            .to_string();

        // The two halves that matter, and they were both already right.
        assert!(why.contains("studio-mac.local"), "names the machine: {why}");
        assert!(
            why.contains("https://127.0.0.1:1"),
            "names the address: {why}"
        );

        // And the plumbing that was riding along with them.
        for leak in [
            "docs.rs",
            "os error",
            "close_notify",
            "Network Error: Network Error",
            "ureq",
            "rustls",
        ] {
            assert!(!why.contains(leak), "{leak:?} reached a person: {why}");
        }
        // The address once, not twice — it used to appear again inside the parenthetical.
        assert_eq!(why.matches("127.0.0.1").count(), 1, "{why}");
    }

    fn machine_with(runners: Vec<epoch_kernel::Runner>) -> epoch_kernel::Have {
        epoch_kernel::Have {
            runners,
            ..Default::default()
        }
    }

    fn runner(id: &str, resident: Option<Vec<&str>>) -> epoch_kernel::Runner {
        epoch_kernel::Runner {
            id: id.to_owned(),
            name: id.to_owned(),
            installed: true,
            serving: true,
            models: Vec::new(),
            resident: resident.map(|it| it.into_iter().map(str::to_owned).collect()),
        }
    }

    /// **The lamp reads one program's card, not the machine's.**
    ///
    /// A machine is several Providers, one per program it serves. Answering *is it anywhere on
    /// that computer* would light the lamp for a model LM Studio is holding while the turn goes
    /// to Ollama — a reading of the right quantity on the wrong instrument.
    #[test]
    fn the_lamp_is_about_the_program_the_turn_goes_to() {
        let machine = machine_with(vec![
            runner("ollama", Some(vec!["gemma4:12b"])),
            runner("lm_studio", Some(vec!["qwen3-14b"])),
        ]);
        assert_eq!(
            resident_in(&machine, Some("ollama"), "gemma4:12b"),
            Some(true)
        );
        assert_eq!(
            resident_in(&machine, Some("lm_studio"), "gemma4:12b"),
            Some(false),
            "LM Studio is not holding Ollama's model"
        );
    }

    /// **Never asked is not *not loaded*.**
    ///
    /// An EpochServices old enough to have no `resident` field answers `None`, and so does a
    /// machine that has no such program at all. Reading either as *the model is not on the card*
    /// is the inversion this codebase has paid for in `uses_tools` and in `Shown` — and here it
    /// would put a dark lamp over a model that is loaded.
    #[test]
    fn silence_from_an_older_machine_is_not_a_no() {
        let old = machine_with(vec![runner("ollama", None)]);
        assert_eq!(resident_in(&old, Some("ollama"), "gemma4:12b"), None);

        let absent = machine_with(vec![runner("ollama", Some(vec![]))]);
        assert_eq!(
            resident_in(&absent, Some("lm_studio"), "gemma4:12b"),
            None,
            "a program that machine does not run was not asked either"
        );

        // And *asked, holding nothing* is a real answer, not silence.
        assert_eq!(
            resident_in(&absent, Some("ollama"), "gemma4:12b"),
            Some(false)
        );
    }

    /// A Bridge built before runners crossed names none, and reads the machine's own program.
    #[test]
    fn a_bridge_with_no_runner_reads_the_machines_first() {
        let machine = machine_with(vec![runner("ollama", Some(vec!["gemma4:12b"]))]);
        assert_eq!(resident_in(&machine, None, "gemma4:12b"), Some(true));
        assert_eq!(resident_in(&machine_with(Vec::new()), None, "x"), None);
    }

    /// A provider with nothing to add renders exactly as it did before `because` existed.
    ///
    /// The separator lives in the string rather than in the format, so an empty one cannot leave
    /// a dangling dash — which is the kind of thing no test would otherwise notice.
    #[test]
    fn saying_nothing_leaves_no_punctuation_behind() {
        let quiet = ProviderError::Unreachable {
            provider: "Ollama".to_owned(),
            endpoint: "http://127.0.0.1:11434".to_owned(),
            because: String::new(),
        };
        assert_eq!(
            quiet.to_string(),
            "Ollama is not reachable at http://127.0.0.1:11434"
        );
    }

    /// **Which `ureq::ErrorKind` each real failure actually produces.**
    ///
    /// `#[ignore]` — it needs the crew's MacBook on and serving. It exists because the sentence
    /// a person reads is chosen from that kind, and choosing it from memory is how a notice
    /// comes to send somebody to fix a machine that was never broken.
    ///
    /// Run with `cargo test -p epoch-engine kinds_a_person_reads -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs the MacBook on and serving"]
    fn kinds_a_person_reads() {
        let mac = "https://10.0.1.20:11500";
        let real = "0dca98cbd2e2c7335c493d868359ac86e567fe37bb9af8c2d080f7accd4c1dbe";

        let kind = |address: &str, pin: &str| {
            let agent = ureq::builder()
                .tls_config(epoch_wire::tls::pinned_to(pin))
                .timeout_connect(Duration::from_secs(5))
                .timeout(Duration::from_secs(10))
                .build();
            match agent.get(&format!("{address}/have")).call() {
                Ok(_) => "answered".to_owned(),
                Err(ureq::Error::Status(code, _)) => format!("status {code}"),
                Err(ureq::Error::Transport(t)) => format!("{:?} :: {t}", t.kind()),
            }
        };

        println!(
            "nothing listening   : {}",
            kind("https://127.0.0.1:1", real)
        );
        println!(
            "name that is not    : {}",
            kind("https://nowhere.invalid:11500", real)
        );
        println!("wrong fingerprint   : {}", kind(mac, &"aa".repeat(32)));
        println!("the real one        : {}", kind(mac, real));

        // **The reason this test exists rather than a comment.** `ConnectionFailed` covers both
        // *nothing is listening* and *the certificate changed*, so a sentence chosen from the
        // kind alone would send somebody to switch on a machine that is already running.
        let said = Bridge {
            id: "bridge:test".to_owned(),
            name: "Ollama".to_owned(),
            machine: "kisloks-MacBook-Pro.local".to_owned(),
            address: mac.to_owned(),
            secret: "not-a-real-secret".to_owned(),
            pin: Some("aa".repeat(32)),
            runner: Some("ollama".to_owned()),
        }
        .have()
        .expect_err("a certificate that is not the pinned one")
        .to_string();
        println!("what a person reads : {said}");
        assert!(
            said.contains(epoch_wire::tls::NOT_THE_SAME_MACHINE),
            "a changed certificate must say so: {said}"
        );
        assert!(
            !said.contains("EpochServices is running"),
            "and must not be reported as a machine that is switched off: {said}"
        );
    }

    /// The smallest real turn: one message, and a decision about what to do afterwards.
    fn bare_turn(keep: crate::provider::KeepLoaded) -> crate::provider::Request {
        crate::provider::Request {
            model: "gemma4:12b".into(),
            conversation: epoch_kernel::Conversation {
                messages: vec![epoch_kernel::Message::user("hola")],
            },
            keep_loaded: keep,
            parameters: Default::default(),
            tuning: Default::default(),
            tools: Vec::new(),
            most_rounds: None,
        }
    }

    /// What the Host decided about somebody else's graphics card must actually leave the Host.
    #[test]
    fn the_turn_carries_how_long_the_far_machine_should_hold_the_model() {
        // The defect, measured on a MacBook Pro: *Run several crew members at once* was off,
        // the reply had arrived, and 8.3 GB was still resident. `keep_loaded` never left this
        // side — so the far machine applied Ollama's own five-minute default, and the toggle
        // governed only the Host's own runtime.
        //
        // Asserted on the wire shape rather than through a live machine: what broke was that a
        // field was absent from the JSON, and that is a thing a test can hold still.
        let request = bare_turn(crate::provider::KeepLoaded::Never);
        let ask = ask_for(&request, None);
        assert_eq!(ask.keep_loaded_seconds, Some(0), "let go now");

        let warm = crate::provider::Request {
            keep_loaded: crate::provider::KeepLoaded::For(std::time::Duration::from_secs(300)),
            ..request.clone()
        };
        assert_eq!(ask_for(&warm, None).keep_loaded_seconds, Some(300));

        // And it survives the wire, which is the half that actually failed.
        let raw = serde_json::to_string(&ask).expect("an Ask serialises");
        assert!(raw.contains("keepLoadedSeconds"), "{raw}");
    }

    #[test]
    fn the_turn_says_how_much_window_it_needs() {
        // Ollama's default is a few thousand tokens and it does not consult the model. The
        // local Provider already says this out loud; a turn sent to another machine was not,
        // so the same defect lived on across the network.
        let request = bare_turn(crate::provider::KeepLoaded::Never);
        assert!(
            ask_for(&request, None).parameters.context_tokens.is_some(),
            "a turn that says nothing gets the runtime's guess"
        );

        // An explicit request still wins: a person asking for less has said something Epoch
        // must not overrule, here or anywhere.
        let asked = crate::provider::Request {
            parameters: epoch_kernel::Parameters {
                context_tokens: Some(4096),
                ..Default::default()
            },
            ..request
        };
        assert_eq!(ask_for(&asked, None).parameters.context_tokens, Some(4096));
    }

    fn paired(id: &str, grants: Vec<crate::Grant>) -> Paired {
        Paired {
            id: id.to_owned(),
            name: "Studio Mac".to_owned(),
            address: "https://10.0.0.9:11500/".to_owned(),
            fingerprint: Some("aa".repeat(32)),
            grants,
            paired_at: 0,
            runners: Vec::new(),
            easels: Vec::new(),
        }
    }

    #[test]
    fn a_bridge_is_never_local() {
        // It costs nothing, and it is still somebody else's computer. `local` is what the
        // disclosure question keys on, so calling this "this PC" would be the surface lying
        // about where a turn went.
        let bridge = Bridge::to(&paired("b1", vec![crate::Grant::Compute]), "s".into());
        let status = bridge.probe();
        assert!(!status.local);
        assert_eq!(
            status.endpoint, "https://10.0.0.9:11500",
            "trailing slash goes"
        );
    }

    #[test]
    fn a_machine_that_may_not_think_is_not_built_at_all() {
        // Not built and refused later: a Provider that exists can be probed by accident.
        let mut pairings = Pairings::default();
        pairings.add(
            "Studio Mac",
            "https://10.0.0.9:11500",
            Some("aa".repeat(32)),
            vec![],
            0,
        );
        let secrets = crate::secrets::Secrets::at(std::path::Path::new("."));
        assert!(bridges(&pairings, &secrets).is_empty());
    }

    /// One machine, several programs on it.
    fn with_runners(id: &str, runners: &[&str]) -> Paired {
        Paired {
            runners: runners.iter().map(|r| (*r).to_owned()).collect(),
            ..paired(id, vec![crate::Grant::Compute])
        }
    }

    #[test]
    fn a_machine_is_one_provider_for_each_program_it_serves() {
        // `Provider · Brain · Model` — the device, the program, the model. The middle term is
        // expressed by there being several Providers on one machine, which is exactly what the
        // local side already looks like: LM Studio here is a second Backend at a second address
        // on the same computer.
        let mut pairings = Pairings::default();
        let id = pairings.add(
            "Studio Mac",
            "https://10.0.0.9:11500",
            Some("aa".repeat(32)),
            vec![crate::Grant::Compute],
            0,
        );
        assert!(pairings.note_runners(&id, vec!["ollama".into(), "lm_studio".into()]));

        let dir = std::env::temp_dir().join(format!("epoch-bridge-{}", crate::now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let secrets = crate::secrets::Secrets::at(&dir);
        // A locked credential store is nobody at the machine, not a fault. `epoch-secrets`
        // explains the three answers; only the middle one stops a test, and *broken* still fails.
        if let epoch_secrets::Reach::Locked(why) = secrets.reachable() {
            eprintln!("skipped: this machine's credential store is locked — {why}");
            return;
        }
        secrets
            .put(
                &crate::pairing::secret_name(&id),
                &epoch_kernel::Secret::new("s".to_owned()),
            )
            .unwrap();

        let ids: Vec<String> = bridges(&pairings, &secrets)
            .iter()
            .map(|provider| provider.id().to_owned())
            .collect();

        // The machine's default keeps the id it always had — every character saved before this
        // names it, and a rename here would silently unassign the whole crew.
        assert_eq!(
            ids,
            vec![format!("bridge:{id}"), format!("bridge:{id}:lm_studio")]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_machines_own_ollama_is_not_offered_twice() {
        // It *is* the default entry. Two Providers reaching one runtime would be one choice the
        // user has to make twice, and neither of them would look wrong.
        let paired = with_runners("b1", &["ollama"]);
        assert_eq!(
            paired
                .runners
                .iter()
                .filter(|runner| runner.as_str() != "ollama")
                .count(),
            0
        );
    }

    #[test]
    fn a_turn_carries_which_program_should_run_it() {
        // And the default carries nothing, because `None` is what every Host said before this
        // field existed and it has to keep meaning the same thing on the far side.
        let request = bare_turn(crate::provider::KeepLoaded::Never);
        assert_eq!(
            ask_for(&request, Some("lm_studio")).runner.as_deref(),
            Some("lm_studio")
        );
        assert_eq!(ask_for(&request, None).runner, None);
    }

    #[test]
    fn a_machine_offers_what_it_was_last_seen_serving_and_nothing_else() {
        // **The ordering this exists to hold.** Providers are built from `Paired.runners`, and
        // a machine nobody has asked yet has none — so a freshly paired machine is one Provider
        // until something writes down what it is serving. That writer is the probe
        // (`State::refresh_runners`), which now runs *before* the registry is built rather than
        // after, and it is deliberately the only writer.
        //
        // Before the order was fixed, PROBE AGAIN on a re-paired machine reported its Ollama
        // and never its LM Studio, however many times it was pressed: not a stale reading, but
        // a reading that could not become visible.
        let dir = std::env::temp_dir().join(format!("epoch-order-{}", crate::now_ms()));
        std::fs::create_dir_all(&dir).unwrap();
        let secrets = crate::secrets::Secrets::at(&dir);
        // A locked credential store is nobody at the machine, not a fault. `epoch-secrets`
        // explains the three answers; only the middle one stops a test, and *broken* still fails.
        if let epoch_secrets::Reach::Locked(why) = secrets.reachable() {
            eprintln!("skipped: this machine's credential store is locked — {why}");
            return;
        }

        let mut pairings = Pairings::default();
        let id = pairings.add(
            "Studio Mac",
            "https://10.0.0.9:11500",
            Some("aa".repeat(32)),
            vec![crate::Grant::Compute],
            0,
        );
        secrets
            .put(
                &crate::pairing::secret_name(&id),
                &epoch_kernel::Secret::new("s".to_owned()),
            )
            .unwrap();

        // Just paired: one Provider, because nothing has asked it anything.
        assert_eq!(bridges(&pairings, &secrets).len(), 1);

        // Somebody asked, and wrote down the answer.
        assert!(pairings.note_runners(&id, vec!["ollama".into(), "lm_studio".into()]));
        assert_eq!(bridges(&pairings, &secrets).len(), 2);

        // And a machine that stopped serving one stops offering it — the same write, backwards.
        assert!(pairings.note_runners(&id, vec!["ollama".into()]));
        assert_eq!(bridges(&pairings, &secrets).len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_named_program_is_named_where_a_person_reads_it() {
        // **The program, and the computer, as two answers.** This used to return the machine's
        // name for both, so the crew editor's `Brain` list offered `studio-mac.local`
        // as something to think with — the machine answering a question about the program.
        let studio =
            Bridge::to_runner(&with_runners("b1", &["lm_studio"]), "s".into(), "lm_studio").probe();
        assert_eq!(studio.name, "LM Studio");
        assert_eq!(studio.machine.as_deref(), Some("Studio Mac"));

        // And the machine's default Provider is that machine's Ollama, which is what
        // EpochServices resolves an unnamed runner to.
        let default = Bridge::to(&with_runners("b1", &["ollama"]), "s".into()).probe();
        assert_eq!(default.name, "Ollama");
        assert_eq!(default.machine.as_deref(), Some("Studio Mac"));
        // And an id this build has never heard of is reported as itself rather than as nothing.
        assert_eq!(runner_name("vllm"), "vllm");
    }

    #[test]
    fn an_offline_machine_says_where_it_looked() {
        // OFFLINE has to be a fact the user can go and check — the same rule every other
        // Provider's status follows.
        let bridge = Bridge::to(&paired("b1", vec![crate::Grant::Compute]), "s".into());
        let status = bridge.probe();
        assert!(!status.online, "nothing is listening on 10.0.0.9 in a test");
        assert!(status.note.is_some());
        assert!(status.endpoint.contains("10.0.0.9"));
    }
}
