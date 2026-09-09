//! The Anthropic Messages API, behind the same [`Provider`] contract as everything else.
//!
//! ## Why this Provider is the interesting one
//!
//! Ollama was chosen first precisely because it made every assumption about credentials
//! somebody else's problem (ADR-0007). This is the other half of that bargain: the first
//! backend that is hosted, paid for, authenticated, and shaped nothing like a local runtime.
//! If [`Provider`] survives it without deforming, the abstraction was real.
//!
//! It mostly did. Four things had to be *translated* rather than passed through, and each one
//! is a place where a naive "forward the canonical value" would have produced a request the API
//! refuses:
//!
//! 1. **There is no system role in `messages`.** A system prompt is a separate top-level field,
//!    so the Composer's `Role::System` blocks are hoisted out of the conversation rather than
//!    sent as turns. This is a *translation*, which is exactly what ADR-0007 says a Provider is
//!    for — the Composer keeps emitting canonical roles and never learns whose dialect this is.
//! 2. **There is no tool role either.** Evidence replays as an ordinary user turn. Epoch records
//!    what a tool produced as text (ADR-0025 — evidence is what was produced), so there is no
//!    `tool_use_id` to pair a structured `tool_result` with, and inventing one would be
//!    fabricating a link the record does not contain.
//! 3. **`max_tokens` is required**, and the ceiling differs per model. Measured from the Models
//!    API and remembered, never guessed at (ADR-0026).
//! 4. **Sampling parameters are refused by the newest models.** Handled by measurement rather
//!    than a table — see [`Refuses`].
//!
//! ## Raw HTTP, deliberately
//!
//! There is no official Anthropic SDK for Rust. The wire shape is small and stable, `ureq` is
//! already here for Ollama, and the whole thing stays inside this module — which is the same
//! containment argument that let the Engine stay synchronous in the first place.

use crate::guard::writing;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;

use epoch_kernel::{
    Arguments, CapabilityId, Conversation, Descriptor, Parameters, Reasoning, Role, Secret, Value,
    ValueKind,
};

use crate::provider::{
    Answer, Chunk, Provider, ProviderError, ProviderStatus, Request, Surface, ToolCall,
};

/// The version of the API this build speaks. Sent on every request.
///
/// A pinned date rather than "latest": a wire format that changes underneath a running build is
/// a build whose behaviour changes without anybody editing it.
const API_VERSION: &str = "2023-06-01";

const ENDPOINT: &str = "https://api.anthropic.com";

/// How long to wait for the service to admit it exists.
///
/// Longer than Ollama's, because this one is across the internet rather than across localhost —
/// and shorter than a turn, because a surface asking "who is online?" must not hang on it.
const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(6);
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);
const TURN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// What to ask for when the model has not said what it can produce.
///
/// Every current model exceeds this; it is a floor for the case where the Models API could not
/// be reached, not a belief about any particular model.
const FALLBACK_OUTPUT: u32 = 8_192;

/// Fields one model refuses.
///
/// **Measured, not tabulated.** The newest models reject `temperature`, `top_p` and some
/// `thinking` shapes outright, and which models those are changes with every release — a table
/// in this file would be correct on the day it was written and quietly wrong afterwards, which
/// is the failure mode this codebase keeps choosing to design out.
///
/// So the first request for a model sends what the character authored. If the service refuses
/// and names a field, that field is dropped and the request is sent again, and the refusal is
/// remembered for the rest of the session. One extra round trip per model per session, and the
/// turn succeeds instead of failing with a message about a parameter the user cannot see.
///
/// Honest about the fragility: this reads the service's own error text. If the wording changes
/// the retry stops helping and the original refusal surfaces — visibly, with the service's own
/// words in it, which is the same bargain the DuckDuckGo backend makes.
type Refuses = BTreeSet<&'static str>;

/// Anthropic's hosted models.
pub struct Anthropic {
    id: String,
    endpoint: String,
    /// `None` means nobody has stored one. That is an ordinary state, not an error: the backend
    /// is configured, it simply cannot be asked anything yet, and `probe` says exactly that.
    key: Option<Secret>,
    /// What each model measurably accepts. Filled as models are used; never persisted, because
    /// it describes a service that can change between sessions.
    learned: RwLock<BTreeMap<String, Learned>>,
}

#[derive(Debug, Clone, Default)]
struct Learned {
    /// Most this model will produce, as it reported. `None` until measured.
    output: Option<u32>,
    refuses: Refuses,
}

impl std::fmt::Debug for Anthropic {
    /// Written out rather than derived so a `{:?}` can never print the key. `Secret`'s own
    /// `Debug` already refuses, but a Provider is the thing most likely to end up in a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Anthropic")
            .field("id", &self.id)
            .field("endpoint", &self.endpoint)
            .field("keyed", &self.key.is_some())
            .finish()
    }
}

impl Anthropic {
    pub fn named(id: impl Into<String>, endpoint: impl Into<String>) -> Self {
        let endpoint = endpoint.into();
        Self {
            id: id.into(),
            endpoint: if endpoint.trim().is_empty() {
                ENDPOINT.to_owned()
            } else {
                endpoint
            },
            key: None,
            learned: RwLock::new(BTreeMap::new()),
        }
    }

    /// Give it the credential it cannot work without.
    pub fn with_key(mut self, key: Option<Secret>) -> Self {
        self.key = key.filter(|k| !k.is_empty());
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// The headers every call carries. `None` when there is no key to carry.
    fn signed(&self, request: ureq::Request) -> Option<ureq::Request> {
        let key = self.key.as_ref()?;
        Some(
            request
                .set("x-api-key", key.expose())
                .set("anthropic-version", API_VERSION)
                .set("content-type", "application/json"),
        )
    }

    fn known(&self, model: &str) -> Learned {
        self.learned
            .read()
            .expect("learned lock")
            .get(model)
            .cloned()
            .unwrap_or_default()
    }

    fn remember(&self, model: &str, change: impl FnOnce(&mut Learned)) {
        let mut all = writing(&self.learned);
        change(all.entry(model.to_owned()).or_default());
    }

    /// What one model says about itself, straight from the Models API.
    ///
    /// Two facts, one request: how much it holds and how much it will produce. Both belong to
    /// the model rather than to the backend, which is why [`Provider::surface`] asks per model.
    fn described(&self, model: &str) -> Option<(Option<u32>, Option<u32>)> {
        let raw: serde_json::Value = self
            .signed(
                ureq::builder()
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout(PROBE_TIMEOUT)
                    .build()
                    .get(&format!("{}/v1/models/{model}", self.endpoint)),
            )?
            .call()
            .ok()?
            .into_json()
            .ok()?;

        let read = |key: &str| raw.get(key).and_then(|v| v.as_u64()).map(|n| n as u32);
        Some((read("max_input_tokens"), read("max_tokens")))
    }

    /// Most this model will produce, measured once and then remembered.
    fn output_ceiling(&self, model: &str) -> u32 {
        if let Some(known) = self.known(model).output {
            return known;
        }
        let measured = self.described(model).and_then(|(_, out)| out);
        if let Some(found) = measured {
            self.remember(model, |l| l.output = Some(found));
        }
        // Not remembered when it could not be measured: the network may be back next turn, and
        // caching a fallback would make one bad moment permanent for the session.
        measured.unwrap_or(FALLBACK_OUTPUT)
    }
}

impl Provider for Anthropic {
    fn id(&self) -> &str {
        &self.id
    }

    fn probe(&self) -> ProviderStatus {
        let offline = |note: &str| ProviderStatus {
            id: self.id.clone(),
            name: "Anthropic".to_owned(),
            // Not a computer anybody owns. Grouped by where it answers, which is what the
            // editor did from the endpoint before this field existed — an honest heading, and
            // never "This PC".
            machine: crate::provider::machine_of(&self.endpoint, false),
            endpoint: self.endpoint.clone(),
            online: false,
            local: false,
            models: Vec::new(),
            note: Some(note.to_owned()),
        };

        // A configured backend with no credential is the ordinary first state, and the note is
        // the whole point of it: it says what to do rather than that something is broken.
        let Some(request) = self.signed(
            ureq::builder()
                .timeout_connect(CONNECT_TIMEOUT)
                .timeout(PROBE_TIMEOUT)
                .build()
                .get(&format!("{}/v1/models?limit=100", self.endpoint)),
        ) else {
            return offline("no credential stored — add one in Connections");
        };

        let raw: serde_json::Value = match request.call() {
            Ok(response) => match response.into_json() {
                Ok(value) => value,
                Err(err) => return offline(&format!("answered with something unreadable: {err}")),
            },
            Err(ureq::Error::Status(401 | 403, _)) => {
                return offline("the stored credential was refused")
            }
            Err(ureq::Error::Status(code, response)) => {
                return offline(&format!(
                    "{code}: {}",
                    response
                        .into_string()
                        .unwrap_or_else(|_| "no detail".into())
                ))
            }
            Err(ureq::Error::Transport(_)) => return offline("not reachable"),
        };

        let models = raw["data"]
            .as_array()
            .map(|all| {
                all.iter()
                    .filter_map(|m| m["id"].as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        ProviderStatus {
            id: self.id.clone(),
            name: "Anthropic".to_owned(),
            // Not a computer anybody owns. Grouped by where it answers, which is what the
            // editor did from the endpoint before this field existed — an honest heading, and
            // never "This PC".
            machine: crate::provider::machine_of(&self.endpoint, false),
            endpoint: self.endpoint.clone(),
            online: true,
            local: false,
            models,
            note: None,
        }
    }

    fn surface(&self, model: &str) -> Surface {
        // No controls of its own, and that is the honest answer rather than an omission: a
        // hosted backend has nothing on this machine to tune. Every knob it offers is already
        // one of the canonical parameters (ADR-0026), so declaring provider-native duplicates
        // would give the user two spellings of one setting.
        let window = self.described(model).and_then(|(window, _)| window);
        Surface {
            window,
            controls: Vec::new(),
            // **Unasked, not assumed.** This backend's model list does not publish a capability
            // set the way Ollama's `/api/show` does, so Epoch has not learned anything — and
            // `None` says exactly that. Filling it in from what these models are known to do
            // would be a remembered fact wearing a measurement's clothes, which is the mistake
            // this whole field exists to stop.
            can: None,
        }
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
        if self.key.is_none() {
            return Err(ProviderError::Refused {
                provider: self.id.clone(),
                detail: "no credential is stored for this backend — add one in Connections"
                    .to_owned(),
            });
        }

        let ceiling = self.output_ceiling(&request.model);

        // Two attempts at most. The second only happens when the first was refused for naming a
        // field, and it carries what that refusal taught us.
        let mut refuses = self.known(&request.model).refuses;
        for attempt in 0..2 {
            let body = self.body(request, ceiling, &refuses);
            match self.stream(&body, sink) {
                Ok(answer) => return Ok(answer),
                Err(ProviderError::Refused { detail, .. }) if attempt == 0 => {
                    let named = blamed(&detail);
                    if named.is_empty() {
                        return Err(ProviderError::Refused {
                            provider: self.id.clone(),
                            detail,
                        });
                    }
                    for field in named {
                        refuses.insert(field);
                    }
                    let learned = refuses.clone();
                    self.remember(&request.model, |l| l.refuses = learned);
                }
                Err(other) => return Err(other),
            }
        }
        unreachable!("the loop returns on both branches of its second pass")
    }

    // `release` is deliberately not implemented. There is nothing of the user's to free — the
    // model was never on their machine — and a call that looks like it did something would be
    // worse than the default that plainly does nothing.
}

impl Anthropic {
    /// The request, in Anthropic's own shape.
    fn body(&self, request: &Request, ceiling: u32, refuses: &Refuses) -> serde_json::Value {
        let (system, messages) = split(&request.conversation);

        let mut body = serde_json::json!({
            "model": request.model,
            "max_tokens": ceiling,
            "stream": true,
            "messages": messages,
        });

        if !system.is_empty() {
            body["system"] = serde_json::Value::from(system);
        }
        if !request.tools.is_empty() {
            body["tools"] =
                serde_json::Value::from(request.tools.iter().map(tool).collect::<Vec<_>>());
        }
        for (field, value) in sampling(&request.parameters) {
            if !refuses.contains(field) {
                body[field] = value;
            }
        }
        if !refuses.contains("thinking") {
            if let Some((thinking, effort)) = deliberation(request.parameters.reasoning) {
                body["thinking"] = thinking;
                if let Some(effort) = effort {
                    body["output_config"] = serde_json::json!({ "effort": effort });
                }
            }
        }

        // Provider-native tuning is deliberately not merged in. Anthropic declares no controls
        // of its own (see `surface`), so a namespace of them would be settings nothing reads —
        // and a Provider that silently applied another backend's tuning would be the exact
        // failure ADR-0026 keeps dormant namespaces dormant to prevent.
        body
    }

    /// Send it, and read the events back as they arrive.
    fn stream(
        &self,
        body: &serde_json::Value,
        sink: &mut dyn FnMut(Chunk),
    ) -> Result<Answer, ProviderError> {
        let response = self
            .signed(
                ureq::builder()
                    // Connecting is bounded even though thinking is not — the same reasoning as
                    // every other backend: a model may take minutes, a socket may not.
                    .timeout_connect(CONNECT_TIMEOUT)
                    .timeout_read(TURN_TIMEOUT)
                    .build()
                    .post(&format!("{}/v1/messages", self.endpoint)),
            )
            .expect("a key was checked before the turn began")
            .send_json(body)
            .map_err(|err| match err {
                ureq::Error::Status(code, r) => ProviderError::Refused {
                    provider: self.id.clone(),
                    detail: format!(
                        "{code}: {}",
                        r.into_string().unwrap_or_else(|_| "no detail".into())
                    ),
                },
                ureq::Error::Transport(_) => ProviderError::Unreachable {
                    provider: self.id.clone(),
                    endpoint: self.endpoint.clone(),
                    because: String::new(),
                },
            })?;

        use std::io::{BufRead, BufReader};
        let mut answer = Answer::default();
        let mut building: Option<Building> = None;
        let mut refusal = false;

        for line in BufReader::new(response.into_reader()).lines() {
            let line = line.map_err(|err| ProviderError::Unreadable {
                provider: self.id.clone(),
                detail: err.to_string(),
            })?;

            // Server-sent events: an `event:` line naming the type, a `data:` line carrying it,
            // and blank lines between. The type is inside the payload too, so only `data:`
            // needs reading — which also means a new event type cannot desynchronise the parse.
            let Some(payload) = line.strip_prefix("data:") else {
                continue;
            };
            let Ok(frame) = serde_json::from_str::<serde_json::Value>(payload.trim()) else {
                continue;
            };

            match frame["type"].as_str().unwrap_or_default() {
                "error" => {
                    return Err(ProviderError::Refused {
                        provider: self.id.clone(),
                        detail: frame["error"]["message"]
                            .as_str()
                            .unwrap_or("no detail")
                            .to_owned(),
                    })
                }
                "content_block_start" => {
                    let block = &frame["content_block"];
                    if block["type"] == "tool_use" {
                        building = Some(Building {
                            name: block["name"].as_str().unwrap_or_default().to_owned(),
                            json: String::new(),
                        });
                    }
                }
                "content_block_delta" => {
                    match frame["delta"]["type"].as_str().unwrap_or_default() {
                        "text_delta" => {
                            if let Some(token) = frame["delta"]["text"].as_str() {
                                if !token.is_empty() {
                                    answer.text.push_str(token);
                                    sink(Chunk::Token(token.to_owned()));
                                }
                            }
                        }
                        "input_json_delta" => {
                            if let Some(part) = frame["delta"]["partial_json"].as_str() {
                                if let Some(open) = building.as_mut() {
                                    open.json.push_str(part);
                                }
                            }
                        }
                        // Thinking arrives as its own delta and carries no text unless it was asked
                        // for. Skipped rather than shown: a stream of empty tokens would look like
                        // the character stammering.
                        _ => {}
                    }
                }
                "content_block_stop" => {
                    if let Some(done) = building.take() {
                        if let Some(call) = done.into_call() {
                            answer.calls.push(call);
                        }
                    }
                }
                "message_delta" => {
                    if frame["delta"]["stop_reason"] == "refusal" {
                        refusal = true;
                    }
                }
                "message_stop" => break,
                _ => {}
            }
        }

        if refusal {
            // A 200 whose content is a decline. Reported as a refusal rather than returned as an
            // empty answer, because a character who says nothing and a character who was
            // stopped are different facts and the World must not show them the same way.
            return Err(ProviderError::Refused {
                provider: self.id.clone(),
                detail: "the service declined this request".to_owned(),
            });
        }

        sink(Chunk::Done);
        Ok(answer)
    }
}

/// One tool call, arriving a fragment of JSON at a time.
struct Building {
    name: String,
    json: String,
}

impl Building {
    /// Everything here came from a **model**, so nothing is assumed — same rule as every other
    /// backend. A call that cannot be read is dropped and the turn continues; failing the whole
    /// turn over one malformed call of three throws away work that succeeded.
    fn into_call(self) -> Option<ToolCall> {
        let capability = CapabilityId::new(&self.name).ok()?;
        let raw: serde_json::Value = if self.json.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::from_str(&self.json).ok()?
        };

        let mut arguments = Arguments::new();
        for (key, value) in raw.as_object()? {
            let held = match value {
                serde_json::Value::String(v) => Value::Text(v.clone()),
                serde_json::Value::Bool(v) => Value::Boolean(*v),
                serde_json::Value::Number(n) if n.is_i64() => Value::Integer(n.as_i64()?),
                serde_json::Value::Number(n) => Value::Number(n.as_f64()?),
                // No capability declares a null, an array or an object, so a model that sent one
                // has misunderstood. Dropping it makes the argument *missing*, which the
                // capability already knows how to report clearly.
                _ => continue,
            };
            arguments = arguments.with(key, held);
        }
        Some(ToolCall {
            capability,
            arguments,
        })
    }
}

/// Hoist the system blocks out of the conversation, and translate what is left.
///
/// Two shapes that do not exist here and the honest mapping for each:
///
/// - **System** is a top-level field, not a turn. Several blocks join with blank lines between
///   them, which is what the Composer's own ordering already means.
/// - **Tool** has no role at all. Evidence replays as a user turn, because Epoch records what a
///   tool produced as text (ADR-0025) and there is no `tool_use_id` in that record to pair a
///   structured result with. Inventing one would fabricate a link the record does not contain.
fn split(conversation: &Conversation) -> (String, Vec<serde_json::Value>) {
    let mut system = String::new();
    let mut messages: Vec<serde_json::Value> = Vec::new();

    for message in &conversation.messages {
        match message.role {
            Role::System => {
                if !system.is_empty() {
                    system.push_str("\n\n");
                }
                system.push_str(&message.content);
            }
            role => {
                let who = if matches!(role, Role::Assistant) {
                    "assistant"
                } else {
                    "user"
                };
                // Consecutive turns of one role are joined rather than sent separately. The API
                // accepts both, and joining keeps a replayed tool result attached to the turn
                // that asked for it instead of arriving as a bare interjection.
                match messages.last_mut() {
                    Some(last) if last["role"] == who => {
                        let joined = format!(
                            "{}\n\n{}",
                            last["content"].as_str().unwrap_or_default(),
                            message.content
                        );
                        last["content"] = serde_json::Value::from(joined);
                    }
                    _ => messages
                        .push(serde_json::json!({ "role": who, "content": message.content })),
                }
            }
        }
    }

    // The API requires the first turn to be the user's. A conversation that opens with the
    // assistant is one the Composer should not have built, and refusing it here would fail a
    // turn over an ordering nobody can see — so it is carried as context instead.
    if messages
        .first()
        .map(|m| m["role"] == "assistant")
        .unwrap_or(false)
    {
        messages.insert(
            0,
            serde_json::json!({ "role": "user", "content": "(continuing)" }),
        );
    }

    (system, messages)
}

/// One capability, in Anthropic's tool shape.
fn tool(descriptor: &Descriptor) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    let mut required = Vec::new();

    for parameter in &descriptor.parameters {
        properties.insert(
            parameter.name.clone(),
            serde_json::json!({
                "type": match parameter.kind {
                    ValueKind::Text => "string",
                    ValueKind::Integer => "integer",
                    ValueKind::Number => "number",
                    ValueKind::Boolean => "boolean",
                },
                "description": parameter.description,
            }),
        );
        if parameter.required {
            required.push(parameter.name.clone());
        }
    }

    serde_json::json!({
        "name": descriptor.id.as_str(),
        "description": descriptor.summary,
        "input_schema": {
            "type": "object",
            "properties": properties,
            "required": required,
        },
    })
}

/// The canonical sampling parameters, in this backend's spelling.
///
/// Returned rather than applied so the caller can drop the ones this model refuses without
/// this function needing to know which those are.
fn sampling(parameters: &Parameters) -> Vec<(&'static str, serde_json::Value)> {
    let mut fields = Vec::new();
    if let Some(t) = parameters.temperature {
        fields.push(("temperature", serde_json::Value::from(t)));
    }
    if let Some(p) = parameters.top_p {
        fields.push(("top_p", serde_json::Value::from(p)));
    }
    // `context_tokens` is deliberately absent. It is a request to hold *less* than the model's
    // window, and this API has nowhere to say that — the window is the window. The Composer
    // already honours the authored value when it builds the conversation, which is where the
    // parameter does its work; sending it here would be a second, contradictory place.
    fields
}

/// How much this character deliberates, translated.
///
/// `None` means the author said nothing, and the service's own default stands — which is not
/// the same as asking for none.
fn deliberation(reasoning: Option<Reasoning>) -> Option<(serde_json::Value, Option<&'static str>)> {
    match reasoning? {
        Reasoning::Off => Some((serde_json::json!({ "type": "disabled" }), None)),
        Reasoning::Low => Some((adaptive(), Some("low"))),
        Reasoning::Medium => Some((adaptive(), Some("medium"))),
        Reasoning::High => Some((adaptive(), Some("high"))),
        // Mapped down, deliberately. This API's effort levels stop at `high` before `max`, and
        // the CLI's `xhigh` is a rung it does not offer — so a character set there deliberates
        // as hard as this backend can, rather than being sent a word it would reject. The rung
        // is not lost: it means something exact on the backend that has it.
        Reasoning::XHigh => Some((adaptive(), Some("high"))),
        Reasoning::Max => Some((adaptive(), Some("max"))),
    }
}

fn adaptive() -> serde_json::Value {
    serde_json::json!({ "type": "adaptive" })
}

/// Which of our fields a refusal blamed.
///
/// Only fields this Provider can actually drop. A refusal about anything else is returned
/// unchanged, because retrying without something we never sent would just spend another round
/// trip to fail the same way.
fn blamed(detail: &str) -> Vec<&'static str> {
    ["temperature", "top_p", "thinking"]
        .into_iter()
        .filter(|field| detail.contains(field))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use epoch_kernel::Message;

    fn said(role: Role, text: &str) -> Message {
        Message {
            role,
            content: text.to_owned(),
            calls: Vec::new(),
            tool: None,
        }
    }

    #[test]
    fn the_system_prompt_is_hoisted_out_of_the_conversation() {
        // There is no system role in this API's `messages`. Sending one as a turn is the
        // mistake this translation exists to prevent.
        let conversation = Conversation {
            messages: vec![
                said(Role::System, "You are Mage."),
                said(Role::System, "You may use these tools."),
                said(Role::User, "Hello"),
            ],
        };
        let (system, messages) = split(&conversation);

        assert_eq!(system, "You are Mage.\n\nYou may use these tools.");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn evidence_replays_as_a_user_turn_joined_to_the_one_that_asked_for_it() {
        // There is no tool role either, and Epoch's record of a tool result is text with no
        // `tool_use_id` to pair a structured result with (ADR-0025).
        let conversation = Conversation {
            messages: vec![
                said(Role::User, "read the file"),
                said(Role::Assistant, "let me look"),
                said(Role::Tool, "[file] 12 lines"),
                said(Role::User, "and?"),
            ],
        };
        let (_, messages) = split(&conversation);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[2]["role"], "user");
        assert_eq!(messages[2]["content"], "[file] 12 lines\n\nand?");
    }

    #[test]
    fn a_conversation_that_opens_with_the_assistant_is_carried_rather_than_refused() {
        // Failing a turn over an ordering nobody can see would be a worse answer than making
        // the first turn a real one.
        let conversation = Conversation {
            messages: vec![said(Role::Assistant, "…"), said(Role::User, "hi")],
        };
        let (_, messages) = split(&conversation);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn context_tokens_is_not_sent() {
        // It asks the model to hold *less*, and this API has nowhere to say that. The Composer
        // already honours it where it does the work.
        let parameters = Parameters {
            temperature: Some(0.2),
            top_p: None,
            context_tokens: Some(8192),
            context_policy: None,
            reasoning: None,
        };
        let fields: Vec<&str> = sampling(&parameters).into_iter().map(|(f, _)| f).collect();
        assert_eq!(fields, vec!["temperature"]);
    }

    #[test]
    fn a_refused_field_is_dropped_from_the_next_attempt() {
        // Measured rather than tabulated: a list of which models reject `temperature` would be
        // right the day it was written and wrong afterwards.
        let named = blamed("temperature: Extra inputs are not permitted");
        assert_eq!(named, vec!["temperature"]);

        let parameters = Parameters {
            temperature: Some(0.2),
            top_p: Some(0.9),
            context_tokens: None,
            context_policy: None,
            reasoning: None,
        };
        let refuses: Refuses = named.into_iter().collect();
        let kept: Vec<&str> = sampling(&parameters)
            .into_iter()
            .map(|(f, _)| f)
            .filter(|f| !refuses.contains(f))
            .collect();
        assert_eq!(kept, vec!["top_p"], "only what was blamed is dropped");
    }

    #[test]
    fn a_refusal_about_something_else_is_not_retried() {
        // Retrying without something we never sent spends a round trip to fail the same way.
        assert!(blamed("credit balance is too low").is_empty());
    }

    #[test]
    fn deliberating_off_is_different_from_saying_nothing() {
        assert!(
            deliberation(None).is_none(),
            "the service's own default stands"
        );
        let (thinking, effort) = deliberation(Some(Reasoning::Off)).unwrap();
        assert_eq!(thinking["type"], "disabled");
        assert_eq!(effort, None);

        let (thinking, effort) = deliberation(Some(Reasoning::Max)).unwrap();
        assert_eq!(thinking["type"], "adaptive");
        assert_eq!(effort, Some("max"));
    }

    #[test]
    fn a_backend_with_no_credential_says_so_rather_than_failing() {
        // The ordinary first state of a configured hosted backend. The note says what to do.
        let status = Anthropic::named("anthropic", ENDPOINT).probe();
        assert!(!status.online);
        assert!(!status.local);
        assert!(status.note.unwrap().contains("credential"));
        assert!(status.models.is_empty());
    }

    #[test]
    fn a_turn_without_a_credential_is_refused_before_reaching_the_network() {
        let provider = Anthropic::named("anthropic", ENDPOINT);
        let request = Request {
            model: "claude-opus-5".into(),
            conversation: Conversation {
                messages: vec![said(Role::User, "hi")],
            },
            keep_loaded: crate::provider::KeepLoaded::Never,
            parameters: Parameters::default(),
            tuning: BTreeMap::new(),
            tools: Vec::new(),
            // The built-in bound: a test is not the place to decide how patient this machine is.
            most_rounds: None,
        };
        let refused = provider.take_turn(&request, &mut |_| {}).unwrap_err();
        assert!(refused.to_string().contains("credential"));
    }

    #[test]
    fn a_key_never_appears_in_a_debug_line() {
        // A Provider is the thing most likely to end up in a log, so its own Debug is written
        // rather than derived.
        let provider =
            Anthropic::named("anthropic", ENDPOINT).with_key(Some(Secret::new("sk-ant-secret")));
        let printed = format!("{provider:?}");
        assert!(!printed.contains("sk-ant"));
        assert!(printed.contains("keyed: true"));
    }
}
