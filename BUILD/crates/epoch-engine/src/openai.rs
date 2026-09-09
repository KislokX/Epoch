//! Anything that speaks the OpenAI API.
//!
//! ## One kind, not a dozen
//!
//! llama.cpp, LM Studio, vLLM, Deepseek, GLM, Groq, Together, OpenRouter — and most of whatever
//! ships next — all expose `/v1/chat/completions` and `/v1/models`. They are not a dozen
//! backends to implement; they are **one shape at a dozen addresses**.
//!
//! That turns adding a backend from a pull request into a form: an endpoint, and a key when the
//! thing on the other end wants one. It is also the answer to *"what about the one that comes out
//! tomorrow"* — if it speaks this, it already works.
//!
//! ## And that is exactly why it must not pretend to know more than it asked
//!
//! A vendor-specific Provider can be opinionated: [`crate::anthropic`] knows Anthropic's models.
//! This one cannot, because it does not know what it is talking to. So every fact it reports is
//! either measured from the answer or absent:
//!
//! - the model list is whatever `/v1/models` returned, in its order;
//! - `Declared` is **`None`** — the OpenAI API publishes no capability set, so Epoch has learned
//!   nothing and says so. Guessing from a model's *name* would put `gpt-4o` and `llama-3-vision`
//!   in a table that goes stale the day either vendor renames anything;
//! - the context window is `None` for the same reason.
//!
//! Unknown, not zero, and never a constant wearing a measurement's clothes.
//!
//! ## Streaming
//!
//! Server-sent events, `data: ` per line, terminated by `data: [DONE]`. Tool calls arrive in
//! fragments that have to be reassembled by index — the one genuinely fiddly part, and the reason
//! [`Streaming`] exists rather than a closure over three mutable locals.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::time::Duration;

use epoch_kernel::{Arguments, CapabilityId, Conversation, Descriptor, Role, Value};
use serde_json::json;

use crate::provider::Declared;
use crate::provider::BETWEEN_ROUNDS;
use crate::provider::{
    Answer, Chunk, Provider, ProviderError, ProviderStatus, Request, Surface, ToolCall,
};
use epoch_kernel::{Control, Secret};

const CONNECT_TIMEOUT: Duration = Duration::from_millis(700);
const PROBE_TIMEOUT: Duration = Duration::from_millis(2_500);
const TURN_TIMEOUT: Duration = Duration::from_secs(600);

/// A backend reached over the OpenAI API.
pub struct OpenAi {
    id: String,
    endpoint: String,
    key: Option<Secret>,
}

impl OpenAi {
    /// Ask a server for its model list at `path`, or learn nothing.
    ///
    /// `None` is **unasked**: a server that is not answering, or does not serve this endpoint at
    /// all, must not be read as a server whose models cannot do anything.
    fn model_list(&self, path: &str) -> Option<serde_json::Value> {
        self.signed(
            ureq::builder()
                .timeout_connect(CONNECT_TIMEOUT)
                .timeout(PROBE_TIMEOUT)
                .build()
                .get(&format!("{}{path}", self.endpoint)),
        )
        .call()
        .ok()?
        .into_json()
        .ok()
    }

    /// How much this model **has been given**, when the server says.
    ///
    /// Not how much it could take. The two are different numbers and both servers publish both,
    /// which is exactly why this is easy to get wrong:
    ///
    /// ```text
    /// LM Studio   /api/v0/models   loaded_context_length 8192   max_context_length 262144
    /// llama.cpp   /v1/models       meta.n_ctx            82944  meta.n_ctx_train   262144
    /// ```
    ///
    /// llama.cpp was read correctly from the start — `n_ctx`, what this model was actually
    /// loaded with. LM Studio was read as `max_context_length`, and **that is a real
    /// measurement of the wrong quantity**, which is the most convincing way an instrument
    /// lies: something genuinely is being measured.
    ///
    /// What it cost, measured through the window: LM Studio loads a model at its own default of
    /// 8192 when a chat request triggers the load, Epoch composed an 8.6k-token turn against a
    /// 262144-token budget so nothing was reduced, and the same `gemma4-12b` that answered
    /// correctly on Ollama and llama.cpp had its second answer cut mid-sentence and answered a
    /// greeting to a question about files. Nothing failed. Nothing was logged. The window showed
    /// an answer.
    ///
    /// So the answer is genuinely unknown before a model is opened, and `None` says so rather
    /// than borrowing the larger number to cover it — a conservative budget is the right answer
    /// to not knowing, and an invented one is not.
    fn window_of(&self, model: &str) -> Option<u32> {
        self.model_list("/api/v0/models")
            .and_then(|body| Self::window_in(&body, model))
            .or_else(|| {
                self.model_list("/v1/models")
                    .and_then(|body| Self::window_in(&body, model))
            })
    }

    /// Make sure the model is holding a window this turn fits inside.
    ///
    /// **The work is in `epoch-models`**, because the Bridge needs the identical thing and only
    /// the machine holding the weights can ever do it: `lms` is a program on that computer, so a
    /// Host reaching across a network cannot load anything there. One implementation, called
    /// from whichever side is standing next to the server.
    fn make_room(&self, request: &Request) {
        // A person who asked for less has said something Epoch must not overrule (ADR-0026).
        let needed = request
            .parameters
            .context_tokens
            .unwrap_or_else(|| crate::provider::window_for(request));
        // Best-effort **here**, and deliberately: a smaller window degrades a local answer,
        // where a refused turn would be no answer at all. The Bridge makes the other choice,
        // because there the server refuses outright and silence becomes a 502.
        let _ = epoch_models::runtimes::make_room(&self.endpoint, &request.model, needed);
    }

    /// The window one model was **given**, out of whichever server's listing this is.
    ///
    /// Separate from the request so the choice of field has a test. It is the whole decision
    /// this function exists to make, and it is one word away from the wrong number in both
    /// servers' vocabularies.
    fn window_in(body: &serde_json::Value, model: &str) -> Option<u32> {
        let entry = body["data"]
            .as_array()?
            .iter()
            .find(|m| m["id"].as_str() == Some(model))?;
        // `loaded_context_length` is LM Studio's; `meta.n_ctx` is llama.cpp's. Their siblings —
        // `max_context_length` and `meta.n_ctx_train` — are what the model *could* hold, and
        // neither is what a turn has to fit inside.
        ["/loaded_context_length", "/meta/n_ctx"]
            .iter()
            .find_map(|at| entry.pointer(at)?.as_u64())
            .and_then(|n| u32::try_from(n).ok())
            .filter(|n| *n > 0)
            // **And what it will load with, for a model that is not loaded yet.**
            //
            // Measured 2026-09-02 against llama.cpp's router: `meta.n_ctx` is published **only
            // while a model is resident**, and Epoch asks this question on a background thread
            // about a *cold* model — deliberately, because asking a loading backend from inside
            // a turn froze the whole World. So every llama.cpp model answered `None`, every
            // character fell back to `Budget::default()`, and turns were composed against
            // **8,192 tokens** on a server holding 32,768.
            //
            // The symptom was a character that forgets what was said the moment a conversation
            // gets going, on a model the deck reports at 32K — and nothing anywhere disagreed,
            // because the gauge honestly showed the budget it had been given.
            //
            // `status.args` is the command line the router will start it with, published for
            // loaded and unloaded models alike. Second, not first: the fitter may reduce the
            // window it was asked for, so a resident model's own `n_ctx` outranks the argument
            // that produced it.
            .or_else(|| Self::window_in_args(entry))
    }

    /// The window from the command line the router publishes for a model.
    ///
    /// Both spellings, because `llama-server` accepts both and a preset writes whichever it was
    /// given. The value is the element *after* the flag: reading the flag alone would report a
    /// window whenever one was set, which is a boolean wearing a number.
    fn window_in_args(entry: &serde_json::Value) -> Option<u32> {
        let args = entry.pointer("/status/args")?.as_array()?;
        args.iter()
            .position(|a| {
                a.as_str()
                    .is_some_and(|it| it == "--ctx-size" || it == "-c")
            })
            .and_then(|at| args.get(at + 1)?.as_str())
            .and_then(|it| it.parse::<u32>().ok())
            .filter(|n| *n > 0)
    }

    /// llama.cpp's answer: `architecture.input_modalities` on each model in `/v1/models`.
    ///
    /// The same door `probe` already uses — there is only one — so this costs no new endpoint.
    fn declared_by_llama_cpp(&self, model: &str) -> Option<Declared> {
        Self::read_llama_cpp(&self.model_list("/v1/models")?, model)
    }

    /// The parsing, separated from the asking so it can be held still by a test.
    fn read_llama_cpp(body: &serde_json::Value, model: &str) -> Option<Declared> {
        let found = body["data"]
            .as_array()?
            .iter()
            .find(|m| m["id"].as_str() == Some(model))?;
        let modalities = found["architecture"]["input_modalities"].as_array()?;
        let takes = |what: &str| modalities.iter().any(|m| m.as_str() == Some(what));

        Some(Declared {
            sees: takes("image"),
            hears: takes("audio"),
            // Declared nowhere per model, so **unasked** — see `declares`.
            uses_tools: None,
            thinks: false,
        })
    }

    /// LM Studio's answer: `type` on each model in its own `/api/v0/models`.
    ///
    /// A second endpoint rather than a second guess. `vlm` is what it calls a model that takes
    /// pictures, and it says so only when the projector is actually beside the weights — which
    /// is how the shelf's missing projector became visible in the first place.
    fn declared_by_lm_studio(&self, model: &str) -> Option<Declared> {
        Self::read_lm_studio(&self.model_list("/api/v0/models")?, model)
    }

    fn read_lm_studio(body: &serde_json::Value, model: &str) -> Option<Declared> {
        let found = body["data"]
            .as_array()?
            .iter()
            .find(|m| m["id"].as_str() == Some(model))?;

        Some(Declared {
            sees: found["type"].as_str() == Some("vlm"),
            // It does not report audio per model, and `Declared` has no way to say *unasked*
            // for one field — so the conservative half is taken rather than inventing a
            // capability out of another server's vocabulary.
            hears: false,
            // **Published, and it took asking twice to find it.** `/api/v0/models` carries a
            // `capabilities` array, and `tool_use` in it is a real yes — measured on this
            // machine, where `qwen3-14b` declares it and `gemma4-26b` does not.
            //
            // The *absence* is not a no. `gpt-oss-20b` has no `capabilities` key at all and
            // takes tool calls perfectly well, so a missing array is unasked. This field used
            // to say `false` unconditionally and the surface read it as a refusal, leaving
            // every character on this backend with no tools whatsoever.
            uses_tools: found["capabilities"]
                .as_array()
                .filter(|caps| caps.iter().any(|c| c.as_str() == Some("tool_use")))
                .map(|_| true),
            thinks: false,
        })
    }
    /// What to call this one on screen.
    ///
    /// **The id, not the kind.** Every OpenAI-compatible backend used to report itself as
    /// `OpenAI-compatible`, so a machine running llama.cpp and LM Studio showed two identical
    /// rows distinguishable only by port number. The id is the name the user gave it — or the
    /// one Epoch gave it when adopting a runtime — and it is the only field that says *which*.
    ///
    /// The two Epoch adopts are spelled the way their makers spell them; anything else is shown
    /// as it was typed, because a name somebody chose is not ours to prettify.
    fn display_name(&self) -> String {
        match self.id.as_str() {
            "llama_cpp" => "llama.cpp".to_owned(),
            "lm_studio" => "LM Studio".to_owned(),
            other => other.to_owned(),
        }
    }

    pub fn named(id: &str, endpoint: &str) -> Self {
        Self {
            id: id.to_owned(),
            // Trailing slashes are the most common thing to paste, and a URL with two of them
            // fails in a way that reads as the server being wrong.
            //
            // And `localhost` is rewritten to the address of the same machine that does not hang
            // — see `provider::same_machine`, where the fifteen seconds is measured.
            endpoint: crate::provider::same_machine(endpoint.trim().trim_end_matches('/')),
            key: None,
        }
    }

    pub fn with_key(mut self, key: Option<Secret>) -> Self {
        self.key = key;
        self
    }

    /// Add the credential, when there is one.
    ///
    /// **Optional on purpose.** A hosted vendor demands it; a llama.cpp on the desk usually has
    /// none, and requiring one would make the local case impossible for no reason.
    fn signed(&self, request: ureq::Request) -> ureq::Request {
        match &self.key {
            Some(key) => request.set("Authorization", &format!("Bearer {}", key.expose())),
            None => request,
        }
    }

    /// The conversation, in this API's shape.
    ///
    /// Tool output used to be sent as `user` content, because this API's `tool` role needs an
    /// id pairing it to a call and the call was not kept anywhere. It is kept now
    /// (`epoch_kernel::ToolCall`), so the pairing is real — and the pairing is what decides
    /// whether a model reads the result or invents around it.
    ///
    /// The ids are generated here rather than stored: they mean nothing outside one request,
    /// and inventing a field on the Kernel to carry a vendor's correlation id would be the
    /// wire shape leaking inward. Results follow their call in order, so a queue is enough.
    fn messages(conversation: &Conversation) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let mut awaiting: std::collections::VecDeque<String> = std::collections::VecDeque::new();

        for (at, message) in conversation.messages.iter().enumerate() {
            match message.role {
                Role::Tool => {
                    match awaiting.pop_front() {
                        Some(id) => out.push(json!({
                            "role": "tool",
                            "tool_call_id": id,
                            "content": message.content,
                        })),
                        // A result with no call in front of it — a compacted conversation can
                        // produce one. `user` content is what it is: something to read.
                        None => out.push(json!({ "role": "user", "content": message.content })),
                    }
                }
                Role::Assistant if !message.calls.is_empty() => {
                    let calls: Vec<serde_json::Value> = message
                        .calls
                        .iter()
                        .enumerate()
                        .map(|(n, call)| {
                            let id = format!("call_{at}_{n}");
                            awaiting.push_back(id.clone());
                            json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": call.capability.as_str(),
                                    // A JSON *string*, which is this API's shape and not
                                    // Ollama's — the same fact, written two ways.
                                    "arguments": serde_json::to_string(&call.arguments)
                                        .unwrap_or_else(|_| "{}".into()),
                                },
                            })
                        })
                        .collect();
                    /*
                        **No prose beside a native call**, and this is not tidiness.

                        `Message::reaching` carries both: the structured calls *and* the sentence
                        `[I called see_image(…)]`, which exists for a backend that has no
                        tool-call shape. Sending both to a backend that does means the model
                        reads, in its own prior turns, assistant messages whose text is
                        `[I called …]` — and it learns to write them.

                        Measured by the owner, with a Spotify server attached: Mage answered
                        `[I called spotify_mcp_search(query: LATIN MAFIA)]` as **prose**, in the
                        middle of a sentence, having made no call at all. Epoch taught it that.

                        The fact is in `tool_calls`. The sentence is a duplicate of the fact, and
                        a duplicate a model can imitate. It still reaches a backend with no
                        native shape, through the arm below, which is the only place it was ever
                        for.
                    */
                    out.push(json!({
                        "role": "assistant",
                        "content": "",
                        "tool_calls": calls,
                    }));
                }
                role => out.push(json!({
                    "role": match role {
                        Role::System => "system",
                        Role::Assistant => "assistant",
                        _ => "user",
                    },
                    "content": message.content,
                })),
            }
        }
        out
    }

    /// Epoch's capabilities, in this API's function shape.
    fn tools(tools: &[Descriptor]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|tool| {
                let mut properties = serde_json::Map::new();
                let mut required = Vec::new();
                for parameter in &tool.parameters {
                    properties.insert(
                        parameter.name.clone(),
                        json!({
                            "type": match parameter.kind {
                                epoch_kernel::ValueKind::Integer => "integer",
                                epoch_kernel::ValueKind::Boolean => "boolean",
                                _ => "string",
                            },
                            "description": parameter.description,
                        }),
                    );
                    if parameter.required {
                        required.push(parameter.name.clone());
                    }
                }
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.id.to_string(),
                        "description": tool.summary,
                        "parameters": {
                            "type": "object",
                            "properties": properties,
                            "required": required,
                        },
                    },
                })
            })
            .collect()
    }
}

/// A tool call being assembled out of fragments.
///
/// The arguments arrive as a string split across events, and the name may come only in the first.
/// Keyed by the index the server gives, because two calls in one answer interleave.
#[derive(Default)]
struct Streaming {
    calls: BTreeMap<u64, (String, String)>,
    text: String,
    /// What the server said about its own decoding, if it said anything.
    pace: Option<crate::provider::Pace>,
}

impl Streaming {
    /// Keep whatever the server said about its own speed.
    ///
    /// **Read where it is offered, and nowhere invented.** llama.cpp puts `timings` on the final
    /// chunk of a stream; LM Studio publishes `stats` with the same quantity under a different
    /// name; an OpenAI-shaped server that reports neither leaves this `None`, which is *it did not
    /// say* rather than *it was instant*.
    ///
    /// The last one wins, because llama.cpp emits a running figure per token and the final one is
    /// the answer.
    fn timed(&mut self, event: &serde_json::Value) {
        let seen = event
            .pointer("/timings/predicted_per_second")
            .and_then(serde_json::Value::as_f64)
            .map(|per_second| {
                (
                    per_second,
                    event
                        .pointer("/timings/predicted_n")
                        .and_then(serde_json::Value::as_u64),
                )
            })
            .or_else(|| {
                // LM Studio's own name for the same measurement.
                event
                    .pointer("/stats/tokens_per_second")
                    .and_then(serde_json::Value::as_f64)
                    .map(|per_second| (per_second, None))
            });
        let Some((per_second, generated)) = seen else {
            return;
        };
        if per_second <= 0.0 {
            return;
        }
        self.pace = Some(crate::provider::Pace {
            per_second,
            generated: generated.and_then(|it| u32::try_from(it).ok()),
        });
    }

    fn take(&mut self, delta: &serde_json::Value, sink: &mut dyn FnMut(Chunk)) {
        if let Some(fragment) = delta.get("content").and_then(|c| c.as_str()) {
            if !fragment.is_empty() {
                self.text.push_str(fragment);
                sink(Chunk::Token(fragment.to_owned()));
            }
        }
        let Some(calls) = delta.get("tool_calls").and_then(|c| c.as_array()) else {
            return;
        };
        for call in calls {
            let at = call
                .get("index")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let entry = self.calls.entry(at).or_default();
            if let Some(name) = call.pointer("/function/name").and_then(|n| n.as_str()) {
                if !name.is_empty() {
                    entry.0 = name.to_owned();
                }
            }
            if let Some(part) = call.pointer("/function/arguments").and_then(|a| a.as_str()) {
                entry.1.push_str(part);
            }
        }
    }

    /// What was assembled, as an Answer.
    ///
    /// A call whose name never arrived, or whose arguments will not parse, is **dropped**. The
    /// alternative is inventing a capability id or empty arguments — and a tool invoked with
    /// arguments nobody sent is worse than a turn that said something and called nothing.
    fn finish(self) -> Answer {
        let calls = self
            .calls
            .into_values()
            .filter_map(|(name, arguments)| {
                let capability = CapabilityId::new(&name).ok()?;
                let parsed: serde_json::Value = serde_json::from_str(&arguments).ok()?;
                let mut given = Arguments::new();
                for (key, value) in parsed.as_object()?.iter() {
                    given = given.with(
                        key,
                        match value {
                            serde_json::Value::Bool(b) => Value::Boolean(*b),
                            serde_json::Value::Number(n) => n
                                .as_i64()
                                .map(Value::Integer)
                                .unwrap_or_else(|| Value::Text(n.to_string())),
                            serde_json::Value::String(s) => Value::Text(s.clone()),
                            other => Value::Text(other.to_string()),
                        },
                    );
                }
                Some(ToolCall {
                    capability,
                    arguments: given,
                })
            })
            .collect();
        Answer {
            text: self.text,
            calls,
            pace: self.pace,
        }
    }
}

impl Provider for OpenAi {
    fn id(&self) -> &str {
        &self.id
    }

    fn probe(&self) -> ProviderStatus {
        let answer: Result<serde_json::Value, _> = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(PROBE_TIMEOUT)
                    .build()
                    .get(&format!("{}/v1/models", self.endpoint)),
            )
            .call()
            .map_err(|err| err.to_string())
            .and_then(|r| r.into_json().map_err(|err| err.to_string()));

        match answer {
            Ok(value) => {
                // `data[].id`, in the order given. Not sorted, for the reason Ollama's list is
                // not: the order a backend reports is information, and alphabetical is not.
                let models: Vec<String> = value["data"]
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .filter_map(|m| m["id"].as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default();
                ProviderStatus {
                    id: self.id.clone(),
                    name: self.display_name(),
                    machine: crate::provider::machine_of(
                        &self.endpoint,
                        is_loopback(&self.endpoint),
                    ),
                    endpoint: self.endpoint.clone(),
                    online: true,
                    // **Measured from the address, not declared by the user.** A backend on
                    // loopback costs nothing and sends nothing anywhere; one at a hostname is
                    // somebody else's machine even when it is in the next room, and `sight`
                    // treats that as a disclosure decision.
                    local: is_loopback(&self.endpoint),
                    models,
                    note: None,
                }
            }
            Err(why) => ProviderStatus {
                id: self.id.clone(),
                name: self.display_name(),
                machine: crate::provider::machine_of(&self.endpoint, is_loopback(&self.endpoint)),
                endpoint: self.endpoint.clone(),
                online: false,
                local: is_loopback(&self.endpoint),
                models: Vec::new(),
                // What it actually said. A backend that is unreachable and one that refused a
                // credential are different problems, and only the server knows which.
                note: Some(why),
            },
        }
    }

    fn surface(&self, model: &str) -> Surface {
        Surface {
            // **Measured where a server offers it, and `None` everywhere else.** The OpenAI API
            // itself reports no context window, so a table of known model sizes would be a
            // constant dressed as a measurement — wrong the day any vendor renames anything.
            //
            // Both local servers do report one, and each reports two — what the model *could*
            // take and what it *was given*. `window_of` takes the second; see there for what
            // taking the first cost.
            window: self.window_of(model),
            controls: Vec::<Control>::new(),
            can: self.declares(model),
        }
    }

    /// What one model on this backend says it can do.
    ///
    /// ## Measured on 2026-08-21, and both servers answer — differently
    ///
    /// The OpenAI API itself declares nothing, which is why `surface()` still reports `None`.
    /// But the two local servers Epoch actually meets each publish it, in their own shape:
    ///
    /// ```text
    /// llama.cpp  GET /v1/models
    ///   gemma4-12b   architecture.input_modalities = ["text","image","audio"]
    ///   gemma4-26b   architecture.input_modalities = ["text"]
    ///   qwen3-14b    architecture.input_modalities = ["text"]
    ///
    /// LM Studio  GET /api/v0/models
    ///   gemma4-12b   type = "vlm"       max_context_length = 262144
    ///   qwen3-14b    type = "llm"       max_context_length = 40960
    /// ```
    ///
    /// The three text models answering `["text"]` is what makes this a reading rather than a
    /// hopeful guess: the field distinguishes, on the same server, on the same day.
    ///
    /// **`None` is unasked, never "cannot".** A vLLM or a vendor with neither endpoint is not
    /// blind — nothing was learned about it, and `who_can_see` already refuses to read silence
    /// as a refusal.
    ///
    /// `uses_tools` is `None` — **unasked** — and the difference between that and `false` cost a
    /// week of a character insisting it had no tools. Neither server declares it per model, and
    /// both take a real tool call (`finish_reason: tool_calls`, measured twice). Filling the
    /// field in from that measurement would be the invented gauge this codebase keeps deleting;
    /// filling it in with `false` was worse, because something downstream *acted* on it.
    fn declares(&self, model: &str) -> Option<Declared> {
        self.declared_by_llama_cpp(model)
            .or_else(|| self.declared_by_lm_studio(model))
    }

    /// Show one image to one model and return what it says.
    ///
    /// The wire shape is the OpenAI one — a content-part list with a `data:` URI — which is what
    /// makes this a different method from Ollama's `images` array rather than a shared one.
    ///
    /// Deliberately **not** a turn, for the reason `Ollama::describe` gives: no Conversation, no
    /// tools, no streaming, nothing entering a Chronicle. It is a translation.
    fn describe(&self, model: &str, image: &[u8], look_for: &str) -> Result<String, String> {
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(image);

        let answer: serde_json::Value = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(TURN_TIMEOUT)
                    .build()
                    .post(&format!("{}/v1/chat/completions", self.endpoint)),
            )
            .send_json(json!({
                "model": model,
                "stream": false,
                "messages": [{
                    "role": "user",
                    "content": [
                        { "type": "text", "text": look_for },
                        {
                            "type": "image_url",
                            // The media type is part of the URI and servers read it, so it is
                            // sniffed from the bytes — the same rule import follows: a picture is
                            // described by what it contains, never by what its file was called.
                            "image_url": { "url": format!("data:{};base64,{encoded}", crate::import::ImageFormat::sniff(image).map_or("image/png", |f| f.mime())) }
                        }
                    ]
                }],
                // Zero, because this is extraction rather than writing. The same picture and the
                // same question should not give two different answers to two turns.
                "temperature": 0,
                // **Generous, and it is not padding.** A reasoning model spends the budget on
                // thinking before it says anything: at 120 the answer came back empty with the
                // reasoning cut mid-sentence, and at 600 the same request described the picture.
                "max_tokens": 600,
            }))
            .map_err(|err| format!("{model} could not look at it: {err}"))?
            .into_json()
            .map_err(|err| format!("{model} answered with something unreadable: {err}"))?;

        let said = answer["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_owned();
        if said.is_empty() {
            return Err(format!("{model} looked at it and said nothing"));
        }
        Ok(said)
    }

    fn take_turn(
        &self,
        request: &Request,
        sink: &mut dyn FnMut(Chunk),
    ) -> Result<Answer, ProviderError> {
        if request.model.trim().is_empty() {
            return Err(ProviderError::NoModel {
                provider: self.id.clone(),
            });
        }

        let mut body = json!({
            "model": request.model,
            "messages": Self::messages(&request.conversation),
            "stream": true,
        });
        // Canonical parameters, translated (ADR-0026). Sent only when the character chose one:
        // a default written out is Epoch deciding something the user did not.
        if let Some(temperature) = request.parameters.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(top_p) = request.parameters.top_p {
            body["top_p"] = json!(top_p);
        }
        if !request.tools.is_empty() {
            body["tools"] = json!(Self::tools(&request.tools));
        }
        /*
            **The same trace the Ollama provider has had, and this is where it was needed.**

            That one was added because "three rounds of screenshots could not distinguish *the
            model was never told about its tools* from *the model was told and declined*". This
            provider serves llama.cpp, LM Studio and every OpenAI-shaped endpoint — three of the
            four brains on this machine — and had no such file, so the identical question asked
            of a llama.cpp turn had no answer but a guess.

            Measured the moment it existed: asked to open a browser, `gemma-4-12B-it-qat` wrote
            the command out as a code block instead of calling anything. Whether it had been
            offered `run_command` was unanswerable from the outside, and it is the whole
            difference between fixing a descriptor and fixing a registration.

            Off unless `EPOCH_TRACE_DIR` names somewhere, and best effort in every direction: a
            diagnostic must never cost a turn.
        */
        trace(&body);

        // How long the model may stay in memory afterwards — but only when the memory being
        // held is the user's own (`is_loopback`). A hosted vendor has nothing of theirs to free,
        // and an unknown field is exactly the kind of thing a strict API rejects.
        //
        // **Measured 2026-08-25, on this machine.** LM Studio reads `ttl` and honours it: with
        // the field absent a 12B model sat at 7.56 GB with `TTL 60m / 1h` and was still there an
        // hour later; with `ttl: 1` it was gone within eight seconds. llama.cpp does not read it
        // and does not object — which is why it is safe to send to a local server without first
        // asking which one it is.
        //
        // **The trap, and it is the obvious spelling:** `ttl: 0` does *nothing*. LM Studio reads
        // zero as unset and falls back to its own hour. Writing the number that means "now"
        // everywhere else would have silently changed nothing at all.
        //
        // **And it is an idle countdown that every request restarts**, not a deadline — measured:
        // `60s / 1m`, `24s / 1m` after 35 seconds, and `60s / 1m` again the moment another request
        // arrived. That is what makes [`BETWEEN_ROUNDS`] the right number: what has to survive is
        // the *gap* while a capability runs, and the countdown after the last round is what
        // finally evicts. `ttl: 1` was correct for a single request and wrong for a turn — it
        // threw the model out while ComfyUI was drawing, and the closing round paid the whole
        // load again.
        //
        // **It binds when the model loads, not when the request arrives.** A model already
        // resident — opened in LM Studio's own window, or by a turn taken before this shipped —
        // keeps the TTL it was loaded with, and this field is ignored for it. Epoch cannot fix
        // that from here: LM Studio serves no unload route (asked, five spellings, all refused),
        // so this is also the *only* release LM Studio has. llama.cpp does not need it — it has a
        // real unload, and `turn.rs` calls it the moment the turn is over.
        if is_loopback(&self.endpoint) {
            body["ttl"] = json!(match request.keep_loaded {
                crate::provider::KeepLoaded::Never => BETWEEN_ROUNDS.as_secs(),
                crate::provider::KeepLoaded::For(d) => d.as_secs().max(1),
            });
        }

        // Say what the turn needs, before asking for it.
        self.make_room(request);

        let response = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(TURN_TIMEOUT)
                    .build()
                    .post(&format!("{}/v1/chat/completions", self.endpoint)),
            )
            .send_json(body)
            .map_err(|err| match err {
                ureq::Error::Status(status, response) => {
                    let detail = response
                        .into_string()
                        .unwrap_or_else(|_| "and would not say why".into());
                    ProviderError::Refused {
                        provider: self.id.clone(),
                        detail: format!("{status}: {}", detail.trim()),
                    }
                }
                // The address is what a person can act on: a typo, a server not started, a
                // machine asleep. The transport's own words say none of those.
                _ => ProviderError::Unreachable {
                    provider: self.id.clone(),
                    endpoint: self.endpoint.clone(),
                    because: String::new(),
                },
            })?;

        let mut streaming = Streaming::default();
        for line in BufReader::new(response.into_reader()).lines() {
            // The stream died mid-answer. Unreadable rather than unreachable: it was reached,
            // and then something went wrong while it was talking.
            let line = line.map_err(|err| ProviderError::Unreadable {
                provider: self.id.clone(),
                detail: err.to_string(),
            })?;
            let Some(payload) = line.strip_prefix("data: ") else {
                continue;
            };
            if payload.trim() == "[DONE]" {
                break;
            }
            let Ok(event) = serde_json::from_str::<serde_json::Value>(payload) else {
                // Somebody else's stream, and it will grow fields. An unreadable line is skipped
                // rather than fatal — the same tolerance the agent parsers have.
                continue;
            };
            if let Some(delta) = event.pointer("/choices/0/delta") {
                streaming.take(delta, sink);
            }
            streaming.timed(&event);
        }

        sink(Chunk::Done);
        Ok(streaming.finish())
    }

    /// Let go of a model now, for a local server that knows how.
    ///
    /// **Asked, not remembered.** `llama-server`'s router answers `POST /models/unload` with
    /// `{"success":true}` and the model's `status.value` in `/v1/models` goes `loaded` ->
    /// `unloaded` — measured. LM Studio has no unload route at all: every spelling tried
    /// (`/api/v0/unload`, `/models/unload`, `/v1/internal/model/unload`) came back *"Unexpected
    /// endpoint or method"*, which is why `ttl` carries that server instead.
    ///
    /// **And LM Studio answers 200 to a route it does not have**, with the refusal in the body.
    /// A status-code check would have reported success for a call that did nothing — the same
    /// invented gauge this codebase keeps deleting, one layer down. Nothing here reads the
    /// result: best-effort, exactly as Ollama's is, because failing to free memory is not worth
    /// failing a turn over.
    fn release(&self, model: &str) {
        if !is_loopback(&self.endpoint) {
            return;
        }
        let _ = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(PROBE_TIMEOUT)
                    .build()
                    .post(&format!("{}/models/unload", self.endpoint)),
            )
            .send_json(json!({ "model": model }));
    }

    /// What this local server is holding, asked in both spellings.
    ///
    /// One Provider serves two programs and they answer different questions:
    ///
    /// ```text
    /// llama.cpp   GET /v1/models      -> data[].status.value  in loaded | sleeping | unloaded
    /// LM Studio   GET /api/v0/models  -> data[].state         in loaded | not-loaded
    /// ```
    ///
    /// So both are asked and the shape decides which answered — the same discipline `release`
    /// uses, rather than storing which program is behind an address. LM Studio also serves
    /// `/v1/models`, with no `status` on the entries, which is exactly why the *field* is what
    /// is looked for and not the route.
    ///
    /// **`sleeping` is not resident.** llama.cpp's idle release gives the memory back — across
    /// one, the process went 8812 MB -> 126 MB and the card 10736 MiB -> 1937 MiB. Counting it
    /// would report a card as busy when it is free, which is the reading this lamp exists to
    /// get right.
    ///
    /// `None` throughout when nothing answered in a shape we recognise. A hosted vendor holds
    /// no memory of the user's, and a local server that will not talk has not said the model is
    /// gone.
    fn resident(&self, model: &str) -> Option<bool> {
        if !is_loopback(&self.endpoint) {
            return None;
        }
        let ask = |path: &str| -> Option<serde_json::Value> {
            self.signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(PROBE_TIMEOUT)
                    .build()
                    .get(&format!("{}{path}", self.endpoint)),
            )
            .call()
            .ok()?
            .into_json()
            .ok()
        };
        // The field, not the route. Whichever program is here, the entry that carries a state
        // is the one that answered the question.
        let read = |body: &serde_json::Value, at: &str, loaded: &str| -> Option<bool> {
            let entries = body["data"].as_array()?;
            let mut asked = false;
            let mut held = false;
            for entry in entries {
                let Some(state) = entry.pointer(at).and_then(|v| v.as_str()) else {
                    continue;
                };
                asked = true;
                if state == loaded
                    && entry["id"]
                        .as_str()
                        .is_some_and(|id| crate::provider::same_model(id, model))
                {
                    held = true;
                }
            }
            asked.then_some(held)
        };

        ask("/v1/models")
            .and_then(|body| read(&body, "/status/value", "loaded"))
            .or_else(|| ask("/api/v0/models").and_then(|body| read(&body, "/state", "loaded")))
    }
}

/// Whether this address is the user's own machine.
///
/// Measured from the URL rather than taken from a checkbox, because it decides something real:
/// `sight::who_can_see` will not send somebody's screenshot to a backend that is not local, and
/// a user who mislabelled a remote box as local would have made that disclosure by accident.
fn is_loopback(endpoint: &str) -> bool {
    let host = endpoint
        .split("//")
        .nth(1)
        .unwrap_or(endpoint)
        .split(['/', ':'])
        .next()
        .unwrap_or("");
    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

#[cfg(test)]
mod tests {

    /// A native call travels as structure, and **only** as structure.
    ///
    /// `Message::reaching` carries the calls *and* the sentence `[I called …]`, which exists for
    /// a backend with no tool-call shape. Sending both to one that has it means the model reads
    /// its own prior turns as assistant messages whose text is `[I called …]` — and learns to
    /// write them. Measured by the owner with a Spotify server attached: a character answered
    /// `[I called spotify_mcp_search(query: LATIN MAFIA)]` as prose, having called nothing.
    #[test]
    fn a_native_call_carries_no_prose_beside_it() {
        use crate::provider::ToolCall;
        use epoch_kernel::{Conversation, Message};

        let mut conversation = Conversation::default();
        conversation.say(Message::reaching(
            "[I called search(query: LATIN MAFIA)]".to_owned(),
            vec![ToolCall {
                capability: epoch_kernel::CapabilityId::new("search").expect("a valid id"),
                arguments: epoch_kernel::Arguments::new()
                    .with("query", epoch_kernel::Value::Text("LATIN MAFIA".into())),
            }],
        ));

        let wire = OpenAi::messages(&conversation);
        let assistant = wire
            .iter()
            .find(|m| m["role"] == "assistant")
            .expect("the reach is on the wire");

        assert!(
            assistant["tool_calls"].is_array(),
            "the fact travels: {assistant}"
        );
        assert_eq!(
            assistant["content"], "",
            "the sentence is a duplicate of the fact, and one a model imitates: {assistant}"
        );
    }

    use super::*;

    /// What a turn is allowed to fill, read from what the server publishes.
    mod window {
        use super::*;

        fn listing(raw: &str) -> serde_json::Value {
            serde_json::from_str(raw).expect("a listing")
        }

        #[test]
        fn a_resident_model_answers_with_what_it_is_holding() {
            // llama.cpp's own `meta.n_ctx`, which exists only while the model is loaded.
            let body = listing(
                r#"{"data":[{"id":"m","meta":{"n_ctx":32768},
                    "status":{"value":"loaded","args":["--ctx-size","16384"]}}]}"#,
            );
            assert_eq!(
                OpenAi::window_in(&body, "m"),
                Some(32_768),
                "what it holds outranks what it was asked for: the fitter may have reduced it",
            );
        }

        #[test]
        fn a_cold_model_answers_with_the_preset_it_will_load_with() {
            /*
                **The defect this was written for.** Measured 2026-09-02: llama.cpp publishes
                `meta.n_ctx` only for a resident model, and Epoch asks about a cold one on a
                background thread. So every model answered `None`, every character fell back to
                `Budget::default()`, and turns were composed against 8,192 tokens on a server
                holding 32,768 — a quarter of the window, on every character on that backend.
            */
            let body = listing(
                r#"{"data":[{"id":"m","status":{"value":"unloaded",
                    "args":["--alias","m","--ctx-size","32768","--cache-type-k","q8_0"]}}]}"#,
            );
            assert_eq!(OpenAi::window_in(&body, "m"), Some(32_768));
        }

        #[test]
        fn the_short_spelling_is_read_too() {
            // `llama-server` takes both, and a preset writes whichever it was given.
            let body = listing(r#"{"data":[{"id":"m","status":{"args":["-c","65536"]}}]}"#);
            assert_eq!(OpenAi::window_in(&body, "m"), Some(65_536));
        }

        #[test]
        fn lm_studios_own_field_still_wins_where_it_exists() {
            let body = listing(
                r#"{"data":[{"id":"m","loaded_context_length":4096,
                    "status":{"args":["--ctx-size","32768"]}}]}"#,
            );
            assert_eq!(OpenAi::window_in(&body, "m"), Some(4096));
        }

        #[test]
        fn a_model_with_no_window_anywhere_stays_unknown() {
            /*
                **`None` is *nobody said*, and the caller's conservative default is the right
                answer to that.** What it must never be is a guess: a window invented here is a
                turn composed to a size the server will refuse.
            */
            let body = listing(r#"{"data":[{"id":"m","status":{"value":"unloaded","args":[]}}]}"#);
            assert_eq!(OpenAi::window_in(&body, "m"), None);
        }

        #[test]
        fn a_flag_with_nothing_after_it_is_not_a_window() {
            // Reading the flag alone would report a window whenever one was set — a boolean
            // wearing a number.
            let body = listing(r#"{"data":[{"id":"m","status":{"args":["--ctx-size"]}}]}"#);
            assert_eq!(OpenAi::window_in(&body, "m"), None);

            let odd = listing(r#"{"data":[{"id":"m","status":{"args":["--ctx-size","0"]}}]}"#);
            assert_eq!(OpenAi::window_in(&odd, "m"), None, "zero is not a window");
        }
    }

    /// What a backend says about its own speed, read where it is offered.
    mod pace {
        use super::*;

        fn frame(raw: &str) -> serde_json::Value {
            serde_json::from_str(raw).expect("a frame")
        }

        #[test]
        fn llama_cpp_reports_it_under_timings() {
            /*
                Measured 2026-09-02 against the router serving
                `Qwen3.8-27B-Uncensored-IQ2-M---MTP` with the compressed cache applied. Epoch's
                own bench had measured that configuration at 11.54 tok/s; the server, asked
                directly, said 11.428 — which is the number this reads, and the agreement is
                what shows the preset is in effect rather than merely written down.
            */
            let mut seen = Streaming::default();
            seen.timed(&frame(
                r#"{"timings":{"predicted_per_second":11.428019981258048,"predicted_n":64}}"#,
            ));
            let got = seen.pace.expect("the server said");
            assert!((got.per_second - 11.428).abs() < 0.001);
            assert_eq!(got.generated, Some(64));
        }

        #[test]
        fn the_last_frame_wins() {
            // llama.cpp emits a running figure per token; the final one is the answer, and an
            // early frame describing three tokens is not a rate for the reply.
            let mut seen = Streaming::default();
            seen.timed(&frame(
                r#"{"timings":{"predicted_per_second":3.0,"predicted_n":1}}"#,
            ));
            seen.timed(&frame(
                r#"{"timings":{"predicted_per_second":11.4,"predicted_n":64}}"#,
            ));
            assert_eq!(seen.pace.expect("kept").generated, Some(64));
        }

        #[test]
        fn a_server_that_says_nothing_leaves_it_unmeasured() {
            /*
                **`None` is *nobody said*, never *instantly*.** An OpenAI-shaped server that
                reports no timing is not a fast one, and a zero on the screen under a reply would
                be the invented gauge this codebase keeps deleting.
            */
            let mut seen = Streaming::default();
            seen.timed(&frame(r#"{"choices":[{"delta":{"content":"hola"}}]}"#));
            assert!(seen.pace.is_none());
        }

        #[test]
        fn a_zero_rate_is_not_a_measurement() {
            let mut seen = Streaming::default();
            seen.timed(&frame(
                r#"{"timings":{"predicted_per_second":0.0,"predicted_n":0}}"#,
            ));
            assert!(seen.pace.is_none());
        }

        #[test]
        fn lm_studio_is_read_under_its_own_name() {
            // The same quantity, a different key. A reader that knew only llama.cpp's spelling
            // would report a whole backend as unmeasured (ADR-0003).
            let mut seen = Streaming::default();
            seen.timed(&frame(r#"{"stats":{"tokens_per_second":42.5}}"#));
            let got = seen.pace.expect("LM Studio said");
            assert!((got.per_second - 42.5).abs() < 0.001);
            assert_eq!(got.generated, None, "it reports a rate and not a count");
        }
    }

    #[test]
    fn an_address_says_whether_it_is_this_machine() {
        // Not a checkbox: it decides whether a screenshot may be sent there (ADR-0025's
        // disclosure rule, applied by `sight::who_can_see`), and somebody mislabelling a remote
        // box as local would make that decision by accident.
        assert!(is_loopback("http://localhost:8080"));
        assert!(is_loopback("http://127.0.0.1:1234/"));
        assert!(!is_loopback("http://192.168.1.40:8080"));
        assert!(!is_loopback("https://api.deepseek.com"));
    }

    #[test]
    fn a_trailing_slash_does_not_become_a_double_one() {
        // The most common thing to paste, and it fails in a way that reads as the server being
        // wrong rather than the address.
        let backend = OpenAi::named("desk", "http://192.168.1.7:8080/");
        assert_eq!(backend.endpoint, "http://192.168.1.7:8080");
    }

    #[test]
    fn the_same_machine_is_reached_at_the_address_that_answers() {
        // Measured: `localhost` resolves to `[::1]` first on Windows, an IPv4-only server does
        // not refuse there but hangs, and the fallback costs fifteen seconds — the same URL as
        // `127.0.0.1` answers in nothing. See `provider::same_machine`.
        let backend = OpenAi::named("here", "http://localhost:1234");
        assert_eq!(backend.endpoint, "http://127.0.0.1:1234");

        // Somebody else's machine is left exactly as they wrote it.
        let far = OpenAi::named("desk", "http://192.168.1.20:11434");
        assert_eq!(far.endpoint, "http://192.168.1.20:11434");
    }

    #[test]
    fn a_streamed_tool_call_is_reassembled_from_its_fragments() {
        // The one genuinely fiddly part of this API: the name arrives once, the arguments arrive
        // in pieces, and two calls in one answer interleave by index.
        let mut streaming = Streaming::default();
        let mut said = String::new();
        let mut sink = |chunk: Chunk| {
            if let Chunk::Token(text) = chunk {
                said.push_str(&text);
            }
        };

        streaming.take(&json!({ "content": "let me look" }), &mut sink);
        streaming.take(
            &json!({ "tool_calls": [
                { "index": 0, "function": { "name": "read_file", "arguments": "{\"pa" } }
            ]}),
            &mut sink,
        );
        streaming.take(
            &json!({ "tool_calls": [
                { "index": 0, "function": { "arguments": "th\":\"README.md\"}" } }
            ]}),
            &mut sink,
        );

        let answer = streaming.finish();
        assert_eq!(said, "let me look");
        assert_eq!(answer.text, "let me look");
        assert_eq!(answer.calls.len(), 1);
        assert_eq!(answer.calls[0].capability.as_str(), "read_file");
        assert_eq!(
            answer.calls[0].arguments.text("path"),
            Ok("README.md"),
            "the arguments survived being split mid-key"
        );
    }

    #[test]
    fn a_call_that_never_finished_arriving_is_dropped() {
        // Inventing empty arguments would run a capability with input nobody sent. A turn that
        // said something and called nothing is the lesser failure by a distance.
        let mut streaming = Streaming::default();
        streaming.take(
            &json!({ "tool_calls": [
                { "index": 0, "function": { "name": "read_file", "arguments": "{\"path\":" } }
            ]}),
            &mut |_| {},
        );

        assert!(streaming.finish().calls.is_empty());
    }
}

#[cfg(test)]
mod sight_over_the_openai_dialect {
    use super::*;

    /// Both local servers declare vision, and they declare it differently.
    ///
    /// Measured 2026-08-21 against the two running on this machine. The parsing is what is held
    /// here; the run that produced these bodies is quoted in `declares`.
    #[test]
    fn llama_cpp_says_it_in_input_modalities() {
        let body = serde_json::json!({"data": [
            {"id": "gemma4-12b", "architecture": {"input_modalities": ["text", "image", "audio"]}},
            {"id": "qwen3-14b", "architecture": {"input_modalities": ["text"]}}
        ]});

        let can = OpenAi::read_llama_cpp(&body, "gemma4-12b").expect("declared");
        assert!(can.sees, "the projector is beside it");
        assert!(can.hears);

        // The same server, the same day, saying no about a different model. That is what makes
        // this a reading rather than a hopeful default.
        let cannot = OpenAi::read_llama_cpp(&body, "qwen3-14b").expect("declared");
        assert!(!cannot.sees);
        assert!(!cannot.hears);

        // A model the server does not list is unasked, never blind.
        assert!(OpenAi::read_llama_cpp(&body, "not-here").is_none());
    }

    #[test]
    fn lm_studio_says_it_in_the_model_type() {
        let body = serde_json::json!({"data": [
            {"id": "gemma4-12b", "type": "vlm", "max_context_length": 262_144},
            {"id": "qwen3-14b", "type": "llm", "max_context_length": 40_960}
        ]});

        assert!(
            OpenAi::read_lm_studio(&body, "gemma4-12b")
                .expect("declared")
                .sees
        );
        assert!(
            !OpenAi::read_lm_studio(&body, "qwen3-14b")
                .expect("declared")
                .sees
        );
        assert!(OpenAi::read_lm_studio(&body, "not-here").is_none());

        // **Never claimed.** LM Studio does not report audio per model, and `Declared` has no
        // way to say *unasked* for one field — so the conservative half is taken rather than
        // inventing a capability from a different server's vocabulary.
        assert!(
            !OpenAi::read_lm_studio(&body, "gemma4-12b")
                .expect("declared")
                .hears
        );
    }

    #[test]
    fn a_window_is_what_the_model_was_given_never_what_it_could_take() {
        // Both servers publish both numbers, side by side, and the wrong one is plausible.
        // Measured on this machine: LM Studio loads at its own default of 8192 while reporting
        // `max_context_length` 262144, so composing against the larger number sent an 8.6k turn
        // into an 8k window — and the same model that answered correctly on two other backends
        // had an answer cut mid-sentence and then answered a greeting to a question about files.
        let lm_studio = serde_json::json!({"data": [
            {"id": "gemma4-12b", "loaded_context_length": 8_192, "max_context_length": 262_144}
        ]});
        assert_eq!(OpenAi::window_in(&lm_studio, "gemma4-12b"), Some(8_192));

        let llama_cpp = serde_json::json!({"data": [
            {"id": "gemma4-12b", "meta": {"n_ctx": 82_944, "n_ctx_train": 262_144}}
        ]});
        assert_eq!(OpenAi::window_in(&llama_cpp, "gemma4-12b"), Some(82_944));

        // Not loaded yet: LM Studio still lists the model and still publishes what it *could*
        // hold. That is unknown, not 262144 — a conservative budget is the right answer to not
        // knowing, and the larger number is an invented one.
        let cold = serde_json::json!({"data": [
            {"id": "gemma4-12b", "state": "not-loaded", "max_context_length": 262_144}
        ]});
        assert_eq!(OpenAi::window_in(&cold, "gemma4-12b"), None);

        // A model this server does not list, and a zero, are both unasked rather than tiny.
        assert_eq!(OpenAi::window_in(&lm_studio, "not-here"), None);
        let zero = serde_json::json!({"data": [
            {"id": "gemma4-12b", "loaded_context_length": 0}
        ]});
        assert_eq!(OpenAi::window_in(&zero, "gemma4-12b"), None);
    }

    /// Tool use, and the three answers a `capabilities` array can give.
    ///
    /// Measured on this machine: `qwen3-14b` lists `tool_use`, `gemma4-26b` lists nothing, and
    /// `gpt-oss-20b` has no array at all — and the last one takes tool calls perfectly well.
    /// So only the first is a yes, and the other two are *unasked*, never a refusal. The field
    /// used to be `false` for all three and the surface left those characters with no tools.
    #[test]
    fn lm_studio_declares_tool_use_and_says_nothing_about_the_rest() {
        let body = serde_json::json!({"data": [
            {"id": "qwen3-14b", "type": "llm", "capabilities": ["tool_use"]},
            {"id": "gemma4-26b", "type": "llm", "capabilities": []},
            {"id": "gpt-oss-20b", "type": "llm"}
        ]});
        let asked = |id| {
            OpenAi::read_lm_studio(&body, id)
                .expect("declared")
                .uses_tools
        };

        assert_eq!(asked("qwen3-14b"), Some(true));
        assert_eq!(asked("gemma4-26b"), None, "an empty array is not a refusal");
        assert_eq!(
            asked("gpt-oss-20b"),
            None,
            "a missing array is not a refusal"
        );
    }

    /// llama.cpp answers this question nowhere, and must keep saying so.
    #[test]
    fn llama_cpp_is_never_asked_about_tools() {
        let body = serde_json::json!({"data": [{
            "id": "gemma4-12b",
            "architecture": {"input_modalities": ["text", "image"]}
        }]});
        assert_eq!(
            OpenAi::read_llama_cpp(&body, "gemma4-12b")
                .expect("declared")
                .uses_tools,
            None
        );
    }

    /// A body that answers nothing teaches nothing.
    ///
    /// The cold-instrument rule at the level of a field: a server with no `architecture` and no
    /// `type` is not a server full of blind models.
    #[test]
    fn a_server_that_declares_nothing_is_unasked() {
        let body = serde_json::json!({"data": [{"id": "mystery"}]});
        assert!(OpenAi::read_llama_cpp(&body, "mystery").is_none());
        // LM Studio's shape has no `type` either, and `sees` would be false — but that is a
        // *read* of its own endpoint, which only LM Studio serves. Reaching it at all means the
        // server answered `/api/v0/models`.
        assert!(
            !OpenAi::read_lm_studio(&body, "mystery")
                .expect("its own endpoint")
                .sees
        );
    }
}

#[cfg(test)]
mod against_a_real_server {
    use super::*;
    use crate::provider::Provider;

    /// The whole path, against LM Studio actually running on this machine.
    ///
    /// `#[ignore]` because it needs a server holding a vision model — run it with
    /// `cargo test -p epoch-engine -- --ignored sees_a_photograph`. It is here rather than in a
    /// note because the four times this went wrong, it went wrong *between* the parts that each
    /// had a passing unit test.
    #[test]
    #[ignore = "needs LM Studio serving a vision model on 1234"]
    fn sees_a_photograph_through_lm_studio() {
        let server = OpenAi::named("lm_studio", "http://127.0.0.1:1234");
        let can = server.declares("gemma4-12b").expect("it declares");
        assert!(can.sees, "type = vlm");

        // And the window comes from the server rather than from a table. Two models on one
        // server report different numbers, which is what makes it a measurement.
        let surface = server.surface("gemma4-12b");
        assert_eq!(surface.window, Some(262_144));
        assert_eq!(server.surface("qwen3-14b").window, Some(40_960));

        let picture = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../vault/images/15ba808435bcdc62.jpg"),
        )
        .expect("a picture to look at");

        let said = server
            .describe(
                "gemma4-12b",
                &picture,
                "Describe this image in one short sentence.",
            )
            .expect("it looked");
        println!("{said}");
        assert!(said.to_lowercase().contains("jacket"), "{said}");
    }

    /// A turn that ends must give the graphics card back.
    ///
    /// **This is the defect the user reported, and it is measured here rather than reasoned
    /// about.** With the crew idle and *"Run several crew members at once"* switched off, a 12B
    /// model sat at 7.56 GB of RAM and 11.2 GB of VRAM on both of these servers, indefinitely:
    /// LM Studio's own `TTL 60m / 1h`, and llama.cpp's `--sleep-idle-seconds` default of `-1`.
    /// Ollama honoured the same setting because its door writes `keep_alive` into every request
    /// and this door wrote nothing at all.
    ///
    /// It goes through `take_turn` — the same call the window makes — because a harness that
    /// posts its own JSON proves the *server* can release a model and proves nothing about
    /// whether Epoch would ask it to. That distinction is exactly what let a toolless character
    /// ship past a green cross-runtime measurement.
    ///
    /// Run it with both servers up:
    /// `cargo test -p epoch-engine -- --ignored --nocapture a_finished_turn`
    #[test]
    #[ignore = "needs llama.cpp on 8080 and LM Studio on 1234, each holding gemma4-12b"]
    fn a_finished_turn_lets_the_model_go() {
        use crate::provider::KeepLoaded;
        use epoch_kernel::Conversation;

        for (name, endpoint, model, at, key) in [
            (
                "llama.cpp",
                "http://127.0.0.1:8080",
                "gemma4-12b",
                "/v1/models",
                "status",
            ),
            (
                "LM Studio",
                "http://127.0.0.1:1234",
                "gemma4-12b",
                "/api/v0/models",
                "state",
            ),
        ] {
            let server = OpenAi::named("probe", endpoint);
            // Whether *anything* on this server is resident. llama.cpp spells it
            // `status.value`, LM Studio spells it `state`, and both say `loaded`.
            let resident = || {
                let body = server.model_list(at).expect("the server answers");
                body["data"]
                    .as_array()
                    .expect("a list")
                    .iter()
                    .filter(|m| {
                        let said = if key == "status" {
                            m["status"]["value"].as_str()
                        } else {
                            m[key].as_str()
                        };
                        said == Some("loaded")
                    })
                    .map(|m| m["id"].as_str().unwrap_or("?").to_owned())
                    .collect::<Vec<_>>()
            };

            let request = Request {
                model: model.into(),
                conversation: Conversation::opening("Answer in one word."),
                keep_loaded: KeepLoaded::Never,
                parameters: Default::default(),
                tuning: Default::default(),
                tools: Vec::new(),
                // The built-in bound: a test is not the place to decide how patient this machine is.
                most_rounds: None,
            };
            server
                .take_turn(&request, &mut |_| {})
                .unwrap_or_else(|err| panic!("{name}: {err}"));

            // llama.cpp is unloaded by the time the call returns; LM Studio's `ttl` is a clock,
            // and one second of it has to actually pass. Polling rather than one long sleep, so
            // the test reports how long it took rather than how long it was willing to wait.
            //
            // **Start it cold** (`lms unload --all`). This failed once on a model that was
            // already resident, and that failure is a true fact about LM Studio rather than a
            // flaky test: `ttl` is read when the model is loaded, so a model somebody else
            // opened keeps their hour no matter what Epoch sends.
            let mut held = resident();
            for _ in 0..20 {
                if held.is_empty() {
                    break;
                }
                std::thread::sleep(Duration::from_secs(1));
                held = resident();
            }
            println!("{name}: released, still resident = {held:?}");
            assert!(
                held.is_empty(),
                "{name} is still holding {held:?} after a turn that asked it not to"
            );
        }
    }
}

/// Where the last turn on this provider is written, for when a screenshot is not enough.
///
/// **Its own filename.** The Ollama provider writes `last-turn.json`; sharing it would mean the
/// newest turn is the only readable one and which backend wrote it is a guess -- which is the
/// class of problem this file exists to remove rather than add to.
fn trace(body: &serde_json::Value) {
    let Some(dir) = std::env::var_os("EPOCH_TRACE_DIR").map(std::path::PathBuf::from) else {
        return;
    };
    let Ok(pretty) = serde_json::to_string_pretty(body) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("last-openai-turn.json"), pretty);
}
