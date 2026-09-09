//! The only way in.
//!
//! ## What this program answers, and to whom
//!
//! Two surfaces on one listener, and they are not the same audience:
//!
//! | path | who | guarded by |
//! |---|---|---|
//! | `GET /` · `POST /pair` · `POST /unpair` | the person at this machine | **loopback only** |
//! | `POST /ask` · `GET /have` | the paired Host | **the bearer** |
//!
//! The split matters. The person's surface accepts no bearer and is refused off-loopback, so a
//! machine on the network cannot reach the page that disconnects this one. The Host's surface
//! accepts no substitute for the secret, so **loopback is not authentication and neither is a
//! LAN** — the same sentence Epoch's own door states, and the reason this program exists rather
//! than `OLLAMA_HOST=0.0.0.0`.
//!
//! ## The bearer is compared in constant time
//!
//! A comparison that stops at the first wrong byte tells whoever is guessing how much of the
//! secret was right, one request at a time.

use epoch_kernel::{Ask, Told};

use crate::keep::{self, Bond};

/// Where this program listens **for the Host**, encrypted. Fixed rather than configurable: the
/// Host learns it from the `Hello`, and a port somebody has to agree on twice is a port they can
/// disagree about.
pub const PORT: u16 = 11500;

/// Where this program serves **its own window**, in clear text, on loopback and nowhere else.
///
/// A different port from [`PORT`] because the network one now speaks TLS and a webview shown a
/// self-signed certificate refuses it — correctly, and with a dialog nobody should be taught to
/// click through. Keeping the *network* port unchanged and moving the internal one is the right
/// way round: 11500 is what the other machine knows.
pub const WINDOW_PORT: u16 = 11499;

/// Where an Epoch Host listens for a machine redeeming a code.
///
/// **A different port from this program's**, and it has to be: the two run on different machines
/// and both would otherwise be `11500` in the user's head. Kept here so the person typing only
/// ever needs the Host's address — an IP off the Host's own screen, and nothing else.
///
/// Must stay equal to `epoch_engine::pairing::HOST_PORT`. Not shared through the Kernel because
/// a port is a fact about a program, not a domain type — and this program may not link the
/// Engine (see `main.rs`).
pub const HOST_PORT: u16 = 11501;

/// The Host's pairing door, from whatever the person typed.
///
/// A person reads `192.168.1.10` off the other screen. Requiring them to also know a scheme and
/// a port is three chances to be wrong for no information gained — so both are filled in, and a
/// person who *did* type them keeps what they typed.
fn door_at(typed: &str) -> String {
    let typed = typed.trim().trim_end_matches('/');
    let (scheme, rest) = match typed.split_once("://") {
        Some((scheme, rest)) => (scheme, rest),
        // **`https`**, because the Host's pairing door is encrypted — the exchange that crosses
        // it carries the long secret, which is the one thing on this wire that is worth stealing
        // permanently. A person who typed `http://` keeps what they typed and will be told the
        // connection failed, which is better than quietly sending a credential in clear text.
        None => ("https", typed),
    };
    // A port is present only if what follows the last colon is entirely digits — otherwise the
    // colon belongs to an IPv6 address, and appending a port to one is how that breaks.
    let has_port = rest
        .rsplit_once(':')
        .is_some_and(|(_, tail)| !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()));
    if has_port {
        format!("{scheme}://{rest}")
    } else {
        format!("{scheme}://{rest}:{HOST_PORT}")
    }
}

/// Whether a request came from this machine.
///
/// The person's surface is loopback-only, so a machine on the network cannot reach the page
/// that would disconnect this one.
///
/// **Loopback is where the request came from, not who sent it.** A page in the user's browser
/// reaches 127.0.0.1 as easily as this program's own window does, so this is one of three
/// conditions rather than the whole of `mine` — see [`from_our_window`].
pub fn from_here(request: &tiny_http::Request) -> bool {
    request
        .remote_addr()
        .map(|addr| addr.ip().is_loopback())
        .unwrap_or(false)
}

fn header<'a>(request: &'a tiny_http::Request, name: &'static str) -> Option<&'a str> {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .map(|h| h.value.as_str())
}

/// The addresses this program's own window uses, and the only ones a browser may name.
///
/// [`WINDOW_PORT`], not [`PORT`]: the page is served on loopback and the network port speaks TLS
/// to a program that is not a browser at all.
fn ours() -> [String; 3] {
    [
        format!("http://127.0.0.1:{WINDOW_PORT}"),
        format!("http://localhost:{WINDOW_PORT}"),
        format!("http://[::1]:{WINDOW_PORT}"),
    ]
}

/// Whether this request could have come from the window this program opened.
///
/// ## Three conditions, and none of them leans on another
///
/// 1. **It arrived on loopback.** Nothing from the network reaches the person's surface.
/// 2. **It named us the way we are addressable.** `Host` must be a loopback name on this port.
///    This is the DNS-rebinding gate: a page at `evil.example` whose name resolves to
///    `127.0.0.1` arrives on loopback and says `Host: evil.example`, which is not one of ours.
/// 3. **It carries no `Origin` a browser did not get from us.** Any cross-origin `POST` carries
///    one — that is the Fetch specification, not an implementation detail — so a page cannot
///    post a form here and have it act as the person sitting at this machine.
///
/// A missing `Origin` is allowed, and deliberately: `curl` on this machine sends none, and a
/// local program can already do anything this program can. What is being kept out is a *page*.
///
/// **Why there is no CSRF token on top of this.** It was in the audit's recommendation and it
/// would defend against a browser that omits `Origin` on a cross-origin `POST`, which is a
/// browser that does not exist. Forty forms carrying a hidden field, and a token to rotate and
/// store, to close a hole nothing can walk through, is the control that changes nothing — the
/// dead `Manual` mode of ADR-0027 wearing a security hat. If a browser ever ships that omission,
/// this is where it goes.
pub fn from_our_window(request: &tiny_http::Request) -> bool {
    ours_to_answer(
        from_here(request),
        header(request, "host"),
        header(request, "origin"),
    )
}

/// The three conditions, decided from the three facts.
///
/// Separate from the request so it can be *tested*: `tiny_http::Request` has no public
/// constructor, so a check written against one is a check nothing can exercise. The adapter
/// above is the only part that cannot be, and it has no decision left in it.
pub fn ours_to_answer(loopback: bool, host: Option<&str>, origin: Option<&str>) -> bool {
    if !loopback {
        return false;
    }
    let named_us = match host {
        // HTTP/1.1 requires `Host`; something that omits it is not a browser, and it already
        // reached us on loopback.
        None => true,
        Some(host) => {
            let host = host.trim();
            let (name, port) = match host.rsplit_once(':') {
                // An IPv6 literal keeps its brackets and has colons of its own; the port is only
                // a port if what follows the last colon is entirely digits.
                Some((name, tail))
                    if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) =>
                {
                    (name, Some(tail))
                }
                _ => (host, None),
            };
            let loopback_name = matches!(name, "127.0.0.1" | "localhost" | "[::1]" | "::1");
            let right_port = port.is_none_or(|p| p == WINDOW_PORT.to_string());
            loopback_name && right_port
        }
    };
    if !named_us {
        return false;
    }
    match origin {
        None => true,
        Some(origin) => ours().iter().any(|one| one == origin.trim()),
    }
}

/// Whether an offered bearer is the secret this machine was given.
///
/// **Constant time**, and `false` for an unpaired machine — before pairing there is no secret,
/// so there is nothing that can be right.
///
/// Takes the string rather than the request: the Host's door is [`epoch_wire::wire`] now, not
/// `tiny_http`, and a check that could only be asked about one library's request type is a check
/// that has to be written twice the day there are two.
pub fn bearer_matches(bond: &Bond, offered: Option<&str>) -> bool {
    if !bond.paired() {
        return false;
    }
    let offered = offered.unwrap_or_default().trim();
    let a = bond.secret.as_bytes();
    let b = offered.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut wrong = 0u8;
    for (x, y) in a.iter().zip(b) {
        wrong |= x ^ y;
    }
    wrong == 0
}

/// Hand one composed turn to the local runtime and bring back what it said.
///
/// **Nothing is remembered.** The Host composes the whole conversation every turn, so there is
/// no session here to resume and nothing to lose when this machine is unplugged (ADR-0029 §2).
///
/// Tools are declared so the model can reach for them, and what it reached for is **reported,
/// never run** — the Host judges through Trust and executes (ADR-0029 §3).
/// How long to wait to **reach** a runtime on this machine.
///
/// Separate from the ten minutes allowed for an answer, because they are two questions. *Is
/// that program running?* is decided in a moment on loopback; *has the model finished loading?*
/// is what the ten minutes are for. Sharing one number is what made a switched-off program cost
/// exactly as much as a working one — and the Host, waiting on this, showed a blinking caret
/// for thirty-five minutes and named nothing.
///
/// Two seconds rather than one: this is loopback, and a server mid-start can be slow to accept
/// without being absent.
const REACH: std::time::Duration = std::time::Duration::from_secs(2);

/// What a person calls one of the programs, from its id.
///
/// Three names rather than a lookup somewhere else: this program already knows which three it
/// resolves, and an id it has never heard of is best reported as itself.
fn plainly(id: &str) -> &str {
    match id {
        "ollama" => "Ollama",
        "llama_cpp" => "llama.cpp",
        "lm_studio" => "LM Studio",
        other => other,
    }
}

/// What to tell the Host when a runtime here did not answer.
///
/// **Two facts, because they have two fixes.** This machine answered — the Host reached it, or
/// it would not be reading this — and the program on it did not. Saying only *"the model did not
/// answer"* sends somebody to check a network that is working: measured, the Host reached this
/// machine in 80 ms while the runtime had not been started at all.
///
/// The transport's own words are kept on the end. They are the only account of what actually
/// happened, and this program does not know better than they do.
fn not_answering(runner: &str, endpoint: &str, err: ureq::Error) -> String {
    format!(
        "{} is not answering on this machine ({endpoint}). This machine is reachable; that \
         program is not running here, or is still starting. ({err})",
        plainly(runner),
    )
}

/// The turn's canonical parameters, in Ollama's own vocabulary.
///
/// **Translated, never relayed.** ADR-0026: a Character holds the value and each Provider says
/// it in its own words — `context_tokens` is Epoch's word, `num_ctx` is Ollama's. They were not
/// being said at all, so a turn composed against a large window was handed to a runtime using
/// its own few-thousand default.
///
/// Nothing is guessed here. An absent value stays absent, and the runtime's default stands.
fn options(turn: &Ask) -> serde_json::Value {
    let mut options = serde_json::Map::new();
    if let Some(temperature) = turn.parameters.temperature {
        options.insert("temperature".into(), serde_json::json!(temperature));
    }
    if let Some(top_p) = turn.parameters.top_p {
        options.insert("top_p".into(), serde_json::json!(top_p));
    }
    // Resolved by the Host, which is the side that composed the turn and can measure it. A
    // second copy of that arithmetic here would be two answers to one question, and this
    // program would have to be redeployed to change either.
    if let Some(window) = turn.parameters.context_tokens {
        options.insert("num_ctx".into(), serde_json::json!(window));
    }
    serde_json::Value::Object(options)
}

/// Let go of a model now, rather than when Ollama's own timer notices.
///
/// **Ollama only, and that is the honest scope.** llama.cpp holds its model for the life of the
/// process and LM Studio unloads on its own idle setting; neither has a request that means
/// *drop it now*. A `POST /release` that pretended otherwise would report success for something
/// that did not happen.
///
/// Ollama's documented way to do this is a generate call with `keep_alive: 0` and no prompt.
/// The difference it makes is a graphics card that is free when the conversation ends instead
/// of one that is free a few minutes later — which on a lent machine is somebody else's memory.
pub fn release(model: &str, runner: Option<&str>) -> Result<(), String> {
    // **Only where there is a timer to beat.** A turn that ran on LM Studio used to end with
    // Ollama being asked to unload a model it has never heard of, because the request carried a
    // model name and no runner. Nothing failed loudly: a 404 over here, the weights still
    // resident over there, and the Host believing it had let go.
    match crate::here::runner_at(runner) {
        Some((crate::here::Dialect::Ollama, _)) => {}
        // Neither of the others unloads on request. Saying so is the answer; doing nothing and
        // reporting success would be the same lie one layer down.
        Some(_) => return Ok(()),
        None => {
            return Err(format!(
                "this machine does not run '{}'",
                runner.unwrap_or("ollama")
            ))
        }
    }

    ureq::post(&format!("{}/api/generate", crate::here::ollama()))
        .timeout(std::time::Duration::from_secs(30))
        .send_json(serde_json::json!({ "model": model, "keep_alive": 0 }))
        .map(|_| ())
        .map_err(|err| format!("the runtime on this machine did not answer: {err}"))
}

/// What one model on this machine says about itself.
///
/// **Asked here rather than guessed on the Host.** The crew editor drew vision, audio, tools and
/// thinking for a model on the Host's own computer and drew nothing at all for the same model on
/// a lent one — which reads as *it cannot see*, and was never asked. A capability is a fact
/// about the model, so it is measured where the model is.
///
/// `None` for everything the runtime will not say. llama.cpp and LM Studio publish an id and
/// nothing else, so a model served by either answers **unasked** rather than *no* — the
/// cold-instrument rule, one network away.
pub fn shown(model: &str, runner: Option<&str>) -> epoch_kernel::Shown {
    let Some((dialect, endpoint)) = crate::here::runner_at(runner) else {
        return epoch_kernel::Shown::default();
    };
    if dialect != crate::here::Dialect::Ollama {
        return epoch_kernel::Shown::default();
    }

    let Ok(answer) = ureq::post(&format!("{endpoint}/api/show"))
        .timeout(std::time::Duration::from_secs(10))
        .send_json(serde_json::json!({ "model": model }))
    else {
        return epoch_kernel::Shown::default();
    };
    let Ok(value) = answer.into_json::<serde_json::Value>() else {
        return epoch_kernel::Shown::default();
    };

    epoch_kernel::Shown {
        window: window_in(&value),
        can: declared_in(&value),
    }
}

/// How much a model holds, from `/api/show`.
///
/// Ollama reports it as `<family>.context_length` inside `model_info`, and the family differs
/// per model — so the key is found by suffix rather than by a table of families somebody would
/// have to keep current.
fn window_in(shown: &serde_json::Value) -> Option<u32> {
    let info = shown.get("model_info")?.as_object()?;
    info.iter()
        .find(|(key, _)| key.ends_with(".context_length"))
        .and_then(|(_, value)| value.as_u64())
        .map(|n| n.min(u32::MAX as u64) as u32)
}

/// What a model publishes about itself, from `/api/show`.
fn declared_in(shown: &serde_json::Value) -> Option<epoch_kernel::Declared> {
    let listed = shown.get("capabilities")?.as_array()?;
    let has = |what: &str| listed.iter().any(|c| c.as_str() == Some(what));
    Some(epoch_kernel::Declared {
        sees: has("vision"),
        hears: has("audio"),
        // `Some`, because Ollama really does publish this per model — so `false` here means no,
        // not unasked. The two are a different answer everywhere else (a backend that describes a
        // model's modalities and says nothing about tools must not be read as a refusal), and
        // this is the one door where the question is genuinely answered.
        uses_tools: Some(has("tools")),
        thinks: has("thinking"),
    })
}

/// One capability, in the shape a model can actually read.
///
/// **This was the worst of the three, and the quietest.** `turn.tools` is a `Vec<Descriptor>` -
/// Epoch's canonical vocabulary - and it was being handed to Ollama verbatim: `id`, `summary`,
/// `effects`, `reversal`, and a `parameters` that is a *list* where a JSON Schema wants an
/// object. Nothing failed. Ollama accepted the field, the model was told about tools in a shape
/// it could not parse, and every turn over the bridge came back as prose about not being able
/// to do anything.
///
/// The same translation the local Provider does, because it is the same runtime on the other
/// end. ADR-0026 in one line: the Character holds the value, the Provider says it in its own
/// words - and a bridge is a Provider whose runtime happens to be somebody else's.
fn tool(descriptor: &epoch_kernel::Descriptor) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for parameter in &descriptor.parameters {
        properties.insert(
            parameter.name.clone(),
            serde_json::json!({
                "type": match parameter.kind {
                    epoch_kernel::ValueKind::Text => "string",
                    epoch_kernel::ValueKind::Integer => "integer",
                    epoch_kernel::ValueKind::Number => "number",
                    epoch_kernel::ValueKind::Boolean => "boolean",
                },
                "description": parameter.description,
            }),
        );
        if parameter.required {
            required.push(parameter.name.clone());
        }
    }

    serde_json::json!({
        "type": "function",
        "function": {
            "name": descriptor.id.as_str(),
            "description": descriptor.summary,
            "parameters": {
                "type": "object",
                "properties": properties,
                "required": required,
            },
        },
    })
}

/// The request this machine will make of its own runtime, from the turn that arrived.
///
/// **Its own function because everything that has ever been wrong here was an absence.** Twice
/// now a field was simply not on the wire - a window, a keep-alive - and from outside that is
/// indistinguishable from a model that was told and ignored it. A shape a test can hold still is
/// the cheapest way to stop paying for that twice more.
fn body_for(turn: &Ask) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": turn.model,
        "messages": wire(&turn.conversation),
        "stream": false,
        "tools": turn.tools.iter().map(tool).collect::<Vec<_>>(),
        "options": options(turn),
    });

    /*
        **How long to keep it, from the machine that decided.**

        This used to send nothing, so Ollama here applied its own five-minute default and a
        model stayed resident on a lent graphics card long after the Host believed it had let
        go — with *Run several crew members at once* switched off on the Host the whole time.
        Measured on a MacBook Pro: 8.3 GB still held after the reply had arrived.

        Absent when the Host did not say. An older Host has no opinion to relay, and inventing
        one would be this program overruling a decision that is not its own.
    */
    if let Some(seconds) = turn.keep_loaded_seconds {
        body["keep_alive"] = serde_json::json!(seconds);
    }

    /*
        **How much the character deliberates**, which is part of who they are.

        `reasoning` is a canonical Character parameter (ADR-0026) and it was arriving here in
        every turn - this file simply never said it to Ollama, so a Guardian set to `Max` and one
        set to `Off` were handed to the far model identically. The local Provider has said it
        since the day reasoning existed; the bridge had a second, quieter copy of the wire that
        did not.

        Absent when the character says nothing, exactly as the local one does: `think` on a model
        that has no thinking to do is a request the runtime should never have been given.
    */
    if let Some(reasoning) = turn.parameters.reasoning {
        body["think"] = serde_json::json!(reasoning != epoch_kernel::Reasoning::Off);
    }

    body
}

/// The same turn, in the dialect llama.cpp and LM Studio both speak.
///
/// **Deliberately smaller than the Ollama one, and that is honest rather than lazy.** Three
/// things do not cross:
///
/// - `num_ctx` — an OpenAI-compatible server takes its window at *startup* (`llama-server -c`,
///   LM Studio's loader). A request cannot move it, so sending one would be a value that looks
///   applied and is not.
/// - `keep_alive` — neither unloads on a timer. LM Studio has its own idle setting and
///   llama.cpp holds the model for the life of the process.
/// - `think` — llama.cpp has grown reasoning fields and they differ by build. A field guessed
///   from memory is exactly what the last four defects were.
///
/// The turn's identity still crosses in full, because that is what a character *is*: the
/// composed prompt, the temperature, the top_p and the tools.
fn openai_body_for(turn: &Ask) -> serde_json::Value {
    let mut body = serde_json::json!({
        "model": turn.model,
        "messages": wire(&turn.conversation),
        "stream": false,
    });
    if !turn.tools.is_empty() {
        body["tools"] = serde_json::json!(turn.tools.iter().map(tool).collect::<Vec<_>>());
    }
    if let Some(temperature) = turn.parameters.temperature {
        body["temperature"] = serde_json::json!(temperature);
    }
    if let Some(top_p) = turn.parameters.top_p {
        body["top_p"] = serde_json::json!(top_p);
    }
    body
}

/// What an OpenAI-compatible server said.
///
/// One difference worth naming: `tool_calls[].function.arguments` arrives as a **JSON string**
/// here and as an object from Ollama. Handing the string through as a single text argument
/// would have produced a tool call that looked well-formed and named nothing the capability
/// takes — the quiet kind of wrong this file has already paid for once.
fn openai_told(answered: ureq::Response) -> Result<Told, String> {
    #[derive(serde::Deserialize)]
    struct Function {
        name: String,
        #[serde(default)]
        arguments: serde_json::Value,
    }
    #[derive(serde::Deserialize)]
    struct Call {
        function: Function,
    }
    #[derive(serde::Deserialize)]
    struct Message {
        #[serde(default)]
        content: Option<String>,
        #[serde(default)]
        tool_calls: Vec<Call>,
    }
    #[derive(serde::Deserialize)]
    struct Choice {
        message: Message,
    }
    #[derive(serde::Deserialize)]
    struct Answer {
        #[serde(default)]
        choices: Vec<Choice>,
    }

    let answer: Answer = answered
        .into_json()
        .map_err(|err| format!("the model answered something unreadable: {err}"))?;

    let Some(choice) = answer.choices.into_iter().next() else {
        return Err("the model on this machine answered with no choices".to_owned());
    };

    Ok(Told {
        text: choice.message.content.unwrap_or_default(),
        calls: choice
            .message
            .tool_calls
            .into_iter()
            .filter_map(|call| {
                let raw = match call.function.arguments {
                    // The string form. Unparseable is empty rather than one text field named
                    // after nothing.
                    serde_json::Value::String(text) => {
                        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)
                    }
                    other => other,
                };
                Some(epoch_kernel::ToolCall {
                    capability: epoch_kernel::CapabilityId::new(&call.function.name).ok()?,
                    arguments: arguments(raw),
                })
            })
            .collect(),
    })
}

pub fn ask(turn: &Ask) -> Result<Told, String> {
    /*
        **Which program on this machine, resolved here.**

        The turn names a runner by id, from a closed set; this file turns that into a loopback
        address. A turn that could carry a URL would be a turn that could point this program at
        something else — the one property that makes a companion program safer than
        `OLLAMA_HOST=0.0.0.0`.
    */
    let runner = turn.runner.as_deref().unwrap_or("ollama").to_owned();
    let (dialect, endpoint) = crate::here::runner_at(turn.runner.as_deref()).ok_or_else(|| {
        format!(
            "this machine does not run '{}'",
            turn.runner.as_deref().unwrap_or("ollama")
        )
    })?;

    if dialect == crate::here::Dialect::OpenAi {
        // **Say what the turn needs, before asking for it.**
        //
        // `openai_body_for` explains why this cannot ride the request: an OpenAI-compatible
        // server takes its window at startup, and a field in the body would be a value that
        // looks applied and is not (measured — four spellings, four 200s, no change). What that
        // reasoning left out is the other half: since a request cannot move it, something must
        // move it first, and only the machine holding the weights can. That is this program.
        //
        // Without it, an 8.6k-token turn into LM Studio's default 8192 comes back **502** and
        // the Host is told the lent machine refused — measured over the Bridge, on the same
        // model that answered fine locally.
        //
        // The Host resolved the number and it arrives on the turn (`bridge::ask_for`), so
        // nothing is computed twice.
        //
        // **And a failure is said, not swallowed.** Measured on a machine already holding two
        // other models: `lms` refused with *"this model requires approximately 9.61 GB of
        // memory"*, the turn went ahead against 8192 and came back 502, and the Host was told
        // only that the lent machine refused — true, and useless. The Host does swallow it,
        // because a smaller window there merely degrades an answer; here it ends the turn.
        if let Some(needed) = turn.parameters.context_tokens {
            epoch_models::runtimes::make_room(&endpoint, &turn.model, needed)?;
        }
        let answered = ureq::builder()
            .timeout_connect(REACH)
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .post(&format!("{endpoint}/v1/chat/completions"))
            .send_json(openai_body_for(turn))
            .map_err(|err| not_answering(&runner, &endpoint, err))?;
        return openai_told(answered);
    }

    let body = body_for(turn);

    let answered = ureq::builder()
        .timeout_connect(REACH)
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .post(&format!("{endpoint}/api/chat"))
        .send_json(body)
        .map_err(|err| not_answering(&runner, &endpoint, err))?;

    #[derive(serde::Deserialize)]
    struct Message {
        #[serde(default)]
        content: String,
        #[serde(default)]
        tool_calls: Vec<Call>,
    }
    #[derive(serde::Deserialize)]
    struct Call {
        function: Function,
    }
    #[derive(serde::Deserialize)]
    struct Function {
        name: String,
        #[serde(default)]
        arguments: serde_json::Value,
    }
    #[derive(serde::Deserialize)]
    struct Answer {
        message: Message,
    }

    let answer: Answer = answered
        .into_json()
        .map_err(|err| format!("the model answered something unreadable: {err}"))?;

    Ok(Told {
        text: answer.message.content,
        calls: answer
            .message
            .tool_calls
            .into_iter()
            .filter_map(|call| {
                Some(epoch_kernel::ToolCall {
                    capability: epoch_kernel::CapabilityId::new(&call.function.name).ok()?,
                    arguments: arguments(call.function.arguments),
                })
            })
            .collect(),
    })
}

/// What a model asked for, in the Kernel's words.
///
/// **Reported, never run.** These cross back to the Host, which judges them through Trust and
/// executes them there (ADR-0029 §3) — so nothing here validates or believes them, and a
/// capability this machine has never heard of travels fine.
///
/// Written here rather than shared with the Engine's identical conversion, because sharing it
/// would mean linking the Engine — and that is the boundary this program exists to keep.
fn arguments(raw: serde_json::Value) -> epoch_kernel::Arguments {
    let mut given = epoch_kernel::Arguments::new();
    let Some(fields) = raw.as_object() else {
        return given;
    };
    for (key, value) in fields {
        given = given.with(
            key,
            match value {
                serde_json::Value::Bool(b) => epoch_kernel::Value::Boolean(*b),
                serde_json::Value::Number(n) => n
                    .as_i64()
                    .map(epoch_kernel::Value::Integer)
                    .unwrap_or_else(|| epoch_kernel::Value::Text(n.to_string())),
                serde_json::Value::String(s) => epoch_kernel::Value::Text(s.clone()),
                other => epoch_kernel::Value::Text(other.to_string()),
            },
        );
    }
    given
}

/// A conversation in the shape the local runtime takes.
fn wire(conversation: &epoch_kernel::Conversation) -> Vec<serde_json::Value> {
    conversation
        .messages
        .iter()
        .map(|message| {
            serde_json::json!({
                "role": match message.role {
                    epoch_kernel::Role::System => "system",
                    epoch_kernel::Role::User => "user",
                    epoch_kernel::Role::Assistant => "assistant",
                    epoch_kernel::Role::Tool => "tool",
                },
                "content": message.content,
            })
        })
        .collect()
}

/// Redeem a code with a Host, and keep what it gives back.
///
/// **This machine dials out.** The Host never needs to find it: the `Hello` carries this
/// machine's own address, so a person types a six-character code and nothing else.
pub fn redeem(host: &str, code: &str) -> Result<Bond, String> {
    if host.trim().is_empty() {
        return Err("say where Epoch is running".to_owned());
    }
    let host = door_at(host);

    // **Which certificate this machine will answer with**, said while the code still stands.
    // Everything the Host does afterwards accepts that one and nothing else, so this is not a
    // detail of the greeting — it is the whole of what replaces a certificate authority.
    let identity = epoch_wire::tls::identity(&keep::dir())?;
    let hello = epoch_kernel::Hello {
        code: code.trim().to_uppercase(),
        name: crate::here::name(),
        address: crate::here::address(PORT),
        fingerprint: identity.fingerprint(),
        have: crate::here::measure(),
    };

    // The Host is not known yet either, so its certificate is *recorded* rather than checked.
    // That is trust on first use and it is bounded by the five minutes of the code; from the
    // next connection on, this machine has nothing to say to the Host at all — the bridge only
    // ever runs the other way — so there is nothing left here to pin.
    let (tls, _seen) = epoch_wire::tls::noting();
    let welcome: epoch_kernel::Welcome = ureq::builder()
        .tls_config(tls)
        .build()
        .post(&format!("{host}/pair"))
        .timeout(std::time::Duration::from_secs(15))
        .send_json(serde_json::to_value(&hello).map_err(|err| err.to_string())?)
        .map_err(|err| match err {
            ureq::Error::Status(403, _) => {
                "that code is wrong or has expired. Show a new one on the Host.".to_owned()
            }
            other => format!("could not reach Epoch at {host}: {other}"),
        })?
        .into_json()
        .map_err(|err| format!("Epoch answered something unreadable: {err}"))?;

    let bond = Bond {
        id: welcome.id,
        secret: welcome.secret,
        host,
    };
    keep::write(&bond)?;
    Ok(bond)
}

#[cfg(test)]
mod reaching {
    /// **Two questions, and they were sharing one number.** A runtime that is not running is
    /// answered on loopback in moments; a model loading deserves ten minutes. Measured: the
    /// Host waited thirty-five minutes on a lent machine whose LM Studio was not started, while
    /// that machine answered the Host in 80 ms.
    #[test]
    fn reaching_a_runtime_is_not_worth_ten_minutes() {
        assert!(super::REACH < std::time::Duration::from_secs(10));
    }

    /// **Two facts, because they have two fixes.** *"The model did not answer"* sends somebody
    /// to check a network that is working.
    #[test]
    fn a_runtime_that_is_not_running_says_so_and_says_which() {
        // A real refusal from a real socket: nothing listens on port 1.
        let refused = ureq::builder()
            .timeout_connect(super::REACH)
            .build()
            .get("http://127.0.0.1:1/v1/models")
            .call()
            .expect_err("nothing listens there");
        let said = super::not_answering("lm_studio", "http://127.0.0.1:1234", refused);
        assert!(said.contains("LM Studio"), "{said}");
        assert!(said.contains("127.0.0.1:1234"), "{said}");
        // The machine is reachable — the Host is reading this — and the program is not.
        assert!(said.contains("reachable"), "{said}");
    }

    /// An id this build has never heard of is reported as itself rather than as nothing.
    #[test]
    fn an_unknown_runner_is_named_as_itself() {
        assert_eq!(super::plainly("vllm"), "vllm");
        assert_eq!(super::plainly("llama_cpp"), "llama.cpp");
    }
}

#[cfg(test)]
mod tests {
    use super::ours_to_answer;

    #[test]
    fn a_page_cannot_act_as_the_person_at_this_machine() {
        let ok = format!("http://127.0.0.1:{}", super::WINDOW_PORT);
        let host = format!("127.0.0.1:{}", super::WINDOW_PORT);

        // This program's own window: loopback, addressed as itself, origin its own.
        assert!(ours_to_answer(true, Some(&host), Some(&ok)));
        // `curl` on this machine sends neither header, and may.
        assert!(ours_to_answer(true, None, None));

        // A page somewhere else, posting a form at loopback. The browser says whose page it is.
        assert!(!ours_to_answer(
            true,
            Some(&host),
            Some("http://evil.example")
        ));
        // Its own machine's name on another port is still another page.
        assert!(!ours_to_answer(
            true,
            Some(&host),
            Some("http://localhost:3000")
        ));

        // DNS rebinding: the name resolved to 127.0.0.1, so it *is* on loopback — and it
        // addressed us by a name that is not one of ours. Caught without the origin header.
        assert!(!ours_to_answer(true, Some("evil.example"), None));
        assert!(!ours_to_answer(true, Some("evil.example:11499"), Some(&ok)));

        // And nothing off this machine reaches the person's surface at all.
        assert!(!ours_to_answer(false, Some(&host), Some(&ok)));
    }

    #[test]
    fn an_ipv6_loopback_keeps_its_colons() {
        // The port is only a port when what follows the last colon is digits — otherwise
        // `[::1]` splits at its own address and stops being loopback.
        assert!(ours_to_answer(true, Some("[::1]"), None));
        assert!(ours_to_answer(
            true,
            Some(&format!("[::1]:{}", super::WINDOW_PORT)),
            None
        ));
        assert!(!ours_to_answer(true, Some("[::1]:3000"), None));
    }

    /// Everything a character *is* has to survive the crossing.
    ///
    /// The Mac lends the model; the Host makes the person. So identity, deliberation and how
    /// much window the turn needs are decided on the Host and said here - and each one of them
    /// has been missing from this wire at some point, silently.
    #[test]
    fn a_character_arrives_whole() {
        let turn = epoch_kernel::Ask {
            model: "gemma4:12b".into(),
            runner: None,
            conversation: epoch_kernel::Conversation {
                messages: vec![
                    epoch_kernel::Message::system("You are Mage. You check the seals."),
                    epoch_kernel::Message::user("hola"),
                ],
            },
            parameters: epoch_kernel::Parameters {
                context_policy: None,
                temperature: Some(0.2),
                top_p: Some(0.85),
                context_tokens: Some(16384),
                reasoning: Some(epoch_kernel::Reasoning::High),
            },
            tools: Vec::new(),
            keep_loaded_seconds: Some(0),
        };

        let body = super::body_for(&turn);

        // Who they are: the composed prompt, in full. The Host builds it; nothing here edits it.
        let messages = body["messages"].as_array().expect("messages travel");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert!(
            messages[0]["content"]
                .as_str()
                .unwrap()
                .contains("checks the seals")
                || messages[0]["content"]
                    .as_str()
                    .unwrap()
                    .contains("check the seals")
        );

        // How they behave, in Ollama's own words (ADR-0026: the Character holds the value, the
        // Provider translates it).
        assert_eq!(body["options"]["temperature"], 0.2);
        assert_eq!(body["options"]["top_p"], 0.85);
        assert_eq!(body["options"]["num_ctx"], 16384);

        // How much they deliberate. This was arriving in every turn and never being said.
        assert_eq!(body["think"], true);

        // And what to do with the card afterwards, which is the Host's call about someone
        // else's machine.
        assert_eq!(body["keep_alive"], 0);
    }

    #[test]
    fn nothing_absent_is_invented() {
        // A character that says nothing about itself must leave the runtime's own defaults
        // alone. Filling them in here would be this program deciding who somebody is.
        let turn = epoch_kernel::Ask {
            model: "gemma4:12b".into(),
            runner: None,
            conversation: epoch_kernel::Conversation {
                messages: vec![epoch_kernel::Message::user("hola")],
            },
            parameters: Default::default(),
            tools: Vec::new(),
            keep_loaded_seconds: None,
        };

        let body = super::body_for(&turn);
        assert!(body.get("think").is_none(), "no opinion is not `off`");
        assert!(body.get("keep_alive").is_none(), "an older Host never said");
        assert_eq!(
            body["options"].as_object().expect("an object").len(),
            0,
            "an empty set, not a set of guesses"
        );
    }

    #[test]
    fn reasoning_off_is_a_decision_and_is_said() {
        // Off is not silence. Somebody who turned deliberation off has said something, and a
        // model that thinks anyway is not the character they configured.
        let turn = epoch_kernel::Ask {
            model: "gemma4:12b".into(),
            runner: None,
            conversation: epoch_kernel::Conversation {
                messages: vec![epoch_kernel::Message::user("hola")],
            },
            parameters: epoch_kernel::Parameters {
                context_policy: None,
                reasoning: Some(epoch_kernel::Reasoning::Off),
                ..Default::default()
            },
            tools: Vec::new(),
            keep_loaded_seconds: Some(0),
        };
        assert_eq!(super::body_for(&turn)["think"], false);
    }

    #[test]
    fn what_a_character_may_use_travels_as_the_tools_it_was_given() {
        // Capabilities, MCP servers and skills all reach a model the same way: as declarations
        // in the turn. They *run* on the Host - this machine lends a model and never touches a
        // World - so what crosses is the offer, never the execution.
        let turn = epoch_kernel::Ask {
            model: "gemma4:12b".into(),
            runner: None,
            conversation: epoch_kernel::Conversation {
                messages: vec![epoch_kernel::Message::user("read the file")],
            },
            parameters: Default::default(),
            tools: vec![epoch_kernel::Descriptor::observing(
                epoch_kernel::CapabilityId::new("read_file").expect("a known capability"),
                "Read a file in the project.",
            )],
            keep_loaded_seconds: Some(0),
        };

        let body = super::body_for(&turn);
        let tools = body["tools"].as_array().expect("tools travel");
        assert_eq!(tools.len(), 1);

        // **In Ollama's shape, not Epoch's.** They were crossing as raw `Descriptor`s - `id`,
        // `summary`, `effects`, and a `parameters` list where a schema wants an object. Nothing
        // failed; the model was simply told about its tools in a language it does not read.
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "read_file");
        assert_eq!(
            tools[0]["function"]["description"],
            "Read a file in the project."
        );
        assert_eq!(tools[0]["function"]["parameters"]["type"], "object");

        // And none of Epoch's own vocabulary leaks into a request meant for a runtime.
        let raw = serde_json::to_string(&tools[0]).unwrap();
        assert!(!raw.contains("summary"), "{raw}");
        assert!(!raw.contains("reversal"), "{raw}");
        assert!(!raw.contains("effects"), "{raw}");
    }

    use super::*;

    #[test]
    fn letting_go_asks_the_program_that_is_actually_holding_it() {
        // A turn that ran on LM Studio used to end with **Ollama** being asked to unload a model
        // it has never heard of, because the request carried a model name and no runner. A 404
        // over here, the weights still resident over there, and the Host believing it had let
        // go. Nothing reports a release, which is what made it the quietest defect of the week.
        //
        // Neither of the other two unloads on request, so the honest answer is to do nothing and
        // say so — never to post at Ollama on their behalf.
        assert!(release("google/gemma-4-e4b", Some("lm_studio")).is_ok());
        assert!(release("anything", Some("llama_cpp")).is_ok());

        // And a runtime this machine does not have is refused rather than quietly becoming the
        // default, exactly as an `Ask` is.
        assert!(release("anything", Some("vllm")).is_err());
    }

    #[test]
    fn an_unpaired_machine_admits_nobody() {
        // Before pairing there is no secret, so there is nothing that can be right — and a
        // program that admitted an empty bearer would admit everybody.
        let bond = Bond::default();
        assert!(!bond.paired());
        // `admitted` needs a request, so the property is asserted at its root: the guard's first
        // branch is the unpaired one.
        assert_eq!(bond.secret, "");
    }

    #[test]
    fn a_person_types_an_address_and_the_rest_is_filled_in() {
        // What is read off the other screen is an IP. A scheme and a port would be two more
        // chances to be wrong for no information gained — and the scheme that is filled in is
        // `https`, because what crosses this exchange is the long secret.
        assert_eq!(door_at("192.168.1.10"), "https://192.168.1.10:11501");
        assert_eq!(door_at("192.168.1.10/"), "https://192.168.1.10:11501");
        // And somebody who did type them keeps what they typed, including a scheme that will
        // fail: quietly upgrading it would hide that the address was wrong, and quietly
        // *honouring* it would send a credential in clear text.
        assert_eq!(door_at("http://10.0.0.2:9000"), "http://10.0.0.2:9000");
        assert_eq!(door_at("https://desk.local"), "https://desk.local:11501");
    }

    #[test]
    fn a_colon_in_an_address_is_not_always_a_port() {
        // An IPv6 address is mostly colons, and appending a port to one is how that breaks.
        assert_eq!(door_at("[fe80::1]"), "https://[fe80::1]:11501");
    }

    #[test]
    fn a_host_that_was_never_named_is_refused_before_the_network_is_touched() {
        assert!(redeem("", "K7M2QX").is_err());
        assert!(redeem("   ", "K7M2QX").is_err());
    }
}
